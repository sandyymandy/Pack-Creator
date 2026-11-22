import type { TourStep } from './tourState';

/** The steps of the tour on the home page. */
export function homeSteps(openDocs: (page: string) => void): TourStep[] {
  return [
    {
      title: 'Welcome to the Pack Creator',
      text: 'This is an editor for packs: the girls, scenes, behaviours and conversations that Pleasure Horizons reads. This short tour shows you round. You can leave it at any time with Skip, and replay it later from here or from the top bar of a project.'
    },
    {
      target: '[data-tour="home-new"]',
      title: 'Start a project',
      text: 'A project is one data pack: a folder the game reads. New project asks what the pack will hold (girls, scenes, traits, dialogue, or any mix) and makes the folders for you.'
    },
    {
      target: '[data-tour="home-open"]',
      title: 'Open a pack you already have',
      text: 'Pick a folder to open a pack made by hand, by a friend or by an older tool. Nothing in it is changed until you edit something.'
    },
    {
      target: '[data-tour="home-projects"]',
      title: 'Your projects',
      text: 'Projects you have opened are listed here, newest first. Removing one from the list never deletes its folder.'
    },
    {
      target: '[data-tour="home-docs"]',
      title: 'The documentation',
      text: 'Guides written for pack makers, and reference pages made from the game itself so they always match the version you play. Press F1 inside a project to open it any time.',
      action: { label: 'Open "Your first pack"', run: () => openDocs('first-pack') }
    },
    {
      title: 'Updates and older packs',
      text: 'The Pack Creator looks on GitHub for a newer version when it starts and tells you at the bottom right, with what is new and a download. A pack made in an older layout opens fine: a window offers to bring it up to date, shows what it will do first, and can undo it.',
      action: { label: 'Open "Pack formats"', run: () => openDocs('formats') }
    },
    {
      title: 'Ready',
      text: 'Make a project or open one. A second, shorter tour will walk you round the workspace the first time a project opens.'
    }
  ];
}

/** The steps of the tour inside a project. */
export function workspaceSteps(openDocs: (page: string) => void): TourStep[] {
  return [
    {
      title: 'Your workspace',
      text: 'This is where a pack is built. The tour touches each part in turn. Nothing you do here is lost: everything saves as you type, and every file keeps its earlier versions.'
    },
    {
      title: 'How a scene fits together',
      text: 'A scene is played by two bodies at once: the girl on her own model, and the male on the base mod\'s male model. Each plays its own clips from its own file, and a small layout file says who may fill which role. Models and files carry a skeleton id so the game knows which rig they are for. You will meet these in the buttons below; the docs explain each one.',
      action: { label: 'Open "The male"', run: () => openDocs('male') }
    },
    {
      target: '.sidebar',
      title: 'The file tree',
      text: 'Every file of the pack. Click to open, right-click to rename, duplicate, copy the path or delete (deleted files go to the Recycle Bin). You can drag files in from Explorer and move them around, and Ctrl+Z undoes a move.'
    },
    {
      target: '[data-tour="new"]',
      title: 'Make something new',
      text: 'Ctrl+N makes a new girl, scene, trait or dialogue from a template. A new girl opens a wizard that checks her models against her skeleton and writes all her files together.'
    },
    {
      target: '[data-tour="quick"]',
      title: 'Jump to a file',
      text: 'Ctrl+P opens a file by typing part of its name. It is quicker than the tree once a pack grows.'
    },
    {
      target: '[data-tour="find"]',
      title: 'Find and replace everywhere',
      text: 'Ctrl+Shift+F searches every file in the pack. Use it to rename an id or a sound in all the places it appears.'
    },
    {
      target: '.editor-area',
      title: 'The editor',
      text: 'Girl, scene, trait and dialogue files open as a Form made from the game\'s own rules, with a JSON switch above to see the same file as text. The two always agree. Models and animations open in a 3D viewer instead, and images in an image view.'
    },
    {
      target: '.problems',
      title: 'Problems',
      text: 'The pack is checked against the game\'s rules whenever you save: a wrong value, a missing model, a cue nothing fires. Click a problem to jump to it, and look for the quick fix. Aim for "No problems found".'
    },
    {
      target: '[data-tour="play"]',
      title: 'Try it in the game',
      text: 'Copies the pack into a world\'s datapacks folder. Open the world, run /reload, and look for your girl. For scenes, the Simulate button on a scene file plays it right here first.'
    },
    {
      target: '[data-tour="export"]',
      title: 'Share it',
      text: 'Writes the pack as a zip. Others drop it into a world\'s datapacks folder, and the game sends the models and sounds to players when they join.'
    },
    {
      target: '[data-tour="scope"]',
      title: 'Who each scene is for',
      text: 'Lists every scene with the animation files it has and whose they are: one girl\'s, or every girl on a skeleton. For packs still in the single-file layout it can make a file skeleton-wide or hers only.'
    },
    {
      target: '[data-tour="rig"]',
      title: 'Skeleton ids',
      text: 'Every model, and every animation made for it, carries the id of its rig, such as pleasurehorizons:jenny. This groups your girls by id and writes the ids in. Give a girl whose model is built differently an id of her own.',
      action: { label: 'Open "Scene animations"', run: () => openDocs('animations') }
    },
    {
      target: '[data-tour="update"]',
      title: 'Update the pack',
      text: 'Brings an older pack to the current layout, in place: it shows what it will do before it does anything, converts only what it has checked is posed exactly as before, and can undo the conversion. It opens by itself when you open an older pack.',
      action: { label: 'Open "Pack formats"', run: () => openDocs('formats') }
    },
    {
      target: '[data-tour="import"]',
      title: 'Bring in an old girl',
      text: 'Converts a girl from an older resource pack or profile into a pack here, brings her up to the current layout, and reports what it changed.'
    },
    {
      target: '[data-tour="blockbench"]',
      title: 'Blockbench and Strut Scenes',
      text: 'Installs Blockbench with the Strut Toolkit for making models, animations and whole scenes. The viewer\'s Edit in Blockbench opens the file you are looking at. Register actors gives Blockbench the game\'s characters, the male and your own girls, so a scene with several characters is a few clicks.',
      action: { label: 'Open "Blockbench and Strut Scenes"', run: () => openDocs('blockbench') }
    },
    {
      target: '[data-tour="log"]',
      title: 'The game log',
      text: 'Run the game, then open this: it lists what the game said about your pack, tied to the file each line names. It is the first place to look when something works here but not in the game.'
    },
    {
      target: '[data-tour="docs"]',
      title: 'Docs, any time',
      text: 'F1 opens the guides and the game\'s reference. Each kind of file has a guide with an example and a list of common problems.',
      action: { label: 'Open "Your first pack"', run: () => openDocs('first-pack') }
    },
    {
      target: '[data-tour="tour"]',
      title: 'Replay this tour',
      text: 'This button starts the tour again whenever you want it. The home page has one too.'
    },
    {
      target: '[data-tour="settings"]',
      title: 'Settings',
      text: 'How fast files save, the editor\'s look, the scene simulator\'s keys and sound, the zoom of the whole window, and Check now for a newer Pack Creator.'
    },
    {
      target: '.statusbar',
      title: 'The status bar',
      text: 'Shows whether everything is saved, how many errors and warnings the pack has, and where your cursor is.'
    },
    {
      title: 'You are set',
      text: 'Try the walk-through: it takes you from an empty project to a girl in the game. If a problem names a code, the checks page says what it means and what to do.',
      action: { label: 'Open "Your first pack"', run: () => openDocs('first-pack') }
    }
  ];
}
