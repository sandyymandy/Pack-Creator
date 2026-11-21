//! Who a scene is for. A scene's animations are looked up in the file that applies to the girl who plays it (`cowgirl-momo.strutanim` hers alone, `cowgirl.bia.strutanim` every girl built on
//! that skeleton, `cowgirl.strutanim` everyone's), and the scene file narrows it further with `supported_girls` and `body_types`. This turns a scene made for one girl into one for her whole
//! skeleton, and back, moving the animation file and changing the scene file together so the two never disagree.

use crate::check::walk;
use crate::error::{Error, Result};
use crate::fs_ops;
use crate::history;
use serde::Serialize;
use serde_json::{json, Value};
use std::path::Path;

/// One animation file of a scene and whose it is.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AnimFile {
    pub rel: String,
    /// `girl`, `skeleton` or `shared`.
    pub character_kind: String,
    /// The girl's id, the body type, or empty for the shared file.
    pub character: String,
}

/// A scene, who its file says it is for, and its animation files.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SceneScope {
    pub id: String,
    pub file: String,
    pub supported_girls: Vec<String>,
    pub body_types: Vec<String>,
    pub files: Vec<AnimFile>,
}

fn scene_id_of(rel: &str) -> Option<String> {
    let rest = rel.strip_prefix("data/")?;
    let (_, tail) = rest.split_once('/')?;
    let name = tail.strip_prefix("scenes/")?.strip_suffix(".json")?;

    (!name.contains('/')).then(|| name.to_string())
}

/// The folder of the scene animations (`data/<ns>/asset_sync/strut/animations/scenes/`) and the file's name without its extension.
fn anim_name_of(rel: &str) -> Option<(String, String)> {
    let rest = rel.strip_prefix("data/")?;
    let (namespace, tail) = rest.split_once('/')?;
    let name = tail.strip_prefix("asset_sync/strut/animations/scenes/")?.strip_suffix(".strutanim")?;

    (!name.contains('/')).then(|| (format!("data/{namespace}/asset_sync/strut/animations/scenes/"), name.to_string()))
}

/// Splits `cowgirl-momo`, `cowgirl.bia` or `cowgirl` into the scene and the character (kind and name), as the game does: a dot before any dash is a skeleton.
fn split(name: &str) -> (String, String, String) {
    let dot = name.find('.');
    let dash = name.find('-');

    match (dot, dash) {
        (Some(d), dash) if dash.map_or(true, |x| d < x) => (name[..d].to_string(), "skeleton".into(), name[d + 1..].to_string()),
        (_, Some(x)) => (name[..x].to_string(), "girl".into(), name[x + 1..].to_string()),
        _ => (name.to_string(), "shared".into(), String::new()),
    }
}

fn strings_of(value: Option<&Value>) -> Vec<String> {
    value.and_then(Value::as_array).map(|list| list.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default()
}

/// Every scene of the pack with its scope and its animation files.
pub fn scopes(root: &Path) -> Vec<SceneScope> {
    let files = walk(root);
    let mut scenes: Vec<SceneScope> = Vec::new();

    for rel in &files {
        let Some(id) = scene_id_of(rel) else { continue };
        let json: Value = fs_ops::resolve(root, rel).ok().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);

        scenes.push(SceneScope { id, file: rel.clone(), supported_girls: strings_of(json.get("supported_girls")), body_types: strings_of(json.get("body_types")), files: Vec::new() });
    }

    for rel in &files {
        let Some((_, name)) = anim_name_of(rel) else { continue };
        let (scene, kind, character) = split(&name);

        if let Some(found) = scenes.iter_mut().find(|s| s.id == scene) {
            found.files.push(AnimFile { rel: rel.clone(), character_kind: kind, character });
        }
    }

    scenes.sort_by(|a, b| a.id.cmp(&b.id));
    scenes
}

/// The body type of a girl of the pack, from her girl file.
fn body_of(root: &Path, girl: &str) -> Option<String> {
    for rel in walk(root) {
        let Some(rest) = rel.strip_prefix("data/") else { continue };
        let Some((_, tail)) = rest.split_once('/') else { continue };

        if tail != format!("girls/{girl}.json") {
            continue;
        }

        let text = std::fs::read_to_string(fs_ops::resolve(root, &rel).ok()?).ok()?;
        let json: Value = serde_json::from_str(&text).ok()?;

        return json["components"]["pleasurehorizons:body"]["body_type"].as_str().map(String::from);
    }

    None
}

fn read_scene(root: &Path, scene: &SceneScope) -> Result<(Value, String)> {
    let path = fs_ops::resolve(root, &scene.file)?;
    let text = std::fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
    let json: Value = serde_json::from_str(&text).map_err(|e| Error::invalid(format!("{} is not valid JSON: {e}", scene.file)))?;

    Ok((json, text))
}

fn write_scene(root: &Path, scene: &SceneScope, json: &Value, before: &str) -> Result<()> {
    let text = serde_json::to_string_pretty(json).map_err(|e| Error::Other(e.to_string()))? + "\n";

    fs_ops::write_text(root, &scene.file, &text)?;
    history::record_now(root, &scene.file, Some(before), &text);

    Ok(())
}

fn find(root: &Path, scene: &str) -> Result<SceneScope> {
    scopes(root).into_iter().find(|s| s.id == scene).ok_or_else(|| Error::invalid(format!("There is no scene called \"{scene}\" in the pack.")))
}

/// Makes a scene made for one girl a scene for her whole skeleton: her animation file `<scene>-<girl>.strutanim` becomes `<scene>.<body>.strutanim`, and the scene file no longer
/// names her (`supported_girls`) and fits her body type (`body_types`). Her own file is moved, not copied, so there is one set of animations; the other girls on the skeleton play it
/// unless they have a file of their own.
pub fn make_skeleton_wide(root: &Path, scene_id: &str, girl: &str) -> Result<Vec<String>> {
    let scene = find(root, scene_id)?;
    let body = body_of(root, girl).ok_or_else(|| Error::invalid(format!("{girl} has no girl file with a body type in this pack, so her skeleton is not known.")))?;
    let source = scene.files.iter().find(|f| f.character_kind == "girl" && f.character == girl).ok_or_else(|| Error::invalid(format!("The scene {scene_id} has no animation file of {girl}'s ({scene_id}-{girl}.strutanim).")))?;
    let (folder, _) = anim_name_of(&source.rel).ok_or_else(|| Error::invalid("not a scene animation file"))?;
    let target = format!("{folder}{scene_id}.{body}.strutanim");

    if scene.files.iter().any(|f| f.rel == target) {
        return Err(Error::invalid(format!("{target} already exists, so {girl}'s file would overwrite it. Delete or rename one of them first.")));
    }

    fs_ops::rename(root, &source.rel, &target)?;

    let (mut json, before) = read_scene(root, &scene)?;
    let mut notes = vec![format!("{} is now {target}: every {body} girl plays it.", source.rel)];

    if let Some(object) = json.as_object_mut() {
        if let Some(list) = object.get_mut("supported_girls").and_then(Value::as_array_mut) {
            list.retain(|v| v.as_str() != Some(girl));

            if list.is_empty() {
                object.remove("supported_girls");
                notes.push("The scene no longer names her: it is for every girl its body types allow.".into());
            }
        }

        let bodies = strings_of(object.get("body_types"));

        if !bodies.contains(&body) {
            let mut next = bodies;

            next.push(body.clone());
            object.insert("body_types".into(), json!(next));
            notes.push(format!("The scene now fits the {body} body type."));
        }
    }

    write_scene(root, &scene, &json, &before)?;

    Ok(notes)
}

/// The opposite: a scene for a skeleton becomes one for a single girl. The skeleton's file (`<scene>.<body>.strutanim`, or the shared one when there is none) is moved to
/// `<scene>-<girl>.strutanim` (or copied, when `keep_original` is set, so the others on the skeleton keep theirs), and the scene file names her (`supported_girls`).
pub fn make_girl_specific(root: &Path, scene_id: &str, girl: &str, keep_original: bool) -> Result<Vec<String>> {
    let scene = find(root, scene_id)?;
    let body = body_of(root, girl).ok_or_else(|| Error::invalid(format!("{girl} has no girl file with a body type in this pack, so her skeleton is not known.")))?;
    let source = scene
        .files
        .iter()
        .find(|f| f.character_kind == "skeleton" && f.character == body)
        .or_else(|| scene.files.iter().find(|f| f.character_kind == "shared"))
        .ok_or_else(|| Error::invalid(format!("The scene {scene_id} has no animation file for the {body} skeleton or for everyone, so there is nothing to give {girl}.")))?;
    let (folder, _) = anim_name_of(&source.rel).ok_or_else(|| Error::invalid("not a scene animation file"))?;
    let target = format!("{folder}{scene_id}-{girl}.strutanim");

    if scene.files.iter().any(|f| f.rel == target) {
        return Err(Error::invalid(format!("{target} already exists. Delete or rename it first.")));
    }

    if keep_original {
        let bytes = fs_ops::read_bytes(root, &source.rel, 64 * 1024 * 1024)?;

        fs_ops::write_bytes(root, &target, &bytes)?;
    } else {
        fs_ops::rename(root, &source.rel, &target)?;
    }

    let (mut json, before) = read_scene(root, &scene)?;

    if let Some(object) = json.as_object_mut() {
        let mut girls = strings_of(object.get("supported_girls"));

        if !girls.iter().any(|g| g == girl) {
            girls.push(girl.to_string());
        }

        object.insert("supported_girls".into(), json!(girls));
    }

    write_scene(root, &scene, &json, &before)?;

    Ok(vec![format!("{} {} {target}; the scene is now for {girl}.", source.rel, if keep_original { "was copied to" } else { "is now" })])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(root: &Path, rel: &str, text: &str) {
        let path = root.join(rel);

        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn pack() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        put(root, "data/pleasurehorizons/girls/amy.json", r#"{ "id": "amy", "components": { "pleasurehorizons:body": { "body_type": "jenny" } } }"#);
        put(root, "data/pleasurehorizons/scenes/hug.json", r#"{ "type": "on_player", "supported_girls": ["amy"], "requires_affection": 3 }"#);
        put(root, "data/pleasurehorizons/asset_sync/strut/animations/scenes/hug-amy.strutanim", "AMY");

        dir
    }

    #[test]
    fn a_scene_becomes_skeleton_wide_and_back() {
        let dir = pack();
        let root = dir.path();
        let before = scopes(root);

        assert_eq!(before[0].supported_girls, vec!["amy"]);
        assert_eq!(before[0].files[0].character_kind, "girl");

        make_skeleton_wide(root, "hug", "amy").unwrap();

        let wide = scopes(root);

        assert!(wide[0].supported_girls.is_empty());
        assert_eq!(wide[0].body_types, vec!["jenny"]);
        assert_eq!(wide[0].files.len(), 1);
        assert_eq!((wide[0].files[0].character_kind.as_str(), wide[0].files[0].character.as_str()), ("skeleton", "jenny"));
        assert!(wide[0].files[0].rel.ends_with("hug.jenny.strutanim"));

        let json: Value = serde_json::from_str(&std::fs::read_to_string(root.join("data/pleasurehorizons/scenes/hug.json")).unwrap()).unwrap();

        assert_eq!(json["requires_affection"], 3);

        make_girl_specific(root, "hug", "amy", false).unwrap();

        let again = scopes(root);

        assert_eq!(again[0].supported_girls, vec!["amy"]);
        assert_eq!((again[0].files[0].character_kind.as_str(), again[0].files[0].character.as_str()), ("girl", "amy"));
        assert_eq!(std::fs::read_to_string(root.join(&again[0].files[0].rel)).unwrap(), "AMY");
    }

    #[test]
    fn a_file_already_there_is_not_overwritten() {
        let dir = pack();
        let root = dir.path();

        put(root, "data/pleasurehorizons/asset_sync/strut/animations/scenes/hug.jenny.strutanim", "OTHER");

        assert!(make_skeleton_wide(root, "hug", "amy").unwrap_err().to_string().contains("already exists"));
        assert_eq!(std::fs::read_to_string(root.join("data/pleasurehorizons/asset_sync/strut/animations/scenes/hug-amy.strutanim")).unwrap(), "AMY");
    }

    #[test]
    fn keeping_the_original_copies_it() {
        let dir = pack();
        let root = dir.path();

        make_skeleton_wide(root, "hug", "amy").unwrap();
        make_girl_specific(root, "hug", "amy", true).unwrap();

        assert_eq!(scopes(root)[0].files.len(), 2);
    }

    #[test]
    fn names_are_split_as_the_game_splits_them() {
        assert_eq!(split("cowgirl-momo"), ("cowgirl".into(), "girl".into(), "momo".into()));
        assert_eq!(split("cowgirl.bia"), ("cowgirl".into(), "skeleton".into(), "bia".into()));
        assert_eq!(split("cowgirl"), ("cowgirl".into(), "shared".into(), String::new()));
    }
}
