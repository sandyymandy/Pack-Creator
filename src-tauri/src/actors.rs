//! Actor presets for the Strut Toolkit's animation workspace: ready-made actors (a character's model, its texture, its role and its skeleton id) written as preset folders the
//! toolkit looks in. One folder holds the base mod's own characters and the male; another holds the characters of the pack that is open, which the person registers (all of
//! them, or the ones they pick). The toolkit finds the folders through Blockbench's storage, which the Pack Creator fills when it opens (or reaches) Blockbench.

use crate::{blockbench, examples};
use pc_core::project::ProjectMeta;
use pc_core::{paths, strut_model};
use serde::Serialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const BASE_FOLDER: &str = "base-game";

#[derive(Serialize, Default)]
pub struct Summary {
    pub base: usize,
    pub pack: usize,
    pub folders: Vec<String>,
    /// Whether a Blockbench that is open was told at once; otherwise it is told the next time the Pack Creator opens it.
    pub announced: bool,
}

fn title(id: &str) -> String {
    id.split('_').filter(|w| !w.is_empty()).map(|w| w[..1].to_uppercase() + &w[1..]).collect::<Vec<_>>().join(" ")
}

fn folder_name(text: &str) -> String {
    let name: String = text.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();

    name.trim_matches('-').to_string()
}

/// The model with its skeleton id written in, so that the toolkit's check that the model is on the actor's skeleton holds.
fn with_id(bytes: &[u8], id: &str) -> Vec<u8> {
    strut_model::set_skeleton_id(bytes, Some(id)).unwrap_or_else(|_| bytes.to_vec())
}

fn write(folder: &Path, rel: &str, bytes: &[u8]) -> Result<(), String> {
    let path = folder.join(rel);

    std::fs::create_dir_all(path.parent().unwrap_or(folder)).map_err(|e| e.to_string())?;
    std::fs::write(&path, bytes).map_err(|e| format!("{} could not be written: {e}", path.display()))
}

/// A folder with its models, textures and its `presets.json`; an old one is replaced.
struct Folder {
    path: PathBuf,
    name: String,
    presets: Vec<Value>,
}

impl Folder {
    fn new(path: PathBuf, name: &str) -> Result<Folder, String> {
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
        Ok(Folder { path, name: name.to_string(), presets: Vec::new() })
    }

    #[allow(clippy::too_many_arguments)]
    fn add(&mut self, id: &str, name: &str, group: &str, role: &str, skeleton: &str, model: &[u8], texture: Option<&[u8]>) -> Result<(), String> {
        let model_rel = format!("models/{id}.strutmodel");

        write(&self.path, &model_rel, &with_id(model, skeleton))?;

        let mut textures = Vec::new();

        if let Some(bytes) = texture {
            let rel = format!("textures/{id}.png");

            write(&self.path, &rel, bytes)?;
            textures.push(rel);
        }

        self.presets.push(json!({ "id": id, "name": name, "group": group, "role": role, "skeleton": skeleton, "model": model_rel, "textures": textures }));
        Ok(())
    }

    fn finish(self) -> Result<usize, String> {
        let count = self.presets.len();

        if count == 0 {
            let _ = std::fs::remove_dir_all(&self.path);
            return Ok(0);
        }

        write(&self.path, "presets.json", (serde_json::to_string_pretty(&json!({ "version": 1, "name": self.name, "presets": self.presets })).map_err(|e| e.to_string())? + "\n").as_bytes())?;
        Ok(count)
    }
}

fn skeleton_of(bytes: &[u8], body_type: Option<&str>, fallback: &str) -> String {
    strut_model::read(bytes).ok().and_then(|m| m.skeleton_id).or_else(|| body_type.map(|b| format!("pleasurehorizons:{b}"))).unwrap_or_else(|| fallback.to_string())
}

fn body_type_of(text: &str) -> Option<String> {
    serde_json::from_str::<Value>(text).ok().and_then(|v| v["components"]["pleasurehorizons:body"]["body_type"].as_str().map(String::from))
}

/// The base mod's characters (every base girl, nude and dressed) and the male.
pub fn write_base() -> Result<usize, String> {
    write_base_into(blockbench::actors_dir().join(BASE_FOLDER))
}

fn write_base_into(path: PathBuf) -> Result<usize, String> {
    let mut folder = Folder::new(path, "Pleasure Horizons")?;
    let mut ids: Vec<String> = examples::names().into_iter().filter_map(|n| n.strip_prefix("data/pleasurehorizons/girls/")?.strip_suffix(".json").map(String::from)).collect();

    ids.sort();

    let texture = |id: &str| examples::bytes(&format!("assets/pleasurehorizons/textures/entities/{id}.png"));

    for id in ids {
        let body = examples::text(&format!("data/pleasurehorizons/girls/{id}.json")).and_then(|t| body_type_of(&t));

        for variant in ["nude", "dressed"] {
            let Some(bytes) = examples::bytes(&format!("assets/pleasurehorizons/{}", paths::girl_model(variant, &id))).or_else(|| examples::bytes(&format!("assets/pleasurehorizons/{}", paths::girl_model_old(variant, &id)))) else { continue };
            let skeleton = skeleton_of(&bytes, body.as_deref(), &format!("pleasurehorizons:{id}"));
            let (preset, label) = if variant == "nude" { (id.clone(), title(&id)) } else { (format!("{id}_dressed"), format!("{} (dressed)", title(&id))) };

            folder.add(&preset, &label, "Girls", "host", &skeleton, &bytes, texture(&id).as_deref())?;
        }
    }

    folder.add("male", "Male (the player)", "Male", "male", pc_core::scene_split::MALE_ID, pc_core::scene_split::male_model_bytes(), texture("male").as_deref())?;
    folder.finish()
}

/// The characters of the pack that is open: each girl that has a model, nude and dressed. `only` limits it to the girls named.
pub fn write_pack(root: &Path, meta: &ProjectMeta, only: &[String]) -> Result<usize, String> {
    let files = pc_core::check::walk(root);
    let ids = pc_core::skeleton_ids::plan(root, &Default::default());
    let id_of = |girl: &str| ids.assignments.iter().find(|a| a.kind == pc_core::skeleton_ids::Kind::Model && a.character == girl).map(|a| a.id.clone());
    let mut folder = Folder::new(blockbench::actors_dir().join(format!("pack-{}", folder_name(&meta.name))), &meta.name)?;
    let mut girls: Vec<String> = files.iter().filter_map(|f| pc_core::layout::locate(f).id.filter(|_| pc_core::layout::locate(f).kind == pc_core::layout::FileKind::Girl)).collect();

    girls.sort();
    girls.dedup();

    for girl in girls.into_iter().filter(|g| only.is_empty() || only.contains(g)) {
        let find = |suffix: String| files.iter().find(|f| f.contains("/asset_sync/") && f.ends_with(&suffix)).cloned();
        let texture = find(format!("/textures/entities/{girl}.png")).and_then(|f| std::fs::read(root.join(f)).ok());
        let name = files
            .iter()
            .find(|f| f.ends_with(&format!("/girls/{girl}.json")))
            .and_then(|f| std::fs::read_to_string(root.join(f)).ok())
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|v| v["components"]["pleasurehorizons:identity"]["name"].as_str().map(String::from))
            .unwrap_or_else(|| title(&girl));

        for variant in ["nude", "dressed"] {
            let Some(rel) = find(format!("/{}", paths::girl_model(variant, &girl))).or_else(|| find(format!("/{}", paths::girl_model_old(variant, &girl)))) else { continue };
            let Ok(bytes) = std::fs::read(root.join(&rel)) else { continue };
            let skeleton = skeleton_of(&bytes, None, &id_of(&girl).unwrap_or_else(|| format!("{}:{girl}", pc_core::layout::GAME_NAMESPACE)));
            let (preset, label) = if variant == "nude" { (girl.clone(), name.clone()) } else { (format!("{girl}_dressed"), format!("{name} (dressed)")) };

            folder.add(&preset, &label, &meta.name, "host", &skeleton, &bytes, texture.as_deref())?;
        }
    }

    folder.finish()
}

/// Writes the base mod's presets and the pack's, and tells a Blockbench that is open where they are.
pub fn register(root: Option<(&Path, &ProjectMeta)>, only: &[String]) -> Result<Summary, String> {
    let mut summary = Summary { base: write_base()?, ..Default::default() };

    if let Some((root, meta)) = root {
        summary.pack = write_pack(root, meta, only)?;
    }

    summary.folders = blockbench::actor_folders();
    summary.announced = blockbench::announce_actors().is_ok();
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_base_game_and_the_male_are_written_as_presets_the_toolkit_can_read() {
        let dir = std::env::temp_dir().join(format!("pc-actors-{}", std::process::id()));
        let count = write_base_into(dir.clone()).unwrap();
        let index: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("presets.json")).unwrap()).unwrap();
        let presets = index["presets"].as_array().unwrap();

        assert_eq!(presets.len(), count);
        assert!(count >= 10, "{count} presets");

        for preset in presets {
            let model = std::fs::read(dir.join(preset["model"].as_str().unwrap())).unwrap();
            let read = strut_model::read(&model).unwrap();

            /* The model carries the id the preset names, so the toolkit's check that they agree holds. */
            assert_eq!(read.skeleton_id.as_deref(), preset["skeleton"].as_str(), "{}", preset["id"]);
            assert!(preset["skeleton"].as_str().unwrap().contains(':'));
            assert!(!preset["role"].as_str().unwrap().is_empty());

            for texture in preset["textures"].as_array().unwrap() {
                assert!(dir.join(texture.as_str().unwrap()).is_file());
            }
        }

        let male = presets.iter().find(|p| p["id"] == "male").expect("the male");

        assert_eq!(male["skeleton"], pc_core::scene_split::MALE_ID);
        assert_eq!(male["role"], "male");
        assert_eq!(male["textures"].as_array().unwrap().len(), 1, "the male has his texture");
        assert!(presets.iter().any(|p| p["group"] == "Girls" && p["role"] == "host"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
