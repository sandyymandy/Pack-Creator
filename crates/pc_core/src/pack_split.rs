//! Converting a whole pack to one model per participant, in place. The male rig (`steve`) is taken out of the girls' models into a model of its own, every scene file is split
//! between the girl and the male and written as a role folder with a layout file, `steve` is taken out of the girls' models and out of their base animations, and the old files
//! are replaced. Every file that is changed, removed or made is kept or listed in a folder of its own (`.packcreator/splits/<stamp>`), so the whole conversion can be undone.
//!
//! A scene file is only converted when the proof says every bone is posed exactly as before. A girl whose male rig differs from the rest gets a male model of her own, and
//! is reported. If any file cannot be converted, no model is stripped (the old scene files that remain still need `steve`).

use crate::layout::ASSET_SYNC;
use crate::scene_split::{self, MALE_ID, MALE_ROOT};
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
    /// Old male bones that had no counterpart in the base mod's male model, and in how many scene files they were animated.
    pub dropped_bones: Vec<(String, usize)>,
    /// Bones mapped as closely as the new model allows, and how far they are from where they were (the largest difference of a world matrix).
    pub approximate: Vec<(String, f64)>,
    /// Files left as they were, and why.
    pub skipped: Vec<String>,
    pub findings: Vec<String>,
    /// The files written and the files removed.
    pub written: Vec<String>,
    pub removed: Vec<String>,
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

/// The model without the male's rig, with its skeleton id set. Everything else (bones, cubes, meshes, skinning, textures, other extension sections) is carried over byte for
/// byte; only the male's bones and cubes are cut out and the numbers that point at bones and cubes are renumbered.
fn strip_male(bytes: &[u8], model: &Model, skeleton_id: &str) -> std::result::Result<Vec<u8>, String> {
    let gone: HashSet<usize> = scene_split::subtree(model, MALE_ROOT).into_iter().collect();
    let without = strut_model::remove_bones(bytes, &gone)?;

    strut_model::set_skeleton_id(&without, Some(skeleton_id))
}

#[derive(Clone, Copy, PartialEq)]
enum Owner {
    Girl,
    Body,
    Shared,
}

/// The body types a scene says it is for (`body_types` in its scene file), looked for in every namespace of the pack.
fn scene_bodies(root: &Path, scene: &str) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(root.join("data")) else { return Vec::new() };

    for entry in entries.flatten() {
        let file = entry.path().join("scenes").join(format!("{scene}.json"));

        if let Some(json) = std::fs::read_to_string(&file).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()) {
            return json.get("body_types").and_then(|b| b.as_array()).map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default();
        }
    }

    Vec::new()
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

    /* The girls' models that carry the male. */
    let mut models: Vec<GirlModel> = Vec::new();

    for rel in &files {
        let parts: Vec<&str> = rel.split('/').collect();

        if parts.len() < 7 || parts[0] != "data" || parts[2] != ASSET_SYNC || parts[3] != "strut" || parts[4] != "models" || !crate::paths::is_girl_model(&parts[4..].join("/")) {
            continue;
        }

        let Ok(bytes) = std::fs::read(root.join(rel)) else { continue };
        let Ok(model) = strut_model::read(&bytes) else { continue };

        if model.group_bones.iter().any(|b| b.name == MALE_ROOT && b.parent_index < 0) {
            models.push(GirlModel { rel: rel.clone(), stem: parts[parts.len() - 1].trim_end_matches(".strutmodel").to_string(), namespace: parts[1].to_string(), bytes, model });
        }
    }

    if models.is_empty() {
        report.findings.push("No scenes need splitting: no model carries the old male rig (steve).".into());
        return Ok(report);
    }

    let namespace = models[0].namespace.clone();
    let shared_id = MALE_ID.to_string();
    let mut dropped: BTreeMap<String, usize> = BTreeMap::new();
    let mut approximate: BTreeMap<String, f64> = BTreeMap::new();

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

        if !clips.iter().any(|c| c.channels.iter().any(|ch| ch.target == MALE_ROOT)) && !in_scenes {
            continue;
        }

        /* Who it is for: base.<girl>, base, <scene>-<girl>, <scene>.<skeleton>, or <scene> (everyone). */
        let (scene, owner, kind) = if name == "base" {
            (None, String::new(), Owner::Shared)
        } else if let Some(girl) = name.strip_prefix("base.") {
            (None, girl.to_string(), Owner::Girl)
        } else if in_scenes {
            match name.rsplit_once('-').filter(|(_, g)| models.iter().any(|m| m.stem == *g)) {
                Some((scene, girl)) => (Some(scene.to_string()), girl.to_string(), Owner::Girl),
                None => match name.rsplit_once('.') {
                    Some((scene, body)) => (Some(scene.to_string()), body.to_string(), Owner::Body),
                    None => (Some(name.clone()), String::new(), Owner::Shared),
                },
            }
        } else {
            continue;
        };

        /* The skeletons it is written for: her own, the one it names, or (a file for everyone) the ones the scene is for. */
        let targets: Vec<(String, String)> = match kind {
            Owner::Girl => vec![(owner.clone(), owner.clone())],
            Owner::Body => vec![(owner.clone(), format!("{}:{}", crate::layout::GAME_NAMESPACE, owner))],
            Owner::Shared => {
                let listed = scene.as_deref().map(|s| scene_bodies(root, s)).unwrap_or_default();
                let mut all: Vec<String> = if listed.is_empty() { models.iter().filter_map(|m| id_of(&m.stem)).map(|id| id_path(&id).to_string()).collect() } else { listed };

                all.sort();
                all.dedup();
                all.into_iter().map(|b| (b.clone(), format!("{}:{}", crate::layout::GAME_NAMESPACE, b))).collect()
            }
        };

        if targets.is_empty() {
            report.skipped.push(format!("{rel}: no skeleton to write it for"));
            continue;
        }

        let mut produced: Vec<(String, Vec<u8>, Vec<u8>)> = Vec::new();
        let mut failed = false;

        for (target, wanted) in &targets {
            /* The model that was animated with it: hers, one of the skeleton named, or (when the skeleton has no girl in this pack) any with the male rig, which is all that the
               male's side and the proof need of it. */
            let stem = match kind {
                Owner::Girl => owner.clone(),
                _ => models.iter().find(|m| id_of(&m.stem).as_deref() == Some(wanted.as_str())).or_else(|| models.first()).map(|m| m.stem.clone()).unwrap_or_default(),
            };
            let Some(model) = models.iter().find(|m| m.stem == stem && m.rel.contains("/nude/")).or_else(|| models.iter().find(|m| m.stem == stem)) else {
                if scene.is_some() {
                    report.skipped.push(format!("{rel}: {stem} has no model with the male rig"));
                }

                failed = true;
                break;
            };
            let host_id = match kind {
                Owner::Girl => id_of(&stem),
                _ => Some(wanted.clone()),
            };
            let Some(host_id) = host_id else {
                report.skipped.push(format!("{rel}: no skeleton id for {stem}"));
                failed = true;
                break;
            };
            let done = match scene_split::split_file(&bytes, &model.model, &host_id) {
                Ok(d) => d,
                Err(e) => {
                    report.skipped.push(format!("{rel}: {e}"));
                    failed = true;
                    break;
                }
            };
            let (Ok(host_clips), Ok(male_clips)) = (strut_anim::read(&done.host), strut_anim::read(&done.male)) else {
                report.skipped.push(format!("{rel}: the split files did not read back"));
                failed = true;
                break;
            };
            let proof = scene_split::prove(&model.model, &clips, &host_clips, &male_clips, 1e-4);

            if let Some(first) = proof.mismatches.first() {
                report.skipped.push(format!("{rel}: converting it would move {} bones (first: {} in {} at tick {}), so it was left as it is", proof.mismatches.len(), first.bone, first.clip, first.tick));
                failed = true;
                break;
            }

            if produced.is_empty() {
                for bone in &done.dropped {
                    *dropped.entry(bone.clone()).or_default() += 1;
                }
            }

            for (bone, by) in proof.approximate {
                let worst = approximate.entry(bone).or_insert(0.0);

                *worst = worst.max(by);
            }

            produced.push((target.clone(), done.host, done.male));
        }

        if failed {
            continue;
        }

        match scene {
            None => {
                /* A base animation keeps the girl's half only: nobody plays the male there. */
                if let Some((_, host, _)) = produced.into_iter().next() {
                    writes.insert(rel.clone(), host);
                }
            }
            Some(scene) => {
                for (target, host, male) in produced {
                    let by_girl = kind == Owner::Girl;
                    let host_id = if by_girl { id_of(&target).unwrap_or_default() } else { format!("{}:{}", crate::layout::GAME_NAMESPACE, target) };
                    let host_rel = if by_girl { format!("{base}/animations/scenes/{scene}/host/owners/{target}.strutanim") } else { format!("{base}/animations/scenes/{scene}/host/skeletons/{}.strutanim", id_path(&host_id)) };
                    let entry = scenes.entry(scene.clone()).or_default();

                    entry.hosts.insert(host_id);
                    entry.males.push((target.clone(), by_girl, shared_id.clone(), male, clips.clone()));
                    writes.insert(host_rel, host);
                }

                deletes.push(rel.clone());
            }
        }
    }

    /* Male files and the layout of every scene. A scene that is already split (an earlier import, or an earlier conversion) keeps what it has: a girl whose male clips are
       the ones already there adds nothing, and the layout takes the new girls in. */
    for (scene, parts) in &scenes {
        let skeleton_rel = format!("{base}/animations/scenes/{scene}/male/skeletons/{}.strutanim", id_path(&shared_id));
        let existing_skeleton = std::fs::read(root.join(&skeleton_rel)).ok().and_then(|b| strut_anim::read(&b).ok());
        let reference = existing_skeleton.clone().unwrap_or_else(|| strut_anim::read(&parts.males[0].3).unwrap_or_default());

        if existing_skeleton.is_none() {
            writes.insert(skeleton_rel.clone(), parts.males[0].3.clone());
        }

        let mut differing = 0;

        for (owner, by_girl, id, bytes, _) in &parts.males {
            if clips_equal(&strut_anim::read(bytes).unwrap_or_default(), &reference) {
                continue;
            }

            let folder = if *by_girl { "owners" } else { "skeletons" };
            let file = if *by_girl { owner.clone() } else { id_path(id).to_string() };

            writes.insert(format!("{base}/animations/scenes/{scene}/male/{folder}/{file}.strutanim"), bytes.clone());
            differing += 1;
        }

        if differing > 0 {
            report.findings.push(format!("The scene {scene} has male motion that differs from girl to girl, so {differing} girls' male clips are written for them alone (male/owners)."));
        }

        let layout_rel = format!("{base}/scenes/{scene}.strutscene");
        let mut layout: serde_json::Value = std::fs::read(root.join(&layout_rel)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_else(|| {
            serde_json::json!({
                "roles": {
                    "host": { "clipSet": format!("scenes/{scene}/host"), "skeleton": parts.hosts.iter().next(), "compatible": [], "required": true },
                    "male": { "clipSet": format!("scenes/{scene}/male"), "skeleton": shared_id, "required": true }
                },
                "clips": {}
            })
        });

        if let Some(host) = layout.pointer_mut("/roles/host") {
            let main = host.get("skeleton").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let mut compatible: Vec<String> = host.get("compatible").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default();

            for id in &parts.hosts {
                if *id != main && !compatible.contains(id) {
                    compatible.push(id.clone());
                }
            }

            host["compatible"] = serde_json::json!(compatible);
        }

        if let Some(clips) = layout.get_mut("clips").and_then(|c| c.as_object_mut()) {
            for clip in &parts.males[0].4 {
                let loop_name = ["once", "loop", "hold"].get(clip.loop_mode as usize).copied().unwrap_or("once");

                clips.entry(clip.name.clone()).or_insert_with(|| serde_json::json!({ "length": (clip.duration * 20.0).round() as u32, "loop": loop_name }));
            }
        }

        writes.insert(layout_rel, serde_json::to_vec_pretty(&layout).unwrap_or_default());
        report.scenes += 1;
    }

    report.dropped_bones = dropped.into_iter().collect();
    report.approximate = approximate.into_iter().filter(|(_, by)| *by > 1e-4).collect();

    /* Strip the male from the girls' models, but only when every scene could be converted: a scene file left behind still needs it. */
    if report.skipped.is_empty() {
        for m in &models {
            let Some(id) = id_of(&m.stem) else { continue };

            match strip_male(&m.bytes, &m.model, &id) {
                Ok(bytes) => {
                    writes.insert(m.rel.clone(), bytes);
                    report.models_stripped += 1;
                }
                Err(e) => report.skipped.push(format!("{}: {e}", m.rel)),
            }
        }
    } else {
        report.findings.push("Some files could not be converted, so the girls' models keep the male rig for now.".into());
    }

    report.files_written = writes.len();
    report.files_removed = deletes.len();
    report.written = writes.keys().cloned().collect();
    report.removed = deletes.clone();

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
    /// Who it was for, whether by girl, the male's id, its half of the file, and the whole file's clips.
    males: Vec<(String, bool, String, Vec<u8>, Vec<Clip>)>,
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

        eprintln!("skipped {:?}\ndropped {:?}\napproximate {:?}", done.skipped, done.dropped_bones, done.approximate);

        let after = snapshot(&copy);

        assert!(after.keys().any(|k| k.contains("/male/") && k.ends_with(".strutanim")));
        assert!(!after.keys().any(|k| k.contains("models/male/")), "the pack writes a male model, but the base mod has it");
        assert!(after.keys().any(|k| k.contains("/scenes/") && k.ends_with(".strutscene")));
        assert!(after.keys().any(|k| k.contains("/host/owners/")));
        assert!(!after.keys().any(|k| k.contains("animations/scenes/") && !k.contains("/host/") && !k.contains("/male/")), "an old scene file is left");

        if done.skipped.is_empty() {
            for (rel, bytes) in &after {
                if crate::paths::is_girl_model(rel.split_once("/strut/").map_or("", |x| x.1)) && rel.contains("/nude/") {
                    let model = strut_model::read(bytes).unwrap();

                    assert!(!model.group_bones.iter().any(|b| b.name == MALE_ROOT), "{rel} still has the male");
                    assert!(model.skeleton_id.is_some(), "{rel} has no skeleton id");
                }
            }
        }

        /* Running it again finds nothing left to convert. */
        let again = split_pack(&copy, true).unwrap();

        assert_eq!((again.scenes, again.files_written, again.models_stripped), (0, 0, 0), "a second run changed {:?}", again.written);

        let undone = undo_split(&copy, done.stamp.as_deref().unwrap()).unwrap();

        assert!(undone > 0);
        assert_eq!(snapshot(&copy), before, "undo did not restore the pack");
        let _ = std::fs::remove_dir_all(&copy);
    }

    /// Ports the base mod's own girls (the game's resources, which are laid out as `assets/` and `data/` rather than as a pack) to the new system: the resources are staged as a
    /// pack, updated, and every file that changed is copied back. Run with `PH_RESOURCES=<path to src/main/resources> PH_WRITE=1 cargo test -p pc_core -- --ignored port_base_mod --nocapture`;
    /// without `PH_WRITE` it only says what it would do.
    #[test]
    #[ignore]
    fn port_base_mod() {
        let resources = std::path::PathBuf::from(std::env::var("PH_RESOURCES").expect("PH_RESOURCES"));
        let stage = std::env::temp_dir().join("pc-port-base-mod");
        let _ = std::fs::remove_dir_all(&stage);

        let pack = stage.join("data/pleasurehorizons");

        copy_dir(&resources.join("assets/pleasurehorizons/strut"), &pack.join("asset_sync/strut"));
        copy_dir(&resources.join("data/pleasurehorizons/girls"), &pack.join("girls"));
        copy_dir(&resources.join("data/pleasurehorizons/scenes"), &pack.join("scenes"));
        std::fs::write(stage.join("pack.mcmeta"), "{\"pack\":{\"pack_format\":48,\"description\":\"staged\"}}").unwrap();

        let before = snapshot(&stage);
        let report = crate::migrate::update(&stage, true).unwrap();

        println!("scenes {} written {} removed {} stripped {}", report.scenes, report.files_written, report.files_removed, report.models_stripped);
        println!("skipped {:#?}", report.skipped);
        println!("dropped {:?}", report.dropped_bones);
        println!("approximate {:?}", report.approximate);
        println!("findings {:#?}", report.findings);

        let after = snapshot(&stage);

        /* Every model that was stripped keeps what it had: each remaining bone is where it was, its meshes and armatures are as they were and hang off the same bones. */
        for (rel, bytes) in &after {
            let Some(old) = before.get(rel).filter(|o| *o != bytes && rel.ends_with(".strutmodel")) else { continue };
            let (a, b) = (strut_model::read(old).unwrap(), strut_model::read(bytes).unwrap());
            let (wa, wb) = (crate::pose::world_matrices(&a, None, 0.0), crate::pose::world_matrices(&b, None, 0.0));

            for bone in &b.group_bones {
                assert!(crate::pose::distance(&wa[&bone.name], &wb[&bone.name]) < 1e-9, "{rel}: {} moved", bone.name);
            }

            assert_eq!(a.meshes.len(), b.meshes.len(), "{rel}");
            assert_eq!(a.cubes.len() - scene_split::subtree(&a, MALE_ROOT).iter().map(|i| a.group_bones[*i].elements.len()).sum::<usize>(), b.cubes.len(), "{rel}");

            for (x, y) in a.armatures.iter().zip(&b.armatures) {
                let name = |m: &Model, i: i16| (i >= 0).then(|| m.group_bones[i as usize].name.clone());

                assert_eq!(name(&a, x.parent_group_bone), name(&b, y.parent_group_bone), "{rel}: an armature moved to another bone");
            }
        }

        let ours = |rel: &str| rel.strip_prefix("data/pleasurehorizons/asset_sync/strut/").map(|r| resources.join("assets/pleasurehorizons/strut").join(r));
        let (mut wrote, mut removed) = (0, 0);

        for (rel, bytes) in &after {
            if before.get(rel) != Some(bytes) {
                if let Some(target) = ours(rel) {
                    wrote += 1;
                    println!("changes {rel}");

                    if std::env::var("PH_WRITE").is_ok() {
                        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
                        std::fs::write(&target, bytes).unwrap();
                    }
                }
            }
        }

        for rel in before.keys() {
            if !after.contains_key(rel) {
                if let Some(target) = ours(rel) {
                    removed += 1;

                    if std::env::var("PH_WRITE").is_ok() {
                        let _ = std::fs::remove_file(&target);
                    }
                }
            }
        }

        println!("would change {wrote} files and remove {removed} (written: {})", std::env::var("PH_WRITE").is_ok());
        let _ = std::fs::remove_dir_all(&stage);
    }

    /// Takes the skeleton id back off models that were given one by mistake (armour, blocks, items), in a resources folder: `PH_RESOURCES=<path> cargo test -p pc_core -- --ignored take_ids_off`.
    #[test]
    #[ignore]
    fn take_ids_off() {
        let resources = std::path::PathBuf::from(std::env::var("PH_RESOURCES").expect("PH_RESOURCES")).join("assets/pleasurehorizons/strut/models");
        let mut count = 0;

        for folder in ["armour", "block", "item"] {
            let mut stack = vec![resources.join(folder)];

            while let Some(dir) = stack.pop() {
                for entry in std::fs::read_dir(dir).unwrap().flatten() {
                    let path = entry.path();

                    if path.is_dir() {
                        stack.push(path);
                    } else if path.extension().map_or(false, |e| e == "strutmodel") {
                        let bytes = std::fs::read(&path).unwrap();

                        if strut_model::read(&bytes).unwrap().skeleton_id.is_some() {
                            std::fs::write(&path, strut_model::set_skeleton_id(&bytes, None).unwrap()).unwrap();
                            count += 1;
                        }
                    }
                }
            }
        }

        println!("took the id off {count} models");
    }
}
