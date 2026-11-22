# Blockbench and Strut Scenes

Models and animations are made in **Blockbench** with the **Strut Toolkit** plugin. The Pack Creator installs both, links to them, and fills Blockbench with what it needs to know about the game.

## Setting it up

The **Blockbench** button (the cube) opens its window.

- **Install Blockbench** downloads Blockbench's own installer (about 110 MiB) and runs it. Blockbench 5.1 or newer is needed.
- **Open Blockbench with the toolkit** starts it with the Strut Toolkit installed in your own Blockbench, with your own settings and plugins. A Blockbench that is already open is used as it is; the first time, close it once so the Pack Creator can link to it.
- **Check for a toolkit update** looks for a newer Strut Toolkit on GitHub (`github.com/sandyysandy/Strut-BlockBench-Plugin`) and replaces the old one. It also looks by itself, at most once an hour, when a model is opened.

## Opening a model or animation

In the 3D viewer, **Edit in Blockbench** opens the file you are looking at, with her texture, and an animation on the model you pick. **Save to pack** (File menu in Blockbench, or Ctrl+Shift+E for a model and Ctrl+Shift+R for animations) writes it straight back over the file and the viewer reloads.

## Characters for a scene: actor presets

Strut Scenes (below) animates several characters at once. To add one you need their model, texture, role and skeleton id. **Register actors**, in the Blockbench window, writes all of that down once so that adding a character is a single choice:

- every girl of the base game, nude and dressed;
- **the male**, with his skin;
- the girls of the pack that is open: all of them, or the ones you tick.

Press it again after you add or change a girl. A Blockbench that is open is told at once; otherwise it is told the next time the Pack Creator opens it.

## Strut Scenes: making a scene

In Blockbench, a new project of the type **Strut Scenes** asks **what kind of scene** it is. The kinds are the game's own scene types:

| Kind | Needs |
|---|---|
| On a bed | a girl and the male; clips for slow, fast, cum, lay on the bed and wait on the bed; an intro if you like |
| On the player | slow, fast and cum |
| Where she stands, in contact / with an intro | the intro and loop clips the scene uses |
| Solo, where she stands | a loop |
| While she is downed / the player is downed | slow, fast and cum |
| One animation with the player | a loop |

The **Scene** panel then lists what is missing:

- Each **role** (the girl, the male) has an **Add** button until it has a character. Choose a preset, or a model file. The male is only offered his own skeleton.
- Each **phase** (intro, slow, fast, cum...) says what it still needs and has a button that makes the next clip with the right name. **Slow** can be one plain clip or one per side, and it can have several variants like fast.
- Animate every character on the one timeline, with Blockbench's normal tools.
- **Check** lists what would stop an export.

A scene's name is its id. Clips are named `animation.<scene>.<phase>` (a side adds `_L`, `_R`..., a second variant `_2`).

## Getting the scene into the pack

**Export Workspace** (File, Export) writes every character's clips into its role folder and the scene's layout, under `assets/<namespace>/strut/...`. A pack keeps those files under `data/<namespace>/asset_sync/`, and the **Update the pack** window moves what Blockbench wrote under `assets/` there for you: open it after an export. Then open the scene in the **simulator** to play it, and look at the **Problems** panel ([checks](doc:checks)).

## When something is missing

- **No presets in Blockbench**: press **Register actors**, then in Blockbench use **Strut Scenes: Character Preset Folders...** to see the folders it knows.
- **The male is white or only shows part of his body**: register again; his skin is the player skin the Pack Creator uses.
- **A kind of scene is missing**: the Pack Creator writes the game's kinds when it opens Blockbench. Close Blockbench and open it from the window again.
