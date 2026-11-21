/**
 * A small in-memory stand-in for the Rust core, used only when the page is opened in a plain browser (developing the layout without the window). It keeps one
 * pretend project, checks JSON syntax and one rule, and answers the rest with believable shapes. The real checks are in pc_core.
 */
import type { Diagnostic, Entry, FileKind } from './types';

type Args = Record<string, unknown>;

const files = new Map<string, string>();
const recent = [
  { path: 'C:/Packs/Amber Pack', name: 'Amber Pack', last_opened: Math.floor(Date.now() / 1000) - 3600, exists: true },
  { path: 'C:/Packs/Dialogue Only', name: 'Dialogue Only', last_opened: Math.floor(Date.now() / 1000) - 86400 * 4, exists: true },
  { path: 'D:/Old/Gone Pack', name: 'Gone Pack', last_opened: Math.floor(Date.now() / 1000) - 86400 * 30, exists: false }
];
let meta = { format: 1, name: 'Amber Pack', namespace: 'pleasurehorizons', author: 'me', description: 'A pack of one girl.', contents: ['girls', 'traits'], created: 0 };

function seed(): void {
  files.clear();
  files.set('pack.mcmeta', '{\n  "pack": {\n    "pack_format": 48,\n    "description": "A pack of one girl."\n  }\n}\n');
  files.set('data/pleasurehorizons/girls/amber.json', '{\n  "id": "amber",\n  "type": "settlr",\n  "components": {\n    "pleasurehorizons:identity": { "name": "Amber" },\n    "pleasurehorizons:body": { "body_type": "jenny", "hitbox_height": 1.8 },\n    "pleasurehorizons:taste": { "tame_item": "minecraft:allium" }\n  }\n}\n');
  files.set('data/pleasurehorizons/girl_traits/shy_fox.json', '{\n  "category": "temperament",\n  "description": "She hides.",\n  "goals": []\n}\n');
  files.set('data/pleasurehorizons/scenes/hug.json', '{\n  "type": "on_player"\n}\n');
  files.set('data/pleasurehorizons/asset_sync/textures/entities/amber.png', 'png');
  files.set('data/pleasurehorizons/asset_sync/strut/models/entities/girls/nude/amber.strutmodel', 'STMD');
  files.set('data/pleasurehorizons/asset_sync/strut/animations/entities/girls/base.amber.strutanim', 'STAM');
  files.set('data/pleasurehorizons/asset_sync/strut/physics/models/entities/girls/nude/amber.json', '{\n  "version": 1,\n  "bones": {\n    "head": { "preset": "jiggle-soft", "frequency": 1.6, "damping": 0.18, "length": 0.3, "limit": { "x": [-40, 40], "z": [-40, 40] } },\n    "body": { "preset": "jiggle-firm", "length": 0.5 },\n    "tail": { "preset": "tail" }\n  },\n  "chains": { "hair": { "bones": ["h1"] } }\n}\n');
}

seed();

function kindOf(rel: string): FileKind {
  if (rel === 'pack.mcmeta') return 'pack_meta';
  if (rel.includes('/girls/')) return 'girl';
  if (rel.includes('/scenes/')) return 'scene';
  if (rel.includes('/girl_traits/')) return 'trait';
  if (rel.includes('/dialogue/')) return 'dialogue';
  if (rel.includes('/social/')) return 'social';
  if (rel.endsWith('.png')) return 'texture';
  if (rel.endsWith('.strutmodel')) return 'model';
  if (rel.endsWith('.strutanim')) return 'animation';
  return 'other';
}

function diagnose(rel: string, text: string): Diagnostic[] {
  if (!rel.endsWith('.json') && !rel.endsWith('.mcmeta')) return [];

  const make = (code: string, message: string, line: number, column: number, endColumn: number, fix: Diagnostic['fix'] = null, severity: Diagnostic['severity'] = 'error'): Diagnostic =>
    ({ severity, code, message, file: rel, line, column, end_line: line, end_column: endColumn, fix });

  try {
    JSON.parse(text);
  } catch (error) {
    const at = Number(/position (\d+)/.exec(String(error))?.[1] ?? 0);
    const before = text.slice(0, at).split('\n');

    return [make('syntax', String(error).replace(/^SyntaxError: /, ''), before.length, before[before.length - 1].length + 1, before[before.length - 1].length + 2)];
  }

  const found: Diagnostic[] = [];
  const lines = text.split('\n');

  lines.forEach((line, index) => {
    const hit = /"type":\s*"settlr"/.exec(line);

    if (hit && kindOf(rel) === 'girl') {
      const column = line.indexOf('"settlr"') + 1;

      found.push(make('unknown-type', '"settlr" is not a kind of girl. Use one of: merchant, settler, wild. Did you mean "settler"?', index + 1, column, column + 8, { title: 'Change to "settler"', replacement: '"settler"' }));
    }
  });

  return found;
}

const delay = <T>(value: T): Promise<T> => new Promise(resolve => setTimeout(() => resolve(value), 40));

function children(rel: string): Entry[] {
  const prefix = rel ? rel + '/' : '';
  const seen = new Map<string, Entry>();

  for (const [path, body] of files) {
    if (!path.startsWith(prefix)) continue;

    const rest = path.slice(prefix.length);
    const name = rest.split('/')[0];
    const isDir = rest.includes('/');

    if (!seen.has(name)) seen.set(name, { name, rel: prefix + name, is_dir: isDir, kind: isDir ? 'other' : kindOf(path), size: isDir ? 0 : body.length });
  }

  return [...seen.values()].sort((a, b) => Number(b.is_dir) - Number(a.is_dir) || a.name.localeCompare(b.name));
}

const girlSchema = {
  type: 'object',
  properties: {
    id: { type: 'string', description: 'A short name in lower case letters, digits and underscores.' },
    type: { enum: ['merchant', 'settler', 'wild'], description: 'What kind of girl she is.' },
    components: {
      type: 'object',
      properties: {
        'pleasurehorizons:body': { type: 'object', properties: { body_type: { enum: ['jenny', 'bia', 'ellie', 'luna'], description: 'The base model her animations are authored against.' }, hitbox_height: { type: 'number', description: 'Her height in blocks.' } } },
        'pleasurehorizons:identity': { type: 'object', properties: { name: { type: 'string', description: 'Her name.' } } },
        'pleasurehorizons:taste': { type: 'object', properties: { tame_item: { type: 'string', description: 'The item that tames her.' } } }
      }
    }
  }
};

export async function mock(command: string, args: Args = {}): Promise<unknown> {
  switch (command) {
    case 'list_recent': return delay(recent);
    case 'forget_recent': recent.splice(recent.findIndex(r => r.path === args.path), 1); return delay(null);
    case 'default_projects_dir': return delay('C:/Users/You/Documents/Pleasure Horizons Packs');
    case 'contents_options': return delay([{ id: 'girls', label: 'Girls (with their models, textures and sounds)' }, { id: 'scenes', label: 'Scenes' }, { id: 'traits', label: 'Traits' }, { id: 'dialogue', label: 'Dialogue' }, { id: 'social', label: 'Small talk on breaks' }]);
    case 'namespace_problem': {
      const ns = String(args.namespace);

      return delay(ns === '' ? 'The namespace cannot be empty.' : /^[a-z0-9_.-]+$/.test(ns) ? (ns === 'minecraft' ? '"minecraft" belongs to Minecraft and cannot be used.' : null) : 'The namespace uses lower case letters, digits and _ - . only.');
    }
    case 'create_project': {
      const request = args.request as { name: string; namespace: string; author: string; description: string; contents: string[] };

      seed();
      meta = { format: 1, name: request.name, namespace: request.namespace, author: request.author, description: request.description, contents: request.contents, created: 0 };
      files.clear();
      files.set('pack.mcmeta', `{\n  "pack": {\n    "pack_format": 48,\n    "description": "${request.description || request.name}"\n  }\n}\n`);
      return delay({ root: 'C:/Packs/' + request.name, meta });
    }
    case 'open_project': seed(); return delay({ root: String(args.path), meta });
    case 'close_project': return delay(null);
    case 'list_dir': return delay(children(String(args.rel)));
    case 'list_files': return delay([...files.keys()]);
    case 'locate_file': return delay({ kind: kindOf(String(args.rel)), namespace: null, id: null });
    case 'read_file': {
      const text = files.get(String(args.rel));

      return text === undefined ? Promise.reject(new Error('Not found.')) : delay({ type: 'text', text });
    }
    case 'export_zip': return delay(files.size);
    case 'install_pack': return delay('C:/Users/You/AppData/Roaming/.minecraft/saves/World/datapacks/' + meta.name);
    case 'saves_dir': return delay('C:/Users/You/AppData/Roaming/.minecraft/saves');
    case 'stamps': return delay((args.rels as string[]).map(rel => (files.has(rel) ? String(files.get(rel)!.length) : null)));
    case 'read_image': return delay('data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==');
    case 'write_file': {
      const rel = String(args.rel);
      const text = String(args.textValue);

      files.set(rel, text);
      return delay({ located: { kind: kindOf(rel), namespace: null, id: null }, diagnostics: diagnose(rel, text), stamp: String(text.length) });
    }
    case 'check_text': return delay(diagnose(String(args.rel), String(args.textValue)));
    case 'check_project': return delay([...files].flatMap(([rel, text]) => diagnose(rel, text)));
    case 'create_entry': files.set(String(args.rel) + (args.isDir ? '/.keep' : ''), ''); return delay(null);
    case 'rename_entry': {
      for (const [path, body] of [...files]) {
        if (path === args.from || path.startsWith(args.from + '/')) {
          files.delete(path);
          files.set(String(args.to) + path.slice(String(args.from).length), body);
        }
      }

      return delay(null);
    }
    case 'delete_entry': for (const path of [...files.keys()]) if (path === args.rel || path.startsWith(args.rel + '/')) files.delete(path); return delay(null);
    case 'new_file': {
      const request = args.request as { kind: string; id: string; name?: string };
      const folder = { girl: 'girls', scene: 'scenes', trait: 'girl_traits', dialogue: 'dialogue', social: 'social' }[request.kind] ?? 'misc';
      const rel = `data/${meta.namespace}/${folder}/${request.id}.json`;

      files.set(rel, request.kind === 'girl' ? `{\n  "id": "${request.id}",\n  "type": "settler",\n  "components": {}\n}\n` : '{}\n');
      return delay(rel);
    }
    case 'choices': return delay({ archetypes: ['merchant', 'settler', 'wild'], body_types: ['jenny', 'bia', 'ellie', 'luna', 'goblin', 'galath', 'kobold', 'copper_golem'], scene_types: ['on_player', 'on_bed', 'stationary'], trait_categories: ['movement', 'temperament', 'combat', 'interaction', 'routine'] });
    case 'import_profile': {
      files.set('data/pleasurehorizons/girls/imported.json', '{\n  "id": "imported",\n  "type": "settler",\n  "components": {}\n}\n');
      return delay({ girl: 'data/pleasurehorizons/girls/imported.json', written: ['data/pleasurehorizons/girls/imported.json'], notes: [{ level: 'kept', message: 'Imported (mock).' }, { level: 'dropped', message: '"gui_size" is not used any more.' }, { level: 'review', message: 'Her model is not imported yet.' }] });
    }
    case 'old_pack_cues': return delay([{ token: 'thrust', count: 4 }, { token: 'hugmoan', count: 2 }]);
    case 'find_profiles': return delay([null, null]);
    case 'old_pack_look': return delay({ id: args.girl, models: ['nude', 'dressed'], texture: true, clips: ['idle', 'walk', 'doggy_intro', 'doggy_slow', 'doggy_fast', 'doggy_cum', 'ride_slow', 'ride_fast', 'wave'], scenes: [{ name: 'Doggy', scene_type: 'on_bed', parts: { intro: ['doggy_intro'], slow: ['doggy_slow'], fast: ['doggy_fast'], cum: ['doggy_cum'] } }, { name: 'Ride', scene_type: 'on_player', parts: { slow: ['ride_slow'], fast: ['ride_fast'] } }, { name: 'Wave', scene_type: 'stationary', parts: { loop: ['wave'] } }], scenes_from: 'names', files_layout: false, bones: ['root', 'body', 'head', 'boobL', 'boobR'] });
    case 'skeleton_plan': return { assignments: [], groups: [{ id: 'pleasurehorizons:jenny', models: ['jenny', 'amber'], animations: 4 }], unreadable: [] };
    case 'skeleton_apply': return { written: 6, already: 0, failed: [] };
    case 'migration_status': return { format: 2, current: 2, needed: false, reason: null, steps: [], newer: false };
    case 'split_scenes': return { applied: !!args.apply, stamp: args.apply ? '1' : null, scenes: 3, files_written: 12, files_removed: 6, models_stripped: 2, dropped_bones: [], approximate: [], skipped: [], findings: [] };
    case 'undo_split_scenes': return 12;
    case 'old_pack_model': return mock('read_model', {});
    case 'old_pack_texture': return delay('data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==');
    case 'old_pack_clips': return Promise.all(['idle', 'walk', 'doggy_intro', 'doggy_slow', 'doggy_fast', 'doggy_cum', 'ride_slow', 'ride_fast', 'wave'].map(async name => ({ ...((await mock('read_animation', {})) as { name: string }[])[0], name })));
    case 'guess_body_type': return delay('jenny');
    case 'detect_scenes': return delay([{ name: 'Doggy', scene_type: 'on_player', parts: { slow: ['doggy_slow'], fast: ['doggy_fast'] } }]);
    case 'old_pack_girls': return delay({ girls: ['zoe', 'amy'], namespace: 'oldns', rewritten: false });
    case 'convert_file': return delay({ girl: '', written: ['data/pleasurehorizons/asset_sync/strut/models/entities/girls/nude/zoe.strutmodel'], notes: [{ level: 'kept', message: 'The model: 12 bones, 30 cubes.' }] });
    case 'import_old_pack': {
      files.set('data/pleasurehorizons/girls/zoe.json', '{\n  "id": "zoe",\n  "type": "settler",\n  "components": {}\n}\n');
      return delay({ girl: 'data/pleasurehorizons/girls/zoe.json', written: [], notes: [{ level: 'kept', message: 'The nude model: 40 bones, 120 cubes.' }, { level: 'kept', message: 'Her animations: 12 base animation(s) and 3 scene file(s).' }, { level: 'error', message: 'The scene "Hug" uses the animation "x", but her animation file has no such animation.' }] });
    }
    case 'viewer_files': return delay({ id: 'amber', models: ['data/pleasurehorizons/asset_sync/strut/models/entities/girls/nude/amber.strutmodel'], texture: null, animations: ['data/pleasurehorizons/asset_sync/strut/animations/entities/girls/base.amber.strutanim'] });
    case 'add_basic_physics': return delay({ path: 'data/pleasurehorizons/asset_sync/strut/physics/models/entities/girls/nude/amber.json', added: ['boobL', 'boobR', 'assL', 'assR'] });
    case 'read_model': return delay({
      version: 101, unitsPerBlock: 16, boundsMin: [0, 0, 0], boundsMax: [0, 0, 0], textures: [{ name: 't', width: 64, height: 64, uvWidth: 64, uvHeight: 64 }],
      groupBones: [{ name: 'body', parentIndex: -1, pivot: [0, 12, 0], rotation: [0, 0, 0], elements: [{ kind: 1, index: 0 }] }, { name: 'head', parentIndex: 0, pivot: [0, 24, 0], rotation: [0, 0, 0], elements: [{ kind: 1, index: 1 }] }],
      meshes: [],
      cubes: [
        { name: 'b', textureIndex: 0, origin: [0, 12, 0], rotation: [0, 0, 0], from: [-4, 0, -2], to: [4, 12, 2], inflate: 0, mirror: false, boxUv: false, faces: [0, 1, 2, 3, 4, 5].map(faceId => ({ faceId, u1: 0, v1: 0, u2: 8, v2: 12, uvRotation: 0 })) },
        { name: 'h', textureIndex: 0, origin: [0, 24, 0], rotation: [0, 0, 0], from: [-4, 24, -4], to: [4, 32, 4], inflate: 0, mirror: false, boxUv: false, faces: [0, 1, 2, 3, 4, 5].map(faceId => ({ faceId, u1: 0, v1: 0, u2: 8, v2: 8, uvRotation: 0 })) }
      ],
      armatureCount: 0, armatures: [], extensions: []
    });
    case 'blockbench_status': return delay({ exe: 'C:/Program Files/Blockbench/Blockbench.exe', version: '5.2.2', versionOk: true, running: false, toolkitVersion: '4.6.1', installing: '' });
    case 'prepare_blockbench': case 'open_in_blockbench': case 'install_blockbench': return delay(null);
    case 'game_log_default': return delay('C:/Users/me/AppData/Roaming/.minecraft/logs/latest.log');
    case 'read_game_log': return delay({ path: 'latest.log', ageSeconds: 120, offer: { time: '17:01:47', files: 8, bytes: 1327142 }, packFiles: 8, entries: [
      { time: '17:01:47', level: 'ERROR', source: 'Pleasure Horizons', message: 'ERROR pleasurehorizons:girls/zoe.json at components.pleasurehorizons:taste.tame_item: The tame item is already used.', resource: 'pleasurehorizons:girls/zoe.json', count: 1, file: 'data/pleasurehorizons/girls/zoe.json' },
      { time: '17:01:48', level: 'WARN', source: 'Minecraft', message: 'Missing sound for event: pleasurehorizons:misc.flap', resource: null, count: 2, file: null }] });
    case 'file_history': return delay([{ id: Date.now() - 60000, size: 400 }, { id: Date.now() - 3600000, size: 380 }]);
    case 'read_version': return delay('{\n  "id": "older"\n}\n');
    case 'bundled_bed': return delay({ model: { version: 101, unitsPerBlock: 16, boundsMin: [0, 0, 0], boundsMax: [0, 0, 0], textures: [], groupBones: [], meshes: [], cubes: [], armatureCount: 0, armatures: [], extensions: [] }, texture: '' });
    case 'check_girl_models': return delay({ nude: { ok: true, message: '40 bones, 120 cubes' }, dressed: { ok: true, message: '40 bones, 130 cubes' }, texture: '64 x 64', skeleton: { reference: 'lucy', same: true, missing: [], extra: [], order_differs: false } });
    case 'create_girl': return delay({ girl: 'data/pleasurehorizons/girls/new.json', written: ['data/pleasurehorizons/girls/new.json'], notes: [] });
    case 'example_preview': return delay(null);
    case 'search_text': return delay([{ rel: 'data/pleasurehorizons/girls/amber.json', line: 3, column: 7, preview: '  "name": "Amber"', start: 10, length: 5 }]);
    case 'replace_in_files': return delay(1);
    case 'duplicate_entry': return delay('copy.json');
    case 'reveal_in_explorer': return delay(null);
    case 'sim_girls': return delay([{ id: 'amber', file: 'data/pleasurehorizons/girls/amber.json', body_type: 'jenny', example: false, has_model: true }]);
    case 'example_girls': return delay([{ id: 'lucy', file: 'example:data/pleasurehorizons/girls/lucy.json', body_type: 'jenny', example: true, has_model: true }]);
    case 'example_scenes': return delay([{ id: 'doggy', file: 'example:data/pleasurehorizons/scenes/doggy.json' }]);
    case 'sim_files': return delay({ nude: null, dressed: null, texture: null, animations: [], binding_layers: [], male_animations: [], bindings: null, sounds_json: [] });
    case 'read_asset': return delay('');
    case 'cue_names': return delay(['pound', 'moan', 'thrust', 'cum']);
    case 'save_clip_events': return delay([]);
    case 'delete_clips': return delay([]);
    case 'scene_scopes': return delay([
      { id: 'hug', file: 'data/pleasurehorizons/scenes/hug.json', supported_girls: ['amber'], body_types: ['jenny'], files: [{ rel: 'data/pleasurehorizons/asset_sync/strut/animations/scenes/hug-amber.strutanim', owner_kind: 'girl', owner: 'amber' }] },
      { id: 'doggy', file: 'data/pleasurehorizons/scenes/doggy.json', supported_girls: [], body_types: ['jenny'], files: [{ rel: 'data/pleasurehorizons/asset_sync/strut/animations/scenes/doggy.jenny.strutanim', owner_kind: 'skeleton', owner: 'jenny' }] }
    ]);
    case 'scene_make_skeleton_wide': case 'scene_make_girl_specific': return delay(['Done (mock).']);
    case 'read_animation': return delay([{
      name: 'animation.base.wave', duration: 2, loopMode: 1,
      channels: [{ target: 'head', kind: 'GROUP_BONE', property: 'ROTATION', keys: [0, 1, 2].map(t => ({ time: t, interpolation: 1, value: { literals: [0, t === 1 ? 40 : 0, 0], expressions: [null, null, null] }, post: null, easing: t === 1 ? 'easeInOutSine' : null, easingArgs: [] })) }],
      events: [{ time: 1, kind: 'instruction', payload: 'pound-moan', authority: 0 }, { time: 1.5, kind: 'sound', payload: 'pleasurehorizons:misc.pounding', authority: 0 }]
    }]);
    case 'reference': return delay({
      archetypes: { 'pleasurehorizons:settler': ['pleasurehorizons:body', 'pleasurehorizons:identity', 'pleasurehorizons:taste'], 'pleasurehorizons:wild': ['pleasurehorizons:body', 'pleasurehorizons:identity', 'pleasurehorizons:taste'] },
      goals: { 'pleasurehorizons:wander': [{ name: 'speed', kind: 'number', required: false, description: 'How fast.', min: 0.1, max: 4, options: [], default: 1, of: null }], 'pleasurehorizons:swim': [] },
      goal_conditions: ['pleasurehorizons:untamed', 'pleasurehorizons:not_sleeping'],
      capabilities: { movement: { name: 'movement', kind: 'movement', required: false, description: 'How she gets around.', min: null, max: null, options: ['ground', 'flying'], default: null, of: null }, fall_damage: { name: 'fall_damage', kind: 'boolean', required: false, description: 'Whether a fall hurts her.', min: null, max: null, options: [], default: null, of: null } },
      taste_tags: ['kind', 'bold', 'calm'],
      dialogue_topics: ['chat', 'flirt'],
      dialogue_ids: ['about_past'],
      scene_fields: [
        { name: 'type', kind: 'choice', required: false, description: 'Where and how the scene is played.', min: null, max: null, options: ['on_player', 'on_bed'], default: null, of: null },
        { name: 'requires_affection', kind: 'integer', required: false, description: 'Affection needed.', min: 0, max: 10, options: [], default: null, of: null },
        { name: 'needs_to_strip', kind: 'boolean', required: false, description: 'Undress first.', min: null, max: null, options: [], default: null, of: null },
        { name: 'body_types', kind: 'list', required: false, description: 'Body types it fits.', min: null, max: null, options: ['jenny', 'ellie'], default: null, of: 'body_type' }
      ],
      categories: { movement: { name: 'movement', description: 'How she gets around.', several: false }, interaction: { name: 'interaction', description: 'What a player can do.', several: true } },
      traits: {
        'pleasurehorizons:walker': { id: 'pleasurehorizons:walker', category: 'movement', description: 'On foot.', params: [{ name: 'speed', kind: 'number', required: false, description: 'How fast.', min: 0.1, max: 4, options: [], default: 1, of: null }], stacks: ['goals'] },
        'pleasurehorizons:flier': { id: 'pleasurehorizons:flier', category: 'movement', description: 'Flies.', params: [], stacks: ['goals'] },
        'pleasurehorizons:collar': { id: 'pleasurehorizons:collar', category: 'interaction', description: 'Collar.', params: [], stacks: ['goals'] }
      },
      components: {
        'pleasurehorizons:identity': { id: 'pleasurehorizons:identity', description: 'Who she is.', required: true, fields: [{ name: 'name', kind: 'text', required: true, description: 'Her name.', min: null, max: null, options: [], default: null, of: null }, { name: 'egg_primary', kind: 'colour', required: false, description: 'Egg colour.', min: null, max: null, options: [], default: '#FFFFFF', of: null }] },
        'pleasurehorizons:body': { id: 'pleasurehorizons:body', description: 'Her shape.', required: true, fields: [{ name: 'body_type', kind: 'choice', required: true, description: 'Skeleton.', min: null, max: null, options: ['jenny', 'ellie'], default: null, of: null }, { name: 'hitbox_height', kind: 'number', required: false, description: 'Height.', min: 0.3, max: 5, options: [], default: 1.65, of: null }, { name: 'held_item_offset', kind: 'vec3', required: false, description: 'Offset.', min: null, max: null, options: [], default: [0, 0, 0], of: null }, { name: 'shading', kind: 'choice', required: false, description: 'How she is shaded: soft, vanilla, toon or flat. default uses the setting the player chose.', min: null, max: null, options: ['default', 'soft', 'vanilla', 'toon', 'flat'], default: 'default', of: null }] },
        'pleasurehorizons:taste': { id: 'pleasurehorizons:taste', description: 'What draws her.', required: true, fields: [{ name: 'tame_item', kind: 'item', required: true, description: 'Tame item.', min: null, max: null, options: [], default: null, of: null }] },
        'pleasurehorizons:ai': { id: 'pleasurehorizons:ai', description: 'How she behaves.', required: false, fields: [{ name: 'traits', kind: 'object', required: false, description: 'Traits.', min: null, max: null, options: [], default: {}, of: null }] }
      }
    });
    case 'schema_for': return delay(kindOf(String(args.rel)) === 'girl' ? girlSchema : null);
    default: return Promise.reject(new Error('The mock has no ' + command));
  }
}
