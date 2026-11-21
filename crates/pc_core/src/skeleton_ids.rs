//! Skeleton ids: the one string on a model (and on every animation file made for it) that says which rig it is, `pleasurehorizons:jenny`. The game and Strut compare these ids and
//! nothing else to decide who may play a clip or fill a scene role. This finds the id each model and animation file of a pack should have, shows the grouping so a person can
//! merge girls who share a rig or split one who does not, and writes the ids in. Writing an id changes no behaviour by itself.

use crate::{fs_ops, layout::ASSET_SYNC, strut_anim, strut_model, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// What a model or an animation file is.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Model,
    Animation,
}

/// One file and the id it should have.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Assignment {
    pub rel: String,
    pub kind: Kind,
    /// Who the file belongs to: the girl (`amber`) or the skeleton (`jenny`) its name gives.
    pub owner: String,
    /// The id it should have.
    pub id: String,
    /// The id it has now.
    pub current: Option<String>,
}

/// Every id and the files that take it, for showing the grouping once.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Group {
    pub id: String,
    pub models: Vec<String>,
    pub animations: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Plan {
    pub assignments: Vec<Assignment>,
    pub groups: Vec<Group>,
    /// Files that could not be read, with why (they are left out).
    pub unreadable: Vec<String>,
}

/// The body type a girl's file names (`pleasurehorizons:body` / `body_type`), by her id, from the girl files of the pack.
fn body_types(root: &Path, files: &[String]) -> BTreeMap<String, String> {
    let mut found = BTreeMap::new();

    for rel in files {
        let parts: Vec<&str> = rel.split('/').collect();

        if parts.len() == 4 && parts[0] == "data" && parts[2] == "girls" && parts[3].ends_with(".json") {
            let Ok(text) = std::fs::read_to_string(root.join(rel)) else { continue };
            let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
            let body = json.get("components").and_then(|c| c.as_object()).and_then(|c| c.iter().find(|(k, _)| k.ends_with(":body")).map(|(_, v)| v)).and_then(|b| b.get("body_type")).and_then(|b| b.as_str());
            let id = json.get("id").and_then(|i| i.as_str()).map(str::to_string).unwrap_or_else(|| parts[3].trim_end_matches(".json").to_string());

            if let Some(body) = body {
                found.insert(id, body.to_string());
            }
        }
    }

    found
}

/// The partner's rig is named for what it is, whatever girl's file it came out of.
pub const PARTNER_ID: &str = "pleasurehorizons:partner";

/// Proposes an id for every model and animation file of the pack: a girl's model takes her body type (`jenny` is `pleasurehorizons:jenny`) when her girl file names one, and
/// her own id otherwise; an animation file takes the id of the girl or the skeleton in its name. `overrides` maps a girl or skeleton (the owner) to an id a person chose.
pub fn plan(root: &Path, overrides: &BTreeMap<String, String>) -> Plan {
    let files = crate::check::walk(root);
    let bodies = body_types(root, &files);
    let mut result = Plan::default();
    let mut models: BTreeMap<String, String> = BTreeMap::new();

    /* Models: data/<ns>/asset_sync/strut/models/<folder>/<girl>.strutmodel */
    for rel in &files {
        let parts: Vec<&str> = rel.split('/').collect();

        if parts.len() < 7 || parts[0] != "data" || parts[2] != ASSET_SYNC || parts[3] != "strut" || parts[4] != "models" || !rel.ends_with(".strutmodel") {
            continue;
        }

        /* Only the models that are a body get an id: not armour, blocks or items, which are fitted to a body or are not one. */
        if !matches!(parts[5], "nude" | "dressed" | "partner") {
            continue;
        }

        let namespace = parts[1];
        let stem = parts[parts.len() - 1].trim_end_matches(".strutmodel");
        let default = match bodies.get(stem) {
            Some(body) => format!("{}:{}", crate::layout::GAME_NAMESPACE, body),
            None if stem == "partner" || stem == "player" || parts[5] == "partner" => PARTNER_ID.to_string(),
            None => format!("{namespace}:{stem}"),
        };
        let id = overrides.get(stem).cloned().unwrap_or(default);

        let current = match std::fs::read(root.join(rel)).map_err(|e| e.to_string()).and_then(|b| strut_model::read(&b)) {
            Ok(model) => model.skeleton_id,
            Err(e) => {
                result.unreadable.push(format!("{rel}: {e}"));
                continue;
            }
        };

        models.entry(stem.to_string()).or_insert_with(|| id.clone());
        result.assignments.push(Assignment { rel: rel.clone(), kind: Kind::Model, owner: stem.to_string(), id, current });
    }

    /* Animation files: base.<girl>, <scene>-<girl> and <scene>.<skeleton>. What matches nothing is left alone (it is for everyone). */
    let skeleton_of = |owner: &str| -> Option<String> {
        if let Some(id) = overrides.get(owner) {
            return Some(id.clone());
        }

        if let Some(id) = models.get(owner) {
            return Some(id.clone());
        }

        /* A skeleton named by its body type, `<scene>.jenny`. */
        let bare = format!("{}:{}", crate::layout::GAME_NAMESPACE, owner);

        models.values().any(|id| *id == bare).then_some(bare)
    };

    for rel in &files {
        let parts: Vec<&str> = rel.split('/').collect();

        if parts.len() < 6 || parts[0] != "data" || parts[2] != ASSET_SYNC || parts[3] != "strut" || parts[4] != "animations" || !rel.ends_with(".strutanim") {
            continue;
        }

        let name = parts[parts.len() - 1].trim_end_matches(".strutanim");
        let owner = if let Some(girl) = name.strip_prefix("base.") {
            Some(girl.to_string())
        } else if parts.len() > 6 && parts[5] == "scenes" {
            /* `<scene>-<girl>` first, then `<scene>.<skeleton>`: the last separator splits them. */
            name.rsplit_once('-').map(|(_, girl)| girl.to_string()).filter(|girl| skeleton_of(girl).is_some()).or_else(|| name.rsplit_once('.').map(|(_, s)| s.to_string()))
        } else {
            None
        };
        let Some(owner) = owner else { continue };
        let Some(id) = skeleton_of(&owner) else { continue };

        let current = match std::fs::read(root.join(rel)).map_err(|e| e.to_string()).and_then(|b| strut_anim::skeleton_id(&b)) {
            Ok(id) => id,
            Err(e) => {
                result.unreadable.push(format!("{rel}: {e}"));
                continue;
            }
        };

        result.assignments.push(Assignment { rel: rel.clone(), kind: Kind::Animation, owner, id, current });
    }

    let mut groups: BTreeMap<String, Group> = BTreeMap::new();

    for a in &result.assignments {
        let group = groups.entry(a.id.clone()).or_insert_with(|| Group { id: a.id.clone(), models: Vec::new(), animations: 0 });

        match a.kind {
            Kind::Model => group.models.push(a.owner.clone()),
            Kind::Animation => group.animations += 1,
        }
    }

    result.groups = groups.into_values().collect();
    result
}

/// What writing the ids did.
#[derive(Debug, Clone, Serialize, Default)]
pub struct Written {
    pub written: usize,
    pub already: usize,
    pub failed: Vec<String>,
}

/// Writes the ids of a plan into the files. A file that already has its id is left as it is. Every new file must read back with its id before it replaces the old one, and a
/// copy of the original is kept in the project's `.packcreator/backups` folder.
pub fn apply(root: &Path, assignments: &[Assignment]) -> Result<Written> {
    let mut done = Written::default();
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);

    for a in assignments {
        let bytes = match fs_ops::read_bytes(root, &a.rel, 64 * 1024 * 1024) {
            Ok(b) => b,
            Err(e) => {
                done.failed.push(format!("{}: {e}", a.rel));
                continue;
            }
        };
        let edited = match a.kind {
            Kind::Model => strut_model::set_skeleton_id(&bytes, Some(&a.id)).and_then(|new| match strut_model::read(&new)?.skeleton_id.as_deref() {
                Some(id) if id == a.id => Ok(new),
                _ => Err("the new file did not read back with its id".to_string()),
            }),
            Kind::Animation => strut_anim::set_skeleton_id(&bytes, Some(&a.id)).and_then(|new| match strut_anim::skeleton_id(&new)?.as_deref() {
                Some(id) if id == a.id => Ok(new),
                _ => Err("the new file did not read back with its id".to_string()),
            }),
        };

        match edited {
            Ok(new) if new == bytes => done.already += 1,
            Ok(new) => {
                let ext = if a.kind == Kind::Model { "strutmodel" } else { "strutanim" };
                let folder = root.join(crate::project::META_DIR).join("backups").join(a.rel.replace(['/', '\\', ':'], "~"));

                if std::fs::create_dir_all(&folder).and_then(|_| std::fs::write(folder.join(format!("{stamp}.{ext}")), &bytes)).is_err() {
                    done.failed.push(format!("{}: a copy of the original could not be kept, so it was not changed", a.rel));
                    continue;
                }

                match fs_ops::write_bytes(root, &a.rel, &new) {
                    Ok(()) => done.written += 1,
                    Err(e) => done.failed.push(format!("{}: {e}", a.rel)),
                }
            }
            Err(e) => done.failed.push(format!("{}: {e}", a.rel)),
        }
    }

    Ok(done)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn copy_dir(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();

        for e in std::fs::read_dir(from).unwrap().flatten() {
            let p = e.path();

            if e.file_name().to_string_lossy().starts_with('.') {
                continue;
            }

            if p.is_dir() {
                copy_dir(&p, &to.join(e.file_name()));
            } else {
                std::fs::copy(&p, to.join(e.file_name())).unwrap();
            }
        }
    }

    #[test]
    fn plans_and_writes_the_ids_of_the_test_pack() {
        let source = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test_pack");

        if !source.join("data").is_dir() {
            return;
        }

        let copy = std::env::temp_dir().join(format!("pc-skeleton-ids-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&copy);

        copy_dir(&source, &copy);

        let first = plan(&copy, &BTreeMap::new());

        assert!(first.assignments.iter().filter(|a| a.kind == Kind::Model).all(|a| a.rel.contains("/models/nude/") || a.rel.contains("/models/dressed/") || a.rel.contains("/models/partner/")));

        assert!(first.unreadable.is_empty(), "{:?}", first.unreadable);
        assert!(first.assignments.iter().any(|a| a.kind == Kind::Model && a.owner == "jenny" && a.id == "pleasurehorizons:jenny"));
        assert!(first.assignments.iter().any(|a| a.kind == Kind::Animation && a.rel.ends_with("base.amber.strutanim") && a.id == "pleasurehorizons:jenny"));
        assert!(first.assignments.iter().any(|a| a.kind == Kind::Animation && a.rel.contains("scenes/") && a.rel.ends_with("-2b.strutanim") && a.id == "pleasurehorizons:jenny"));
        assert!(first.assignments.iter().all(|a| a.current.is_none()));

        let written = apply(&copy, &first.assignments).unwrap();

        assert!(written.failed.is_empty(), "{:?}", written.failed);
        assert_eq!(written.written, first.assignments.len());

        /* Planning again finds every id in place, and writing again changes nothing. */
        let second = plan(&copy, &BTreeMap::new());

        assert!(second.assignments.iter().all(|a| a.current.as_deref() == Some(a.id.as_str())));
        assert_eq!(apply(&copy, &second.assignments).unwrap().written, 0);

        /* A choice made in the app wins: amber gets a rig of her own. */
        let split = plan(&copy, &BTreeMap::from([("amber".to_string(), "pleasurehorizons:amber".to_string())]));

        assert!(split.groups.iter().find(|g| g.id == "pleasurehorizons:amber").unwrap().models.contains(&"amber".to_string()));
        assert!(!split.groups.iter().find(|g| g.id == "pleasurehorizons:jenny").unwrap().models.contains(&"amber".to_string()));
        let _ = std::fs::remove_dir_all(&copy);
    }
}
