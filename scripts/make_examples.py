"""
Makes `src-tauri/resources/examples.zip`: the game's own base girls (their models, textures, animations, event bindings, chat lines, profiles and the scenes) as examples for the
Pack Creator's scene simulator and as models to learn from. Run it after the game's assets change:

    python scripts/make_examples.py [path to the Pleasure Horizons repository]

The repository is the argument, or the PH_REPO environment variable, or the default below. The zip keeps the game's own paths (`assets/pleasurehorizons/...`,
`data/pleasurehorizons/...`). Sounds are included too (14 MB), so cues play in the simulator.
"""
import os
import sys
import zipfile

HERE = os.path.abspath(os.path.join(os.path.dirname(__file__), '..'))
REPO = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else os.environ.get('PH_REPO', os.path.join(os.path.expanduser('~'), 'Documents', 'Idea Projects', 'Pleasure Horizons')))
ASSETS = os.path.join(REPO, 'src', 'main', 'resources', 'assets', 'pleasurehorizons')
DATA = os.path.join(REPO, 'src', 'main', 'resources', 'data', 'pleasurehorizons')
OUT = os.path.join(HERE, 'src-tauri', 'resources', 'examples.zip')

GIRLS = ['lucy', 'mika', 'momo', 'kobold', 'slime', 'bee_girl', 'wandering_merchant']


def add(z, base, rel, prefix):
    if not os.path.isfile(os.path.join(base, rel)):
        print('missing, left out:', rel)
        return

    z.write(os.path.join(base, rel), f'{prefix}/{rel}'.replace('\\', '/'))


with zipfile.ZipFile(OUT, 'w', zipfile.ZIP_DEFLATED, compresslevel=9) as z:
    for girl in GIRLS:
        for rel in [f'strut/models/entities/girls/nude/{girl}.strutmodel', f'strut/models/entities/girls/dressed/{girl}.strutmodel', f'textures/entities/{girl}.png', f'strut/event_bindings/girls/{girl}.json']:
            add(z, ASSETS, rel, 'assets/pleasurehorizons')

        add(z, DATA, f'girls/{girl}.json', 'data/pleasurehorizons')

    for root, _, files in os.walk(os.path.join(ASSETS, 'strut', 'animations')):
        for name in files:
            rel = os.path.relpath(os.path.join(root, name), ASSETS).replace('\\', '/')

            if '/blocks/' in rel or '/items/' in rel or '/block/' in rel or '/item/' in rel:
                continue

            add(z, ASSETS, rel, 'assets/pleasurehorizons')

    scenes = os.path.join(ASSETS, 'strut', 'scenes')

    if os.path.isdir(scenes):
        for name in os.listdir(scenes):
            add(z, ASSETS, f'strut/scenes/{name}', 'assets/pleasurehorizons')

    for name in os.listdir(os.path.join(DATA, 'scenes')):
        if name.endswith('.json'):
            add(z, DATA, f'scenes/{name}', 'data/pleasurehorizons')

    add(z, ASSETS, 'sounds.json', 'assets/pleasurehorizons')

    for root, _, files in os.walk(os.path.join(ASSETS, 'sounds')):
        for name in files:
            if name.endswith('.ogg'):
                add(z, ASSETS, os.path.relpath(os.path.join(root, name), ASSETS).replace(os.sep, '/'), 'assets/pleasurehorizons')

print(OUT, round(os.path.getsize(OUT) / 1e6, 2), 'MB')
