use super::*;
use crate::diagnostics::Severity;
use std::path::PathBuf;

fn reference() -> Reference {
    Reference::builtin()
}

fn codes(rel: &str, text: &str) -> Vec<String> {
    check_text(rel, text, &reference(), &PackFacts::default()).into_iter().filter(|d| d.severity == Severity::Error).map(|d| d.code).collect()
}

const GIRL: &str = "data/pleasurehorizons/girls/amber.json";

#[test]
fn a_good_girl_has_no_errors() {
    let text = r##"{ "id": "amber", "type": "settler", "components": {
        "pleasurehorizons:identity": { "name": "Amber", "egg_primary": "#FFAA00", "egg_secondary": "#000000" },
        "pleasurehorizons:body": { "body_type": "jenny", "hitbox_height": 1.8, "held_item_offset": [0, 1, 0] },
        "pleasurehorizons:taste": { "tame_item": "minecraft:allium" } } }"##;

    assert_eq!(codes(GIRL, text), Vec::<String>::new());
}

#[test]
fn finds_the_mistakes_in_a_girl_and_says_where() {
    let text = r#"{ "id": "amber", "type": "settlr", "components": {
        "pleasurehorizons:body": { "body_type": "jeny", "hitbox_height": 9, "held_item_offset": [1, 2] },
        "pleasurehorizons:bodyy": {} } }"#;
    let found = check_text(GIRL, text, &reference(), &PackFacts::default());
    let has = |code: &str| found.iter().any(|d| d.code == code);

    assert!(has("unknown-type"));
    assert!(has("bad-choice"));
    assert!(has("too-large"));
    assert!(has("bad-vec3"));
    assert!(has("unknown-component"));
    assert!(!has("missing-component"), "a girl of an unknown kind is not asked for components");

    let wild = check_text(GIRL, r#"{ "id": "amber", "type": "wild", "components": {} }"#, &reference(), &PackFacts::default());

    assert_eq!(wild.iter().filter(|d| d.code == "missing-component").count(), 3);

    let kind = found.iter().find(|d| d.code == "unknown-type").unwrap();

    assert_eq!((kind.line, kind.column), (1, 26));
    assert_eq!(kind.fix.as_ref().unwrap().replacement, "\"settler\"");
}

#[test]
fn a_trait_can_be_misspelt_or_in_the_wrong_category() {
    let wrong = |traits: &str| {
        let text = format!(r##"{{ "id": "amber", "type": "wild", "components": {{
            "pleasurehorizons:identity": {{ "name": "A" }}, "pleasurehorizons:body": {{ "body_type": "jenny" }}, "pleasurehorizons:taste": {{ "tame_item": "minecraft:allium" }},
            "pleasurehorizons:ai": {{ "traits": {traits} }} }} }}"##);

        codes(GIRL, &text)
    };

    assert_eq!(wrong(r#"{ "movement": "walker" }"#), Vec::<String>::new());
    assert_eq!(wrong(r#"{ "movement": "walkr" }"#), vec!["unknown-trait"]);
    assert_eq!(wrong(r#"{ "movement": "hostile" }"#), vec!["wrong-category"]);
    assert_eq!(wrong(r#"{ "moovement": "walker" }"#), vec!["unknown-field"]);
    assert_eq!(wrong(r#"{ "movement": { "type": "walker", "speed": 9 } }"#), vec!["too-large"]);
    assert_eq!(wrong(r#"{ "movement": { "type": "walker", "spede": 1 } }"#), vec!["unknown-field"]);
    assert_eq!(wrong(r#"{ "interaction": ["collar", "curious"] }"#), Vec::<String>::new());
    assert_eq!(wrong(r#"{ "movement": ["walker"] }"#), vec!["wrong-type"]);
}

#[test]
fn a_traits_of_the_pack_are_known() {
    let mut facts = PackFacts::default();

    facts.traits.insert("mypack:shy_fox".into(), vec!["distance".into()]);

    let text = r##"{ "id": "amber", "type": "wild", "components": {
        "pleasurehorizons:identity": { "name": "A" }, "pleasurehorizons:body": { "body_type": "jenny" }, "pleasurehorizons:taste": { "tame_item": "minecraft:allium" },
        "pleasurehorizons:ai": { "traits": { "temperament": { "type": "mypack:shy_fox", "distance": 4 } } } } }"##;

    assert!(check_text(GIRL, text, &reference(), &facts).iter().all(|d| d.severity != Severity::Error));
}

#[test]
fn syntax_errors_are_reported_with_their_place() {
    let found = check_text(GIRL, "{\n  \"id\": \"amber\",\n}", &reference(), &PackFacts::default());

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].code, "syntax");
    assert_eq!(found[0].line, 3);
}

#[test]
fn a_scene_is_checked_against_the_scene_settings() {
    assert_eq!(codes("data/x/scenes/s.json", r#"{ "type": "on_bed", "body_types": ["ellie"], "tags": ["kind"], "requires_affection": 8 }"#), Vec::<String>::new());
    assert_eq!(codes("data/x/scenes/s.json", r#"{ "type": "on_bd" }"#), vec!["bad-choice"]);
    assert_eq!(codes("data/x/scenes/s.json", r#"{ "body_types": ["jeny"] }"#), vec!["bad-choice"]);
    assert_eq!(codes("data/x/scenes/s.json", r#"{ "requires_affection": 11 }"#), vec!["too-large"]);
    assert_eq!(codes("data/x/scenes/s.json", r#"{ "supported_girls": ["a"], "unsupported_girls": ["b"] }"#), vec!["conflicting-fields", "conflicting-fields"]);
    assert_eq!(codes("data/x/scenes/s.json", r#"{ "girl_overrides": { "lucy": { "requires_affection": 99 } } }"#), vec!["too-large"]);
}

#[test]
fn a_trait_file_is_checked_against_the_goals_the_game_has() {
    let good = r#"{ "category": "movement", "params": { "speed": { "type": "number", "default": 1 } },
        "goals": [ { "priority": 7, "goal": "wander", "when": "not_sleeping", "speed": "$speed" } ], "capabilities": { "movement": "ground" } }"#;

    assert_eq!(codes("data/x/girl_traits/t.json", good), Vec::<String>::new());
    assert_eq!(codes("data/x/girl_traits/t.json", r#"{ "category": "movement", "goals": [ { "priority": 1, "goal": "wandr" } ] }"#), vec!["unknown-goal"]);
    assert_eq!(codes("data/x/girl_traits/t.json", r#"{ "category": "movement", "goals": [ { "priority": 1, "goal": "wander", "speed": "$nope" } ] }"#), vec!["unknown-setting"]);
    assert_eq!(codes("data/x/girl_traits/t.json", r#"{ "category": "movment" }"#), vec!["bad-choice"]);
    assert_eq!(codes("data/x/girl_traits/t.json", r#"{ "category": "movement", "capabilities": { "flys": true } }"#), vec!["unknown-field"]);
    assert_eq!(codes("data/x/girl_traits/t.json", r#"{ "category": "movement", "goals": [ { "goal": "wander" } ] }"#), vec!["missing-field"]);
}

#[test]
fn dialogue_lines_are_checked() {
    let rel = "data/x/dialogue/d.json";
    let run = |text: &str| codes(rel, text);
    let line = |extra: &str| format!(r#"[ {{ "id": "hello", "topic": "chat", "player_line": "Hi", "reply_good": "Hi.", "reply_bad": "No." {extra} }} ]"#);

    assert_eq!(run(&line("")), Vec::<String>::new());
    assert_eq!(run(&line(r#", "tags": ["kind", "romantic"], "requires_affection": 5, "weight": 2, "once": true"#)), Vec::<String>::new());
    assert_eq!(run(&line(r#", "tags": ["kindd"]"#)), vec!["bad-choice"]);
    assert_eq!(run(&line(r#", "requires_affection": -1"#)), vec!["too-small"]);
    assert_eq!(run(&line(r#", "requires_said": "about_past""#)), Vec::<String>::new());
    assert_eq!(run(&line(r#", "requires_said": "nothing_like_it""#)), vec!["unknown-line"]);
    assert_eq!(run(&line(r#", "wieght": 2"#)), vec!["unknown-field"]);
    assert_eq!(run(r#"[ { "id": "a", "topic": "chatt", "player_line": "x", "reply_good": "y" } ]"#), vec!["bad-choice"]);
    assert_eq!(run(r#"[ { "id": "a", "topic": "chat", "player_line": "x", "answers": [ { "text": "t", "tags": ["bold"], "reply_good": "g" } ] } ]"#), vec!["missing-field"]);
    assert_eq!(run(r#"[ { "id": "a", "topic": "chat", "player_line": "x", "reply_good": "g" }, { "id": "a", "topic": "chat", "player_line": "y", "reply_good": "g" } ]"#), vec!["duplicate-id"]);
}

#[test]
fn the_game_s_own_files_pass() {
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../src/main/resources/data/pleasurehorizons");

    if !data.exists() {
        return;
    }

    let reference = reference();
    let mut facts = PackFacts::default();

    for rel in walk(&data) {
        if rel.starts_with("girl_traits/") {
            let name = rel.trim_start_matches("girl_traits/").trim_end_matches(".json");

            facts.traits.insert(format!("pleasurehorizons:{name}"), Vec::new());
        }
    }

    let mut problems = Vec::new();

    for rel in walk(&data) {
        let kind_folder = rel.split('/').next().unwrap_or("");

        if !matches!(kind_folder, "girls" | "scenes" | "girl_traits" | "dialogue" | "social") || !rel.ends_with(".json") {
            continue;
        }

        let text = std::fs::read_to_string(data.join(&rel)).unwrap();
        let as_pack = format!("data/pleasurehorizons/{rel}");

        for d in check_text(&as_pack, &text, &reference, &facts) {
            if d.severity == Severity::Error {
                problems.push(format!("{} {}:{} {} ({})", d.file, d.line, d.column, d.message, d.code));
            }
        }
    }

    assert!(problems.is_empty(), "the game's own files have problems:\n{}", problems.join("\n"));
}

#[test]
fn a_project_is_checked_as_a_whole() {
    let dir = tempfile::tempdir().unwrap();
    let write = |rel: &str, text: &str| {
        let path = dir.path().join(rel);

        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    };

    write("data/pleasurehorizons/girls/amber.json", r#"{ "id": "amber", "type": "wild", "components": { "pleasurehorizons:identity": {"name":"A"}, "pleasurehorizons:body": {"body_type":"jenny"}, "pleasurehorizons:taste": {"tame_item":"minecraft:allium"} } }"#);

    let found = check_project(dir.path(), &reference());
    let has = |code: &str| found.iter().any(|d| d.code == code);

    assert!(has("missing-mcmeta"));
    assert!(has("girl-without-files"));

    write("pack.mcmeta", r#"{ "pack": { "pack_format": 48, "description": "x" } }"#);
    write("data/pleasurehorizons/asset_sync/textures/entities/amber.png", "not a png");

    let found = check_project(dir.path(), &reference());
    let has = |code: &str| found.iter().any(|d| d.code == code);

    assert!(!has("missing-mcmeta"));
    assert!(has("girl-files-missing"));
    assert!(has("not-a-png"));
}

/// The game's own assets, laid out as a pack sends them (`assets/<ns>/x` is `data/<ns>/asset_sync/x`), must pass the cue, sound and animation checks.
#[test]
fn the_games_own_cues_and_sounds_pass() {
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../src/main/resources/assets/pleasurehorizons");

    if !assets.exists() {
        return;
    }

    let reference = reference();
    let mut facts = PackFacts::default();
    let mut files: Vec<(String, PathBuf)> = Vec::new();

    for rel in walk(&assets) {
        let as_pack = format!("data/pleasurehorizons/asset_sync/{rel}");

        facts.files.insert(as_pack.clone());
        files.push((as_pack, assets.join(&rel)));
    }

    for (rel, path) in &files {
        match locate(rel).kind {
            FileKind::SoundsJson => {
                let node = crate::json_ast::parse(&std::fs::read_to_string(path).unwrap()).unwrap();

                facts.sound_events.extend(cues::sound_events_of("pleasurehorizons", &node));
            }
            FileKind::Animation => facts.cues.extend(cues::animation(rel, &std::fs::read(path).unwrap()).1),
            _ => {}
        }
    }

    let mut problems = Vec::new();

    for (rel, path) in &files {
        let kind = locate(rel).kind;
        let found = match kind {
            FileKind::EventBindings | FileKind::KeyframeEvents | FileKind::SoundsJson => check_text(rel, &std::fs::read_to_string(path).unwrap(), &reference, &facts),
            FileKind::Animation => cues::animation(rel, &std::fs::read(path).unwrap()).0,
            _ => continue,
        };

        for d in found {
            /* Cues nothing fires are reported, not failed: the game ships a few (see the test's output). The example file is an example. */
            if d.code == "unknown-cue" {
                println!("FINDING {} {}:{} {}", d.file, d.line, d.column, d.message);
                continue;
            }

            if rel.ends_with("/example.json") {
                continue;
            }

            if d.severity == Severity::Error || d.code == "clip-name" || d.code == "bad-animation" {
                problems.push(format!("{} {}:{} {} ({})", d.file, d.line, d.column, d.message, d.code));
            }
        }
    }

    assert!(problems.is_empty(), "the game's own cues and sounds have problems:\n{}", problems.iter().take(30).cloned().collect::<Vec<_>>().join("\n"));
}

#[test]
fn cues_and_sounds_are_checked_against_what_exists() {
    let mut facts = PackFacts::default();

    facts.cues.insert("mycue".into());
    facts.sound_events.insert("pleasurehorizons:amber.moan".into());
    facts.files.insert("data/pleasurehorizons/asset_sync/sounds/amber/moan0.ogg".into());

    let rel = "data/pleasurehorizons/asset_sync/strut/event_bindings/girls/amber.json";
    let run = |text: &str| check_text(rel, text, &reference(), &facts).into_iter().map(|d| (d.code, d.fix.map(|f| f.replacement))).collect::<Vec<_>>();

    assert!(run(r#"{ "bindings": { "mycue": { "sounds": ["pleasurehorizons:amber.moan"] }, "thrust": {} } }"#).is_empty());

    let typo = run(r#"{ "bindings": { "trhust": { "random_sounds": ["pleasurehorizons:amber.mon"] } } }"#);

    assert!(typo.contains(&("unknown-cue".to_string(), Some("\"thrust\"".to_string()))));
    assert!(typo.contains(&("unknown-sound".to_string(), Some("\"pleasurehorizons:amber.moan\"".to_string()))));
    assert_eq!(run(r#"{ "bindings": { "thrust": { "sounds": ["minecraft:entity.cat.ambient", "ambient"] } } }"#), vec![]);
    assert_eq!(run(r#"{ "nothing": {} }"#).iter().map(|d| d.0.as_str()).collect::<Vec<_>>(), vec!["missing-field"]);

    let sounds = check_text("data/pleasurehorizons/asset_sync/sounds.json", r#"{ "amber.moan": { "sounds": [ { "name": "pleasurehorizons:amber/moan0" }, "pleasurehorizons:amber/gone" ] } }"#, &reference(), &facts);

    assert_eq!(sounds.iter().map(|d| d.code.as_str()).collect::<Vec<_>>(), vec!["missing-sound-file"]);
}

#[test]
fn an_animation_that_cannot_be_read_is_reported() {
    let (found, cues) = cues::animation("data/pleasurehorizons/asset_sync/strut/animations/entities/girls/base.amber.strutanim", b"not an animation at all, just text");

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].code, "bad-animation");
    assert!(cues.is_empty());
    assert_eq!(cues::cues_of("Pound-DoggyMoan"), vec!["pound", "doggymoan"]);
}

#[test]
fn small_talk_entries_are_checked() {
    let rel = "data/x/social/s.json";
    let good = r#"[ { "id": "a", "kind": "line", "text": "Hello {partner}.", "outcome": ["warm", "easy"], "job": "farmer", "weather": "rain", "interest": "food", "role": "starter", "weight": 2 } ]"#;

    assert_eq!(codes(rel, good), Vec::<String>::new());

    let bad = r#"[ { "kind": "chat", "text": "", "outcome": ["warmm"], "job": ["farmr"], "time": "noon", "weather": "snow", "interest": "cheese", "role": "first", "weight": 0, "colour": 1 }, { "text": "Hi", "mood": "grumpy" }, { "kind": "line" } ]"#;
    let found = check_text(rel, bad, &reference(), &PackFacts::default());
    let count = |code: &str| found.iter().filter(|d| d.code == code).count();

    assert!(count("bad-choice") >= 7, "the misspelt conditions are named");
    assert_eq!(count("empty-text"), 1);
    assert_eq!(count("unknown-field"), 1);
    assert_eq!(count("missing-field"), 1, "the entry with no text");

    let placeholder = check_text(rel, r#"[ { "text": "Hi {friend}." } ]"#, &reference(), &PackFacts::default());

    assert!(placeholder.iter().any(|d| d.code == "bad-placeholder"));

    let offer = check_text(rel, r#"[ { "kind": "offer", "text": "Hi", "role": "starter" } ]"#, &reference(), &PackFacts::default());

    assert!(offer.iter().any(|d| d.code == "ignored-condition"));

    /* One entry on its own is accepted, as the game accepts it. */
    assert_eq!(codes(rel, r#"{ "text": "Hi" }"#), Vec::<String>::new());
}

/// The game's own armour, physics and models, laid out as a pack, raise no errors and no unknown bones: the checks that name bones must not cry wolf at what ships.
#[test]
fn the_games_own_armour_and_physics_match_its_models() {
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../src/main/resources/assets/pleasurehorizons/strut");

    if !assets.exists() {
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("data/pleasurehorizons/asset_sync/strut");

    for rel in walk(&assets) {
        if !(rel.starts_with("armour/") || rel.starts_with("physics/") || rel.starts_with("models/")) {
            continue;
        }

        let to = target.join(&rel);

        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(assets.join(&rel), to).unwrap();
    }

    let facts = collect_facts(dir.path());

    assert!(!facts.model_bones.is_empty(), "the models are read for their bones");

    let mut problems = Vec::new();

    for rel in &facts.files {
        if !rel.ends_with(".json") {
            continue;
        }

        let text = std::fs::read_to_string(dir.path().join(rel)).unwrap();

        for d in check_text(rel, &text, &reference(), &facts) {
            if d.severity != Severity::Info {
                problems.push(format!("{} {}:{} {} ({})", d.file, d.line, d.column, d.message, d.code));
            }
        }
    }

    assert!(problems.is_empty(), "the game's own files have problems:
{}", problems.join("
"));
}

#[test]
fn a_bone_that_is_not_in_the_model_is_found() {
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../src/main/resources/assets/pleasurehorizons/strut/models/entities/girls/nude/lucy.strutmodel");

    if !assets.exists() {
        return;
    }

    let dir = tempfile::tempdir().unwrap();
    let model = dir.path().join("data/pleasurehorizons/asset_sync/strut/models/entities/girls/nude/lucy.strutmodel");

    std::fs::create_dir_all(model.parent().unwrap()).unwrap();
    std::fs::copy(&assets, &model).unwrap();

    let facts = collect_facts(dir.path());
    let wearer = "data/pleasurehorizons/asset_sync/strut/armour/wearers/lucy.json";
    let found = check_text(wearer, r#"{ "extends": "pleasurehorizons:gurl", "slots": { "chest": { "covers": ["boobL", "boobX"] }, "belt": {} } }"#, &reference(), &facts);
    let has = |code: &str| found.iter().any(|d| d.code == code);

    assert!(has("armour-unknown-bone"));
    assert!(has("armour-unknown-slot"));
    assert!(has("armour-extends-missing"));
    assert_eq!(found.iter().filter(|d| d.code == "armour-unknown-bone").count(), 1, "boobL is a real bone");

    let physics = "data/pleasurehorizons/asset_sync/strut/physics/models/entities/girls/nude/lucy.json";
    let found = check_text(physics, r#"{ "version": 1, "bones": { "boobs": {}, "tail_of_a_dragon": {} } }"#, &reference(), &facts);

    assert!(found.iter().any(|d| d.code == "physics-unknown-bone" && d.message.contains("tail_of_a_dragon")));
}

#[test]
fn a_scene_the_pack_adds_needs_an_animation() {
    let dir = tempfile::tempdir().unwrap();
    let write = |rel: &str, text: &str| {
        let path = dir.path().join(rel);

        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    };

    write("pack.mcmeta", r#"{ "pack": { "pack_format": 48, "description": "x" } }"#);
    write("data/pleasurehorizons/scenes/hug.json", r#"{ "type": "stationary" }"#);
    write("data/pleasurehorizons/scenes/doggy.json", r#"{ "type": "on_bed" }"#);

    let found = check_project(dir.path(), &reference());
    let flagged: Vec<&str> = found.iter().filter(|d| d.code == "scene-without-animation").map(|d| d.file.as_str()).collect();

    assert_eq!(flagged, vec!["data/pleasurehorizons/scenes/hug.json"], "the game's own scene needs no animation of the pack's");

    write("data/pleasurehorizons/asset_sync/strut/animations/scenes/hug.jenny.strutanim", "x");

    assert!(!check_project(dir.path(), &reference()).iter().any(|d| d.code == "scene-without-animation"));
}

#[test]
fn dialogue_graphs_are_checked() {
    let rel = "data/x/dialogue/graphs/g.json";
    let run = |text: &str| codes(rel, text);
    let graph = |nodes: &str| format!(r#"{{ "format": 1, "kind": "dialogue", "start": "a", "nodes": {{ {nodes} }} }}"#);

    assert_eq!(run(&graph(r#""a": { "type": "line", "speaker": "her", "text": "Hi", "reaction": "smile", "next": "b" }, "b": { "type": "end" }"#)), Vec::<String>::new());
    assert_eq!(run(&graph(r#""a": { "type": "line", "speaker": "her", "text": "Hi", "next": "c" }, "b": { "type": "end" }"#)), vec!["unknown-node"]);
    assert_eq!(run(&graph(r#""a": { "type": "line", "speaker": "her", "text": "Hi", "reaction": "backflip", "next": "b" }, "b": { "type": "end" }"#)), vec!["unknown-reaction"]);
    assert_eq!(run(&graph(r#""a": { "type": "talk" }, "b": { "type": "end" }"#)), vec!["bad-choice"]);
    assert_eq!(run(&graph(r#""a": { "type": "choice", "options": [ { "text": "x", "reaction": "nod", "next": "b" } ] }, "b": { "type": "end" }"#)), Vec::<String>::new());
}

#[test]
fn bark_pools_are_checked() {
    let rel = "data/x/barks/b.json";
    let run = |text: &str| codes(rel, text);

    assert_eq!(run(r#"{ "pool": "gift_refused", "lines": [ { "text": "No." }, { "text": "Maybe.", "weight": 2 } ] }"#), Vec::<String>::new());
    assert_eq!(run(r#"{ "pool": "gift_refused", "lines": [ { "weight": 2 } ] }"#), vec!["missing-field"]);
    assert_eq!(run(r#"[ { "pool": "p", "lines": [ { "text": "x", "wieght": 2 } ] } ]"#), vec!["unknown-field"]);
}
