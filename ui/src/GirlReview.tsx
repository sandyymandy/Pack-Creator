import { Fragment } from 'preact';
import { useEffect, useRef, useState } from 'preact/hooks';
import type { MutableRef } from 'preact/hooks';
import { api } from './api';
import { Dialog } from './Dialogs';
import { messageOf, notify } from './notify';
import type { ClipData, GirlLook, ModelData, ScenePlan } from './types';
import { createViewer, type Viewer as Scene } from './viewer/scene';

/** What the person has settled for one girl while going through the review. */
export interface Draft {
  include: boolean;
  bodyType: string;
  profile: string;
  /** Her scenes as she will be imported with them; null until she has been looked at. */
  scenes: ScenePlan[] | null;
  reviewed: boolean;
}

/** Where and how a scene plays, and which parts of it have clips. */
const TYPES: { id: string; label: string; hint: string; roles: string[] }[] = [
  { id: 'on_player', label: 'On the player', hint: 'Plays on the player', roles: ['intro', 'slow', 'fast', 'cum'] },
  { id: 'on_bed', label: 'On a bed', hint: 'She goes to a bed, lies down, and it plays there', roles: ['lay_on_bed', 'bed_idle', 'intro', 'slow', 'fast', 'cum'] },
  { id: 'stationary_contact', label: 'Standing, with the player in contact', hint: 'Where she stands, with the player brought into contact', roles: ['intro', 'slow', 'fast', 'cum'] },
  { id: 'stationary_intro', label: 'Standing alone, with an intro', hint: 'A solo scene where she stands: an intro, then a loop', roles: ['solo_intro', 'loop'] },
  { id: 'stationary', label: 'Standing alone', hint: 'A solo scene where she stands: one loop', roles: ['loop'] },
  { id: 'downed', label: 'While she is downed', hint: 'Plays while she is downed', roles: ['intro', 'slow', 'fast', 'cum'] },
  { id: 'player_downed', label: 'While the player is downed', hint: 'Plays while the player is downed', roles: ['intro', 'slow', 'fast', 'cum'] },
  { id: 'player_animation', label: 'With the player: one animation', hint: 'Needs the player, but plays one animation (the Loop part): once, or the number of times the scene says, then it is over', roles: ['loop'] }
];

const typeOf = (scene: ScenePlan) => TYPES.find(t => t.id === (scene.scene_type || 'on_player')) ?? TYPES[0];

/** The references to compare a girl with: the game's own girls. */
const REFERENCES = ['lucy', 'mika', 'momo', 'bee_girl', 'kobold', 'slime', 'wandering_merchant'];

type Compare = 'off' | 'side' | 'overlay';
type View = { position: [number, number, number]; target: [number, number, number] };

const ROLES: { id: string; label: string; hint: string }[] = [
  { id: 'intro', label: 'Intro', hint: 'Plays once as the scene starts' },
  { id: 'slow', label: 'Slow', hint: 'The slow thrusting loop (one of them is picked at random)' },
  { id: 'fast', label: 'Fast', hint: 'The fast thrusting loop (one of them is picked at random)' },
  { id: 'cum', label: 'Climax', hint: 'Plays at the end' },
  { id: 'lay_on_bed', label: 'Lie down', hint: 'Getting onto the bed' },
  { id: 'bed_idle', label: 'Idle on bed', hint: 'Waiting on the bed' },
  { id: 'loop', label: 'Loop', hint: 'One animation that plays on its own' },
  { id: 'solo_intro', label: 'Solo intro', hint: 'The intro of a scene she plays alone' }
];

const roleName = (id: string) => ROLES.find(r => r.id === id)?.label ?? id;

/** The plan without a clip: the clip is taken out of whichever part has it, and a part left empty is dropped. */
function without(plan: ScenePlan[], clip: string): ScenePlan[] {
  return plan.map(scene => ({
    ...scene,
    parts: Object.fromEntries(Object.entries(scene.parts).map(([role, list]) => [role, list.filter(c => c !== clip)] as [string, string[]]).filter(([, list]) => list.length > 0))
  }));
}

/** Where a dragged clip would land. */
type Target = { kind: 'slot'; scene: number; role: string; index: number } | { kind: 'pool' } | { kind: 'new' };

const sameTarget = (a: Target | null, b: Target | null) => JSON.stringify(a) === JSON.stringify(b);

type Bus = MutableRef<Set<(time: number, playing: boolean) => void>>;

/** The play button, the scrubber and the time. Kept apart so that the clock ticking does not redraw the rest of the screen. */
function Playback({ scene, refScene, clip, bus }: { scene: MutableRef<Scene | null>; refScene: MutableRef<Scene | null>; clip: ClipData | null; bus: Bus }) {
  const [time, setTime] = useState(0);
  const [playing, setPlaying] = useState(false);

  useEffect(() => {
    const listener = (t: number, p: boolean) => { setTime(t); setPlaying(p); };

    bus.current.add(listener);

    return () => { bus.current.delete(listener); };
  }, []);

  const toggle = () => {
    if (!clip) return;

    if (scene.current?.isPlaying()) {
      scene.current.pause();
      refScene.current?.pause();
    } else {
      scene.current?.play();
      refScene.current?.play();
    }

    setPlaying(scene.current?.isPlaying() ?? false);
  };

  return (
    <>
      <button class="btn small" disabled={!clip} onClick={toggle}>{playing ? 'Pause' : 'Play'}</button>
      <input type="range" class="scrub" min={0} max={clip?.duration || 1} step={0.001} value={time} disabled={!clip} onInput={e => { const t = Number((e.target as HTMLInputElement).value); scene.current?.pause(); refScene.current?.pause(); setPlaying(false); scene.current?.seek(t); refScene.current?.seek(t); setTime(t); }} />
      <span class="muted time">{time.toFixed(2)} / {(clip?.duration ?? 0).toFixed(2)} s</span>
    </>
  );
}

/**
 * Goes through the chosen girls one at a time: her model on screen (nude or dressed, and optionally one of the game's own girls beside or over her), a list of her animations to
 * play, and the scenes they were found to make up. A clip is dragged from the list into a part of a scene, dragged up or down inside a part to set the order, or dragged back to the
 * list to take it out. What is settled is handed back for the import to use instead of her profile's scene list.
 */
export function GirlReview({ pack, ids, initial, bodyTypes, onDone, onClose }: {
  pack: string;
  ids: string[];
  initial: Record<string, Draft>;
  bodyTypes: string[];
  onDone: (drafts: Record<string, Draft>) => void;
  onClose: () => void;
}) {
  const host = useRef<HTMLDivElement>(null);
  const refHost = useRef<HTMLDivElement>(null);
  const scene = useRef<Scene | null>(null);
  const refScene = useRef<Scene | null>(null);
  const compareNow = useRef<Compare>('off');
  const bus: Bus = useRef(new Set());
  /** The camera both views were last set to, to tell which one the person moved. */
  const shared = useRef<View | null>(null);
  const clipNow = useRef<ClipData | null>(null);
  const ghost = useRef<HTMLDivElement>(null);
  const [drafts, setDrafts] = useState<Record<string, Draft>>(initial);
  const [index, setIndex] = useState(0);
  const [look, setLook] = useState<GirlLook | null>(null);
  const [kind, setKind] = useState('nude');
  const [clips, setClips] = useState<ClipData[]>([]);
  const [clipName, setClipName] = useState('');
  const [problem, setProblem] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [compare, setCompare] = useState<Compare>('off');
  const [refId, setRefId] = useState('lucy');
  const [search, setSearch] = useState('');
  const [hideUsed, setHideUsed] = useState(false);
  const [showPlayer, setShowPlayer] = useState(false);
  const [showBed, setShowBed] = useState(false);
  const playerNow = useRef(false);
  const bedNow = useRef(false);
  const bedModel = useRef<{ model: ModelData; texture: string } | null>(null);
  const [dragging, setDragging] = useState<string | null>(null);
  const [over, setOver] = useState<Target | null>(null);

  compareNow.current = compare;
  playerNow.current = showPlayer;
  bedNow.current = showBed;

  const id = ids[index];
  const draft = drafts[id];
  const clip = clips.find(c => c.name === clipName) ?? null;
  const plan = draft?.scenes ?? [];
  const editable = !!look && !look.files_layout;

  clipNow.current = clip;

  const sceneOfClip = plan.find(s => Object.values(s.parts).some(list => list.includes(clipName)));
  const bedOffset = sceneOfClip?.bed_offset ?? 0;

  const change = (patch: Partial<Draft>) => setDrafts(previous => ({ ...previous, [id]: { ...previous[id], ...patch } }));
  const setPlan = (next: ScenePlan[]) => change({ scenes: next, reviewed: true });

  /* The girl's 3D view is made once. */
  useEffect(() => {
    const created = createViewer(host.current!);

    created.onTick((t, p) => {
      bus.current.forEach(listener => listener(t, p));
    });
    scene.current = created;

    /* The reference, when there is one, follows her: the same clip at the same moment, and the same view from the same side, whichever of the two is turned. */
    let frame = 0;
    const follow = () => {
      const other = refScene.current;

      if (other && compareNow.current !== 'off') {
        const mine = created.getView();
        const theirs = other.getView();
        const differs = (x: View, y: View) => [...x.position, ...x.target].some((n, i) => Math.abs(n - [...y.position, ...y.target][i]) > 1e-5);

        if (!shared.current) shared.current = mine;

        if (differs(mine, shared.current)) {
          other.setView(mine);
          shared.current = mine;
        } else if (differs(theirs, shared.current)) {
          created.setView(theirs);
          shared.current = theirs;
        }

        if (other.isPlaying() !== created.isPlaying()) {
          if (created.isPlaying()) other.play();
          else other.pause();
        }

        if (Math.abs(other.getTime() - created.getTime()) > 0.05) other.seek(created.getTime());
      }

      frame = requestAnimationFrame(follow);
    };

    frame = requestAnimationFrame(follow);

    return () => { cancelAnimationFrame(frame); created.dispose(); scene.current = null; };
  }, []);

  /* Looking at a girl: what she has, the scenes to start from, and a body type guessed from her bones. */
  useEffect(() => {
    let current = true;

    setLook(null);
    setClips([]);
    setClipName('');
    setProblem(null);
    setLoading(true);

    void (async () => {
      try {
        const found = await api.oldPackLook(pack, id, drafts[id].profile.trim() || null);

        if (!current) return;

        setLook(found);
        setKind(found.models.includes('nude') ? 'nude' : found.models[0] ?? 'nude');

        const guess = drafts[id].bodyType === '' && found.bones.length > 0 ? await api.guessBodyType(found.bones).catch(() => null) : null;

        if (!current) return;

        setDrafts(previous => ({
          ...previous,
          [id]: { ...previous[id], bodyType: previous[id].bodyType || guess || '', scenes: previous[id].scenes ?? (found.files_layout ? null : found.scenes) }
        }));

        if (!found.files_layout) {
          const list = await api.oldPackClips(pack, id).catch(error => { setProblem(messageOf(error)); return [] as ClipData[]; });

          if (current) setClips(list);
        }
      } catch (error) {
        if (current) setProblem(messageOf(error));
      } finally {
        if (current) setLoading(false);
      }
    })();

    return () => { current = false; };
  }, [id]);

  /* Her model and texture. */
  useEffect(() => {
    if (!look || look.models.length === 0) return;

    let current = true;

    void (async () => {
      try {
        const data = await api.oldPackModel(pack, id, kind);
        const texture = look.texture ? await api.oldPackTexture(pack, id).catch(() => null) : null;

        if (!current) return;

        scene.current?.loadModel(data, texture);
        scene.current?.setRigSkin('/player_skin.png');
        scene.current?.setRigHidden(!playerNow.current, false);
      } catch (error) {
        if (current) setProblem(messageOf(error));
      }
    })();

    return () => { current = false; };
  }, [look, kind]);

  /* The player (the male's body in the model's own scene-rig bones, with the player's skin) and the game's bed, shown or hidden. */
  useEffect(() => {
    scene.current?.setRigSkin('/player_skin.png');
    scene.current?.setRigHidden(!showPlayer, false);
  }, [showPlayer]);

  useEffect(() => {
    let current = true;

    if (!showBed) {
      scene.current?.setBed(null);

      return;
    }

    void api.bundledBed().then(bed => { if (current && bedNow.current) { bedModel.current = bed; scene.current?.setBed({ model: bed.model, texture: bed.texture, forward: 1 - bedOffset }); } }).catch(error => notify(messageOf(error)));

    return () => { current = false; };
  }, [showBed]);

  /* The bed sits where the scene puts it: moved by the offset of the scene the clip belongs to, as the game does with bed_alignment_offset. */
  useEffect(() => {
    if (showBed && bedModel.current) scene.current?.setBed({ model: bedModel.current.model, texture: bedModel.current.texture, forward: 1 - bedOffset });
  }, [bedOffset]);

  /* The reference girl, one of the game's own: a second view that exists only while comparing (a view nobody sees would still be drawn every frame), shown the same way (nude or
     dressed) as the one being looked at. */
  useEffect(() => {
    if (compare === 'off') return;

    const other = createViewer(refHost.current!);

    refScene.current = other;
    shared.current = null;
    other.setShading(compare === 'overlay' ? 'wire' : 'lit');
    other.setGrid(compare === 'side');

    let current = true;

    void (async () => {
      try {
        const data = await api.readModel(`example:assets/pleasurehorizons/strut/models/${kind === 'dressed' ? 'dressed' : 'nude'}/${refId}.strutmodel`);
        const texture = await api.readImage(`example:assets/pleasurehorizons/textures/entities/${refId}.png`).catch(() => null);

        if (!current) return;

        other.loadModel(data, texture);
        other.setClip(clipNow.current);
        other.setView(scene.current!.getView());
        shared.current = scene.current!.getView();

        if (scene.current?.isPlaying()) other.play();
      } catch (error) {
        if (current) notify(messageOf(error));
      }
    })();

    return () => { current = false; other.dispose(); refScene.current = null; };
  }, [compare, refId, kind]);

  /* Playing a clip, on her and on the reference. */
  const preview = (name: string) => {
    const found = clips.find(c => c.name === name);

    if (!found) return;

    setClipName(name);

    for (const view of [scene.current, refScene.current]) {
      view?.setClip(found);
      view?.setLoop(true);
      view?.play();
    }
  };

  // =================================================================
  // Changing the scenes
  // =================================================================

  const where = new Map<string, string>();

  plan.forEach(s => Object.entries(s.parts).forEach(([role, list]) => list.forEach(c => where.set(c, `${s.name || 'Scene'} · ${roleName(role)}`))));

  const shownClips = (look?.clips ?? []).filter(c => c.toLowerCase().includes(search.trim().toLowerCase()) && (!hideUsed || !where.has(c)));

  /* Puts a clip in a part of a scene at a place in its order (the end when none is given), taking it out of wherever it was. */
  const place = (clipToMove: string, sceneIndex: number, role: string, at?: number) => {
    const before = plan[sceneIndex]?.parts[role] ?? [];
    const was = before.indexOf(clipToMove);
    const next = without(plan, clipToMove);
    const list = [...(next[sceneIndex].parts[role] ?? [])];
    let insert = at ?? list.length;

    if (was >= 0 && at !== undefined && was < at) insert -= 1;

    list.splice(Math.max(0, Math.min(insert, list.length)), 0, clipToMove);
    next[sceneIndex] = { ...next[sceneIndex], parts: { ...next[sceneIndex].parts, [role]: list } };
    setPlan(next);
  };

  /* A clip's name suggests the part it plays; a new scene starts from it with that part (a loop when it does not say). */
  const startScene = (name0: string) => {
    const lower = name0.toLowerCase();
    const role = ROLES.find(r => ['intro', 'slow', 'fast', 'cum'].includes(r.id) && new RegExp(`(^|_)${r.id}\\d*(_|$)`).test(lower))?.id ?? 'loop';
    const base = name0.replace(/_?(intro|slow|fast|cum)\d*$/i, '').replace(/^(intro|slow|fast|cum)_/i, '').replace(/_/g, ' ').trim() || name0;

    setPlan([...without(plan, name0), { name: base.charAt(0).toUpperCase() + base.slice(1), parts: { [role]: [name0] }, scene_type: role === 'loop' ? 'stationary' : 'on_player' }]);
  };

  const rename = (at: number, name: string) => setPlan(plan.map((s, i) => (i === at ? { ...s, name } : s)));
  const setBedOffset = (at: number, bed_offset: number | null) => setPlan(plan.map((s, i) => (i === at ? { ...s, bed_offset } : s)));
  const retype = (at: number, scene_type: string) => setPlan(plan.map((s, i) => (i === at ? { ...s, scene_type } : s)));
  const remove = (at: number) => setPlan(plan.filter((_, i) => i !== at));
  const addScene = () => setPlan([...plan, { name: 'New scene', parts: {}, scene_type: 'on_player' }]);
  const takeOut = (clipToTake: string) => setPlan(without(plan, clipToTake));

  const findAgain = async () => {
    if (!look) return;

    try {
      const found = await api.detectScenes(look.clips);

      setPlan(found);
      notify(found.length === 0 ? 'No scene could be found from the clip names.' : `Found ${found.length} scene${found.length === 1 ? '' : 's'} from the clip names.`);
    } catch (error) {
      notify(messageOf(error));
    }
  };

  const fromProfile = async () => {
    if (!look) return;

    try {
      const again = await api.oldPackLook(pack, id, draft.profile.trim() || null);

      setPlan(again.scenes_from === 'profile' ? again.scenes : []);
      if (again.scenes_from !== 'profile') notify('There is no profile for her, so there are no scenes to take from it.');
    } catch (error) {
      notify(messageOf(error));
    }
  };

  // =================================================================
  // Dragging
  // =================================================================

  /* Drag and drop is done with the pointer rather than the browser's own, which a window of this kind may swallow. A press that does not move is a click (it plays the clip). */
  const press = (event: PointerEvent, name: string) => {
    if (event.button !== 0 || !editable) return;

    const startX = event.clientX;
    const startY = event.clientY;
    let moved = false;
    let target: Target | null = null;

    const find = (x: number, y: number): Target | null => {
      const element = (document.elementFromPoint(x, y) as HTMLElement | null)?.closest('[data-drop]') as HTMLElement | null;

      if (!element) return null;

      const kindOfDrop = element.dataset.drop;

      if (kindOfDrop === 'pool') return { kind: 'pool' };
      if (kindOfDrop === 'new') return { kind: 'new' };

      const sceneIndex = Number(element.dataset.scene);
      const role = element.dataset.role ?? '';

      if (kindOfDrop === 'row') {
        const rect = element.getBoundingClientRect();
        const rowIndex = Number(element.dataset.index);

        return { kind: 'slot', scene: sceneIndex, role, index: y < rect.top + rect.height / 2 ? rowIndex : rowIndex + 1 };
      }

      return { kind: 'slot', scene: sceneIndex, role, index: Number(element.dataset.count ?? 0) };
    };

    const move = (e: PointerEvent) => {
      if (!moved && Math.hypot(e.clientX - startX, e.clientY - startY) < 5) return;

      if (!moved) {
        moved = true;
        setDragging(name);
      }

      if (ghost.current) ghost.current.style.transform = `translate(${e.clientX + 12}px, ${e.clientY + 6}px)`;

      const now = find(e.clientX, e.clientY);

      if (!sameTarget(now, target)) {
        target = now;
        setOver(now);
      }
    };

    const up = (e: PointerEvent) => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
      window.removeEventListener('pointercancel', up);

      if (!moved) {
        preview(name);

        return;
      }

      const drop = find(e.clientX, e.clientY);

      setDragging(null);
      setOver(null);

      if (!drop) return;

      if (drop.kind === 'slot') place(name, drop.scene, drop.role, drop.index);
      else if (drop.kind === 'new') startScene(name);
      else takeOut(name);
    };

    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
    window.addEventListener('pointercancel', up);
    event.preventDefault();
  };

  const last = index === ids.length - 1;

  // =================================================================
  // Drawing
  // =================================================================

  const slotOf = (s: ScenePlan, si: number, role: string) => {
    const list = s.parts[role] ?? [];
    const allowed = typeOf(s).roles.includes(role);
    const hot = over?.kind === 'slot' && over.scene === si && over.role === role;

    return (
      <div class={'slot' + (hot ? ' hot' : '') + (allowed ? '' : ' odd')} key={role} data-drop="slot" data-scene={si} data-role={role} data-count={list.length}>
        <span class="slot-name" title={allowed ? ROLES.find(r => r.id === role)?.hint : 'This type of scene does not use this part'}>{roleName(role)}</span>
        <div class="slot-rows">
          {list.length === 0 && <span class="slot-empty">{hot ? 'Drop it here' : 'Drag a clip here'}</span>}
          {list.map((c, i) => (
            <Fragment key={c}>
              {hot && over.index === i && <div class="drop-line" />}
              <div class={'slot-row' + (clipName === c ? ' on' : '') + (dragging === c ? ' lifted' : '')} data-drop="row" data-scene={si} data-role={role} data-index={i} onPointerDown={e => press(e as unknown as PointerEvent, c)}>
                <span class="grip">⋮⋮</span>
                <span class="row-name">{c}</span>
                <button class="clip-x" title="Take it out of this part" onPointerDown={e => e.stopPropagation()} onClick={() => takeOut(c)}>×</button>
              </div>
            </Fragment>
          ))}
          {hot && over.index === list.length && list.length > 0 && <div class="drop-line" />}
        </div>
      </div>
    );
  };

  return (
    <Dialog title={`Looking at her: ${id}  (${index + 1} of ${ids.length})`} onClose={onClose}>
      <div class="review">
        <div class="review-left">
          <div class="review-bar">
            {look && look.models.length > 1 && (
              <select class="input small-select" value={kind} onChange={e => setKind((e.target as HTMLSelectElement).value)}>
                {look.models.map(m => <option key={m} value={m}>{m === 'nude' ? 'Nude model' : 'Dressed model'}</option>)}
              </select>
            )}
            <select class="input small-select" value={compare} title="Compare her with one of the game's own girls: the clip you play plays on both" onChange={e => setCompare((e.target as HTMLSelectElement).value as Compare)}>
              <option value="off">Compare: off</option>
              <option value="side">Side by side</option>
              <option value="overlay">Overlaid</option>
            </select>
            {compare !== 'off' && (
              <select class="input small-select" value={refId} onChange={e => setRefId((e.target as HTMLSelectElement).value)}>
                {REFERENCES.map(r => <option key={r} value={r}>{r.replace('_', ' ')}</option>)}
              </select>
            )}
            <label class="check-inline" title="Shows the player with their skin on, as the scene rig in the model (the male's body bones: steve, torso2, and so on), so you can see how the clips line up with a body. A model without those bones shows nothing."><input type="checkbox" checked={showPlayer} onChange={e => setShowPlayer((e.target as HTMLInputElement).checked)} /> Player</label>
            <label class="check-inline" title="Shows the game's bed under her, for a scene on a bed"><input type="checkbox" checked={showBed} onChange={e => setShowBed((e.target as HTMLInputElement).checked)} /> Bed</label>
            <Playback scene={scene} refScene={refScene} clip={clip} bus={bus} />
          </div>
          <div class={'review-stage mode-' + compare}>
            <div class="review-canvas" ref={host}><span class="stage-label">{id}{clip ? ` · ${clip.name}` : ''}</span></div>
            <div class="review-canvas ref" ref={refHost}><span class="stage-label ref">{refId}{compare === 'overlay' ? ' (outline)' : ''}</span></div>
          </div>
          {loading && <div class="viewer-problem" style="background:transparent;color:var(--muted)">Looking…</div>}
          {problem && <div class="viewer-problem">{problem}</div>}
        </div>

        <div class={'review-clips' + (over?.kind === 'pool' ? ' hot' : '')} data-drop="pool">
          <b>Her animations <span class="muted">({look?.clips.length ?? 0})</span></b>
          {editable ? (
            <>
              <input class="input" value={search} placeholder="Search…" onInput={e => setSearch((e.target as HTMLInputElement).value)} />
              <label class="check-inline"><input type="checkbox" checked={hideUsed} onChange={e => setHideUsed((e.target as HTMLInputElement).checked)} /> Hide the ones already in a scene</label>
              <span class="hint">Click one to play it. Drag it into a part of a scene.</span>
              <div class="clip-list">
                {shownClips.map(c => (
                  <div class={'clip-item' + (clipName === c ? ' on' : '') + (where.has(c) ? ' used' : '') + (dragging === c ? ' lifted' : '')} key={c} onPointerDown={e => press(e as unknown as PointerEvent, c)} title={where.get(c) ?? 'In no scene: stays a base animation'}>
                    <span class="grip">⋮⋮</span>
                    <span class="row-name">{c}</span>
                    {where.has(c) && <span class="where">{where.get(c)}</span>}
                  </div>
                ))}
                {shownClips.length === 0 && <span class="muted">Nothing matches.</span>}
              </div>
            </>
          ) : (
            <span class="hint">{look?.files_layout ? 'Her animations are already one file for each scene, so they are imported as they are.' : 'Looking…'}</span>
          )}
        </div>

        <div class="review-right">
          <div class="row" style="gap:12px;align-items:center;flex-wrap:wrap">
            <label class="check-inline"><input type="checkbox" checked={draft.include} onChange={e => change({ include: (e.target as HTMLInputElement).checked })} /> Import her</label>
            <select class="input small-select" value={draft.bodyType} title="The skeleton her model was built from (guessed from her bones: check it against the model)" onChange={e => change({ bodyType: (e.target as HTMLSelectElement).value })}>
              <option value="">Body type…</option>
              {bodyTypes.map(b => <option key={b} value={b}>{b}</option>)}
            </select>
          </div>

          {look && look.files_layout && <div class="hint">Her scenes: {look.scenes.map(s => s.name).join(', ') || 'none'}. They are imported as they are.</div>}

          {editable && (
            <>
              <div class="row" style="justify-content:space-between;align-items:center;gap:6px;flex-wrap:wrap">
                <b>Scenes {plan.length > 0 && <span class="muted">({plan.length})</span>}</b>
                <span class="row" style="gap:6px">
                  <button class="btn ghost small" title="Look at the names of her clips again and find the scenes in them" onClick={() => void findAgain()}>Find from clip names</button>
                  <button class="btn ghost small" title="Take the scenes her old profile lists" onClick={() => void fromProfile()}>From her profile</button>
                  <button class="btn ghost small" onClick={addScene}>Add scene</button>
                </span>
              </div>
              <span class="hint">{look?.scenes_from === 'profile' ? 'From her old profile.' : look?.scenes_from === 'names' ? 'Found from the names of her clips; check them.' : 'No scene was found.'} The order in a part is the order they are tried.</span>

              {plan.map((s, si) => {
                const type = typeOf(s);
                const solo = type.id === 'stationary' || type.id === 'stationary_intro' || type.id === 'player_animation';
                const missing = solo ? !s.parts.loop?.length : !(s.parts.slow?.length || s.parts.fast?.length);
                const extras = Object.keys(s.parts).filter(role => !type.roles.includes(role));

                return (
                  <div class="scene-card" key={si}>
                    <div class="row" style="gap:6px;align-items:center">
                      <input class="input grow" value={s.name} onInput={e => rename(si, (e.target as HTMLInputElement).value)} />
                      <button class="btn ghost small" title="Remove this scene; its clips become base animations" onClick={() => remove(si)}>Remove</button>
                    </div>
                    <select class="input small-select type-select" title={type.hint} value={type.id} onChange={e => retype(si, (e.target as HTMLSelectElement).value)}>
                      {TYPES.map(t => <option key={t.id} value={t.id}>{t.label}</option>)}
                    </select>
                    {type.id === 'on_bed' && (
                      <label class="row" style="gap:6px;align-items:center" title="How far the bed is moved from where it is by default (blocks). The Bed preview follows it while one of this scene's clips is playing.">
                        <span class="muted">Bed offset</span>
                        <input class="input" style="width:80px" type="number" step="0.05" value={s.bed_offset ?? 0} onChange={e => { const n = Number((e.target as HTMLInputElement).value); setBedOffset(si, Number.isFinite(n) ? n : null); }} />
                      </label>
                    )}
                    {missing && <div class="hint" style="color:var(--warning)">{solo ? 'A scene she plays alone needs a loop clip.' : 'A scene needs a slow or a fast clip to play.'}</div>}
                    {type.roles.map(role => slotOf(s, si, role))}
                    {extras.map(role => slotOf(s, si, role))}
                  </div>
                );
              })}

              <div class={'new-scene' + (over?.kind === 'new' ? ' hot' : '')} data-drop="new">Drop a clip here to start a new scene from it</div>
            </>
          )}
        </div>
      </div>

      {dragging && <div class="drag-ghost" ref={ghost}>{dragging}</div>}

      <div class="foot">
        <button class="btn ghost" onClick={onClose}>Cancel</button>
        <span class="grow" />
        <button class="btn ghost" disabled={index === 0} onClick={() => setIndex(index - 1)}>Back</button>
        {!last && <button class="btn ghost" title="Keep what is settled so far and go back to the list" onClick={() => onDone({ ...drafts, [id]: { ...draft, reviewed: true } })}>Finish here</button>}
        {!last && <button class="btn primary" onClick={() => { change({ reviewed: true }); setIndex(index + 1); }}>Next girl</button>}
        {last && <button class="btn primary" onClick={() => onDone({ ...drafts, [id]: { ...draft, reviewed: true } })}>Done</button>}
      </div>
    </Dialog>
  );
}
