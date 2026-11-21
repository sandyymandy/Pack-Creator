import { useEffect, useMemo, useRef, useState } from 'preact/hooks';
import { api } from '../api';
import { withPartner, withPartnerClips } from './partner';
import { messageOf, notify } from '../notify';
import { getSettings, keyName } from '../settings';
import type { ClipData, ModelData, SimFiles, SimGirl } from '../types';
import { createViewer, type Viewer } from '../viewer/scene';
import { SceneSim, TICK_MS, type CueLog, type SceneInfo, type SceneType, type Stage } from './engine';
import { deriveFeel } from './feel';
import { DEFAULT_PACE, type PaceSettings } from './pace';
import { SceneCatalogue, ROLES, parseClipName, type AnimationFile } from './roles';

/** The pieces of a girl's files the simulation reads, loaded once for her. */
interface Assets {
  files: SimFiles;
  nude: ModelData | null;
  dressed: ModelData | null;
  texture: string | null;
  animations: AnimationFile[];
  bindings: Record<string, Record<string, unknown>>;
  /** The sound events the pack defines: `namespace:name` to the sound files it picks from. */
  sounds: Record<string, string[]>;
}

interface Effect {
  kind: 'sound' | 'chat' | 'note';
  text: string;
}

interface LogEntry {
  id: number;
  age: number;
  marker: string;
  phase: string;
  clip: string | null;
  effects: Effect[];
}

const SCENE_TYPES: SceneType[] = ['on_player', 'on_bed', 'stationary_contact', 'stationary_intro', 'stationary', 'downed', 'player_downed', 'player_animation'];

const isObject = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value);

/** The scene as it is for one girl: the file's settings with her overrides on top (`girl_overrides`). */
function infoFor(sceneId: string, scene: Record<string, unknown>, girlId: string, minigame: 'classic' | 'pace' = 'classic'): SceneInfo {
  const authored = isObject(scene.minigame) ? (scene.minigame as Record<string, unknown>) : {};
  const paceOverrides: Partial<PaceSettings> = {};
  const pair = (value: unknown): [number, number] | undefined => (Array.isArray(value) && value.length === 2 && value.every(v => typeof v === 'number') ? [value[0] as number, value[1] as number] : undefined);
  const positive = (value: unknown): number | undefined => (typeof value === 'number' && value > 0 ? value : undefined);

  if (pair(authored.slow_band)) paceOverrides.slowBand = pair(authored.slow_band);
  if (pair(authored.fast_band)) paceOverrides.fastBand = pair(authored.fast_band);
  if (positive(authored.slow_seconds)) paceOverrides.slowSeconds = positive(authored.slow_seconds);
  if (positive(authored.fast_seconds)) paceOverrides.fastSeconds = positive(authored.fast_seconds);
  if (positive(authored.blend_seconds)) paceOverrides.blendSeconds = positive(authored.blend_seconds);
  if (positive(authored.fill_seconds)) paceOverrides.fillSeconds = positive(authored.fill_seconds);
  const overrides = isObject(scene.girl_overrides) && isObject(scene.girl_overrides[girlId]) ? (scene.girl_overrides[girlId] as Record<string, unknown>) : {};
  const get = (key: string): unknown => (key in overrides ? overrides[key] : scene[key]);
  const type = String(get('type') ?? 'on_player') as SceneType;

  return {
    id: sceneId,
    type: SCENE_TYPES.includes(type) ? type : 'on_player',
    needsToStrip: get('needs_to_strip') === true,
    cumThreshold: typeof get('cum_threshold') === 'number' ? (get('cum_threshold') as number) : 6,
    useKeyFrameEvents: get('use_keyframe_events') === true,
    replaceDefault: get('replace_default_animations') !== false,
    bedOffset: typeof get('bed_alignment_offset') === 'number' ? (get('bed_alignment_offset') as number) : 0,
    amountOfLoops: typeof get('amount_of_loops') === 'number' ? (get('amount_of_loops') as number) : 1,
    minigame,
    pace: DEFAULT_PACE,
    paceOverrides
  };
}

/** The scene's settings for one girl: the file's, with her overrides on top. */
function settingsOf(scene: Record<string, unknown>, girlId: string): Record<string, unknown> {
  const overrides = isObject(scene.girl_overrides) && isObject(scene.girl_overrides[girlId]) ? (scene.girl_overrides[girlId] as Record<string, unknown>) : {};

  return { ...scene, ...overrides };
}

/**
 * Whether the game would offer a scene to a girl: the scene names her (or does not exclude her), and her skeleton is one the scene's animations were made for
 * (`SceneDefinition#fits`: a scene that names no body types fits all).
 */
export function compatible(scene: Record<string, unknown> | null, girl: { id: string; body_type: string | null }): boolean {
  if (!scene) return true;

  const settings = settingsOf(scene, girl.id);
  const list = (value: unknown): string[] | null => (Array.isArray(value) ? value.map(String) : null);
  const supported = list(scene.supported_girls);
  const unsupported = list(scene.unsupported_girls);
  const bodies = list(settings.body_types) ?? [];

  if (supported && !supported.includes(girl.id)) return false;
  if (!supported && unsupported && unsupported.includes(girl.id)) return false;

  return bodies.length === 0 || (girl.body_type !== null && bodies.includes(girl.body_type));
}

interface SceneChoice {
  id: string;
  file: string;
  example: boolean;
  json: Record<string, unknown>;
}

async function readJson(rel: string | null | undefined): Promise<Record<string, unknown>> {
  if (!rel) return {};

  try {
    const content = await api.readFile(rel);

    return content.type === 'text' ? ((JSON.parse(content.text) as Record<string, unknown>) ?? {}) : {};
  } catch {
    return {};
  }
}

async function loadAssets(sceneId: string, girlId: string, example: boolean, bodyType: string | null): Promise<Assets> {
  const files = await api.simFiles(sceneId, girlId, example, bodyType);
  const [girlNude, girlDressed, textureUrl, ownAnimations, partnerFiles, bindingsFile] = await Promise.all([
    files.nude ? api.readModel(files.nude).catch(() => null) : Promise.resolve(null),
    files.dressed ? api.readModel(files.dressed).catch(() => null) : Promise.resolve(null),
    files.texture ? api.readImage(files.texture).catch(() => null) : Promise.resolve(null),
    Promise.all(files.animations.map(async path => ({ path, clips: await api.readAnimation(path).catch(() => [] as ClipData[]) }))),
    Promise.all((files.partner_animations ?? []).map(async path => ({ path, clips: await api.readAnimation(path).catch(() => [] as ClipData[]) }))),
    readJson(files.bindings)
  ]);

  /* A scene in the new layout plays the girl and the partner together: the partner's model and clips are joined to hers for drawing. */
  const partner = partnerFiles.length > 0 ? await api.bundledPartner().catch(() => null) : null;
  const nude = girlNude && partner ? withPartner(girlNude, partner) : girlNude;
  const dressed = girlDressed && partner ? withPartner(girlDressed, partner) : girlDressed;
  const animations = partner ? withPartnerClips(ownAnimations, partnerFiles, partner) : ownAnimations;

  const sounds: Record<string, string[]> = {};

  for (const rel of files.sounds_json) {
    const namespace = /data\/([^/]+)\//.exec(rel)?.[1] ?? 'pleasurehorizons';
    const defined = await readJson(rel);

    for (const [name, entry] of Object.entries(defined)) {
      const list = isObject(entry) && Array.isArray(entry.sounds) ? entry.sounds : [];

      sounds[`${namespace}:${name}`] = list.map(item => (typeof item === 'string' ? item : isObject(item) ? String(item.name ?? '') : '')).filter(Boolean);
    }
  }

  return {
    files,
    nude,
    dressed,
    texture: textureUrl,
    animations,
    bindings: isObject(bindingsFile.bindings) ? (bindingsFile.bindings as Record<string, Record<string, unknown>>) : {},
    sounds
  };
}

/**
 * A scene played as the game plays it: she starts dressed, strips on the real cue if the scene needs it, goes to the bed and lies down, idles until she is clicked, and then
 * her clips follow the cues and the thrusting through the game's own planner. The cues are the real ones: what each is bound to (sounds, chat lines) plays and is listed.
 */
export function SimView({ sceneRel, girlRel }: { sceneRel: string; girlRel?: string }) {
  const host = useRef<HTMLDivElement>(null);
  const viewer = useRef<Viewer | null>(null);
  const sim = useRef<SceneSim | null>(null);
  const assets = useRef<Assets | null>(null);
  const audio = useRef(new Map<string, string>());
  const soundOn = useRef(true);
  const nextId = useRef(1);
  const [girls, setGirls] = useState<SimGirl[]>([]);
  const [scenes, setScenes] = useState<SceneChoice[]>([]);
  const [girlFile, setGirlFile] = useState('');
  const [sceneFile, setSceneFile] = useState('');
  const [problem, setProblem] = useState('');
  const [log, setLog] = useState<LogEntry[]>([]);
  const [chat, setChat] = useState<string[]>([]);
  const [muted, setMuted] = useState(!getSettings().simSound);
  const [minigame, setMinigame] = useState<'classic' | 'pace'>('pace');
  const [feelText, setFeelText] = useState('');
  const [, refresh] = useState(0);
  const scheduled = useRef(false);
  const chosenScene = scenes.find(c => c.file === sceneFile) ?? null;
  const chosenGirl = girls.find(g => g.file === girlFile) ?? null;
  const sceneId = chosenScene?.id ?? '';
  const sceneJson = chosenScene?.json ?? null;
  const girlId = chosenGirl?.id ?? '';

  soundOn.current = !muted;

  const redraw = () => {
    if (scheduled.current) return;

    scheduled.current = true;
    window.setTimeout(() => { scheduled.current = false; refresh(n => n + 1); }, 40);
  };

  /* ---------- What the scene and the girls are ---------- */

  useEffect(() => {
    void (async () => {
      try {
        const [mine, examples, files, exampleScenes] = await Promise.all([api.simGirls(), api.exampleGirls(), api.listFiles(), api.exampleScenes()]);
        const packScenes = files.filter(f => /^data\/[^/]+\/scenes\/[^/]+\.json$/.test(f)).map(f => ({ id: f.slice(f.lastIndexOf('/') + 1).replace(/\.json$/, ''), file: f, example: false }));
        const all = [...packScenes, ...exampleScenes.map(e => ({ ...e, example: true }))];
        const loaded: SceneChoice[] = await Promise.all(all.map(async entry => ({ ...entry, json: await readJson(entry.file) })));
        const everyone = [...mine, ...examples].filter(g => g.has_model);
        const wanted = girlRel ? everyone.find(g => g.file === girlRel) : undefined;
        const start = loaded.find(c => c.file === sceneRel) ?? (wanted ? loaded.find(c => compatible(c.json, wanted) && !c.example) ?? loaded.find(c => compatible(c.json, wanted)) : loaded[0]);
        const pick = wanted ?? (start ? everyone.find(g => compatible(start.json, g) && g.example === start.example) ?? everyone.find(g => compatible(start.json, g)) : everyone[0]);

        setGirls(everyone);
        setScenes(loaded);
        setSceneFile(start?.file ?? '');
        setGirlFile(pick?.file ?? '');
      } catch (error) {
        setProblem(messageOf(error));
      }
    })();
  }, [sceneRel, girlRel]);

  /* Choosing one narrows the other: a scene is only offered to girls whose skeleton it was made for, and she only the scenes that fit her. */
  const chooseScene = (file: string) => {
    const next = scenes.find(c => c.file === file);

    setSceneFile(file);

    if (next && chosenGirl && !compatible(next.json, chosenGirl)) setGirlFile(girls.find(g => compatible(next.json, g))?.file ?? '');
  };

  const chooseGirl = (file: string) => {
    const next = girls.find(g => g.file === file);

    setGirlFile(file);

    if (next && chosenScene && !compatible(chosenScene.json, next)) setSceneFile(scenes.find(c => compatible(c.json, next))?.file ?? '');
  };

  /* ---------- The stage ---------- */

  useEffect(() => {
    const created = createViewer(host.current!);

    created.setRigSkin('/player_skin.png');
    created.setRigHidden(true, false);
    viewer.current = created;

    return () => { created.dispose(); viewer.current = null; };
  }, []);

  const info = useMemo(() => (sceneJson && girlId ? infoFor(sceneId, sceneJson, girlId, minigame) : null), [sceneJson, girlId, sceneId, minigame]);

  /* ---------- Sound ---------- */

  const play = async (id: string): Promise<Effect> => {
    const [namespace, name] = id.includes(':') ? id.split(':') : ['minecraft', id];
    const picks = assets.current?.sounds[`${namespace}:${name}`];

    if (!picks || picks.length === 0) return { kind: 'sound', text: `${id} (a game sound: not played here)` };

    const pick = picks[Math.floor(Math.random() * picks.length)];
    const [fileNamespace, path] = pick.includes(':') ? pick.split(':') : [namespace, pick];
    const rel = `data/${fileNamespace}/asset_sync/sounds/${path}.ogg`;

    if (!soundOn.current) return { kind: 'sound', text: `${id} (muted)` };

    /* The pack's own file; or, when it has none (a game sound the pack refers to, or a game girl), the game's, shipped with the Pack Creator. */
    const bundled = `example:assets/${fileNamespace}/sounds/${path}.ogg`;

    try {
      let url = audio.current.get(rel) ?? audio.current.get(bundled);

      if (!url && !chosenGirl?.example) url = await api.readAsset(rel).then(u => { audio.current.set(rel, u); return u; }).catch(() => undefined);

      if (!url) {
        url = await api.readAsset(bundled);
        audio.current.set(bundled, url);
      }

      const sound = new Audio(url);

      sound.volume = getSettings().simVolume;
      void sound.play().catch(() => undefined);

      return { kind: 'sound', text: `${id}  →  ${path}.ogg` };
    } catch {
      return { kind: 'sound', text: `${id}: the file ${rel} is not in the pack` };
    }
  };

  /** What a marker does: each cue it names, looked up in her event bindings (sounds, chat lines). */
  const effectsOf = async (marker: string): Promise<Effect[]> => {
    const data = assets.current;
    const out: Effect[] = [];

    if (!data) return out;

    for (const token of marker.toLowerCase().split(/[-,\s]+/).filter(Boolean)) {
      const binding = data.bindings[token];

      if (!binding) continue;

      for (const id of Array.isArray(binding.sounds) ? binding.sounds : []) out.push(await play(String(id)));

      const random = Array.isArray(binding.random_sounds) ? binding.random_sounds : [];

      if (random.length > 0) out.push(await play(String(random[Math.floor(Math.random() * random.length)])));

      for (const line of Array.isArray(binding['pleasurehorizons:messages']) ? binding['pleasurehorizons:messages'] : []) out.push({ kind: 'chat', text: String(line) });

      for (const line of Array.isArray(binding['pleasurehorizons:player_messages']) ? binding['pleasurehorizons:player_messages'] : []) out.push({ kind: 'note', text: `the player says "${String(line)}"` });

      for (const name of Object.keys(binding)) if (name.includes(':') && !name.startsWith('pleasurehorizons:')) out.push({ kind: 'note', text: `${name} (shown in the game, not here)` });
    }

    return out;
  };

  /* ---------- Starting a run ---------- */

  const start = async (who: SimGirl) => {
    const created = viewer.current;
    const scene = sceneJson;
    const girl = who.id;

    if (!created || !scene || !girl) return;

    sim.current && created.playClip(null, false);
    setProblem('');
    setLog([]);
    setChat([]);

    try {
      const data = await loadAssets(sceneId, girl, who.example, who.body_type);
      const bed = await api.bundledBed().catch(() => null);

      assets.current = data;

      if (!data.dressed && !data.nude) {
        setProblem(`${girl} has no model (strut/models/nude or dressed), so there is nothing to show.`);
        return;
      }

      const found = infoFor(sceneId, scene, girl, minigame);
      const catalogue = new SceneCatalogue(data.animations, sceneId, girl, found.replaceDefault);
      const nameOfClip = (name: string): ClipData | null => {
        for (const file of data.animations) {
          const clip = file.clips.find(c => c.name === name);

          if (clip) return clip;
        }

        return null;
      };

      /* How hard the scene is to play is read from the scene: its clips' length and beat, and how long the classic game would take. */
      const feel = deriveFeel(catalogue, nameOfClip, found.cumThreshold, found.useKeyFrameEvents);

      found.pace = { ...feel.settings, ...(found.paceOverrides ?? {}) };
      setFeelText(feel.summary);
      const clipNamed = (name: string): ClipData | null => {
        for (const file of data.animations) {
          const clip = file.clips.find(c => c.name === name);

          if (clip) return clip;
        }

        return null;
      };

      let first = true;
      const show = (kind: 'nude' | 'dressed') => {
        const model = (kind === 'nude' ? data.nude : data.dressed) ?? data.nude ?? data.dressed!;

        created.loadModel(model, data.texture, !first);
        first = false;
      };

      const stage: Stage = {
        hasClip: name => clipNamed(name) !== null,
        playClip: (name, repeat) => created.playClip(name ? clipNamed(name) : null, repeat),
        setModel: show,
        wearsClothes: data.dressed !== null,
        setRigVisible: visible => created.setRigHidden(!visible, false),
        setBed: (visible, offset) => created.setBed(visible && bed ? { model: bed.model, texture: bed.texture, forward: 1 - offset } : null)
      };

      const engine = new SceneSim(found, catalogue, stage, (cue: CueLog) => {
        const entry: LogEntry = { id: nextId.current++, age: cue.age, marker: cue.marker, phase: cue.phase, clip: cue.clip, effects: [] };

        setLog(previous => [entry, ...previous].slice(0, 200));

        void effectsOf(cue.marker).then(effects => {
          entry.effects = effects;
          setLog(previous => [...previous]);
          setChat(previous => [...previous, ...effects.filter(e => e.kind === 'chat').map(e => e.text)].slice(-80));
        });
      }, redraw);

      show('dressed');
      sim.current = engine;

      created.onEvent(event => {
        if (event.kind === 'instruction') engine.cue(event.payload);
        else if (event.kind === 'sound') void play(event.payload).then(effect => setLog(previous => [{ id: nextId.current++, age: engine.age, marker: `(sound) ${event.payload}`, phase: engine.state.phase, clip: engine.current, effects: [effect] }, ...previous].slice(0, 200)));
      });
      created.onClipEnd(() => engine.clipEnded());
      created.onPick(target => { if (target === 'girl') engine.join(); });
      engine.start();
    } catch (error) {
      setProblem(messageOf(error));
      notify(messageOf(error));
    }
  };

  useEffect(() => { if (chosenGirl && sceneJson) void start(chosenGirl); }, [girlFile, sceneFile, scenes.length, minigame]);

  /* ---------- The clock and the keys ---------- */

  useEffect(() => {
    /* A controller's trigger or left stick (pushed up) sets the pace outright, so the whole game can be played with one thumb. */
    let padCum = false;
    const timer = window.setInterval(() => {
      /* The controller is read first, and cannot stop the clock: whatever it does (or fails to), the scene goes on. */
      try {
        const pads = navigator.getGamepads ? [...navigator.getGamepads()] : [];
        const pad = pads.find(p => p && p.connected);

        if (pad) {
          const trigger = pad.buttons[7]?.value ?? 0;
          const stick = Math.max(0, -(pad.axes[1] ?? 0));

          sim.current?.setAnalog(Math.max(trigger, stick > 0.15 ? stick : 0));

          const pressed = !!pad.buttons[0]?.pressed;

          if (pressed && !padCum) { sim.current?.cum(); redraw(); }

          padCum = pressed;
        }
      } catch {
        /* No controller, or the page may not read it: the keyboard and the button play. */
      }

      sim.current?.tick();
    }, TICK_MS);
    const typing = (event: KeyboardEvent) => !!(event.target as Element | null)?.closest?.('input, textarea, select');
    const down = (event: KeyboardEvent) => { if (event.code === getSettings().thrustKey && !typing(event)) { event.preventDefault(); sim.current?.setThrusting(true); redraw(); } };
    const finish = (event: KeyboardEvent) => { if (event.code === getSettings().cumKey && !typing(event)) { sim.current?.cum(); redraw(); } };
    const up = (event: KeyboardEvent) => { if (event.code === getSettings().thrustKey) { sim.current?.setThrusting(false); redraw(); } };

    window.addEventListener('keydown', down);
    window.addEventListener('keydown', finish);
    window.addEventListener('keyup', up);

    return () => { window.clearInterval(timer); window.removeEventListener('keydown', down); window.removeEventListener('keydown', finish); window.removeEventListener('keyup', up); };
  }, []);

  /* ---------- Screen ---------- */

  const engine = sim.current;
  const state = engine?.state;
  const catalogue = useMemo(() => (assets.current && info ? new SceneCatalogue(assets.current.animations, sceneId, girlId, info.replaceDefault) : null), [info, assets.current, girlId]);
  const progress = state && state.cumThreshold > 0 ? Math.min(1, state.progress / state.cumThreshold) : 0;
  const waiting = engine?.waitingForClick() ?? false;

  const status = !engine ? '' : engine.director === 'IN_SCENE' ? phaseText(engine) : engine.note || engine.director;

  const missing: string[] = [];

  if (catalogue && info) {
    const need = (role: (typeof ROLES)[number], why: string) => { if (catalogue.of(role).length === 0) missing.push(`no ${role} clip: ${why}`); };

    if (info.type === 'on_bed') { need('lay_on_bed', 'she cannot lie down'); need('bed_idle', 'she cannot wait on the bed'); }
    if (!info.type.startsWith('stationary')) { need('slow', 'nothing plays while you are not thrusting'); need('fast', 'nothing plays while you thrust'); need('cum', 'the scene cannot finish'); }
    if (info.type === 'stationary') need('loop', 'a solo scene has nothing to repeat');
    if (info.needsToStrip && assets.current && !assets.current.animations.some(f => f.clips.some(c => c.name === 'animation.base.strip'))) missing.push('no animation.base.strip clip: she is changed with no animation');
  }

  const girl = chosenGirl;
  const fitting = (list: SimGirl[]) => list.filter(g => compatible(sceneJson, g));
  const fittingScenes = (list: SceneChoice[]) => list.filter(c => !chosenGirl || compatible(c.json, chosenGirl));

  return (
    <div class="sim">
      <div class="sim-bar">
        <select class="input small-select" title="The girl. Only girls this scene was made for are listed." value={girlFile} onChange={e => chooseGirl((e.target as HTMLSelectElement).value)}>
          {fitting(girls.filter(g => !g.example)).length > 0 && <optgroup label="This pack">{fitting(girls.filter(g => !g.example)).map(g => <option key={g.file} value={g.file}>{g.id}{g.body_type ? ` (${g.body_type})` : ''}</option>)}</optgroup>}
          {fitting(girls.filter(g => g.example)).length > 0 && <optgroup label="The game's girls (examples)">{fitting(girls.filter(g => g.example)).map(g => <option key={g.file} value={g.file}>{g.id}{g.body_type ? ` (${g.body_type})` : ''}</option>)}</optgroup>}
        </select>
        <select class="input small-select" title="The scene. Only scenes made for her skeleton are listed." value={sceneFile} onChange={e => chooseScene((e.target as HTMLSelectElement).value)}>
          {fittingScenes(scenes.filter(c => !c.example)).length > 0 && <optgroup label="This pack">{fittingScenes(scenes.filter(c => !c.example)).map(c => <option key={c.file} value={c.file}>{c.id}</option>)}</optgroup>}
          {fittingScenes(scenes.filter(c => c.example)).length > 0 && <optgroup label="The game's scenes">{fittingScenes(scenes.filter(c => c.example)).map(c => <option key={c.file} value={c.file}>{c.id}</option>)}</optgroup>}
        </select>
        {info && <span class="chip">{info.type}</span>}
        {info?.needsToStrip && <span class="chip">strips first</span>}
        <span class="grow" />
        <select class="input small-select" title="How the scene is played: the game's (hold until the bar is full) or the pace prototype (keep her pace in her sweet spot)" value={minigame} onChange={e => setMinigame((e.target as HTMLSelectElement).value as 'classic' | 'pace')}>
          <option value="pace">Pace (prototype)</option>
          <option value="classic">Classic</option>
        </select>
        <label class="check-inline"><input type="checkbox" checked={muted} onChange={e => setMuted((e.target as HTMLInputElement).checked)} /> mute</label>
        <button class="btn small" onClick={() => chosenGirl && void start(chosenGirl)}>Restart</button>
        <button class="btn small" disabled={!engine || engine.director === 'DONE'} onClick={() => engine?.stop('stopped')}>Stop scene</button>
      </div>

      <div class="sim-main">
        <div class="sim-stage">
          <div class="viewer-canvas" ref={host} />

          <div class="sim-hud">
            <div class="sim-status">{status}</div>
            {state && engine?.director === 'IN_SCENE' && state.cumThreshold > 0 && (state.phase === 'HAVING_SEX' || state.phase === 'INTRO' || state.phase === 'CUM') && (
              <div class="sim-progress" title={`Progress ${state.progress.toFixed(1)} of ${state.cumThreshold}`}><div style={{ width: `${progress * 100}%` }} /></div>
            )}
            {engine?.info.minigame === 'pace' && engine.director === 'IN_SCENE' && engine.state.phase === 'HAVING_SEX' && (
              <div class="sim-pace" title="Your pace. The band is where she likes it.">
                <div class="sim-pace-band" style={{ left: `${engine.pace.spot((engine.age * TICK_MS) / 1000).lo * 100}%`, width: `${(engine.pace.spot((engine.age * TICK_MS) / 1000).hi - engine.pace.spot((engine.age * TICK_MS) / 1000).lo) * 100}%` }} />
                <div class="sim-pace-line" title="Past this she plays the fast clip" />
                <div class="sim-pace-marker" style={{ left: `${engine.pace.pace * 100}%` }} />
              </div>
            )}
            {engine?.info.minigame === 'pace' && engine.director === 'IN_SCENE' && (engine.state.phase === 'HAVING_SEX' || engine.state.phase === 'CUM') && (
              <div class="sim-edge" title="Your own excitement. If it fills before she is ready you finish too soon.">
                <div style={{ width: `${engine.pace.edge * 100}%` }} />
                <span>{engine.age - engine.hitAt < 8 ? `On the beat! x${engine.pace.combo}` : engine.pace.combo > 1 ? `Combo x${engine.pace.combo}` : ''}{engine.pace.combo > 1 ? `  +${Math.round(engine.pace.comboBonus() * 100)}% pleasure` : ''}</span>
              </div>
            )}
            {engine?.info.minigame === 'pace' && engine.director === 'IN_SCENE' && engine.state.phase === 'HAVING_SEX' && engine.pace.hint() && <div class="sim-hint">{engine.pace.hint()}</div>}
            {waiting && <div class="sim-hint">Click on her to join.</div>}
            {engine?.canCum() && <div class="sim-hint">The bar is full. Press Cum to finish.</div>}
          </div>

          <div class="sim-controls">
            <button
              class={'btn sim-thrust' + (state?.thrusting ? ' on' : '')}
              onPointerDown={() => { engine?.setThrusting(true); redraw(); }}
              onPointerUp={() => { engine?.setThrusting(false); redraw(); }}
              onPointerLeave={() => { if (state?.thrusting) { engine?.setThrusting(false); redraw(); } }}
            >{info?.minigame === 'pace' ? 'Hold to speed up' : 'Hold to thrust'} ({keyName(getSettings().thrustKey)})</button>
            {engine?.canSkip() && <button class="btn" title="Skip the undressing, the bed and the intro: straight to the main part" onClick={() => { engine.skip(); redraw(); }}>Skip to the main part</button>}
            {engine?.canCum() && <button class="btn primary sim-cum" onClick={() => { engine.cum(); redraw(); }}>Cum ({keyName(getSettings().cumKey)})</button>}
            {engine?.director === 'DONE' && <button class="btn primary" onClick={() => chosenGirl && void start(chosenGirl)}>Play it again</button>}
          </div>

          {problem && <div class="viewer-problem">{problem}</div>}
        </div>

        <div class="sim-side">
          {girl && info && <div class="muted" style="margin-bottom:6px">{girl.id}{girl.body_type ? `, ${girl.body_type}` : ''}{girl.example ? '. One of the game\'s girls, shipped as an example.' : '.'}</div>}

          {missing.length > 0 && <div class="form-note">{missing.map(m => <div key={m}>{m}</div>)}</div>}

          {minigame === 'pace' && feelText && <div class="muted" style="margin-bottom:6px" title="Read from this scene's clips, so the pace game follows how the scene moves. A minigame block in the scene file overrides it.">{feelText}</div>}

          <h4>Now</h4>
          <div class="muted">{engine?.current ? engine.current.replace(/^animation\./, '') : 'no clip'}{state && engine?.director === 'IN_SCENE' ? `  ·  ${state.phase}${state.phase === 'INTRO' ? ` ${state.introIndex + 1}` : ''}${state.thrusting ? '  ·  thrusting' : ''}` : ''}</div>

          <h4>Chat</h4>
          <div class="sim-chat">{chat.length === 0 ? <span class="muted">Nothing said yet.</span> : chat.map((line, i) => <div key={i}>{line}</div>)}</div>

          <h4>Cues fired</h4>
          <div class="sim-log">
            {log.length === 0 && <span class="muted">No cue has fired yet.</span>}
            {log.map(entry => (
              <div class="sim-cue" key={entry.id}>
                <div><b>{entry.marker}</b> <span class="muted">{entry.phase.toLowerCase()} · {(entry.age / 20).toFixed(1)} s</span></div>
                {entry.effects.map((effect, i) => <div key={i} class={'sim-effect ' + effect.kind}>{effect.kind === 'chat' ? '“' + effect.text + '”' : effect.text}</div>)}
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}

function phaseText(engine: SceneSim): string {
  const s = engine.state;

  switch (s.phase) {
    case 'LAYING_DOWN': return 'She lies down.';
    case 'BED_IDLE': return 'She waits on the bed. Click on her to join.';
    case 'INTRO': return 'Intro';
    case 'HAVING_SEX': {
      const role = engine.current ? parseClipName(engine.current)?.role : undefined;

      return role ? role.replace(/_/g, ' ') : s.thrusting ? 'Thrusting' : 'Idle';
    }
    case 'CUM': return 'Finishing';
    case 'STATIONARY_INTRO': return 'Solo intro';
    case 'STATIONARY': return `Solo (loop ${s.stationaryLoop + 1} of ${s.stationaryLoopThreshold + 1})`;
    default: return s.phase;
  }
}
