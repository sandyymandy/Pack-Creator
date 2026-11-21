//! What a file in a pack is, from where it is. A pack is one data pack: its content is in `data/<namespace>/...`, and what the game sends to players (models,
//! textures, sounds, animations) is under `data/<namespace>/asset_sync/`, as Strut's asset sync defines.

use serde::{Deserialize, Serialize};

/// The folder (inside `data/<namespace>/`) that holds what is sent to players.
pub const ASSET_SYNC: &str = "asset_sync";

/// The namespace the game looks in for a girl's own files. Her model, texture and animations are always found here, whatever namespace the pack uses for its data.
pub const GAME_NAMESPACE: &str = "pleasurehorizons";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    PackMeta,
    Girl,
    Scene,
    Trait,
    GirlAi,
    Dialogue,
    /// What settlers say on a break (`data/<namespace>/social/`).
    Social,
    LootTable,
    /// Data of some other kind the game or Minecraft reads (tags, structures and so on).
    OtherData,
    Model,
    Animation,
    Texture,
    Sound,
    SoundsJson,
    Lang,
    EventBindings,
    KeyframeEvents,
    Physics,
    /// Which of a girl's bones armour hides, what texture it uses and which materials look which way (`strut/armour/`).
    ArmourWearer,
    ArmourSet,
    ArmourLook,
    /// A file under `asset_sync` that is none of the above.
    OtherAsset,
    Other,
}

/// Where a file sits in the pack, read from its path.
#[derive(Debug, Clone, Serialize)]
pub struct Located {
    pub kind: FileKind,
    /// The namespace folder it is in (`pleasurehorizons`), when it is under `data/`.
    pub namespace: Option<String>,
    /// The file name without its ending.
    pub id: Option<String>,
}

/// Tells what a file is from its path relative to the pack root (written with `/`).
pub fn locate(rel: &str) -> Located {
    let parts: Vec<&str> = rel.split('/').collect();
    let name = parts.last().copied().unwrap_or("");
    let stem = name.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(name).to_string();
    let ext = name.rsplit_once('.').map(|(_, ext)| ext.to_ascii_lowercase()).unwrap_or_default();

    let plain = |kind: FileKind| Located { kind, namespace: None, id: None };

    if parts.len() == 1 {
        return plain(if name == "pack.mcmeta" { FileKind::PackMeta } else { FileKind::Other });
    }

    if parts[0] != "data" || parts.len() < 4 {
        return plain(FileKind::Other);
    }

    let namespace = Some(parts[1].to_string());
    let section = parts[2];
    let located = |kind: FileKind| Located { kind, namespace: namespace.clone(), id: Some(stem.clone()) };

    match section {
        "girls" if ext == "json" => located(FileKind::Girl),
        "scenes" if ext == "json" => located(FileKind::Scene),
        "girl_traits" if ext == "json" => located(FileKind::Trait),
        "girl_ai" if ext == "json" => located(FileKind::GirlAi),
        "dialogue" | "barks" if ext == "json" => located(FileKind::Dialogue),
        "social" if ext == "json" => located(FileKind::Social),
        "loot_table" | "loot_tables" if ext == "json" => located(FileKind::LootTable),
        ASSET_SYNC => located(asset_kind(&parts[3..], &ext, name)),
        _ => located(FileKind::OtherData),
    }
}

fn asset_kind(rest: &[&str], ext: &str, name: &str) -> FileKind {
    let inside = |folder: &str| rest.first() == Some(&folder);

    match ext {
        "strutmodel" => FileKind::Model,
        "strutanim" => FileKind::Animation,
        "png" => FileKind::Texture,
        "ogg" => FileKind::Sound,
        "lang" => FileKind::Lang,
        _ if name == "sounds.json" => FileKind::SoundsJson,
        "json" if rest.starts_with(&["lang"]) => FileKind::Lang,
        "json" if rest.starts_with(&["strut", "event_bindings"]) => FileKind::EventBindings,
        "json" if inside("keyframe_events") => FileKind::KeyframeEvents,
        "json" if rest.starts_with(&["strut", "physics"]) => FileKind::Physics,
        "json" if rest.starts_with(&["strut", "armour", "wearers"]) => FileKind::ArmourWearer,
        "json" if rest.starts_with(&["strut", "armour", "sets"]) => FileKind::ArmourSet,
        "json" if rest.starts_with(&["strut", "armour", "looks"]) => FileKind::ArmourLook,
        _ => FileKind::OtherAsset,
    }
}

/// The folder a kind of file goes in, inside `data/<namespace>/`, for a new file of that kind.
pub fn folder_for(kind: &FileKind) -> Option<&'static str> {
    match kind {
        FileKind::Girl => Some("girls"),
        FileKind::Scene => Some("scenes"),
        FileKind::Trait => Some("girl_traits"),
        FileKind::GirlAi => Some("girl_ai"),
        FileKind::Dialogue => Some("dialogue"),
        FileKind::Social => Some("social"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knows_the_content_of_a_pack() {
        assert_eq!(locate("pack.mcmeta").kind, FileKind::PackMeta);
        assert_eq!(locate("data/mypack/girls/amber.json").kind, FileKind::Girl);
        assert_eq!(locate("data/mypack/girls/amber.json").id.as_deref(), Some("amber"));
        assert_eq!(locate("data/pleasurehorizons/scenes/doggy.json").kind, FileKind::Scene);
        assert_eq!(locate("data/pleasurehorizons/girl_traits/shy.json").kind, FileKind::Trait);
        assert_eq!(locate("data/x/dialogue/chat.json").kind, FileKind::Dialogue);
        assert_eq!(locate("data/x/social/chat.json").kind, FileKind::Social);
    }

    #[test]
    fn knows_what_is_sent_to_players() {
        let base = "data/pleasurehorizons/asset_sync/";

        assert_eq!(locate(&format!("{base}strut/models/entities/girls/nude/amber.strutmodel")).kind, FileKind::Model);
        assert_eq!(locate(&format!("{base}strut/animations/entities/girls/base.amber.strutanim")).kind, FileKind::Animation);
        assert_eq!(locate(&format!("{base}textures/entities/amber.png")).kind, FileKind::Texture);
        assert_eq!(locate(&format!("{base}sounds/amber/moan1.ogg")).kind, FileKind::Sound);
        assert_eq!(locate(&format!("{base}sounds.json")).kind, FileKind::SoundsJson);
        assert_eq!(locate(&format!("{base}lang/en_us.json")).kind, FileKind::Lang);
        assert_eq!(locate(&format!("{base}strut/event_bindings/girls/amber.json")).kind, FileKind::EventBindings);
        assert_eq!(locate(&format!("{base}keyframe_events/amber.json")).kind, FileKind::KeyframeEvents);
        assert_eq!(locate(&format!("{base}strut/physics/models/entities/girls/nude/amber.json")).kind, FileKind::Physics);
    }

    #[test]
    fn anything_else_is_other() {
        assert_eq!(locate("readme.txt").kind, FileKind::Other);
        assert_eq!(locate("data/x").kind, FileKind::Other);
    }
}
