//! Converting a pack to one model per participant. The old girl models carry the partner as a second root (`steve`) that the same clip animates. This takes that rig out into a
//! model of its own, splits the scene files between the girl and the partner, and proves that nothing moved: every bone is posed the old way and the new way and the world
//! matrices compared. See `AI Info/SCENE_SPLIT_PLAN.md`.

use crate::anim_split::{self, SplitSpec};
use crate::convert::model as raw;
use crate::pose::{self, Mat4};
use crate::strut_anim::{self, Clip};
use crate::strut_model::{self, Model};
use std::collections::{BTreeSet, HashSet};

/// The name of the partner's root bone in the girls' models (a setting in the plan; this is the game's own).
pub const PARTNER_ROOT: &str = "steve";

/// The bones of the subtree under `root`, root first, in the model's order.
pub fn subtree(model: &Model, root: &str) -> Vec<usize> {
    let Some(start) = model.group_bones.iter().position(|b| b.name == root) else { return Vec::new() };

    model
        .group_bones
        .iter()
        .enumerate()
        .filter(|(i, _)| {
            let mut at = *i as i32;

            while at >= 0 {
                if at as usize == start {
                    return true;
                }

                at = model.group_bones[at as usize].parent_index as i32;
            }

            false
        })
        .map(|(i, _)| i)
        .collect()
}

/// The partner's rig taken out of a girl's model.
pub struct Partner {
    /// The model as it is written, with its skeleton id.
    pub bytes: Vec<u8>,
    pub bones: Vec<String>,
    /// What a clip's root position is moved by (sixteenths of a block) so that the rest root can sit at the partner's own origin: where the root's pivot was.
    pub shift: [f32; 3],
    pub fingerprint: i32,
}

/// Takes the partner's rig out of a model into a model of its own: its bones and cubes, moved so that the root's pivot is the origin, with the given skeleton id. Meshes and
/// armatures are not carried (the rig is cubes in the game's models) and are reported.
pub fn extract_partner(model: &Model, root: &str, skeleton_id: &str) -> Result<Partner, String> {
    let members = subtree(model, root);

    if members.is_empty() {
        return Err(format!("the model has no bone called {root}"));
    }

    if model.group_bones[members[0]].parent_index >= 0 {
        return Err(format!("{root} is not a root bone"));
    }

    let mut new_index = vec![-1i32; model.group_bones.len()];

    for (new, old) in members.iter().enumerate() {
        new_index[*old] = new as i32;
    }

    let pivot = model.group_bones[members[0]].pivot;
    let shift = [-pivot[0], -pivot[1], -pivot[2]];
    let moved = |v: [f32; 3]| [(v[0] + shift[0]) as f64, (v[1] + shift[1]) as f64, (v[2] + shift[2]) as f64];
    let mut bones = Vec::new();
    let mut cubes: Vec<raw::Cube> = Vec::new();
    let mut strings: Vec<String> = vec!["partner".to_string()];
    let intern = |s: &str, strings: &mut Vec<String>| {
        if !strings.iter().any(|t| t == s) {
            strings.push(s.to_string());
        }
    };

    for old in &members {
        let bone = &model.group_bones[*old];

        if bone.elements.iter().any(|e| e.kind == 0) {
            return Err(format!("the bone {} holds a mesh, and the partner's rig is cubes only", bone.name));
        }

        for element in &bone.elements {
            let cube = &model.cubes[element.index as usize];

            intern(&cube.name, &mut strings);
            cubes.push(raw::Cube {
                name: cube.name.clone(),
                texture: 0,
                origin: moved(cube.origin),
                rotation: cube.rotation.map(|v| v as f64),
                from: moved(cube.from),
                to: moved(cube.to),
                inflate: cube.inflate as f64,
                mirror: cube.mirror,
                box_uv: cube.box_uv,
                faces: cube.faces.iter().map(|f| raw::CubeFace { face_id: f.face_id as u32, u1: f.u1 as f64, v1: f.v1 as f64, u2: f.u2 as f64, v2: f.v2 as f64, uv_rotation: f.uv_rotation as i64 }).collect(),
            });
        }

        intern(&bone.name, &mut strings);
        bones.push(bone.name.clone());
    }

    let group_bones: Vec<raw::GroupBone> = members
        .iter()
        .enumerate()
        .map(|(n, old)| {
            let bone = &model.group_bones[*old];
            let mut elements = Vec::new();
            let mut taken = 0;

            for (earlier, o) in members.iter().enumerate() {
                if earlier == n {
                    break;
                }

                taken += model.group_bones[*o].elements.len();
            }

            for k in 0..bone.elements.len() {
                elements.push(raw::Element { kind: 1, index: (taken + k) as u32 });
            }

            raw::GroupBone { name: bone.name.clone(), parent: if bone.parent_index >= 0 { new_index[bone.parent_index as usize] } else { -1 }, pivot: moved(bone.pivot), rotation: bone.rotation.map(|v| v as f64), elements }
        })
        .collect();

    let source = model.textures.first().ok_or("the model has no texture to take the partner's from")?;
    let id_bytes = {
        let mut payload = (skeleton_len(skeleton_id)?).to_le_bytes().to_vec();

        payload.extend_from_slice(skeleton_id.as_bytes());
        payload
    };
    let mut model_raw = raw::RawModel {
        model_id: None,
        bounds_min: model.bounds_min.map(|v| v as f64),
        bounds_max: model.bounds_max.map(|v| v as f64),
        strings,
        textures: vec![raw::Texture { name: "partner".into(), width: source.width as f64, height: source.height as f64, uv_width: source.uv_width as f64, uv_height: source.uv_height as f64 }],
        group_bones,
        cubes,
        extensions: vec![(strut_model::SKELETON_EXTENSION.to_string(), id_bytes)],
    };

    model_raw.strings.sort();
    model_raw.strings.dedup();

    let fingerprint = raw::fingerprint(&model_raw);
    let bytes = raw::encode(&model_raw)?;

    strut_model::read(&bytes).map_err(|e| format!("the partner model did not read back: {e}"))?;

    /* The root sat at `pivot` from the model's origin (in model units); a clip's position is in sixteenths of a block. */
    let to_sixteenths = 16.0 / model.units_per_block;

    Ok(Partner { bytes, bones, shift: [pivot[0] * to_sixteenths, pivot[1] * to_sixteenths, pivot[2] * to_sixteenths], fingerprint })
}

fn skeleton_len(id: &str) -> Result<u16, String> {
    u16::try_from(id.len()).map_err(|_| "the skeleton id is too long".to_string())
}

/// What the split of one scene file made.
pub struct SplitFile {
    pub host: Vec<u8>,
    pub partner: Vec<u8>,
    pub notes: Vec<String>,
}

/// Splits a scene file between the host and the partner extracted from the same kind of model.
pub fn split_file(bytes: &[u8], partner: &Partner, root: &str, host_id: &str, partner_id: &str) -> Result<SplitFile, String> {
    let bones: HashSet<String> = partner.bones.iter().cloned().collect();
    /* The shift is in blocks of the model; the clips' positions are in sixteenths of a block. */
    let spec = SplitSpec {
        partner_bones: &bones,
        root,
        shift: [partner.shift[0], partner.shift[1], partner.shift[2]],
        partner_fingerprint: partner.fingerprint,
        host_id: Some(host_id),
        partner_id: Some(partner_id),
    };
    let done = anim_split::split(bytes, &spec)?;

    Ok(SplitFile { host: done.host, partner: done.partner, notes: done.notes })
}

/// One thing the proof found different.
#[derive(Debug, Clone)]
pub struct Mismatch {
    pub clip: String,
    pub tick: u32,
    pub bone: String,
    pub by: f64,
}

/// Poses the old girl model with the old clip, and the (still whole) girl model and the partner model with their halves, at several ticks, and compares the world matrix of every
/// bone. The partner is placed at the origin, as the game does. Returns what differs by more than `tolerance`.
pub fn prove(old_model: &Model, old_clips: &[Clip], partner_model: &Model, host_clips: &[Clip], partner_clips: &[Clip], partner_bones: &[String], tolerance: f64) -> Vec<Mismatch> {
    let mut found = Vec::new();
    let partner_set: BTreeSet<&String> = partner_bones.iter().collect();

    for old in old_clips {
        let host = host_clips.iter().find(|c| c.name == old.name);
        let partner = partner_clips.iter().find(|c| c.name == old.name);
        let steps = 8;

        for step in 0..=steps {
            let time = old.duration as f64 * step as f64 / steps as f64;
            let before = pose::world_matrices(old_model, Some(old), time);
            let host_after = host.map(|c| pose::world_matrices(old_model, Some(c), time));
            let partner_after = partner.map(|c| pose::world_matrices(partner_model, Some(c), time));

            for (name, matrix) in &before {
                let after: Option<&Mat4> = if partner_set.contains(name) { partner_after.as_ref().and_then(|p| p.get(name)) } else { host_after.as_ref().and_then(|h| h.get(name)) };
                let by = after.map_or(f64::INFINITY, |a| pose::distance(matrix, a));

                if by > tolerance {
                    found.push(Mismatch { clip: old.name.clone(), tick: (time * 20.0).round() as u32, bone: name.clone(), by });
                }
            }
        }
    }

    found
}

/// Splits a scene file and proves the result in one go: the proof's mismatches, empty when nothing moved.
pub fn split_and_prove(old_model_bytes: &[u8], scene_bytes: &[u8], host_id: &str, partner_id: &str) -> Result<(SplitFile, Vec<Mismatch>), String> {
    let model = strut_model::read(old_model_bytes)?;
    let partner = extract_partner(&model, PARTNER_ROOT, partner_id)?;
    let done = split_file(scene_bytes, &partner, PARTNER_ROOT, host_id, partner_id)?;
    let partner_model = strut_model::read(&partner.bytes)?;
    let mismatches = prove(&model, &strut_anim::read(scene_bytes)?, &partner_model, &strut_anim::read(&done.host)?, &strut_anim::read(&done.partner)?, &partner.bones, 1e-4);

    Ok((done, mismatches))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn test_pack() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test_pack/data/pleasurehorizons/asset_sync/strut")
    }

    #[test]
    fn splitting_a_scene_moves_nothing() {
        let folder = test_pack();
        let Ok(scenes) = std::fs::read_dir(folder.join("animations/scenes")) else { return };
        let mut checked = 0;

        {
            for entry in scenes.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                let Some(girl) = name.strip_suffix(".strutanim").and_then(|n| n.rsplit_once('-')).map(|(_, g)| g.to_string()) else { continue };
                let Ok(model) = std::fs::read(folder.join(format!("models/nude/{girl}.strutmodel"))) else { continue };

                let bytes = std::fs::read(entry.path()).unwrap();
                let (done, mismatches) = split_and_prove(&model, &bytes, &format!("pleasurehorizons:{girl}"), "pleasurehorizons:partner").unwrap_or_else(|e| panic!("{name}: {e}"));

                assert!(done.notes.is_empty(), "{name}: {:?}", done.notes);
                assert!(mismatches.is_empty(), "{name}: {} bones moved, first {:?}", mismatches.len(), mismatches.first());

                /* The halves are one clip: same names, lengths and loop modes; the markers stay with the host. */
                let whole = strut_anim::read(&bytes).unwrap();
                let host = strut_anim::read(&done.host).unwrap();
                let partner = strut_anim::read(&done.partner).unwrap();

                assert_eq!(whole.len(), host.len());
                assert_eq!(whole.len(), partner.len());

                for ((w, h), p) in whole.iter().zip(&host).zip(&partner) {
                    assert_eq!((&w.name, w.duration, w.loop_mode), (&h.name, h.duration, h.loop_mode));
                    assert_eq!((&w.name, w.duration, w.loop_mode), (&p.name, p.duration, p.loop_mode));
                    assert_eq!(w.events, h.events);
                    assert!(p.events.is_empty());
                    let had_root_position = w.channels.iter().any(|c| c.target == PARTNER_ROOT && c.property == "POSITION") as usize;

                    assert_eq!(w.channels.len() + 1 - had_root_position, h.channels.len() + p.channels.len());
                }

                assert_eq!(strut_anim::skeleton_id(&done.host).unwrap().as_deref(), Some(format!("pleasurehorizons:{girl}").as_str()));
                assert_eq!(strut_anim::skeleton_id(&done.partner).unwrap().as_deref(), Some("pleasurehorizons:partner"));
                checked += 1;
            }
        }

        assert!(checked > 0, "no scene file was checked");
    }
}
