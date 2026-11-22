# Checks for scenes and animations

The **Problems** panel names each check by a short code. These are the ones for scenes, layouts and animation files. Each says what is wrong and what to do.

| Code | Means | Do |
|---|---|---|
| `scene-without-animation` | A scene the pack adds has no animation | Add the scene's layout and role folders ([scene animations](doc:animations)) |
| `scene-without-layout` | A scene has role folders but no `strut/scenes/<scene>.strutscene` | Make the layout (the **Update the pack** window writes it; Blockbench's export writes it too) |
| `bad-layout` | The layout is not valid JSON, or has no `roles` | Restore it from History, or export the scene again |
| `role-without-files` | A role of a scene has no animation files | Add the role's folder with its files |
| `role-without-file-for-skeleton` | Nothing in a role plays for a skeleton the layout allows | Add `<role>/skeletons/<skeleton>.strutanim`, or a character file for a girl who has it |
| `girl-not-in-role` | A girl's file is for a role her skeleton is not allowed in | Give her the right skeleton id, or add her skeleton to the role's `compatible` list |
| `old-and-new-scene-files` | A single old scene file lies beside the new role folder; the game reads the folder, so the old file is never played | Delete the old file |
| `character-clip-not-in-skeleton` | One character's file has clips her skeleton's file for the role does not | Add the clip to the skeleton's file, or remove it |
| `male-missing-clip` | The girl has a clip the male has no clip of the same name for, so he stands still while she moves | Give the male a clip of that name, in `male/characters/<girl>.strutanim` or his skeleton file |
| `clip-differs-between-roles` | The same clip has a different length or loop mode for two roles | Make them agree; the roles play one clip together |
| `layout-clip-length` | The layout says a clip is a different length from the file's | Export the scene again, or fix the number |
| `animation-without-skeleton-id` | An animation file does not say which skeleton it was made for | **Skeleton ids**, then write the ids |
| `model-without-skeleton-id` | A model has no skeleton id, so it cannot fill a role that names one | The same |
| `stale-fingerprint` | An animation was made for a model with different bones (a rig was cut out of the model since), so the game warns it was "authored against a different skeleton" | **Update the pack** stamps it again |
| `unused-male-rig` | A model still carries the old male skeleton (`steve`) although every scene is split | **Update the pack** takes it out |
| `sound-event-number` | A sound event in a clip has something after the sound's name that is not a number (`name, volume, pitch`) | Fix the event, or make a second sound its own event |

## Quick checks of your own

- A scene plays but stops halfway for the male: look for `male-missing-clip`.
- The game log says a clip is "authored against a different skeleton": look for `stale-fingerprint`.
- A girl does not take part in a scene she should: look for `girl-not-in-role` and `model-without-skeleton-id`.
