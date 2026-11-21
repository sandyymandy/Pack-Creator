//! Layered files (pack format 4). Strut reads some files in layers: what is the same for every model of a skeleton is written once, in a file for the skeleton, and what one
//! character changes is written in a file for her (looked up first). This puts a pack's files in that shape:
//!
//! - event bindings: `strut/event_bindings/girls/<girl>.json` (one full table per girl) becomes `skeletons/<skeleton>.json` and `owners/<girl>.json`;
//! - everyday animations: `strut/animations/entities/girls/base.<girl>.strutanim` becomes the clip set `base`: `strut/animations/base/skeletons/<skeleton>.strutanim` and
//!   `strut/animations/base/owners/<girl>.strutanim`;
//! - scene layouts get each clip's phase, variant and side, so the game need not read them from the clip's name.
//!
//! Only what every girl of a skeleton has, in exactly the same form, is moved to the skeleton's file, so nobody gains or loses a binding or a clip.

use crate::strut_anim;
use crate::{skeleton_ids, Result};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// What a layering would write and remove.
#[derive(Default)]
pub struct Plan {
    pub writes: BTreeMap<String, Vec<u8>>,
    pub deletes: Vec<String>,
    pub notes: Vec<String>,
}

fn id_path(id: &str) -> &str {
    id.split_once(':').map_or(id, |(_, p)| p)
}

/// The skeleton id of every girl that has a model in the pack.
fn girl_skeletons(root: &Path) -> BTreeMap<String, String> {
    skeleton_ids::plan(root, &BTreeMap::new()).assignments.into_iter().filter(|a| a.kind == skeleton_ids::Kind::Model).map(|a| (a.owner, a.id)).collect()
}

/// The groups of girls that share a skeleton id.
fn groups(of: &BTreeMap<String, String>, girls: &BTreeSet<String>) -> BTreeMap<String, Vec<String>> {
    let mut found: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for girl in girls {
        let id = of.get(girl).cloned().unwrap_or_else(|| format!("{}:{girl}", crate::layout::GAME_NAMESPACE));

        found.entry(id).or_default().push(girl.clone());
    }

    found
}

// =================================================================
// Event bindings
// =================================================================

/// The entries of a bindings file by key: `bindings` is an object of them (keyed by cue), `events` an array of objects each with a `key`.
fn entries(file: &Value) -> BTreeMap<String, Value> {
    let mut found = BTreeMap::new();

    if let Some(map) = file.get("bindings").and_then(|b| b.as_object()) {
        for (key, value) in map {
            found.insert(format!("bindings/{key}"), value.clone());
        }
    }

    if let Some(list) = file.get("events").and_then(|b| b.as_array()) {
        for value in list {
            if let Some(key) = value.get("key").and_then(|k| k.as_str()) {
                found.insert(format!("events/{key}"), value.clone());
            }
        }
    }

    found
}

fn file_of(entries: &BTreeMap<String, Value>, original: &Value) -> Value {
    let mut bindings = serde_json::Map::new();
    let mut events = Vec::new();

    for (key, value) in entries {
        match key.split_once('/') {
            Some(("bindings", name)) => {
                bindings.insert(name.to_string(), value.clone());
            }
            Some(("events", _)) => events.push(value.clone()),
            _ => {}
        }
    }

    let mut out = serde_json::Map::new();

    /* What the file said besides its entries (a version, say) stays with it. */
    if let Some(object) = original.as_object() {
        for (key, value) in object {
            if key != "bindings" && key != "events" {
                out.insert(key.clone(), value.clone());
            }
        }
    }

    out.insert("bindings".into(), Value::Object(bindings));

    if !events.is_empty() {
        out.insert("events".into(), Value::Array(events));
    }

    Value::Object(out)
}

pub fn plan_bindings(root: &Path) -> Plan {
    let mut plan = Plan::default();
    let skeletons = girl_skeletons(root);
    let mut files: BTreeMap<String, (String, Value)> = BTreeMap::new();

    for rel in crate::check::walk(root) {
        let Some((head, tail)) = rel.split_once("/strut/event_bindings/girls/") else { continue };
        let Some(girl) = tail.strip_suffix(".json") else { continue };

        if girl.contains('/') {
            continue;
        }

        let Some(json) = std::fs::read_to_string(root.join(&rel)).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) else {
            plan.notes.push(format!("{rel} is not valid JSON and was left as it is."));
            continue;
        };

        files.insert(girl.to_string(), (head.to_string(), json));
    }

    let girls: BTreeSet<String> = files.keys().cloned().collect();

    for (id, members) in groups(&skeletons, &girls) {
        let tables: Vec<BTreeMap<String, Value>> = members.iter().map(|g| entries(&files[g].1)).collect();
        let namespace_head = files[&members[0]].0.clone();

        /* What every girl of the skeleton has, in the same form. */
        let mut shared: BTreeMap<String, Value> = BTreeMap::new();

        if members.len() > 1 {
            for (key, value) in &tables[0] {
                if tables.iter().all(|t| t.get(key) == Some(value)) {
                    shared.insert(key.clone(), value.clone());
                }
            }
        }

        if !shared.is_empty() {
            let rel = format!("{namespace_head}/strut/event_bindings/skeletons/{}.json", id_path(&id));

            plan.writes.insert(rel, (serde_json::to_string_pretty(&file_of(&shared, &json!({}))).unwrap_or_default() + "\n").into_bytes());
            plan.notes.push(format!("{} bindings are the same for every girl of {id}, so they are in its own file.", shared.len()));
        }

        for (girl, table) in members.iter().zip(&tables) {
            let hers: BTreeMap<String, Value> = table.iter().filter(|(k, _)| !shared.contains_key(*k)).map(|(k, v)| (k.clone(), v.clone())).collect();
            let (head, original) = &files[girl];

            plan.writes.insert(format!("{head}/strut/event_bindings/owners/{girl}.json"), (serde_json::to_string_pretty(&file_of(&hers, original)).unwrap_or_default() + "\n").into_bytes());
            plan.deletes.push(format!("{head}/strut/event_bindings/girls/{girl}.json"));
        }
    }

    plan
}

// =================================================================
// Everyday animations
// =================================================================

pub fn plan_base(root: &Path) -> Plan {
    let mut plan = Plan::default();
    let skeletons = girl_skeletons(root);
    let mut files: BTreeMap<String, (String, String, Vec<u8>, Vec<strut_anim::Clip>)> = BTreeMap::new();

    for rel in crate::check::walk(root) {
        let Some((head, tail)) = rel.split_once("/strut/animations/entities/girls/base.") else { continue };
        let Some(girl) = tail.strip_suffix(".strutanim") else { continue };
        let Ok(bytes) = std::fs::read(root.join(&rel)) else { continue };
        let Ok(clips) = strut_anim::read(&bytes) else {
            plan.notes.push(format!("{rel} could not be read and was left as it is."));
            continue;
        };

        files.insert(girl.to_string(), (head.to_string(), rel.clone(), bytes, clips));
    }

    let girls: BTreeSet<String> = files.keys().cloned().collect();

    for (id, members) in groups(&skeletons, &girls) {
        let head = files[&members[0]].0.clone();
        let mut shared: Vec<String> = Vec::new();

        if members.len() > 1 {
            for clip in &files[&members[0]].3 {
                if members.iter().all(|g| files[g].3.iter().any(|c| c.name == clip.name && *c == *clip)) {
                    shared.push(clip.name.clone());
                }
            }
        }

        if !shared.is_empty() {
            let (_, _, bytes, _) = &files[&members[0]];

            if let Ok(subset) = strut_anim::subset(bytes, &shared, Some(&id)) {
                plan.writes.insert(format!("{head}/strut/animations/base/skeletons/{}.strutanim", id_path(&id)), subset);
                plan.notes.push(format!("{} everyday clips are the same for every girl of {id}, so they are in its own file.", shared.len()));
            }
        }

        for girl in &members {
            let (head, rel, bytes, clips) = &files[girl];
            let hers: Vec<String> = clips.iter().filter(|c| !shared.contains(&c.name)).map(|c| c.name.clone()).collect();

            plan.deletes.push(rel.clone());

            if hers.is_empty() {
                continue;
            }

            let owned = if hers.len() == clips.len() { Ok(bytes.clone()) } else { strut_anim::subset(bytes, &hers, None) };

            match owned {
                Ok(owned) => {
                    plan.writes.insert(format!("{head}/strut/animations/base/owners/{girl}.strutanim"), owned);
                }
                Err(e) => plan.notes.push(format!("{rel}: {e}")),
            }
        }
    }

    plan
}

// =================================================================
// Clip phases in layouts
// =================================================================

/// What a scene clip does, from its name `animation.<scene>.<part>[_n]`: the phase (`intro`, `slow`, `fast`, `cum`, `lay_on_bed`, `bed_idle`, `loop`, `solo_intro`), the side
/// of a slow clip that has one (`L`, `R`, and `L_swap`, `R_swap`), and the variant (the number the name ends in; 1 for none). The same parts the game reads from the name.
pub fn clip_parts(name: &str) -> Option<(String, Option<String>, u32)> {
    const PARTS: [&str; 12] = ["intro", "slow", "slow_L", "slow_R", "slow_L_swap", "slow_R_swap", "fast", "cum", "lay_on_bed", "bed_idle", "loop", "solo_intro"];

    let rest = name.strip_prefix("animation.")?;
    let (_, part) = rest.split_once('.')?;
    let best = PARTS.iter().filter(|p| part.starts_with(*p)).max_by_key(|p| p.len())?;
    let suffix = &part[best.len()..];
    let variant = if suffix.is_empty() {
        1
    } else {
        let digits = suffix.strip_prefix('_').unwrap_or(suffix);

        digits.parse::<u32>().ok().filter(|n| *n >= 1)?
    };
    let (phase, side) = match best.strip_prefix("slow_") {
        Some(side) => ("slow".to_string(), Some(side.to_string())),
        None => (best.to_string(), None),
    };

    Some((phase, side, variant))
}

pub fn plan_layouts(root: &Path) -> Plan {
    let mut plan = Plan::default();

    for rel in crate::check::walk(root) {
        if !rel.contains("/strut/scenes/") || !rel.ends_with(".strutscene") {
            continue;
        }

        let Some(mut layout) = std::fs::read_to_string(root.join(&rel)).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) else { continue };
        let mut changed = false;

        if let Some(clips) = layout.get_mut("clips").and_then(|c| c.as_object_mut()) {
            for (name, entry) in clips.iter_mut() {
                let Some((phase, side, variant)) = clip_parts(name) else { continue };
                let Some(object) = entry.as_object_mut() else { continue };

                for (key, value) in [("phase", Some(json!(phase))), ("variant", Some(json!(variant))), ("side", side.map(Value::from))] {
                    match value {
                        Some(value) if object.get(key) != Some(&value) => {
                            object.insert(key.to_string(), value);
                            changed = true;
                        }
                        _ => {}
                    }
                }
            }
        }

        if changed {
            plan.writes.insert(rel, (serde_json::to_string_pretty(&layout).unwrap_or_default() + "\n").into_bytes());
        }
    }

    plan
}

/// All three, as one plan.
pub fn plan_all(root: &Path) -> Plan {
    let mut all = Plan::default();

    for part in [plan_bindings(root), plan_base(root), plan_layouts(root)] {
        all.writes.extend(part.writes);
        all.deletes.extend(part.deletes);
        all.notes.extend(part.notes);
    }

    all
}

/// Applies the layering in place, with the changes kept for undo; returns the stamp, or None when there was nothing to do.
pub fn apply(root: &Path) -> Result<(Option<String>, Vec<String>)> {
    let plan = plan_all(root);

    if plan.writes.is_empty() && plan.deletes.is_empty() {
        return Ok((None, plan.notes));
    }

    Ok((Some(crate::pack_split::commit(root, &plan.writes, &plan.deletes)?), plan.notes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clip_name_gives_its_phase_side_and_variant() {
        assert_eq!(clip_parts("animation.doggy.slow"), Some(("slow".into(), None, 1)));
        assert_eq!(clip_parts("animation.doggy.slow_L"), Some(("slow".into(), Some("L".into()), 1)));
        assert_eq!(clip_parts("animation.doggy.slow_R_swap_3"), Some(("slow".into(), Some("R_swap".into()), 3)));
        assert_eq!(clip_parts("animation.doggy.fast_2"), Some(("fast".into(), None, 2)));
        assert_eq!(clip_parts("animation.doggy.lay_on_bed"), Some(("lay_on_bed".into(), None, 1)));
        assert_eq!(clip_parts("animation.doggy.bed_idle"), Some(("bed_idle".into(), None, 1)));
        assert_eq!(clip_parts("animation.base.blink"), None);
        assert_eq!(clip_parts("animation.doggy.fast_0"), None);
    }
}
