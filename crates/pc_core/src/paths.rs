//! Where a pack keeps its models and animations (pack format 3: the game's files are grouped under `entities`, `blocks` and `items`). Paths here are relative to a namespace's
//! `asset_sync` folder, starting with `strut/`, and the old (format 1 and 2) places are listed beside them so that a pack that has not been updated can still be read.

/// A girl's model: `variant` is `nude` or `dressed`.
pub fn girl_model(variant: &str, girl: &str) -> String {
    format!("strut/models/entities/girls/{variant}/{girl}.strutmodel")
}

/// Where that model was before format 3.
pub fn girl_model_old(variant: &str, girl: &str) -> String {
    format!("strut/models/{variant}/{girl}.strutmodel")
}

/// Whether `rest` (a path after `strut/`) is a girl's model, in either place.
pub fn is_girl_model(rest: &str) -> bool {
    let parts: Vec<&str> = rest.split('/').collect();

    rest.ends_with(".strutmodel") && (matches!(parts.as_slice(), ["models", "nude" | "dressed", _]) || matches!(parts.as_slice(), ["models", "entities", "girls", "nude" | "dressed", _]))
}

/// Whether `rest` is a model of a body (a girl's, the male, or one in a folder of the old kind): the models that carry a skeleton id.
pub fn is_body_model(rest: &str) -> bool {
    let parts: Vec<&str> = rest.split('/').collect();

    is_girl_model(rest) || (rest.ends_with(".strutmodel") && matches!(parts.as_slice(), ["models", "entities", "male", ..] | ["models", "male", ..]))
}

/// A girl's everyday animations, and the shared ones.
pub fn girl_base(girl: &str) -> String {
    format!("strut/animations/entities/girls/base.{girl}.strutanim")
}

pub fn girl_base_old(girl: &str) -> String {
    format!("strut/animations/base.{girl}.strutanim")
}

pub const SHARED_BASE: &str = "strut/animations/entities/base.strutanim";
pub const SHARED_BASE_OLD: &str = "strut/animations/base.strutanim";

/// The physics file of a girl's model: it sits where the model's own path puts it.
pub fn girl_physics(variant: &str, girl: &str) -> String {
    format!("strut/physics/models/entities/girls/{variant}/{girl}.json")
}

pub fn girl_physics_old(variant: &str, girl: &str) -> String {
    format!("strut/physics/models/{variant}/{girl}.json")
}

/// The base mod's male model.
pub const MALE_MODEL: &str = "strut/models/entities/male/nude/player.strutmodel";
pub const MALE_MODEL_OLD: &str = "strut/models/nude/player.strutmodel";

/// The physics file for a model path: `strut/models/<rest>.strutmodel` has `strut/physics/models/<rest>.json`.
pub fn physics_of(model: &str) -> String {
    model.replace("/strut/models/", "/strut/physics/models/").trim_end_matches(".strutmodel").to_string() + ".json"
}

/// Moves one old path to where format 3 keeps it, or None when the path is not one of the moved ones. `rel` is relative to the pack, `data/<ns>/asset_sync/strut/...`.
pub fn moved(rel: &str) -> Option<String> {
    let at = rel.find("/asset_sync/strut/")? + "/asset_sync/strut/".len();
    let (head, tail) = rel.split_at(at);
    let parts: Vec<&str> = tail.split('/').collect();
    let new_tail = match parts.as_slice() {
        ["models", "nude" | "dressed", file] if *file == "player.strutmodel" && parts[1] == "nude" => format!("models/entities/male/nude/{file}"),
        ["models", "nude" | "dressed", file] if *file == "player.strutmodel" => return None,
        ["models", variant @ ("nude" | "dressed"), file] => format!("models/entities/girls/{variant}/{file}"),
        ["models", "block", rest @ ..] => format!("models/blocks/{}", rest.join("/")),
        ["models", "item", rest @ ..] => format!("models/items/{}", rest.join("/")),
        ["animations", "base.strutanim"] => "animations/entities/base.strutanim".to_string(),
        ["animations", file] if file.starts_with("base.") && file.ends_with(".strutanim") => format!("animations/entities/girls/{file}"),
        ["animations", "block", rest @ ..] => format!("animations/blocks/{}", rest.join("/")),
        ["animations", "item", rest @ ..] => format!("animations/items/{}", rest.join("/")),
        ["physics", "models", variant @ ("nude" | "dressed"), file] => format!("physics/models/entities/girls/{variant}/{file}"),
        _ => return None,
    };

    Some(format!("{head}{new_tail}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_paths_move_to_the_new_ones() {
        let b = "data/pleasurehorizons/asset_sync/strut/";
        let at = |tail: &str| moved(&format!("{b}{tail}")).map(|m| m.trim_start_matches(b).to_string());

        assert_eq!(at("models/nude/lucy.strutmodel").as_deref(), Some("models/entities/girls/nude/lucy.strutmodel"));
        assert_eq!(at("models/dressed/lucy.strutmodel").as_deref(), Some("models/entities/girls/dressed/lucy.strutmodel"));
        assert_eq!(at("models/nude/player.strutmodel").as_deref(), Some("models/entities/male/nude/player.strutmodel"));
        assert_eq!(at("models/dressed/player.strutmodel"), None);
        assert_eq!(at("models/block/girl_bed.strutmodel").as_deref(), Some("models/blocks/girl_bed.strutmodel"));
        assert_eq!(at("models/item/girl_staff.strutmodel").as_deref(), Some("models/items/girl_staff.strutmodel"));
        assert_eq!(at("animations/base.strutanim").as_deref(), Some("animations/entities/base.strutanim"));
        assert_eq!(at("animations/base.lucy.strutanim").as_deref(), Some("animations/entities/girls/base.lucy.strutanim"));
        assert_eq!(at("animations/block/x.strutanim").as_deref(), Some("animations/blocks/x.strutanim"));
        assert_eq!(at("physics/models/nude/lucy.json").as_deref(), Some("physics/models/entities/girls/nude/lucy.json"));
        assert_eq!(at("models/armour/lucy/chest.strutmodel"), None);
        assert_eq!(at("animations/scenes/doggy/host/owners/lucy.strutanim"), None);
        assert_eq!(at("models/entities/girls/nude/lucy.strutmodel"), None);
    }
}
