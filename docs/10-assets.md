# Models, textures and how the game reads a pack

## What goes where

Everything below is inside `data/pleasurehorizons/asset_sync/` (a girl's own files are always looked up in the `pleasurehorizons` namespace).

| File | Path |
|---|---|
| Nude model | `strut/models/entities/girls/nude/<girl>.strutmodel` |
| Dressed model | `strut/models/entities/girls/dressed/<girl>.strutmodel` (leave it out for a girl who is always nude) |
| Texture | `textures/entities/<girl>.png` |
| Animations | `strut/animations/entities/girls/base.<girl>.strutanim` for her everyday ones, `strut/animations/scenes/` for scenes ([scene animations](doc:animations)) |
| Physics | `strut/physics/models/entities/girls/nude/<girl>.json` (it sits where its model's path puts it) |
| Armour looks | `strut/armour/` |
| Sounds and bindings | `sounds.json`, `strut/event_bindings/` ([cues](doc:cues)) |
| Language | `lang/` |

The new girl wizard writes the set together, and can start from a game girl's models.

## Rules for asset sync

The game checks these when a pack loads, and the Pack Creator checks them as you work:

- Names use lower case letters, digits and `_ . -` only, with `/` between folders. A name with a space, a capital or a brace is refused.
- Models and animations must be the binary Strut files. A `.bbmodel` is not sent: convert it (the **Blockbench** button and the Strut Toolkit do).
- Textures are PNG. Very large images and very large files are refused, so size matters for everything players download.
- Namespaces `minecraft` and a few others are reserved.
- A girl's own files in any namespace but `pleasurehorizons` are not found by the game, and the checks warn about them.

## Strut models

A `.strutmodel` opens in the 3D viewer with her texture. The viewer shows armatures, skinning and the scene rig, and **Edit in Blockbench** round-trips the file: it opens her model with her texture, and **Save to pack** writes it back and the viewer reloads.

If her arms look wrong after editing, **Fix arms** repairs the lower arm's pivot and rotation the way Blockbench would, and removes the old weapon, off-hand and custom-hand bones. It shows a preview of what it will change, and keeps a backup in `.packcreator/backups`.

The viewer works out Molang expressions (a query it does not know counts as 0) and runs the soft bones of a physics file (see the Tools guide); chains, cloth and collisions are only in the game.

## Armour

Armour is three kinds of small file under `strut/armour/`:

| Folder | File | What it says |
|---|---|---|
| `wearers/<girl>.json` | `slots` (`head`, `chest`, `legs`, `feet`), each with `covers`: the bones of hers the armour hides; `extends`: another wearer to start from | Which parts of her body armour covers |
| `sets/<girl>.json` | `texture`, `texture_size`, `column` (`[x, y]`), `extends` | The armour's texture and where its pieces sit on it |
| `looks/<name>.json` | `match.materials` (such as `minecraft:iron`), `column` (a whole number), `tint`, `undyed` | Which column of the texture a material looks like |

The checks read these: an `extends` that names nothing is an error, a texture the pack does not have and a slot that does not exist are warnings, and a bone in `covers` that is not a bone of her models is a warning with the nearest bone offered. The same goes for the bones a merchant takes off as she undresses (in her girl file) and the bones in a physics file.

## How the game reads a pack

1. When a world loads, the game reads every data pack: girls, scenes, traits, dialogue.
2. Strut collects each pack's `asset_sync` folder and sends what players are missing when they join. Players do not install anything.
3. `/reload` re-reads data files in a running world. Models, textures and animations are sent again on the next join.
4. Anything the game cannot read is logged. The **Game log** button shows it next to the file it names.

## Common problems

- **A girl shows as a placeholder or "missing" model**: the model path or her id does not match the girl file.
- **The texture is wrong or black**: the texture's file name must be the girl's id, in `textures/entities/`.
- **Players see nothing new**: they need to rejoin; assets are sent on join, not on `/reload`.
