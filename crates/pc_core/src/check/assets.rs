//! The files a pack sends to players. Strut's asset sync refuses anything that breaks its rules, and says so only in the server log, so the pack is held to them here.

use crate::diagnostics::{file_level, Diagnostic, Severity};
use crate::layout::{locate, FileKind, ASSET_SYNC, GAME_NAMESPACE};

const ALLOWED: [&str; 8] = [".png", ".ogg", ".json", ".mcmeta", ".strutmodel", ".strutanim", ".lang", ".txt"];
const RESERVED: [&str; 5] = ["minecraft", "realms", "fabric", "fabricloader", "c"];
const MAX_PATH: usize = 256;
const MAX_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PNG_SIDE: u32 = 8192;

/// Checks one file under `data/<namespace>/asset_sync/`. `rel` is its path in the pack, `size` its size, `head` its first bytes (enough for a PNG's size).
pub fn check(rel: &str, size: u64, head: &[u8]) -> Vec<Diagnostic> {
    let mut found = Vec::new();
    let parts: Vec<&str> = rel.split('/').collect();

    if parts.len() < 5 || parts[0] != "data" || parts[2] != ASSET_SYNC {
        return found;
    }

    let namespace = parts[1];
    let synced = format!("{namespace}/{}", parts[3..].join("/"));
    let mut problem = |severity: Severity, code: &str, message: String| found.push(file_level(severity, code, message, rel));

    if RESERVED.contains(&namespace) {
        problem(Severity::Error, "reserved-namespace", format!("\"{namespace}\" cannot hold synced files: the game refuses to send anything into minecraft, realms, fabric, fabricloader or c."));
        return found;
    }

    if synced.len() > MAX_PATH {
        problem(Severity::Error, "path-too-long", format!("The path is {} characters; asset sync accepts {MAX_PATH} at most.", synced.len()));
    }

    for segment in synced.split('/') {
        if segment.is_empty() || !segment.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '.' | '-')) {
            /* A scene file for a skeleton written `scene{skeleton}.strutanim` is a natural thing to write, and the game cannot load a name with braces in it. */
            let hint = match segment.strip_suffix(".strutanim").and_then(|stem| stem.strip_suffix('}')).and_then(|stem| stem.split_once('{')) {
                Some((scene, skeleton)) if !scene.is_empty() && !skeleton.is_empty() => format!(" For a scene file that every girl on one skeleton plays, name it \"{scene}.{skeleton}.strutanim\"."),
                _ => String::new(),
            };

            problem(Severity::Error, "bad-asset-path", format!("\"{segment}\" cannot be sent: asset paths use lower case letters, digits and _ . - only (and / between folders).{hint}"));
        }
    }

    let name = parts[parts.len() - 1].to_ascii_lowercase();

    if !ALLOWED.iter().any(|ext| name.ends_with(ext)) {
        problem(Severity::Warning, "not-synced", "Asset sync skips this kind of file (it sends png, ogg, json, mcmeta, strutmodel, strutanim, lang and txt only).".into());
    }

    if name.ends_with(".bbmodel") || name.ends_with(".geo.json") || (name.ends_with(".animation.json") && parts.contains(&"strut")) {
        problem(Severity::Error, "needs-conversion", "Strut sends binary models and animations only (.strutmodel, .strutanim). Convert this file with Strut's converter.".into());
    }

    if size > MAX_BYTES {
        problem(Severity::Error, "file-too-large", format!("The file is {} MiB; asset sync accepts {} MiB at most.", size / 1024 / 1024, MAX_BYTES / 1024 / 1024));
    }

    if name.ends_with(".png") {
        match png_size(head) {
            Some((w, h)) if w > MAX_PNG_SIDE || h > MAX_PNG_SIDE => problem(Severity::Error, "png-too-large", format!("The picture is {w} x {h}; asset sync refuses a PNG over {MAX_PNG_SIDE} x {MAX_PNG_SIDE}.")),
            Some(_) => {}
            None => problem(Severity::Error, "not-a-png", "This does not look like a PNG picture.".into()),
        }
    }

    if namespace != GAME_NAMESPACE && matches!(locate(rel).kind, FileKind::Model | FileKind::Animation | FileKind::Texture | FileKind::EventBindings | FileKind::KeyframeEvents) {
        problem(Severity::Warning, "wrong-asset-namespace", format!("The game looks for a girl's models, textures, animations and cues in \"{GAME_NAMESPACE}\", not \"{namespace}\"."));
    }

    found
}

/// The width and height a PNG says it has, or None when the bytes are not a PNG.
fn png_size(head: &[u8]) -> Option<(u32, u32)> {
    const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

    if head.len() < 24 || head[..8] != SIGNATURE {
        return None;
    }

    Some((u32::from_be_bytes(head[16..20].try_into().ok()?), u32::from_be_bytes(head[20..24].try_into().ok()?)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13, b'I', b'H', b'D', b'R'];

        bytes.extend(w.to_be_bytes());
        bytes.extend(h.to_be_bytes());
        bytes
    }

    #[test]
    fn a_good_file_has_no_problems() {
        assert!(check("data/pleasurehorizons/asset_sync/textures/entities/amber.png", 1000, &png(64, 64)).is_empty());
        assert!(check("data/pleasurehorizons/asset_sync/strut/models/entities/girls/nude/amber.strutmodel", 1000, &[]).is_empty());
    }

    #[test]
    fn a_scene_file_named_with_braces_is_pointed_at_the_name_that_works() {
        let found = check("data/dnd/asset_sync/strut/animations/scenes/cowgirl{bia}.strutanim", 10, &[]);
        let message = found.iter().find(|d| d.code == "bad-asset-path").map(|d| d.message.clone()).unwrap_or_default();

        assert!(message.contains("cowgirl.bia.strutanim"), "{message}");
        assert!(check("data/dnd/asset_sync/strut/animations/scenes/cowgirl.bia.strutanim", 10, &[]).iter().all(|d| d.code != "bad-asset-path"));
    }

    #[test]
    fn breaks_the_rules_of_asset_sync() {
        let codes = |rel: &str, size: u64, head: &[u8]| check(rel, size, head).into_iter().map(|d| d.code).collect::<Vec<_>>();

        assert_eq!(codes("data/minecraft/asset_sync/textures/a.png", 10, &png(1, 1)), vec!["reserved-namespace"]);
        assert!(codes("data/pleasurehorizons/asset_sync/textures/Amber.png", 10, &png(1, 1)).contains(&"bad-asset-path".to_string()));
        assert!(codes("data/pleasurehorizons/asset_sync/textures/a.png", 10, &png(9000, 10)).contains(&"png-too-large".to_string()));
        assert!(codes("data/pleasurehorizons/asset_sync/textures/a.png", 10, b"nope").contains(&"not-a-png".to_string()));
        assert!(codes("data/pleasurehorizons/asset_sync/strut/models/a.bbmodel", 10, &[]).contains(&"needs-conversion".to_string()));
        assert!(codes("data/pleasurehorizons/asset_sync/strut/animations/a.strutanim", 70 * 1024 * 1024, &[]).contains(&"file-too-large".to_string()));
        assert!(codes("data/mypack/asset_sync/textures/entities/a.png", 10, &png(1, 1)).contains(&"wrong-asset-namespace".to_string()));
        assert!(codes("data/pleasurehorizons/asset_sync/shaders/a.fsh", 10, &[]).contains(&"not-synced".to_string()));
    }
}
