import { useEffect, useRef, useState } from 'preact/hooks';
import { api } from '../api';
import { BlockbenchDialog } from '../BlockbenchDialog';
import { Dialog } from '../Dialogs';
import { messageOf, notify } from '../notify';
import type { ArmFixReport, ClipData, ViewerFiles } from '../types';
import { ClipManager } from '../ClipManager';
import { CueEditor } from './CueEditor';
import { PhysicsPanel } from './PhysicsPanel';
import { createViewer, type Shading, type Smoothing, type Viewer as Scene } from './scene';

const nameOf = (rel: string) => rel.slice(rel.lastIndexOf('/') + 1);
const SPEEDS = [0.25, 0.5, 1, 2];

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
  const [armReport, setArmReport] = useState<ArmFixReport | null>(null);
  const [armBusy, setArmBusy] = useState(false);
  const [managing, setManaging] = useState(false);

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

  /* The model and its texture. */
  useEffect(() => {
    if (!model || !files) return;

    let current = true;

    void (async () => {
      try {
        const data = await api.readModel(model);
        const texture = files.texture ? await api.readImage(files.texture).catch(() => null) : null;

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

  /* Asks what fixing the lower arms would do, and shows it before anything is changed. */
  const previewArmFix = async () => {
    setArmBusy(true);

    try {
      const report = await api.fixArms(rel, false);

      if (report.fixed.length === 0 && report.skipped.length === 0 && report.removed.length === 0) notify('The lower arms of this model are already straight and it has no old item bones: nothing to fix.');
      else setArmReport(report);
    } catch (error) {
      notify(messageOf(error));
    } finally {
      setArmBusy(false);
    }
  };

  const applyArmFix = async () => {
    setArmBusy(true);

    try {
      const report = await api.fixArms(rel, true);

      setArmReport(null);
      setReload(count => count + 1);
      notify(`Fixed ${report.fixed.length} lower arm${report.fixed.length === 1 ? '' : 's'} and removed ${report.removed.length} old item bone${report.removed.length === 1 ? '' : 's'}. A copy of the model as it was is kept in the project's .packcreator/backups folder.`);
    } catch (error) {
      notify(messageOf(error));
    } finally {
      setArmBusy(false);
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
        {isModel && <button class="btn small" disabled={armBusy} title="Straightens a lower arm bone that is turned (and turned back in its cubes), keeping the arm looking the same, and removes the old weapon, offhand and customHand bones." onClick={previewArmFix}>Fix arms</button>}
        <span class="grow" />
        <select class="input small-select" value={shading} title="How the model is drawn" onChange={e => setShading((e.target as HTMLSelectElement).value as Shading)}>
          <option value="lit">Lit</option>
          <option value="flat">Flat (texture only)</option>
          <option value="normals">Normals</option>
          <option value="wire">Wireframe</option>
        </select>
        <select class="input small-select" value={smoothing} title="Whether neighbouring faces of a mesh blend their shading" onChange={e => setSmoothing((e.target as HTMLSelectElement).value as Smoothing)}>
          <option value="authored">Smooth: as authored</option>
          <option value="smooth">Smooth: all meshes</option>
          <option value="flat">Smooth: off</option>
        </select>
        <label class="check-inline" title="How bright the model is"><span class="muted">Brightness</span> <input type="range" class="bright" min={0.2} max={2} step={0.05} value={brightness} onInput={e => setBrightness(Number((e.target as HTMLInputElement).value))} /></label>
        <label class="check-inline"><input type="checkbox" checked={grid} onChange={e => setGrid((e.target as HTMLInputElement).checked)} /> Grid</label>
        <label class="check-inline" title="The scene rig: the male's body, items, armour and cameras"><input type="checkbox" checked={!rigHidden} onChange={e => setRigHidden(!(e.target as HTMLInputElement).checked)} /> Scene rig</label>
        <span class="muted">{info}</span>
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

      {armReport && (
        <Dialog title="Fix the lower arms" onClose={() => setArmReport(null)}>
          {armReport.fixed.length > 0 && (
            <div class="field">
              <label>What will change</label>
              {armReport.fixed.map(item => (
                <div key={item.bone} class="hint">
                  <strong>{item.bone}</strong>: the bone's rotation {item.oldRotation}° becomes 0°.
                  {item.cubes.map(cube => <div key={cube.name}>&nbsp;&nbsp;{cube.name}: pivot moves to the bone's, rotation {cube.oldRotation}° becomes {cube.newRotation}°, and its box is moved so it stays where it is.</div>)}
                  {item.children.length > 0 && <div>&nbsp;&nbsp;{item.children.join(', ')} hangs off this bone and is no longer turned by it.</div>}
                </div>
              ))}
              <div class="hint">The arm looks the same at rest. Animations that turn the lower arm are relative to the bone's rest rotation, so look over the arms in them afterwards.</div>
            </div>
          )}
          {armReport.removed.length > 0 && (
            <div class="field">
              <label>Old item bones that will be removed</label>
              <div class="hint">{armReport.removed.join(', ')}. The game places held items itself, so these are only in the way.</div>
            </div>
          )}
          {armReport.skipped.length > 0 && (
            <div class="field">
              <label>Left alone</label>
              {armReport.skipped.map(item => <div key={item.bone} class="hint"><strong>{item.bone}</strong>: {item.reason}.</div>)}
            </div>
          )}
          <div class="foot">
            <button class="btn ghost" onClick={() => setArmReport(null)}>Cancel</button>
            <button class="btn" disabled={armBusy || (armReport.fixed.length === 0 && armReport.removed.length === 0)} onClick={applyArmFix}>{armBusy ? 'Fixing...' : 'Fix them'}</button>
          </div>
        </Dialog>
      )}

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
