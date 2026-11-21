//! Pack format versions, and bringing an older pack up to the current one. The pack's format is a number in `.packcreator/project.json`; every step that changes what the files
//! look like raises it by one, and a pack is updated by running the steps from its number up, in place, with the changes kept so they can be undone.
//!
//! - 1: a girl's model carries the partner as a second rig (`steve`) and each scene's animation moves both.
//! - 2: one model per participant: the partner is the base mod's own model, every scene is a role folder with a layout, models and animation files carry skeleton ids.
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
    detail: "Takes the old partner rig out of the girls' models, splits every scene's animation between the girl and the base mod's partner, writes a layout for each scene and gives models and animation files their skeleton ids.",
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

/// Whether any model of the pack still carries the old partner rig.
fn has_old_partner(root: &Path) -> bool {
    crate::check::walk(root).iter().any(|rel| {
        rel.ends_with(".strutmodel") && rel.contains("/strut/models/") && !rel.contains("/models/partner/") && std::fs::read(root.join(rel)).ok().and_then(|b| crate::strut_model::read(&b).ok()).map_or(false, |m| m.group_bones.iter().any(|b| b.name == crate::scene_split::PARTNER_ROOT && b.parent_index < 0))
    })
}

pub fn status(root: &Path, meta: &ProjectMeta) -> Status {
    let old_rig = has_old_partner(root);
    let needed = meta.format < CURRENT || old_rig;

    Status {
        format: meta.format,
        current: CURRENT,
        needed,
        reason: (old_rig && meta.format >= CURRENT).then(|| "Some models still carry the old partner rig.".to_string()),
        steps: STEPS.iter().filter(|s| s.to > meta.format || old_rig).cloned().collect(),
        newer: meta.format > CURRENT,
    }
}

/// Brings the pack up to the current format (when `apply`), or only says what it would do. The pack's number is raised only when the update is applied and nothing was left
/// in the old form.
pub fn update(root: &Path, apply: bool) -> Result<Report> {
    let mut meta = project::open(root)?;
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
