# The male and updating a pack

## The male

The male is the body that plays opposite the girl in a scene: the player, or later another character. The base mod has **one** male model, and every scene uses it. A pack does not bring a male of its own.

Its clips are in each scene's `male` role ([scene animations](doc:animations)). The male stands at the scene's origin, as the girl does, and the clips move it to where it should be.

Before pack format 2 the male was a second skeleton called `steve` inside every girl's model, and one animation file moved both. That rig is no longer used.

## Updating an older pack

Open a pack made before format 2 and the **Update this pack** window opens by itself. You can also open it from the top bar (**Split scenes**). It shows what it would do before it does anything:

- the old male rig is taken out of every girl's model;
- every scene's animation file is split into the girl's clips and the male's, and written as role folders with a layout file;
- each model and animation file gets its skeleton id;
- the pack's format number goes to 2.

It is done in place, and **Undo** puts every file back as it was. A scene is converted only when the Pack Creator has checked that every bone is posed exactly as before. A file that fails that check is left as it was, and then no model is stripped, so the old scenes keep working.

### What carries over

Each bone of the old male rig is moved onto the bone it became in the base mod's male model. Most are the same bone with a new name (`Torso2` is `torso`, `Head2` is `head`, `LeftArm` is `armL`, and so on). A few differ:

- The penis was rebuilt as `penis`, `shaft`, `penisHead` and the two balls. Its animation moves onto them as closely as the new model allows; the window lists how far off each bone can be, so look at those scenes in the simulator.
- The right arm's Alex and Steve variants moved slightly.
- Bones the new model has no counterpart for (the extra bones some girls' old rigs carry) lose their animation. The window lists them.

### Importing

Importing a girl from an old resource pack ends with the same update, so what you import is already in the new layout.

## Format numbers

The pack's format is a number in its project file. It goes up when the files change shape, and an older pack is brought up step by step. A pack made by a newer tool than yours is not changed: update the Pack Creator first.
