//! Fixes for the models of a pack that a person can apply one at a time or all together: the old male rig (`steve`) taken out of the girls' models, the lower arms built with a rotated
//! bone, and the armour bones cut out into models of their own. An import applies the fixes that matter on its own; this is for a pack that was not imported, or was imported before
//! a fix existed. Every apply keeps what it replaces so it can be undone like the scene conversion (`pack_split::undo_split`).

use crate::{arm_fix, armour, layout::ASSET_SYNC, migrate, pack_split, strut_model, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

pub const MALE: &str = "male";
pub const ARMS: &str = "arms";
pub const ARMOUR: &str = "armour";

/// One thing that can be fixed: a fix for one girl, or (the male rig) for the whole pack, when `girl` is empty.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Item {
    pub fix: String,
    pub girl: String,
    pub title: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Selection {
    pub fix: String,
    pub girl: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Outcome {
    pub stamp: Option<String>,
    pub notes: Vec<String>,
    pub written: Vec<String>,
}

/// A girl's models in the pack, by her id.
#[derive(Default)]
struct Girl {
    namespace: String,
    nude: Option<String>,
    dressed: Option<String>,
}

fn girls(root: &Path) -> BTreeMap<String, Girl> {
    let mut found: BTreeMap<String, Girl> = BTreeMap::new();

    for rel in crate::check::walk(root) {
        let parts: Vec<&str> = rel.split('/').collect();

        if parts.len() < 6 || parts[0] != "data" || parts[2] != ASSET_SYNC || parts[3] != "strut" || !crate::paths::is_girl_model(&parts[3..].join("/").trim_start_matches("strut/").to_string()) {
            continue;
        }

        let stem = parts[parts.len() - 1].trim_end_matches(".strutmodel").to_string();
        let girl = found.entry(stem).or_default();

        girl.namespace = parts[1].to_string();

        if rel.contains("/nude/") {
            girl.nude = Some(rel.clone());
        } else {
            girl.dressed = Some(rel.clone());
        }
    }

    found
}

fn model_of(root: &Path, rel: &Option<String>) -> Option<(String, Vec<u8>)> {
    let rel = rel.as_ref()?;

    std::fs::read(root.join(rel)).ok().map(|bytes| (rel.clone(), bytes))
}

/// What can be fixed in this pack.
pub fn scan(root: &Path) -> Vec<Item> {
    let mut items = Vec::new();

    if migrate::has_old_male(root) {
        items.push(Item {
            fix: MALE.into(),
            girl: String::new(),
            title: "Old male rig".into(),
            detail: "Takes the old male (steve) out of the girls' models and splits every scene between the girl and the base mod's male, after checking that every bone is posed as it was.".into(),
        });
    }

    for (id, girl) in girls(root) {
        let models: Vec<(String, Vec<u8>)> = [model_of(root, &girl.dressed), model_of(root, &girl.nude)].into_iter().flatten().collect();
        let mut found = armour::Found::default();
        let mut from = "";

        for (rel, bytes) in &models {
            if let Ok(model) = strut_model::read(bytes) {
                let here = armour::find(&model);

                if found.is_empty() && !here.is_empty() {
                    found = here;
                    from = if rel.contains("/dressed/") { "dressed" } else { "nude" };
                }
            }
        }

        if !found.is_empty() {
            let slots: Vec<&str> = armour::SLOTS.iter().copied().filter(|s| found.slots.contains_key(*s)).collect();

            items.push(Item {
                fix: ARMOUR.into(),
                girl: id.clone(),
                title: "Armour".into(),
                detail: format!("Cuts her armour ({}) out of her {from} model into models of their own, and takes the armour bones out of both of her models.", slots.join(", ")),
            });
        }

        let mut arms = Vec::new();

        for (rel, bytes) in &models {
            if let Ok((_, report)) = arm_fix::fix(bytes) {
                if !report.fixed.is_empty() || !report.removed.is_empty() {
                    arms.push(format!("{}: {} arm bones fixed, {} item bones removed", if rel.contains("/dressed/") { "dressed" } else { "nude" }, report.fixed.len(), report.removed.len()));
                }
            }
        }

        if !arms.is_empty() {
            items.push(Item { fix: ARMS.into(), girl: id, title: "Arms".into(), detail: format!("Moves the rotation of the lower arms into their cubes and takes out the old item bones ({}).", arms.join("; ")) });
        }
    }

    items
}

fn json_of(extends: &str) -> Vec<u8> {
    format!("{{\n  \"extends\": \"{extends}\"\n}}\n").into_bytes()
}

/// Applies the chosen fixes. The ones for girls are done together, then the male rig's (which changes scenes as well). Returns what was done and the undo stamp.
pub fn apply(root: &Path, chosen: &[Selection]) -> Result<Outcome> {
    let mut outcome = Outcome::default();
    let mut current: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut created: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let all = girls(root);

    for choice in chosen.iter().filter(|c| c.fix == ARMOUR || c.fix == ARMS) {
        let Some(girl) = all.get(&choice.girl) else { continue };

        for rel in [&girl.dressed, &girl.nude].into_iter().flatten() {
            if !current.contains_key(rel) {
                if let Ok(bytes) = std::fs::read(root.join(rel)) {
                    current.insert(rel.clone(), bytes);
                }
            }
        }
    }

    /* Armour first, since the arm fix then works on the model that is left. */
    for choice in chosen.iter().filter(|c| c.fix == ARMOUR) {
        let Some(girl) = all.get(&choice.girl) else { continue };
        let mut source: Option<armour::Extracted> = None;
        let mut from = "";

        for (variant, rel) in [("dressed", &girl.dressed), ("nude", &girl.nude)] {
            let Some(rel) = rel else { continue };
            let Some(bytes) = current.get(rel).cloned() else { continue };

            match armour::extract(&bytes) {
                Ok(done) if !done.found.is_empty() => {
                    current.insert(rel.clone(), done.stripped.clone());

                    if source.is_none() {
                        from = variant;
                        source = Some(done);
                    }
                }
                Ok(_) => {}
                Err(e) => outcome.notes.push(format!("{}: the armour in the {variant} model was left alone: {e}", choice.girl)),
            }
        }

        let Some(done) = source else { continue };

        for (slot, bytes) in &done.slots {
            let rel = format!("data/{}/{ASSET_SYNC}/strut/models/armour/{}/{slot}.strutmodel", girl.namespace, choice.girl);

            if root.join(&rel).is_file() {
                outcome.notes.push(format!("{}: {slot} armour already exists ({rel}), so it was not replaced.", choice.girl));
            } else {
                created.insert(rel, bytes.clone());
            }
        }

        for kind in ["sets", "wearers"] {
            let rel = format!("data/{}/{ASSET_SYNC}/strut/armour/{kind}/{}.json", girl.namespace, choice.girl);

            if !root.join(&rel).is_file() {
                created.insert(rel, json_of(&format!("{}:girl", crate::layout::GAME_NAMESPACE)));
            }
        }

        let slots: Vec<&String> = done.slots.keys().collect();

        outcome.notes.push(format!("{}: armour ({}) cut from her {from} model and taken out of her models.", choice.girl, slots.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")));

        if !done.found.unknown.is_empty() {
            outcome.notes.push(format!("{}: {} name no armour slot, so they were left in her model.", choice.girl, done.found.unknown.join(", ")));
        }
    }

    for choice in chosen.iter().filter(|c| c.fix == ARMS) {
        let Some(girl) = all.get(&choice.girl) else { continue };

        for rel in [&girl.dressed, &girl.nude].into_iter().flatten() {
            let Some(bytes) = current.get(rel).cloned() else { continue };

            match arm_fix::fix(&bytes) {
                Ok((fixed, report)) => {
                    if !report.fixed.is_empty() || !report.removed.is_empty() {
                        current.insert(rel.clone(), fixed);
                        outcome.notes.push(format!("{rel}: {} arm bones fixed, {} item bones removed.", report.fixed.len(), report.removed.len()));
                    }

                    for skipped in report.skipped {
                        outcome.notes.push(format!("{rel}: {} was left alone: {}", skipped.bone, skipped.reason));
                    }
                }
                Err(e) => outcome.notes.push(format!("{rel} was not changed: {e}")),
            }
        }
    }

    /* Only files that are different are written. */
    let mut writes: BTreeMap<String, Vec<u8>> = created;

    for (rel, bytes) in current {
        if std::fs::read(root.join(&rel)).ok().as_deref() != Some(bytes.as_slice()) {
            writes.insert(rel, bytes);
        }
    }

    if !writes.is_empty() {
        let stamp = pack_split::commit(root, &writes, &[])?;

        outcome.written = writes.keys().cloned().collect();
        outcome.stamp = Some(stamp);
        migrate::restamp(root)?;
    }

    if chosen.iter().any(|c| c.fix == MALE) {
        let report = migrate::update(root, true)?;

        outcome.notes.extend(report.findings);
        outcome.notes.extend(report.skipped);
        outcome.written.extend(report.written);

        if let Some(later) = report.stamp {
            match &outcome.stamp {
                Some(first) => pack_split::chain(root, first, &later)?,
                None => outcome.stamp = Some(later),
            }
        }
    }

    Ok(outcome)
}

/// Every fix that applies, for a girl who has just been imported (or for every girl, when `only` is empty). The male rig is not part of it: an import already converts that.
pub fn apply_all(root: &Path, only: &[String]) -> Result<Outcome> {
    let chosen: Vec<Selection> = scan(root).into_iter().filter(|i| i.fix != MALE && (only.is_empty() || only.contains(&i.girl))).map(|i| Selection { fix: i.fix, girl: i.girl }).collect();

    apply(root, &chosen)
}
