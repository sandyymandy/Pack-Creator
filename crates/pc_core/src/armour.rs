//! Armour taken out of a girl's model. In the old mods the armour was bones inside the girl's model (`armorHelmet`, `armorChest`, `armorBoobs`, `armorPantsUpL`, `armorShoesR`, ...),
//! on her dressed model (and sometimes on the nude one too). The game now wears armour from models of its own, one for each slot (`strut/models/armour/<girl>/head|chest|legs|feet
//! .strutmodel`), made on top of the girl's skeleton: the same bone names and pivots, only the worn part kept (the bones above it stay, empty, so it hangs where it did).
//!
//! This cuts those models out of the girl's model, byte for byte, and takes the armour bones out of both her models. Nothing else in her models changes.

use crate::strut_model::{self, Model};
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};

pub const SLOTS: [&str; 4] = ["head", "chest", "legs", "feet"];

/// The size of the game's armour texture; the faces are written in a 64 wide space, as they always were.
const ATLAS: u16 = 4096;

/// Which slot an armour bone is for, from its name. None for a bone that is not armour, or that is but names no slot.
pub fn slot_of(name: &str) -> Option<&'static str> {
    let n = name.to_ascii_lowercase();
    let rest = n.strip_prefix("armor").or_else(|| n.strip_prefix("armour"))?;

    if rest.contains("helmet") || rest.contains("head") || rest.contains("hat") {
        Some("head")
    } else if rest.contains("booty") || rest.contains("butt") {
        Some("legs")
    } else if rest.contains("shoe") || rest.contains("boot") || rest.contains("feet") || rest.contains("foot") {
        Some("feet")
    } else if rest.contains("chest") || rest.contains("boob") || rest.contains("shoulder") || rest.contains("bra") || rest.contains("torso") {
        Some("chest")
    } else if rest.contains("pants") || rest.contains("booty") || rest.contains("hip") || rest.contains("leg") || rest.contains("knee") {
        Some("legs")
    } else {
        None
    }
}

/// What is in a model: the top armour bones by slot, and top armour bones that name no slot (those are left where they are).
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Found {
    pub slots: BTreeMap<String, Vec<String>>,
    pub unknown: Vec<String>,
}

impl Found {
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }
}

fn is_under(model: &Model, mut bone: i16, ancestor: usize) -> bool {
    while bone >= 0 {
        if bone as usize == ancestor {
            return true;
        }

        bone = model.group_bones[bone as usize].parent_index;
    }

    false
}

/// The top bones of the armour: an armour bone with no armour bone above it. Each is (index, slot).
fn roots(model: &Model) -> (Vec<(usize, &'static str)>, Vec<usize>) {
    let mut named = Vec::new();
    let mut unknown = Vec::new();

    for (i, bone) in model.group_bones.iter().enumerate() {
        let lower = bone.name.to_ascii_lowercase();

        if !(lower.starts_with("armor") || lower.starts_with("armour")) {
            continue;
        }

        let mut above = bone.parent_index;
        let mut covered = false;

        while above >= 0 {
            let name = model.group_bones[above as usize].name.to_ascii_lowercase();

            if name.starts_with("armor") || name.starts_with("armour") {
                covered = true;
                break;
            }

            above = model.group_bones[above as usize].parent_index;
        }

        if covered {
            continue;
        }

        match slot_of(&bone.name) {
            Some(slot) => named.push((i, slot)),
            None => unknown.push(i),
        }
    }

    (named, unknown)
}

pub fn find(model: &Model) -> Found {
    let (named, unknown) = roots(model);
    let mut found = Found::default();

    for (i, slot) in named {
        found.slots.entry(slot.to_string()).or_default().push(model.group_bones[i].name.clone());
    }

    found.unknown = unknown.into_iter().map(|i| model.group_bones[i].name.clone()).collect();
    found
}

/// The armour cut out of one model.
pub struct Extracted {
    pub found: Found,
    /// The slot models, by slot.
    pub slots: BTreeMap<String, Vec<u8>>,
    /// The model without the armour bones.
    pub stripped: Vec<u8>,
}

/// Cuts the armour out of a model: a model for each slot that has any, and the model without it.
pub fn extract(bytes: &[u8]) -> Result<Extracted, String> {
    let model = strut_model::read(bytes)?;
    let (named, _) = roots(&model);
    let found = find(&model);
    let mut slots = BTreeMap::new();

    for slot in SLOTS {
        let tops: Vec<usize> = named.iter().filter(|(_, s)| *s == slot).map(|(i, _)| *i).collect();

        if tops.is_empty() {
            continue;
        }

        let mut keep: HashSet<usize> = HashSet::new();
        let mut empty: HashSet<usize> = HashSet::new();

        for (i, _) in model.group_bones.iter().enumerate() {
            if tops.iter().any(|top| is_under(&model, i as i16, *top)) {
                keep.insert(i);
            }
        }

        for top in &tops {
            let mut above = model.group_bones[*top].parent_index;

            while above >= 0 {
                if !keep.contains(&(above as usize)) {
                    empty.insert(above as usize);
                }

                above = model.group_bones[above as usize].parent_index;
            }
        }

        keep.extend(empty.iter().copied());

        let remove: HashSet<usize> = (0..model.group_bones.len()).filter(|i| !keep.contains(i)).collect();
        let cut = strut_model::edit_bones(bytes, &remove, &empty).map_err(|e| format!("the {slot} armour: {e}"))?;
        let plain = strut_model::drop_extensions(&cut)?;

        slots.insert(slot.to_string(), strut_model::set_texture_size(&plain, ATLAS, ATLAS)?);
    }

    let gone: HashSet<usize> = (0..model.group_bones.len()).filter(|i| named.iter().any(|(top, _)| is_under(&model, *i as i16, *top))).collect();
    let stripped = if gone.is_empty() { bytes.to_vec() } else { strut_model::remove_bones(bytes, &gone).map_err(|e| format!("the armour could not be taken out: {e}"))? };

    Ok(Extracted { found, slots, stripped })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bones_go_to_the_slot_their_name_says() {
        for (name, slot) in [("armorHelmet", "head"), ("armorChest", "chest"), ("armorBoobs", "chest"), ("armorShoulderL", "chest"), ("armorHip", "legs"), ("armorBootyR", "legs"), ("armorPantsUpL", "legs"), ("armorPantsLowR", "legs"), ("armorShoesL", "feet")] {
            assert_eq!(slot_of(name), Some(slot), "{name}");
        }

        assert_eq!(slot_of("torso"), None);
        assert_eq!(slot_of("armorThing"), None);
    }
}

#[cfg(test)]
mod probe {
    /// `ARMOUR_GEO=<dressed geo json> ARMOUR_OUT=<folder> cargo test -p pc_core --lib -- --ignored --nocapture armour_from_geo`
    #[test]
    #[ignore]
    fn armour_from_geo() {
        let text = std::fs::read_to_string(std::env::var("ARMOUR_GEO").unwrap()).unwrap();
        let root: serde_json::Value = serde_json::from_str(&text).unwrap();
        let converted = crate::convert::model::convert_geo(&root, None);
        let bytes = crate::convert::model::encode(&converted.model).unwrap();
        let done = super::extract(&bytes).unwrap();

        println!("PROBE found {:?}", done.found);

        let out = std::path::PathBuf::from(std::env::var("ARMOUR_OUT").unwrap());

        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(out.join("stripped.strutmodel"), &done.stripped).unwrap();

        for (slot, b) in &done.slots {
            std::fs::write(out.join(format!("{slot}.strutmodel")), b).unwrap();
        }
    }
}
