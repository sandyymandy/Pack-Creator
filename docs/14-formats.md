# Pack formats and updates

Pleasure Horizons has changed how a pack's files are laid out more than once. The Pack Creator keeps a number for the shape of your pack, its **format**, and brings an older pack up to date for you. You never have to move files by hand.

## Where the number is

It is in `.packcreator/project.json` (`"format"`), written when the project is made and raised when an update is applied. A pack made by hand has no such file; the Pack Creator makes one when you open it and judges the pack by what is in it as well, so an old pack is recognised either way.

## The formats

| Format | What it is |
|---|---|
| 1 | A girl's model carries the male as a second skeleton called `steve`, and one animation file per girl and scene moves both of them |
| 2 | One model per participant: the girl and the male each play their own clips ([the male](doc:male)); every scene is a folder of role files with a layout file; models and animation files carry skeleton ids ([scene animations](doc:animations)) |
| 3 | The game's files grouped under `entities`, `blocks` and `items`: a girl's models are in `strut/models/entities/girls/nude` and `.../dressed`, her everyday animations in `strut/animations/entities/girls/`, her physics beside her model ([assets](doc:assets)) |
| 4 | Layered files: event bindings and everyday animations are written once for a skeleton and again only for what one character changes; every clip in a scene layout says its phase, variant and side |
| 5 | The folder of one character's own files is called `characters` (it was `owners`) |

The current format is **5**.

## How an update works

Open a pack that is in an older format and the **Update this pack** window opens by itself. It says which steps apply to this pack (only those), what it would write or move, and why. **Update the pack** does it, in place.

- A scene is converted only when the Pack Creator has checked that every bone is posed exactly as before. A file that fails that check is left as it was, and the window says why.
- The scene conversion and the layering can be undone with **Undo** in the same window. Moving files to their new folders cannot; a rename is easy to see in your own file manager.
- A copy of every file that is replaced is kept in `.packcreator/` of the project, so nothing is lost.
- Running an update on a pack that is already current does nothing.

You can open the window yourself from the top bar (**Update the pack**).

## Skeleton ids

Every model and every animation file made for it carries a **skeleton id** such as `pleasurehorizons:jenny`, which says which rig it is. The update writes them in; the **Skeleton ids** button shows them grouped by id and lets you give a girl whose model is built differently an id of her own. The game compares these ids, nothing else, to decide who may play a clip or fill a role in a scene.

## A pack from a newer Pack Creator

If a pack's format is higher than yours, the window says so and nothing is changed. Update the Pack Creator first (Settings, **Updates**, **Check now**).

## Importing

Importing an old girl ends with the same update, so what you import is already in the current layout.
