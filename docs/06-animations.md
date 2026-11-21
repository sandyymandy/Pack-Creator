# Scene animations

Animations are `.strutanim` files in `data/pleasurehorizons/asset_sync/strut/animations/`. They are binary: make them in Blockbench with the Strut Toolkit (the **Blockbench** button installs both) and save them to the pack. The Pack Creator never asks you to write one by hand.

A scene is played by **one model per character**: the girl has her model and the male has the base mod's own male model ([the male](doc:male)). Each plays its own clips, and the two play together as one scene. This is pack format 2. A pack made before it is offered an update when you open it ([updating a pack](doc:male)).

## The layout of a scene

A scene called `cowgirl` is these files:

```
strut/scenes/cowgirl.strutscene                                       the layout: who fills which role
strut/animations/scenes/cowgirl/host/skeletons/jenny.strutanim        the girl's clips, for every model with the skeleton pleasurehorizons:jenny
strut/animations/scenes/cowgirl/host/owners/momo.strutanim            only what Momo does differently
strut/animations/scenes/cowgirl/male/skeletons/male.strutanim   the male's clips
```

- A **role** is a part in the scene: `host` (the girl) and `male`. Each has a folder of animation files.
- A **skeleton** file is played by every model with that skeleton id. An **owner** file is played by one girl and holds only the clips she changes.
- **Her own file wins over her skeleton's.** A clip is looked up in her file first and in her skeleton's second; the first one that has it plays.
- The male's clips usually do not change from girl to girl, so one file serves everyone. When a scene moves the male differently with different girls, the male has a file for each girl under `male/owners/`.

## The layout file

`strut/scenes/<scene>.strutscene` says who may fill each role and how long each clip is:

```json
{
  "roles": {
    "host": { "clipSet": "scenes/cowgirl/host", "skeleton": "pleasurehorizons:jenny", "compatible": ["pleasurehorizons:bia"], "required": true },
    "male": { "clipSet": "scenes/cowgirl/male", "skeleton": "pleasurehorizons:male", "required": true }
  },
  "clips": { "animation.cowgirl.loop": { "length": 40, "loop": "loop" } }
}
```

`skeleton` is the id a model must have to fill the role, and `compatible` lists others that may. `clips` gives each clip's length in ticks and how it plays (`once`, `loop` or `hold`). The Pack Creator writes this file when it converts a scene, and the checks tell you when it disagrees with the clips.

## Skeleton ids

A skeleton id says which rig a model is, such as `pleasurehorizons:jenny`. Models carry it, and so does every animation file made for them. Two models with the same id are meant to have the same bones. The game compares these ids and nothing else to decide who may play a clip or fill a role.

The **Skeleton ids** button in the top bar shows the ids of every model and animation file, groups the girls by id and writes them in. A girl whose model is built differently from the rest gets an id of her own by typing one next to her name.

## Older files (format 1)

Before format 2 a scene was a single file in the `scenes/` folder, with the male inside the girl's model: `<scene>-<girl>.strutanim`, `<scene>.<skeleton>.strutanim` or `<scene>.strutanim`. The game reads the role folder when there is one, so a single file left beside it is never played; the checks flag it. Her everyday animations are in `base.strutanim` (shared) and `base.<girl>.strutanim` (hers) and stay single files.

### Choosing where to put an animation

- Animate for a **skeleton** when the same motion is right for every girl built on it. One file then serves them all, and a new girl on that skeleton gets the scene for free.
- Animate for **one girl** only when she differs: a different height, a prop, a unique pose.

`replace_default_animations` in the scene file controls whether a girl's own file replaces the shared one entirely or is used alongside it ([scene files](doc:scenes)).

## Clip names

A clip in a scene is named `animation.<scene>.<part>`. The slow and fast loops the pace game switches between are separate clips. The girl's and the male's file name the **same clips**, with the same length and loop mode, because the two halves of one clip play together; the checks tell you when they differ.

## Markers (cues)

A clip can carry markers at points in time. A marker is a dash-separated list of cue names (`moan-thrust`). What each cue does is set by bindings: sounds to play, lines to say. See [cues and sounds](doc:cues). Markers stay in the girl's file.

A few cues are special to the game itself, such as `becomenude` (she undresses) and `thrust` (the beat the pace game rewards). The reference lists every cue the game's own animations use.

## Looking at one

Opening a `.strutanim` in the viewer plays any clip, lists its cues and lets you add, move, retime and remove them ([the cue editor](doc:tools)). The scene simulator plays the girl and the male together. **Edit in Blockbench** opens a file on its model.

## Common problems

- **A scene plays nothing for a girl**: the role has no file for her skeleton (the checks say which), or her model has no skeleton id, or its id is not the one the role names.
- **Clips not found**: the clip names must start `animation.<scene>.`, with the scene id from the layout's name.
- **A girl does nothing in a scene**: her bones differ from the skeleton the animation was made for.
