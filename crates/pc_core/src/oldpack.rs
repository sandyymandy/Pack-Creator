//! Bringing an old girl in from her old resource pack: her models become `.strutmodel`, her one all-in-one animation file is split into a base file and one file for each of
//! her scenes (named for the parts each scene plays) and becomes `.strutanim`, her cue table becomes event bindings, and her texture, sounds and language files are
//! copied into the pack's asset sync folder. Her profile (the girl file and her scene files) comes from [`crate::import`].

use crate::check::assets;
use crate::convert::animation::{self, Options};
use crate::convert::events::{self, Mapping};
use crate::convert::model;
use crate::convert::{Finding, Severity};
use crate::error::{Error, Result};
use crate::import::{self, ImportedScene, Level, Note};
use crate::reference::Reference;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

const GAME_NAMESPACE: &str = "pleasurehorizons";
const SYNC: &str = "data/pleasurehorizons/asset_sync";

/// An old resource pack that is opened: the folder of its assets, and the girls it holds.
pub struct OldPack {
    /// `assets/<namespace>/` of the pack, or the folder itself when that is where the models are.
    pub root: PathBuf,
    pub namespace: String,
    /// The ids of the girls with a model in it, as the pack writes them.
    pub girls: Vec<String>,
    /// Set for a pack made for the old mod, which was rewritten into this layout when it was opened ([`crate::fapcraft`]).
    pub rewritten: bool,
    /// Profiles made for the girls of a rewritten pack, by id, and what was noticed while rewriting it.
    profiles: std::collections::BTreeMap<String, String>,
    notes: Vec<Note>,
    temp: Option<PathBuf>,
}

impl OldPack {
    /// The profile made for a girl of a rewritten pack, if there is one.
    pub fn profile_text(&self, id: &str) -> Option<&str> {
        self.profiles.get(id).map(String::as_str)
    }
}

impl Drop for OldPack {
    fn drop(&mut self) {
        if let Some(dir) = &self.temp {
            let _ = fs::remove_dir_all(dir);
        }
    }
}

/// Opens a resource pack given as a folder or a zip (a zip is unpacked to a temporary folder that is removed afterwards).
pub fn open(path: &Path) -> Result<OldPack> {
    let (dir, temp) = if path.is_dir() {
        (path.to_path_buf(), None)
    } else {
        let temp = std::env::temp_dir().join(format!("ph-oldpack-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)));

        unzip(path, &temp)?;
        (temp.clone(), Some(temp))
    };

    let Some((root, namespace)) = asset_root(&dir) else {
        if let Some(temp) = &temp {
            let _ = fs::remove_dir_all(temp);
        }

        return Err(Error::invalid(format!("{} has no assets/<namespace>/ folder with geo, animations, sounds or textures in it, so it is not an old girl's resource pack.", path.display())));
    };

    /* A pack made for the old mod is rewritten into the layout read below. */
    let (root, rewritten, temp) = if crate::fapcraft::is_layout(&root) {
        let into = std::env::temp_dir().join(format!("ph-fapcraft-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)));
        let result = crate::fapcraft::rewrite(&root, &into);

        if let Some(old) = &temp {
            let _ = fs::remove_dir_all(old);
        }

        match result {
            Ok(done) => {
                let girls = done.girls.clone();

                return Ok(OldPack { root: into.clone(), namespace, girls, rewritten: true, profiles: done.profiles, notes: done.notes, temp: Some(into) });
            }
            Err(error) => {
                let _ = fs::remove_dir_all(&into);
                return Err(error);
            }
        }
    } else {
        (root, false, temp)
    };

    let mut girls = BTreeSet::new();

    for kind in ["dressed", "nude"] {
        if let Ok(entries) = fs::read_dir(root.join("geo").join(kind)) {
            for entry in entries.flatten() {
                if let Some(id) = entry.file_name().to_string_lossy().strip_suffix(".geo.json") {
                    girls.insert(id.to_string());
                }
            }
        }
    }

    if girls.is_empty() {
        return Err(Error::invalid("The pack has no geo/dressed/<id>.geo.json or geo/nude/<id>.geo.json, so it holds no girl."));
    }

    Ok(OldPack { root, namespace, girls: girls.into_iter().collect(), rewritten, profiles: Default::default(), notes: Vec::new(), temp })
}

fn asset_root(dir: &Path) -> Option<(PathBuf, String)> {
    let score = |base: &Path| ["geo", "animations", "keyframe_events", "sounds", "textures"].iter().filter(|d| base.join(d).is_dir()).count();
    let assets = dir.join("assets");
    let mut best: Option<(usize, String)> = None;

    if let Ok(entries) = fs::read_dir(&assets) {
        for entry in entries.flatten().filter(|e| e.path().is_dir()) {
            let s = score(&entry.path());

            if s > 0 && best.as_ref().map_or(true, |(b, _)| s > *b) {
                best = Some((s, entry.file_name().to_string_lossy().to_string()));
            }
        }
    }

    if let Some((_, name)) = best {
        return Some((assets.join(&name), name));
    }

    (score(dir) > 0).then(|| (dir.to_path_buf(), GAME_NAMESPACE.to_string()))
}

fn unzip(path: &Path, into: &Path) -> Result<()> {
    let file = fs::File::open(path).map_err(|e| Error::io(path, e))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| Error::invalid(format!("{} is not a zip or a folder: {e}", path.display())))?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| Error::Other(format!("the zip could not be read: {e}")))?;
        /* A name that points outside the folder is skipped, not followed. */
        let Some(name) = entry.enclosed_name() else { continue };
        let target = into.join(name);

        if entry.is_dir() {
            fs::create_dir_all(&target).map_err(|e| Error::io(&target, e))?;
            continue;
        }

        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
        }

        let mut out = fs::File::create(&target).map_err(|e| Error::io(&target, e))?;

        std::io::copy(&mut entry, &mut out).map_err(|e| Error::io(&target, e))?;
    }

    Ok(())
}

// =================================================================
// Splitting the old animation file
// =================================================================

struct Split {
    base: Value,
    scene_files: Vec<(String, Value)>,
}

/// The old file holds every clip, named `animation.<girl>.<name>`. A scene's clips are taken out under `animation.<scene>.<role>`; what no scene uses is the base file, as
/// `animation.base.<name>`.
fn split_animations(old: &Value, girl_id: &str, scenes: &[ImportedScene], notes: &mut Vec<Note>) -> Split {
    let empty = Map::new();
    let animations = old.get("animations").and_then(Value::as_object).unwrap_or(&empty);
    let mut short: Vec<(String, String)> = Vec::new();
    let mut foreign: BTreeSet<String> = BTreeSet::new();

    for key in animations.keys() {
        let parsed = key.strip_prefix("animation.").and_then(|rest| rest.split_once('.')).filter(|(owner, name)| !owner.is_empty() && !name.is_empty());
        let name = parsed.map_or(key.clone(), |(_, name)| name.to_string());
        let own = parsed.map_or(false, |(owner, _)| owner.to_lowercase() == girl_id);

        match short.iter().position(|(n, _)| *n == name) {
            Some(at) => {
                if own {
                    short[at].1 = key.clone();
                }
            }
            None => short.push((name, key.clone())),
        }

        if let Some((owner, _)) = parsed {
            if !own {
                foreign.insert(owner.to_string());
            }
        }
    }

    if !foreign.is_empty() {
        notes.push(Note { level: Level::Changed, message: format!("Some animations are named for \"{}\" instead of her; the prefix is dropped and they are kept by their short names.", foreign.into_iter().collect::<Vec<_>>().join(", ")) });
    }

    let mut header = Map::new();

    header.insert("format_version".into(), old.get("format_version").cloned().unwrap_or(json!("1.8.0")));

    if let Some(g) = old.get("geckolib_format_version") {
        header.insert("geckolib_format_version".into(), g.clone());
    }

    let lookup = |name: &str| short.iter().find(|(n, _)| n == name).or_else(|| short.iter().find(|(n, _)| *n == name.to_lowercase())).map(|(_, key)| key.clone());
    let mut used: HashSet<String> = HashSet::new();
    let mut scene_files = Vec::new();

    for scene in scenes {
        let mut out = Map::new();

        for (role, old_name) in &scene.roles {
            match lookup(old_name) {
                Some(key) => {
                    out.insert(format!("animation.{}.{role}", scene.id), animations[&key].clone());
                    used.insert(key);
                }
                None => notes.push(Note { level: Level::Error, message: format!("The scene \"{}\" uses the animation \"{old_name}\" ({role}), but her animation file has no such animation.", scene.old_name) }),
            }
        }

        let mut file = header.clone();

        file.insert("animations".into(), Value::Object(out));
        scene_files.push((scene.id.clone(), Value::Object(file)));
    }

    let mut base = Map::new();

    for (name, key) in &short {
        if !used.contains(key) {
            base.insert(format!("animation.base.{name}"), animations[key].clone());
        }
    }

    let mut file = header;

    file.insert("animations".into(), Value::Object(base));
    Split { base: Value::Object(file), scene_files }
}

/// The same finding said once for each axis (x, y, z), bone, moment or value is said once, with how many times it happened. Two findings are the same when they differ only in
/// the digits and the quoted names and values they mention.
pub(crate) fn collapse(notes: Vec<Note>) -> Vec<Note> {
    let mut seen: Vec<(Level, String, String, usize)> = Vec::new();

    for note in notes {
        let message = [" the x value", " the y value", " the z value"].iter().fold(note.message.clone(), |m, axis| m.replace(axis, " a value"));
        let message = [" for x", " for y", " for z"].iter().fold(message, |m, axis| m.strip_suffix(axis).map(String::from).unwrap_or(m));

        /* What the finding says apart from which bone, which moment and which value. */
        let mut shape = String::new();
        let mut quoted = false;

        for c in message.chars() {
            if c == '"' {
                quoted = !quoted;
            } else if !quoted && !c.is_ascii_digit() {
                shape.push(c);
            }
        }

        match seen.iter_mut().find(|(level, _, s, _)| *level == note.level && *s == shape) {
            Some(entry) => entry.3 += 1,
            None => seen.push((note.level, message, shape, 1)),
        }
    }

    seen.into_iter().map(|(level, message, _, count)| Note { level, message: if count > 1 { format!("{message} (and {} more like it)", count - 1) } else { message } }).collect()
}

pub(crate) fn note_of(finding: &Finding) -> Note {
    let place = [finding.file.clone(), finding.clip.as_ref().map(|c| format!("clip \"{c}\"")), finding.bone.as_ref().map(|b| format!("bone \"{b}\""))].into_iter().flatten().collect::<Vec<_>>().join(", ");
    let message = if place.is_empty() { finding.message.clone() } else { format!("{place}: {}", finding.message) };

    Note { level: match finding.severity { Severity::Lossy => Level::Review, Severity::Changed => Level::Changed, Severity::Info => Level::Kept }, message }
}

// =================================================================
// Importing one girl
// =================================================================

/// What the person chose for the girl.
pub struct Request<'a> {
    /// Her id as the old pack writes it.
    pub raw_id: &'a str,
    /// Her old profile (the text of the JSON file), or None to make a plain one.
    pub profile: Option<&'a str>,
    pub body_type: &'a str,
    pub namespace: &'a str,
    /// Add the fluid to her cum cue (needs the `nut` bone in her nude model).
    pub liquid: bool,
    /// Files and scene ids the pack already has, so nothing is overwritten.
    pub existing_files: &'a HashSet<String>,
    pub existing_scenes: &'a [String],
    /// What the person decided for particular cues (a sound, a custom event, left out); a cue with no rule becomes a marker.
    pub rules: &'a Mapping,
    /// Make a basic physics file for a model that came without one (the soft bones it can find by name).
    pub physics_preset: bool,
    /// A girl imported after another from the same pack: the pack's shared textures, sounds and language files came with the first one.
    pub skip_shared: bool,
    /// The tame items the pack's girls already use. Each girl needs her own (the item is how the game finds her), so one of these in her old profile is swapped for a free one.
    pub taken_tame_items: &'a [String],
    /// The scenes the person settled on (their names and the clips of each part), replacing the ones her profile lists. None keeps her profile's own.
    pub scene_plan: Option<&'a [crate::review::ScenePlan]>,
}

/// What importing her makes: files to write (path in the pack and bytes), entries to merge into JSON objects, and the report.
pub struct Outcome {
    pub girl_file: String,
    pub files: Vec<(String, Vec<u8>)>,
    /// JSON objects to merge into (path, entries): the sounds and the language files.
    pub merges: Vec<(String, BTreeMap<String, Value>)>,
    pub notes: Vec<Note>,
}

/// Which `soft` entry and preset a bone belongs to, by its name (spaces, dashes and case ignored): the chest, the bottom (cheeks and ass) and the belly, the soft bodies Strut makes of a mesh.
fn soft_group(name: &str) -> Option<(&'static str, &'static str)> {
    let lower = name.to_lowercase().replace(['_', '-', ' '], "");

    match lower.as_str() {
        "boobs" | "boobl" | "boobr" | "breastl" | "breastr" | "leftbreast" | "rightbreast" => Some(("breasts", "pouch")),
        "belly" => Some(("belly", "belly")),
        "booty" | "cheekl" | "cheekr" | "fleshl" | "fleshr" | "assl" | "assr" | "ass" | "buttl" | "buttr" | "butt" | "buttockl" | "buttockr" | "buttocks" | "glutel" | "gluter" => Some(("butt", "seat")),
        _ => None,
    }
}

/// A bone of a model as the basic physics sees it: its name, and whether it is drawn with meshes that can move (flesh moves meshes only; a bone of cubes is a spring bone instead).
#[derive(Debug, Clone)]
pub struct PhysicsBone {
    pub name: String,
    pub mesh: bool,
    /// Whether the bone draws anything itself. A bone that only holds others (a `boobs` over `boobL` and `boobR`) draws nothing, and moving it would move them twice.
    pub drawn: bool,
}

/// The bones of a model for the basic physics. A mesh bound to an armature is moved by the armature, not by flesh, so a bone with only those counts as cubes.
pub fn physics_bones(model: &crate::strut_model::Model) -> Vec<PhysicsBone> {
    model.group_bones.iter().map(|bone| PhysicsBone {
        name: bone.name.clone(),
        mesh: bone.elements.iter().any(|e| e.kind == 0 && model.meshes.get(e.index as usize).is_some_and(|m| m.armature_index < 0)),
        drawn: !bone.elements.is_empty(),
    }).collect()
}

/// The physics a model can be given in one click, found by bone name: the chest, the bottom and the belly. A bone made of meshes becomes **flesh** (Strut's own presets, with the left and right of a
/// pair in one entry so they press each other); a bone made of cubes cannot be flesh, so it becomes a **spring bone**. Returns the file and the names found.
pub fn basic_physics(bones: &[PhysicsBone]) -> Option<(Value, Vec<String>)> {
    let mut groups: Vec<(&str, &str, Vec<String>)> = Vec::new();
    let mut springs = Map::new();
    let mut names = Vec::new();

    for bone in bones {
        let Some((entry, preset)) = soft_group(&bone.name) else { continue };

        if !bone.drawn {
            continue;
        }

        names.push(bone.name.clone());

        if !bone.mesh {
            springs.insert(bone.name.clone(), json!({ "preset": "soft-bone" }));
            continue;
        }

        match groups.iter_mut().find(|g| g.0 == entry) {
            Some(group) => group.2.push(bone.name.clone()),
            None => groups.push((entry, preset, vec![bone.name.clone()])),
        }
    }

    if names.is_empty() {
        return None;
    }

    let mut flesh = Map::new();

    for (entry, preset, members) in groups {
        flesh.insert(entry.to_string(), json!({ "preset": preset, "bones": members }));
    }

    let mut file = json!({ "version": 1, "bones": springs, "chains": {} });

    if !flesh.is_empty() {
        file["flesh"] = Value::Object(flesh);
    }

    Some((file, names))
}

/// A physics file's text with the basic soft bodies of a model added to it: a bone the file already has (as a spring bone or in a `soft` entry) is left exactly as it is, and only the soft bodies it lacks
/// are added. Returns the new text and the bone names added, or None when no bone of the model has a name the basic setup knows, or every one is already in the file.
pub fn add_basic_physics(existing: Option<&str>, bones: &[PhysicsBone]) -> Result<Option<(String, Vec<String>)>> {
    let Some((basic, _)) = basic_physics(bones) else { return Ok(None) };
    let mut file: Value = match existing {
        Some(text) => serde_json::from_str(text).map_err(|e| Error::invalid(format!("The physics file is not valid JSON, so nothing was added to it: {e}")))?,
        None => json!({ "version": 1, "bones": {}, "chains": {} }),
    };
    let root = file.as_object_mut().ok_or_else(|| Error::invalid("The physics file is not a JSON object."))?;
    let mut present: Vec<String> = root.get("bones").and_then(Value::as_object).map(|m| m.keys().cloned().collect()).unwrap_or_default();

    /* Strut's old name for the section is `soft`; it is the same system, so a file that has it is brought to `flesh`. */
    if let Some(old) = root.remove("soft") {
        if !root.contains_key("flesh") {
            root.insert("flesh".to_string(), old);
        }
    }

    if let Some(soft) = root.get("flesh") {
        let soft = soft.as_object().ok_or_else(|| Error::invalid("The physics file's \"flesh\" is not an object."))?;

        for entry in soft.values() {
            present.extend(entry.get("bones").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_string));
        }
    }

    let mut added = Vec::new();

    {
        let springs = root.entry("bones").or_insert_with(|| json!({}));
        let springs = springs.as_object_mut().ok_or_else(|| Error::invalid("The physics file's \"bones\" is not an object."))?;

        for (name, bone) in basic["bones"].as_object().into_iter().flatten() {
            if !present.contains(name) {
                springs.insert(name.clone(), bone.clone());
                added.push(name.clone());
            }
        }
    }

    if let Some(wanted_flesh) = basic["flesh"].as_object() {
        let soft = root.entry("flesh").or_insert_with(|| json!({}));
        let soft = soft.as_object_mut().ok_or_else(|| Error::invalid("The physics file's \"flesh\" is not an object."))?;

        for (entry, wanted) in wanted_flesh {
            let remaining: Vec<String> = wanted["bones"].as_array().into_iter().flatten().filter_map(Value::as_str).filter(|b| !present.iter().any(|p| p == b)).map(str::to_string).collect();

            if remaining.is_empty() {
                continue;
            }

            let mut name = entry.clone();
            let mut number = 2;

            while soft.contains_key(&name) {
                name = format!("{entry}{number}");
                number += 1;
            }

            soft.insert(name, json!({ "preset": wanted["preset"], "bones": remaining }));
            added.extend(remaining);
        }
    }

    if root.get("flesh").and_then(Value::as_object).is_some_and(|m| m.is_empty()) {
        root.remove("flesh");
    }

    if added.is_empty() {
        return Ok(None);
    }

    Ok(Some((serde_json::to_string_pretty(&file).unwrap() + "\n", added)))
}

/// Items a girl is given as her tame item when the one her old profile names is already another girl's.
const TAME_ITEM_POOL: [&str; 24] = [
    "minecraft:allium", "minecraft:poppy", "minecraft:azure_bluet", "minecraft:cornflower", "minecraft:lily_of_the_valley", "minecraft:oxeye_daisy", "minecraft:pink_tulip", "minecraft:red_tulip",
    "minecraft:orange_tulip", "minecraft:white_tulip", "minecraft:blue_orchid", "minecraft:dandelion", "minecraft:sweet_berries", "minecraft:honey_bottle", "minecraft:cookie", "minecraft:melon_slice",
    "minecraft:apple", "minecraft:golden_carrot", "minecraft:pumpkin_pie", "minecraft:cake", "minecraft:sugar", "minecraft:glow_berries", "minecraft:cocoa_beans", "minecraft:bread",
];

/// Swaps her tame item for a free one when it is already taken, in the girl file's bytes. Returns the note to show, or None when nothing had to change.
fn free_tame_item(bytes: &mut Vec<u8>, taken: &[String]) -> Option<Note> {
    let mut file: Value = serde_json::from_slice(bytes).ok()?;
    let current = file.pointer("/components/pleasurehorizons:taste/tame_item")?.as_str()?.to_string();

    if !taken.iter().any(|t| *t == current) {
        return None;
    }

    let free = TAME_ITEM_POOL.iter().find(|item| !taken.iter().any(|t| t == **item))?;

    *file.pointer_mut("/components/pleasurehorizons:taste/tame_item")? = Value::String((*free).to_string());
    *bytes = (serde_json::to_string_pretty(&file).ok()? + "
").into_bytes();

    Some(Note { level: Level::Review, message: format!("Her tame item {current} is already another girl's, and each girl needs her own, so it is now {free}. Change it in her file if you want another.") })
}

/// The tame items the girls in a pack's `data/<namespace>/girls` folders already use.
pub fn taken_tame_items(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let Ok(namespaces) = fs::read_dir(root.join("data")) else { return found };

    for namespace in namespaces.flatten() {
        let Ok(girls) = fs::read_dir(namespace.path().join("girls")) else { continue };

        for girl in girls.flatten() {
            let Ok(text) = fs::read_to_string(girl.path()) else { continue };
            let Ok(file) = serde_json::from_str::<Value>(&text) else { continue };

            if let Some(item) = file.pointer("/components/pleasurehorizons:taste/tame_item").and_then(Value::as_str) {
                found.push(item.to_string());
            }
        }
    }

    found
}

/// Reads many files under `base` at once (a pack holds thousands of sounds), keeping their order.
fn read_many(base: &Path, rels: &[String]) -> Result<Vec<(String, Vec<u8>)>> {
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(8);
    let chunk = rels.len().div_ceil(threads).max(1);
    let parts: Vec<Result<Vec<(String, Vec<u8>)>>> = std::thread::scope(|scope| {
        let handles: Vec<_> = rels
            .chunks(chunk)
            .map(|part| scope.spawn(move || part.iter().map(|rel| fs::read(base.join(rel)).map(|bytes| (rel.clone(), bytes)).map_err(|e| Error::io(rel, e))).collect::<Result<Vec<_>>>()))
            .collect();

        handles.into_iter().map(|h| h.join().unwrap_or_else(|_| Err(Error::Other("a file could not be read".into())))).collect()
    });
    let mut all = Vec::with_capacity(rels.len());

    for part in parts {
        all.extend(part?);
    }

    Ok(all)
}

/// The cues (the tokens of her sound effects) her old animations fire, and how many times, so the person can decide what each becomes before importing.
pub fn scan_cues(pack: &OldPack, raw_id: &str) -> Result<BTreeMap<String, usize>> {
    let id = lowercase_id(raw_id);
    let rules = Mapping::new();
    let mut tokens: BTreeMap<String, usize> = BTreeMap::new();
    let mut cues: BTreeMap<String, usize> = BTreeMap::new();
    let mut docs: Vec<Value> = Vec::new();
    let single = [raw_id.to_string(), id.clone()].iter().map(|n| pack.root.join(format!("animations/{n}.animation.json"))).find(|p| p.is_file());

    if let Some(path) = single {
        docs.push(serde_json::from_str(&fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?).map_err(|e| Error::invalid(format!("Her animation file is not valid JSON: {e}")))?);
    } else {
        for rel in crate::check::walk(&pack.root.join("animations")) {
            if rel.ends_with(".animation.json") || rel.ends_with(".scene.json") {
                if let Ok(doc) = serde_json::from_str::<Value>(&fs::read_to_string(pack.root.join("animations").join(&rel)).unwrap_or_default()) {
                    docs.push(doc);
                }
            }
        }
    }

    for doc in &docs {
        let mut mapper = events::marker_mapper(&rules, &mut tokens, &mut cues);

        animation::convert(doc, Options { file: None, fingerprint: 0, map_event: Some(&mut mapper) });
    }

    Ok(cues)
}

fn lowercase_id(raw: &str) -> String {
    raw.to_lowercase()
}

pub fn import_girl(pack: &OldPack, request: &Request, reference: &Reference) -> Result<Outcome> {
    let id = lowercase_id(request.raw_id);
    let mut notes: Vec<Note> = Vec::new();

    if !id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') || id.is_empty() {
        return Err(Error::invalid(format!("\"{}\" cannot be a girl id: use only a-z, 0-9 and _ .", request.raw_id)));
    }

    if id != request.raw_id {
        notes.push(Note { level: Level::Changed, message: format!("The id \"{}\" is written in lower case, \"{id}\" (asset sync refuses capitals).", request.raw_id) });
    }

    if pack.namespace != GAME_NAMESPACE {
        notes.push(Note { level: Level::Changed, message: format!("The pack's namespace \"{}\" is moved to \"{GAME_NAMESPACE}\", the only one the game reads for a girl's files.", pack.namespace) });
    }

    if !request.skip_shared {
        notes.extend(pack.notes.iter().cloned());
    }

    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut merges: Vec<(String, BTreeMap<String, Value>)> = Vec::new();

    /* Her profile: the girl file and a file for each scene. */
    let profile_text = match request.profile.or_else(|| pack.profiles.get(&id).map(String::as_str)) {
        Some(text) => text.to_string(),
        None => {
            notes.push(Note { level: Level::Review, message: "No profile was given, so a plain one is made. Check her name, tame item and stats.".into() });
            json!({ "id": id, "name": id, "tame_item": "minecraft:carrot", "hitbox_height": 1.8, "attributes": { "health": 20.0, "speed": 0.2, "damage": 2.0 } }).to_string()
        }
    };
    let profile_text = match request.scene_plan {
        Some(plan) => crate::review::apply_plan(&profile_text, plan)?,
        None => profile_text,
    };
    let imported = import::import_profile(&profile_text, &id, request.body_type, request.namespace, request.existing_scenes, reference)?;

    /* Her notes about the files that are not imported yet are replaced by the real thing below. */
    notes.extend(imported.notes.into_iter().filter(|n| !n.message.contains("are not imported yet") && !n.message.contains("is not imported yet")));
    files.extend(imported.files.into_iter().map(|(rel, text)| (rel, text.into_bytes())));

    let girl_file = format!("data/{}/girls/{id}.json", request.namespace);

    /* Her tame item is how the game finds her, so it cannot be one another girl has. */
    if let Some((_, bytes)) = files.iter_mut().find(|(rel, _)| *rel == girl_file) {
        if let Some(note) = free_tame_item(bytes, request.taken_tame_items) {
            notes.push(note);
        }
    }
    let mut lang_entries: BTreeMap<String, Value> = imported.lang.iter().map(|(k, v)| (k.clone(), Value::String(v.clone()))).collect();

    /* Models. The nude one names the skeleton her animations are authored against. */
    let mut fingerprint = 0;
    let mut has_tip = false;
    let mut bones_of: BTreeMap<&str, Vec<PhysicsBone>> = BTreeMap::new();

    for kind in ["nude", "dressed"] {
        let source = pack.root.join("geo").join(kind).join(format!("{}.geo.json", request.raw_id));

        let Ok(text) = fs::read_to_string(&source) else {
            let other = if kind == "nude" { "dressed" } else { "nude" };

            /* One model is enough: a girl with only a nude (or only a dressed) model is always drawn from the one she has. */
            if pack.root.join("geo").join(other).join(format!("{}.geo.json", request.raw_id)).is_file() {
                notes.push(Note { level: Level::Kept, message: format!("She has no {kind} model, so she is always drawn from her {other} one.") });
            } else {
                notes.push(Note { level: Level::Error, message: format!("There is no geo/{kind}/{}.geo.json or geo/{other}/{}.geo.json (she needs a nude or a dressed model, or both).", request.raw_id, request.raw_id) });
            }

            continue;
        };
        let json: Value = serde_json::from_str(&text).map_err(|e| Error::invalid(format!("geo/{kind}/{}.geo.json is not valid JSON: {e}", request.raw_id)))?;
        let converted = model::convert_geo(&json, Some(&format!("geo/{kind}/{}.geo.json", request.raw_id)));
        let bytes = model::encode(&converted.model).map_err(Error::Other)?;

        notes.extend(converted.findings.items.iter().map(note_of));
        notes.push(Note { level: Level::Kept, message: format!("The {kind} model: {} bones, {} cubes.", converted.model.group_bones.len(), converted.model.cubes.len()) });

        /* An old model is made of cubes only, so none of its bones can be flesh. */
        bones_of.insert(kind, converted.model.group_bones.iter().map(|b| PhysicsBone { name: b.name.clone(), mesh: false, drawn: true }).collect());

        if kind == "nude" || fingerprint == 0 {
            fingerprint = model::fingerprint(&converted.model);
        }

        if kind == "nude" {
            has_tip = converted.model.group_bones.iter().any(|b| b.name == "nut");
        }

        files.push((format!("{SYNC}/{}", crate::paths::girl_model(kind, &id)), bytes));

        /* Her physics file, if the old pack has one for this model. */
        let physics = pack.root.join(format!("strut/physics/models/{kind}/{}.json", request.raw_id));

        if let Ok(bytes) = fs::read(&physics) {
            files.push((format!("{SYNC}/{}", crate::paths::girl_physics(kind, &id)), bytes));
            notes.push(Note { level: Level::Kept, message: format!("Her {kind} physics file.") });
        } else if request.physics_preset {
            match basic_physics(&bones_of[kind]) {
                Some((preset, names)) => {
                    files.push((format!("{SYNC}/{}", crate::paths::girl_physics(kind, &id)), (serde_json::to_string_pretty(&preset).unwrap() + "\n").into_bytes()));
                    notes.push(Note { level: Level::Review, message: format!("She had no {kind} physics file, so physics was made for {} (spring bones: an old model is made of cubes, and flesh needs meshes). Tune it in the physics file.", names.join(", ")) });
                }
                None => notes.push(Note { level: Level::Kept, message: format!("She had no {kind} physics file, and no bone of her {kind} model has a name the basic setup knows (boobL, boobR, belly, cheekL, cheekR, assL, assR...), so none was made.") }),
            }
        }
    }

    /* Animations and cues. */
    let rules = request.rules;
    let mut tokens: BTreeMap<String, usize> = BTreeMap::new();
    let mut cues: BTreeMap<String, usize> = BTreeMap::new();

    let mut convert_file = |doc: &Value, name: &str, target: String, files: &mut Vec<(String, Vec<u8>)>, notes: &mut Vec<Note>| -> Result<()> {
        let mut mapper = events::marker_mapper(&rules, &mut tokens, &mut cues);
        let converted = animation::convert(doc, Options { file: Some(name.to_string()), fingerprint, map_event: Some(&mut mapper) });

        notes.extend(converted.findings.items.iter().map(note_of));
        files.push((target, animation::encode(&converted.animation).map_err(Error::Other)?));
        Ok(())
    };

    let old_animation = [request.raw_id.to_string(), id.clone()].iter().map(|n| pack.root.join(format!("animations/{n}.animation.json"))).find(|p| p.is_file());

    if let Some(path) = old_animation {
        let old: Value = serde_json::from_str(&fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?).map_err(|e| Error::invalid(format!("Her animation file is not valid JSON: {e}")))?;
        let split = split_animations(&old, &id, &imported.scenes, &mut notes);
        let base_count = split.base.get("animations").and_then(Value::as_object).map_or(0, Map::len);

        convert_file(&split.base, &format!("base.{id}"), format!("{SYNC}/{}", crate::paths::girl_base(&id)), &mut files, &mut notes)?;

        for (scene_id, doc) in &split.scene_files {
            convert_file(doc, &format!("{scene_id}-{id}"), format!("{SYNC}/strut/animations/scenes/{scene_id}-{id}.strutanim"), &mut files, &mut notes)?;
        }

        notes.push(Note { level: Level::Kept, message: format!("Her animations: {base_count} base animation(s) and {} scene file(s).", split.scene_files.len()) });
    } else if pack.root.join(format!("animations/base.{}.animation.json", request.raw_id)).is_file() {
        /* The pack already uses the newer layout: every file is converted as it is. */
        let mut count = 0;
        let girl_ids: std::collections::HashSet<String> = pack.girls.iter().map(|g| g.to_lowercase()).collect();
        /* Whether a file's name is one of some girl's (`base.<id>` or `<scene>-<id>`), found by looking the endings up, not by trying every girl. */
        let owner_known = |name: &str| {
            name.strip_prefix("base.").is_some_and(|rest| girl_ids.contains(rest)) || name.match_indices('-').any(|(at, _)| girl_ids.contains(&name[at + 1..]))
        };

        for rel in crate::check::walk(&pack.root.join("animations")) {
            let Some(stem) = rel.strip_suffix(".animation.json").or_else(|| rel.strip_suffix(".scene.json")) else { continue };

            /* A pack of several girls keeps all their files in one folder: hers are `base.<id>` and `<scene>-<id>`, anyone else's are left for her own turn, and what
               belongs to no girl (the shared base, blocks, items) comes once, with the first. */
            let name = stem.rsplit('/').next().unwrap_or(stem).to_lowercase();
            let belongs_to = |girl: &str| {
                let girl = girl.to_lowercase();

                name == format!("base.{girl}") || name.ends_with(&format!("-{girl}"))
            };

            if belongs_to(&id) || belongs_to(request.raw_id) {
                /* hers */
            } else if request.skip_shared || owner_known(&name) {
                continue;
            }

            let doc: Value = serde_json::from_str(&fs::read_to_string(pack.root.join("animations").join(&rel)).map_err(|e| Error::io(&rel, e))?).map_err(|e| Error::invalid(format!("animations/{rel} is not valid JSON: {e}")))?;

            convert_file(&doc, &rel, format!("{SYNC}/strut/animations/{}.strutanim", stem.to_lowercase()), &mut files, &mut notes)?;
            count += 1;
        }

        notes.push(Note { level: Level::Kept, message: format!("Her animations were already one file for each scene: {count} converted as they are.") });
    } else {
        notes.push(Note { level: Level::Error, message: format!("There is no animation file (animations/{}.animation.json).", request.raw_id) });
    }

    /* Her cue table becomes bindings, with her chat lines written in them (the game does not read the old file). */
    let table = [request.raw_id.to_string(), id.clone()].iter().map(|n| pack.root.join(format!("keyframe_events/{n}.json"))).find(|p| p.is_file());
    let mut bindings = None;

    if let Some(path) = table {
        let text = fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
        let json: Value = serde_json::from_str(&text).map_err(|e| Error::invalid(format!("Her keyframe_events file is not valid JSON: {e}")))?;
        let registry = events::parse_keyframe_events(&json, &format!("{id}.json"));

        let set = events::binding_set_for(&registry, tokens.keys().cloned());

        notes.push(Note { level: Level::Kept, message: format!("Her cues: {} binding(s) from her cue table.", set["bindings"].as_object().map_or(0, Map::len)) });
        bindings = Some(set);
    } else {
        notes.push(Note { level: Level::Kept, message: "She has no keyframe_events file in the old pack, so her cues have no sounds or chat lines.".into() });
    }

    if request.liquid && has_tip {
        let mut set = bindings.take().unwrap_or_else(|| json!({ "bindings": {} }));
        let table = set["bindings"].as_object_mut().unwrap();
        let entry = table.entry("cumin").or_insert_with(|| json!({}));

        entry["strut_physics:fluid"] = json!({ "bone": "nut", "count": 14, "speed": 2.2, "spread": 0.25, "duration": 0.8 });
        bindings = Some(set);
        notes.push(Note { level: Level::Kept, message: "The cum cue shows the fluid from her \"nut\" bone.".into() });
    } else if request.liquid {
        notes.push(Note { level: Level::Kept, message: "Her nude model has no \"nut\" bone, so the fluid is not added to her cum cue.".into() });
    }

    if let Some(set) = bindings {
        files.push((format!("{SYNC}/strut/event_bindings/girls/{id}.json"), (serde_json::to_string_pretty(&set).unwrap() + "\n").into_bytes()));
    }

    /* Textures, sounds, language. */
    let texture = [request.raw_id.to_string(), id.clone()].iter().map(|n| pack.root.join(format!("textures/entities/{n}.png"))).find(|p| p.is_file());

    match texture {
        Some(path) => {
            files.push((format!("{SYNC}/textures/entities/{id}.png"), fs::read(&path).map_err(|e| Error::io(&path, e))?));
            notes.push(Note { level: Level::Kept, message: "Her texture.".into() });
        }
        None => notes.push(Note { level: Level::Error, message: format!("textures/entities/{}.png is missing.", request.raw_id) }),
    }

    let girl_textures: std::collections::HashSet<String> = pack.girls.iter().map(|g| format!("entities/{}.png", g.to_lowercase())).collect();
    let shared_textures: Vec<String> = crate::check::walk(&pack.root.join("textures"))
        .into_iter()
        .filter(|_| !request.skip_shared)
        /* Her own texture is written with her above, and another girl's comes with her when she is imported. */
        .filter(|rel| *rel != format!("entities/{}.png", request.raw_id) && !girl_textures.contains(&rel.to_lowercase()))
        .collect();

    for (rel, bytes) in read_many(&pack.root.join("textures"), &shared_textures)? {
        files.push((format!("{SYNC}/textures/{rel}"), bytes));
    }

    let mut sound_files = 0;

    let sound_list: Vec<String> = crate::check::walk(&pack.root.join("sounds")).into_iter().filter(|_| !request.skip_shared).collect();

    for (rel, bytes) in read_many(&pack.root.join("sounds"), &sound_list)? {
        files.push((format!("{SYNC}/sounds/{rel}"), bytes));
        sound_files += 1;
    }

    if sound_files > 0 {
        notes.push(Note { level: Level::Kept, message: format!("{sound_files} sound file(s).") });
    }

    if let Some(text) = fs::read_to_string(pack.root.join("sounds.json")).ok().filter(|_| !request.skip_shared) {
        match serde_json::from_str::<Value>(&text) {
            Ok(Value::Object(map)) => merges.push((format!("{SYNC}/sounds.json"), map.into_iter().collect())),
            _ => notes.push(Note { level: Level::Error, message: "sounds.json is not valid JSON, so her sounds are not defined.".into() }),
        }
    }

    for rel in crate::check::walk(&pack.root.join("lang")).into_iter().filter(|_| !request.skip_shared) {
        let source = pack.root.join("lang").join(&rel);
        let text = fs::read_to_string(&source).unwrap_or_default();

        if let Some(stem) = rel.strip_suffix(".lang") {
            let entries: BTreeMap<String, Value> = text.lines().filter(|l| !l.starts_with('#')).filter_map(|l| l.split_once('=')).filter(|(k, _)| !k.trim().is_empty()).map(|(k, v)| (k.trim().to_string(), Value::String(v.trim().to_string()))).collect();

            notes.push(Note { level: Level::Changed, message: format!("lang/{rel} (the old .lang format) is written as lang/{}.json.", stem.to_lowercase()) });
            merges.push((format!("{SYNC}/lang/{}.json", stem.to_lowercase()), entries));
        } else if rel.ends_with(".json") {
            match serde_json::from_str::<Value>(&text) {
                Ok(Value::Object(map)) => merges.push((format!("{SYNC}/lang/{rel}"), map.into_iter().collect())),
                _ => notes.push(Note { level: Level::Error, message: format!("lang/{rel} is not valid JSON.") }),
            }
        }
    }

    if !lang_entries.is_empty() {
        let path = format!("{SYNC}/lang/en_us.json");

        match merges.iter_mut().find(|(p, _)| *p == path) {
            Some((_, entries)) => entries.append(&mut lang_entries),
            None => merges.push((path, lang_entries)),
        }
    }

    /* Asset sync's rules: a file it would refuse is left out, with the reason. */
    let mut kept = Vec::new();

    for (rel, bytes) in files {
        let problems: Vec<_> = assets::check(&rel, bytes.len() as u64, &bytes[..bytes.len().min(32)]);
        let refused = problems.iter().find(|d| d.severity == crate::diagnostics::Severity::Error || d.code == "not-synced");

        match refused {
            Some(problem) => notes.push(Note { level: Level::Dropped, message: format!("{rel} is left out: {}", problem.message) }),
            None => kept.push((rel, bytes)),
        }
    }

    /* What a pack shares between its girls (sounds, other textures) may already be in from another pack that used the same names: the first one is kept, and the file is not brought in again. Her own files may not clash. */
    let shared = |rel: &str| rel.contains("/asset_sync/sounds/") || (rel.contains("/asset_sync/textures/") && !rel.ends_with(&format!("/entities/{id}.png")));
    let before = kept.len();

    kept.retain(|(rel, _)| !(shared(rel) && request.existing_files.contains(rel)));

    if kept.len() < before {
        notes.push(Note { level: Level::Kept, message: format!("{} shared sound or texture file(s) were already in the pack (from another resource pack) and were left as they are.", before - kept.len()) });
    }

    let clashes: Vec<&String> = kept.iter().map(|(rel, _)| rel).filter(|rel| request.existing_files.contains(*rel)).collect();

    if !clashes.is_empty() {
        return Err(Error::invalid(format!("These files already exist in the pack, so nothing was imported: {}. Rename or remove them first.", clashes.iter().take(5).map(|s| s.as_str()).collect::<Vec<_>>().join(", "))));
    }

    Ok(Outcome { girl_file, files: kept, merges, notes: collapse(notes) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::{check_project, PackFacts};
    use crate::diagnostics::Severity as Sev;

    fn write(root: &Path, rel: &str, bytes: &[u8]) {
        let path = root.join(rel);

        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    fn tiny_png() -> Vec<u8> {
        let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13, b'I', b'H', b'D', b'R'];

        bytes.extend(64u32.to_be_bytes());
        bytes.extend(64u32.to_be_bytes());
        bytes
    }

    const GEO: &str = r#"{ "format_version": "1.12.0", "minecraft:geometry": [ { "description": { "identifier": "geometry.zoe", "texture_width": 64, "texture_height": 64 },
        "bones": [ { "name": "root", "pivot": [0, 0, 0] }, { "name": "nut", "parent": "root", "pivot": [0, 1, 0], "cubes": [ { "origin": [-1, 0, -1], "size": [2, 2, 2], "uv": [0, 0] } ] } ] } ] }"#;

    const ANIMATIONS: &str = r##"{ "format_version": "1.8.0", "animations": {
        "animation.zoe.idle": { "loop": true, "bones": { "root": { "rotation": [0, 5, 0] } } },
        "animation.zoe.hug_slow": { "loop": true, "animation_length": 1.0, "sound_effects": { "0.5": { "effect": "thrust, hugmoan" } }, "bones": { "root": { "position": { "0": [1, 0, 0], "1": [0, 0, 0] } } } },
        "animation.zoe.hug_fast": { "loop": true, "bones": { "root": { "rotation": [1, 2, 3] } } }
    } }"##;

    #[test]
    fn brings_a_girl_in_from_her_old_pack() {
        let dir = tempfile::tempdir().unwrap();
        let pack = dir.path().join("pack");

        write(&pack, "assets/oldns/geo/nude/zoe.geo.json", GEO.as_bytes());
        write(&pack, "assets/oldns/geo/dressed/zoe.geo.json", GEO.as_bytes());
        write(&pack, "assets/oldns/animations/zoe.animation.json", ANIMATIONS.as_bytes());
        write(&pack, "assets/oldns/keyframe_events/zoe.json", br#"{ "girl_id": "zoe", "events": [ { "key": "hugmoan", "sounds": ["pleasurehorizons:zoe.moan"] } ] }"#);
        write(&pack, "assets/oldns/textures/entities/zoe.png", &tiny_png());
        write(&pack, "assets/oldns/sounds/zoe/moan0.ogg", b"OggS");
        write(&pack, "assets/oldns/sounds.json", br#"{ "zoe.moan": { "sounds": [ "pleasurehorizons:zoe/moan0" ] } }"#);
        write(&pack, "assets/oldns/lang/en_us.lang", b"# comment\nsomething=Something\n");

        let opened = open(&pack).unwrap();

        assert_eq!(opened.girls, vec!["zoe"]);
        assert_eq!(opened.namespace, "oldns");

        let profile = r#"{ "id": "zoe", "name": "Zoe", "tame_item": "minecraft:golden_apple", "scenes": [ { "name": "Hug", "slow_anim": "hug_slow", "fast_anim": "hug_fast", "cum_anim": "missing_clip" } ] }"#;
        let existing = HashSet::new();
        let outcome = import_girl(&opened, &Request { raw_id: "zoe", profile: Some(profile), body_type: "jenny", namespace: "pleasurehorizons", liquid: true, existing_files: &existing, existing_scenes: &[], rules: &Mapping::new(), physics_preset: false, skip_shared: false, taken_tame_items: &[], scene_plan: None }, &Reference::builtin()).unwrap();
        let names: Vec<&str> = outcome.files.iter().map(|(p, _)| p.as_str()).collect();

        for expected in [
            "data/pleasurehorizons/girls/zoe.json",
            "data/pleasurehorizons/scenes/hug.json",
            "data/pleasurehorizons/asset_sync/strut/models/entities/girls/nude/zoe.strutmodel",
            "data/pleasurehorizons/asset_sync/strut/models/entities/girls/dressed/zoe.strutmodel",
            "data/pleasurehorizons/asset_sync/strut/animations/entities/girls/base.zoe.strutanim",
            "data/pleasurehorizons/asset_sync/strut/animations/scenes/hug-zoe.strutanim",
            "data/pleasurehorizons/asset_sync/strut/event_bindings/girls/zoe.json",
            "data/pleasurehorizons/asset_sync/textures/entities/zoe.png",
            "data/pleasurehorizons/asset_sync/sounds/zoe/moan0.ogg",
        ] {
            assert!(names.contains(&expected), "missing {expected} in {names:?}");
        }

        assert!(outcome.notes.iter().any(|n| n.level == Level::Error && n.message.contains("missing_clip")), "a scene that names a clip she does not have is reported");

        /* The base file holds the clip no scene uses; the scene file has its two parts. */
        let read = |rel: &str| crate::strut_anim::read(&outcome.files.iter().find(|(p, _)| p.ends_with(rel)).unwrap().1).unwrap();
        let base = read("base.zoe.strutanim");
        let scene = read("scenes/hug-zoe.strutanim");

        assert_eq!(base.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), vec!["animation.base.idle"]);
        assert_eq!(scene.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), vec!["animation.hug.slow", "animation.hug.fast"]);
        assert_eq!(scene[0].events.iter().map(|e| (e.kind.as_str(), e.payload.as_str())).collect::<Vec<_>>(), vec![("instruction", "thrust"), ("instruction", "hugmoan")]);

        let bindings: Value = serde_json::from_slice(&outcome.files.iter().find(|(p, _)| p.ends_with("event_bindings/girls/zoe.json")).unwrap().1).unwrap();

        assert_eq!(bindings["bindings"]["hugmoan"]["sounds"][0], "pleasurehorizons:zoe.moan");
        assert_eq!(bindings["bindings"]["cumin"]["strut_physics:fluid"]["bone"], "nut");

        /* What was made is a pack that passes its own checks. */
        let project = dir.path().join("project");

        write(&project, "pack.mcmeta", br#"{ "pack": { "pack_format": 48, "description": "x" } }"#);

        for (rel, bytes) in &outcome.files {
            write(&project, rel, bytes);
        }

        for (rel, entries) in &outcome.merges {
            write(&project, rel, serde_json::to_string_pretty(&entries).unwrap().as_bytes());
        }

        let problems: Vec<_> = check_project(&project, &Reference::builtin()).into_iter().filter(|d| d.severity == Sev::Error || d.severity == Sev::Warning).collect();
        let _ = PackFacts::default();

        assert!(problems.is_empty(), "the imported pack has problems: {:?}", problems.iter().map(|d| (&d.file, &d.message)).collect::<Vec<_>>());
    }

    const GEO_SOFT: &str = r#"{ "format_version": "1.12.0", "minecraft:geometry": [ { "description": { "identifier": "geometry.zoe", "texture_width": 64, "texture_height": 64 },
        "bones": [ { "name": "root", "pivot": [0, 0, 0] }, { "name": "boobL", "parent": "root", "pivot": [1, 1, 0], "cubes": [ { "origin": [0, 0, -1], "size": [2, 2, 2], "uv": [0, 0] } ] }, { "name": "belly", "parent": "root", "pivot": [0, 1, 0] } ] } ] }"#;

    fn bone(name: &str, mesh: bool) -> PhysicsBone {
        PhysicsBone { name: name.to_string(), mesh, drawn: true }
    }

    #[test]
    fn basic_physics_is_added_to_a_file_without_touching_what_it_has() {
        let bones: Vec<PhysicsBone> = ["root", "boobL", "boobR", "assL", "assR", "belly", "cheekL"].iter().map(|b| bone(b, *b != "root")).collect();
        let (text, added) = add_basic_physics(None, &bones).unwrap().unwrap();
        let file: Value = serde_json::from_str(&text).unwrap();

        assert_eq!(added, ["boobL", "boobR", "assL", "assR", "cheekL", "belly"]);
        assert_eq!(file["flesh"]["breasts"]["preset"], "pouch");
        assert_eq!(file["flesh"]["breasts"]["bones"], json!(["boobL", "boobR"]), "a left and right pair in one entry, so they press each other");
        assert_eq!(file["flesh"]["butt"]["bones"], json!(["assL", "assR", "cheekL"]));
        assert_eq!(file["flesh"]["belly"]["bones"], json!(["belly"]));
        assert!(file["bones"].as_object().unwrap().is_empty(), "meshes are flesh, not spring bones");
        assert!(file["chains"].is_object());

        /* A bone that draws nothing itself (a container over the real ones) is left out. */
        let held = vec![PhysicsBone { name: "boobs".into(), mesh: false, drawn: false }, bone("boobL", true), bone("boobR", true)];
        let file: Value = serde_json::from_str(&add_basic_physics(None, &held).unwrap().unwrap().0).unwrap();

        assert_eq!(file["flesh"]["breasts"]["bones"], json!(["boobL", "boobR"]));
        assert!(file["bones"].as_object().unwrap().is_empty());

        /* A bone of cubes cannot be flesh: it is a spring bone, and a model with none of mesh gets no flesh section. */
        let cubes: Vec<PhysicsBone> = ["root", "boobL", "boobR", "belly"].iter().map(|b| bone(b, false)).collect();
        let (text, added) = add_basic_physics(None, &cubes).unwrap().unwrap();
        let file: Value = serde_json::from_str(&text).unwrap();

        assert_eq!(added, ["boobL", "boobR", "belly"]);
        assert_eq!(file["bones"]["boobL"]["preset"], "soft-bone");
        assert!(file.get("flesh").is_none());

        /* Some of each: the mesh bones are flesh, the cube bones springs. */
        let mixed = vec![bone("boobL", true), bone("boobR", true), bone("belly", false)];
        let file: Value = serde_json::from_str(&add_basic_physics(None, &mixed).unwrap().unwrap().0).unwrap();

        assert_eq!(file["flesh"]["breasts"]["bones"], json!(["boobL", "boobR"]));
        assert_eq!(file["bones"]["belly"]["preset"], "soft-bone");
        assert!(file["flesh"].get("belly").is_none());

        /* A file that already has boobL as a spring bone and belly as flesh keeps both; only what is missing is added. */
        let mine = r#"{ "version": 1, "bones": { "boobL": { "preset": "soft-bone", "frequency": 9.0 } }, "chains": { "hair": {} }, "soft": { "stomach": { "preset": "belly", "bones": ["belly"] } } }"#;
        let (text, added) = add_basic_physics(Some(mine), &bones).unwrap().unwrap();
        let file: Value = serde_json::from_str(&text).unwrap();

        assert!(!added.contains(&"boobL".to_string()) && !added.contains(&"belly".to_string()));
        assert_eq!(file["bones"]["boobL"]["frequency"], 9.0);
        assert_eq!(file["flesh"]["stomach"]["bones"], json!(["belly"]));
        assert_eq!(file["flesh"]["breasts"]["bones"], json!(["boobR"]));
        assert!(file["chains"]["hair"].is_object());

        /* A single chest bone (a dressed model's boobs) is a region of its own. */
        let dressed: Vec<PhysicsBone> = ["root", "boobs", "cheekL", "cheekR", "belly"].iter().map(|b| bone(b, *b != "root")).collect();
        let (text, _) = add_basic_physics(None, &dressed).unwrap().unwrap();
        let file: Value = serde_json::from_str(&text).unwrap();

        assert_eq!(file["flesh"]["breasts"]["bones"], json!(["boobs"]));

        /* Nothing to add: every bone is there, or none has a name it knows. */
        assert!(add_basic_physics(Some(&text), &dressed).unwrap().is_none());
        assert!(add_basic_physics(None, &[bone("root", false)]).unwrap().is_none());
        assert!(add_basic_physics(Some("not json"), &bones).is_err());
    }

    #[test]
    fn a_tame_item_another_girl_has_is_swapped_for_a_free_one() {
        let girl = br#"{ "id": "lulu", "components": { "pleasurehorizons:taste": { "tame_item": "minecraft:carrot" } } }"#.to_vec();
        let mut free = girl.clone();

        assert!(free_tame_item(&mut free, &["minecraft:poppy".to_string()]).is_none());
        assert_eq!(free, girl, "a free item is left as it is");

        let mut taken = girl.clone();
        let note = free_tame_item(&mut taken, &["minecraft:carrot".to_string(), "minecraft:allium".to_string()]).unwrap();
        let file: Value = serde_json::from_slice(&taken).unwrap();

        assert_eq!(file["components"]["pleasurehorizons:taste"]["tame_item"], "minecraft:poppy");
        assert!(note.message.contains("minecraft:carrot"));
    }

    #[test]
    fn cue_rules_a_basic_physics_preset_and_a_second_girl() {
        let dir = tempfile::tempdir().unwrap();
        let pack = dir.path().join("pack");

        for girl in ["zoe", "amy"] {
            write(&pack, &format!("assets/oldns/geo/nude/{girl}.geo.json"), GEO_SOFT.as_bytes());
            write(&pack, &format!("assets/oldns/geo/dressed/{girl}.geo.json"), GEO_SOFT.as_bytes());
            write(&pack, &format!("assets/oldns/animations/{girl}.animation.json"), ANIMATIONS.replace("zoe", girl).as_bytes());
            write(&pack, &format!("assets/oldns/textures/entities/{girl}.png"), &tiny_png());
        }

        write(&pack, "assets/oldns/sounds/shared/moan0.ogg", b"OggS");
        write(&pack, "assets/oldns/sounds.json", br#"{ "shared.moan": { "sounds": [ "pleasurehorizons:shared/moan0" ] } }"#);

        let opened = open(&pack).unwrap();
        let cues = scan_cues(&opened, "zoe").unwrap();

        assert_eq!(cues.get("thrust"), Some(&1));
        assert_eq!(cues.get("hugmoan"), Some(&1));

        let mut rules = Mapping::new();

        rules.insert("thrust".into(), crate::convert::events::CueRule { action: crate::convert::events::CueAction::Sound, value: Some("pleasurehorizons:shared.moan".into()), authority: 2 });
        rules.insert("hugmoan".into(), crate::convert::events::CueRule { action: crate::convert::events::CueAction::LeaveOut, value: None, authority: 0 });

        let existing = HashSet::new();
        let profile = r#"{ "id": "zoe", "name": "Zoe", "tame_item": "minecraft:golden_apple", "scenes": [ { "name": "Hug", "slow_anim": "hug_slow", "fast_anim": "hug_fast" } ] }"#;
        let first = import_girl(&opened, &Request { raw_id: "zoe", profile: Some(profile), body_type: "jenny", namespace: "pleasurehorizons", liquid: false, existing_files: &existing, existing_scenes: &[], rules: &rules, physics_preset: true, skip_shared: false, taken_tame_items: &[], scene_plan: None }, &Reference::builtin()).unwrap();
        let names: Vec<&str> = first.files.iter().map(|(p, _)| p.as_str()).collect();

        assert!(names.contains(&"data/pleasurehorizons/asset_sync/strut/physics/models/entities/girls/nude/zoe.json"), "{names:?}");

        let physics: Value = serde_json::from_slice(&first.files.iter().find(|(p, _)| p.ends_with("physics/models/entities/girls/nude/zoe.json")).unwrap().1).unwrap();

        assert_eq!(physics["bones"]["boobL"]["preset"], "soft-bone", "an old model is made of cubes, so its chest is a spring bone");
        assert!(physics.get("flesh").is_none());
        assert!(first.files.iter().any(|(p, _)| p.ends_with("sounds/shared/moan0.ogg")));

        let scene = crate::strut_anim::read(&first.files.iter().find(|(p, _)| p.ends_with("scenes/hug-zoe.strutanim")).unwrap().1).unwrap();
        let events: Vec<(&str, &str, u8)> = scene[0].events.iter().map(|e| (e.kind.as_str(), e.payload.as_str(), e.authority)).collect();

        assert_eq!(events, vec![("sound", "pleasurehorizons:shared.moan", 2)], "thrust became a sound with the chosen authority and hugmoan was left out");

        /* The second girl of the same pack brings none of what the pack shares. */
        let second = import_girl(&opened, &Request { raw_id: "amy", profile: None, body_type: "jenny", namespace: "pleasurehorizons", liquid: false, existing_files: &existing, existing_scenes: &[], rules: &Mapping::new(), physics_preset: false, skip_shared: true, taken_tame_items: &[], scene_plan: None }, &Reference::builtin()).unwrap();

        assert!(second.files.iter().any(|(p, _)| p.ends_with("textures/entities/amy.png")));
        assert!(!second.files.iter().any(|(p, _)| p.contains("/sounds/")), "shared sounds are not copied twice");
        assert!(!second.merges.iter().any(|(p, _)| p.ends_with("sounds.json")), "sounds.json is not merged twice");
    }

    #[test]
    fn a_pack_of_several_girls_in_the_newer_layout_gives_each_girl_her_own_files() {
        let dir = tempfile::tempdir().unwrap();
        let pack = dir.path().join("pack");
        let clip = r#"{ "format_version": "1.8.0", "animations": { "animation.x.idle": { "loop": true, "bones": { "root": { "rotation": [0, 5, 0] } } } } }"#;

        for girl in ["zoe", "amy"] {
            write(&pack, &format!("assets/oldns/geo/nude/{girl}.geo.json"), GEO.as_bytes());
            write(&pack, &format!("assets/oldns/geo/dressed/{girl}.geo.json"), GEO.as_bytes());
            write(&pack, &format!("assets/oldns/animations/base.{girl}.animation.json"), clip.as_bytes());
            write(&pack, &format!("assets/oldns/animations/scenes/hug-{girl}.scene.json"), clip.as_bytes());
            write(&pack, &format!("assets/oldns/textures/entities/{girl}.png"), &tiny_png());
        }

        write(&pack, "assets/oldns/animations/base.animation.json", clip.as_bytes());

        let opened = open(&pack).unwrap();
        let existing = HashSet::new();
        let request = |id: &'static str, skip: bool| Request { raw_id: id, profile: None, body_type: "jenny", namespace: "pleasurehorizons", liquid: false, existing_files: &existing, existing_scenes: &[], rules: Box::leak(Box::new(Mapping::new())), physics_preset: false, skip_shared: skip, taken_tame_items: &[], scene_plan: None };
        let zoe = import_girl(&opened, &request("zoe", false), &Reference::builtin()).unwrap();
        let amy = import_girl(&opened, &request("amy", true), &Reference::builtin()).unwrap();
        let animations = |o: &Outcome| o.files.iter().map(|(p, _)| p.rsplit("animations/").next().unwrap().to_string()).filter(|p| p.ends_with(".strutanim")).collect::<Vec<_>>();

        assert!(!zoe.files.iter().any(|(p, _)| p.ends_with("entities/amy.png")), "another girl's texture comes with her, not with the first girl");
        let mut first = animations(&zoe);
        first.sort();

        assert_eq!(first, vec!["base.strutanim", "base.zoe.strutanim", "scenes/hug-zoe.strutanim"]);
        assert_eq!(animations(&amy).into_iter().collect::<std::collections::BTreeSet<_>>().into_iter().collect::<Vec<_>>(), vec!["base.amy.strutanim", "scenes/hug-amy.strutanim"]);
    }

    #[test]
    fn a_pack_made_for_the_old_mod_is_rewritten_and_imported() {
        let dir = tempfile::tempdir().unwrap();
        let pack = dir.path().join("pack");

        write(&pack, "assets/sexmod/geo/cat/cat.geo.json", GEO.as_bytes());
        write(&pack, "assets/sexmod/animations/cat/cat.animation.json", br#"{ "format_version": "1.8.0", "animations": {
            "animation.cat.idle": { "loop": true, "bones": { "root": { "rotation": [0, 5, 0] } } },
            "animation.cat.sit_slow": { "loop": true, "animation_length": 1.0, "sound_effects": { "0.5": { "effect": "Moan" } }, "bones": { "root": { "rotation": [1, 0, 0] } } },
            "animation.cat.sit_fast": { "loop": true, "bones": { "root": { "rotation": [2, 0, 0] } } } } }"#);
        write(&pack, "assets/sexmod/textures/entity/cat/cat.png", &tiny_png());
        write(&pack, "assets/sexmod/textures/entity/cat/hand.png", &tiny_png());
        write(&pack, "assets/sexmod/sounds/girls/Luna/moan/moan0.ogg", b"OggS");
        write(&pack, "assets/sexmod/sounds/girls/Luna/moan/moan1.ogg", b"OggS");
        write(&pack, "assets/sexmod/lang/en_us.lang", b"entity.luna.name=Siren
entity.jenny.name=Jenny
");

        let opened = open(&pack).unwrap();

        assert!(opened.rewritten);
        assert_eq!(opened.girls, vec!["luna"]);

        let existing = HashSet::new();
        let outcome = import_girl(&opened, &Request { raw_id: "luna", profile: None, body_type: "jenny", namespace: "pleasurehorizons", liquid: false, existing_files: &existing, existing_scenes: &[], rules: &Mapping::new(), physics_preset: false, skip_shared: false, taken_tame_items: &[], scene_plan: None }, &Reference::builtin()).unwrap();
        let names: Vec<&str> = outcome.files.iter().map(|(p, _)| p.as_str()).collect();

        for expected in [
            "data/pleasurehorizons/girls/luna.json",
            "data/pleasurehorizons/scenes/sit.json",
            "data/pleasurehorizons/asset_sync/strut/models/entities/girls/nude/luna.strutmodel",
            "data/pleasurehorizons/asset_sync/strut/models/entities/girls/dressed/luna.strutmodel",
            "data/pleasurehorizons/asset_sync/strut/animations/entities/girls/base.luna.strutanim",
            "data/pleasurehorizons/asset_sync/strut/animations/scenes/sit-luna.strutanim",
            "data/pleasurehorizons/asset_sync/strut/event_bindings/girls/luna.json",
            "data/pleasurehorizons/asset_sync/textures/entities/luna.png",
            "data/pleasurehorizons/asset_sync/textures/entities/luna_hand.png",
            "data/pleasurehorizons/asset_sync/sounds/luna/moan/moan0.ogg",
        ] {
            assert!(names.contains(&expected), "missing {expected} in {names:?}");
        }

        let bindings: Value = serde_json::from_slice(&outcome.files.iter().find(|(p, _)| p.ends_with("event_bindings/girls/luna.json")).unwrap().1).unwrap();

        assert_eq!(bindings["bindings"]["moan"]["sounds"][0], "pleasurehorizons:luna.moan");

        let sounds = outcome.merges.iter().find(|(p, _)| p.ends_with("sounds.json")).unwrap();

        assert_eq!(sounds.1["luna.moan"]["sounds"].as_array().unwrap().len(), 2);

        let girl = String::from_utf8(outcome.files.iter().find(|(p, _)| p.ends_with("girls/luna.json")).unwrap().1.clone()).unwrap();

        assert!(girl.contains("Siren"), "her name comes from the language file");
    }

    #[test]
    #[ignore = "reads packs on the author's machine: set FAPCRAFT_PACKS to zips or folders separated by ;"]
    fn real_packs() {
        for path in std::env::var("FAPCRAFT_PACKS").unwrap().split(';') {
            let opened = open(Path::new(path)).unwrap();
            let existing = HashSet::new();

            for girl in &opened.girls {
                let outcome = import_girl(&opened, &Request { raw_id: girl, profile: None, body_type: "jenny", namespace: "pleasurehorizons", liquid: true, existing_files: &existing, existing_scenes: &[], rules: &Mapping::new(), physics_preset: true, skip_shared: false, taken_tame_items: &[], scene_plan: None }, &Reference::builtin()).unwrap();

                println!("== {path} / {girl}: {} files", outcome.files.len());

                for n in &outcome.notes {
                    println!("  {:?}: {}", n.level, n.message);
                }

                let project = std::env::temp_dir().join(format!("ph-real-{girl}"));
                let _ = fs::remove_dir_all(&project);

                write(&project, "pack.mcmeta", br#"{ "pack": { "pack_format": 48, "description": "x" } }"#);

                for (rel, bytes) in &outcome.files {
                    write(&project, rel, bytes);
                }

                for (rel, entries) in &outcome.merges {
                    write(&project, rel, serde_json::to_string_pretty(&entries).unwrap().as_bytes());
                }

                for d in check_project(&project, &Reference::builtin()).iter().filter(|d| d.severity == Sev::Error || d.severity == Sev::Warning).take(25) {
                    println!("  PROBLEM {:?} {}: {}", d.severity, d.file, d.message);
                }
            }
        }
    }

    #[test]
    fn a_zip_is_opened_and_cleaned_up() {
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("old.zip");
        let mut writer = zip::ZipWriter::new(fs::File::create(&zip_path).unwrap());
        let options = zip::write::SimpleFileOptions::default();

        writer.start_file("assets/ns/geo/nude/amy.geo.json", options).unwrap();
        writer.write_all(GEO.as_bytes()).unwrap();
        writer.start_file("../escape.txt", options).unwrap();
        writer.write_all(b"x").unwrap();
        writer.finish().unwrap();

        let temp;

        {
            let opened = open(&zip_path).unwrap();

            temp = opened.temp.clone().unwrap();
            assert_eq!(opened.girls, vec!["amy"]);
            assert!(temp.exists());
        }

        assert!(!temp.exists(), "the unpacked copy is removed");
        assert!(!dir.path().join("escape.txt").exists(), "a name that leaves the folder is not followed");
        assert!(open(dir.path()).is_err());
    }
}
