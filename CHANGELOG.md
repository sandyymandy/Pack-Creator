# Changelog

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
