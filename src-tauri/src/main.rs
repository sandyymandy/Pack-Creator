#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! The Pack Creator's window. The logic is in `pc_core`; this file lends it to the page as commands, and holds which project is open.
//!
//! The page never names a place on disk after the project is opened: it names files relative to the project, and `pc_core` refuses anything that leaves it.

mod blockbench;
mod clipboard;
mod examples;
mod wizard;

use base64::Engine;
use pc_core::check::{self, PackFacts};
use pc_core::diagnostics::Diagnostic;
use pc_core::fs_ops::{self, Content, Entry};
use pc_core::import::{self, Note};
use pc_core::layout::{locate, FileKind, Located};
use pc_core::project::{self, NewProject, ProjectMeta, CONTENTS};
use pc_core::recent::{self, RecentView};
use pc_core::reference::Reference;
use pc_core::templates::{self, GirlSpec};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Manager, State};

struct Opened {
    root: PathBuf,
    meta: ProjectMeta,
}

pub struct App {
    reference: Reference,
    recent_file: PathBuf,
    current: Mutex<Option<Opened>>,
    /// The old resource pack being looked at in the import dialog, kept so a zip is unpacked once and not for every girl.
    old: Mutex<Option<(String, pc_core::oldpack::OldPack)>>,
}

pub type Reply<T> = Result<T, String>;

pub fn text<E: std::fmt::Display>(error: E) -> String {
    error.to_string()
}

impl App {
    fn open(&self) -> Reply<(PathBuf, ProjectMeta)> {
        self.current.lock().unwrap().as_ref().map(|o| (o.root.clone(), o.meta.clone())).ok_or_else(|| "No project is open.".to_string())
    }
}

#[derive(Serialize)]
struct OpenedProject {
    root: String,
    meta: ProjectMeta,
}

#[derive(Serialize)]
struct ContentOption {
    id: &'static str,
    label: &'static str,
}

#[derive(Serialize)]
struct Written {
    located: Located,
    diagnostics: Vec<Diagnostic>,
    /// What the file looks like now, so the page does not mistake its own save for a change made elsewhere.
    stamp: Option<String>,
}

// =================================================================
// Home
// =================================================================

#[tauri::command(async)]
fn list_recent(app: State<App>) -> Vec<RecentView> {
    recent::list(&app.recent_file)
}

#[tauri::command(async)]
fn forget_recent(app: State<App>, path: String) -> Reply<()> {
    recent::forget(&app.recent_file, &path).map_err(text)
}

#[tauri::command(async)]
fn default_projects_dir(handle: tauri::AppHandle) -> String {
    let base = handle.path().document_dir().unwrap_or_else(|_| PathBuf::from("."));

    base.join("Pleasure Horizons Packs").to_string_lossy().to_string()
}

#[tauri::command(async)]
fn contents_options() -> Vec<ContentOption> {
    CONTENTS.iter().map(|(id, label)| ContentOption { id, label }).collect()
}

#[tauri::command(async)]
fn namespace_problem(namespace: String) -> Option<String> {
    project::namespace_problem(&namespace)
}

#[tauri::command(async)]
fn create_project(app: State<App>, request: NewProject) -> Reply<OpenedProject> {
    /* The folder that holds the packs is made on the first project. */
    std::fs::create_dir_all(&request.parent).map_err(text)?;

    let (root, meta) = project::create(&request).map_err(text)?;

    remember(&app, root, meta)
}

#[tauri::command(async)]
fn open_project(app: State<App>, path: String) -> Reply<OpenedProject> {
    let root = PathBuf::from(path);
    let meta = project::open(&root).map_err(text)?;

    remember(&app, root, meta)
}

fn remember(app: &App, root: PathBuf, meta: ProjectMeta) -> Reply<OpenedProject> {
    recent::touch(&app.recent_file, &root, &meta.name).map_err(text)?;

    let opened = OpenedProject { root: root.to_string_lossy().to_string(), meta: meta.clone() };

    *app.current.lock().unwrap() = Some(Opened { root, meta });
    Ok(opened)
}

#[tauri::command(async)]
fn close_project(app: State<App>) {
    *app.current.lock().unwrap() = None;
}

// =================================================================
// Files
// =================================================================

#[tauri::command(async)]
fn list_dir(app: State<App>, rel: String) -> Reply<Vec<Entry>> {
    let (root, _) = app.open()?;

    fs_ops::list_dir(&root, &rel).map_err(text)
}

#[tauri::command(async)]
fn list_files(app: State<App>) -> Reply<Vec<String>> {
    let (root, _) = app.open()?;

    Ok(check::walk(&root))
}

#[tauri::command(async)]
fn locate_file(rel: String) -> Located {
    locate(&rel)
}

#[tauri::command(async)]
fn read_file(app: State<App>, rel: String) -> Reply<Content> {
    if let Some(path) = examples::strip(&rel) {
        return examples::text(path).map(|text| Content::Text { text }).ok_or_else(|| format!("{rel} is not one of the examples."));
    }

    let (root, _) = app.open()?;

    fs_ops::read(&root, &rel).map_err(text)
}

/// A picture as a data URL, for the viewer. Only PNGs, and only up to 16 MiB.
#[tauri::command(async)]
fn read_image(app: State<App>, rel: String) -> Reply<String> {
    if let Some(path) = examples::strip(&rel) {
        let bytes = examples::bytes(path).ok_or_else(|| format!("{rel} is not one of the examples."))?;

        return Ok(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)));
    }

    let (root, _) = app.open()?;
    let bytes = fs_ops::read_bytes(&root, &rel, 16 * 1024 * 1024).map_err(text)?;

    Ok(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

/// Saves a file and checks it, so the page shows the problems of what was just saved.
#[tauri::command(async)]
fn write_file(app: State<App>, rel: String, text_value: String) -> Reply<Written> {
    let (root, _) = app.open()?;
    let previous = match fs_ops::read(&root, &rel) {
        Ok(Content::Text { text }) => Some(text),
        _ => None,
    };

    fs_ops::write_text(&root, &rel, &text_value).map_err(text)?;
    pc_core::history::record_now(&root, &rel, previous.as_deref(), &text_value);

    let facts = check::collect_facts(&root);

    Ok(Written { located: locate(&rel), diagnostics: check::check_text(&rel, &text_value, &app.reference, &facts), stamp: fs_ops::stamp(&root, &rel) })
}

/// The earlier versions kept of a file, newest first.
#[tauri::command(async)]
fn file_history(app: State<App>, rel: String) -> Reply<Vec<pc_core::history::Version>> {
    let (root, _) = app.open()?;

    Ok(pc_core::history::list(&root, &rel))
}

#[tauri::command(async)]
fn read_version(app: State<App>, rel: String, id: u64) -> Reply<String> {
    let (root, _) = app.open()?;

    pc_core::history::read(&root, &rel, id).map_err(text)
}

/// Checks text that is not saved yet (what is being typed).
#[tauri::command(async)]
fn check_text(app: State<App>, rel: String, text_value: String) -> Reply<Vec<Diagnostic>> {
    let (root, _) = app.open()?;
    let facts = check::collect_facts(&root);

    Ok(check::check_text(&rel, &text_value, &app.reference, &facts))
}

/// The stamps of some files (nothing for one that is gone), to notice changes made by other programs.
#[tauri::command(async)]
fn stamps(app: State<App>, rels: Vec<String>) -> Reply<Vec<Option<String>>> {
    let (root, _) = app.open()?;

    Ok(rels.iter().map(|rel| fs_ops::stamp(&root, rel)).collect())
}

#[tauri::command(async)]
fn check_project(app: State<App>) -> Reply<Vec<Diagnostic>> {
    let (root, _) = app.open()?;

    Ok(check::check_project(&root, &app.reference))
}

#[tauri::command(async)]
fn create_entry(app: State<App>, rel: String, is_dir: bool) -> Reply<()> {
    let (root, _) = app.open()?;

    if is_dir { fs_ops::create_dir(&root, &rel) } else { fs_ops::create_file(&root, &rel, "") }.map_err(text)
}

#[tauri::command(async)]
fn rename_entry(app: State<App>, from: String, to: String) -> Reply<()> {
    let (root, _) = app.open()?;

    fs_ops::rename(&root, &from, &to).map_err(text)
}

#[tauri::command(async)]
fn delete_entry(app: State<App>, rel: String) -> Reply<()> {
    let (root, _) = app.open()?;

    fs_ops::delete(&root, &rel).map_err(text)
}

// =================================================================
// New files
// =================================================================

#[derive(Deserialize)]
struct NewFile {
    kind: FileKind,
    id: String,
    /// For a girl.
    name: Option<String>,
    archetype: Option<String>,
    body_type: Option<String>,
    tame_item: Option<String>,
    /// A scene's type, a trait's category or a dialogue's topic.
    option: Option<String>,
}

/// Makes a new girl, scene, trait or dialogue file in the pack's own namespace, and gives back its path.
#[tauri::command(async)]
fn new_file(app: State<App>, request: NewFile) -> Reply<String> {
    let (root, meta) = app.open()?;
    let rel = templates::path_for(&meta, &request.kind, &request.id).map_err(text)?;
    let option = request.option.clone().unwrap_or_default();

    let body = match request.kind {
        FileKind::Girl => {
            let name = request.name.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| request.id.clone());
            let body_type = request.body_type.clone().ok_or("Choose a body type: it decides which scenes fit her.")?;

            templates::girl(&GirlSpec {
                id: &request.id,
                name: &name,
                archetype: request.archetype.as_deref().unwrap_or("settler"),
                body_type: &body_type,
                tame_item: request.tame_item.as_deref().filter(|t| !t.is_empty()).unwrap_or("minecraft:allium"),
                hitbox_height: 1.8,
            })
        }
        FileKind::Scene => templates::scene(if option.is_empty() { "on_player" } else { &option }),
        FileKind::Trait => templates::trait_file(if option.is_empty() { "interaction" } else { &option }),
        FileKind::Dialogue => templates::dialogue(if option.is_empty() { "chat" } else { &option }),
        FileKind::Social => templates::social(if option.is_empty() { "line" } else { &option }),
        _ => return Err("That kind of file cannot be made from a template.".into()),
    };

    fs_ops::create_file(&root, &rel, &body).map_err(text)?;
    Ok(rel)
}

// =================================================================
// Getting a pack out
// =================================================================

/// Saves the pack as a zip. Returns how many files went in.
#[tauri::command(async)]
fn export_zip(app: State<App>, destination: String) -> Reply<usize> {
    let (root, _) = app.open()?;

    pc_core::export::zip_pack(&root, &PathBuf::from(destination)).map_err(text_of)
}

/// Copies the pack into a world's datapacks folder. Returns where it went.
#[tauri::command(async)]
fn install_pack(app: State<App>, world: String) -> Reply<String> {
    let (root, meta) = app.open()?;
    let folder = project::folder_name(&meta.name);

    pc_core::export::install(&root, &PathBuf::from(world), &folder).map(|p| p.to_string_lossy().to_string()).map_err(text_of)
}

/// Where Minecraft keeps its worlds, when that folder is there, to start the world picker in.
#[tauri::command(async)]
fn saves_dir() -> Option<String> {
    let appdata = std::env::var("APPDATA").ok()?;
    let saves = PathBuf::from(appdata).join(".minecraft").join("saves");

    saves.is_dir().then(|| saves.to_string_lossy().to_string())
}

// =================================================================
// Finding, copying, showing
// =================================================================

#[derive(Serialize)]
struct SearchHit {
    rel: String,
    line: usize,
    column: usize,
    /// The line the match is on, trimmed.
    preview: String,
    /// Where the match is in that preview (characters), to show it marked.
    start: usize,
    length: usize,
}

/// Looks for text in every text file of the pack (up to 500 matches).
#[tauri::command(async)]
fn search_text(app: State<App>, query: String, case_sensitive: bool) -> Reply<Vec<SearchHit>> {
    let (root, _) = app.open()?;
    let mut hits = Vec::new();

    if query.is_empty() {
        return Ok(hits);
    }

    let needle: Vec<char> = if case_sensitive { query.chars().collect() } else { query.to_lowercase().chars().collect() };

    'files: for rel in check::walk(&root) {
        let Ok(path) = fs_ops::resolve(&root, &rel) else { continue };

        if std::fs::metadata(&path).map_or(true, |m| m.len() > 4 * 1024 * 1024) {
            continue;
        }

        let Ok(text) = std::fs::read_to_string(&path) else { continue };

        for (index, line) in text.lines().enumerate() {
            let chars: Vec<char> = if case_sensitive { line.chars().collect() } else { line.to_lowercase().chars().collect() };
            let original: Vec<char> = line.chars().collect();

            /* Lower casing can change a line's length (rare letters); then the match is skipped rather than marked in the wrong place. */
            if chars.len() != original.len() || chars.len() < needle.len() {
                continue;
            }

            let mut at = 0;

            while at + needle.len() <= chars.len() {
                if chars[at..at + needle.len()] == needle[..] {
                    /* The preview starts a little before the match, so a long line still shows it. */
                    let from = at.saturating_sub(40);
                    let preview: String = original[from..(at + needle.len() + 80).min(original.len())].iter().collect();

                    hits.push(SearchHit { rel: rel.clone(), line: index + 1, column: at + 1, preview, start: at - from, length: needle.len() });

                    if hits.len() >= 500 {
                        break 'files;
                    }

                    at += needle.len();
                } else {
                    at += 1;
                }
            }
        }
    }

    Ok(hits)
}

/// Replaces text in the named files; gives the number of places changed.
#[tauri::command(async)]
fn replace_in_files(app: State<App>, query: String, replacement: String, case_sensitive: bool, files: Vec<String>) -> Reply<usize> {
    let (root, _) = app.open()?;
    let mut count = 0;

    if query.is_empty() {
        return Ok(0);
    }

    for rel in files {
        let Ok(Content::Text { text }) = fs_ops::read(&root, &rel) else { continue };
        let (changed, n) = if case_sensitive {
            (text.replace(&query, &replacement), text.matches(&query).count())
        } else {
            /* Case-blind: find the matches in a lower-cased copy and cut the same characters from the original. */
            let lower: Vec<char> = text.to_lowercase().chars().collect();
            let original: Vec<char> = text.chars().collect();
            let needle: Vec<char> = query.to_lowercase().chars().collect();

            if lower.len() != original.len() {
                continue;
            }

            let mut out = String::new();
            let mut at = 0;
            let mut n = 0;

            while at < original.len() {
                if at + needle.len() <= lower.len() && lower[at..at + needle.len()] == needle[..] {
                    out.push_str(&replacement);
                    at += needle.len();
                    n += 1;
                } else {
                    out.push(original[at]);
                    at += 1;
                }
            }

            (out, n)
        };

        if n > 0 {
            fs_ops::write_text(&root, &rel, &changed).map_err(text_of)?;
            pc_core::history::record_now(&root, &rel, Some(&text), &changed);
            count += n;
        }
    }

    Ok(count)
}

fn copy_recursively(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    if from.is_dir() {
        std::fs::create_dir_all(to)?;

        for entry in std::fs::read_dir(from)? {
            let entry = entry?;

            copy_recursively(&entry.path(), &to.join(entry.file_name()))?;
        }

        Ok(())
    } else {
        std::fs::copy(from, to).map(|_| ())
    }
}

/// Copies a file or folder beside itself (`name_copy.json`, `name_copy_2.json`), and gives the new path.
#[tauri::command(async)]
fn duplicate_entry(app: State<App>, rel: String) -> Reply<String> {
    let (root, _) = app.open()?;
    let source = fs_ops::resolve(&root, &rel).map_err(text_of)?;
    let (parent, name) = rel.rsplit_once('/').map(|(p, n)| (format!("{p}/"), n.to_string())).unwrap_or((String::new(), rel.clone()));
    let (stem, ext) = if source.is_dir() { (name.clone(), String::new()) } else { name.rsplit_once('.').map(|(s, e)| (s.to_string(), format!(".{e}"))).unwrap_or((name.clone(), String::new())) };
    let mut candidate = format!("{parent}{stem}_copy{ext}");
    let mut n = 2;

    while fs_ops::resolve(&root, &candidate).map_err(text_of)?.exists() {
        candidate = format!("{parent}{stem}_copy_{n}{ext}");
        n += 1;
    }

    let target = fs_ops::resolve(&root, &candidate).map_err(text_of)?;

    copy_recursively(&source, &target).map_err(|e| format!("{rel} could not be copied: {e}"))?;
    Ok(candidate)
}

/// A file that came from outside: where it goes inside the folder it is put in, and its bytes as base64.
#[derive(Deserialize)]
struct Incoming {
    rel: String,
    data: String,
}

/// The first free name for something put into `folder`: its own if that is free, else `name (2)`, `name (3)` ... (before the extension, for a file).
fn free_name(root: &std::path::Path, folder: &str, name: &str, is_dir: bool) -> Result<String, String> {
    let (stem, ext) = if is_dir { (name.to_string(), String::new()) } else { name.rsplit_once('.').map(|(s, e)| (s.to_string(), format!(".{e}"))).unwrap_or((name.to_string(), String::new())) };
    let prefix = if folder.is_empty() { String::new() } else { format!("{folder}/") };
    let mut candidate = name.to_string();
    let mut n = 2;

    while fs_ops::resolve(root, &format!("{prefix}{candidate}")).map_err(text_of)?.exists() {
        candidate = format!("{stem} ({n}){ext}");
        n += 1;
    }

    Ok(candidate)
}

/// Puts files from outside the Pack Creator (dragged in from Explorer, or pasted) into a folder of the pack. A dropped folder keeps the folders inside it. Nothing that is already
/// there is replaced: a name that is taken gets ` (2)` after it, as Explorer does.
///
/// @return the names (relative to the pack) of what was made at the top: one for each dropped file or folder
#[tauri::command(async)]
fn import_files(app: State<App>, folder: String, files: Vec<Incoming>) -> Reply<Vec<String>> {
    let (root, _) = app.open()?;
    let prefix = if folder.is_empty() { String::new() } else { format!("{folder}/") };
    let mut tops: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    let mut made = Vec::new();

    for file in files {
        let parts: Vec<&str> = file.rel.split('/').filter(|part| !part.is_empty()).collect();

        let Some(first) = parts.first() else { continue };

        for part in &parts {
            if let Some(problem) = fs_ops::name_problem(part) {
                return Err(format!("{} cannot be put in the pack: {problem}", file.rel));
            }
        }

        /* The top name is decided once for everything under it, so a dropped folder lands in one place and does not split over names. */
        let top = match tops.get(*first) {
            Some(top) => top.clone(),
            None => {
                let chosen = free_name(&root, &folder, first, parts.len() > 1)?;

                made.push(format!("{prefix}{chosen}"));
                tops.insert(first.to_string(), chosen.clone());
                chosen
            }
        };

        let inside = if parts.len() > 1 { format!("/{}", parts[1..].join("/")) } else { String::new() };
        let rel = format!("{prefix}{top}{inside}");
        let bytes = base64::engine::general_purpose::STANDARD.decode(file.data.as_bytes()).map_err(|e| format!("{} could not be read: {e}", file.rel))?;
        let target = fs_ops::resolve(&root, &rel).map_err(text_of)?;

        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{rel} could not be made: {e}"))?;
        }

        fs_ops::write_bytes(&root, &rel, &bytes).map_err(text)?;
    }

    Ok(made)
}

/// Copies a file or folder of the pack into another folder of it (a copy and paste in the tree). The name is its own if it is free there, else ` (2)` after it.
#[tauri::command(async)]
fn copy_into(app: State<App>, rel: String, folder: String) -> Reply<String> {
    let (root, _) = app.open()?;
    let source = fs_ops::resolve(&root, &rel).map_err(text_of)?;
    let name = rel.rsplit('/').next().unwrap_or(&rel).to_string();

    if source.is_dir() && (folder == rel || folder.starts_with(&format!("{rel}/"))) {
        return Err(format!("{rel} cannot be copied into itself."));
    }

    let chosen = free_name(&root, &folder, &name, source.is_dir())?;
    let target_rel = if folder.is_empty() { chosen } else { format!("{folder}/{chosen}") };
    let target = fs_ops::resolve(&root, &target_rel).map_err(text_of)?;

    copy_recursively(&source, &target).map_err(|e| format!("{rel} could not be copied: {e}"))?;
    Ok(target_rel)
}

/// Puts files and folders of the pack on the Windows clipboard, so they can be pasted into Explorer or another program: a copy, or a cut (the pasting program moves them).
#[tauri::command(async)]
fn copy_to_clipboard(app: State<App>, rels: Vec<String>, cut: bool) -> Reply<()> {
    let (root, _) = app.open()?;
    let mut paths = Vec::new();

    for rel in &rels {
        paths.push(fs_ops::resolve(&root, rel).map_err(text_of)?);
    }

    clipboard::put_files(&paths, cut).map_err(|e| format!("Nothing was copied: {e}."))
}

/// Shows a file or folder of the pack in Windows Explorer.
#[tauri::command(async)]
fn reveal_in_explorer(app: State<App>, rel: String) -> Reply<()> {
    use std::os::windows::process::CommandExt;

    let (root, _) = app.open()?;
    let path = if rel.is_empty() { root.clone() } else { fs_ops::resolve(&root, &rel).map_err(text_of)? };
    let argument = if path.is_dir() { path.display().to_string() } else { format!("/select,{}", path.display()) };

    std::process::Command::new("explorer").raw_arg(argument.replace('/', "\\")).spawn().map_err(|e| format!("Explorer could not be opened: {e}"))?;
    Ok(())
}

// =================================================================
// The scene simulator
// =================================================================

#[derive(Serialize)]
struct BundledModel {
    model: pc_core::strut_model::Model,
    texture: String,
}

/// The game's own bed (`girl_bed`), shipped with the Pack Creator so the simulator lies her on the same bed as the game.
#[tauri::command(async)]
fn bundled_bed() -> Reply<BundledModel> {
    let model = pc_core::strut_model::read(include_bytes!("../resources/girl_bed.strutmodel")).map_err(text)?;

    Ok(BundledModel { model, texture: format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(include_bytes!("../resources/girl_bed.png"))) })
}

/// The base mod's male model, which every scene of the new layout plays together with the girl.
#[tauri::command(async)]
fn bundled_male() -> pc_core::strut_model::Model {
    pc_core::scene_split::male_model()
}

#[derive(Serialize)]
struct SimGirl {
    id: String,
    file: String,
    body_type: Option<String>,
    /// One of the game's own base girls, shipped with the Pack Creator as an example.
    example: bool,
    /// Whether she has a model to show (a girl without one cannot be simulated).
    has_model: bool,
}

fn body_type_of(text: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(text).ok().and_then(|v| v["components"]["pleasurehorizons:body"]["body_type"].as_str().map(String::from))
}

/// The game's base girls, to try scenes on.
#[tauri::command(async)]
fn example_girls() -> Vec<SimGirl> {
    let mut girls: Vec<SimGirl> = examples::names()
        .into_iter()
        .filter_map(|name| {
            let id = name.strip_prefix("data/pleasurehorizons/girls/")?.strip_suffix(".json")?.to_string();

            Some(SimGirl { body_type: examples::text(&name).and_then(|t| body_type_of(&t)), file: format!("{}{name}", examples::PREFIX), id, example: true, has_model: true })
        })
        .collect();

    girls.sort_by(|a, b| a.id.cmp(&b.id));
    girls
}

#[derive(Serialize)]
struct ExampleScene {
    id: String,
    file: String,
}

/// The game's own scenes, to play on the girls they fit.
#[tauri::command(async)]
fn example_scenes() -> Vec<ExampleScene> {
    let mut scenes: Vec<ExampleScene> = examples::names()
        .into_iter()
        .filter_map(|name| {
            let id = name.strip_prefix("data/pleasurehorizons/scenes/")?.strip_suffix(".json")?.to_string();

            Some(ExampleScene { id, file: format!("{}{name}", examples::PREFIX) })
        })
        .collect();

    scenes.sort_by(|a, b| a.id.cmp(&b.id));
    scenes
}

/// The girls of this pack, for the scene simulator to choose from.
#[tauri::command(async)]
fn sim_girls(app: State<App>) -> Reply<Vec<SimGirl>> {
    let (root, _) = app.open()?;
    let mut girls = Vec::new();
    let all_files = check::walk(&root);

    for rel in all_files.iter() {
        let rel = rel.clone();
        let located = locate(&rel);

        if located.kind != FileKind::Girl {
            continue;
        }

        let body_type = match fs_ops::read(&root, &rel) {
            Ok(Content::Text { text }) => body_type_of(&text),
            _ => None,
        };

        let id = located.id.unwrap_or_default();
        let has_model = ["nude", "dressed"].iter().any(|kind| all_files.iter().any(|f| f.ends_with(&format!("/{}", pc_core::paths::girl_model(kind, &id))) || f.ends_with(&format!("/{}", pc_core::paths::girl_model_old(kind, &id)))));

        girls.push(SimGirl { id, file: rel, body_type, example: false, has_model });
    }

    Ok(girls)
}

#[derive(Serialize)]
struct SimFiles {
    nude: Option<String>,
    dressed: Option<String>,
    texture: Option<String>,
    /// The animation files she plays a scene from, in the game's lookup order (hers for the scene, the shared one, her base file, the shared base).
    animations: Vec<String>,
    /// Her event bindings in layers, the skeleton's first and hers after (hers win): empty for a girl whose bindings are one file.
    binding_layers: Vec<String>,
    /// For a scene in the new layout (one with a layout file): the male's clip files, in the order they are looked up. Empty for a scene that is not split.
    male_animations: Vec<String>,
    bindings: Option<String>,
    sounds_json: Vec<String>,
}

/// The path of the skeleton a girl's model carries (`jenny` for `pleasurehorizons:jenny`): from the model's own id, or her body type when it has none.
fn skeleton_path(root: &std::path::Path, from_zip: bool, model: Option<String>, body_type: Option<&str>) -> Option<String> {
    let from_model = (!from_zip).then_some(()).and(model).and_then(|rel| std::fs::read(root.join(rel)).ok()).and_then(|b| pc_core::strut_model::read(&b).ok()).and_then(|m| m.skeleton_id);

    from_model.map(|id| id.split_once(':').map_or(id.clone(), |(_, p)| p.to_string())).or_else(|| body_type.filter(|b| !b.is_empty()).map(str::to_string))
}

/// The files of one girl that a simulation of one scene needs.
#[tauri::command(async)]
fn sim_files(app: State<App>, scene_id: String, girl_id: String, example: Option<bool>, body_type: Option<String>) -> Reply<SimFiles> {
    /* An example girl's files are in the zip, under the names the game keeps them by. */
    let from_zip = example == Some(true);
    let (root, files): (PathBuf, Vec<String>) = if from_zip {
        (PathBuf::new(), examples::names())
    } else {
        let (root, _) = app.open()?;
        let files = check::walk(&root);

        (root, files)
    };

    let _ = root;
    let tag = |f: &String| if from_zip { format!("{}{f}", examples::PREFIX) } else { f.clone() };
    let find = |suffix: String| files.iter().find(|f| (from_zip || f.contains("/asset_sync/")) && f.ends_with(&suffix)).map(tag);
    let at = |suffix: String| files.iter().find(|f| f.ends_with(&suffix)).map(tag);
    /* A file where format 3 keeps it, or where it was kept before (a pack not yet updated, or the examples). */
    let find_either = |new: String, old: String| find(format!("/{new}")).or_else(|| find(format!("/{old}")));
    let mut animations = Vec::new();

    /* The files the game looks a clip up in, in its order: her own file of the scene, the file for her skeleton (her body type: `<scene>.<skeleton>.strutanim`), the shared file of the
       scene, then her base file and the shared base. Her own beats her skeleton's, which beats the shared one. */
    /* A scene in the new layout has a layout file and a folder for each role: the girl's clips come from her own file, then her skeleton's; the male's from the file made for
       her (when the male moves differently with her), then the male's. */
    let mut male_animations = Vec::new();
    let mut order = Vec::new();

    if find(format!("/strut/scenes/{scene_id}.strutscene")).is_some() {
        let skeleton = skeleton_path(&root, from_zip, find_either(pc_core::paths::girl_model("nude", &girl_id), pc_core::paths::girl_model_old("nude", &girl_id)).or_else(|| find_either(pc_core::paths::girl_model("dressed", &girl_id), pc_core::paths::girl_model_old("dressed", &girl_id))), body_type.as_deref());

        order.push(format!("/strut/animations/scenes/{scene_id}/host/owners/{girl_id}.strutanim"));

        if let Some(skeleton) = skeleton {
            order.push(format!("/strut/animations/scenes/{scene_id}/host/skeletons/{skeleton}.strutanim"));
        }

        for suffix in [format!("/strut/animations/scenes/{scene_id}/male/owners/{girl_id}.strutanim"), format!("/strut/animations/scenes/{scene_id}/male/skeletons/male.strutanim")] {
            if let Some(found) = find(suffix) {
                male_animations.push(found);
            }
        }
    }

    order.push(format!("/strut/animations/scenes/{scene_id}-{girl_id}.strutanim"));

    if let Some(body) = body_type.as_deref().filter(|body| !body.is_empty()) {
        order.push(format!("/strut/animations/scenes/{scene_id}.{body}.strutanim"));
    }

    order.push(format!("/strut/animations/scenes/{scene_id}.strutanim"));
    /* Her everyday clips: her own file, her skeleton's (the clip set `base`), then the older single files. */
    order.push(format!("/strut/animations/base/owners/{girl_id}.strutanim"));

    if let Some(skeleton) = skeleton_path(&root, from_zip, find_either(pc_core::paths::girl_model("nude", &girl_id), pc_core::paths::girl_model_old("nude", &girl_id)).or_else(|| find_either(pc_core::paths::girl_model("dressed", &girl_id), pc_core::paths::girl_model_old("dressed", &girl_id))), body_type.as_deref()) {
        order.push(format!("/strut/animations/base/skeletons/{skeleton}.strutanim"));
    }

    order.push(format!("/{}", pc_core::paths::girl_base(&girl_id)));
    order.push(format!("/{}", pc_core::paths::girl_base_old(&girl_id)));
    order.push(format!("/{}", pc_core::paths::SHARED_BASE));
    order.push(format!("/{}", pc_core::paths::SHARED_BASE_OLD));

    for suffix in order {
        if let Some(found) = find(suffix) {
            animations.push(found);
        }
    }

    /* The game's shared animations (the scene's own file and the base file) are in the game's jar, not in a pack: a girl without clips of her own plays them, so they are
       taken from the examples when the pack has none. */
    if !from_zip {
        for suffix in [format!("/strut/animations/scenes/{scene_id}.strutanim"), format!("/{}", pc_core::paths::SHARED_BASE), format!("/{}", pc_core::paths::SHARED_BASE_OLD)] {
            if !animations.iter().any(|a| a.ends_with(&suffix)) {
                if let Some(shared) = examples::names().into_iter().find(|n| n.ends_with(&suffix)) {
                    animations.push(format!("{}{shared}", examples::PREFIX));
                }
            }
        }
    }

    /* A pack's cues may play the game's own sounds (`pleasurehorizons:misc.pounding`): their definitions come first, so the pack's override them. */
    let mut sounds_json: Vec<String> = if from_zip { Vec::new() } else { examples::names().into_iter().filter(|n| n.ends_with("/sounds.json")).map(|n| format!("{}{n}", examples::PREFIX)).collect() };

    sounds_json.extend(if from_zip { at("/sounds.json".into()).into_iter().collect::<Vec<_>>() } else { files.iter().filter(|f| f.contains("/asset_sync/") && f.ends_with("/sounds.json")).cloned().collect::<Vec<_>>() });

    Ok(SimFiles {
        nude: find_either(pc_core::paths::girl_model("nude", &girl_id), pc_core::paths::girl_model_old("nude", &girl_id)),
        dressed: find_either(pc_core::paths::girl_model("dressed", &girl_id), pc_core::paths::girl_model_old("dressed", &girl_id)),
        texture: find(format!("/textures/entities/{girl_id}.png")),
        animations,
        male_animations,
        binding_layers: {
            let mut layers = Vec::new();

            if let Some(skeleton) = skeleton_path(&root, from_zip, find_either(pc_core::paths::girl_model("nude", &girl_id), pc_core::paths::girl_model_old("nude", &girl_id)).or_else(|| find_either(pc_core::paths::girl_model("dressed", &girl_id), pc_core::paths::girl_model_old("dressed", &girl_id))), body_type.as_deref()) {
                layers.extend(find(format!("/strut/event_bindings/skeletons/{skeleton}.json")));
            }

            layers.extend(find(format!("/strut/event_bindings/owners/{girl_id}.json")));
            layers
        },
        bindings: find(format!("/strut/event_bindings/girls/{girl_id}.json")),
        sounds_json,
    })
}

/// A file of the pack as a data URL (a sound to play).
#[tauri::command(async)]
fn read_asset(app: State<App>, rel: String) -> Reply<String> {
    if let Some(path) = examples::strip(&rel) {
        let bytes = examples::bytes(path).ok_or_else(|| format!("{rel} is not one of the examples."))?;

        return Ok(format!("data:audio/ogg;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)));
    }

    let (root, _) = app.open()?;
    let bytes = fs_ops::read_bytes(&root, &rel, 32 * 1024 * 1024).map_err(text)?;
    let mime = if rel.ends_with(".ogg") { "audio/ogg" } else if rel.ends_with(".png") { "image/png" } else { "application/octet-stream" };

    Ok(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

// =================================================================
// The game's log
// =================================================================

#[derive(Serialize)]
struct GameLogEntry {
    #[serde(flatten)]
    entry: pc_core::gamelog::Entry,
    /// The file of this pack the message is about, when there is one.
    file: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GameLogView {
    path: String,
    /// How long ago the log was last written.
    age_seconds: Option<u64>,
    entries: Vec<GameLogEntry>,
    offer: Option<pc_core::gamelog::AssetOffer>,
    /// The files this pack has under asset sync, to compare with what the game offered.
    pack_files: usize,
}

/// The usual place of the game's log, when it is there.
#[tauri::command(async)]
fn game_log_default() -> Option<String> {
    let appdata = std::env::var("APPDATA").ok()?;
    let log = PathBuf::from(appdata).join(".minecraft").join("logs").join("latest.log");

    log.is_file().then(|| log.to_string_lossy().to_string())
}

/// Reads the end of the game's log and picks out what it says about Pleasure Horizons and Strut, tying each message to the pack file it names.
#[tauri::command(async)]
fn read_game_log(app: State<App>, path: String) -> Reply<GameLogView> {
    use std::io::{Read, Seek, SeekFrom};

    let (root, _) = app.open()?;
    let mut file = std::fs::File::open(&path).map_err(|e| format!("{path} could not be opened: {e}"))?;
    let metadata = file.metadata().map_err(text)?;
    let limit: u64 = 8 * 1024 * 1024;

    if metadata.len() > limit {
        file.seek(SeekFrom::Start(metadata.len() - limit)).map_err(text)?;
    }

    let mut bytes = Vec::new();

    file.read_to_end(&mut bytes).map_err(text)?;

    let report = pc_core::gamelog::parse(&String::from_utf8_lossy(&bytes));
    let entries = report
        .entries
        .into_iter()
        .map(|entry| {
            let file = entry.resource.as_deref().and_then(|id| id.split_once(':')).map(|(ns, rest)| format!("data/{ns}/{rest}")).filter(|rel| fs_ops::resolve(&root, rel).map_or(false, |p| p.is_file()));

            GameLogEntry { entry, file }
        })
        .collect();
    let pack_files = check::walk(&root).iter().filter(|f| f.contains("/asset_sync/")).count();
    let age_seconds = metadata.modified().ok().and_then(|m| m.elapsed().ok()).map(|d| d.as_secs());

    Ok(GameLogView { path, age_seconds, entries, offer: report.offer, pack_files })
}

// =================================================================
// The viewer
// =================================================================

/// What the viewer needs around one model or animation file: the girl it is for, her models, her texture and her animation files in this pack.
#[derive(Serialize)]
struct ViewerFiles {
    id: String,
    models: Vec<String>,
    texture: Option<String>,
    animations: Vec<String>,
}

/// Works out the girl a model or animation file is for from its name (`strut/models/nude/<id>.strutmodel`, `strut/animations/base.<id>.strutanim`,
/// `strut/animations/scenes/<scene>-<id>.strutanim`) and finds her other files in the pack.
#[tauri::command(async)]
fn viewer_files(app: State<App>, rel: String) -> Reply<ViewerFiles> {
    let (root, _) = app.open()?;
    let name = rel.rsplit('/').next().unwrap_or("");
    let stem = name.rsplit_once('.').map(|(s, _)| s).unwrap_or(name);
    let id = if rel.contains("/strut/models/") {
        stem.to_string()
    } else if let Some((_, girl)) = stem.rsplit_once('-') {
        girl.to_string()
    } else if let Some(girl) = stem.strip_prefix("base.") {
        girl.to_string()
    } else {
        stem.to_string()
    };

    let files = check::walk(&root);
    let in_sync = |f: &String, part: &str| f.contains("/asset_sync/") && f.contains(part);
    let mut models: Vec<String> = Vec::new();

    for kind in ["nude", "dressed"] {
        let (wanted, old) = (format!("/{}", pc_core::paths::girl_model(kind, &id)), format!("/{}", pc_core::paths::girl_model_old(kind, &id)));

        models.extend(files.iter().filter(|f| f.ends_with(&wanted) || f.ends_with(&old)).cloned());
    }

    let texture = files.iter().find(|f| f.ends_with(&format!("/textures/entities/{id}.png"))).cloned();
    let animations: Vec<String> = files
        .iter()
        .filter(|f| in_sync(f, "/strut/animations/") && f.ends_with(".strutanim"))
        .filter(|f| {
            let n = f.rsplit('/').next().unwrap_or("");

            n == format!("{id}.strutanim") && f.contains("/animations/base/owners/") || n == format!("base.{id}.strutanim") || n.ends_with(&format!("-{id}.strutanim")) || n == "base.strutanim" || f.contains(&format!("/scenes/")) && f.contains(&format!("/owners/{id}.strutanim"))
        })
        .cloned()
        .collect();

    Ok(ViewerFiles { id: id.clone(), models, texture, animations })
}

/// Fixes the lower arms of a model that are built with a rotated bone (see `pc_core::arm_fix`). With `apply` false it only says what it would do; with it true the model is
/// changed, after a copy of the original is kept in the project's own `.packcreator/backups` folder (never exported).
#[tauri::command(async)]
fn fix_arms(app: State<App>, rel: String, apply: bool) -> Reply<pc_core::arm_fix::Report> {
    let (root, _) = app.open()?;
    let bytes = fs_ops::read_bytes(&root, &rel, 64 * 1024 * 1024).map_err(text)?;
    let (edited, report) = pc_core::arm_fix::fix(&bytes).map_err(|e| format!("{rel} was not changed: {e}"))?;

    if !apply || (report.fixed.is_empty() && report.removed.is_empty()) {
        return Ok(report);
    }

    /* What is written must read back, and nothing but the arms may have moved: a model the game cannot read is never left behind. */
    pc_core::strut_model::read(&edited).map_err(|e| format!("{rel} was not changed: the fixed model did not read back ({e})"))?;

    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let folder = root.join(project::META_DIR).join("backups").join(rel.replace(['/', '\\', ':'], "~"));

    std::fs::create_dir_all(&folder).map_err(text)?;
    std::fs::write(folder.join(format!("{stamp}.strutmodel")), &bytes).map_err(text)?;
    fs_ops::write_bytes(&root, &rel, &edited).map_err(text)?;

    Ok(report)
}

/// The skeleton id every model and animation file of the pack should have, and the grouping of them. `choices` maps a girl or skeleton to an id a person picked instead.
#[tauri::command(async)]
fn skeleton_plan(app: State<App>, choices: std::collections::BTreeMap<String, String>) -> Reply<pc_core::skeleton_ids::Plan> {
    let (root, _) = app.open()?;

    Ok(pc_core::skeleton_ids::plan(&root, &choices))
}

/// Writes the skeleton ids of the plan (made with the same `choices`) into the models and animation files, keeping a copy of each original in `.packcreator/backups`.
#[tauri::command(async)]
fn skeleton_apply(app: State<App>, choices: std::collections::BTreeMap<String, String>) -> Reply<pc_core::skeleton_ids::Written> {
    let (root, _) = app.open()?;
    let plan = pc_core::skeleton_ids::plan(&root, &choices);

    pc_core::skeleton_ids::apply(&root, &plan.assignments).map_err(text)
}

/// Converts the pack to one model per participant (see `pc_core::pack_split`). With `apply` false it only says what it would do.
#[tauri::command(async)]
fn split_scenes(app: State<App>, apply: bool) -> Reply<pc_core::pack_split::Report> {
    let (root, _) = app.open()?;

    pc_core::migrate::update(&root, apply).map_err(text)
}

/// Whether the pack is in an older format than this tool writes, and what updating it would do.
#[tauri::command(async)]
fn migration_status(app: State<App>) -> Reply<pc_core::migrate::Status> {
    let (root, _) = app.open()?;
    let meta = project::open(&root).map_err(text)?;

    Ok(pc_core::migrate::status(&root, &meta))
}

/// Puts back what a conversion (named by its stamp) changed.
#[tauri::command(async)]
fn undo_split_scenes(app: State<App>, stamp: String) -> Reply<usize> {
    let (root, _) = app.open()?;

    pc_core::pack_split::undo_split(&root, &stamp).map_err(text)
}

/// Removes clips from an animation file. A copy of the file as it was is kept in the project's `.packcreator/backups` folder, and the new file must read back before it replaces the old.
#[tauri::command(async)]
fn delete_clips(app: State<App>, rel: String, clips: Vec<String>) -> Reply<Vec<pc_core::strut_anim::Clip>> {
    let (root, _) = app.open()?;
    let bytes = fs_ops::read_bytes(&root, &rel, 64 * 1024 * 1024).map_err(text)?;
    let edited = pc_core::strut_anim::remove_clips(&bytes, &clips).map_err(|e| format!("{rel} was not changed: {e}"))?;
    let left = pc_core::strut_anim::read(&edited).map_err(|e| format!("{rel} was not changed: the file did not read back ({e})"))?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let folder = root.join(project::META_DIR).join("backups").join(rel.replace(['/', '\\', ':'], "~"));

    std::fs::create_dir_all(&folder).map_err(text)?;
    std::fs::write(folder.join(format!("{stamp}.strutanim")), &bytes).map_err(text)?;
    fs_ops::write_bytes(&root, &rel, &edited).map_err(text)?;

    Ok(left)
}

/// Every scene with who it is for and its animation files.
#[tauri::command(async)]
fn scene_scopes(app: State<App>) -> Reply<Vec<pc_core::scope::SceneScope>> {
    let (root, _) = app.open()?;

    Ok(pc_core::scope::scopes(&root))
}

#[tauri::command(async)]
fn scene_make_skeleton_wide(app: State<App>, scene: String, girl: String) -> Reply<Vec<String>> {
    let (root, _) = app.open()?;

    pc_core::scope::make_skeleton_wide(&root, &scene, &girl).map_err(text)
}

#[tauri::command(async)]
fn scene_make_girl_specific(app: State<App>, scene: String, girl: String, keep_original: bool) -> Reply<Vec<String>> {
    let (root, _) = app.open()?;

    pc_core::scope::make_girl_specific(&root, &scene, &girl, keep_original).map_err(text)
}

#[tauri::command(async)]
fn read_model(app: State<App>, rel: String) -> Reply<pc_core::strut_model::Model> {
    let bytes = match examples::strip(&rel) {
        Some(path) => examples::bytes(path).ok_or_else(|| format!("{rel} is not one of the examples."))?,
        None => {
            let (root, _) = app.open()?;

            fs_ops::read_bytes(&root, &rel, 64 * 1024 * 1024).map_err(text)?
        }
    };

    pc_core::strut_model::read(&bytes).map_err(|e| format!("{rel} cannot be read: {e}"))
}

/// What adding the basic soft bones to a model's physics file did.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PhysicsAdded {
    /// The physics file written.
    path: String,
    /// The bones put in it.
    added: Vec<String>,
}

/// Puts the basic physics (chest, belly, cheeks, ass, found by name: flesh for a bone of meshes, a spring bone for a bone of cubes) in the physics file of a model, making the file if there is none. Bones the file already has are left alone.
#[tauri::command(async)]
fn add_basic_physics(app: State<App>, rel: String) -> Reply<PhysicsAdded> {
    let (root, _) = app.open()?;
    let path = if rel.contains("/strut/models/") && rel.ends_with(".strutmodel") {
        pc_core::paths::physics_of(&rel)
    } else {
        return Err(format!("{rel} is not a model in a pack's strut/models folder."));
    };
    let bytes = fs_ops::read_bytes(&root, &rel, 64 * 1024 * 1024).map_err(text)?;
    let model = pc_core::strut_model::read(&bytes).map_err(|e| format!("{rel} cannot be read: {e}"))?;
    let bones = pc_core::oldpack::physics_bones(&model);
    let existing = match fs_ops::read(&root, &path) {
        Ok(Content::Text { text }) => Some(text),
        _ => None,
    };

    match pc_core::oldpack::add_basic_physics(existing.as_deref(), &bones).map_err(text)? {
        Some((new_text, added)) => {
            fs_ops::write_text(&root, &path, &new_text).map_err(text)?;
            pc_core::history::record_now(&root, &path, existing.as_deref(), &new_text);

            Ok(PhysicsAdded { path, added })
        }
        None => Err("Nothing to add: no bone of this model has a name the basic physics knows (boobL, boobR, belly, cheekL, cheekR, assL, assR...), or its physics file already has them all.".to_string()),
    }
}

#[tauri::command(async)]
fn read_animation(app: State<App>, rel: String) -> Reply<Vec<pc_core::strut_anim::Clip>> {
    let bytes = match examples::strip(&rel) {
        Some(path) => examples::bytes(path).ok_or_else(|| format!("{rel} is not one of the examples."))?,
        None => {
            let (root, _) = app.open()?;

            fs_ops::read_bytes(&root, &rel, 64 * 1024 * 1024).map_err(text)?
        }
    };

    pc_core::strut_anim::read(&bytes).map_err(|e| format!("{rel} cannot be read: {e}"))
}

/// Replaces the cues (events) of one clip in an animation file. Only the events change; the keyframes are copied as they were.
#[tauri::command(async)]
fn save_clip_events(app: State<App>, rel: String, clip: String, events: Vec<pc_core::strut_anim::Event>) -> Reply<Vec<pc_core::strut_anim::Clip>> {
    let (root, _) = app.open()?;
    let bytes = fs_ops::read_bytes(&root, &rel, 64 * 1024 * 1024).map_err(text)?;
    let edited = pc_core::strut_anim::edit_events(&bytes, &[pc_core::strut_anim::ClipEvents { clip, events }]).map_err(|e| format!("{rel} was not changed: {e}"))?;

    /* What was written must read back: a file the game cannot read is never left behind. */
    let clips = pc_core::strut_anim::read(&edited).map_err(|e| format!("{rel} was not changed: the edited file did not read back ({e})"))?;

    fs_ops::write_bytes(&root, &rel, &edited).map_err(text)?;
    Ok(clips)
}

/// Every cue name the game's animations and this pack's animations fire, for suggestions while a cue is typed.
#[tauri::command(async)]
fn cue_names(app: State<App>) -> Reply<Vec<String>> {
    let (root, _) = app.open()?;
    let facts = check::collect_facts(&root);
    let mut names: std::collections::BTreeSet<String> = app.reference.known_cues.iter().cloned().collect();

    names.extend(facts.cues.iter().cloned());
    Ok(names.into_iter().collect())
}

// =================================================================
// Blockbench
// =================================================================

#[tauri::command]
async fn blockbench_status() -> Reply<blockbench::Status> {
    tauri::async_runtime::spawn_blocking(blockbench::status).await.map_err(text)
}

#[tauri::command(async)]
fn install_blockbench() -> Reply<()> {
    blockbench::clear_install_failure();
    blockbench::install()
}

/// Starts Blockbench with the Strut Toolkit loaded, without opening anything.
#[tauri::command]
async fn prepare_blockbench() -> Reply<()> {
    tauri::async_runtime::spawn_blocking(blockbench::prepare).await.map_err(text)?
}

/// Looks for a newer release of the Strut Toolkit on GitHub and downloads it.
#[tauri::command]
async fn update_toolkit() -> Reply<String> {
    tauri::async_runtime::spawn_blocking(blockbench::update_plugin).await.map_err(text)?
}

/// Opens a model (with the texture, and an animation file on it when one is given) in Blockbench. The files are named relative to the project.
#[tauri::command]
async fn open_in_blockbench(app: State<'_, App>, model: String, texture: Option<String>, animation: Option<String>) -> Reply<()> {
    let (root, _) = app.open()?;
    let job = blockbench::Job {
        model: fs_ops::resolve(&root, &model).map_err(text)?,
        texture: texture.map(|t| fs_ops::resolve(&root, &t)).transpose().map_err(text)?,
        animation: animation.map(|a| fs_ops::resolve(&root, &a)).transpose().map_err(text)?,
    };

    tauri::async_runtime::spawn_blocking(move || blockbench::open(&job)).await.map_err(text)?
}

// =================================================================
// Importing
// =================================================================

#[derive(Serialize)]
struct ImportResult {
    girl: String,
    written: Vec<String>,
    notes: Vec<Note>,
}

/// Brings an old girl's profile into the pack: her girl file, a scene file for each old scene, and the scene names in the pack's language file.
#[tauri::command(async)]
fn import_profile(app: State<App>, path: String, body_type: String) -> Reply<ImportResult> {
    let (root, meta) = app.open()?;
    let file = PathBuf::from(&path);
    let text = std::fs::read_to_string(&file).map_err(|e| format!("{path} could not be read: {e}"))?;
    let stem = file.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let existing: Vec<String> = check::walk(&root)
        .iter()
        .filter_map(|rel| {
            let located = locate(rel);

            (located.kind == FileKind::Scene).then(|| located.id).flatten()
        })
        .collect();
    let imported = import::import_profile(&text, &stem, &body_type, &meta.namespace, &existing, &app.reference).map_err(text_of)?;

    for (rel, _) in &imported.files {
        if root.join(rel).exists() {
            return Err(format!("{rel} already exists in the pack. Rename or remove it first."));
        }
    }

    for (rel, body) in &imported.files {
        fs_ops::create_file(&root, rel, body).map_err(text_of)?;
    }

    let mut written: Vec<String> = imported.files.iter().map(|(rel, _)| rel.clone()).collect();

    if !imported.lang.is_empty() {
        let lang_rel = "data/pleasurehorizons/asset_sync/lang/en_us.json".to_string();
        let mut lang: serde_json::Map<String, serde_json::Value> = match fs_ops::read(&root, &lang_rel) {
            Ok(Content::Text { text }) => serde_json::from_str(&text).unwrap_or_default(),
            _ => Default::default(),
        };

        for (key, value) in &imported.lang {
            lang.insert(key.clone(), serde_json::Value::String(value.clone()));
        }

        let body = serde_json::to_string_pretty(&lang).unwrap() + "
";

        if root.join(&lang_rel).exists() {
            fs_ops::write_text(&root, &lang_rel, &body).map_err(text_of)?;
        } else {
            fs_ops::create_file(&root, &lang_rel, &body).map_err(text_of)?;
        }

        written.push(lang_rel);
    }

    Ok(ImportResult { girl: imported.files[0].0.clone(), written, notes: imported.notes })
}

fn text_of<E: std::fmt::Display>(error: E) -> String {
    error.to_string()
}

/// Merges entries into a JSON object file of the pack (a language file, sounds.json), making it if it is not there. An entry the file already has is kept as it is.
fn merge_object(root: &std::path::Path, rel: &str, entries: &std::collections::BTreeMap<String, serde_json::Value>, replace: bool) -> Result<(), String> {
    let mut object: serde_json::Map<String, serde_json::Value> = match fs_ops::read(root, rel) {
        Ok(Content::Text { text }) => serde_json::from_str(&text).unwrap_or_default(),
        _ => Default::default(),
    };

    for (key, value) in entries {
        if replace || !object.contains_key(key) {
            object.insert(key.clone(), value.clone());
        }
    }

    let body = serde_json::to_string_pretty(&object).unwrap() + "\n";

    if root.join(rel).exists() { fs_ops::write_text(root, rel, &body) } else { fs_ops::create_file(root, rel, &body) }.map_err(text_of)
}

#[derive(Serialize)]
struct OldPackInfo {
    girls: Vec<String>,
    namespace: String,
    /// The pack was made for the old mod (one model, one animation file) and was rewritten into the usual layout on opening.
    rewritten: bool,
}

/// The girls an old resource pack (a folder or a zip) holds.
#[tauri::command(async)]
fn old_pack_girls(app: State<App>, path: String) -> Reply<OldPackInfo> {
    /* Opened into the one held for the dialog, so a zip is unpacked once and not again for every look at a girl. */
    app.with_old_pack(&path, |pack| Ok(OldPackInfo { girls: pack.girls.clone(), namespace: pack.namespace.clone(), rewritten: pack.rewritten }))
}

/// Converts one model (`.geo.json`) or animation (`.animation.json`) file into the pack as a `.strutmodel` or `.strutanim` for a girl. Nothing is overwritten.
///
/// `role` is `model_nude`, `model_dressed`, `animation_base`, `animation_scene` (with `scene`) or `animation_keep`. `stamp` is the `.geo.json` her animation was made for.
#[tauri::command(async)]
fn convert_file(app: State<App>, source: String, role: String, girl: String, scene: Option<String>, stamp: Option<String>) -> Reply<ImportResult> {
    use pc_core::convert::standalone::{convert, Role};

    let (root, _) = app.open()?;
    let path = PathBuf::from(&source);
    let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{source} could not be read: {e}"))?;
    let stamp_text = match stamp.filter(|s| !s.trim().is_empty()) {
        Some(file) => Some(std::fs::read_to_string(&file).map_err(|e| format!("{file} could not be read: {e}"))?),
        None => None,
    };
    let role = match role.as_str() {
        "model_nude" => Role::ModelNude,
        "model_dressed" => Role::ModelDressed,
        "animation_base" => Role::AnimationBase,
        "animation_scene" => Role::AnimationScene(scene.unwrap_or_default()),
        _ => Role::AnimationKeep,
    };
    let output = convert(&text, &name, &role, &girl, stamp_text.as_deref()).map_err(text_of)?;
    let target = fs_ops::resolve(&root, &output.rel).map_err(text_of)?;

    if target.exists() {
        return Err(format!("{} already exists in the pack. Delete or rename it first, then convert again.", output.rel));
    }

    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(text_of)?;
    }

    std::fs::write(&target, &output.bytes).map_err(|e| format!("{} could not be written: {e}", output.rel))?;

    Ok(ImportResult { girl: String::new(), written: vec![output.rel], notes: output.notes })
}

// =================================================================
// Reviewing an old girl before she is imported
// =================================================================

impl App {
    /// Runs something on the old pack at this path, opening it the first time (and again when the path changes).
    fn with_old_pack<T>(&self, path: &str, work: impl FnOnce(&pc_core::oldpack::OldPack) -> Reply<T>) -> Reply<T> {
        let mut held = self.old.lock().unwrap();

        if held.as_ref().map(|(open, _)| open.as_str()) != Some(path) {
            *held = Some((path.to_string(), pc_core::oldpack::open(&PathBuf::from(path)).map_err(text_of)?));
        }

        work(&held.as_ref().unwrap().1)
    }
}

/// What is in one old girl's pack, and the scenes to start from (her profile's, else found from her clip names).
#[tauri::command(async)]
fn old_pack_look(app: State<App>, path: String, girl: String, profile: Option<String>) -> Reply<pc_core::review::GirlLook> {
    let profile_text = match profile.filter(|p| !p.trim().is_empty()) {
        Some(file) => Some(std::fs::read_to_string(&file).map_err(|e| format!("{file} could not be read: {e}"))?),
        None => None,
    };

    app.with_old_pack(&path, |pack| pc_core::review::look(pack, &girl, profile_text.as_deref()).map_err(text_of))
}

/// One of her models, converted, for the 3D view.
#[tauri::command(async)]
fn old_pack_model(app: State<App>, path: String, girl: String, kind: String) -> Reply<pc_core::strut_model::Model> {
    app.with_old_pack(&path, |pack| pc_core::review::model(pack, &girl, &kind).map_err(text_of))
}

/// Her texture as a data URL, if she has one.
#[tauri::command(async)]
fn old_pack_texture(app: State<App>, path: String, girl: String) -> Reply<Option<String>> {
    app.with_old_pack(&path, |pack| Ok(pc_core::review::texture(pack, &girl).map(|bytes| format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))))
}

/// Every clip of her animation file, converted, to play on her model.
#[tauri::command(async)]
fn old_pack_clips(app: State<App>, path: String, girl: String) -> Reply<Vec<pc_core::strut_anim::Clip>> {
    app.with_old_pack(&path, |pack| pc_core::review::clips(pack, &girl).map_err(text_of))
}

/// The scenes found from the names of a girl's clips.
#[tauri::command(async)]
fn detect_scenes(clips: Vec<String>) -> Vec<pc_core::review::ScenePlan> {
    pc_core::review::detect(&clips)
}

/// The body type whose skeleton (of the game's own base girls) is most like these bones, or nothing when none is close enough to say.
#[tauri::command(async)]
fn guess_body_type(bones: Vec<String>) -> Option<String> {
    use std::collections::BTreeSet;

    let names: BTreeSet<&str> = bones.iter().map(String::as_str).collect();
    let mut best: Option<(String, f64)> = None;

    for name in examples::names() {
        let Some(id) = name.strip_prefix("data/pleasurehorizons/girls/").and_then(|n| n.strip_suffix(".json")) else { continue };
        let Some(body) = examples::text(&name).and_then(|t| body_type_of(&t)) else { continue };
        let Some(model_path) = examples::names().into_iter().find(|n| n.ends_with(&format!("/nude/{id}.strutmodel")) && !n.contains("/armour/")) else { continue };
        let Some(model) = examples::bytes(&model_path).and_then(|b| pc_core::strut_model::read(&b).ok()) else { continue };
        let theirs: BTreeSet<&str> = model.group_bones.iter().map(|b| b.name.as_str()).collect();
        let score = names.intersection(&theirs).count() as f64 * 2.0 / (names.len() + theirs.len()).max(1) as f64;

        if best.as_ref().map_or(true, |(_, top)| score > *top) {
            best = Some((body, score));
        }
    }

    /* Under about half alike is a guess not worth showing. */
    best.filter(|(_, score)| *score >= 0.5).map(|(body, _)| body)
}

#[derive(Serialize)]
struct CueCount {
    token: String,
    count: usize,
}

/// The cues a girl's old animations fire, for deciding what each becomes.
#[tauri::command(async)]
fn old_pack_cues(app: State<App>, path: String, girl: String) -> Reply<Vec<CueCount>> {
    app.with_old_pack(&path, |pack| Ok(pc_core::oldpack::scan_cues(pack, &girl).map_err(text_of)?.into_iter().map(|(token, count)| CueCount { token, count }).collect()))
}

/// For each girl id, her old profile if the folder holds `<id>.json` (the old config keeps one file for each girl).
#[tauri::command(async)]
fn find_profiles(folder: String, ids: Vec<String>) -> Vec<Option<String>> {
    ids.iter().map(|id| [id.clone(), id.to_lowercase()].iter().map(|n| PathBuf::from(&folder).join(format!("{n}.json"))).find(|p| p.is_file()).map(|p| p.display().to_string())).collect()
}

#[derive(Deserialize)]
struct GirlJob {
    girl: String,
    profile: Option<String>,
    body_type: String,
    /// The scenes the person settled on in the review, replacing the ones her profile lists.
    scenes: Option<Vec<pc_core::review::ScenePlan>>,
}

#[derive(Deserialize)]
struct CueRuleArg {
    token: String,
    /// marker, sound, custom or leave_out
    action: String,
    value: Option<String>,
    authority: u32,
}

/// Brings girls in from an old resource pack, each with her old profile if there is one. The pack is opened once; what it shares between girls (textures, sounds,
/// language files) comes with the first.
#[tauri::command(async)]
fn import_old_pack(app: State<App>, pack: String, jobs: Vec<GirlJob>, liquid: bool, physics_preset: bool, rules: Vec<CueRuleArg>) -> Reply<ImportResult> {
    use pc_core::convert::events::{CueAction, CueRule, Mapping};

    let (root, meta) = app.open()?;
    let mut mapping = Mapping::new();

    for rule in rules {
        let action = match rule.action.as_str() {
            "sound" => CueAction::Sound,
            "custom" => CueAction::Custom,
            "leave_out" => CueAction::LeaveOut,
            _ => CueAction::Marker,
        };

        mapping.insert(rule.token.to_lowercase(), CueRule { action, value: rule.value, authority: rule.authority });
    }

    let several = jobs.len() > 1;
    let mut result = ImportResult { girl: String::new(), written: Vec::new(), notes: Vec::new() };

    /* The pack the dialog already opened (a zip is not unpacked again). */
    app.with_old_pack(&pack, |opened| {
    for (position, job) in jobs.into_iter().enumerate() {
        let one = import_one(&app, &root, &meta, opened, &job, liquid, physics_preset, &mapping, position > 0)?;

        if result.girl.is_empty() {
            result.girl = one.girl;
        }

        result.written.extend(one.written);
        result.notes.extend(one.notes.into_iter().map(|n| if several { Note { level: n.level, message: format!("{}: {}", job.girl, n.message) } } else { n }));
    }

    Ok(())
    })?;

    /* The last pass: what was brought in carries the old male rig in the girls' models and scene files, so the pack is converted to the new scene system (one model per
       participant), in place. The male is the base mod's own, so the old rig is not kept. */
    match pc_core::pack_split::split_pack(&root, true) {
        Ok(split) => {
            use pc_core::import::Level;

            if split.scenes > 0 || split.models_stripped > 0 {
                result.notes.push(Note { level: Level::Changed, message: format!("Converted to the new scene system: {} scenes now have a layout, the girls' scene animations are split into the girl's clips and the male's, and the old male rig was taken out of {} models.", split.scenes, split.models_stripped) });
            }

            if !split.dropped_bones.is_empty() {
                result.notes.push(Note { level: Level::Dropped, message: format!("Animation of old male bones with no counterpart in the new male model was left out: {}.", split.dropped_bones.iter().map(|(b, n)| format!("{b} ({n} files)")).collect::<Vec<_>>().join(", ")) });
            }

            if !split.approximate.is_empty() {
                result.notes.push(Note { level: Level::Review, message: format!("These male bones only come close to where they were, because the new male model differs: {}. Check the scenes in the viewer.", split.approximate.iter().map(|(b, by)| format!("{b} (up to {by:.2} off)")).collect::<Vec<_>>().join(", ")) });
            }

            for skipped in &split.skipped {
                result.notes.push(Note { level: Level::Review, message: format!("Left in the old form: {skipped}") });
            }

            result.written.extend(split.written);
        }
        Err(error) => result.notes.push(Note { level: pc_core::import::Level::Error, message: format!("The pack could not be converted to the new scene system: {}", text_of(error)) }),
    }

    Ok(result)
}

fn import_one(app: &App, root: &std::path::Path, meta: &ProjectMeta, opened: &pc_core::oldpack::OldPack, job: &GirlJob, liquid: bool, physics_preset: bool, rules: &pc_core::convert::events::Mapping, skip_shared: bool) -> Reply<ImportResult> {
    let girl = job.girl.clone();
    let body_type = job.body_type.clone();
    let profile_text = match job.profile.clone().filter(|p| !p.trim().is_empty()) {
        Some(path) => Some(std::fs::read_to_string(&path).map_err(|e| format!("{path} could not be read: {e}"))?),
        None => None,
    };
    let files = check::walk(root).into_iter().collect::<std::collections::HashSet<String>>();
    let scenes: Vec<String> = files
        .iter()
        .filter_map(|rel| {
            let located = locate(rel);

            (located.kind == FileKind::Scene).then(|| located.id).flatten()
        })
        .collect();
    let taken = pc_core::oldpack::taken_tame_items(root);
    let outcome = pc_core::oldpack::import_girl(
        opened,
        &pc_core::oldpack::Request { raw_id: &girl, profile: profile_text.as_deref(), body_type: &body_type, namespace: &meta.namespace, liquid, existing_files: &files, existing_scenes: &scenes, rules, physics_preset, skip_shared, taken_tame_items: &taken, scene_plan: job.scenes.as_deref() },
        &app.reference,
    )
    .map_err(text_of)?;

    /* The folders are made once each, then the files are written several at a time: a girl brings hundreds of sounds, and a folder of them is slow one by one. */
    let mut targets: Vec<(PathBuf, &Vec<u8>)> = Vec::with_capacity(outcome.files.len());
    let mut folders = std::collections::HashSet::new();

    for (rel, bytes) in &outcome.files {
        let path = fs_ops::resolve(&root, rel).map_err(text_of)?;

        if let Some(parent) = path.parent() {
            if folders.insert(parent.to_path_buf()) {
                std::fs::create_dir_all(parent).map_err(text_of)?;
            }
        }

        targets.push((path, bytes));
    }

    let failed = std::sync::Mutex::new(None::<String>);
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(8);
    let chunk = targets.len().div_ceil(threads).max(1);

    std::thread::scope(|scope| {
        for part in targets.chunks(chunk) {
            let failed = &failed;

            scope.spawn(move || {
                for (path, bytes) in part {
                    if let Err(error) = std::fs::write(path, bytes) {
                        failed.lock().unwrap().get_or_insert(format!("{} could not be written: {error}", path.display()));
                        return;
                    }
                }
            });
        }
    });

    if let Some(message) = failed.into_inner().unwrap() {
        return Err(message);
    }

    let mut written: Vec<String> = outcome.files.iter().map(|(rel, _)| rel.clone()).collect();

    for (rel, entries) in &outcome.merges {
        /* A language file takes the new entries over the old; sounds.json keeps what it already defines. */
        merge_object(&root, rel, entries, rel.contains("/lang/"))?;
        written.push(rel.clone());
    }

    Ok(ImportResult { girl: outcome.girl_file, written, notes: outcome.notes })
}

// =================================================================
// Reference
// =================================================================

#[derive(Serialize)]
struct Choices {
    archetypes: Vec<String>,
    body_types: Vec<String>,
    scene_types: Vec<String>,
    trait_categories: Vec<String>,
}

/// Everything the game says it has, for the guided forms.
#[tauri::command(async)]
fn reference(app: State<App>) -> Reference {
    app.reference.clone()
}

#[tauri::command(async)]
fn choices(app: State<App>) -> Choices {
    let reference = &app.reference;

    Choices {
        archetypes: reference.archetype_names(),
        body_types: reference.body_types(),
        scene_types: reference.scene_fields.iter().find(|f| f.name == "type").map(|f| f.options.clone()).unwrap_or_default(),
        trait_categories: reference.categories.keys().cloned().collect(),
    }
}

/// The JSON Schema the editor uses for a file, or nothing for a kind that has none.
#[tauri::command(async)]
fn schema_for(app: State<App>, rel: String) -> Reply<Option<serde_json::Value>> {
    let (root, _) = app.open()?;
    let facts: PackFacts = check::collect_facts(&root);
    let pack_traits: Vec<String> = facts.traits.keys().map(|id| id.rsplit_once(':').map(|(_, name)| name).unwrap_or(id).to_string()).collect();

    Ok(pc_core::schema::for_kind(&locate(&rel).kind, &app.reference, &pack_traits))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            std::thread::spawn(clear_stale_unpacked);

            let data = app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("."));

            app.manage(App { reference: Reference::builtin(), recent_file: data.join("recent.json"), current: Mutex::new(None), old: Mutex::new(None) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_recent,
            forget_recent,
            default_projects_dir,
            contents_options,
            namespace_problem,
            create_project,
            open_project,
            close_project,
            list_dir,
            list_files,
            locate_file,
            read_file,
            stamps,
            read_image,
            write_file,
            fix_arms,
            skeleton_plan,
            skeleton_apply,
            split_scenes,
            migration_status,
            undo_split_scenes,
            file_history,
            read_version,
            check_text,
            check_project,
            create_entry,
            rename_entry,
            delete_entry,
            import_files,
            copy_to_clipboard,
            copy_into,
            new_file,
            choices,
            reference,
            import_profile,
            viewer_files,
            read_model,
            add_basic_physics,
            read_animation,
            delete_clips,
            scene_scopes,
            scene_make_skeleton_wide,
            scene_make_girl_specific,
            save_clip_events,
            blockbench_status,
            game_log_default,
            sim_girls,
            search_text,
            replace_in_files,
            duplicate_entry,
            reveal_in_explorer,
            wizard::check_girl_models,
            wizard::create_girl,
            wizard::example_preview,
            example_girls,
            example_scenes,
            bundled_bed,
            bundled_male,
            sim_files,
            read_asset,
            read_game_log,
            install_blockbench,
            prepare_blockbench,
            update_toolkit,
            open_in_blockbench,
            cue_names,
            old_pack_girls,
            old_pack_cues,
            old_pack_look,
            old_pack_model,
            old_pack_texture,
            old_pack_clips,
            guess_body_type,
            detect_scenes,
            find_profiles,
            import_old_pack,
            convert_file,
            export_zip,
            install_pack,
            saves_dir,
            schema_for
        ])
        .build(tauri::generate_context!())
        .expect("the Pack Creator could not start")
        .run(|handle, event| {
            /* The old pack held for the import dialog was unpacked into a temporary folder; it is removed when the window closes. */
            if let tauri::RunEvent::Exit = event {
                if let Some(app) = handle.try_state::<App>() {
                    *app.old.lock().unwrap() = None;
                }
            }
        });
}

/// Removes the temporary folders of packs unpacked by an earlier run that did not get to clean up (it was closed in the middle of an import, or crashed).
fn clear_stale_unpacked() {
    let mine = std::process::id().to_string();
    let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) else { return };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(rest) = name.strip_prefix("ph-oldpack-").or_else(|| name.strip_prefix("ph-fapcraft-")).or_else(|| name.strip_prefix("ph-toolkit-")) else { continue };

        /* Not this run's, and not touched for ten minutes (another window of the program may still be using it). */
        let old_enough = entry.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).map_or(false, |age| age.as_secs() > 600);

        if rest.split('-').next() != Some(mine.as_str()) && old_enough {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}
