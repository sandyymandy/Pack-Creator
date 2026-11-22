# Getting started

The Pack Creator is an editor for **packs**: the data that adds girls, scenes, behaviours and conversations to Pleasure Horizons. It knows the game's own rules, so it can complete what you type, check it as you save, and play a scene back the way the game will.

New here? Press **Take the tour** on the home page (or the tour button in the top bar of a project) for a short walk round the window, then follow [Your first pack](doc:first-pack).

## What a project is

A **project** is one data pack: a folder the game reads. It can hold girls, scenes, traits, dialogue, or only one of those. Nothing is required, so a pack that only adds a few conversation lines is a fine pack.

Everything is under `data/<namespace>/`. The namespace is your project's. The default is `pleasurehorizons`, which is the right choice for almost every pack, because the game looks up a girl's own model, texture and animations in that namespace no matter what namespace your data uses.

| Folder | What is in it |
|---|---|
| `girls/` | One file per girl: who she is ([girl files](doc:girls)) |
| `scenes/` | One file per scene: when and how it plays ([scene files](doc:scenes)) |
| `girl_traits/` | Behaviours a girl can be given ([trait files](doc:traits)) |
| `dialogue/` | The lines she says in conversation ([dialogue](doc:dialogue)) |
| `asset_sync/` | Everything sent to players: models, textures, animations, sounds, language files ([assets](doc:assets)) |

There is no separate resource pack. What players need to see and hear travels in `asset_sync`, which the game sends to them when they join, so a player never installs anything by hand.

## How it keeps your work safe

- Everything **autosaves** about a second after you stop typing (you can change the delay in Settings). A file is never half written: it is written beside the old one and swapped in.
- Each file's earlier versions are kept. **History** on a tab shows them side by side with the file now and restores one as an edit you can undo.
- **Delete** sends a file to the Recycle Bin, not into nothing.
- Undo and redo (Ctrl+Z, Ctrl+Y) also work on moves and renames in the file tree.

## The checks

The **Problems** panel at the bottom checks the pack with the game's own rules: a wrong value, a missing model, a cue nothing fires, a scene file with a name the game cannot load. Click a problem to jump to the place. Many problems have a **quick fix**: the lightbulb in the editor offers the repair.

Problems are rechecked when a file is saved, and the whole pack a moment later. An **error** means the game will not load that part of the pack the way you meant. A **warning** means it will load but probably not do what you want.

## Keys worth knowing

| Key | Does |
|---|---|
| Ctrl+P | Open a file by name |
| Ctrl+N | New girl, scene, trait or dialogue |
| Ctrl+Shift+F | Find and replace in every file |
| Ctrl+W | Close the tab |
| Shift+Alt+F | Tidy the file's layout |
| F2, Delete | Rename or delete what is selected in the tree |
| Ctrl+C, X, V | Copy, cut and paste files in the tree |
| F1 | These docs |

## Pack formats

The way a pack's files are laid out has changed over time. Open an older pack and the **Update this pack** window offers to bring it up to date, in place, showing what it will do first and with an Undo ([pack formats](doc:formats)). A pack made by hand or by an older Pack Creator opens fine either way; the update is for the game to read it.

## Trying your work

- **Simulate** on a scene file plays the scene the way the game plans it, with the real cues and sounds ([the simulator](doc:tools)).
- **Try in a world** copies the pack into a world's `datapacks` folder. In the game, run `/reload`.
- The **Game log** button reads what the game said about your pack on its last run.
- **Export** makes a zip to share.
