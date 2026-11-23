# Changelog

## 1.3.0

Armour, a tools dock, and fixes for the models and scenes of imported girls.

### Armour
- **Armour is cut out of the girls' models.** The armour bones of an old model (`armorChest`, `armorPantsUpL`, `armorShoesR`...) become the four slot models the game wears (`strut/models/armour/<girl>/head|chest|legs|feet`), taken from her dressed model (or the nude one if only that has it) and removed from both of her models. A set and a wearer file that extend the game's are written for her.
- **Armour previews** use the game's armour texture (or the pack's own `textures/armor/armor.png`), in the viewer and when opened in Blockbench.
- **Wear her armour in the viewer:** the View menu has head, chest, legs and feet checkboxes that put each slot on her model, following her bones.
- **Armour wearer form:** wearer files open as a form (the wearer it starts from, the bones each slot hides, its damps) with a JSON view beside it.

### Fixing models and scenes
- **Fix the models** (new tool): lists what can be fixed, per girl (armour, the lower-arm bones, the old male rig). Apply everything or pick, and undo it. Imports and the pack update apply the armour and arm fixes on their own. The old Fix arms button in the viewer is gone.
- **Scenes whose girl's model has already lost the old male rig** (an earlier import) can now be converted: the update uses the old rig to check every bone is posed as before, then writes the role files and the layout.
- **Rename animations:** the Animations dialog lets you rename clips as well as remove them. Suggest names adds the `animation.` prefix and calls `start` `intro`. A scene layout that lists a clip follows the rename, with its phase, variant and side.

### The app
- **Tools dock:** the top bar's many buttons moved into a dock that slides in from the right (the three dots, or Ctrl+J), with each tool's name and what it does, in groups. Customize reorders the tools and pins any of them to the top bar.
- **The viewer's toolbar** is tidied: the display options (shading, smoothing, brightness, grid, scene rig, armour) are in a View menu.
- **Preview a girl** from her girl file with one button.
- **Import:** the old resource pack can be picked as a folder or a zip (two buttons), and a pack with one girl asks for her profile file instead of a folder.
- **New project from old girls** on the home page: makes the project and opens the import straight away.

### Fixes
- The player's skin is drawn only on the male's own body, no longer on the girl's armour, items or cameras.
- The male's penis shows again in the scene simulator (his meshes and the armatures that move them were left out).
- Armour models are visible in the viewer (their bones were hidden as scene rig).

## 1.2.0

One model per participant, and a lot of tooling around it. **Packs made before this are offered an update when you open them** (it shows what it will do first, and has an Undo for the scene conversion). See the "Pack formats" page in the docs.

### Scenes and animations
- **The male is his own model.** A scene is now played by the girl and the base mod's male model together, each from its own clip files, with a layout file per scene (`strut/scenes/<scene>.strutscene`) saying who fills which role. The old male skeleton (`steve`) inside every girl's model is gone.
- **Skeleton ids** on every model and animation file, with a **Skeleton ids** button to review the grouping and give a girl whose model is built differently an id of her own.
- **Update the pack** (new top-bar button, also opens by itself on an older pack): converts scenes in place, checking that every bone is posed exactly as before; layers event bindings and everyday animations by skeleton and character; moves files to the game's new folders (`entities`, `blocks`, `items`); renames `owners` to `characters`; moves scenes Blockbench exported under `assets/` into `asset_sync`. Pack format is now **5**.
- Importing an old girl ends with the same update, so imports come out in the current layout.
- The scene **simulator** plays the girl and the male together, and reads the new layout (it was broken for scenes in it before this release).

### Blockbench
- Uses **your own Blockbench** profile and plugins, and reuses a Blockbench that is already open.
- The **Strut Toolkit updates itself** from its GitHub releases. A start that hung on "Starting..." now times out.
- **Strut Scenes:** the Pack Creator gives Blockbench the game's characters (every base girl, the male with his skin, and your pack's girls) as presets, and the game's scene kinds (on a bed, on the player, solo, downed, one animation) as templates. **Register actors** in the Blockbench window writes them.

### Checks
- New checks for layouts, role folders and skeleton ids (`scene-without-layout`, `role-without-files`, `girl-not-in-role`, `old-and-new-scene-files`, `unused-male-rig`, and more), `male-missing-clip` (the male has no clip of a name the girl plays), `stale-fingerprint`, and `sound-event-number`. The "Checks" docs page lists them all.

### The app
- **Checks GitHub for a newer Pack Creator** when it starts, with what is new and a download; Settings has **Check now**.
- Opening a pack, the viewer and the importer are much faster, and the window fits the screen with a zoom setting.
- The import window was redesigned.
- Every top-bar button has its own icon.
- A longer **tour** and new docs: Pack formats, Blockbench and Strut Scenes, Checks, the male; the window, tools and troubleshooting pages are brought up to date.

### Fixes
- Top and bottom cube faces are drawn the right way up, the player skin no longer leaks onto girls' models, and the bed offset shows in the import preview.
- Animation playback is much smoother, and the plugin update picks up a release uploaded again under the same tag.
- A girl with extra scene clips (variants) keeps them for the male as well, so he no longer stands still while she moves.
- Animation files are stamped again with the bones of their girl's model, which clears the "authored against a different skeleton" warning.
