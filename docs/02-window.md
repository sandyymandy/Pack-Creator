# The window, piece by piece

## Home

The home page lists your recent projects, newest first. A project whose folder has gone is greyed; remove it from the list or move the folder back. **New project** asks what the pack will hold (girls, scenes, traits, dialogue, or any mix) and makes the folders; **Open a folder** opens a pack you already have, including one made by hand. Removing a project from the list never touches its folder.

## The top bar

The icon bar runs along the top of a project. Hover a button for its name. From left to right:

| Button | Does |
|---|---|
| Home | Saves everything and goes back to the projects |
| Plus | New girl, scene, trait or dialogue from a template (Ctrl+N) |
| Magnifier | Open a file by name (Ctrl+P) |
| Find | Find and replace across every file (Ctrl+Shift+F) |
| Play | **Try in a world**: copies the pack into a world's `datapacks` folder |
| Export | Writes the pack as a zip to share |
| Two people | **Who each scene is for**: one girl, or every girl on a skeleton (for packs still in the single-file layout, [formats](doc:formats)) |
| Joints | **Skeleton ids**: which rig each model and animation file is made for ([scene animations](doc:animations)) |
| Split arrows | **Update the pack**: brings an older pack to the current format, in place and with an Undo ([formats](doc:formats)). It opens by itself when you open an older pack |
| Import | Brings in an old girl from her resource pack or profile ([importing](doc:tools)) |
| Cube | Blockbench: install it, open it with the Strut Toolkit, and register the characters Strut Scenes offers ([Blockbench](doc:blockbench)) |
| Log | The game log: what the game said about this pack |
| Book | These docs (F1) |
| Question mark | Replays the tour |
| Sliders | Settings: saving, the editor, the simulator's keys, the window zoom, and **Updates** |
| Panel | Moves the file tree to the other side |

## Updates

When a newer Pack Creator is on GitHub, a note appears at the bottom right with what is new. **Download** saves the new version into your Downloads folder and shows it in Explorer; close the Pack Creator and run it. **Skip this version** stops the note for that version, and **Later** hides it until next time. The check is made when the program starts, at most twice a day, and **Check now** in Settings makes it at once.

## The file tree

The **Files** panel shows the project's folders. Click a file to open it. Right-click for New, Rename, Duplicate, Copy path, Show in Explorer and Delete. You can drag files from Explorer into a folder, drag them around inside the tree, and copy or paste them. Dropping a file whose name is already taken keeps both and numbers the new one. A coloured dot or underline on a file means the problems panel has something to say about it.

The bar between the tree and the editor can be dragged to resize; double-click it to put the size back.

## Tabs and views

Each open file is a tab. A dot on a tab means it is waiting to autosave. Right-click a tab to close the others, copy its path or show it in Explorer.

What a tab shows depends on the file:

- **A girl, scene, trait or dialogue file** opens as a **Form** made from the game's reference, with a **JSON** switch to the same file as text. The two always agree; edit in whichever is quicker.
- **Any other text file** opens in the editor with completion and hover help where the game has a schema for it.
- **A `.strutmodel` or `.strutanim`** opens in the 3D viewer.
- **A PNG** opens in the image view.
- A scene file also has **Simulate**.

## Problems

The panel at the bottom lists errors and warnings with the file and line. Click one to jump to it. The count is also in the status bar. Drag the bar above the panel to make it taller, or click its header to fold it away.

## The status bar

Shows whether everything is saved, the error and warning counts, the cursor position and the project's folder.
