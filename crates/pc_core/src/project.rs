//! A project is a folder that is a data pack. What the tool adds is one small file, `.packcreator/project.json`; everything else is the pack itself.

use crate::error::{Error, Result};
use crate::layout::{folder_for, FileKind, ASSET_SYNC, GAME_NAMESPACE};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const META_DIR: &str = ".packcreator";
pub const META_FILE: &str = "project.json";
/// The format a new pack is made in (see `migrate`).
const FORMAT: u32 = crate::migrate::CURRENT;

/// What a pack can hold. A pack may hold any of them, one of them or none yet.
pub const CONTENTS: [(&str, &str); 5] = [
    ("girls", "Girls (with their models, textures and sounds)"),
    ("scenes", "Scenes"),
    ("traits", "Traits"),
    ("dialogue", "Dialogue"),
    ("social", "Small talk on breaks"),
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectMeta {
    pub format: u32,
    pub name: String,
    /// The namespace the pack's data is in. Girls' own files are always in `pleasurehorizons` whatever this is.
    pub namespace: String,
    pub author: String,
    pub description: String,
    /// What the pack was set up to hold (the ids of [`CONTENTS`]).
    pub contents: Vec<String>,
    /// When it was made, in seconds since 1970.
    pub created: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewProject {
    pub name: String,
    /// The folder the pack's folder is made in.
    pub parent: PathBuf,
    pub namespace: String,
    pub author: String,
    pub description: String,
    pub contents: Vec<String>,
}

/// Why a namespace is not acceptable, or None when it is.
pub fn namespace_problem(namespace: &str) -> Option<String> {
    if namespace.is_empty() {
        return Some("The namespace cannot be empty.".into());
    }

    if !namespace.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '-' | '.')) {
        return Some("The namespace uses lower case letters, digits and _ - . only.".into());
    }

    if ["minecraft", "realms", "fabric", "fabricloader", "c"].contains(&namespace) {
        return Some(format!("\"{namespace}\" belongs to Minecraft or the loader and cannot be used."));
    }

    None
}

/// The folder name a pack of this name gets: the name with what Windows does not allow in a folder name taken out.
pub fn folder_name(name: &str) -> String {
    let cleaned: String = name.trim().chars().filter(|c| !matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') && !c.is_control()).collect();

    cleaned.trim().trim_end_matches('.').to_string()
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Makes the pack's folder with its `pack.mcmeta`, the folders for what it holds, and the tool's file. Returns the folder.
pub fn create(request: &NewProject) -> Result<(PathBuf, ProjectMeta)> {
    let folder = folder_name(&request.name);

    if folder.is_empty() {
        return Err(Error::invalid("Give the pack a name."));
    }

    if let Some(problem) = namespace_problem(&request.namespace) {
        return Err(Error::invalid(problem));
    }

    if !request.parent.is_dir() {
        return Err(Error::invalid(format!("The folder {} does not exist.", request.parent.display())));
    }

    let root = request.parent.join(&folder);

    if root.exists() && fs::read_dir(&root).map(|mut entries| entries.next().is_some()).unwrap_or(true) {
        return Err(Error::invalid(format!("{} already exists and is not empty. Choose another name or folder.", root.display())));
    }

    let meta = ProjectMeta {
        format: FORMAT,
        name: request.name.trim().to_string(),
        namespace: request.namespace.clone(),
        author: request.author.trim().to_string(),
        description: request.description.trim().to_string(),
        contents: request.contents.iter().filter(|c| CONTENTS.iter().any(|(id, _)| id == c)).cloned().collect(),
        created: now(),
    };

    fs::create_dir_all(&root).map_err(|e| Error::io(&root, e))?;

    let description = if meta.description.is_empty() { meta.name.clone() } else { meta.description.clone() };
    let mcmeta = serde_json::json!({ "pack": { "pack_format": crate::check::PACK_FORMAT, "description": description } });

    write(&root.join("pack.mcmeta"), &(serde_json::to_string_pretty(&mcmeta).unwrap() + "\n"))?;

    for content in &meta.contents {
        let kind = match content.as_str() {
            "girls" => FileKind::Girl,
            "scenes" => FileKind::Scene,
            "traits" => FileKind::Trait,
            "social" => FileKind::Social,
            _ => FileKind::Dialogue,
        };

        if let Some(folder) = folder_for(&kind) {
            make_dir(&root.join("data").join(&meta.namespace).join(folder))?;
        }

        if content == "girls" {
            make_dir(&root.join("data").join(GAME_NAMESPACE).join(ASSET_SYNC))?;
        }
    }

    save_meta(&root, &meta)?;

    Ok((root, meta))
}

/// Opens a folder as a project. A folder that is a data pack but was not made with the tool is taken on: its name and namespace are worked out from what is in it.
pub fn open(root: &Path) -> Result<ProjectMeta> {
    if !root.is_dir() {
        return Err(Error::invalid(format!("{} is not a folder.", root.display())));
    }

    let meta_path = root.join(META_DIR).join(META_FILE);

    if meta_path.is_file() {
        let text = fs::read_to_string(&meta_path).map_err(|e| Error::io(&meta_path, e))?;

        return serde_json::from_str(&text).map_err(|e| Error::invalid(format!("The project file {} is damaged: {e}", meta_path.display())));
    }

    if !root.join("pack.mcmeta").is_file() {
        return Err(Error::invalid(format!("{} is not a data pack: it has no pack.mcmeta.", root.display())));
    }

    let namespaces: Vec<String> = fs::read_dir(root.join("data"))
        .map(|entries| entries.flatten().filter(|e| e.path().is_dir()).map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n != "minecraft").collect())
        .unwrap_or_default();
    let namespace = namespaces.iter().find(|n| n.as_str() != GAME_NAMESPACE).or(namespaces.first()).cloned().unwrap_or_else(|| GAME_NAMESPACE.to_string());

    let has = |folder: &str| namespaces.iter().any(|ns| root.join("data").join(ns).join(folder).is_dir());
    let contents = [("girls", "girls"), ("scenes", "scenes"), ("traits", "girl_traits"), ("dialogue", "dialogue")]
        .iter()
        .filter(|(_, folder)| has(folder))
        .map(|(id, _)| id.to_string())
        .collect();

    let meta = ProjectMeta {
        format: FORMAT,
        name: root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "Pack".into()),
        namespace,
        author: String::new(),
        description: String::new(),
        contents,
        created: now(),
    };

    save_meta(root, &meta)?;
    Ok(meta)
}

pub fn save_meta(root: &Path, meta: &ProjectMeta) -> Result<()> {
    let dir = root.join(META_DIR);

    make_dir(&dir)?;
    write(&dir.join(META_FILE), &(serde_json::to_string_pretty(meta).unwrap() + "\n"))
}

fn make_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path).map_err(|e| Error::io(path, e))
}

fn write(path: &Path, text: &str) -> Result<()> {
    fs::write(path, text).map_err(|e| Error::io(path, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(parent: &Path, name: &str) -> NewProject {
        NewProject { name: name.into(), parent: parent.into(), namespace: "pleasurehorizons".into(), author: "me".into(), description: "".into(), contents: vec!["girls".into(), "traits".into()] }
    }

    #[test]
    fn makes_a_pack_folder() {
        let dir = tempfile::tempdir().unwrap();
        let (root, meta) = create(&request(dir.path(), "My Pack: v1?")).unwrap();

        assert_eq!(root.file_name().unwrap(), "My Pack v1");
        assert_eq!(meta.name, "My Pack: v1?");
        assert!(root.join("pack.mcmeta").is_file());
        assert!(root.join("data/pleasurehorizons/girls").is_dir());
        assert!(root.join("data/pleasurehorizons/girl_traits").is_dir());
        assert!(root.join("data/pleasurehorizons/asset_sync").is_dir());
        assert!(!root.join("data/pleasurehorizons/scenes").exists());
        assert_eq!(open(&root).unwrap(), meta);
    }

    #[test]
    fn refuses_a_bad_name_namespace_or_folder() {
        let dir = tempfile::tempdir().unwrap();

        assert!(create(&request(dir.path(), "  ")).is_err());

        let mut bad = request(dir.path(), "A");

        bad.namespace = "Minecraft".into();
        assert!(create(&bad).is_err());
        bad.namespace = "minecraft".into();
        assert!(create(&bad).is_err());

        create(&request(dir.path(), "Taken")).unwrap();
        assert!(create(&request(dir.path(), "Taken")).unwrap_err().to_string().contains("already exists"));
    }

    #[test]
    fn takes_on_a_pack_made_elsewhere() {
        let dir = tempfile::tempdir().unwrap();

        assert!(open(dir.path()).is_err());

        fs::write(dir.path().join("pack.mcmeta"), "{}").unwrap();
        fs::create_dir_all(dir.path().join("data/mypack/scenes")).unwrap();

        let meta = open(dir.path()).unwrap();

        assert_eq!(meta.namespace, "mypack");
        assert_eq!(meta.contents, vec!["scenes"]);
        assert!(dir.path().join(META_DIR).join(META_FILE).is_file());
    }
}
