//! Checks that name bones of a model, and the armour files: a physics file, an armour wearer, set or look, and the bones a merchant takes off as she undresses. A name that is not a
//! bone of her model does nothing in the game, and nothing says so, which is what these are for.

use super::values::{is_identifier, quoted};
use super::PackFacts;
use crate::diagnostics::Sink;
use crate::json_ast::{Member, Node};
use crate::layout::{ASSET_SYNC, GAME_NAMESPACE};
use crate::reference::{nearest, qualified};
use std::collections::BTreeSet;

/// The armour the game ships under `pleasurehorizons`, which a pack can extend without having a file of its own.
const GAME_ARMOUR: [&str; 5] = ["girl", "kobold", "lucy", "mika", "momo"];

/// The armour texture the game ships.
const GAME_ARMOUR_TEXTURE: &str = "textures/armor/armor.png";

/// The bones of a girl's model the pack has (where format 3 keeps it, or where it was kept before), or None when it has not got that model (or it could not be read), in which
/// case no name can be judged.
fn bones_of<'a>(facts: &'a PackFacts, variant: &str, girl: &str) -> Option<&'a BTreeSet<String>> {
    let at = |path: String| facts.model_bones.get(&format!("data/{GAME_NAMESPACE}/{ASSET_SYNC}/{path}"));

    at(crate::paths::girl_model(variant, girl)).or_else(|| at(crate::paths::girl_model_old(variant, girl)))
}

/// The bones of either of a girl's models: what armour and a merchant's clothes name may be a bone of her clothes as well as of her body.
fn girl_bones(facts: &PackFacts, girl: &str) -> Option<BTreeSet<String>> {
    match (bones_of(facts, "nude", girl), bones_of(facts, "dressed", girl)) {
        (None, None) => None,
        (a, b) => Some(a.into_iter().chain(b).flatten().cloned().collect()),
    }
}

/// Warns about a name that is not a bone, with the nearest bone offered.
fn unknown_bone(sink: &mut Sink, code: &str, name: &str, bones: &BTreeSet<String>, model: &str, start: usize, end: usize) {
    let fix = nearest(name, bones.iter().map(String::as_str));
    let message = format!("\"{name}\" is not a bone of {model}.{}", fix.as_ref().map(|f| format!(" Did you mean \"{f}\"?")).unwrap_or_default());

    match fix {
        Some(fix) => sink.warn_with_fix(code, message, start, end, format!("Change to \"{fix}\""), quoted(&fix)),
        None => sink.warn(code, message, start, end),
    }
}

// =================================================================
// Physics
// =================================================================

/// A physics file (`strut/physics/models/<nude or dressed>/<girl>.json`): every bone it moves has to be a bone of the model of the same name.
pub fn physics(sink: &mut Sink, root: &Node, rel: &str, facts: &PackFacts) {
    let parts: Vec<&str> = rel.split('/').collect();
    let (Some(variant), Some(file)) = (parts.get(parts.len().wrapping_sub(2)), parts.last()) else { return };
    let girl = file.trim_end_matches(".json");
    let Some(bones) = bones_of(facts, variant, girl) else { return };

    if let Some(listed) = root.get("bones").and_then(|m| m.value.as_object()) {
        for member in listed {
            if !bones.contains(&member.key) {
                unknown_bone(sink, "physics-unknown-bone", &member.key, bones, &format!("{girl}'s {variant} model"), member.key_start, member.key_end);
            }
        }
    }

    /* A flesh entry (`"flesh": { "breasts": { "bones": ["boobL", "boobR"] } }`) names the bones of its region, which are bones of the model too. */
    if let Some(entries) = root.get("flesh").or_else(|| root.get("soft")).and_then(|m| m.value.as_object()) {
        for entry in entries {
            let Some(listed) = entry.value.get("bones").and_then(|m| m.value.as_array()) else { continue };

            for item in listed {
                if let Some(name) = item.as_str() {
                    if !bones.contains(name) {
                        unknown_bone(sink, "physics-unknown-bone", name, bones, &format!("{girl}'s {variant} model"), item.start, item.end);
                    }
                }
            }
        }
    }
}

// =================================================================
// A merchant's clothes
// =================================================================

/// The bones a merchant takes off as she undresses, in her girl file. They are bones of her nude model.
pub fn merchant(sink: &mut Sink, root: &Node, girl: &str, facts: &PackFacts) {
    let Some(bones) = girl_bones(facts, girl) else { return };
    let bones = &bones;
    let Some(component) = root.get("components").and_then(|c| c.value.get("pleasurehorizons:merchant")) else { return };

    for key in ["cloak_bones", "shirt_bones", "skirt_back_bones", "bare_bones"] {
        let Some(list) = component.value.get(key).and_then(|m| m.value.as_array()) else { continue };

        for item in list {
            if let Some(name) = item.as_str() {
                if !bones.contains(name) {
                    unknown_bone(sink, "merchant-unknown-bone", name, bones, &format!("{girl}'s models"), item.start, item.end);
                }
            }
        }
    }
}

// =================================================================
// Armour
// =================================================================

fn warn_unknown_keys(sink: &mut Sink, members: &[Member], known: &[&str], what: &str) {
    for member in members {
        if !known.contains(&member.key.as_str()) {
            sink.warn("unknown-key", format!("\"{}\" is not a key of {what} that this tool knows ({}); the game may ignore it.", member.key, known.join(", ")), member.key_start, member.key_end);
        }
    }
}

/// Where the armour files of one kind are, in a pack's namespace.
fn armour_file(namespace: &str, kind: &str, name: &str) -> String {
    format!("data/{namespace}/{ASSET_SYNC}/strut/armour/{kind}/{name}.json")
}

/// `extends` names another armour file of the same kind: the game's own, or one in the pack. A name that is neither is an error, since the file cannot be read without it.
fn extends(sink: &mut Sink, root: &Node, kind: &str, facts: &PackFacts) {
    let Some(member) = root.get("extends") else { return };
    let Some(id) = member.value.as_str() else {
        sink.error("wrong-type", "\"extends\" is the id of another file, such as \"pleasurehorizons:girl\".", member.value.start, member.value.end);
        return;
    };

    if !is_identifier(id) {
        sink.error("bad-id", format!("\"{id}\" is not an id. Write it as namespace:name, such as \"pleasurehorizons:girl\"."), member.value.start, member.value.end);
        return;
    }

    let full = qualified(id);
    let (namespace, name) = full.split_once(':').unwrap_or((GAME_NAMESPACE, id));

    if (namespace == GAME_NAMESPACE && GAME_ARMOUR.contains(&name)) || facts.files.contains(&armour_file(namespace, kind, name)) {
        return;
    }

    let known: Vec<String> = GAME_ARMOUR.iter().map(|g| format!("{GAME_NAMESPACE}:{g}")).collect();
    let fix = nearest(&full, known.iter().map(String::as_str));
    let message = format!("There is no armour {kind} called \"{full}\", in this pack or in the game.{}", fix.as_ref().map(|f| format!(" Did you mean \"{f}\"?")).unwrap_or_default());

    match fix {
        Some(fix) => sink.error_with_fix("armour-extends-missing", message, member.value.start, member.value.end, format!("Change to \"{fix}\""), quoted(&fix)),
        None => sink.error("armour-extends-missing", message, member.value.start, member.value.end),
    }
}

/// A wearer (`strut/armour/wearers/<girl>.json`): which slots of armour hide which of her bones, and what she shares with another wearer.
pub fn wearer(sink: &mut Sink, root: &Node, rel: &str, facts: &PackFacts) {
    let Some(members) = root.as_object() else {
        sink.error("wrong-type", format!("An armour wearer is an object ({{ ... }}), not {}.", root.describe()), root.start, root.end);
        return;
    };

    warn_unknown_keys(sink, members, &["slots", "extends"], "an armour wearer");
    extends(sink, root, "wearers", facts);

    let girl = rel.rsplit('/').next().unwrap_or("").trim_end_matches(".json");

    /* Armour hides bones of either of her models: her clothes (the dressed model) have bones of their own, such as pants. */
    let bones = girl_bones(facts, girl);

    let Some(slots) = root.get("slots") else { return };
    let Some(slots) = slots.value.as_object() else {
        sink.error("wrong-type", "\"slots\" is an object of slots (head, chest, legs, feet).", slots.value.start, slots.value.end);
        return;
    };

    for slot in slots {
        if !["head", "chest", "legs", "feet"].contains(&slot.key.as_str()) {
            sink.warn("armour-unknown-slot", format!("\"{}\" is not an armour slot. The slots are head, chest, legs and feet.", slot.key), slot.key_start, slot.key_end);
        }

        let Some(covers) = slot.value.get("covers") else { continue };
        let Some(list) = covers.value.as_array() else {
            sink.error("wrong-type", "\"covers\" is a list of bone names the armour hides.", covers.value.start, covers.value.end);
            continue;
        };

        for item in list {
            match item.as_str() {
                None => sink.error("wrong-type", "A bone name is text.", item.start, item.end),
                Some(name) => {
                    if let Some(bones) = &bones {
                        if !bones.contains(name) {
                            unknown_bone(sink, "armour-unknown-bone", name, bones, &format!("{girl}'s models"), item.start, item.end);
                        }
                    }
                }
            }
        }
    }
}

/// A set (`strut/armour/sets/<girl>.json`): the armour's texture and where its pieces are on it.
pub fn set(sink: &mut Sink, root: &Node, facts: &PackFacts) {
    let Some(members) = root.as_object() else {
        sink.error("wrong-type", format!("An armour set is an object ({{ ... }}), not {}.", root.describe()), root.start, root.end);
        return;
    };

    warn_unknown_keys(sink, members, &["texture", "texture_size", "column", "extends"], "an armour set");
    extends(sink, root, "sets", facts);

    if let Some(member) = root.get("texture") {
        match member.value.as_str() {
            None => sink.error("wrong-type", "\"texture\" is the id of a texture, such as \"pleasurehorizons:textures/armor/armor.png\".", member.value.start, member.value.end),
            Some(id) => {
                let full = qualified(id);
                let (namespace, path) = full.split_once(':').unwrap_or((GAME_NAMESPACE, id));
                let shipped = namespace == GAME_NAMESPACE && path == GAME_ARMOUR_TEXTURE;

                if !shipped && !facts.files.contains(&format!("data/{namespace}/{ASSET_SYNC}/{path}")) {
                    sink.warn("armour-texture-missing", format!("The pack has no file for \"{full}\" (data/{namespace}/{ASSET_SYNC}/{path}), so the armour will show as the missing texture."), member.value.start, member.value.end);
                }
            }
        }
    }

    if let Some(member) = root.get("texture_size") {
        let ok = member.value.as_array().map_or(false, |l| l.len() == 2 && l.iter().all(|n| n.as_f64().map_or(false, |v| v > 0.0 && v.fract() == 0.0)));

        if !ok {
            sink.error("wrong-type", "\"texture_size\" is [width, height] in whole pixels, such as [4096, 4096].", member.value.start, member.value.end);
        }
    }

    if let Some(member) = root.get("column") {
        let ok = member.value.as_array().map_or(false, |l| l.len() == 2 && l.iter().all(|n| n.as_f64().is_some()));

        if !ok {
            sink.error("wrong-type", "A set's \"column\" is [x, y]: where its first piece sits on the texture.", member.value.start, member.value.end);
        }
    }
}

/// A look (`strut/armour/looks/<name>.json`): which materials it is for, and which column of the texture it uses.
pub fn look(sink: &mut Sink, root: &Node) {
    let Some(members) = root.as_object() else {
        sink.error("wrong-type", format!("An armour look is an object ({{ ... }}), not {}.", root.describe()), root.start, root.end);
        return;
    };

    warn_unknown_keys(sink, members, &["match", "column", "tint", "undyed"], "an armour look");

    if let Some(member) = root.get("column") {
        if !member.value.as_f64().map_or(false, |v| v >= 0.0 && v.fract() == 0.0) {
            sink.error("wrong-type", "\"column\" is a whole number, 0 or more: which column of the texture the look uses.", member.value.start, member.value.end);
        }
    }

    let Some(matching) = root.get("match") else { return };
    let Some(list) = matching.value.get("materials").and_then(|m| m.value.as_array()) else { return };

    for item in list {
        match item.as_str() {
            Some(id) if is_identifier(id) => {}
            Some(id) => sink.error("bad-id", format!("\"{id}\" is not a material id. Write it as minecraft:iron."), item.start, item.end),
            None => sink.error("wrong-type", "A material is text, such as \"minecraft:iron\".", item.start, item.end),
        }
    }
}
