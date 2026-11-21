//! Converting one model or animation file on its own, without an old pack around it: a Bedrock or Geckolib `.geo.json` becomes a `.strutmodel`, an `.animation.json` becomes a
//! `.strutanim`, and the result is placed where the game looks for it for the girl the person names.

use super::{animation, events, model};
use crate::error::{Error, Result};
use crate::import::{Level, Note};
use crate::oldpack::{collapse, note_of};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

const SYNC: &str = "data/pleasurehorizons/asset_sync";

/// What the converted file is for.
#[derive(Debug, Clone, PartialEq)]
pub enum Role {
    ModelNude,
    ModelDressed,
    /// Her everyday clips: the clips are renamed `animation.base.<name>`.
    AnimationBase,
    /// One scene's clips: renamed `animation.<scene>.<name>`, which the person then points at the scene's parts (slow, fast...) in Blockbench.
    AnimationScene(String),
    /// Every clip keeps its name; the file is named for the source.
    AnimationKeep,
}

pub struct Output {
    pub rel: String,
    pub bytes: Vec<u8>,
    pub notes: Vec<Note>,
}

/// The kind of file a name says it is: `model`, `animation` or none.
pub fn kind_of(file_name: &str) -> Option<&'static str> {
    let lower = file_name.to_lowercase();

    if lower.ends_with(".geo.json") {
        Some("model")
    } else if lower.ends_with(".animation.json") {
        Some("animation")
    } else {
        None
    }
}

fn lower_id(text: &str, what: &str) -> Result<String> {
    let id = text.trim();

    if id.is_empty() || !id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
        return Err(Error::invalid(format!("\"{text}\" cannot be {what}: use only a-z (lower case), 0-9 and _ .")));
    }

    Ok(id.to_string())
}

/// Gives every clip a new prefix: `animation.<anything>.<name>` becomes `animation.<prefix>.<name>`. A name that does not have that shape gets the prefix in front.
fn rename_clips(doc: &mut Value, prefix: &str) {
    let Some(clips) = doc.get_mut("animations").and_then(Value::as_object_mut) else { return };
    let renamed: Map<String, Value> = clips
        .iter()
        .map(|(key, value)| {
            let short = key.strip_prefix("animation.").and_then(|rest| rest.split_once('.')).map_or(key.as_str(), |(_, name)| name);

            (format!("animation.{prefix}.{short}"), value.clone())
        })
        .collect();

    *clips = renamed;
}

/// Converts the text of a file. `girl` is who it is for (a lower case id). `stamp_model` is the text of the `.geo.json` her animations were authored against, if known, so the
/// animation is stamped with its skeleton (without it the stamp is empty, which the game only warns about).
pub fn convert(text: &str, source_name: &str, role: &Role, girl: &str, stamp_model: Option<&str>) -> Result<Output> {
    let girl = lower_id(girl, "a girl id")?;
    let json: Value = serde_json::from_str(text).map_err(|e| Error::invalid(format!("{source_name} is not valid JSON: {e}")))?;

    match role {
        Role::ModelNude | Role::ModelDressed => {
            if kind_of(source_name) == Some("animation") {
                return Err(Error::invalid(format!("{source_name} is an animation file, not a model.")));
            }

            let converted = model::convert_geo(&json, Some(source_name));
            let bytes = model::encode(&converted.model).map_err(Error::Other)?;
            let variant = if *role == Role::ModelNude { "nude" } else { "dressed" };
            let mut notes: Vec<Note> = converted.findings.items.iter().map(note_of).collect();

            notes.push(Note { level: Level::Kept, message: format!("The model: {} bones, {} cubes.", converted.model.group_bones.len(), converted.model.cubes.len()) });
            Ok(Output { rel: format!("{SYNC}/{}", crate::paths::girl_model(variant, &girl)), bytes, notes: collapse(notes) })
        }
        _ => {
            if kind_of(source_name) == Some("model") {
                return Err(Error::invalid(format!("{source_name} is a model file, not an animation.")));
            }

            if json.get("animations").and_then(Value::as_object).map_or(true, Map::is_empty) {
                return Err(Error::invalid(format!("{source_name} has no animations in it.")));
            }

            let mut doc = json;
            let stem = source_name.rsplit(['/', '\\']).next().unwrap_or(source_name).to_lowercase();
            let stem = stem.strip_suffix(".animation.json").unwrap_or(&stem).to_string();
            let fingerprint = match stamp_model {
                Some(model_text) => {
                    let geo: Value = serde_json::from_str(model_text).map_err(|e| Error::invalid(format!("The model to stamp from is not valid JSON: {e}")))?;

                    model::fingerprint(&model::convert_geo(&geo, None).model)
                }
                None => 0,
            };

            let rel = match role {
                Role::AnimationBase => {
                    rename_clips(&mut doc, "base");
                    format!("{SYNC}/{}", crate::paths::girl_base(&girl))
                }
                Role::AnimationScene(scene) => {
                    let scene = lower_id(scene, "a scene id")?;

                    rename_clips(&mut doc, &scene);
                    format!("{SYNC}/strut/animations/scenes/{scene}-{girl}.strutanim")
                }
                _ => {
                    let name: String = stem.chars().map(|c| if c.is_ascii_lowercase() || c.is_ascii_digit() { c } else { '_' }).collect();

                    format!("{SYNC}/strut/animations/{name}.strutanim")
                }
            };

            let mut tokens = BTreeMap::new();
            let mut cues = BTreeMap::new();
            let rules = events::Mapping::new();
            let converted = {
                let mut mapper = events::marker_mapper(&rules, &mut tokens, &mut cues);

                animation::convert(&doc, animation::Options { file: Some(source_name.to_string()), fingerprint, map_event: Some(&mut mapper) })
            };
            let bytes = animation::encode(&converted.animation).map_err(Error::Other)?;
            let mut notes: Vec<Note> = converted.findings.items.iter().map(note_of).collect();

            notes.push(Note { level: Level::Kept, message: format!("{} clip(s).", converted.animation.clips.len()) });

            if !cues.is_empty() {
                notes.push(Note { level: Level::Kept, message: format!("Cues in the clips: {}. Each is a marker now; give it a sound or a line in her bindings file.", cues.keys().cloned().collect::<Vec<_>>().join(", ")) });
            }

            Ok(Output { rel, bytes, notes: collapse(notes) })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GEO: &str = r#"{ "format_version": "1.12.0", "minecraft:geometry": [ { "description": { "identifier": "geometry.x", "texture_width": 64, "texture_height": 64 },
        "bones": [ { "name": "root", "pivot": [0, 0, 0] }, { "name": "head", "parent": "root", "pivot": [0, 1, 0], "cubes": [ { "origin": [-1, 0, -1], "size": [2, 2, 2], "uv": [0, 0] } ] } ] } ] }"#;
    const ANIMATION: &str = r#"{ "format_version": "1.8.0", "animations": { "animation.old.wave": { "loop": true, "animation_length": 1.0, "sound_effects": { "0.5": { "effect": "hello" } }, "bones": { "head": { "rotation": [0, 5, 0] } } } } }"#;

    #[test]
    fn a_model_goes_where_the_game_reads_it() {
        assert!(convert(GEO, "x.geo.json", &Role::ModelDressed, "Amy", None).is_err());

        let output = convert(GEO, "x.geo.json", &Role::ModelDressed, "amy", None).unwrap();

        assert_eq!(output.rel, "data/pleasurehorizons/asset_sync/strut/models/entities/girls/dressed/amy.strutmodel");
        assert!(!output.bytes.is_empty());
    }

    #[test]
    fn animations_are_renamed_for_their_role_and_cues_become_markers() {
        let base = convert(ANIMATION, "old.animation.json", &Role::AnimationBase, "amy", Some(GEO)).unwrap();
        let clips = crate::strut_anim::read(&base.bytes).unwrap();

        assert_eq!(base.rel, "data/pleasurehorizons/asset_sync/strut/animations/entities/girls/base.amy.strutanim");
        assert_eq!(clips[0].name, "animation.base.wave");
        assert_eq!(clips[0].events[0].payload, "hello");

        let scene = convert(ANIMATION, "old.animation.json", &Role::AnimationScene("hug".into()), "amy", None).unwrap();

        assert_eq!(scene.rel, "data/pleasurehorizons/asset_sync/strut/animations/scenes/hug-amy.strutanim");
        assert_eq!(crate::strut_anim::read(&scene.bytes).unwrap()[0].name, "animation.hug.wave");

        let kept = convert(ANIMATION, "My Old.animation.json", &Role::AnimationKeep, "amy", None).unwrap();

        assert_eq!(kept.rel, "data/pleasurehorizons/asset_sync/strut/animations/my_old.strutanim");
        assert_eq!(crate::strut_anim::read(&kept.bytes).unwrap()[0].name, "animation.old.wave");
    }

    #[test]
    fn the_wrong_kind_of_file_is_refused() {
        assert!(convert(GEO, "x.geo.json", &Role::AnimationBase, "amy", None).is_err());
        assert!(convert(ANIMATION, "x.animation.json", &Role::ModelNude, "amy", None).is_err());
        assert!(convert("{ }", "x.animation.json", &Role::AnimationBase, "amy", None).is_err());
    }
}
