# Scene files

`data/<namespace>/scenes/<scene_id>.json`. A scene file says when a scene can play, who it fits and how it behaves. The animations themselves are separate: a folder of clips for each role in the scene, and a layout file ([scene animations](doc:animations)).

```json
{
  "type": "on_bed",
  "requires_affection": 3,
  "body_types": ["jenny"],
  "tags": ["romantic"]
}
```

Every setting, its range and its meaning is in [Scene settings](ref:scene). The scene form makes the file for you.

## Type

`type` is where and how the scene plays:

| Type | Plays |
|---|---|
| `on_bed` | On a bed the girl goes to |
| `on_player` | On the player |
| `stationary_intro` | Where she stands, after an intro animation |
| `stationary_contact` | Where she stands, with the player brought into contact |
| `stationary` | Where she stands, with no intro |
| `downed` | While she is downed |
| `player_downed` | While the player is downed |
| `player_animation` | Needs the player (she goes to them and they ride her), but plays one animation only: the `loop` clip, `amount_of_loops` times (`1`, or the setting left out, for a single shot), and then the scene is over. No intro, no thrusting |

Pick the type by what the animations assume. A bed scene is positioned by the bed, so `bed_alignment_offset` moves her along it; a stationary scene starts where she is.

## Who gets it

- `body_types` limits the scene to girls built on those skeletons. Empty means every body.
- `supported_girls` gives the scene only to those girls. `unsupported_girls` gives it to everyone but those. Use one, not both.
- `girl_overrides` replaces any setting of the scene for one girl, by her id. Use it when one girl needs a different threshold or bed offset without a second scene file.
- `requires_affection` is the affection she needs before she agrees.

## Tags

`tags` is what the scene appeals to. A girl's tastes decide how she reacts. The tags are `food`, `nature`, `luxury`, `work`, `adventure`, `kind`, `bold`, `playful`, `honest`, `calm`, `social` and `romantic`.

## Behaviour switches

| Setting | Meaning |
|---|---|
| `needs_to_strip` | She is undressed first, on the `becomenude` cue |
| `hide_player` | The player's body is hidden |
| `counts_towards_impregnation` | The scene can lead to a pregnancy |
| `bed_alignment_offset` | How far along the bed she is moved |
| `amount_of_loops` | How many loops the scene plays |
| `replace_default_animations` | Her own scene file replaces the shared one |
| `display_name` | The name shown in the scene menu, as a language key |

## The pace game

During a scene the player sets the pace and keeps their own edge from running away; the game picks the slow or the fast clip from it. `minigame` either keeps the classic hold-the-key bar (`"classic"`) or gives a block of numbers that replaces the feel read from the clips: `slow_band` and `fast_band` as `[low, high]`, `slow_seconds`, `fast_seconds`, `blend_seconds` and `fill_seconds`. Leave it out and the feel is read from the animation clips themselves, so most scenes need nothing here.

`cum_threshold` is how far along the scene must be before it can end.

The simulator plays exactly this, so tune it there: change a number, press Simulate, feel it.

## Keyframe events

With `use_keyframe_events` on, the scene's clips change on the cues in the animation itself. The sounds and chat lines of a cue come from the girl's event bindings ([cues](doc:cues)).

## Naming

The file name is the scene's id. The scene's layout (`strut/scenes/<scene>.strutscene`) and its animation folder (`strut/animations/scenes/<scene>/`) are named for it too, and the clips inside must be named `animation.<scene>.<part>`, so renaming a scene means renaming those as well. The checks flag a clip that does not follow the pattern.
