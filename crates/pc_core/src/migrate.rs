//! Pack format versions, and bringing an older pack up to the current one. The pack's format is a number in `.packcreator/project.json`; every step that changes what the files
//! look like raises it by one, and a pack is updated by running the steps from its number up, in place, with the changes kept so they can be undone.
//!
//! - 1: a girl's model carries the male as a second rig (`steve`) and each scene's animation moves both.
//! - 5: the folder of one character's own files is called `characters`, not `owners` (`animations/<set>/characters/`, `event_bindings/characters/`).
//! - 4: layered files: event bindings and everyday animations by skeleton and character, and the phase, variant and side of each clip in the scene layouts (see `layers`).
//! - 3: the files are grouped under `entities`, `blocks` and `items` (girls' models, base animations and physics files move; see `paths`).
//! - 2: one model per participant: the male is the base mod's own model, every scene is a role folder with a layout, models and animation files carry skeleton ids.
//!
//! A pack's number is not trusted alone: a pack made before the number existed, or one with old files copied in, is judged by what is in it too.

use crate::pack_split::{self, Report};
use crate::project::{self, ProjectMeta};
use crate::{skeleton_ids, Result};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;

/// The format this version of the tool writes.
pub const CURRENT: u32 = 5;

#[derive(Debug, Clone, Serialize)]
pub struct Step {
    pub to: u32,
    pub title: &'static str,
    pub detail: &'static str,
}

const STEPS: [Step; 4] = [Step {
    to: 5,
    title: "Characters, not owners",
    detail: "Renames the folders of one character's own files: owners becomes characters (strut/animations/<set>/characters, strut/event_bindings/characters).",
}, Step {
    to: 4,
    title: "Layered files",
    detail: "Event bindings and everyday animations are written once for a skeleton and again only for what one girl changes (strut/event_bindings/skeletons and characters, strut/animations/base/skeletons and characters), and every clip in a scene layout gets its phase, variant and side.",
}, Step {
    to: 3,
    title: "Files grouped under entities, blocks and items",
    detail: "Moves the girls' models, base animations and physics files to the folders the game reads (strut/models/entities/girls/..., strut/animations/entities/...), and blocks and items to their own plural folders.",
}, Step {
    to: 2,
    title: "One model per participant",
    detail: "Takes the old male rig out of the girls' models, splits every scene's animation between the girl and the base mod's male, writes a layout for each scene and gives models and animation files their skeleton ids.",
}];

#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub format: u32,
    pub current: u32,
    pub needed: bool,
    /// Why the pack is thought to be old when its number says it is not.
    pub reason: Option<String>,
    pub steps: Vec<Step>,
    /// The pack was written by a newer tool than this one.
    pub newer: bool,
}

/// Whether any model of the pack still carries the old male rig.
fn has_old_male(root: &Path) -> bool {
    crate::check::walk(root).iter().any(|rel| {
        rel.ends_with(".strutmodel") && rel.contains("/strut/models/") && !rel.contains("/models/male/") && std::fs::read(root.join(rel)).ok().and_then(|b| crate::strut_model::read(&b).ok()).map_or(false, |m| m.group_bones.iter().any(|b| b.name == crate::scene_split::MALE_ROOT && b.parent_index < 0))
    })
}

/// The partner role was called `partner` before it was called `male`: a pack converted in between has role folders, a layout role and a skeleton id with the old name.
const LEGACY: &str = "partner";

fn legacy_files(root: &Path) -> Vec<String> {
    crate::check::walk(root).into_iter().filter(|rel| rel.contains("/asset_sync/strut/") && (rel.contains(&format!("/{LEGACY}/")) || rel.ends_with(&format!("/{LEGACY}.strutanim")) || rel.ends_with(".strutscene") && std::fs::read_to_string(root.join(rel)).map_or(false, |t| t.contains(LEGACY)))).collect()
}

/// Renames what a pack converted when the role was called `partner` carries: role folders, the layout's role, and the skeleton id `pleasurehorizons:partner`.
fn rename_legacy(root: &Path) -> Result<usize> {
    let old_id = format!("{}:{LEGACY}", crate::layout::GAME_NAMESPACE);
    let new_id = format!("{}:male", crate::layout::GAME_NAMESPACE);
    let mut count = 0;

    for rel in legacy_files(root) {
        let path = root.join(&rel);

        if rel.ends_with(".strutscene") {
            let text = std::fs::read_to_string(&path).map_err(|e| crate::Error::io(&path, e))?;
            let renamed = text.replace(&format!("{LEGACY}\""), "male\"").replace(&format!("/{LEGACY}\""), "/male\"").replace(&old_id, &new_id);

            if renamed != text {
                std::fs::write(&path, renamed).map_err(|e| crate::Error::io(&path, e))?;
                count += 1;
            }

            continue;
        }

        let new_rel = rel.replace(&format!("/{LEGACY}/"), "/male/").replace(&format!("/{LEGACY}.strutanim"), "/male.strutanim");
        let mut bytes = std::fs::read(&path).map_err(|e| crate::Error::io(&path, e))?;

        if crate::strut_anim::skeleton_id(&bytes).ok().flatten().as_deref() == Some(old_id.as_str()) {
            bytes = crate::strut_anim::set_skeleton_id(&bytes, Some(&new_id)).map_err(crate::Error::invalid)?;
        }

        let target = root.join(&new_rel);

        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| crate::Error::io(parent, e))?;
        }

        std::fs::write(&target, bytes).map_err(|e| crate::Error::io(&target, e))?;
        std::fs::remove_file(&path).map_err(|e| crate::Error::io(&path, e))?;
        count += 1;
    }

    /* Models carrying the old id (the base mod's own player model). */
    for rel in crate::check::walk(root) {
        if rel.ends_with(".strutmodel") && rel.contains("/asset_sync/strut/models/") {
            let path = root.join(&rel);

            if let Ok(bytes) = std::fs::read(&path) {
                if crate::strut_model::read(&bytes).ok().and_then(|m| m.skeleton_id).as_deref() == Some(old_id.as_str()) {
                    std::fs::write(&path, crate::strut_model::set_skeleton_id(&bytes, Some(&new_id)).map_err(crate::Error::invalid)?).map_err(|e| crate::Error::io(&path, e))?;
                    count += 1;
                }
            }
        }
    }

    /* The old role folders are empty now: remove them, bottom up. */
    fn prune(dir: &Path) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    prune(&entry.path());

                    if entry.file_name() == LEGACY {
                        let _ = std::fs::remove_dir(entry.path());
                    }
                }
            }
        }
    }

    prune(root);

    Ok(count)
}

/// Whether the pack still has what format 4 layers: a girl's whole bindings table, her whole base animation file, or a layout whose clips have no phase.
fn needs_layers(root: &Path) -> bool {
    crate::check::walk(root).iter().any(|rel| {
        rel.contains("/strut/event_bindings/girls/")
            || rel.contains("/strut/animations/entities/girls/base.")
            || (rel.contains("/strut/scenes/") && rel.ends_with(".strutscene") && std::fs::read_to_string(root.join(rel)).map_or(false, |t| t.contains("\"clips\"") && !t.contains("\"phase\"")))
    })
}

/// The files that are still where format 2 kept them.
fn files_to_move(root: &Path) -> Vec<(String, String)> {
    crate::check::walk(root).into_iter().filter_map(|rel| crate::paths::moved(&rel).map(|new| (rel, new))).collect()
}

/// Moves the files to where format 3 keeps them. A file whose new place is taken is left, and reported.
fn move_files(root: &Path) -> Result<(usize, Vec<String>)> {
    let mut moved = 0;
    let mut left = Vec::new();

    for (from, to) in files_to_move(root) {
        let target = root.join(&to);

        if target.exists() {
            left.push(format!("{from}: {to} is already there"));
            continue;
        }

        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| crate::Error::io(parent, e))?;
        }

        std::fs::rename(root.join(&from), &target).map_err(|e| crate::Error::io(&target, e))?;
        moved += 1;
    }

    /* The folders they left are empty now. */
    fn prune(dir: &Path) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    prune(&entry.path());
                    let _ = std::fs::remove_dir(entry.path());
                }
            }
        }
    }

    prune(&root.join("data"));
    Ok((moved, left))
}

pub fn status(root: &Path, meta: &ProjectMeta) -> Status {
    let rig = has_old_male(root);
    let renames = !legacy_files(root).is_empty();
    let to_move = files_to_move(root);
    let moves = to_move.iter().any(|(from, _)| !from.contains("/owners/"));
    let owners = to_move.iter().any(|(from, _)| from.contains("/owners/"));
    let layers = needs_layers(root);
    let old_rig = rig || renames || moves || layers || owners;
    let needed = meta.format < CURRENT || old_rig;

    Status {
        format: meta.format,
        current: CURRENT,
        needed,
        reason: (old_rig && meta.format >= CURRENT).then(|| "Some files are still in the form of an older format.".to_string()),
        /* Only the steps that have something to do in this pack are listed. */
        steps: STEPS.iter().filter(|s| match s.to { 5 => owners, 4 => layers, 3 => moves, _ => rig || renames }).cloned().collect(),
        newer: meta.format > CURRENT,
    }
}

/// Gives the animation files made for one girl's model (her base file and her scene files) the fingerprint of the model as it is now: once the old male rig is cut out of the
/// model its bone names are fewer, and a file made before carries the hash of the old list.
fn restamp(root: &Path) -> Result<usize> {
    let mut models: BTreeMap<String, i32> = BTreeMap::new();
    let files = crate::check::walk(root);

    for rel in &files {
        let Some(rest) = rel.split_once("/asset_sync/strut/").map(|x| x.1) else { continue };

        if crate::paths::is_girl_model(rest) {
            if let Some(model) = std::fs::read(root.join(rel)).ok().and_then(|b| crate::strut_model::read(&b).ok()) {
                let girl = rest.rsplit('/').next().unwrap_or("").trim_end_matches(".strutmodel").to_string();

                if rel.contains("/nude/") || !models.contains_key(&girl) {
                    models.insert(girl, model.fingerprint());
                }
            }
        }
    }

    let mut count = 0;

    for rel in &files {
        let Some(rest) = rel.split_once("/asset_sync/strut/").map(|x| x.1) else { continue };
        let parts: Vec<&str> = rest.split('/').collect();
        let girl = if let Some(g) = parts.last().and_then(|n| n.strip_prefix("base.")).and_then(|n| n.strip_suffix(".strutanim")) {
            g.to_string()
        } else if parts.len() == 6 && parts[0] == "animations" && parts[3] == "host" && parts[4] == "characters" && parts[5].ends_with(".strutanim") {
            parts[5].trim_end_matches(".strutanim").to_string()
        } else if parts.len() == 4 && parts[0] == "animations" && parts[1] == "base" && parts[2] == "characters" && parts[3].ends_with(".strutanim") {
            parts[3].trim_end_matches(".strutanim").to_string()
        } else {
            continue;
        };
        let Some(wanted) = models.get(&girl) else { continue };
        let Ok(bytes) = std::fs::read(root.join(rel)) else { continue };

        if crate::strut_anim::fingerprint(&bytes).map_or(false, |f| f != *wanted) {
            if let Ok(fixed) = crate::strut_anim::set_fingerprint(&bytes, *wanted) {
                std::fs::write(root.join(rel), fixed).map_err(|e| crate::Error::io(&root.join(rel), e))?;
                count += 1;
            }
        }
    }

    Ok(count)
}

/// Brings the pack up to the current format (when `apply`), or only says what it would do. The pack's number is raised only when the update is applied and nothing was left
/// in the old form.
pub fn update(root: &Path, apply: bool) -> Result<Report> {
    let mut meta = project::open(root)?;

    if apply {
        rename_legacy(root)?;
    }

    let (moved, left) = if apply { move_files(root)? } else { (files_to_move(root).len(), Vec::new()) };
    let mut report = pack_split::split_pack(root, apply)?;

    if moved > 0 {
        report.findings.push(format!("{moved} files {} to the folders the game reads now.", if apply { "were moved" } else { "would be moved" }));
        report.files_written += moved;
    }

    report.skipped.extend(left);

    if apply {
        let plan = skeleton_ids::plan(root, &BTreeMap::new());
        let written = skeleton_ids::apply(root, &plan.assignments)?;

        /* The layering comes after the move and the split, and is kept for undo on top of the split's. */
        let (layer_stamp, layer_notes) = crate::layers::apply(root)?;

        report.findings.extend(layer_notes);

        if let Some(later) = layer_stamp {
            match &report.stamp {
                Some(first) => crate::pack_split::chain(root, first, &later)?,
                None => report.stamp = Some(later),
            }

            report.files_written += 1;
        }

        let stamped = restamp(root)?;

        if stamped > 0 {
            report.findings.push(format!("{stamped} animation files were stamped again with the bones of their girl's model."));
        }

        if written.written > 0 {
            report.findings.push(format!("Skeleton ids written to {} more models and animation files.", written.written));
        }

        for failed in written.failed {
            report.findings.push(format!("Skeleton id not written: {failed}"));
        }

        if report.skipped.is_empty() && meta.format < CURRENT {
            meta.format = CURRENT;
            project::save_meta(root, &meta)?;
        }
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn an_old_pack_is_updated_and_then_current() {
        let source = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test_pack");

        if !source.join("data").is_dir() {
            return;
        }

        let copy = std::env::temp_dir().join(format!("pc-migrate-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&copy);
        copy_dir(&source, &copy);

        let meta = project::open(&copy).unwrap();
        let before = status(&copy, &meta);

        assert!(before.needed && !before.steps.is_empty());

        let report = update(&copy, true).unwrap();

        assert!(report.skipped.is_empty(), "{:?}", report.skipped);

        let meta = project::open(&copy).unwrap();

        assert_eq!(meta.format, CURRENT);
        assert!(!status(&copy, &meta).needed);
        let _ = std::fs::remove_dir_all(&copy);
    }

    #[test]
    fn layering_gives_every_girl_exactly_what_she_had() {
        let source = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test_pack");

        if !source.join("data").is_dir() {
            return;
        }

        let copy = std::env::temp_dir().join(format!("pc-layers-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&copy);
        copy_dir(&source, &copy);
        update(&copy, true).unwrap();

        let ids: BTreeMap<String, String> = skeleton_ids::plan(&copy, &BTreeMap::new()).assignments.into_iter().filter(|a| a.kind == skeleton_ids::Kind::Model).map(|a| (a.character, a.id)).collect();
        let sync = "data/pleasurehorizons/asset_sync/strut";
        let path_of = |id: &str| id.split_once(':').map_or(id.to_string(), |(_, p)| p.to_string());
        let (mut tables, mut base_files) = (0, 0);

        for girl in ids.keys() {
            /* Bindings: the skeleton's table with hers over it is the table she had. */
            let original = source.join(format!("{sync}/event_bindings/girls/{girl}.json"));

            if let Ok(text) = std::fs::read_to_string(&original) {
                let had: serde_json::Value = serde_json::from_str(&text).unwrap();
                let mut merged = serde_json::Map::new();

                for file in [format!("{sync}/event_bindings/skeletons/{}.json", path_of(&ids[girl])), format!("{sync}/event_bindings/characters/{girl}.json")] {
                    if let Some(layer) = std::fs::read_to_string(copy.join(file)).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()) {
                        merged.extend(layer["bindings"].as_object().cloned().unwrap_or_default());
                    }
                }

                assert_eq!(serde_json::Value::Object(merged), had["bindings"], "{girl}: her bindings changed");
                assert!(!copy.join(format!("{sync}/event_bindings/girls/{girl}.json")).exists());
                tables += 1;
            }

            /* Everyday clips: the skeleton's and hers together are the clips she had, clip for clip. */
            let original = source.join(format!("{sync}/animations/base.{girl}.strutanim"));

            if let Ok(bytes) = std::fs::read(&original) {
                let mut had = crate::strut_anim::read(&bytes).unwrap();

                /* The old male rig's channels went to the male when the scenes were split (and are not in a base file's host half). */
                if let Some(model) = std::fs::read(source.join(format!("{sync}/models/nude/{girl}.strutmodel"))).ok().and_then(|b| crate::strut_model::read(&b).ok()) {
                    let rig: std::collections::HashSet<String> = crate::scene_split::subtree(&model, crate::scene_split::MALE_ROOT).into_iter().map(|i| model.group_bones[i].name.clone()).collect();

                    for clip in &mut had {
                        clip.channels.retain(|c| !rig.contains(&c.target));
                    }
                }

                let mut got: Vec<crate::strut_anim::Clip> = Vec::new();

                for file in [format!("{sync}/animations/base/characters/{girl}.strutanim"), format!("{sync}/animations/base/skeletons/{}.strutanim", path_of(&ids[girl]))] {
                    if let Some(clips) = std::fs::read(copy.join(file)).ok().and_then(|b| crate::strut_anim::read(&b).ok()) {
                        for clip in clips {
                            if !got.iter().any(|c| c.name == clip.name) {
                                got.push(clip);
                            }
                        }
                    }
                }

                assert_eq!(got.len(), had.len(), "{girl}: clip count");

                for clip in &had {
                    if !got.iter().any(|c| c == clip) {
                        let other = got.iter().find(|c| c.name == clip.name).unwrap();

                        eprintln!("DIFF {} channels {} vs {} events {:?} vs {:?} dur {} {} targets {:?} / {:?}", clip.name, clip.channels.len(), other.channels.len(), clip.events.len(), other.events.len(), clip.duration, other.duration, clip.channels.iter().map(|c| &c.target).take(6).collect::<Vec<_>>(), other.channels.iter().map(|c| &c.target).take(6).collect::<Vec<_>>());
                    }

                    assert!(got.iter().any(|c| c == clip), "{girl}: the clip {} changed", clip.name);
                }

                base_files += 1;
            }
        }

        assert!(tables > 3 && base_files > 3, "{tables} tables, {base_files} base files");
        assert!(!status(&copy, &project::open(&copy).unwrap()).needed);
        let _ = std::fs::remove_dir_all(&copy);
    }
}
