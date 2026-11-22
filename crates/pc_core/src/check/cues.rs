//! Cues and sounds. An animation fires cues (the parts of its markers, such as `pound` and `doggymoan` in `pound-doggymoan`); a girl's event bindings and keyframe events
//! say what each cue plays (a sound, a chat line); `sounds.json` says what each sound is. These checks find the places where the three stop matching: a sound that does
//! not exist, a cue nothing fires (usually a misspelt one), a sound file that is not in the pack.

use super::values::quoted;
use super::PackFacts;
use crate::diagnostics::{file_level, Diagnostic, Severity, Sink};
use crate::json_ast::{Node, Value};
use crate::layout::{locate, FileKind};
use crate::reference::{nearest, Reference};
use crate::strut_anim;

/// The cue names in a marker: `pound-doggymoan` is two, `pound` and `doggymoan`. Cues are not case sensitive.
pub fn cues_of(marker: &str) -> Vec<String> {
    marker.to_lowercase().split('-').filter(|part| !part.is_empty()).map(String::from).collect()
}

/// The sound events a `sounds.json` defines, as full ids (`pleasurehorizons:mika.moan`), for the namespace of the folder it is in.
pub fn sound_events_of(namespace: &str, root: &Node) -> Vec<String> {
    root.as_object().map(|members| members.iter().map(|m| format!("{namespace}:{}", m.key)).collect()).unwrap_or_default()
}

/// A sound id as the game reads it: without a namespace it is Minecraft's.
fn qualified_sound(id: &str) -> String {
    if id.contains(':') { id.to_string() } else { format!("minecraft:{id}") }
}

fn sound_ref(sink: &mut Sink, node: &Node, reference: &Reference, facts: &PackFacts) {
    let Some(id) = node.as_str() else {
        sink.error("wrong-type", format!("A sound is its id (text), not {}.", node.describe()), node.start, node.end);
        return;
    };

    let full = qualified_sound(id);

    if full.starts_with("minecraft:") || reference.sound_events.contains(&full) || facts.sound_events.contains(&full) {
        return;
    }

    let known = reference.sound_events.iter().chain(facts.sound_events.iter()).map(String::as_str);
    let fix = nearest(&full, known);
    let message = format!("There is no sound \"{id}\": it is not one of the game's sounds, and no sounds.json in this pack defines it.{}", fix.as_ref().map(|f| format!(" Did you mean \"{f}\"?")).unwrap_or_default());

    match fix {
        Some(fix) => sink.error_with_fix("unknown-sound", message, node.start, node.end, format!("Change to \"{fix}\""), quoted(&fix)),
        None => sink.error("unknown-sound", message, node.start, node.end),
    }
}

fn sound_list(sink: &mut Sink, node: &Node, reference: &Reference, facts: &PackFacts) {
    match node.as_array() {
        Some(items) => items.iter().for_each(|item| sound_ref(sink, item, reference, facts)),
        None => sink.error("wrong-type", "This must be a list of sound ids.", node.start, node.end),
    }
}

/// A cue that no animation fires never plays. The nearest known cue is offered, because it is nearly always a typing mistake.
fn cue_key(sink: &mut Sink, text: &str, start: usize, end: usize, reference: &Reference, facts: &PackFacts) {
    let lower = text.to_lowercase();

    /* A key may name several cues with dashes, as a marker does. */
    for cue in cues_of(&lower) {
        if reference.known_cues.contains(&cue) || facts.cues.contains(&cue) {
            continue;
        }

        let known = reference.known_cues.iter().chain(facts.cues.iter()).map(String::as_str);
        let fix = nearest(&cue, known);
        let message = format!("No animation fires the cue \"{cue}\" (neither the game's nor this pack's), so this never plays.{}", fix.as_ref().map(|f| format!(" Did you mean \"{f}\"?")).unwrap_or_default());

        match fix {
            Some(fix) if lower == cue => sink.report(Severity::Warning, "unknown-cue", message, start, end, Some(crate::diagnostics::QuickFix { title: format!("Change to \"{fix}\""), replacement: quoted(&fix) })),
            _ => sink.warn("unknown-cue", message, start, end),
        }
    }
}

fn string_list(sink: &mut Sink, node: &Node, what: &str) {
    match node.as_array() {
        Some(items) => {
            for item in items {
                if item.as_str().is_none() {
                    sink.error("wrong-type", format!("Every entry of {what} must be text."), item.start, item.end);
                }
            }
        }
        None => sink.error("wrong-type", format!("{what} must be a list of text."), node.start, node.end),
    }
}

/// `strut/event_bindings/girls/<id>.json`: `{ "bindings": { "<cue>": { "sounds": [], "random_sounds": [], "pleasurehorizons:messages": [] } } }`.
pub fn bindings(sink: &mut Sink, root: &Node, reference: &Reference, facts: &PackFacts) {
    let Some(table) = root.get("bindings") else {
        sink.error("missing-field", "Add \"bindings\": what each cue plays.", root.start, root.start + 1);
        return;
    };

    let Some(entries) = table.value.as_object() else {
        sink.error("wrong-type", "\"bindings\" must be an object with one entry for each cue.", table.value.start, table.value.end);
        return;
    };

    for member in root.as_object().into_iter().flatten().filter(|m| m.key != "bindings") {
        sink.warn("unknown-key", format!("\"{}\" is ignored. An event bindings file has \"bindings\".", member.key), member.key_start, member.key_end);
    }

    for entry in entries {
        cue_key(sink, &entry.key, entry.key_start, entry.key_end, reference, facts);

        let Some(body) = entry.value.as_object() else {
            sink.error("wrong-type", format!("The cue \"{}\" must be an object ({{ \"sounds\": [...] }}).", entry.key), entry.value.start, entry.value.end);
            continue;
        };

        for member in body {
            match member.key.as_str() {
                "sounds" | "random_sounds" => sound_list(sink, &member.value, reference, facts),
                key if key.contains(':') && key.ends_with("messages") => string_list(sink, &member.value, "the messages"),
                /* Any other namespaced key is an action another mod or Strut provides (strut_physics:fluid): its settings are its own. */
                key if key.contains(':') => {}
                other => sink.warn("unknown-key", format!("\"{other}\" is ignored. A cue has sounds, random_sounds, pleasurehorizons:messages and namespaced actions."), member.key_start, member.key_end),
            }
        }
    }
}

/// `keyframe_events/<id>.json` is no longer read by the game: a cue's sounds and chat lines are written in the girl's event bindings.
pub fn old_keyframe_events(sink: &mut Sink, root: &Node) {
    sink.warn(
        "keyframe-events-unused",
        "The game no longer reads keyframe_events files. Put each cue's sounds and chat lines in the girl's event bindings (strut/event_bindings/characters/<girl>.json): \"sounds\", \"random_sounds\" and \"pleasurehorizons:messages\" (the lines themselves).",
        root.start,
        root.start + 1,
    );
}

/// `sounds.json`: each sound event and the files it plays. A file the pack does not have is a mistake unless the game already has it.
pub fn sounds_json(sink: &mut Sink, root: &Node, namespace: &str, facts: &PackFacts) {
    let Some(events) = root.as_object() else {
        sink.error("wrong-type", "sounds.json is an object with one entry for each sound.", root.start, root.end);
        return;
    };

    for event in events {
        let Some(sounds) = event.value.get("sounds") else {
            sink.error("missing-field", format!("The sound \"{}\" needs \"sounds\": the files it plays.", event.key), event.value.start, event.value.start + 1);
            continue;
        };

        let Some(list) = sounds.value.as_array() else {
            sink.error("wrong-type", "\"sounds\" must be a list.", sounds.value.start, sounds.value.end);
            continue;
        };

        for item in list {
            let name = match &item.value {
                Value::Str(name) => Some((name.clone(), item)),
                Value::Object(_) => item.get("name").and_then(|m| m.value.as_str().map(|n| (n.to_string(), &m.value))),
                _ => None,
            };

            let Some((name, at)) = name else {
                sink.error("wrong-type", "A sound is a file name or an object with a \"name\".", item.start, item.end);
                continue;
            };

            let (file_ns, path) = name.split_once(':').unwrap_or((namespace, name.as_str()));

            if file_ns == "minecraft" {
                continue;
            }

            if !facts.files.contains(&format!("data/{file_ns}/asset_sync/sounds/{path}.ogg")) {
                sink.warn("missing-sound-file", format!("The pack has no file data/{file_ns}/asset_sync/sounds/{path}.ogg for this sound. That is fine if the game already has it."), at.start, at.end);
            }
        }
    }
}

/// The problems of an animation file, and the cues it fires.
pub fn animation(rel: &str, bytes: &[u8]) -> (Vec<Diagnostic>, Vec<String>) {
    let clips = match strut_anim::read(bytes) {
        Ok(clips) => clips,
        Err(message) => return (vec![file_level(Severity::Error, "bad-animation", format!("This animation cannot be read: {message}."), rel)], Vec::new()),
    };

    let located = locate(rel);
    let stem = located.id.clone().unwrap_or_default();
    let in_scenes = rel.contains("/strut/animations/scenes/");

    /* A scene file's clips are animation.<scene>.<role>. A base file's names depend on the body that plays it (a girl's are animation.base.<name>, another entity's are its
       own), so only scene files are held to a name. */
    let expected = if in_scenes { Some(format!("animation.{}.", stem.split('-').next().unwrap_or(&stem))) } else { None };
    let mut found = Vec::new();
    let mut cues = Vec::new();

    for clip in &clips {
        if let Some(expected) = &expected {
            if !clip.name.starts_with(expected.as_str()) {
                found.push(file_level(Severity::Warning, "clip-name", format!("The clip \"{}\" should be named {expected}<name>, or the game will not find it.", clip.name), rel));
            }
        }

        for event in &clip.events {
            if event.kind == "instruction" {
                cues.extend(cues_of(&event.payload));
            }

            /* A sound event is `name`, `name, volume` or `name, volume, pitch`: what follows the name has to be a number. */
            if event.kind == "sound" {
                let parts: Vec<&str> = event.payload.split(',').map(str::trim).collect();

                for extra in parts.iter().skip(1).filter(|p| p.parse::<f32>().is_err()) {
                    found.push(file_level(Severity::Warning, "sound-event-number", format!("In the clip {} at {:.2} s, the sound event \"{}\" has \"{extra}\" where a volume or pitch (a number) belongs. The game cannot read it. If it is a second sound, make it an event of its own.", clip.name, event.time, event.payload), rel));
                }
            }
        }
    }

    if clips.is_empty() {
        found.push(file_level(Severity::Warning, "no-clips", "This animation file has no clips.", rel));
    }

    cues.sort();
    cues.dedup();
    (found, cues)
}

pub fn is_cue_file(kind: &FileKind) -> bool {
    matches!(kind, FileKind::EventBindings | FileKind::KeyframeEvents | FileKind::SoundsJson)
}
