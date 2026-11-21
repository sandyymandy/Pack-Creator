//! Converting a pack to one model per participant. The old girl models carry the male as a second root (`steve`) that the same clip animates. The base mod now has one male
//! model (`player`) of its own, so this moves the male's channels onto its bones (a bone is renamed to the bone it became), splits every scene file between the girl and the
//! male, and proves that the bones that kept their place are posed exactly as before. See `AI Info/SCENE_SPLIT_PLAN.md`.

use crate::anim_split::{self, SplitSpec};
use crate::pose::{self, Mat4};
use crate::strut_anim::{self, Clip};
use crate::strut_model::{self, Model};
use std::collections::{BTreeMap, HashMap, HashSet};

/// The name of the male's root bone in the girls' models.
pub const MALE_ROOT: &str = "steve";

/// The skeleton id of the base mod's male model.
pub const MALE_ID: &str = "pleasurehorizons:male";

/// The base mod's male model (`models/entities/male/nude/player.strutmodel`), as it was when this was built: the proof poses it, and its bone names give the fingerprint of the male's clips.
const MALE_MODEL: &[u8] = include_bytes!("../resources/male_player.strutmodel");

/// What each bone of the old male rig became in the new male model, checked against both models and confirmed. A bone of the old rig that is not here (the penis details, the
/// `steveLeftArm` and `steveRightArm` joints) has no counterpart, and its channels are dropped.
pub const BONE_MAP: &[(&str, &str, &str)] = &[
    ("steve", "player", ""),
    ("Torso2", "torso", "steve"),
    ("Head2", "head", "Torso2"),
    ("boyCam", "boyCam", "Head2"),
    ("LeftArm", "armL", "Torso2"),
    ("LeftLowerArm", "armLowerL", "LeftArm"),
    ("leftLowerArmAlex", "armLowerAlexL", "LeftLowerArm"),
    ("leftLowerArmSteve", "armLowerSteveL", "LeftLowerArm"),
    ("leftArmAlex", "armAlexL", "steveLeftArm"),
    ("leftArmSteve", "armSteveL", "steveLeftArm"),
    ("RightArm", "armR", "Torso2"),
    ("RightLowerArm", "armLowerR", "RightArm"),
    ("rightLowerArmAlex", "armLowerAlexR", "RightLowerArm"),
    ("rightLowerArmSteve", "armLowerSteveR", "RightLowerArm"),
    ("rightArmAlex", "armAlexR", "steveRightArm"),
    ("rightArmSteve", "armSteveR", "steveRightArm"),
    ("RightLeg", "legR", "steve"),
    ("kneeR2", "kneeR", "RightLeg"),
    ("shinR2", "shinR", "RightLeg"),
    ("bone18", "bone18", "kneeR2"),
    ("LeftLeg", "legL", "steve"),
    ("kneeR3", "kneeL", "LeftLeg"),
    ("shinL2", "shinL", "LeftLeg"),
    ("bone19", "bone19", "kneeR3"),
    ("cock", "penis", "Torso2"),
    ("shaft", "shaft", "cock"),
    ("nut", "penisHead", "cock"),
    ("ballL", "ballL", "Torso2"),
    ("ballR", "ballR", "Torso2"),
];

/// The map for one girl's model: the bones that are in her male rig, hanging off the parent they should.
pub fn rename_for(model: &Model) -> HashMap<String, String> {
    let parent_of = |name: &str| -> Option<String> {
        let b = model.group_bones.iter().find(|b| b.name == name)?;

        Some(if b.parent_index >= 0 { model.group_bones[b.parent_index as usize].name.clone() } else { String::new() })
    };

    BONE_MAP.iter().filter(|(old, _, parent)| parent_of(old).map_or(false, |p| p == *parent)).map(|(a, b, _)| (a.to_string(), b.to_string())).collect()
}

/// The new bones whose pose must equal the old bone's: the rest pose of the bone and of every bone above it is the same in both models, and every bone above it is mapped, so
/// nothing that is dropped can move it.
fn exact_bones(old: &Model, new: &Model, rename: &HashMap<String, String>) -> HashSet<String> {
    let rest_old = pose::world_matrices(old, None, 0.0);
    let rest_new = pose::world_matrices(new, None, 0.0);
    let mut exact = HashSet::new();

    'bones: for (from, to) in rename {
        let mut at = old.group_bones.iter().position(|b| &b.name == from);

        while let Some(i) = at {
            let name = &old.group_bones[i].name;

            let Some(mapped) = rename.get(name) else { continue 'bones };
            let (Some(a), Some(b)) = (rest_old.get(name), rest_new.get(mapped)) else { continue 'bones };

            if pose::distance(a, b) > 1e-4 {
                continue 'bones;
            }

            at = (old.group_bones[i].parent_index >= 0).then(|| old.group_bones[i].parent_index as usize);
        }

        exact.insert(to.clone());
    }

    exact
}

/// The base mod's male model.
pub fn male_model() -> Model {
    strut_model::read(MALE_MODEL).expect("the male model that is built in reads")
}

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

/// The hash the game recomputes from a skeleton: every group bone's name in order, each followed by a NUL, hashed as one string (Java's `String.hashCode`).
pub fn fingerprint(model: &Model) -> i32 {
    let mut names = String::new();

    for bone in &model.group_bones {
        names.push_str(&bone.name);
        names.push('\0');
    }

    names.encode_utf16().fold(0i32, |hash, unit| hash.wrapping_mul(31).wrapping_add(unit as i32))
}

/// What the split of one scene file made.
pub struct SplitFile {
    pub host: Vec<u8>,
    pub male: Vec<u8>,
    /// Old male bones whose channels had nowhere to go.
    pub dropped: Vec<String>,
}

/// Splits a scene file between the host and the base mod's male. `old_model` is the girl's model the file was made with (its male root says where the root sat).
pub fn split_file(bytes: &[u8], old_model: &Model, host_id: &str) -> Result<SplitFile, String> {
    let members = subtree(old_model, MALE_ROOT);

    if members.is_empty() {
        return Err(format!("the model has no bone called {MALE_ROOT}"));
    }

    let bones: HashSet<String> = members.iter().map(|i| old_model.group_bones[*i].name.clone()).collect();
    let root = &old_model.group_bones[members[0]];
    /* A clip's position is in sixteenths of a block; a model's pivot in its own units. */
    let to_sixteenths = 16.0 / old_model.units_per_block;
    let rename = rename_for(old_model);
    let spec = SplitSpec {
        male_bones: &bones,
        rename: &rename,
        root: MALE_ROOT,
        new_root: "player",
        root_position: root.pivot.map(|v| v * to_sixteenths),
        root_rotation: root.rotation,
        male_fingerprint: fingerprint(&male_model()),
        host_id: Some(host_id),
        male_id: Some(MALE_ID),
    };
    let done = anim_split::split(bytes, &spec)?;

    Ok(SplitFile { host: done.host, male: done.male, dropped: done.dropped.into_iter().collect() })
}

/// One thing the proof found different.
#[derive(Debug, Clone)]
pub struct Mismatch {
    pub clip: String,
    pub tick: u32,
    pub bone: String,
    pub by: f64,
}

/// What the proof found: bones that had to be identical and were not, and how far the others are from where they were (the largest difference of a world matrix, by new bone name).
#[derive(Debug, Clone, Default)]
pub struct Proof {
    pub mismatches: Vec<Mismatch>,
    pub approximate: BTreeMap<String, f64>,
}

/// Poses the old girl model with the old clip, and the girl's model and the male model with their halves, at several ticks, and compares the world matrix of every bone that
/// has a counterpart. The male is placed at the origin, as the game does.
pub fn prove(old_model: &Model, old_clips: &[Clip], host_clips: &[Clip], male_clips: &[Clip], tolerance: f64) -> Proof {
    let new_model = male_model();
    let rename = rename_for(old_model);
    let exact = exact_bones(old_model, &new_model, &rename);
    let male_set: HashSet<String> = subtree(old_model, MALE_ROOT).into_iter().map(|i| old_model.group_bones[i].name.clone()).collect();
    let mut proof = Proof::default();

    for old in old_clips {
        let host = host_clips.iter().find(|c| c.name == old.name);
        let male = male_clips.iter().find(|c| c.name == old.name);
        let steps = 8;

        for step in 0..=steps {
            let time = old.duration as f64 * step as f64 / steps as f64;
            let before = pose::world_matrices(old_model, Some(old), time);
            let host_after = host.map(|c| pose::world_matrices(old_model, Some(c), time));
            let male_after = male.map(|c| pose::world_matrices(&new_model, Some(c), time));

            for (name, matrix) in &before {
                let (label, after): (String, Option<&Mat4>) = if male_set.contains(name) {
                    let Some(new_name) = rename.get(name) else { continue };

                    (new_name.clone(), male_after.as_ref().and_then(|p| p.get(new_name)))
                } else {
                    (name.clone(), host_after.as_ref().and_then(|h| h.get(name)))
                };
                let by = after.map_or(f64::INFINITY, |a| pose::distance(matrix, a));

                if male_set.contains(name) && !exact.contains(&label) {
                    let worst = proof.approximate.entry(label).or_insert(0.0);

                    *worst = worst.max(by);
                } else if by > tolerance {
                    proof.mismatches.push(Mismatch { clip: old.name.clone(), tick: (time * 20.0).round() as u32, bone: label, by });
                }
            }
        }
    }

    proof
}

/// Splits a scene file and proves the result in one go.
pub fn split_and_prove(old_model_bytes: &[u8], scene_bytes: &[u8], host_id: &str) -> Result<(SplitFile, Proof), String> {
    let model = strut_model::read(old_model_bytes)?;
    let done = split_file(scene_bytes, &model, host_id)?;
    let proof = prove(&model, &strut_anim::read(scene_bytes)?, &strut_anim::read(&done.host)?, &strut_anim::read(&done.male)?, 1e-4);

    Ok((done, proof))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn the_map_only_names_bones_that_exist() {
        let new_model = male_model();
        let old = std::fs::read(r"C:\Users\Sandyy\Downloads\model.strutmodel").ok().and_then(|b| strut_model::read(&b).ok());

        for (from, to, _) in BONE_MAP {
            assert!(new_model.group_bones.iter().any(|b| b.name == *to), "the new model has no {to}");

            if let Some(old) = &old {
                assert!(old.group_bones.iter().any(|b| b.name == *from), "the old model has no {from}");
            }
        }

        let targets: HashSet<_> = BONE_MAP.iter().map(|(_, t, _)| *t).collect();

        assert_eq!(targets.len(), BONE_MAP.len(), "two bones became one");
    }

    #[test]
    fn splitting_a_scene_keeps_every_bone_where_it_was() {
        let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test_pack/data/pleasurehorizons/asset_sync/strut");
        let Ok(scenes) = std::fs::read_dir(folder.join("animations/scenes")) else { return };
        let mut checked = 0;
        let mut approximate: BTreeMap<String, f64> = BTreeMap::new();

        for entry in scenes.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let Some(girl) = name.strip_suffix(".strutanim").and_then(|n| n.rsplit_once('-')).map(|(_, g)| g.to_string()) else { continue };
            let Ok(model) = std::fs::read(folder.join(format!("models/nude/{girl}.strutmodel"))) else { continue };
            let bytes = std::fs::read(entry.path()).unwrap();
            let (done, proof) = split_and_prove(&model, &bytes, &format!("pleasurehorizons:{girl}")).unwrap_or_else(|e| panic!("{name}: {e}"));

            assert!(proof.mismatches.is_empty(), "{name}: {} bones moved, first {:?}", proof.mismatches.len(), proof.mismatches.first());

            for (bone, by) in proof.approximate {
                let worst = approximate.entry(bone).or_insert(0.0);

                *worst = worst.max(by);
            }

            /* The halves are one clip: same names, lengths and loop modes; the markers stay with the host. */
            let whole = strut_anim::read(&bytes).unwrap();
            let host = strut_anim::read(&done.host).unwrap();
            let male = strut_anim::read(&done.male).unwrap();

            assert_eq!(whole.len(), host.len());
            assert_eq!(whole.len(), male.len());

            for ((w, h), p) in whole.iter().zip(&host).zip(&male) {
                assert_eq!((&w.name, w.duration, w.loop_mode), (&h.name, h.duration, h.loop_mode));
                assert_eq!((&w.name, w.duration, w.loop_mode), (&p.name, p.duration, p.loop_mode));
                assert_eq!(w.events, h.events);
                assert!(p.events.is_empty());
                assert!(p.channels.iter().all(|c| male_model().group_bones.iter().any(|b| b.name == c.target)), "{name}: a male channel names a bone the male model lacks");
            }

            assert_eq!(strut_anim::skeleton_id(&done.male).unwrap().as_deref(), Some(MALE_ID));
            checked += 1;
        }

        println!("checked {checked} files; bones that are only approximate (largest matrix difference): {approximate:?}");
        assert!(checked > 0, "no scene file was checked");
    }
}
