//! Looking at an old girl before she is imported: her models and texture to see, her clips to play, and the scenes her clips make up, so the person can check the guess and fix it.
//!
//! The scenes come from her old profile when she has one, else from the names of her clips ([`detect`]). What the person settles on is a list of [`ScenePlan`]s, which
//! [`apply_plan`] writes into her profile before the import, so the import itself is unchanged.

use crate::convert::animation::{self, Options};
use crate::convert::model;
use crate::error::{Error, Result};
use crate::oldpack::OldPack;
use crate::{strut_anim, strut_model};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

/// The parts a scene's clips can play, in the order a scene runs.
pub const ROLES: [&str; 8] = ["intro", "slow", "fast", "cum", "lay_on_bed", "bed_idle", "loop", "solo_intro"];

/// The animation field of an old scene for each part (the first is the one written), and the older names of it that are read.
const FIELDS: [(&str, &[&str]); 7] = [
    ("intro", &["intro_anim"]),
    ("slow", &["slow_anim"]),
    ("fast", &["fast_anim"]),
    ("cum", &["cum_anim"]),
    ("lay_on_bed", &["lay_on_bed_anim", "lay_down_anim"]),
    ("bed_idle", &["bed_idle_anim", "idle_anim"]),
    ("loop", &["anim"]),
];

/// One scene as the person sees it: its name, and the clips (by short name, as the pack writes them) that play each part. A part may have several clips; the game picks one.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ScenePlan {
    pub name: String,
    pub parts: BTreeMap<String, Vec<String>>,
    /// Where and how the scene plays: one of [`SCENE_TYPES`]. Empty keeps what her profile says (an on-player scene when it says nothing).
    #[serde(default)]
    pub scene_type: String,
    /// How far the bed is moved from where it is by default, for a scene on a bed (`bed_offset` in the old profile). None keeps what the profile says.
    #[serde(default)]
    pub bed_offset: Option<f64>,
}

/// The game's scene types (`type` in a scene file).
pub const SCENE_TYPES: [&str; 8] = ["on_player", "on_bed", "stationary_contact", "stationary_intro", "stationary", "downed", "player_downed", "player_animation"];

/// What looking at one girl found.
#[derive(Debug, Serialize)]
pub struct GirlLook {
    pub id: String,
    /// `nude` and `dressed`, whichever she has.
    pub models: Vec<String>,
    pub texture: bool,
    /// Her clips by short name, in the order the file has them.
    pub clips: Vec<String>,
    pub scenes: Vec<ScenePlan>,
    /// Where `scenes` came from: `profile`, `names` (found from her clip names) or `none`.
    pub scenes_from: String,
    /// Her animations are already in the newer layout (one file for each scene), which is imported as it is: the scenes are shown but cannot be changed here.
    pub files_layout: bool,
    /// The names of the bones of her nude (or dressed) model, to guess her body type from.
    pub bones: Vec<String>,
}

fn lower(text: &str) -> String {
    text.to_lowercase()
}

/// The clips of an old animation file: `(short name, key)`. A clip is named `animation.<girl>.<name>`; the short name is the part after the owner.
fn short_names(old: &Value, girl_id: &str) -> Vec<(String, String)> {
    let empty = Map::new();
    let animations = old.get("animations").and_then(Value::as_object).unwrap_or(&empty);
    let mut short: Vec<(String, String)> = Vec::new();

    for key in animations.keys() {
        let parsed = key.strip_prefix("animation.").and_then(|rest| rest.split_once('.')).filter(|(owner, name)| !owner.is_empty() && !name.is_empty());
        let name = parsed.map_or(key.clone(), |(_, name)| name.to_string());
        let own = parsed.map_or(false, |(owner, _)| lower(owner) == girl_id);

        match short.iter().position(|(n, _)| *n == name) {
            Some(at) => {
                if own {
                    short[at].1 = key.clone();
                }
            }
            None => short.push((name, key.clone())),
        }
    }

    short
}

/// Her old animation file, if she has one in the old layout (every clip in one file).
fn old_animation(pack: &OldPack, raw_id: &str) -> Result<Option<Value>> {
    let id = lower(raw_id);
    let found = [raw_id.to_string(), id].iter().map(|n| pack.root.join(format!("animations/{n}.animation.json"))).find(|p| p.is_file());

    match found {
        Some(path) => {
            let text = fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;

            Ok(Some(serde_json::from_str(&text).map_err(|e| Error::invalid(format!("Her animation file is not valid JSON: {e}")))?))
        }
        None => Ok(None),
    }
}

// =================================================================
// Finding scenes
// =================================================================

/// The part a word names: `intro`, `slow`, `fast`, `cum` and the words an author might use for them.
fn role_word(word: &str) -> Option<&'static str> {
    match word {
        "intro" | "start" | "begin" | "starting" => Some("intro"),
        "slow" | "slowly" | "gentle" | "soft" => Some("slow"),
        "fast" | "faster" | "quick" | "hard" | "rough" => Some("fast"),
        "cum" | "climax" | "orgasm" | "finish" | "finale" | "end" | "ending" => Some("cum"),
        "loop" | "looping" => Some("loop"),
        _ => None,
    }
}

/// The part a clip plays and the scene it belongs to, from its name: the part is a word at the end (`touch_boobs_slow`, `touch_boobs_slow2`, `touch_boobs_slow_2`) or at the
/// start (`slow_touch_boobs`) of it, and what is left is the scene.
#[cfg(test)]
fn role_of(name: &str) -> Option<(String, &'static str)> {
    role_at(name).map(|(stem, role, _)| (stem, role))
}

/// As [`role_of`], and whether the part was found at the start of the name (a looser sign of a scene than one at the end).
fn role_at(name: &str) -> Option<(String, &'static str, bool)> {
    let name = lower(name);
    let mut words: Vec<&str> = name.split('_').filter(|w| !w.is_empty()).collect();

    if words.len() > 1 && words.last().is_some_and(|w| w.chars().all(|c| c.is_ascii_digit())) {
        words.pop();
    }

    if words.len() < 2 {
        return None;
    }

    /* "solo_intro": the intro of a scene she plays alone. */
    if words.len() > 2 && words[words.len() - 1] == "intro" && words[words.len() - 2] == "solo" {
        return Some((words[..words.len() - 2].join("_"), "solo_intro", false));
    }

    for at in [words.len() - 1, 0] {
        let word = words[at].trim_end_matches(|c: char| c.is_ascii_digit());

        if let Some(role) = role_word(word) {
            let stem: Vec<&str> = words.iter().enumerate().filter(|(i, _)| *i != at).map(|(_, w)| *w).collect();

            return Some((stem.join("_"), role, at == 0 && words.len() - 1 != 0));
        }
    }

    None
}

/// Words from a clip's name: `touch_boobs` is "Touch boobs".
fn title_of(stem: &str) -> String {
    let mut text = stem.replace('_', " ");

    if let Some(first) = text.get(0..1).map(str::to_uppercase) {
        text.replace_range(0..1, &first);
    }

    text
}

/// The type a scene probably is, from its name and the parts it has: a loop with no thrusting is a solo scene, a lie-down or bed idle is a bed scene, and a name saying
/// "downed" is a downed one. A guess; the review lets the person change it.
fn guess_type(stem: &str, roles: &[&str]) -> String {
    let lower = stem.to_lowercase();
    let has = |role: &str| roles.contains(&role);

    let guess = if lower.contains("player_down") || lower.contains("playerdown") {
        "player_downed"
    } else if lower.contains("downed") {
        "downed"
    } else if has("solo_intro") {
        "stationary_intro"
    } else if has("loop") && !has("slow") && !has("fast") {
        "stationary"
    } else if has("lay_on_bed") || has("bed_idle") || lower.contains("bed") {
        "on_bed"
    } else {
        "on_player"
    };

    guess.to_string()
}

/// The scenes her clips make up: a name with a `slow` or a `fast` part (and any intro or cum part) is one scene; clips of the same part are its variants. What is left is not
/// part of any scene.
pub fn detect(clips: &[String]) -> Vec<ScenePlan> {
    let mut grouped: BTreeMap<String, BTreeMap<&'static str, Vec<String>>> = BTreeMap::new();
    let mut loose: BTreeMap<String, bool> = BTreeMap::new();

    for clip in clips {
        if let Some((stem, role, at_start)) = role_at(clip) {
            grouped.entry(stem.clone()).or_default().entry(role).or_default().push(clip.clone());

            let all_loose = loose.entry(stem).or_insert(true);

            *all_loose = *all_loose && at_start;
        }
    }

    grouped
        .into_iter()
        /* A part at the start of a name (`fast_walk`) is only a scene when it has another part with it. */
        .filter(|(stem, parts)| (parts.contains_key("slow") || parts.contains_key("fast") || parts.contains_key("loop")) && (!loose[stem] || parts.len() >= 2))
        .map(|(stem, parts)| {
            let scene_type = guess_type(&stem, &parts.keys().copied().collect::<Vec<_>>());

            ScenePlan { name: title_of(&stem), parts: parts.into_iter().map(|(role, names)| (role.to_string(), names)).collect(), scene_type, bed_offset: None }
        })
        .collect()
}

fn names_of(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::String(s)) if !s.is_empty() => vec![s.clone()],
        Some(Value::Array(list)) => list.iter().filter_map(|v| v.as_str().filter(|s| !s.is_empty()).map(String::from)).collect(),
        _ => Vec::new(),
    }
}

/// The scenes of an old profile and the clips each plays.
fn plans_of_profile(profile: &Value) -> Vec<ScenePlan> {
    let mut plans = Vec::new();

    for scene in profile.get("scenes").and_then(Value::as_array).into_iter().flatten() {
        let Some(name) = scene.get("name").and_then(Value::as_str) else { continue };
        let mut parts: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let scene_type = scene.get("scene_type").and_then(Value::as_str).map(str::to_lowercase).unwrap_or_else(|| if scene.get("is_bed_scene").and_then(Value::as_bool) == Some(true) { "on_bed".into() } else { "on_player".into() });

        for (role, fields) in FIELDS {
            for field in fields {
                let names = names_of(scene.get(*field));

                if !names.is_empty() {
                    parts.entry(role.to_string()).or_default().extend(names);
                }
            }
        }

        /* A solo scene with an intro keeps it in the intro field of the profile; here it is its own part. */
        if scene_type == "stationary_intro" {
            if let Some(intro) = parts.remove("intro") {
                parts.insert("solo_intro".into(), intro);
            }
        }

        plans.push(ScenePlan { name: name.to_string(), parts, scene_type, bed_offset: scene.get("bed_offset").and_then(Value::as_f64) });
    }

    plans
}

// =================================================================
// Looking
// =================================================================

fn geo_path(pack: &OldPack, kind: &str, raw_id: &str) -> std::path::PathBuf {
    pack.root.join("geo").join(kind).join(format!("{raw_id}.geo.json"))
}

fn bones_of_geo(json: &Value) -> BTreeSet<String> {
    let mut names = BTreeSet::new();

    for geometry in json.get("minecraft:geometry").and_then(Value::as_array).into_iter().flatten() {
        for bone in geometry.get("bones").and_then(Value::as_array).into_iter().flatten() {
            if let Some(name) = bone.get("name").and_then(Value::as_str) {
                names.insert(name.to_string());
            }
        }
    }

    names
}

/// What is in her pack, and the scenes to start from.
pub fn look(pack: &OldPack, raw_id: &str, profile: Option<&str>) -> Result<GirlLook> {
    let id = lower(raw_id);
    let models: Vec<String> = ["nude", "dressed"].iter().filter(|kind| geo_path(pack, kind, raw_id).is_file()).map(|k| k.to_string()).collect();
    let texture = [raw_id.to_string(), id.clone()].iter().any(|n| pack.root.join(format!("textures/entities/{n}.png")).is_file());
    let mut bones = BTreeSet::new();

    for kind in ["nude", "dressed"] {
        if let Ok(text) = fs::read_to_string(geo_path(pack, kind, raw_id)) {
            if let Ok(json) = serde_json::from_str::<Value>(&text) {
                bones = bones_of_geo(&json);

                break;
            }
        }
    }

    let profile_value: Option<Value> = profile.or_else(|| pack.profile_text(&id)).and_then(|text| serde_json::from_str(text).ok());
    let from_profile = profile_value.as_ref().map(plans_of_profile).unwrap_or_default();

    match old_animation(pack, raw_id)? {
        Some(old) => {
            let clips: Vec<String> = short_names(&old, &id).into_iter().map(|(name, _)| name).collect();
            let (scenes, from) = if !from_profile.is_empty() { (from_profile, "profile") } else {
                let found = detect(&clips);

                if found.is_empty() { (found, "none") } else { (found, "names") }
            };

            Ok(GirlLook { id: raw_id.to_string(), models, texture, clips, scenes, scenes_from: from.to_string(), files_layout: false, bones: bones.into_iter().collect() })
        }
        None => {
            /* The newer layout: `base.<id>` and one `<scene>-<id>` file for each scene. */
            let mut clips = Vec::new();
            let mut scenes = Vec::new();

            for rel in crate::check::walk(&pack.root.join("animations")) {
                let Some(stem) = rel.strip_suffix(".animation.json").or_else(|| rel.strip_suffix(".scene.json")) else { continue };
                let file = stem.rsplit('/').next().unwrap_or(stem).to_lowercase();
                let hers = file == format!("base.{id}") || file.ends_with(&format!("-{id}"));

                if !hers {
                    continue;
                }

                let Ok(text) = fs::read_to_string(pack.root.join("animations").join(&rel)) else { continue };
                let Ok(doc) = serde_json::from_str::<Value>(&text) else { continue };
                let names: Vec<String> = doc.get("animations").and_then(Value::as_object).map(|m| m.keys().cloned().collect()).unwrap_or_default();

                clips.extend(names.iter().cloned());

                if let Some(scene) = file.strip_suffix(&format!("-{id}")) {
                    let mut parts: BTreeMap<String, Vec<String>> = BTreeMap::new();

                    for name in names {
                        let role = name.rsplit('.').next().unwrap_or(&name).to_string();

                        parts.entry(role).or_default().push(name);
                    }

                    scenes.push(ScenePlan { name: title_of(scene), parts, ..Default::default() });
                }
            }

            let from = if scenes.is_empty() { "none" } else { "profile" };

            Ok(GirlLook { id: raw_id.to_string(), models, texture, clips, scenes, scenes_from: from.to_string(), files_layout: true, bones: bones.into_iter().collect() })
        }
    }
}

/// One of her models, converted, for the 3D view.
pub fn model(pack: &OldPack, raw_id: &str, kind: &str) -> Result<strut_model::Model> {
    let path = geo_path(pack, kind, raw_id);
    let text = fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
    let json: Value = serde_json::from_str(&text).map_err(|e| Error::invalid(format!("geo/{kind}/{raw_id}.geo.json is not valid JSON: {e}")))?;
    let converted = model::convert_geo(&json, Some(&format!("geo/{kind}/{raw_id}.geo.json")));
    let bytes = model::encode(&converted.model).map_err(Error::Other)?;

    strut_model::read(&bytes).map_err(Error::Other)
}

/// Her texture (a PNG), if she has one.
pub fn texture(pack: &OldPack, raw_id: &str) -> Option<Vec<u8>> {
    [raw_id.to_string(), lower(raw_id)].iter().map(|n| pack.root.join(format!("textures/entities/{n}.png"))).find(|p| p.is_file()).and_then(|p| fs::read(p).ok())
}

/// Every clip of her old animation file, converted and named by its short name, to play on her model.
pub fn clips(pack: &OldPack, raw_id: &str) -> Result<Vec<strut_anim::Clip>> {
    let id = lower(raw_id);
    let Some(old) = old_animation(pack, raw_id)? else {
        return Err(Error::invalid("Her animations are already one file for each scene; open them in the pack after importing to look at them."));
    };
    let fingerprint = ["nude", "dressed"]
        .iter()
        .find_map(|kind| {
            let text = fs::read_to_string(geo_path(pack, kind, raw_id)).ok()?;
            let json: Value = serde_json::from_str(&text).ok()?;

            Some(model::fingerprint(&model::convert_geo(&json, None).model))
        })
        .unwrap_or(0);
    let converted = animation::convert(&old, Options { file: Some(format!("{raw_id}.animation.json")), fingerprint, map_event: None });
    let bytes = animation::encode(&converted.animation).map_err(Error::Other)?;
    let mut list = strut_anim::read(&bytes).map_err(Error::Other)?;
    let names = short_names(&old, &id);

    for clip in &mut list {
        if let Some((short, _)) = names.iter().find(|(_, key)| *key == clip.name) {
            clip.name = short.clone();
        }
    }

    Ok(list)
}

// =================================================================
// Applying the person's choice
// =================================================================

/// Her profile's text with her scenes replaced by the plan. A scene the plan keeps (by name) keeps everything else it had (its type, affection, tags) and only has its clips
/// changed; a scene the plan adds is a plain one; a scene the plan leaves out is dropped, and its clips go to her base file.
pub fn apply_plan(profile_text: &str, plan: &[ScenePlan]) -> Result<String> {
    let mut profile: Value = serde_json::from_str(profile_text).map_err(|e| Error::invalid(format!("The profile is not valid JSON: {e}")))?;
    let Some(object) = profile.as_object_mut() else {
        return Err(Error::invalid("The profile must be a JSON object."));
    };
    let old: Vec<Value> = object.get("scenes").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut scenes = Vec::new();

    for scene in plan {
        let wanted = lower(&scene.name);
        let mut entry = old.iter().find(|s| s.get("name").and_then(Value::as_str).is_some_and(|n| lower(n) == wanted)).cloned().unwrap_or_else(|| json!({ "name": scene.name, "use_keyframe": true }));

        if let Some(map) = entry.as_object_mut() {
            map.insert("name".into(), json!(scene.name));

            for (_, fields) in FIELDS {
                for field in fields {
                    map.remove(*field);
                }
            }

            if let Some(offset) = scene.bed_offset {
                map.insert("bed_offset".into(), json!(offset));
            }

            if !scene.scene_type.is_empty() {
                map.insert("scene_type".into(), json!(scene.scene_type.to_uppercase()));
                map.remove("is_bed_scene");
            }

            /* The import reads a solo scene's intro from the intro field of the profile. */
            if let Some(solo) = scene.parts.get("solo_intro").filter(|list| !list.is_empty()) {
                map.insert("intro_anim".into(), if solo.len() == 1 { json!(solo[0]) } else { json!(solo) });
            }

            for (role, fields) in FIELDS {
                let names: Vec<&String> = scene.parts.get(role).map(|list| list.iter().collect()).unwrap_or_default();

                match names.len() {
                    0 => {}
                    1 => {
                        map.insert(fields[0].into(), json!(names[0]));
                    }
                    _ => {
                        map.insert(fields[0].into(), json!(names));
                    }
                }
            }
        }

        scenes.push(entry);
    }

    object.insert("scenes".into(), Value::Array(scenes));

    Ok(serde_json::to_string(&profile).map_err(|e| Error::Other(e.to_string()))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn finds_scenes_from_clip_names_with_the_part_at_either_end() {
        let scenes = detect(&names(&["idle", "walk", "touch_boobs_intro", "touch_boobs_slow", "touch_boobs_slow2", "touch_boobs_fast", "touch_boobs_cum", "slow_riding", "fast_riding", "climax_riding", "head_pat"]));

        assert_eq!(scenes.len(), 2);

        let touch = scenes.iter().find(|s| s.name == "Touch boobs").unwrap();

        assert_eq!(touch.parts["slow"], names(&["touch_boobs_slow", "touch_boobs_slow2"]));
        assert_eq!(touch.parts["intro"], names(&["touch_boobs_intro"]));
        assert!(scenes.iter().any(|s| s.name == "Riding" && s.parts["cum"] == names(&["climax_riding"])));
    }

    #[test]
    fn a_name_that_only_ends_like_a_part_is_not_a_scene() {
        assert!(detect(&names(&["fast_walk", "slowdown", "intro", "hard"])).is_empty());
        assert_eq!(role_of("sitting_cum"), Some(("sitting".to_string(), "cum")));
        assert_eq!(role_of("cum"), None);
        assert_eq!(role_of("doggy_slow_2"), Some(("doggy".to_string(), "slow")));
    }

    #[test]
    fn the_plan_replaces_the_scenes_and_keeps_what_a_kept_scene_had() {
        let profile = r#"{ "id": "amy", "scenes": [ { "name": "Doggy", "scene_type": "ON_BED", "required_level": 4, "intro_anim": "a", "slow_anim": "b" }, { "name": "Gone", "slow_anim": "x" } ] }"#;
        let plan = vec![
            ScenePlan { name: "Doggy".into(), parts: BTreeMap::from([("slow".to_string(), names(&["c", "d"])), ("fast".to_string(), names(&["e"]))]), ..Default::default() },
            ScenePlan { name: "New one".into(), parts: BTreeMap::from([("loop".to_string(), names(&["walk"]))]), ..Default::default() },
        ];
        let written: Value = serde_json::from_str(&apply_plan(profile, &plan).unwrap()).unwrap();
        let scenes = written["scenes"].as_array().unwrap();

        assert_eq!(scenes.len(), 2);
        assert_eq!(scenes[0]["required_level"], 4);
        assert_eq!(scenes[0]["scene_type"], "ON_BED");
        assert!(scenes[0].get("intro_anim").is_none());
        assert_eq!(scenes[0]["slow_anim"], json!(["c", "d"]));
        assert_eq!(scenes[0]["fast_anim"], "e");
        assert_eq!(scenes[1]["anim"], "walk");
        assert_eq!(scenes[1]["use_keyframe"], true);
    }

    const GEO: &str = r#"{ "format_version": "1.12.0", "minecraft:geometry": [ { "description": { "identifier": "geometry.zoe", "texture_width": 64, "texture_height": 64 },
        "bones": [ { "name": "root", "pivot": [0, 0, 0] }, { "name": "nut", "parent": "root", "pivot": [0, 1, 0], "cubes": [ { "origin": [-1, 0, -1], "size": [2, 2, 2], "uv": [0, 0] } ] } ] } ] }"#;

    const ANIMATIONS: &str = r##"{ "format_version": "1.8.0", "animations": {
        "animation.zoe.idle": { "loop": true, "bones": { "root": { "rotation": [0, 5, 0] } } },
        "animation.zoe.hug_slow": { "loop": true, "animation_length": 1.0, "bones": { "root": { "position": { "0": [1, 0, 0], "1": [0, 0, 0] } } } },
        "animation.zoe.hug_fast": { "loop": true, "bones": { "root": { "rotation": [1, 2, 3] } } }
    } }"##;

    fn write(root: &std::path::Path, rel: &str, bytes: &[u8]) {
        let path = root.join(rel);

        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn a_girl_is_looked_at_and_imported_with_the_scenes_the_person_chose() {
        let dir = tempfile::tempdir().unwrap();
        let pack = dir.path().join("pack");

        write(&pack, "assets/oldns/geo/nude/zoe.geo.json", GEO.as_bytes());
        write(&pack, "assets/oldns/animations/zoe.animation.json", ANIMATIONS.as_bytes());

        let opened = crate::oldpack::open(&pack).unwrap();
        let look = look(&opened, "zoe", None).unwrap();

        assert_eq!(look.models, names(&["nude"]));
        assert_eq!(look.clips, names(&["idle", "hug_slow", "hug_fast"]));
        assert_eq!(look.scenes_from, "names");
        assert_eq!(look.scenes[0].name, "Hug");
        assert!(look.bones.contains(&"nut".to_string()));

        assert_eq!(model(&opened, "zoe", "nude").unwrap().group_bones.len(), 2);

        let played = clips(&opened, "zoe").unwrap();

        assert_eq!(played.iter().map(|c| c.name.clone()).collect::<Vec<_>>(), names(&["idle", "hug_slow", "hug_fast"]));
        assert!(played.iter().find(|c| c.name == "hug_slow").unwrap().duration > 0.9);

        /* The person moves hug_fast out of the scene and into nothing, and makes a second scene out of the idle clip. */
        let plan = vec![
            ScenePlan { name: "Hug".into(), parts: BTreeMap::from([("slow".to_string(), names(&["hug_slow"]))]), ..Default::default() },
            ScenePlan { name: "Stand".into(), parts: BTreeMap::from([("loop".to_string(), names(&["idle"]))]), scene_type: "stationary".into(), ..Default::default() },
        ];
        let existing = std::collections::HashSet::new();
        let outcome = crate::oldpack::import_girl(
            &opened,
            &crate::oldpack::Request {
                raw_id: "zoe", profile: None, body_type: "jenny", namespace: "pleasurehorizons", liquid: false, existing_files: &existing, existing_scenes: &[],
                rules: &crate::convert::events::Mapping::new(), physics_preset: false, skip_shared: false, taken_tame_items: &[], scene_plan: Some(&plan),
            },
            &crate::reference::Reference::builtin(),
        )
        .unwrap();
        let files: Vec<&str> = outcome.files.iter().map(|(p, _)| p.as_str()).collect();

        assert!(files.contains(&"data/pleasurehorizons/scenes/hug.json"));
        assert!(files.contains(&"data/pleasurehorizons/scenes/stand.json"));
        assert!(files.contains(&"data/pleasurehorizons/asset_sync/strut/animations/scenes/stand-zoe.strutanim"));

        let base = outcome.files.iter().find(|(p, _)| p.ends_with("animations/entities/girls/base.zoe.strutanim")).unwrap();
        let base_clips = strut_anim::read(&base.1).unwrap();

        assert!(base_clips.iter().any(|c| c.name.ends_with("hug_fast")), "a clip no scene uses stays in her base file");
    }

    #[test]
    fn the_type_of_a_scene_is_guessed_and_written() {
        let scenes = detect(&names(&["masturbate_loop", "pose_solo_intro", "pose_loop", "bed_ride_lay_on_bed", "bed_ride_slow", "tied_downed_slow", "tied_downed_fast", "player_downed_slow", "fast_walk"]));
        let type_of = |name: &str| scenes.iter().find(|s| s.name == name).map(|s| s.scene_type.clone());

        assert_eq!(type_of("Masturbate"), Some("stationary".to_string()));
        assert_eq!(type_of("Pose"), Some("stationary_intro".to_string()));
        assert_eq!(type_of("Bed ride"), Some("on_bed".to_string()));
        assert_eq!(type_of("Tied downed"), Some("downed".to_string()));
        assert_eq!(type_of("Player downed"), Some("player_downed".to_string()));

        let profile = r#"{ "scenes": [ { "name": "Pose", "scene_type": "STATIONARY_INTRO", "intro_anim": "a", "anim": "b" } ] }"#;
        let read = plans_of_profile(&serde_json::from_str(profile).unwrap());

        assert_eq!(read[0].parts["solo_intro"], names(&["a"]));
        assert_eq!(read[0].scene_type, "stationary_intro");

        let plan = ScenePlan { name: "Pose".into(), parts: BTreeMap::from([("solo_intro".to_string(), names(&["x"])), ("loop".to_string(), names(&["y"]))]), scene_type: "downed".into(), ..Default::default() };
        let written: Value = serde_json::from_str(&apply_plan(profile, &[plan]).unwrap()).unwrap();

        assert_eq!(written["scenes"][0]["scene_type"], "DOWNED");
        assert_eq!(written["scenes"][0]["intro_anim"], "x");
        assert_eq!(written["scenes"][0]["anim"], "y");
    }

    #[test]
    fn scenes_are_read_back_from_a_profile() {
        let profile: Value = serde_json::from_str(r#"{ "scenes": [ { "name": "Doggy", "intro_anim": "a", "slow_anim": ["b", "c"], "idle_anim": "z" } ] }"#).unwrap();
        let plans = plans_of_profile(&profile);

        assert_eq!(plans[0].parts["slow"], names(&["b", "c"]));
        assert_eq!(plans[0].parts["bed_idle"], names(&["z"]));
    }
}
