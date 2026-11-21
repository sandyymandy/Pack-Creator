import { useEffect, useState } from 'preact/hooks';
import type { RefObject } from 'preact';
import { api } from '../api';
import { messageOf, notify } from '../notify';
import { MOTIONS, parsePhysics, type Motion, type ParsedPhysics } from './physics';
import type { Viewer as Scene } from './scene';

/* =================================================================
 * Physics Panel
 * ================================================================= */

/** Where the physics file for a model is: `strut/models/<rest>.strutmodel` has `strut/physics/models/<rest>.json`. */
export function physicsPathOf(model: string): string | null {
  return model.includes('/strut/models/') && model.endsWith('.strutmodel')
    ? model.replace('/strut/models/', '/strut/physics/models/').replace(/\.strutmodel$/, '.json')
    : null;
}

const show = (value: unknown): string => {
  if (Array.isArray(value)) return `[${value.map(show).join(', ')}]`;
  if (value && typeof value === 'object') return Object.entries(value).map(([key, inner]) => `${key} ${show(inner)}`).join(', ');

  return String(value);
};

interface SoftBone {
  name: string;
  preset: string;
  settings: [string, string][];
}

/**
 * A model's physics file: which bones are soft and how, with the soft bones simulated in the 3D view the way the game does it. **Add basic physics** puts the usual setup (chest,
 * belly, cheeks, ass: whichever of those bones the model has, found by name) in the file in one click, to be tuned afterwards in Blockbench's Physics tab or by hand in the JSON.
 */
export function PhysicsPanel({ model, reload, scene }: { model: string; reload: number; scene: RefObject<Scene | null> }) {
  const [bones, setBones] = useState<SoftBone[] | null>(null);
  const [chains, setChains] = useState<string[]>([]);
  const [parsed, setParsed] = useState<ParsedPhysics | null>(null);
  const [missing, setMissing] = useState<string[]>([]);
  const [modelBones, setModelBones] = useState<string[]>([]);
  const [problem, setProblem] = useState<string | null>(null);
  const [on, setOn] = useState(false);
  const [motion, setMotion] = useState<Motion>('walk');
  const [wind, setWind] = useState(0);
  const [refresh, setRefresh] = useState(0);
  const [busy, setBusy] = useState(false);

  const path = physicsPathOf(model);

  useEffect(() => {
    let current = true;

    setBones(null);
    setChains([]);
    setParsed(null);
    setMissing([]);
    setProblem(null);
    scene.current?.setPhysics(null);

    if (!path) return;

    void api.readFile(path).then(content => {
      if (!current) return;

      if (content.type !== 'text') {
        setProblem('The physics file could not be read.');
        return;
      }

      const root = JSON.parse(content.text) as { bones?: Record<string, Record<string, unknown>>; chains?: Record<string, unknown> };
      const result = parsePhysics(content.text, path);

      setBones(Object.entries(root.bones ?? {}).map(([name, bone]) => ({
        name,
        preset: String(bone.preset ?? 'custom'),
        settings: Object.entries(bone).filter(([key]) => key !== 'preset').map(([key, value]) => [key, show(value)] as [string, string])
      })));
      setChains(Object.keys(root.chains ?? {}));
      setParsed(result);
      scene.current?.setPhysics(result.bones);
    }).catch(() => { if (current) setBones([]); });

    return () => { current = false; };
  }, [model, reload, refresh]);

  useEffect(() => { scene.current?.setPhysicsRun({ on, motion, wind }); }, [on, motion, wind, parsed]);

  /* The scene says which soft bones the model has no bone for, whenever the model or the file changes. */
  useEffect(() => {
    scene.current?.onPhysicsBound(bound => { setMissing(bound.missing); setModelBones(bound.names); });

    return () => scene.current?.onPhysicsBound(null);
  }, []);

  /* Leaving the panel (another model, or the viewer closing) puts the bones back. */
  useEffect(() => () => { scene.current?.setPhysicsRun({ on: false, motion: 'walk', wind: 0 }); scene.current?.setPhysics(null); }, []);

  const addBasic = async () => {
    setBusy(true);

    try {
      const result = await api.addBasicPhysics(model);

      setRefresh(n => n + 1);
      notify(`Added soft bodies for ${result.added.join(', ')}. Tune them in ${result.path}.`, true);
    } catch (error) {
      notify(messageOf(error));
    } finally {
      setBusy(false);
    }
  };

  if (problem) return <div class="physics"><span class="muted">{problem}</span></div>;

  if (bones === null) return null;

  const soft = parsed?.bones.size ?? 0;
  const softBodies = parsed?.soft.length ?? 0;

  return (
    <div class="physics">
      <span class="muted">Physics</span>
      {bones.length === 0 && chains.length === 0 && <span class="muted">This model has no physics file. Add soft bodies in one click here, or set bones in Blockbench's Physics tab.</span>}
      {bones.map(bone => (
        <span key={bone.name} class={'physics-bone' + (missing.includes(bone.name) ? ' missing' : '')} title={(missing.includes(bone.name) ? 'This model has no bone with this name, so nothing moves.\n' : '') + bone.settings.map(([key, value]) => `${key}: ${value}`).join('\n')}>
          <strong>{bone.name}</strong> <span class="muted">{bone.preset}</span>
          {bone.settings.filter(([key]) => ['frequency', 'damping', 'length', 'squish'].includes(key)).map(([key, value]) => <span key={key} class="muted"> {key} {value}</span>)}
          {missing.includes(bone.name) && <span class="muted"> not in this model</span>}
        </span>
      ))}
      {chains.map(name => <span key={name} class="physics-bone"><strong>{name}</strong> <span class="muted">chain</span></span>)}
      {(parsed?.soft ?? []).map(entry => {
        const absent = modelBones.length > 0 ? entry.bones.filter(b => !modelBones.includes(b)) : [];

        return (
          <span key={entry.name} class={'physics-bone' + (absent.length > 0 ? ' missing' : '')} title={(absent.length > 0 ? `The model has no bone called ${absent.join(', ')}.\n` : '') + 'A soft body: the game moves the meshes of these bones as one piece that swings, presses and squeezes. The viewer lists it; it does not simulate it.'}>
            <strong>{entry.name}</strong> <span class="muted">soft {entry.preset}</span> <span class="muted">{entry.bones.join(', ')}</span>
            {absent.length > 0 && <span class="muted"> not in this model</span>}
          </span>
        );
      })}

      {path && (
        <button class="btn small" disabled={busy} title="Puts the usual soft bodies in the physics file: chest, bottom and belly, whichever of them this model has (found by name: boobL, boobR, boobs, belly, cheekL, cheekR, assL, assR...), as Strut's soft presets. Bones the file already has are left as they are." onClick={addBasic}>Add basic physics</button>
      )}

      {soft > 0 && (
        <div class="physics-run">
          <label class="check-inline" title="Moves the soft bones the way the game does: they feel the model's own motion"><input type="checkbox" checked={on} onChange={e => setOn((e.target as HTMLInputElement).checked)} /> Simulate</label>
          <select class="input small-select" value={motion} disabled={!on} title="How the model is moved to wake its soft bones" onChange={e => setMotion((e.target as HTMLSelectElement).value as Motion)}>
            {MOTIONS.map(m => <option key={m.id} value={m.id}>{m.label}</option>)}
          </select>
          <button class="btn small" disabled={!on} title="A quick shove forward and back" onClick={() => scene.current?.bumpPhysics()}>Bump</button>
          <label class="check-inline" title="A steady push to one side, in blocks per second squared"><span class="muted">Wind</span> <input type="range" class="bright" min={0} max={12} step={0.5} value={wind} disabled={!on} onInput={e => setWind(Number((e.target as HTMLInputElement).value))} /></label>
          <button class="btn ghost small" disabled={!on} title="Put the soft bones back to rest" onClick={() => scene.current?.resetPhysics()}>Reset</button>
        </div>
      )}

      {parsed && parsed.skipped.length > 0 && <span class="muted">Not simulated here (the game does): {parsed.skipped.join(', ')}.</span>}
      {parsed && parsed.warnings.length > 0 && <span class="muted" title={parsed.warnings.join('\n')}>{parsed.warnings.length} warning{parsed.warnings.length === 1 ? '' : 's'} in the file (hover).</span>}
      {soft > 0 && <span class="muted">Collisions and squish are left out: the spring bones move from the model's own motion, gravity and wind.</span>}
      {softBodies > 0 && <span class="muted">Soft bodies are moved by the game, not here: the viewer lists them but does not simulate them.</span>}
    </div>
  );
}
