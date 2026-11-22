//! The game's kinds of scene, as templates for the Strut Toolkit's Strut Scenes (`docs/STRUT_SCENES_API.md` in the toolkit). The toolkit knows no game: it asks what kind of scene a
//! new one is, shows the roles and clip phases that kind needs, and checks the export against them. The kinds here are the game's scene types (`type` in a scene file) with what
//! the simulator and the checks of the Pack Creator treat as needed for each, so what Blockbench says is missing is what the simulator would say is missing.
//!
//! They are written as a folder with a `templates.json` that the Pack Creator registers in Blockbench's storage (`strut_scene_template_folders`).

use crate::blockbench;
use serde_json::{json, Value};
use std::path::PathBuf;

const GROUP: &str = "Pleasure Horizons";

/// Where the templates are written.
pub fn folder() -> PathBuf {
    blockbench::data_dir().join("scenes")
}

fn host() -> Value {
    json!({
        "id": "host", "name": "Girl", "description": "The girl the scene is played with. Any skeleton: the scene file's body_types say whom it fits.",
        "required": true, "presetGroup": "Girls"
    })
}

fn male() -> Value {
    json!({
        "id": "male", "name": "Male", "description": "The male body (the player). Plays the clip of the same name as the girl.",
        "skeleton": "pleasurehorizons:male", "required": true, "camera": "boyCam", "presetGroup": "Male"
    })
}

/// A phase. `required` phases must have a clip; `variants` is how many there may be. Loop modes are what the game's own scenes use most; the game does not insist on them, so the
/// toolkit is asked not to either (`"loop": "any"`).
fn phase(id: &str, name: &str, description: &str, required: bool, max_variants: u32) -> Value {
    json!({
        "id": id, "name": name, "description": description, "loop": "any", "required": required,
        "variants": { "min": if required { 1 } else { 0 }, "max": max_variants }
    })
}

/// The slow phase: she changes side during it, so it may come in one clip per side (`slow_L`, `slow_R`, and the swapping sides), or as one plain clip.
fn slow(required: bool) -> Value {
    let mut p = phase("slow", "Slow", "What plays while the player is not thrusting. One plain clip, or one for each side.", required, 1);

    p["sides"] = json!(["L", "R", "L_swap", "R_swap"]);
    p["sidesOptional"] = json!(true);
    p
}

fn kind(id: &str, name: &str, description: &str, phases: Vec<Value>) -> Value {
    json!({
        "id": format!("pleasurehorizons:{id}"), "name": name, "group": GROUP, "description": description,
        "roles": [host(), male()],
        "clips": { "naming": "animation.{scene}.{phase}", "phases": phases, "freeClips": false },
        "rules": { "keyedRoles": ["host", "male"] }
    })
}

/// The sex phases a scene played with the player has: what plays at rest, while thrusting, and at the end.
fn sex(required: bool) -> Vec<Value> {
    vec![
        slow(required),
        phase("fast", "Fast", "What plays while the player thrusts.", required, 4),
        phase("cum", "Cum", "How the scene ends.", required, 1),
    ]
}

pub fn templates() -> Value {
    let intro = |required: bool| phase("intro", "Intro", "How the scene starts. More than one plays in turn.", required, 3);
    let mut on_bed = vec![intro(false)];

    on_bed.extend(sex(true));
    on_bed.push(phase("lay_on_bed", "Lay on the bed", "How she lies down on the bed.", true, 1));
    on_bed.push(phase("bed_idle", "Waiting on the bed", "What she does on the bed until the player starts.", true, 1));

    let mut on_player = vec![intro(false)];

    on_player.extend(sex(true));

    let mut contact = vec![phase("solo_intro", "Solo intro", "How the scene starts where she stands.", false, 3), intro(false)];

    contact.extend(sex(false));
    contact.push(phase("loop", "Loop", "What repeats.", false, 1));

    json!({
        "version": 1,
        "name": GROUP,
        "templates": [
            kind("on_bed", "On a bed", "She goes to a bed and lies down; the player takes part from there (type on_bed).", on_bed),
            kind("on_player", "On the player", "She goes to the player and they ride her (type on_player).", on_player.clone()),
            kind("stationary_contact", "Where she stands, in contact", "She stays where she is and the player is brought into contact (type stationary_contact).", contact.clone()),
            kind("stationary_intro", "Where she stands, with an intro", "She stays where she is and an intro plays first (type stationary_intro).", {
                let mut p = contact.clone();

                p[0]["required"] = json!(true);
                p[0]["variants"]["min"] = json!(1);
                p
            }),
            kind("stationary", "Solo, where she stands", "A scene with no player: one loop that repeats (type stationary).", vec![phase("solo_intro", "Solo intro", "An intro before the loop.", false, 3), phase("loop", "Loop", "What repeats; it plays amount_of_loops times.", true, 1)]),
            kind("downed", "While she is downed", "She is downed and the player takes part (type downed).", on_player.clone()),
            kind("player_downed", "While the player is downed", "The player is downed (type player_downed).", on_player),
            kind("player_animation", "One animation with the player", "She goes to the player and plays one loop, then the scene is over (type player_animation).", vec![phase("loop", "Loop", "The one animation: it plays amount_of_loops times.", true, 1)]),
        ]
    })
}

/// Writes the templates; returns how many.
pub fn write() -> Result<usize, String> {
    let all = templates();
    let dir = folder();

    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("templates.json"), serde_json::to_string_pretty(&all).map_err(|e| e.to_string())? + "\n").map_err(|e| format!("the scene templates could not be written: {e}"))?;
    Ok(all["templates"].as_array().map_or(0, |a| a.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_templates_are_the_games_scene_types_and_say_what_the_simulator_needs() {
        let all = templates();
        let list = all["templates"].as_array().unwrap();
        let ids: Vec<&str> = list.iter().map(|t| t["id"].as_str().unwrap()).collect();

        /* One for each scene type the game has, once each. */
        for scene_type in ["on_bed", "on_player", "stationary_contact", "stationary_intro", "stationary", "downed", "player_downed", "player_animation"] {
            assert_eq!(ids.iter().filter(|i| **i == format!("pleasurehorizons:{scene_type}")).count(), 1, "{scene_type}");
        }

        let required = |template: &str| -> Vec<String> {
            list.iter().find(|t| t["id"] == format!("pleasurehorizons:{template}")).unwrap()["clips"]["phases"].as_array().unwrap().iter().filter(|p| p["required"] == true).map(|p| p["id"].as_str().unwrap().to_string()).collect()
        };

        /* The same needs the simulator reports: a bed scene needs her to lie down and wait, and every scene with the player a slow, a fast and a cum clip. */
        assert_eq!(required("on_bed"), ["slow", "fast", "cum", "lay_on_bed", "bed_idle"]);
        assert_eq!(required("on_player"), ["slow", "fast", "cum"]);
        assert_eq!(required("stationary"), ["loop"]);
        assert_eq!(required("player_animation"), ["loop"]);
        assert_eq!(required("stationary_intro"), ["solo_intro"]);

        for template in list {
            let roles: Vec<&str> = template["roles"].as_array().unwrap().iter().map(|r| r["id"].as_str().unwrap()).collect();

            assert_eq!(roles, ["host", "male"]);
            assert_eq!(template["roles"][1]["skeleton"], "pleasurehorizons:male");

            /* The phases are the ones the layout writes and the game reads, each once. */
            let phases: Vec<&str> = template["clips"]["phases"].as_array().unwrap().iter().map(|p| p["id"].as_str().unwrap()).collect();
            let known = ["intro", "slow", "fast", "cum", "lay_on_bed", "bed_idle", "loop", "solo_intro"];

            assert!(phases.iter().all(|p| known.contains(p)), "{phases:?}");
            assert_eq!(phases.iter().collect::<std::collections::BTreeSet<_>>().len(), phases.len());
        }
    }

    #[test]
    fn the_phases_agree_with_the_layout_parser() {
        /* The names the toolkit makes from a template are the names the layering code reads back to the same phase, side and variant. */
        for (name, phase, side, variant) in [("animation.x.slow_R_swap", "slow", Some("R_swap"), 1), ("animation.x.fast_3", "fast", None, 3), ("animation.x.lay_on_bed", "lay_on_bed", None, 1)] {
            assert_eq!(pc_core::layers::clip_parts(name), Some((phase.to_string(), side.map(String::from), variant)));
        }
    }
}
