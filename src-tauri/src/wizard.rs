//! The new girl wizard: what a girl needs to exist in the game, made together. A girl is her profile (`girls/<id>.json`) and, in asset sync, her nude and dressed models, her
//! texture, her event bindings and her chat lines; the models must be built on the skeleton of the body type she is given, because the scenes are made for a skeleton.
//! This checks the models against that skeleton and writes every file, so nothing is left for the person to remember.

use crate::{examples, text, App, Reply};
use base64::Engine as _;
use pc_core::fs_ops;
use pc_core::strut_model::{self, Model};
use pc_core::templates::{self, GirlSpec};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use tauri::State;

/* A girl's own files are always under the game's namespace, whatever the pack's. */
const SYNC: &str = "data/pleasurehorizons/asset_sync";

/// Where the bytes of a file the person picked come from: a real path, or one of the examples.
fn bytes_of(source: &str) -> Result<Vec<u8>, String> {
    match examples::strip(source) {
        Some(path) => examples::bytes(path).ok_or_else(|| format!("{source} is not one of the examples.")),
        None => std::fs::read(source).map_err(|e| format!("{source} could not be read: {e}")),
    }
}

fn read_model(source: &str) -> Result<Model, String> {
    strut_model::read(&bytes_of(source)?).map_err(|e| format!("{} cannot be read as a Strut model: {e}", file_name(source)))
}

fn file_name(source: &str) -> String {
    source.rsplit(['/', '\\']).next().unwrap_or(source).to_string()
}

#[derive(Serialize)]
pub struct ModelCheck {
    ok: bool,
    /// What is wrong, or what was found.
    message: String,
    bones: usize,
    cubes: usize,
}

#[derive(Serialize)]
pub struct SkeletonCheck {
    /// The base girl whose skeleton the body type is.
    reference: Option<String>,
    same: bool,
    missing: Vec<String>,
    extra: Vec<String>,
    /// The same bones in a different order (the game hashes the order, so animations would be refused).
    order_differs: bool,
}

#[derive(Serialize)]
pub struct ModelReport {
    nude: Option<ModelCheck>,
    dressed: Option<ModelCheck>,
    texture: Option<String>,
    skeleton: Option<SkeletonCheck>,
}

fn check_one(source: &Option<String>) -> Option<(ModelCheck, Option<Model>)> {
    let source = source.as_deref().filter(|s| !s.is_empty())?;

    Some(match read_model(source) {
        Ok(model) => {
            let message = format!("{} bones, {} cubes{}", model.group_bones.len(), model.cubes.len(), if model.meshes.is_empty() { String::new() } else { format!(", {} meshes", model.meshes.len()) });

            (ModelCheck { ok: true, message, bones: model.group_bones.len(), cubes: model.cubes.len() }, Some(model))
        }
        Err(message) => (ModelCheck { ok: false, message, bones: 0, cubes: 0 }, None),
    })
}

/// The base girl whose nude model is the skeleton of a body type.
fn reference_for(body_type: &str) -> Option<(String, Model)> {
    for name in examples::names() {
        let Some(id) = name.strip_prefix("data/pleasurehorizons/girls/").and_then(|n| n.strip_suffix(".json")) else { continue };
        let text = examples::text(&name)?;
        let parsed: serde_json::Value = serde_json::from_str(&text).ok()?;

        if parsed["components"]["pleasurehorizons:body"]["body_type"].as_str() != Some(body_type) {
            continue;
        }

        let model = examples::bytes(&format!("assets/pleasurehorizons/{}", pc_core::paths::girl_model("nude", id))).or_else(|| examples::bytes(&format!("assets/pleasurehorizons/{}", pc_core::paths::girl_model_old("nude", id)))).and_then(|b| strut_model::read(&b).ok())?;

        return Some((id.to_string(), model));
    }

    None
}

fn skeleton(body_type: &str, model: &Model) -> Option<SkeletonCheck> {
    let (reference, base) = reference_for(body_type)?;
    let want: Vec<&str> = base.group_bones.iter().map(|b| b.name.as_str()).collect();
    let have: Vec<&str> = model.group_bones.iter().map(|b| b.name.as_str()).collect();
    let want_set: BTreeSet<&str> = want.iter().copied().collect();
    let have_set: BTreeSet<&str> = have.iter().copied().collect();
    let missing: Vec<String> = want_set.difference(&have_set).map(|s| s.to_string()).collect();
    let extra: Vec<String> = have_set.difference(&want_set).map(|s| s.to_string()).collect();

    Some(SkeletonCheck { reference: Some(reference), same: want == have, order_differs: missing.is_empty() && extra.is_empty() && want != have, missing, extra })
}

/// Checks the models and texture a girl was given, and her skeleton against the body type's.
#[tauri::command(async)]
pub fn check_girl_models(body_type: String, nude: Option<String>, dressed: Option<String>, texture: Option<String>) -> ModelReport {
    let nude_result = check_one(&nude);
    let dressed_result = check_one(&dressed);
    let texture_message = texture.as_deref().filter(|t| !t.is_empty()).map(|source| match bytes_of(source) {
        Ok(bytes) if bytes.len() >= 24 && bytes.starts_with(&[0x89, b'P', b'N', b'G']) => {
            let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
            let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);

            if width > 8192 || height > 8192 { format!("{width} x {height}: asset sync refuses pictures over 8192 pixels") } else { format!("{width} x {height}") }
        }
        Ok(_) => "this is not a PNG".to_string(),
        Err(e) => e,
    });

    let skeleton_check = nude_result.as_ref().and_then(|(_, model)| model.as_ref()).and_then(|model| skeleton(&body_type, model));

    ModelReport { nude: nude_result.map(|(c, _)| c), dressed: dressed_result.map(|(c, _)| c), texture: texture_message, skeleton: skeleton_check }
}

#[derive(Deserialize)]
pub struct NewGirl {
    id: String,
    name: String,
    archetype: String,
    body_type: String,
    tame_item: String,
    /// Real paths of the models and texture she brings, or `example:` paths when she starts from a base girl's.
    nude: String,
    dressed: String,
    texture: String,
    /// Make a basic physics file for the soft bones the models have.
    physics: bool,
}

#[derive(Serialize)]
pub struct CreatedGirl {
    girl: String,
    written: Vec<String>,
    notes: Vec<String>,
}

fn write(root: &std::path::Path, written: &mut Vec<String>, rel: &str, bytes: &[u8]) -> Result<(), String> {
    let path = fs_ops::resolve(root, rel).map_err(text)?;

    if path.exists() {
        return Err(format!("{rel} already exists in the pack, so nothing was made. Choose another id, or remove it first."));
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(text)?;
    }

    std::fs::write(&path, bytes).map_err(|e| format!("{rel} could not be written: {e}"))?;
    written.push(rel.to_string());
    Ok(())
}

/// Makes the girl: her profile and every file she needs, all together or none.
#[tauri::command(async)]
pub fn create_girl(app: State<App>, request: NewGirl) -> Reply<CreatedGirl> {
    let (root, meta) = app.open()?;
    let id = request.id.trim().to_string();
    let rel = templates::path_for(&meta, &pc_core::layout::FileKind::Girl, &id).map_err(text)?;

    if request.body_type.is_empty() {
        return Err("Choose a body type: it decides which scenes fit her.".into());
    }

    /* Everything is read and checked before anything is written. */
    let nude_bytes = bytes_of(&request.nude)?;
    let dressed_bytes = bytes_of(&request.dressed)?;
    let texture_bytes = bytes_of(&request.texture)?;
    let nude = strut_model::read(&nude_bytes).map_err(|e| format!("the nude model cannot be read: {e}"))?;
    let dressed = strut_model::read(&dressed_bytes).map_err(|e| format!("the dressed model cannot be read: {e}"))?;

    if !texture_bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return Err("the texture is not a PNG.".into());
    }

    let mut notes = Vec::new();

    if let Some(check) = skeleton(&request.body_type, &nude) {
        if !check.same {
            let reference = check.reference.unwrap_or_default();

            notes.push(if check.order_differs {
                format!("Her nude model has the bones of {}'s skeleton ({}) but in a different order, and the game checks the order, so animations made for {} would be refused. Re-export it from {}'s model.", request.body_type, reference, request.body_type, reference)
            } else {
                format!("Her nude model is not built on the {} skeleton ({}'s): missing {}; extra {}. Scenes made for {} will not move her correctly until the bones match.", request.body_type, reference, if check.missing.is_empty() { "none".into() } else { check.missing.join(", ") }, if check.extra.is_empty() { "none".into() } else { check.extra.join(", ") }, request.body_type)
            });
        }
    }

    let mut written = Vec::new();
    let name = if request.name.trim().is_empty() { id.clone() } else { request.name.trim().to_string() };
    let profile = templates::girl(&GirlSpec { id: &id, name: &name, archetype: &request.archetype, body_type: &request.body_type, tame_item: if request.tame_item.is_empty() { "minecraft:allium" } else { &request.tame_item }, hitbox_height: 1.8 });

    write(&root, &mut written, &rel, profile.as_bytes())?;
    write(&root, &mut written, &format!("{SYNC}/{}", pc_core::paths::girl_model("nude", &id)), &nude_bytes)?;
    write(&root, &mut written, &format!("{SYNC}/{}", pc_core::paths::girl_model("dressed", &id)), &dressed_bytes)?;
    write(&root, &mut written, &format!("{SYNC}/textures/entities/{id}.png"), &texture_bytes)?;
    write(&root, &mut written, &format!("{SYNC}/strut/event_bindings/girls/{id}.json"), b"{\n  \"bindings\": {}\n}\n")?;

    if request.physics {
        for (kind, model) in [("nude", &nude), ("dressed", &dressed)] {
            let bones = pc_core::oldpack::physics_bones(model);

            if let Some((preset, names)) = pc_core::oldpack::basic_physics(&bones) {
                write(&root, &mut written, &format!("{SYNC}/{}", pc_core::paths::girl_physics(kind, &id)), (serde_json::to_string_pretty(&preset).map_err(text)? + "\n").as_bytes())?;
                notes.push(format!("A basic {kind} physics file was made for {}. Tune it in Blockbench's Physics tab.", names.join(", ")));
            }
        }
    }

    Ok(CreatedGirl { girl: rel, written, notes })
}

/// A texture of a base girl as a data URL, to show next to her name when choosing one to start from.
#[tauri::command(async)]
pub fn example_preview(id: String) -> Option<String> {
    examples::bytes(&format!("assets/pleasurehorizons/textures/entities/{id}.png")).map(|b| format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(b)))
}
