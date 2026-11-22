//! The checks of the one-model-per-participant layout (format 2): a scene's layout file (`strut/scenes/<scene>.strutscene`), its role folders of animation files
//! (`strut/animations/scenes/<scene>/<role>/skeletons|characters/...`) and the skeleton ids on models and animation files. These look at the pack as a whole, so they run once per
//! check of the project.

use super::PackFacts;
use crate::diagnostics::{file_level, Diagnostic, Severity};
use crate::layout::ASSET_SYNC;
use crate::{strut_anim, strut_model};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use std::sync::Mutex;
use std::time::SystemTime;

/// What the checks need of a model or an animation file, kept until the file changes (reading them is the slow part, and every save checks the pack).
#[derive(Clone, Default)]
struct Info {
    skeleton_id: Option<String>,
    /// A model: it still has the old male rig. An animation: its clips as (name, seconds, loop mode).
    has_male_rig: bool,
    clips: Vec<(String, f32, u8)>,
    readable: bool,
    /// A model: its fingerprint. An animation: the fingerprint it carries.
    fingerprint: i32,
}

fn info_of(root: &Path, rel: &str) -> Info {
    type Entry = (u64, SystemTime, Info);
    static CACHE: Mutex<Option<HashMap<std::path::PathBuf, Entry>>> = Mutex::new(None);

    let path = root.join(rel);
    let Ok(meta) = std::fs::metadata(&path) else { return Info::default() };
    let Ok(modified) = meta.modified() else { return Info::default() };

    if let Ok(guard) = CACHE.lock() {
        if let Some((len, when, info)) = guard.as_ref().and_then(|c| c.get(&path)) {
            if *len == meta.len() && *when == modified {
                return info.clone();
            }
        }
    }

    let mut info = Info::default();

    if let Ok(bytes) = std::fs::read(&path) {
        if rel.ends_with(".strutmodel") {
            if let Ok(model) = strut_model::read(&bytes) {
                info.readable = true;
                info.skeleton_id = model.skeleton_id.clone();
                info.fingerprint = model.fingerprint();
                info.has_male_rig = model.group_bones.iter().any(|b| b.name == crate::scene_split::MALE_ROOT && b.parent_index < 0);
            }
        } else if let Ok(clips) = strut_anim::read(&bytes) {
            info.readable = true;
            info.skeleton_id = strut_anim::skeleton_id(&bytes).ok().flatten();
            info.fingerprint = strut_anim::fingerprint(&bytes).unwrap_or(0);
            info.clips = clips.into_iter().map(|c| (c.name, c.duration, c.loop_mode)).collect();
        }
    }

    if let Ok(mut guard) = CACHE.lock() {
        guard.get_or_insert_with(HashMap::new).insert(path, (meta.len(), modified, info.clone()));
    }

    info
}

/// `data/<ns>/asset_sync/strut/<rest>` split into the namespace and the rest, or None for any other path.
fn strut_path(rel: &str) -> Option<(&str, &str)> {
    let rest = rel.strip_prefix("data/")?;
    let (namespace, rest) = rest.split_once('/')?;
    let rest = rest.strip_prefix(ASSET_SYNC)?.strip_prefix("/strut/")?;

    Some((namespace, rest))
}

/// The path part of a skeleton id (`jenny` for `pleasurehorizons:jenny`).
fn id_path(id: &str) -> &str {
    id.split_once(':').map_or(id, |(_, p)| p)
}

struct RoleFile {
    rel: String,
    /// `skeletons` or `characters`, and the name of the file without its extension.
    kind: &'static str,
    name: String,
}

pub fn check(root: &Path, facts: &PackFacts) -> Vec<Diagnostic> {
    let mut found = Vec::new();

    /* The role folders: scene -> role -> files. */
    let mut roles: BTreeMap<String, BTreeMap<String, Vec<RoleFile>>> = BTreeMap::new();
    let mut layouts: BTreeMap<String, String> = BTreeMap::new();
    let mut old_files: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut models: Vec<(String, String)> = Vec::new();

    for rel in &facts.files {
        let Some((_, rest)) = strut_path(rel) else { continue };
        let parts: Vec<&str> = rest.split('/').collect();

        if parts.len() == 2 && parts[0] == "scenes" && parts[1].ends_with(".strutscene") {
            layouts.insert(parts[1].trim_end_matches(".strutscene").to_string(), rel.clone());
        } else if parts.len() == 6 && parts[0] == "animations" && parts[1] == "scenes" && (parts[4] == "skeletons" || parts[4] == "characters") && parts[5].ends_with(".strutanim") {
            roles.entry(parts[2].to_string()).or_default().entry(parts[3].to_string()).or_default().push(RoleFile { rel: rel.clone(), kind: if parts[4] == "skeletons" { "skeletons" } else { "characters" }, name: parts[5].trim_end_matches(".strutanim").to_string() });
        } else if parts.len() == 3 && parts[0] == "animations" && parts[1] == "scenes" && parts[2].ends_with(".strutanim") {
            let name = parts[2].trim_end_matches(".strutanim");
            let scene = name.rsplit_once('-').map(|(s, _)| s).or_else(|| name.rsplit_once('.').map(|(s, _)| s)).unwrap_or(name);

            old_files.entry(scene.to_string()).or_default().push(rel.clone());
            old_files.entry(name.to_string()).or_default().push(rel.clone());
        } else if crate::paths::is_girl_model(rest) {
            models.push((rel.clone(), parts[parts.len() - 1].trim_end_matches(".strutmodel").to_string()));
        }
    }

    /* The animation files made for one girl's model carry that model's fingerprint; the game compares it when a file names no skeleton id. */
    let mut fingerprints: BTreeMap<String, i32> = BTreeMap::new();

    for (rel, girl) in &models {
        let info = info_of(root, rel);

        if info.readable && (rel.contains("/nude/") || !fingerprints.contains_key(girl)) {
            fingerprints.insert(girl.clone(), info.fingerprint);
        }
    }

    for rel in &facts.files {
        let Some((_, rest)) = strut_path(rel) else { continue };
        let parts: Vec<&str> = rest.split('/').collect();
        let girl = if let Some(girl) = parts.last().and_then(|n| n.strip_prefix("base.")).and_then(|n| n.strip_suffix(".strutanim")) {
            Some(girl.to_string())
        } else if parts.len() == 6 && parts[4] == "characters" && parts[3] == "host" && parts[5].ends_with(".strutanim") {
            Some(parts[5].trim_end_matches(".strutanim").to_string())
        } else if parts.len() == 4 && parts[0] == "animations" && parts[1] == "base" && parts[2] == "characters" && parts[3].ends_with(".strutanim") {
            Some(parts[3].trim_end_matches(".strutanim").to_string())
        } else {
            None
        };

        if let Some(wanted) = girl.as_ref().and_then(|g| fingerprints.get(g)) {
            let info = info_of(root, rel);

            if info.readable && info.fingerprint != *wanted {
                found.push(file_level(Severity::Warning, "stale-fingerprint", format!("This animation was made for a model with a different set of bones (it carries 0x{:08x}, her model has 0x{:08x}). The game warns that it was authored against a different skeleton. The pack update stamps it again.", info.fingerprint as u32, *wanted as u32), rel));
            }
        }
    }

    /* A girl's id by her name, from her models. */
    let mut girl_ids: BTreeMap<String, String> = BTreeMap::new();
    let mut any_old_rig = Vec::new();

    for (rel, girl) in &models {
        let info = info_of(root, rel);

        if !info.readable {
            continue;
        }

        match &info.skeleton_id {
            Some(id) => {
                girl_ids.entry(girl.clone()).or_insert_with(|| id.clone());
            }
            None if !roles.is_empty() => found.push(file_level(Severity::Info, "model-without-skeleton-id", "This model has no skeleton id, so no scene role that names one can be filled by it. Give it one in the Skeleton ids dialog (or in Blockbench).", rel)),
            None => {}
        }

        if info.has_male_rig {
            any_old_rig.push(rel.clone());
        }
    }

    let old_scene_files_left = !old_files.is_empty();

    for scene in roles.keys() {
        if let Some(old) = old_files.get(scene) {
            for rel in old {
                found.push(file_level(Severity::Warning, "old-and-new-scene-files", format!("The scene {scene} also has a folder of role files. The game reads the folder, so this single file is never played: remove it."), rel));
            }
        }
    }

    for (scene, by_role) in &roles {
        let layout_rel = layouts.get(scene);

        if layout_rel.is_none() {
            let first = by_role.values().flatten().next().map(|f| f.rel.clone()).unwrap_or_default();

            found.push(file_level(Severity::Warning, "scene-without-layout", format!("The scene {scene} has role folders but no layout file (strut/scenes/{scene}.strutscene), so the game cannot tell which skeleton fills which role."), &first));
        }

        let mut clip_by_role: BTreeMap<&String, BTreeMap<String, (f32, u8, String)>> = BTreeMap::new();
        let mut skeleton_clips: BTreeMap<&String, BTreeSet<String>> = BTreeMap::new();

        for (role, files) in by_role {
            for file in files {
                let info = info_of(root, &file.rel);

                if !info.readable {
                    continue;
                }

                if info.skeleton_id.is_none() {
                    found.push(file_level(Severity::Warning, "animation-without-skeleton-id", "This animation file does not say which skeleton it was made for, so the game matches it by its bone fingerprint and cannot report a mismatch by name. Write the id in the Skeleton ids dialog.", &file.rel));
                }

                for (name, seconds, mode) in &info.clips {
                    clip_by_role.entry(role).or_default().entry(name.clone()).or_insert((*seconds, *mode, file.rel.clone()));

                    if file.kind == "skeletons" {
                        skeleton_clips.entry(role).or_default().insert(name.clone());
                    }
                }
            }

            /* A file for one character may only change clips its skeleton has. */
            if let Some(known) = skeleton_clips.get(role) {
                for file in files.iter().filter(|f| f.kind == "characters") {
                    let extra: Vec<String> = info_of(root, &file.rel).clips.into_iter().map(|c| c.0).filter(|n| !known.contains(n)).collect();

                    if !extra.is_empty() {
                        found.push(file_level(Severity::Warning, "character-clip-not-in-skeleton", format!("This file for {} has clips its skeleton's file for the role {role} does not: {}.", file.name, extra.join(", ")), &file.rel));
                    }
                }
            }
        }

        /* Every role plays the clip of the same name: a clip the host has (in her own file or her skeleton's) has to be in the male's layers too, or he stands still while she moves. */
        let names_of = |files: &[RoleFile], keep: &dyn Fn(&RoleFile) -> bool| -> BTreeSet<String> { files.iter().filter(|f| keep(f)).flat_map(|f| info_of(root, &f.rel).clips.into_iter().map(|c| c.0)).collect() };

        if let (Some(host), Some(male)) = (by_role.get("host"), by_role.get("male")) {
            let male_everyone = names_of(male, &|f| f.kind == "skeletons");

            for file in host.iter().filter(|f| f.kind == "skeletons") {
                let missing: Vec<String> = info_of(root, &file.rel).clips.into_iter().map(|c| c.0).filter(|n| !male_everyone.contains(n) && !male.iter().filter(|f| f.kind == "characters").any(|f| info_of(root, &f.rel).clips.iter().any(|c| &c.0 == n))).collect();

                if !missing.is_empty() {
                    found.push(file_level(Severity::Warning, "male-missing-clip", format!("The host plays {} in this file, but the male has no clip of that name, so he stands still while she moves.", missing.join(", ")), &file.rel));
                }
            }

            for file in host.iter().filter(|f| f.kind == "characters") {
                let own = names_of(male, &|f| f.kind == "characters" && f.name == file.name);
                let missing: Vec<String> = info_of(root, &file.rel).clips.into_iter().map(|c| c.0).filter(|n| !male_everyone.contains(n) && !own.contains(n)).collect();

                if !missing.is_empty() {
                    found.push(file_level(Severity::Warning, "male-missing-clip", format!("{} plays {} in this scene, but the male has no clip of that name for her (male/characters/{}.strutanim, or the male's file for everyone), so he stands still while she moves.", file.name, missing.join(", "), file.name), &file.rel));
                }
            }
        }

        /* One clip name, one length and one loop mode for every role. */
        let mut seen: BTreeMap<&String, (&String, f32, u8)> = BTreeMap::new();

        for (role, clips) in &clip_by_role {
            for (name, (seconds, mode, rel)) in clips {
                match seen.get(name) {
                    None => {
                        seen.insert(name, (role, *seconds, *mode));
                    }
                    Some((other, s, m)) if (s - seconds).abs() > 0.026 || m != mode => {
                        found.push(file_level(Severity::Warning, "clip-differs-between-roles", format!("The clip {name} is {seconds:.2} s ({}) for the role {role} but {s:.2} s ({}) for {other}. The roles of a scene play one clip together, so the length and loop mode must agree.", loop_name(*mode), loop_name(*m)), rel));
                    }
                    _ => {}
                }
            }
        }

        let Some(layout_rel) = layout_rel else { continue };
        let Some(layout) = std::fs::read_to_string(root.join(layout_rel)).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()) else {
            found.push(file_level(Severity::Error, "bad-layout", "This scene layout is not valid JSON.", layout_rel));
            continue;
        };

        let Some(layout_roles) = layout.get("roles").and_then(|r| r.as_object()) else {
            found.push(file_level(Severity::Error, "bad-layout", "This scene layout has no \"roles\".", layout_rel));
            continue;
        };

        /* The lengths the layout states match the clips. */
        if let Some(clips) = layout.get("clips").and_then(|c| c.as_object()) {
            for (name, entry) in clips {
                let ticks = entry.get("length").and_then(|l| l.as_f64()).unwrap_or(0.0);

                for (role, role_clips) in &clip_by_role {
                    if let Some((seconds, _, _)) = role_clips.get(name) {
                        if ((*seconds as f64 * 20.0) - ticks).abs() > 1.0 {
                            found.push(file_level(Severity::Warning, "layout-clip-length", format!("The layout says the clip {name} is {ticks} ticks, but the role {role} has it at {:.0} ticks.", *seconds as f64 * 20.0), layout_rel));
                        }
                    }
                }
            }
        }

        for (role, definition) in layout_roles {
            let skeleton = definition.get("skeleton").and_then(|s| s.as_str()).unwrap_or("");
            let mut ids: Vec<String> = vec![skeleton.to_string()];

            ids.extend(definition.get("compatible").and_then(|c| c.as_array()).map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect::<Vec<_>>()).unwrap_or_default());

            let files = by_role.get(role);

            if files.map_or(true, |f| f.is_empty()) {
                found.push(file_level(Severity::Warning, "role-without-files", format!("The role {role} of the scene {scene} has no animation files (strut/animations/scenes/{scene}/{role}/)."), layout_rel));
                continue;
            }

            let files = files.unwrap();

            /* Every skeleton that may fill the role has a file of its own, or girls of it have one each. */
            for id in ids.iter().filter(|i| !i.is_empty()) {
                let has_skeleton_file = files.iter().any(|f| f.kind == "skeletons" && f.name == id_path(id));
                let has_character_file = files.iter().any(|f| f.kind == "characters" && girl_ids.get(&f.name).map_or(false, |g| g == id));

                if !has_skeleton_file && !has_character_file {
                    found.push(file_level(Severity::Warning, "role-without-file-for-skeleton", format!("Nothing in the role {role} of the scene {scene} plays for the skeleton {id}: add {role}/skeletons/{}.strutanim (or a file for a girl who has it).", id_path(id)), layout_rel));
                }
            }

            /* A girl's file is for a girl whose model the role accepts. */
            if role == "host" || !ids.is_empty() {
                for file in files.iter().filter(|f| f.kind == "characters") {
                    if let Some(id) = girl_ids.get(&file.name) {
                        if !ids.iter().any(|i| i == id) && !skeleton.is_empty() {
                            found.push(file_level(Severity::Warning, "girl-not-in-role", format!("{} has the skeleton {id}, but the role {role} of the scene {scene} is for {}. The game will not let her fill it.", file.name, ids.join(", ")), &file.rel));
                        }
                    }
                }
            }
        }
    }

    /* The old male rig is only needed by scenes that are not split. */
    if !old_scene_files_left && !roles.is_empty() {
        for rel in any_old_rig {
            found.push(file_level(Severity::Info, "unused-male-rig", "Every scene of this pack is split, so the male rig (steve) in this model is never used. Run the pack update to take it out.", &rel));
        }
    }

    found
}

fn loop_name(mode: u8) -> &'static str {
    ["once", "loop", "hold"].get(mode as usize).copied().unwrap_or("once")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::Reference;

    fn copy_dir(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();

        for e in std::fs::read_dir(from).unwrap().flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().to_string();

            if name == "backups" || name == "history" {
                continue;
            }

            if p.is_dir() {
                copy_dir(&p, &to.join(e.file_name()));
            } else {
                std::fs::copy(&p, to.join(e.file_name())).unwrap();
            }
        }
    }

    fn codes(root: &Path) -> BTreeSet<String> {
        crate::check::check_project(root, &Reference::builtin()).into_iter().map(|d| d.code).collect()
    }

    #[test]
    fn a_converted_pack_is_clean_and_each_mistake_is_found() {
        let source = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test_pack");

        if !source.join("data").is_dir() {
            return;
        }

        let copy = std::env::temp_dir().join(format!("pc-roles-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&copy);
        copy_dir(&source, &copy);

        /* Before the update: no role folders, nothing to say about them. */
        let before = codes(&copy);

        assert!(!before.contains("role-without-files") && !before.contains("scene-without-layout"), "{before:?}");

        crate::migrate::update(&copy, true).unwrap();

        let after = codes(&copy);
        let new_codes = ["scene-without-layout", "role-without-files", "role-without-file-for-skeleton", "girl-not-in-role", "old-and-new-scene-files", "animation-without-skeleton-id", "model-without-skeleton-id", "clip-differs-between-roles", "character-clip-not-in-skeleton", "layout-clip-length", "unused-male-rig", "bad-layout", "scene-without-animation", "stale-fingerprint", "male-missing-clip"];

        assert!(new_codes.iter().all(|c| !after.contains(*c)), "a converted pack should be clean of the layout checks: {:?}", new_codes.iter().filter(|c| after.contains(**c)).collect::<Vec<_>>());

        /* Mistakes. */
        let scene_dir = copy.join("data/pleasurehorizons/asset_sync/strut");
        let layout = std::fs::read_dir(scene_dir.join("scenes")).unwrap().flatten().next().unwrap().path();
        let scene = layout.file_stem().unwrap().to_string_lossy().to_string();

        /* A layout without its role folder. */
        std::fs::remove_dir_all(scene_dir.join(format!("animations/scenes/{scene}/male"))).unwrap();
        assert!(codes(&copy).contains("role-without-files"));

        /* A layout that is not JSON. */
        std::fs::write(&layout, "not json").unwrap();
        assert!(codes(&copy).contains("bad-layout"));

        /* An old file beside the folder. */
        std::fs::copy(source.join(format!("data/pleasurehorizons/asset_sync/strut/animations/scenes/{}", std::fs::read_dir(source.join("data/pleasurehorizons/asset_sync/strut/animations/scenes")).unwrap().flatten().find(|e| e.file_name().to_string_lossy().starts_with(&format!("{scene}-"))).unwrap().file_name().to_string_lossy())), scene_dir.join(format!("animations/scenes/{scene}-old.strutanim"))).unwrap();
        assert!(codes(&copy).contains("old-and-new-scene-files"));

        let _ = std::fs::remove_dir_all(&copy);
    }
}
