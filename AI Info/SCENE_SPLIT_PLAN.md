# Scene split: converting packs to one model per participant

Status: **plan, not started** (2026-11-21). Part of the move to multi-participant scenes:

- Strut: `Strut/docs-internal/MULTI_ACTOR_SCENE_PLAN.md` (per-role animation files, one clip name for every role, scene seats, skeleton ids).
- PH: `Pleasure Horizons/AI Info/design/MULTI_PARTICIPANT_SCENES.md` (cast, male body, the scene system without riding).
- Blockbench toolkit: `Strut BB Plugin/docs-internal/SCENE_WORKSPACE_PLAN.md` (the animation workspace, per-actor export).

This plan is the Pack Creator's part: converting every pack in bulk, checking it, and showing it.

## 1. What changes in a pack

**Before.** A scene file plays on the girl alone. Every girl model carries the male as a second root, `steve` (132 bones: torso, head with `boyCam`,
Alex and Steve arms, legs, his parts), animated by the same clip. Files: `scenes/<scene>.strutanim`, `<scene>.<body type>`, `<scene>-<girl>`.

**After.**

```
asset_sync/strut/models/male/default.strutmodel                         the male, its own model (from the girls' steve rig), skeleton id pleasurehorizons:male
asset_sync/strut/animations/scenes/doggy/host/skeletons/jenny.strutanim    the girl's clips, for every model with skeleton id pleasurehorizons:jenny
asset_sync/strut/animations/scenes/doggy/host/owners/momo.strutanim        only what Momo changes (clips identical to the skeleton's are dropped)
asset_sync/strut/animations/scenes/doggy/male/skeletons/male.strutanim  the male's clips
asset_sync/strut/scenes/doggy.strutscene                                   the layout: roles, clip sets, skeleton ids, compatible ids, clip lengths
```

Files follow Strut's clip sets (`Strut/docs-internal/CLIP_SETS_PLAN.md`): a model's file first, its skeleton's second, first hit wins per clip. The old
shared file becomes the skeleton file of the scene's host skeleton, and the scene's `body_types` become the layout's `compatible` ids. Girl models lose `steve`
once a pack is fully split. The PH-side picture and the de-duplication pass are in `Pleasure Horizons/AI Info/design/ANIMATION_WORKFLOW_V2.md`.

**Skeleton ids.** Every model gets a skeleton id in its header and every animation file the id it was made for (a string, `pleasurehorizons:jenny`;
nothing else, no skeleton file). The converter proposes one per girl model: from the girl's `body_type` where the pack has one (`jenny` becomes
`pleasurehorizons:jenny`), otherwise the girl's own id, and the app shows the grouping once so a person can merge girls who share a rig or split one who
does not. The id is what the checks and the game compare.

## 2. The converter

A new pass in `convert/`, `scene_split`, run on a pack (or the whole of `test_pack`, or a folder of old packs as part of the existing import):

1. **Find the male rig** in each girl model: the root named `steve` (the name is a setting; the converter suggests it when a root's bone names
   match the male rig).
2. **Extract** it from one model into `models/male/default.strutmodel` (bones, pivots, rotations, geometry, UVs), with the skeleton id `pleasurehorizons:male`. Every other girl's copy is compared with it. A copy that matches is dropped. A copy that differs is reported, bone
   by bone (measured on the game's own girls: kobold adds `bone3`; slime and the merchant add up to seven `boneN` bones and turn `bone2` and `bone14` by
   45°), with three choices: the bones belong to the girl (move them out of the subtree), to the male (add them to the male model), or this girl
   gets a male model of her own.
3. **Split every scene file**: the channels of bones under `steve` go to the male's file, the rest stay in the host's. File names map as in §1
   (`doggy-momo` becomes `host/owners/momo` and a male clip file, then de-duplicated against the skeleton's). Clip names, lengths and loop modes are kept, so the two halves of one clip stay one
   clip.
4. **Markers**: each marker goes to the host's clip unless the cue mapping says it is the male's (the cue table gets an `owner` column; sounds that
   come from the male's body, for example). The default keeps every marker on the host, which is how the game reads them today.
5. **Rebase the male's root**: the `steve` root's pivot sits at `[0, 0, -16]` from the girl's origin. The male model's rest root goes to its own
   origin (so the same body walks about normally), and that offset is folded into the root channels of every male clip (position, and rotation
   composed about the girl's origin).
6. **Write the layout** for each scene (roles `host` and `male` with their skeleton ids, clip lengths and loop modes).
7. **Strip** `steve` from the girl models (a separate, last step, only when every scene of the pack is split).

## 3. Proving it

The split must change nothing on screen. For every clip at a set of ticks, the converter poses the **old** girl model with the old clip and the
**new** girl and male models with their split clips (male placed at the origin), and compares the world matrix of every bone (within 1e-4).
This runs:

- as a test on the game's own scene files and on `test_pack` (a golden per file, as the converter's existing goldens);
- in the app after a conversion, as a finding per clip that fails, naming the bone and the tick.

The posing code is the existing viewer's (models and clips are already read and played there), moved into `pc_core` so tests can use it.

## 4. In the app

- **Convert panel**: a "Split scenes" action on a pack, with a preview (files to be written, removed, renamed), the rig findings of §2.2 to decide, and
  the proof's result. Undoable through the existing history.
- **Scene simulator and viewer**: plays every role of a scene together, each on its own model, at the origin (the host's girl swappable, as in game).
  A scene that is not split yet plays the old way, so a half-converted pack still looks right.
- **Checks** (`check/scene.rs`, `check/assets.rs`):
  - a role with no file for a model that may fill it (its skeleton or a compatible one has none), and a model file naming a clip its skeleton lacks;
  - clips of one name whose length or loop mode differ between roles;
  - a girl model whose skeleton id is not the one a scene role names, and a model or animation file with no skeleton id;
  - a pack that has both the old single file and the new folder for one scene (the folder wins in game; the old file is dead);
  - a girl model that still has `steve` when every scene is split (wasted bones).
- **Docs**: `05-scenes.md` and `06-animations.md` rewritten for role folders, the layout file, skeleton ids and the cast; a page on the male model.
- **Skeleton id field** in the model and girl views, so an id can be read and changed without Blockbench.
- **Edit in Blockbench**: opens a split scene as a workspace (the toolkit's `.strutstage`), host and male as actors, instead of one model.

## 5. Order

| Stage | What | Needs |
|---|---|---|
| 1 | `pc_core` posing (moved from the viewer) and the bone-matrix comparison | nothing |
| 2 | Skeleton ids written on models and animation files; `scene_split` on one scene (doggy), with the proof as a test | Strut's header fields (its stage 4) |
| 3 | Every scene of the game, the rig findings and choices, the layout files | — |
| 4 | Convert panel action, simulator and viewer for several roles, checks | — |
| 5 | Strip `steve`, docs, Edit in Blockbench as a workspace | the toolkit's workspace |
| 6 | The importer of old third-party packs runs the split as its last pass | — |

## 6. Questions

1. Strays in kobold, slime and the merchant: decide per girl in the app (above), or fix the game's own models by hand once?
2. Should a pack keep its old single files after a split (for older game versions), or be converted in place? Recommendation: in place, with the
   history to undo; the game reads only the new layout once PH has moved.

## Talking to the other sessions (the Agent Bridge)

The Strut session (the engine), the game mod's session, the Pack Creator's session (`pc`) and the Blockbench toolkit's session (`plugin`) cannot see each other. They
talk through a shared mailbox folder that lives outside every repository, so nothing in it is committed: `Documents/Idea Projects/Agent Bridge/`. **Read its
`README.md` once**, then work like this:

- **You are the `pc` session.** Write only in `Agent Bridge/pc/`. Read `strut/`, `ph/` and the other tool's folder before each unit of work, and after a
  build failure that might come from another repo. Never edit or delete a file another session wrote; to answer, write a new file.
- **A message is one file**, `NNN-pc-to-<who>-<slug>.md` (`NNN` the next three-digit number in your own folder, from 001), starting with the header the README
  gives (`From`, `To`, `Type` question | request | answer | decision-needed | fyi | done, `Re`, `Status`, `Date`), then a short summary and the detail. Never rewrite
  an old message; a correction is a new one.
- **Keep a `STATUS.md`** in your folder and overwrite it freely: *Now*, *Blocked on*, *Last commit*.
- **Who to ask**: an engine API, a format change or a bug in Strut, or a new jar: `strut`. How the game reads a file or what a scene means: `ph`. How to convert, check, or show a pack: that is you. An export or editor feature in Blockbench: `plugin`.
  A choice only Sandy can make: a `decision-needed` message addressed `To: sandy`; do not build on either option until he has answered.
- **The formats are shared** by `strut` (reads them), `plugin` (writes them) and `pc` (reads, writes and checks them). The byte layout is written down in the Strut
  repository, `docs-internal/MULTI_ACTOR_SCENE_PLAN.md` section 4. A change to a format is proposed in the bridge by `strut` and answered by the other two before
  anyone builds it; if a byte there does not match what your code does, say so first.
- This repository may name the game mod's own content in its own docs; the public ones (the toolkit, Strut Public) never do, so a message that is to be copied into a public repo is written neutrally.
- **What does not go through the bridge**: secrets, tokens, personal data, and anything that belongs in a repo's own docs (put it there and link it).

**Open for you now:** `strut/062-strut-to-pc-and-plugin-formats-ready.md`: the Strut end of skeleton ids, clip sets, event bindings by layers and scene layouts is built and the formats are written down byte for byte. **Stage 2 of this plan starts with the ids**: write a skeleton id into every model and animation file of the game's own packs. Answer with a short `pc/001-pc-to-strut-...` saying you have read it and where you are starting. Also read `strut/061-...` (what the game mod has been asked) so your checks agree with what it will declare.
