//! The checks: what is wrong with a pack, found without starting the game.
//!
//! A file is checked by what it is ([`crate::layout`]) against what the game says it has ([`crate::reference`]); what a file needs from other files of the pack
//! (a girl's trait that the pack defines, her model) is gathered first as [`PackFacts`].

pub mod assets;
mod bones;
pub mod cues;
mod girl;
mod misc;
mod roles;
mod scene;
mod social;
mod trait_file;
pub mod values;

pub use misc::PACK_FORMAT;

use crate::diagnostics::{file_level, Diagnostic, Severity, Sink};
use crate::json_ast;
use crate::layout::{locate, FileKind, ASSET_SYNC, GAME_NAMESPACE};
use crate::reference::{qualified, Reference};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

/// Largest text file that is read to be checked.
const MAX_TEXT_BYTES: u64 = 4 * 1024 * 1024;

/// What the rest of the pack has that one file may refer to.
#[derive(Debug, Default, Clone)]
pub struct PackFacts {
    /// The traits the pack defines, by full id, with the names of their settings.
    pub traits: BTreeMap<String, Vec<String>>,
    /// The ids of the girls the pack defines.
    pub girls: BTreeSet<String>,
    /// Every file of the pack, relative with `/`.
    pub files: BTreeSet<String>,
    /// The sound events the pack's sounds.json files define (full ids), and the cues its animations fire.
    pub sound_events: BTreeSet<String>,
    pub cues: BTreeSet<String>,
    /// The ids of every dialogue line in the pack.
    pub dialogue_ids: BTreeSet<String>,
    /// The bones of each model the pack has, by the model's path, for the checks that name bones.
    pub model_bones: BTreeMap<String, BTreeSet<String>>,
}

/// Reads the pack's files to learn what it defines.
pub fn collect_facts(root: &Path) -> PackFacts {
    let mut facts = PackFacts::default();

    for rel in walk(root) {
        facts.files.insert(rel.clone());

        let located = locate(&rel);

        match located.kind {
            FileKind::Girl => {
                if let Some(id) = located.id {
                    facts.girls.insert(id);
                }
            }
            FileKind::Trait => {
                let (Some(namespace), Some(id)) = (located.namespace, located.id) else { continue };
                let params = read_text(&root.join(&rel))
                    .and_then(|text| json_ast::parse(&text).ok())
                    .and_then(|node| node.get("params").and_then(|p| p.value.as_object().map(|o| o.iter().map(|m| m.key.clone()).collect::<Vec<_>>())))
                    .unwrap_or_default();

                facts.traits.insert(qualified(&format!("{namespace}:{id}")), params);
            }
            FileKind::Dialogue => {
                let lines = read_text(&root.join(&rel)).and_then(|text| json_ast::parse(&text).ok());

                for line in lines.as_ref().and_then(|n| n.as_array()).unwrap_or(&[]) {
                    if let Some(id) = line.get("id").and_then(|m| m.value.as_str()) {
                        facts.dialogue_ids.insert(id.to_string());
                    }
                }
            }
            FileKind::SoundsJson => {
                if let (Some(namespace), Some(node)) = (located.namespace, read_text(&root.join(&rel)).and_then(|text| json_ast::parse(&text).ok())) {
                    facts.sound_events.extend(cues::sound_events_of(&namespace, &node));
                }
            }
            FileKind::Animation => {
                if let Some((_, fired)) = animation_checked(root, &rel) {
                    facts.cues.extend(fired);
                }
            }
            FileKind::Model => {
                if let Some(bones) = model_bones(&root.join(&rel)) {
                    facts.model_bones.insert(rel.clone(), bones);
                }
            }
            _ => {}
        }
    }

    facts
}

/// An animation's problems and the cues it fires. Reading one is the slowest part of checking a pack and every save checks the pack, so the answer is kept until the file changes.
fn animation_checked(root: &Path, rel: &str) -> Option<(Vec<Diagnostic>, Vec<String>)> {
    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::time::SystemTime;

    type Entry = (u64, SystemTime, Vec<Diagnostic>, Vec<String>);
    static CACHE: Mutex<Option<HashMap<std::path::PathBuf, Entry>>> = Mutex::new(None);

    let path = root.join(rel);
    let meta = fs::metadata(&path).ok()?;
    let stamp = (meta.len(), meta.modified().ok()?);

    if let Ok(guard) = CACHE.lock() {
        if let Some((len, modified, problems, fired)) = guard.as_ref().and_then(|c| c.get(&path)) {
            if (*len, *modified) == stamp {
                return Some((problems.clone(), fired.clone()));
            }
        }
    }

    let bytes = fs::read(&path).ok()?;
    let (problems, fired) = cues::animation(rel, &bytes);

    if let Ok(mut guard) = CACHE.lock() {
        guard.get_or_insert_with(HashMap::new).insert(path, (stamp.0, stamp.1, problems.clone(), fired.clone()));
    }

    Some((problems, fired))
}

/// The names of a model's bones. Reading a model is not free and every save checks the pack, so the answer is kept until the file changes.
fn model_bones(path: &Path) -> Option<BTreeSet<String>> {
    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::time::SystemTime;

    static CACHE: Mutex<Option<HashMap<std::path::PathBuf, (u64, SystemTime, BTreeSet<String>)>>> = Mutex::new(None);

    let meta = fs::metadata(path).ok()?;
    let stamp = (meta.len(), meta.modified().ok()?);
    let mut guard = CACHE.lock().ok()?;
    let cache = guard.get_or_insert_with(HashMap::new);

    if let Some((len, modified, bones)) = cache.get(path) {
        if (*len, *modified) == stamp {
            return Some(bones.clone());
        }
    }

    let model = crate::strut_model::read(&fs::read(path).ok()?).ok()?;
    let bones: BTreeSet<String> = model.group_bones.iter().map(|b| b.name.clone()).collect();

    cache.insert(path.to_path_buf(), (stamp.0, stamp.1, bones.clone()));
    Some(bones)
}

/// Checks the text of one file. `rel` is its path in the pack.
pub fn check_text(rel: &str, text: &str, reference: &Reference, facts: &PackFacts) -> Vec<Diagnostic> {
    let located = locate(rel);
    let mut sink = Sink::new(rel, text);
    let json_kind = rel.ends_with(".json") || rel.ends_with(".mcmeta");

    if !json_kind {
        return sink.items;
    }

    let root = match json_ast::parse(text) {
        Ok(root) => root,
        Err(error) => {
            sink.error("syntax", error.message, error.offset, error.offset + 1);
            return sink.items;
        }
    };

    let stem = located.id.clone().unwrap_or_default();

    match located.kind {
        FileKind::PackMeta => misc::mcmeta(&mut sink, &root),
        FileKind::Girl => {
            girl::check(&mut sink, &root, &stem, reference, facts);
            bones::merchant(&mut sink, &root, &stem, facts);
        }
        FileKind::Scene => scene::check(&mut sink, &root, reference),
        FileKind::Trait => trait_file::check(&mut sink, &root, reference),
        FileKind::Dialogue => misc::dialogue(&mut sink, &root, reference, facts),
        FileKind::Social => social::check(&mut sink, &root, reference),
        FileKind::Physics => bones::physics(&mut sink, &root, rel, facts),
        FileKind::ArmourWearer => bones::wearer(&mut sink, &root, rel, facts),
        FileKind::ArmourSet => bones::set(&mut sink, &root, facts),
        FileKind::ArmourLook => bones::look(&mut sink, &root),
        FileKind::Lang => misc::lang(&mut sink, &root),
        FileKind::EventBindings => cues::bindings(&mut sink, &root, reference, facts),
        FileKind::KeyframeEvents => cues::old_keyframe_events(&mut sink, &root),
        FileKind::SoundsJson => cues::sounds_json(&mut sink, &root, located.namespace.as_deref().unwrap_or(GAME_NAMESPACE), facts),
        _ => {}
    }

    sink.items
}

/// Checks the whole pack: every file, and what the pack as a whole needs.
pub fn check_project(root: &Path, reference: &Reference) -> Vec<Diagnostic> {
    let facts = collect_facts(root);
    let mut found = Vec::new();

    if !facts.files.contains("pack.mcmeta") {
        found.push(file_level(Severity::Error, "missing-mcmeta", "The pack has no pack.mcmeta, so Minecraft will not load it. Add one with a pack_format and a description.", "pack.mcmeta"));
    }

    for rel in &facts.files {
        let path = root.join(rel);
        let located = locate(rel);
        let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

        if rel.starts_with(&format!("data/")) && rel.split('/').nth(2) == Some(ASSET_SYNC) {
            let head = read_head(&path);

            found.extend(assets::check(rel, size, &head));
        }

        if (rel.ends_with(".json") || rel.ends_with(".mcmeta")) && size <= MAX_TEXT_BYTES {
            if let Some(text) = read_text(&path) {
                found.extend(check_text(rel, &text, reference, &facts));
            }
        }

        if located.kind == FileKind::Animation {
            if let Some((problems, _)) = animation_checked(root, rel) {
                found.extend(problems);
            }
        }

        if located.kind == FileKind::Girl {
            if let Some(id) = &located.id {
                found.extend(girl_files(rel, id, &facts));
            }
        }

        if located.kind == FileKind::Scene {
            if let Some(id) = &located.id {
                found.extend(scene_animation(rel, id, &facts));
            }
        }
    }

    found.extend(roles::check(root, &facts));

    found
}

/// The scenes the game ships, whose animations are in the game: a pack that overrides one does not have to bring its animation.
const GAME_SCENES: [&str; 10] = ["anal", "blow_job", "cowgirl", "doggy", "face_fuck", "masturbation", "mating_press", "missionary", "paizuri", "standing_sex"];

/// A scene the pack adds needs an animation to play: a file in `strut/animations/scenes/` named for it (`<scene>.strutanim`, `<scene>.<skeleton>.strutanim` or
/// `<scene>-<girl>.strutanim`). Without one the scene is offered and then nothing plays.
fn scene_animation(scene_file: &str, id: &str, facts: &PackFacts) -> Vec<Diagnostic> {
    if GAME_SCENES.contains(&id) {
        return Vec::new();
    }

    let has = facts.files.iter().any(|path| {
        let in_scenes = path.contains(&format!("/{ASSET_SYNC}/strut/animations/scenes/"));
        let name = path.rsplit('/').next().unwrap_or("");

        let split = path.contains(&format!("/{ASSET_SYNC}/strut/animations/scenes/{id}/")) || path.ends_with(&format!("/{ASSET_SYNC}/strut/scenes/{id}.strutscene"));

        split || (in_scenes && name.ends_with(".strutanim") && (name.starts_with(&format!("{id}.")) || name.starts_with(&format!("{id}-"))))
    });

    if has {
        Vec::new()
    } else {
        vec![file_level(Severity::Warning, "scene-without-animation", format!("The scene {id} has no animation in the pack: add {id}.strutanim (everyone), {id}.<skeleton>.strutanim or {id}-<girl>.strutanim under data/{GAME_NAMESPACE}/{ASSET_SYNC}/strut/animations/scenes/. Until then it is offered and nothing plays."), scene_file)]
    }
}

/// A girl's own files: the model, the texture and the animations the game looks for under her id. Missing all of them is how a girl with no art looks (the game shows
/// a placeholder); missing some is a mistake.
fn girl_files(girl_file: &str, id: &str, facts: &PackFacts) -> Vec<Diagnostic> {
    let base = format!("data/{GAME_NAMESPACE}/{ASSET_SYNC}");
    let nude = format!("{base}/{}", crate::paths::girl_model("nude", id));
    let dressed = format!("{base}/{}", crate::paths::girl_model("dressed", id));
    let (old_nude, old_dressed) = (format!("{base}/{}", crate::paths::girl_model_old("nude", id)), format!("{base}/{}", crate::paths::girl_model_old("dressed", id)));
    /* One model is enough: a girl with only a nude (or only a dressed) model is always drawn from the one she has. */
    let has_model = [&nude, &dressed, &old_nude, &old_dressed].iter().any(|p| facts.files.contains(*p));
    let wanted = [
        (format!("{base}/textures/entities/{id}.png"), "her texture"),
        (format!("{base}/{}", crate::paths::girl_base(id)), "her animations"),
    ];
    let old_base = format!("{base}/{}", crate::paths::girl_base_old(id));
    let character_base = format!("{base}/strut/animations/base/characters/{id}.strutanim");
    let skeleton_base = facts.files.iter().any(|f| f.starts_with(&format!("{base}/strut/animations/base/skeletons/")));
    let mut missing: Vec<&str> = wanted.iter().filter(|(path, _)| !facts.files.contains(path) && !facts.files.contains(&old_base) && !facts.files.contains(&character_base) && !skeleton_base).map(|(_, what)| *what).collect();

    if !has_model {
        missing.insert(0, "her model (nude, or dressed, or both)");
    }

    if missing.len() == wanted.len() + 1 {
        vec![file_level(Severity::Info, "girl-without-files", format!("The pack has no files for {id} (model, texture, animations). She will show as the placeholder girl unless the game has them already."), girl_file)]
    } else if !missing.is_empty() {
        vec![file_level(Severity::Warning, "girl-files-missing", format!("{id} is missing {}. Add them under {base}/.", missing.join(", ")), girl_file)]
    } else {
        Vec::new()
    }
}

/// Every file under `root`, relative with `/`, leaving out the tool's own folder and hidden files.
pub fn walk(root: &Path) -> Vec<String> {
    let mut found = Vec::new();

    fn visit(dir: &Path, prefix: &str, found: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(dir) else { return };
        let mut entries: Vec<_> = entries.flatten().collect();

        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let name = entry.file_name().to_string_lossy().to_string();

            if name.starts_with('.') {
                continue;
            }

            let rel = if prefix.is_empty() { name.clone() } else { format!("{prefix}/{name}") };

            match entry.file_type() {
                Ok(kind) if kind.is_dir() => visit(&entry.path(), &rel, found),
                Ok(kind) if kind.is_file() => found.push(rel),
                _ => {}
            }
        }
    }

    visit(root, "", &mut found);
    found
}

fn read_text(path: &Path) -> Option<String> {
    String::from_utf8(fs::read(path).ok()?).ok()
}

fn read_head(path: &Path) -> Vec<u8> {
    use std::io::Read;

    let mut head = vec![0u8; 32];
    let read = fs::File::open(path).and_then(|mut file| file.read(&mut head)).unwrap_or(0);

    head.truncate(read);
    head
}

#[cfg(test)]
mod tests;
