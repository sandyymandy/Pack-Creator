# The tools

## The new girl wizard

**Plus, then Girl** opens the wizard. It asks who she is, which skeleton she uses, and which models she has, checks the models against the skeleton's bones, then writes every file together: her girl file, models, texture and animation stubs. You can start from a game girl's models and change them afterwards. Nothing is written until the last step.

## The 3D viewer

Open a `.strutmodel` or `.strutanim` to see it.

- A model is drawn with her texture, nude or dressed, with armature skinning and smooth shading. The scene rig can be shown or hidden.
- An animation is drawn on a girl's model. Pick any clip, press **Play** or **Pause**, and the cues it fires are listed under it. Click a cue to stop at its moment.
- Under the model, a **Physics** strip lists the soft bones from her physics file (preset, frequency, damping, length, squish; hover for the rest). Tick **Simulate** to run the soft bones the way the game does (the same spring solver, presets and limits): they feel the model's own motion, so pick how it is moved (standing still, walking back and forth, hopping, turning), give it a **Bump**, or add **Wind**; **Reset** puts them at rest. A bone the model has no bone for is greyed and says so. Chains, cloth, flesh and the ragdoll are listed as not simulated, and so are collisions and squish, because they need the bodies around her. Tune the values in Blockbench's Physics tab (or the file) and the viewer picks the change up when it is saved.
- **Add basic physics.** The button in the Physics strip puts the usual **soft bodies** in the model's physics file in one click, making the file if there is none: the chest (`boobL` and `boobR`, `breastL`..., or a single `boobs`), the bottom (`cheekL`, `cheekR`, `assL`, `assR`, `buttL`...) and the belly, whichever of them the model has, found by name. A bone made of **meshes** becomes flesh; a bone made of **cubes** (or of meshes bound to an armature) cannot be moved by flesh, so it becomes a spring bone (`soft-bone`) instead. Flesh goes in the `flesh` section Strut reads (its old name `soft` still loads, and a file that has it is renamed when basic physics is added) (`{ "breasts": { "preset": "pouch", "bones": ["boobL", "boobR"] } }`; the left and right of a pair are one entry so they press each other). A soft body moves the **meshes** of those bones as one piece that swings from its attachment, presses against hands and the partner, and flattens against its pair; a bone that is made of cubes is not moved, and neither is a mesh bound to an armature or turned on its own pivot (the game's log says which). Bones the file already has, as spring bones or in a `flesh` entry, are left exactly as they are. Tune the entries in the JSON (`softness`, `limit`, `anchor`, `lump`, `node`, `volume`, `presses`... are listed in Strut's physics guide). The viewer lists the soft bodies but does not simulate them; spring bones, hair and tails are simulated here as before.
- **Edit in Blockbench** opens the file in Blockbench. For an animation you choose the model to put it on. **Save to pack** in Blockbench writes it back, and the viewer reloads.
- **Fix arms** repairs a turned lower arm bone and removes the old weapon bones ([assets](doc:assets)).

## The cue editor

In the viewer, **Edit cues** turns the cue list into an editor for the open clip. You can add a cue, move it, retime it, change its type and remove it. Names are suggested from the cues the game knows, and a name nothing fires is flagged. Only the cues are rewritten; the rest of the animation is copied byte for byte, so editing cues cannot damage your animation.

## The scene simulator

On a scene file, **Simulate** opens a tab that plays the scene as the game does:

1. She starts dressed.
2. If the scene needs it, she strips on the real `becomenude` cue.
3. She goes to a bed and lies down, and waits until you click her.
4. The game's own planner, ported to the simulator, chooses her clips from the cues and your thrust key, with a progress bar, the rig drawn with a player skin, and every cue listed with the sounds that play and the chat lines it says.

Both bodies are drawn: the girl on her own model and **the male** on the base mod's male model, each playing the clips of its own role ([the male](doc:male)). Click the scene rig to hide or show him.

The **Cum** key ends the scene once the bar is full. The pace game is played too, so you can tune a scene's `minigame` block and feel the change at once. The keys and sound volume are in Settings.

The simulator offers only the scenes that fit a skeleton and the girls they fit. The game's own girls and scenes are included as examples, so you can try any scene on a girl of yours.

## Importing an old girl

**Import** (the arrow into a tray) brings in a girl made for the older tools:

- An old **profile** becomes a girl file and one scene file per old scene, with a report of what was changed.
- An old **resource pack** (a folder or a zip) is converted: her models, her all-in-one animation file split into a base file and one file per scene, her cues as markers, bindings from her cue table, texture, sounds and language files.
- A pack with **several girls** imports them together. Each gets a body type and a profile found by name; you can change what her cues mean, and a basic physics file is made for a model without one.

- **Doing a whole pack at once, or one girl at a time.** Above the list of girls are a name filter, **Select all** and **Select none**, **Guess body types** (each girl's model bones are compared with the game's own skeletons) and **Review the chosen one by one**. To import everyone by name, select them, guess the body types, and press Import.
- **The one-by-one review** shows each girl's nude or dressed model in 3D and plays any of her clips on it. It lists the scenes her clips make up, found from her old profile when she has one, else from clip names with a part in them (`_intro`, `_slow`, `_fast`, `_cum`, a number after them, or the part first as in `slow_riding`). Rename a scene, move a clip into another part or scene, start a new scene from a clip, remove a scene (its clips become base animations), or press **Find from clip names** or **From her profile** to start over. Each scene becomes its own scene file and its own animation file. A pack already in the newer layout (one animation file for each scene) shows its scenes but imports them as they are.
- **Comparing with the game's own girls.** In the review, **Compare** shows Lucy, Mika, Momo, Bee girl, Kobold, Slime or the Wandering merchant next to her (**Side by side**, two views) or on top of her (**Overlaid**: the reference is drawn as an outline over her, in one view). The camera is shared, and the clip you play plays on both at the same moment, so you can see whether her proportions, bones and motion line up with a girl the game ships. A clip only moves a reference with the same bone names.
- **Scene types.** Each scene in the review has a **type**: on the player, with the player and one animation (it needs the player but plays only its loop clip, once or a number of times), on a bed, standing with the player in contact, standing alone with an intro, standing alone, while she is downed, or while the player is downed. The type decides which parts are offered (a bed scene has a lie-down and an idle; a solo scene has a loop, and a solo intro for the one with an intro; the others have intro, slow, fast and climax). It is guessed from the clip names (a loop with no thrusting is a solo scene, a lie-down makes a bed scene, "downed" in the name makes a downed one) or read from her profile, and written to the scene file.
- **Girls the project already has are left out.** When a pack's girls are listed, any whose girl file is already in the project are marked "already in the project", cannot be ticked, and are skipped by Select all, the body type guess and the review.
- **Removing animations from a file.** An animation file opened in the viewer has an **Animations...** button: it lists every clip with its length and cues, with a search box and tick boxes, and **Remove** takes the ticked ones out (a file keeps at least one). A copy of the file as it was is kept in the project's `.packcreator/backups` folder. The other clips are copied byte for byte.
- **Who a scene is for** (for packs still in the single-file layout; a pack in the current layout has role folders instead, [scene animations](doc:animations)). The two-people button in the top bar (or **Who this scene is for...** on a scene file in the tree) lists every scene with its animation files and whose they are: one girl's (`scene-girl`), a skeleton's (`scene.body`) or everyone's. **Make skeleton-wide** moves her file to `scene.body.strutanim` and takes her name out of the scene file's `supported_girls` (adding her body type to `body_types`); **Make hers only** does the opposite (moving the skeleton file to `scene-girl.strutanim`, or copying it when "Keep the skeleton file" is ticked) and names her in the scene file. **Make all skeleton-wide** does every one-girl scene, saying which it left alone (a file already in the way is never overwritten). The scene file's earlier text is in its History.
- A resource pack made for the **old mod** (its files are under `assets/sexmod/`: one model in `geo/<name>/`, one animation file, a folder of sounds for each cue) is recognised and rewritten as it opens. The single model is used for both her nude and her dressed model, scenes are found from clip names ending `_intro`, `_slow`, `_fast` and `_cum`, each sound folder becomes a sound her cues play, and her name is read from the pack's language file. Her scene types, affection and tags are guesses, so open each scene file after importing. A pack with no sounds or language file (just a model, animations and a texture) comes in as a girl named for her model; rename her afterwards.

The report lists everything it made and anything it could not convert. Read it before deleting the original.

### Converting one file

The **Convert a model or animation file** tab of the same dialog takes a single plain `.geo.json` model or `.animation.json` animation and writes the `.strutmodel` or `.strutanim` where the game reads it, for the girl you name. Pick whether a model is her nude or dressed one, and whether an animation is her everyday set (clips renamed `animation.base.<name>`), one scene's clips (`animation.<scene>.<name>`) or kept as named. Give the model the animation was made on and it is stamped with that skeleton. Cues in the clips become markers. Nothing already in the pack is overwritten.

## Blockbench

The **Blockbench** button installs Blockbench and the Strut Toolkit for you and links to your own Blockbench. Use it to make models and animations, and, with **Strut Scenes**, whole scenes with several characters on one timeline. The Pack Creator gives Blockbench the game's characters (the base girls, the male and your pack's girls) and its kinds of scene. See [Blockbench and Strut Scenes](doc:blockbench), [assets](doc:assets) and [animations](doc:animations).

## Skeleton ids

The **Skeleton ids** button (joints) lists every model and animation file with the skeleton id it should carry, grouped by id. Girls on one id share a rig. Type another id beside a girl whose model is built differently, then **Write ids**: a copy of each file as it was is kept, and the file is checked to read back with its id ([scene animations](doc:animations)).

## Update the pack

The **Update the pack** button opens the window that brings a pack to the current format ([pack formats](doc:formats)): it lists the steps that apply to this pack, what it would write or move, and has an **Undo** for the conversion.

## History

**History** on a tab shows earlier versions of a file. A copy is kept as you save, at most one every 30 seconds and forty per file. Pick one to compare it side by side with the file now, and restore it as a single undoable edit.

## Find and replace

**Ctrl+Shift+F** searches every file in the pack. Replace changes the matches you choose; the files it touches reload in their tabs.

## Game log

**Game log** reads the game's `logs/latest.log`: what Pleasure Horizons and Strut said about your files, each tied to the file it names, and what asset sync offered. The game cannot be asked directly, so the log of its last run is what you see. Run the game, then open the log.

## Try in a world, and Export

**Try in a world** copies the pack into a world's `datapacks` folder (choose the world's folder, the one with `level.dat`). In the game, run `/reload` and check it with `/datapack list`.

**Export** writes the pack as a zip to share.

## Changes made outside the Pack Creator

If another program changes a file you have open, it reloads, and the tree follows renames and new files. You can use any editor next to the Pack Creator.

## Settings

The sliders icon opens Settings: autosave delay, editor text size, minimap, word wrap, tab size, the simulator's thrust and cum keys and its sound. Every change applies at once.


## Fix the models

The wrench button in the top bar lists what can be fixed in the models of the pack, for each girl: her **armour** (the `armor...` bones cut out of her dressed model into one model for each slot, `head`, `chest`, `legs` and `feet`, under `strut/models/armour/<girl>/`, and taken out of both of her models), her **arms** (a lower arm built with a rotated bone, and the old hand item bones) and the **old male rig** for the whole pack. Tick what you want and choose **Apply picked**, or **Apply all**. Undo puts the files back. An import and the pack update apply the armour and arms fixes by themselves, so this is for a pack that was not imported.
