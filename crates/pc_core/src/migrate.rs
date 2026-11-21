//! Pack format versions, and bringing an older pack up to the current one. The pack's format is a number in `.packcreator/project.json`; every step that changes what the files
//! look like raises it by one, and a pack is updated by running the steps from its number up, in place, with the changes kept so they can be undone.
//!
//! - 1: a girl's model carries the male as a second rig (`steve`) and each scene's animation moves both.
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
pub const CURRENT: u32 = 2;

#[derive(Debug, Clone, Serialize)]
pub struct Step {
    pub to: u32,
    pub title: &'static str,
    pub detail: &'static str,
}

const STEPS: [Step; 1] = [Step {
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

pub fn status(root: &Path, meta: &ProjectMeta) -> Status {
    let old_rig = has_old_male(root) || !legacy_files(root).is_empty();
    let needed = meta.format < CURRENT || old_rig;

    Status {
        format: meta.format,
        current: CURRENT,
        needed,
        reason: (old_rig && meta.format >= CURRENT).then(|| "Some models still carry the old male rig.".to_string()),
        steps: STEPS.iter().filter(|s| s.to > meta.format || old_rig).cloned().collect(),
        newer: meta.format > CURRENT,
    }
}

/// Brings the pack up to the current format (when `apply`), or only says what it would do. The pack's number is raised only when the update is applied and nothing was left
/// in the old form.
pub fn update(root: &Path, apply: bool) -> Result<Report> {
    let mut meta = project::open(root)?;

    if apply {
        rename_legacy(root)?;
    }

    let mut report = pack_split::split_pack(root, apply)?;

    if apply {
        let plan = skeleton_ids::plan(root, &BTreeMap::new());
        let written = skeleton_ids::apply(root, &plan.assignments)?;

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
}
