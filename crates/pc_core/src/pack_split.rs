//! Converting a whole pack to one model per participant, in place. The partner rig (`steve`) is taken out of the girls' models into a model of its own, every scene file is split
//! between the girl and the partner and written as a role folder with a layout file, `steve` is taken out of the girls' models and out of their base animations, and the old files
//! are replaced. Every file that is changed, removed or made is kept or listed in a folder of its own (`.packcreator/splits/<stamp>`), so the whole conversion can be undone.
//!
//! A scene file is only converted when the proof says every bone is posed exactly as before. A girl whose partner rig differs from the rest gets a partner model of her own, and
//! is reported. If any file cannot be converted, no model is stripped (the old scene files that remain still need `steve`).

use crate::convert::model as raw;
use crate::layout::ASSET_SYNC;
use crate::scene_split::{self, Partner, PARTNER_ROOT};
use crate::skeleton_ids;
use crate::strut_anim::{self, Clip};
use crate::strut_model::{self, Model};
use crate::{fs_ops, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::path::Path;

/// What a conversion did, or would do.
#[derive(Debug, Clone, Serialize, Default)]
pub struct Report {
    pub applied: bool,
    /// The folder the undo information is in, when applied.
    pub stamp: Option<String>,
    pub scenes: usize,
    pub files_written: usize,
    pub files_removed: usize,
    pub models_stripped: usize,
    /// The partner models made, and which girls use each.
    pub partners: Vec<PartnerUse>,
    /// Girls whose partner rig differs from the shared one, and how.
    pub differences: Vec<String>,
    /// Files left as they were, and why.
    pub skipped: Vec<String>,
    pub findings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PartnerUse {
    pub rel: String,
    pub id: String,
    pub girls: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Manifest {
    created: Vec<String>,
    replaced: Vec<String>,
    deleted: Vec<String>,
}

struct GirlModel {
    rel: String,
    stem: String,
    namespace: String,
    bytes: Vec<u8>,
    model: Model,
}

/// What a bone's place in the rig looks like, relative to the rig's root, so two copies of the rig can be compared wherever they sit.
fn signature(model: &Model, root: &str) -> Vec<String> {
    let members = scene_split::subtree(model, root);
    let Some(first) = members.first() else { return Vec::new() };
    let base = model.group_bones[*first].pivot;
    let rel = |v: [f32; 3]| format!("{:.3},{:.3},{:.3}", v[0] - base[0], v[1] - base[1], v[2] - base[2]);

    members
        .iter()
        .map(|i| {
            let b = &model.group_bones[*i];
            let parent = if b.parent_index >= 0 { model.group_bones[b.parent_index as usize].name.as_str() } else { "" };
            let cubes: Vec<String> = b
                .elements
                .iter()
                .map(|e| {
                    let c = &model.cubes[e.index as usize];

                    format!("{}[{} {} {} r{:?} {:?}]", c.name, rel(c.origin), rel(c.from), rel(c.to), c.rotation, c.faces.iter().map(|f| (f.face_id, f.u1, f.v1, f.u2, f.v2, f.uv_rotation)).collect::<Vec<_>>())
                })
                .collect();

            format!("{}|{}|{}|{:?}|{}", b.name, parent, rel(b.pivot), b.rotation, cubes.join(";"))
        })
        .collect()
}

/// Says how a girl's copy of the rig differs from the shared one, bone by bone.
fn differences(shared: &[String], hers: &[String]) -> Vec<String> {
    let name = |line: &String| line.split('|').next().unwrap_or("").to_string();
    let shared_names: HashSet<String> = shared.iter().map(name).collect();
    let her_names: HashSet<String> = hers.iter().map(name).collect();
    let mut out = Vec::new();

    for line in hers {
        if !shared_names.contains(&name(line)) {
            out.push(format!("adds {}", name(line)));
        } else if !shared.contains(line) {
            out.push(format!("changes {}", name(line)));
        }
    }

    for line in shared {
        if !her_names.contains(&name(line)) {
            out.push(format!("lacks {}", name(line)));
        }
    }

    out
}

fn extension_payloads(bytes: &[u8], model: &Model) -> Result<Vec<(String, Vec<u8>)>> {
    let mut sections = Vec::new();
    let flags = u16::from_le_bytes([bytes[6], bytes[7]]);

    if flags & 1 != 0 {
        let mut at = model.offsets.extensions_at;
        let u16_at = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]) as usize;
        let count = u16_at(at);

        at += 2;

        for _ in 0..count {
            let name_length = u16_at(at);

            at += 2;

            let name = String::from_utf8_lossy(&bytes[at..at + name_length]).to_string();

            at += name_length;

            let length = u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]) as usize;

            at += 4;
            sections.push((name, bytes[at..at + length].to_vec()));
            at += length;
        }
    }

    Ok(sections)
}

/// The model without the partner's rig, with its skeleton id set. Everything else (bones, cubes, textures, other extension sections) is carried over as it was. A model with
/// meshes or armatures is refused: the converter writes cubes only.
fn strip_partner(bytes: &[u8], model: &Model, skeleton_id: &str) -> std::result::Result<Vec<u8>, String> {
    if !model.meshes.is_empty() || !model.armatures.is_empty() {
        return Err("the model has meshes or armatures, which the stripper does not carry".into());
    }

    let gone: HashSet<usize> = scene_split::subtree(model, PARTNER_ROOT).into_iter().collect();
    let keep: Vec<usize> = (0..model.group_bones.len()).filter(|i| !gone.contains(i)).collect();
    let mut new_index = vec![-1i32; model.group_bones.len()];

    for (n, old) in keep.iter().enumerate() {
        new_index[*old] = n as i32;
    }

    let mut cube_map: BTreeMap<usize, u32> = BTreeMap::new();
    let mut cubes = Vec::new();
    let mut strings: Vec<String> = model.textures.iter().map(|t| t.name.clone()).collect();
    let mut group_bones = Vec::new();

    for old in &keep {
        let bone = &model.group_bones[*old];
        let mut elements = Vec::new();

        for element in &bone.elements {
            let index = *cube_map.entry(element.index as usize).or_insert_with(|| {
                let c = &model.cubes[element.index as usize];

                cubes.push(raw::Cube {
                    name: c.name.clone(),
                    texture: c.texture_index as u32,
                    origin: c.origin.map(|v| v as f64),
                    rotation: c.rotation.map(|v| v as f64),
                    from: c.from.map(|v| v as f64),
                    to: c.to.map(|v| v as f64),
                    inflate: c.inflate as f64,
                    mirror: c.mirror,
                    box_uv: c.box_uv,
                    faces: c.faces.iter().map(|f| raw::CubeFace { face_id: f.face_id as u32, u1: f.u1 as f64, v1: f.v1 as f64, u2: f.u2 as f64, v2: f.v2 as f64, uv_rotation: f.uv_rotation as i64 }).collect(),
                });
                strings.push(c.name.clone());
                (cubes.len() - 1) as u32
            });

            elements.push(raw::Element { kind: 1, index });
        }

        strings.push(bone.name.clone());
        group_bones.push(raw::GroupBone { name: bone.name.clone(), parent: if bone.parent_index >= 0 { new_index[bone.parent_index as usize] } else { -1 }, pivot: bone.pivot.map(|v| v as f64), rotation: bone.rotation.map(|v| v as f64), elements });
    }

    strings.sort();
    strings.dedup();

    let mut extensions = extension_payloads(bytes, model).map_err(|e| e.to_string())?;

    extensions.retain(|(name, _)| name != strut_model::SKELETON_EXTENSION);

    let mut payload = (skeleton_id.len() as u16).to_le_bytes().to_vec();

    payload.extend_from_slice(skeleton_id.as_bytes());
    extensions.insert(0, (strut_model::SKELETON_EXTENSION.to_string(), payload));

    let rebuilt = raw::RawModel {
        model_id: None,
        bounds_min: model.bounds_min.map(|v| v as f64),
        bounds_max: model.bounds_max.map(|v| v as f64),
        strings,
        textures: model.textures.iter().map(|t| raw::Texture { name: t.name.clone(), width: t.width as f64, height: t.height as f64, uv_width: t.uv_width as f64, uv_height: t.uv_height as f64 }).collect(),
        group_bones,
        cubes,
        extensions,
    };
    let out = raw::encode(&rebuilt)?;
    let back = strut_model::read(&out).map_err(|e| format!("the stripped model did not read back: {e}"))?;

    if back.group_bones.len() != keep.len() || back.cubes.len() != cubes_len(model, &keep) {
        return Err("the stripped model lost bones or cubes it should have kept".into());
    }

    Ok(out)
}

fn cubes_len(model: &Model, keep: &[usize]) -> usize {
    keep.iter().flat_map(|i| model.group_bones[*i].elements.iter().map(|e| e.index)).collect::<HashSet<_>>().len()
}

fn id_path(id: &str) -> &str {
    id.split_once(':').map_or(id, |(_, path)| path)
}

/// Converts the pack (when `apply`), or only says what it would do.
pub fn split_pack(root: &Path, apply: bool) -> Result<Report> {
    let mut report = Report { applied: apply, ..Default::default() };
    let files = crate::check::walk(root);
    let ids = skeleton_ids::plan(root, &BTreeMap::new());
    let id_of = |owner: &str| ids.assignments.iter().find(|a| a.kind == skeleton_ids::Kind::Model && a.owner == owner).map(|a| a.id.clone());

    /* The girls' models that carry the partner. */
    let mut models: Vec<GirlModel> = Vec::new();

    for rel in &files {
        let parts: Vec<&str> = rel.split('/').collect();

        if parts.len() < 7 || parts[0] != "data" || parts[2] != ASSET_SYNC || parts[3] != "strut" || parts[4] != "models" || parts[5] == "partner" || !rel.ends_with(".strutmodel") {
            continue;
        }

        let Ok(bytes) = std::fs::read(root.join(rel)) else { continue };
        let Ok(model) = strut_model::read(&bytes) else { continue };

        if model.group_bones.iter().any(|b| b.name == PARTNER_ROOT && b.parent_index < 0) {
            models.push(GirlModel { rel: rel.clone(), stem: parts[parts.len() - 1].trim_end_matches(".strutmodel").to_string(), namespace: parts[1].to_string(), bytes, model });
        }
    }

    if models.is_empty() {
        report.findings.push("No model in this pack carries the partner rig (steve): there is nothing to split.".into());
        return Ok(report);
    }

    /* The shared partner is the rig of the first model; a girl whose rig differs gets a partner of her own. */
    let namespace = models[0].namespace.clone();
    let shared_id = skeleton_ids::PARTNER_ID.to_string();
    let shared = scene_split::extract_partner(&models[0].model, PARTNER_ROOT, &shared_id).map_err(crate::Error::invalid)?;
    let shared_signature = signature(&models[0].model, PARTNER_ROOT);
    let mut partner_of: BTreeMap<String, (String, Partner)> = BTreeMap::new();
    let mut own: BTreeMap<String, Partner> = BTreeMap::new();

    for m in models.iter().filter(|m| m.rel.contains("/nude/")).chain(models.iter().filter(|m| !m.rel.contains("/nude/"))) {
        if partner_of.contains_key(&m.stem) {
            continue;
        }

        let hers = signature(&m.model, PARTNER_ROOT);
        let diff = differences(&shared_signature, &hers);

        if diff.is_empty() {
            partner_of.insert(m.stem.clone(), (shared_id.clone(), clone_partner(&shared)));
        } else {
            let id = format!("{}:partner_{}", m.namespace, m.stem);
            let partner = scene_split::extract_partner(&m.model, PARTNER_ROOT, &id).map_err(crate::Error::invalid)?;

            report.differences.push(format!("{}'s partner rig differs from the shared one ({}), so she gets a partner model of her own ({id}).", m.stem, diff.iter().take(8).cloned().collect::<Vec<_>>().join(", ")));
            own.insert(m.stem.clone(), clone_partner(&partner));
            partner_of.insert(m.stem.clone(), (id, partner));
        }
    }

    let base = format!("data/{namespace}/{ASSET_SYNC}/strut");
    let mut writes: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut deletes: Vec<String> = Vec::new();
    let mut scenes: BTreeMap<String, SceneParts> = BTreeMap::new();

    /* Scene files and base animations. */
    for rel in &files {
        let parts: Vec<&str> = rel.split('/').collect();

        if parts.len() < 6 || parts[0] != "data" || parts[2] != ASSET_SYNC || parts[3] != "strut" || parts[4] != "animations" || !rel.ends_with(".strutanim") {
            continue;
        }

        let name = parts[parts.len() - 1].trim_end_matches(".strutanim").to_string();
        let in_scenes = parts.len() == 7 && parts[5] == "scenes";
        let Ok(bytes) = std::fs::read(root.join(rel)) else { continue };
        let Ok(clips) = strut_anim::read(&bytes) else { continue };

        if !clips.iter().any(|c| c.channels.iter().any(|ch| ch.target == PARTNER_ROOT || partner_of.values().any(|(_, p)| p.bones.contains(&ch.target)))) && !in_scenes {
            continue;
        }

        /* Who it is for: base.<girl>, <scene>-<girl>, or <scene>.<skeleton>. */
        let (scene, owner, by_girl) = if let Some(girl) = name.strip_prefix("base.") {
            (None, girl.to_string(), true)
        } else if in_scenes {
            match name.rsplit_once('-').filter(|(_, g)| partner_of.contains_key(*g)) {
                Some((scene, girl)) => (Some(scene.to_string()), girl.to_string(), true),
                None => match name.rsplit_once('.') {
                    Some((scene, body)) => (Some(scene.to_string()), body.to_string(), false),
                    None => continue,
                },
            }
        } else {
            continue;
        };

        /* The model that was animated with it. */
        let stem = if by_girl {
            owner.clone()
        } else {
            let wanted = format!("{}:{}", crate::layout::GAME_NAMESPACE, owner);

            match models.iter().find(|m| id_of(&m.stem).as_deref() == Some(wanted.as_str())) {
                Some(m) => m.stem.clone(),
                None => {
                    report.skipped.push(format!("{rel}: no model of the skeleton {owner} to split it with"));
                    continue;
                }
            }
        };
        let Some(model) = models.iter().find(|m| m.stem == stem && m.rel.contains("/nude/")).or_else(|| models.iter().find(|m| m.stem == stem)) else {
            if scene.is_some() {
                report.skipped.push(format!("{rel}: {stem} has no model with the partner rig"));
            }

            continue;
        };
        let Some(host_id) = id_of(&stem) else {
            report.skipped.push(format!("{rel}: no skeleton id for {stem}"));
            continue;
        };
        let (partner_id, partner) = &partner_of[&stem];
        let done = match scene_split::split_file(&bytes, partner, PARTNER_ROOT, &host_id, partner_id) {
            Ok(d) => d,
            Err(e) => {
                report.skipped.push(format!("{rel}: {e}"));
                continue;
            }
        };
        let (Ok(partner_model), Ok(host_clips), Ok(partner_clips)) = (strut_model::read(&partner.bytes), strut_anim::read(&done.host), strut_anim::read(&done.partner)) else {
            report.skipped.push(format!("{rel}: the split files did not read back"));
            continue;
        };
        let mismatches = scene_split::prove(&model.model, &clips, &partner_model, &host_clips, &partner_clips, &partner.bones, 1e-4);

        if let Some(first) = mismatches.first() {
            report.skipped.push(format!("{rel}: converting it would move {} bones (first: {} in {} at tick {}), so it was left as it is", mismatches.len(), first.bone, first.clip, first.tick));
            continue;
        }

        match scene {
            None => {
                /* A base animation keeps the girl's half only: nobody plays the partner there. */
                writes.insert(rel.clone(), done.host);
            }
            Some(scene) => {
                let host_rel = if by_girl { format!("{base}/animations/scenes/{scene}/host/owners/{owner}.strutanim") } else { format!("{base}/animations/scenes/{scene}/host/skeletons/{}.strutanim", id_path(&host_id)) };
                let entry = scenes.entry(scene.clone()).or_default();

                entry.hosts.insert(host_id.clone());
                entry.partners.push((owner.clone(), by_girl, partner_id.clone(), done.partner, clips.clone()));
                writes.insert(host_rel, done.host);
                deletes.push(rel.clone());
            }
        }
    }

    /* Partner files and the layout of every scene. */
    for (scene, parts) in &scenes {
        let first = &parts.partners[0];
        let all_same_shared = parts.partners.iter().all(|p| p.2 == shared_id) && parts.partners.iter().all(|p| clips_equal(&strut_anim::read(&p.3).unwrap_or_default(), &strut_anim::read(&first.3).unwrap_or_default()));

        if all_same_shared {
            writes.insert(format!("{base}/animations/scenes/{scene}/partner/skeletons/{}.strutanim", id_path(&shared_id)), first.3.clone());
        } else {
            for (owner, by_girl, id, bytes, _) in &parts.partners {
                let folder = if *by_girl { "owners" } else { "skeletons" };
                let file = if *by_girl { owner.clone() } else { id_path(id).to_string() };

                writes.insert(format!("{base}/animations/scenes/{scene}/partner/{folder}/{file}.strutanim"), bytes.clone());
            }

            report.findings.push(format!("The scene {scene} has partner motion that differs from girl to girl, so the partner's clips are written per girl (partner/owners)."));
        }

        let hosts: Vec<&String> = parts.hosts.iter().collect();
        let mut clip_lengths = serde_json::Map::new();

        for clip in &first.4 {
            let loop_name = ["once", "loop", "hold"].get(clip.loop_mode as usize).copied().unwrap_or("once");

            clip_lengths.insert(clip.name.clone(), serde_json::json!({ "length": (clip.duration * 20.0).round() as u32, "loop": loop_name }));
        }

        let layout = serde_json::json!({
            "roles": {
                "host": { "clipSet": format!("scenes/{scene}/host"), "skeleton": hosts[0], "compatible": hosts[1..], "required": true },
                "partner": { "clipSet": format!("scenes/{scene}/partner"), "skeleton": shared_id, "required": true }
            },
            "clips": clip_lengths
        });

        writes.insert(format!("{base}/scenes/{scene}.strutscene"), serde_json::to_vec_pretty(&layout).unwrap_or_default());
        report.scenes += 1;
    }

    /* Partner models. */
    writes.insert(format!("{base}/models/partner/default.strutmodel"), shared.bytes.clone());
    report.partners.push(PartnerUse { rel: format!("{base}/models/partner/default.strutmodel"), id: shared_id.clone(), girls: partner_of.iter().filter(|(_, (id, _))| *id == shared_id).map(|(g, _)| g.clone()).collect() });

    for (stem, partner) in &own {
        let rel = format!("{base}/models/partner/{stem}.strutmodel");

        writes.insert(rel.clone(), partner.bytes.clone());
        report.partners.push(PartnerUse { rel, id: partner_of[stem].0.clone(), girls: vec![stem.clone()] });
    }

    /* Strip the partner from the girls' models, but only when every scene could be converted: a scene file left behind still needs it. */
    if report.skipped.is_empty() {
        for m in &models {
            let Some(id) = id_of(&m.stem) else { continue };

            match strip_partner(&m.bytes, &m.model, &id) {
                Ok(bytes) => {
                    writes.insert(m.rel.clone(), bytes);
                    report.models_stripped += 1;
                }
                Err(e) => report.skipped.push(format!("{}: {e}", m.rel)),
            }
        }
    } else {
        report.findings.push("Some files could not be converted, so the girls' models keep the partner rig for now.".into());
    }

    report.files_written = writes.len();
    report.files_removed = deletes.len();

    if !apply {
        return Ok(report);
    }

    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0).to_string();
    let folder = root.join(crate::project::META_DIR).join("splits").join(&stamp);
    let mut manifest = Manifest::default();

    for rel in writes.keys().chain(deletes.iter()) {
        let path = root.join(rel);

        if path.is_file() {
            let keep = folder.join("files").join(rel);

            if let Some(parent) = keep.parent() {
                std::fs::create_dir_all(parent).map_err(|e| crate::Error::io(parent, e))?;
            }

            std::fs::copy(&path, &keep).map_err(|e| crate::Error::io(&path, e))?;

            if writes.contains_key(rel) {
                manifest.replaced.push(rel.clone());
            } else {
                manifest.deleted.push(rel.clone());
            }
        } else if writes.contains_key(rel) {
            manifest.created.push(rel.clone());
        }
    }

    std::fs::create_dir_all(&folder).map_err(|e| crate::Error::io(&folder, e))?;
    std::fs::write(folder.join("manifest.json"), serde_json::to_vec_pretty(&manifest).unwrap_or_default()).map_err(|e| crate::Error::io(&folder, e))?;

    for (rel, bytes) in &writes {
        if let Some(parent) = root.join(rel).parent() {
            std::fs::create_dir_all(parent).map_err(|e| crate::Error::io(parent, e))?;
        }

        fs_ops::write_bytes(root, rel, bytes)?;
    }

    for rel in &deletes {
        if !writes.contains_key(rel) {
            let _ = std::fs::remove_file(root.join(rel));
        }
    }

    report.stamp = Some(stamp);
    Ok(report)
}

#[derive(Default)]
struct SceneParts {
    hosts: std::collections::BTreeSet<String>,
    /// Who it was for, whether by girl, the partner's id, its half of the file, and the whole file's clips.
    partners: Vec<(String, bool, String, Vec<u8>, Vec<Clip>)>,
}

fn clone_partner(p: &Partner) -> Partner {
    Partner { bytes: p.bytes.clone(), bones: p.bones.clone(), shift: p.shift, fingerprint: p.fingerprint }
}

fn clips_equal(a: &[Clip], b: &[Clip]) -> bool {
    a == b
}

/// Puts back what a conversion changed: files it made are removed, files it replaced or removed are restored.
pub fn undo_split(root: &Path, stamp: &str) -> Result<usize> {
    let folder = root.join(crate::project::META_DIR).join("splits").join(stamp);
    let manifest: Manifest = serde_json::from_slice(&std::fs::read(folder.join("manifest.json")).map_err(|e| crate::Error::io(&folder, e))?).map_err(|e| crate::Error::invalid(format!("the undo information is damaged: {e}")))?;
    let mut count = 0;

    for rel in &manifest.created {
        let _ = std::fs::remove_file(root.join(rel));
        count += 1;
    }

    for rel in manifest.replaced.iter().chain(&manifest.deleted) {
        let target = root.join(rel);

        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| crate::Error::io(parent, e))?;
        }

        std::fs::copy(folder.join("files").join(rel), &target).map_err(|e| crate::Error::io(&target, e))?;
        count += 1;
    }

    /* Empty role folders left behind go too. */
    for rel in manifest.created.iter() {
        let mut dir = root.join(rel).parent().map(Path::to_path_buf);

        while let Some(d) = dir {
            if d == root || std::fs::remove_dir(&d).is_err() {
                break;
            }

            dir = d.parent().map(Path::to_path_buf);
        }
    }

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn copy_dir(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();

        for e in std::fs::read_dir(from).unwrap().flatten() {
            let p = e.path();

            if e.file_name().to_string_lossy().starts_with('.') {
                continue;
            }

            if p.is_dir() {
                copy_dir(&p, &to.join(e.file_name()));
            } else {
                std::fs::copy(&p, to.join(e.file_name())).unwrap();
            }
        }
    }

    fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
        crate::check::walk(root).into_iter().map(|r| (r.clone(), std::fs::read(root.join(&r)).unwrap())).collect()
    }

    #[test]
    fn converts_the_test_pack_in_place_and_undoes_it() {
        let source = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test_pack");

        if !source.join("data").is_dir() {
            return;
        }

        let copy = std::env::temp_dir().join(format!("pc-pack-split-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&copy);
        copy_dir(&source, &copy);

        let before = snapshot(&copy);
        let preview = split_pack(&copy, false).unwrap();

        assert!(preview.scenes > 20, "{} scenes", preview.scenes);
        assert_eq!(snapshot(&copy), before, "a preview changed files");

        let done = split_pack(&copy, true).unwrap();

        eprintln!("skipped {:?}\ndifferences {:?}\nfindings {:?}", done.skipped, done.differences, done.findings);

        let after = snapshot(&copy);

        assert!(after.keys().any(|k| k.ends_with("models/partner/default.strutmodel")));
        assert!(after.keys().any(|k| k.contains("/scenes/") && k.ends_with(".strutscene")));
        assert!(after.keys().any(|k| k.contains("/host/owners/")));
        assert!(!after.keys().any(|k| k.contains("animations/scenes/") && !k.contains("/host/") && !k.contains("/partner/")), "an old scene file is left");

        if done.skipped.is_empty() {
            for (rel, bytes) in &after {
                if rel.contains("/models/nude/") && rel.ends_with(".strutmodel") {
                    let model = strut_model::read(bytes).unwrap();

                    assert!(!model.group_bones.iter().any(|b| b.name == PARTNER_ROOT), "{rel} still has the partner");
                    assert!(model.skeleton_id.is_some(), "{rel} has no skeleton id");
                }
            }
        }

        let undone = undo_split(&copy, done.stamp.as_deref().unwrap()).unwrap();

        assert!(undone > 0);
        assert_eq!(snapshot(&copy), before, "undo did not restore the pack");
        let _ = std::fs::remove_dir_all(&copy);
    }
}
