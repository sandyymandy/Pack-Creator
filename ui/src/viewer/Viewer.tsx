import { useEffect, useRef, useState } from 'preact/hooks';
import { api } from '../api';
import { BlockbenchDialog } from '../BlockbenchDialog';
import { Dialog } from '../Dialogs';
import { messageOf, notify } from '../notify';
import type { ModelData, ClipData, ViewerFiles } from '../types';
import { ClipManager } from '../ClipManager';
import { CueEditor } from './CueEditor';
import { PhysicsPanel } from './PhysicsPanel';
import { createViewer, type Shading, type Smoothing, type Viewer as Scene } from './scene';

const nameOf = (rel: string) => rel.slice(rel.lastIndexOf('/') + 1);
const SPEEDS = [0.25, 0.5, 1, 2];

const SLOT_ORDER = ['head', 'chest', 'legs', 'feet'];
const slotOf = (file: string) => (file.split('/').pop() ?? '').replace('.strutmodel', '');

/** The 3D view of a model or an animation of a girl in the pack: her model, her texture, and any clip of hers played on it with its cues. */
export function Viewer({ rel }: { rel: string }) {
  const host = useRef<HTMLDivElement>(null);
  const scene = useRef<Scene | null>(null);
  const [files, setFiles] = useState<ViewerFiles | null>(null);
  const [model, setModel] = useState<string | null>(null);
  const [animation, setAnimation] = useState<string | null>(null);
  const [clips, setClips] = useState<ClipData[]>([]);
  const [clipName, setClipName] = useState('');
  const [playing, setPlaying] = useState(false);
  const [time, setTime] = useState(0);
  const [speed, setSpeed] = useState(1);
  const [rigHidden, setRigHidden] = useState(true);
  const [shading, setShading] = useState<Shading>('lit');
  const [brightness, setBrightness] = useState(1);
  const [smoothing, setSmoothing] = useState<Smoothing>('authored');
  const [grid, setGrid] = useState(true);
  const [problem, setProblem] = useState<string | null>(null);
  const [info, setInfo] = useState('');
  const [editingCues, setEditingCues] = useState(false);
  const [blockbenchDialog, setBlockbenchDialog] = useState(false);
  const [pickModel, setPickModel] = useState<string | null>(null);
  const [opening, setOpening] = useState(false);
  const [reload, setReload] = useState(0);
  const [managing, setManaging] = useState(false);
  const [viewOpen, setViewOpen] = useState(false);
  /* Which slots of her armour are worn in the preview. */
  const [worn, setWorn] = useState<Set<string>>(new Set());

  const isModel = rel.endsWith('.strutmodel');
  const clip = clips.find(c => c.name === clipName) ?? null;
  /* A model can finish loading after the clips do, so it is given the clip as it is then, not as it was when the load started. */
  const latestClip = useRef<ClipData | null>(null);

  latestClip.current = clip;

  /* The scene is made once; what it shows follows the choices below. */
  useEffect(() => {
    const created = createViewer(host.current!);

    created.onTick((t, p) => { setTime(t); setPlaying(p); });
    scene.current = created;

    return () => { created.dispose(); scene.current = null; };
  }, []);

  /* The girl's files. */
  useEffect(() => {
    let current = true;

    void api.viewerFiles(rel).then(found => {
      if (!current) return;

      setFiles(found);
      setModel(isModel ? rel : found.models[0] ?? null);

      const base = found.animations.find(a => nameOf(a) === `base.${found.id}.strutanim`);

      setAnimation(isModel ? base ?? found.animations[0] ?? null : rel);
    }).catch(error => setProblem(messageOf(error)));

    return () => { current = false; };
  }, [rel]);

  /* Coming back from Blockbench, where the model, animation or texture may have been saved: read them again. */
  const watched = [model, animation, files?.texture].filter((f): f is string => !!f);
  const lastStamps = useRef('');

  useEffect(() => {
    let current = true;

    void api.stamps(watched).then(list => { if (current) lastStamps.current = JSON.stringify(list); }).catch(() => undefined);

    return () => { current = false; };
  }, [model, animation, files, reload]);

  useEffect(() => {
    const check = () => {
      void api.stamps(watched).then(list => {
        const now = JSON.stringify(list);

        if (lastStamps.current && now !== lastStamps.current) setReload(n => n + 1);
      }).catch(() => undefined);
    };

    window.addEventListener('focus', check);

    return () => window.removeEventListener('focus', check);
  }, [model, animation, files]);

  /* The armour worn in the preview: each worn slot's model, on the pack's armour texture or the game's. */
  useEffect(() => {
    if (!files) return;

    let current = true;

    void (async () => {
      const pieces: { model: ModelData; texture: string | null }[] = [];
      const texture = files.armourTexture ? await api.readImage(files.armourTexture).catch(() => null) : await api.bundledArmourTexture().catch(() => null);

      for (const file of files.armour.filter(f => worn.has(slotOf(f)))) {
        try {
          pieces.push({ model: await api.readModel(file), texture });
        } catch (error) {
          notify(messageOf(error));
        }
      }

      if (current) scene.current?.setArmour(pieces);
    })();

    return () => { current = false; };
  }, [files, worn, reload]);

  /* The model and its texture. */
  useEffect(() => {
    if (!model || !files) return;

    let current = true;

    void (async () => {
      try {
        const data = await api.readModel(model);
        const texture = files.texture ? await api.readImage(files.texture).catch(() => null) : model.includes('/strut/models/armour/') ? await api.bundledArmourTexture().catch(() => null) : null;

        if (!current) return;

        scene.current?.loadModel(data, texture);
        scene.current?.setClip(latestClip.current);
        setInfo(`${data.groupBones.length} bones, ${data.cubes.length} cubes${data.meshes.length ? `, ${data.meshes.length} meshes` : ''}${data.extensions.length ? ', physics' : ''}`);
        setProblem(null);
      } catch (error) {
        if (current) setProblem(messageOf(error));
      }
    })();

    return () => { current = false; };
  }, [model, files, reload]);

  /* The clips of the chosen animation file. */
  useEffect(() => {
    if (!animation) {
      setClips([]);
      setClipName('');
      return;
    }

    let current = true;

    void api.readAnimation(animation).then(list => {
      if (!current) return;

      setClips(list);
      setClipName(previous => (list.some(c => c.name === previous) ? previous : list[0]?.name ?? ''));
    }).catch(error => notify(messageOf(error)));

    return () => { current = false; };
  }, [animation, reload]);

  useEffect(() => {
    scene.current?.setClip(clip);
    setTime(0);
    setPlaying(false);
  }, [clipName, clips]);

  useEffect(() => { scene.current?.setSpeed(speed); }, [speed]);
  /* An armour model is all armour bones, which count as the scene rig: show them for it. */
  useEffect(() => { if (model) setRigHidden(!model.includes('/strut/models/armour/')); }, [model]);
  useEffect(() => { scene.current?.setRigHidden(rigHidden); }, [rigHidden]);
  useEffect(() => { scene.current?.setShading(shading); }, [shading]);
  useEffect(() => { scene.current?.setBrightness(brightness); }, [brightness]);
  useEffect(() => { scene.current?.setSmoothing(smoothing); }, [smoothing]);
  useEffect(() => { scene.current?.setGrid(grid); }, [grid]);

  const expressions = clip ? clip.channels.reduce((n, c) => n + c.keys.filter(k => k.value.expressions.some(e => e !== null)).length, 0) : 0;
  const duration = clip?.duration ?? 0;

  const toggle = () => {
    const now = scene.current?.isPlaying() ?? false;

    if (now) scene.current?.pause();
    else scene.current?.play();

    setPlaying(scene.current?.isPlaying() ?? false);
  };

  /* Opens the model in Blockbench with her texture; an animation file is opened on a model the person picks. */
  const openInBlockbench = async (modelFile: string, animationFile: string | null) => {
    setOpening(true);

    try {
      const status = await api.blockbenchStatus();

      if (!status.exe || !status.versionOk) {
        setBlockbenchDialog(true);
        return;
      }

      await api.openInBlockbench(modelFile, files?.texture ?? null, animationFile);
    } catch (error) {
      notify(messageOf(error));
    } finally {
      setOpening(false);
    }
  };

  const editInBlockbench = () => {
    if (isModel) {
      void openInBlockbench(rel, null);
    } else if (files && files.models.length === 1) {
      void openInBlockbench(files.models[0], rel);
    } else if (files && files.models.length > 1) {
      setPickModel(model ?? files.models[0]);
    } else {
      notify('There is no model of hers in this pack to put the animation on. Add or import her model first.');
    }
  };

  return (
    <div class="viewer">
      <div class="viewer-bar">
        {files && files.models.length > 1 && (
          <select class="input small-select" value={model ?? ''} onChange={e => setModel((e.target as HTMLSelectElement).value)}>
            {files.models.map(m => <option key={m} value={m}>{m.includes('/nude/') ? 'Nude model' : m.includes('/dressed/') ? 'Dressed model' : nameOf(m)}</option>)}
          </select>
        )}
        {files && files.animations.length > 0 && (
          <select class="input small-select" value={animation ?? ''} onChange={e => setAnimation((e.target as HTMLSelectElement).value)}>
            {files.animations.map(a => <option key={a} value={a}>{nameOf(a)}</option>)}
          </select>
        )}
        {clips.length > 0 && (
          <select class="input small-select" value={clipName} onChange={e => setClipName((e.target as HTMLSelectElement).value)}>
            {clips.map(c => <option key={c.name} value={c.name}>{c.name.replace(/^animation\./, '')}</option>)}
          </select>
        )}
        <button class="btn small" disabled={opening} title={isModel ? 'Open this model and her texture in Blockbench' : 'Open this animation in Blockbench, on a model you choose'} onClick={editInBlockbench}>{opening ? 'Opening...' : 'Edit in Blockbench'}</button>
        {!isModel && animation && clips.length > 0 && <button class="btn small" title="See every animation in this file and remove the ones you do not want" onClick={() => setManaging(true)}>Animations…</button>}
        <span class="grow" />
        <span class="muted viewer-info">{info}</span>
        <div class="view-menu">
          <button class={'btn small' + (viewOpen ? ' on' : '')} title="How the model is drawn, the armour it wears, the grid and the scene rig" onClick={() => setViewOpen(open => !open)}>View ▾</button>
          {viewOpen && (
            <>
              <div class="view-backdrop" onClick={() => setViewOpen(false)} />
              <div class="view-panel">
                <label class="view-row"><span>Shading</span>
                  <select class="input small-select" value={shading} onChange={e => setShading((e.target as HTMLSelectElement).value as Shading)}>
                    <option value="lit">Lit</option>
                    <option value="flat">Flat (texture only)</option>
                    <option value="normals">Normals</option>
                    <option value="wire">Wireframe</option>
                  </select>
                </label>
                <label class="view-row"><span>Smoothing</span>
                  <select class="input small-select" value={smoothing} onChange={e => setSmoothing((e.target as HTMLSelectElement).value as Smoothing)}>
                    <option value="authored">As authored</option>
                    <option value="smooth">All meshes</option>
                    <option value="flat">Off</option>
                  </select>
                </label>
                <label class="view-row"><span>Brightness</span><input type="range" class="bright" min={0.2} max={2} step={0.05} value={brightness} onInput={e => setBrightness(Number((e.target as HTMLInputElement).value))} /></label>
                <label class="view-row check-inline"><input type="checkbox" checked={grid} onChange={e => setGrid((e.target as HTMLInputElement).checked)} /> Grid</label>
                <label class="view-row check-inline" title="The scene rig: the male's body, items, armour and cameras"><input type="checkbox" checked={!rigHidden} onChange={e => setRigHidden(!(e.target as HTMLInputElement).checked)} /> Scene rig</label>
                {files && files.armour.length > 0 && isModel && (
                  <div class="view-group">
                    <span class="muted">Armour worn</span>
                    {files.armour.map(file => slotOf(file)).sort((x, y) => SLOT_ORDER.indexOf(x) - SLOT_ORDER.indexOf(y)).map(slot => (
                      <label key={slot} class="view-row check-inline"><input type="checkbox" checked={worn.has(slot)} onChange={e => setWorn(previous => { const next = new Set(previous); if ((e.target as HTMLInputElement).checked) next.add(slot); else next.delete(slot); return next; })} /> {slot}</label>
                    ))}
                  </div>
                )}
              </div>
            </>
          )}
        </div>
      </div>

      <div class="viewer-canvas" ref={host} />

      {problem && <div class="viewer-problem">{problem}</div>}

      <div class="viewer-controls">
        <button class="btn small" disabled={!clip} onClick={toggle}>{playing ? 'Pause' : 'Play'}</button>
        <input
          type="range"
          class="scrub"
          min={0}
          max={duration || 1}
          step={0.001}
          value={time}
          disabled={!clip}
          onInput={e => { const t = Number((e.target as HTMLInputElement).value); scene.current?.pause(); setPlaying(false); scene.current?.seek(t); setTime(t); }}
        />
        <span class="muted time">{time.toFixed(2)} / {duration.toFixed(2)} s</span>
        <select class="input small-select" value={speed} onChange={e => setSpeed(Number((e.target as HTMLSelectElement).value))}>
          {SPEEDS.map(s => <option key={s} value={s}>{s}x</option>)}
        </select>
        <label class="check-inline"><input type="checkbox" checked={clip ? clip.loopMode !== 0 : true} onChange={e => scene.current?.setLoop((e.target as HTMLInputElement).checked)} /> Loop</label>
      </div>

      {clip && editingCues && animation && (
        <CueEditor
          rel={animation}
          clip={clip}
          time={time}
          onSeek={t => { scene.current?.pause(); setPlaying(false); scene.current?.seek(t); setTime(t); }}
          onSaved={list => setClips(previous => previous.map(c => list.find(n => n.name === c.name) ?? c))}
        />
      )}

      {clip && !editingCues && clip.events.length === 0 && animation && <div class="cues"><span class="muted">No cues in this clip.</span><button class="btn ghost small" onClick={() => setEditingCues(true)}>Add some</button></div>}

      {clip && !editingCues && clip.events.length > 0 && (
        <div class="cues">
          <span class="muted">Cues</span>
          <button class="btn ghost small" onClick={() => setEditingCues(true)}>Edit cues</button>
          {clip.events.map((event, index) => (
            <button key={index} class={'cue ' + event.kind + (Math.abs(event.time - time) < 0.04 ? ' now' : '')} title={`${event.kind} at ${event.time.toFixed(2)} s`} onClick={() => { scene.current?.pause(); setPlaying(false); scene.current?.seek(event.time); setTime(event.time); }}>
              <span class="when">{event.time.toFixed(2)}</span> {event.payload.replace(/\u001f/g, ' · ')}
            </button>
          ))}
        </div>
      )}

      {clip && editingCues && <div class="cues"><button class="btn ghost small" onClick={() => setEditingCues(false)}>Done editing cues</button></div>}

      {model && <PhysicsPanel model={model} reload={reload} scene={scene} />}

      {managing && animation && <ClipManager rel={animation} clips={clips} onClose={() => setManaging(false)} onDeleted={() => { setManaging(false); setReload(count => count + 1); }} />}

      {blockbenchDialog && <BlockbenchDialog onClose={() => setBlockbenchDialog(false)} />}

      {pickModel && files && (
        <Dialog title="Edit the animation on which model?" onClose={() => setPickModel(null)}>
          <p class="muted" style="margin:0 0 8px">Blockbench needs her model to show the animation on.</p>
          <div style="display:flex;flex-direction:column;gap:6px">
            {files.models.map(m => (
              <label key={m} class="check-inline"><input type="radio" name="bbmodel" checked={pickModel === m} onChange={() => setPickModel(m)} /> {m.includes('/nude/') ? 'Nude model' : m.includes('/dressed/') ? 'Dressed model' : nameOf(m)} <span class="muted">{m}</span></label>
            ))}
          </div>
          <div class="foot">
            <button class="btn ghost" onClick={() => setPickModel(null)}>Cancel</button>
            <button class="btn primary" onClick={() => { const chosen = pickModel; setPickModel(null); void openInBlockbench(chosen, rel); }}>Open in Blockbench</button>
          </div>
        </Dialog>
      )}

      {expressions > 0 && <div class="viewer-note">{expressions} keyframes use Molang expressions. The viewer works them out (query.anim_time and math.*; a query it does not know counts as 0), so check one in the game if it matters.</div>}
    </div>
  );
}
