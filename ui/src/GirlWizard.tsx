import { useEffect, useState } from 'preact/hooks';
import { api, pickFile } from './api';
import { messageOf, notify } from './notify';
import type { Choices, SimGirl } from './types';

interface Report {
  nude: { ok: boolean; message: string } | null;
  dressed: { ok: boolean; message: string } | null;
  texture: string | null;
  skeleton: { reference: string | null; same: boolean; missing: string[]; extra: string[]; order_differs: boolean } | null;
}

interface Created {
  girl: string;
  written: string[];
  notes: string[];
}

const STEPS = ['Who she is', 'Her skeleton', 'Her models', 'Check and make'];
const EXAMPLE = 'example:assets/pleasurehorizons/';

/**
 * The new girl wizard. A girl is more than her profile: she needs her nude and dressed models, her texture, her event bindings and her chat lines, and the models must be
 * built on the skeleton of her body type, or the scenes made for it will not move her right. This asks for each, checks the models against the skeleton, and then writes every
 * file together (the profile, the models, the texture, the bindings and chat lines, a basic physics file). She can start from one of the game's girls: her models are copied as
 * placeholders to edit in Blockbench.
 */
export function GirlWizard({ choices, onCreated }: { choices: Choices | null; onCreated: (rel: string) => void }) {
  const [step, setStep] = useState(0);
  const [id, setId] = useState('');
  const [name, setName] = useState('');
  const [archetype, setArchetype] = useState('settler');
  const [tameItem, setTameItem] = useState('minecraft:allium');
  const [bodyType, setBodyType] = useState('');
  const [mode, setMode] = useState<'mine' | 'example'>('example');
  const [nude, setNude] = useState('');
  const [dressed, setDressed] = useState('');
  const [texture, setTexture] = useState('');
  const [exampleId, setExampleId] = useState('');
  const [physics, setPhysics] = useState(true);
  const [examples, setExamples] = useState<SimGirl[]>([]);
  const [mine, setMine] = useState<SimGirl[]>([]);
  const [report, setReport] = useState<Report | null>(null);
  const [preview, setPreview] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [made, setMade] = useState<Created | null>(null);

  useEffect(() => {
    void api.exampleGirls().then(setExamples).catch(() => undefined);
    void api.simGirls().then(setMine).catch(() => undefined);
  }, []);

  const sameBody = examples.filter(g => g.body_type === bodyType);
  const files = mode === 'example' && exampleId ? { nude: `${EXAMPLE}strut/models/entities/girls/nude/${exampleId}.strutmodel`, dressed: `${EXAMPLE}strut/models/entities/girls/dressed/${exampleId}.strutmodel`, texture: `${EXAMPLE}textures/entities/${exampleId}.png` } : { nude, dressed, texture };

  /* Starting from a base girl of the body type chosen: the first one, until another is picked. */
  useEffect(() => {
    if (mode === 'example' && !sameBody.some(g => g.id === exampleId)) setExampleId(sameBody[0]?.id ?? '');
  }, [bodyType, mode, examples.length]);

  useEffect(() => {
    setPreview(null);

    if (mode === 'example' && exampleId) void api.examplePreview(exampleId).then(setPreview).catch(() => undefined);
  }, [mode, exampleId]);

  /* The models are checked as soon as they are chosen. */
  useEffect(() => {
    if (!bodyType || (!files.nude && !files.dressed && !files.texture)) { setReport(null); return; }

    let live = true;

    void api.checkGirlModels(bodyType, files.nude || null, files.dressed || null, files.texture || null).then(r => { if (live) setReport(r as Report); }).catch(() => undefined);

    return () => { live = false; };
  }, [bodyType, files.nude, files.dressed, files.texture]);

  const idBad = id !== '' && !/^[a-z0-9_]+$/.test(id);
  const idTaken = mine.some(g => g.id === id);
  const whoReady = id !== '' && !idBad && !idTaken;
  const modelsReady = !!files.nude && !!files.dressed && !!files.texture && !!report?.nude?.ok && !!report?.dressed?.ok && report?.texture !== undefined && !/not a PNG|could not be read|refuses/.test(report?.texture ?? '');

  const pick = async (title: string, extensions: string[], set: (path: string) => void) => {
    const chosen = await pickFile(title, extensions);

    if (chosen) set(chosen);
  };

  const make = async () => {
    setBusy(true);

    try {
      setMade(await api.createGirl({ id, name, archetype, body_type: bodyType, tame_item: tameItem, nude: files.nude, dressed: files.dressed, texture: files.texture, physics }));
    } catch (error) {
      notify(messageOf(error));
    } finally {
      setBusy(false);
    }
  };

  const mark = (ok: boolean | undefined) => (ok === undefined ? '' : ok ? '✓ ' : '✗ ');

  if (made) {
    return (
      <div>
        <p style="margin-top:0">{name || id} is made. These files were written:</p>
        <ul class="wiz-files">{made.written.map(w => <li key={w}>{w}</li>)}</ul>
        {made.notes.map(n => <div class="form-note" key={n}>{n}</div>)}
        <div class="foot"><button class="btn primary" onClick={() => onCreated(made.girl)}>Open her file</button></div>
      </div>
    );
  }

  return (
    <div class="wizard">
      <div class="wiz-steps">{STEPS.map((label, i) => <span key={label} class={'wiz-step' + (i === step ? ' on' : i < step ? ' done' : '')}>{i + 1}. {label}</span>)}</div>

      {step === 0 && (
        <div>
          <div class="field">
            <label>Id (the file's name)</label>
            <input class={'input' + (idBad || idTaken ? ' bad' : '')} autoFocus value={id} placeholder="amber" onInput={e => setId((e.target as HTMLInputElement).value)} />
            {idBad ? <span class="problem">Use lower case letters, digits and underscores only.</span> : idTaken ? <span class="problem">This pack already has a girl called {id}.</span> : <span class="hint">Lower case letters, digits and underscores. Her files are named for it.</span>}
          </div>
          <div class="field"><label>Her name</label><input class="input" value={name} placeholder="Amber" onInput={e => setName((e.target as HTMLInputElement).value)} /></div>
          <div class="row">
            <div class="field grow">
              <label>Kind of girl</label>
              <select class="input" value={archetype} onChange={e => setArchetype((e.target as HTMLSelectElement).value)}>{(choices?.archetypes ?? ['settler']).map(a => <option key={a} value={a}>{a}</option>)}</select>
            </div>
            <div class="field grow"><label>Tame item</label><input class="input" value={tameItem} onInput={e => setTameItem((e.target as HTMLInputElement).value)} /></div>
          </div>
          <span class="hint">The item that wins her over. Each girl needs a different one.</span>
        </div>
      )}

      {step === 1 && (
        <div>
          <p class="muted" style="margin-top:0">The skeleton is the set of bones her model is built from. Scenes are animated for one skeleton, so it decides which scenes she can play, and her models must have exactly its bones.</p>
          <div class="skeletons">
            {(choices?.body_types ?? []).map(b => (
              <div key={b} class={'kind' + (bodyType === b ? ' on' : '')} onClick={() => setBodyType(b)}>
                <b>{b}</b>
                <div class="muted" style="font-size:11px">{examples.filter(g => g.body_type === b).map(g => g.id).join(', ') || 'no base girl'}</div>
              </div>
            ))}
          </div>
        </div>
      )}

      {step === 2 && (
        <div>
          <div class="row" style="margin-bottom:8px">
            <label class="check-inline"><input type="radio" checked={mode === 'example'} onChange={() => setMode('example')} /> Start from one of the game's {bodyType} girls</label>
            <label class="check-inline"><input type="radio" checked={mode === 'mine'} onChange={() => setMode('mine')} /> I have her models</label>
          </div>

          {mode === 'example' && (
            <div>
              <p class="muted" style="margin-top:0">Her models and texture are copied from one of the game's girls, so she exists at once on the right skeleton. Change them in Blockbench afterwards (the Blockbench button).</p>
              {sameBody.length === 0 && <div class="form-note">The game has no base girl with the {bodyType} skeleton to copy. Bring her own models instead.</div>}
              <div class="row">
                <select class="input" value={exampleId} onChange={e => setExampleId((e.target as HTMLSelectElement).value)}>{sameBody.map(g => <option key={g.id} value={g.id}>{g.id}</option>)}</select>
                {preview && <img class="wiz-preview" src={preview} alt="" />}
              </div>
            </div>
          )}

          {mode === 'mine' && (
            <div>
              {([['Nude model (.strutmodel)', nude, setNude, ['strutmodel']], ['Dressed model (.strutmodel)', dressed, setDressed, ['strutmodel']], ['Texture (.png)', texture, setTexture, ['png']]] as const).map(([label, value, set, ext]) => (
                <div class="field" key={label}>
                  <label>{label}</label>
                  <div class="row">
                    <input class="input grow" value={value} onInput={e => set((e.target as HTMLInputElement).value)} />
                    <button class="btn" onClick={() => void pick(label, [...ext], set)}>Browse…</button>
                  </div>
                </div>
              ))}
              <span class="hint">Make them in Blockbench with the Strut Toolkit (Export Strut Model), built on the {bodyType} skeleton.</span>
            </div>
          )}

          {report && (
            <div class="wiz-report">
              {report.nude && <div class={report.nude.ok ? '' : 'problem'}>{mark(report.nude.ok)}Nude model: {report.nude.message}</div>}
              {report.dressed && <div class={report.dressed.ok ? '' : 'problem'}>{mark(report.dressed.ok)}Dressed model: {report.dressed.message}</div>}
              {report.texture && <div>Texture: {report.texture}</div>}
              {report.skeleton && (report.skeleton.same
                ? <div>✓ Her nude model is built on the {bodyType} skeleton ({report.skeleton.reference}'s).</div>
                : <div class="problem">✗ {report.skeleton.order_differs ? `Her bones are the ${bodyType} bones in a different order, which the game refuses.` : `Her nude model is not built on the ${bodyType} skeleton. Missing: ${report.skeleton.missing.join(', ') || 'none'}. Extra: ${report.skeleton.extra.join(', ') || 'none'}.`}</div>)}
            </div>
          )}
        </div>
      )}

      {step === 3 && (
        <div>
          <p style="margin-top:0"><b>{name || id}</b> ({id}), a {archetype} on the {bodyType} skeleton, tamed with {tameItem}.</p>
          <label class="check-inline"><input type="checkbox" checked={physics} onChange={e => setPhysics((e.target as HTMLInputElement).checked)} /> Make a basic physics file for her soft bones (boobL, boobR, belly)</label>
          <p class="muted">These will be made:</p>
          <ul class="wiz-files">
            <li>girls/{id}.json: her profile</li>
            <li>asset_sync/strut/models/entities/girls/nude/{id}.strutmodel and dressed/{id}.strutmodel</li>
            <li>asset_sync/textures/entities/{id}.png</li>
            <li>asset_sync/strut/event_bindings/girls/{id}.json: what her cues play and say (empty)</li>
            {physics && <li>asset_sync/strut/physics/models/.../{id}.json: basic physics</li>}
          </ul>
          <p class="muted">She plays the game's shared animations until she has her own, so every scene made for the {bodyType} skeleton already works on her.</p>
          {report?.skeleton && !report.skeleton.same && <div class="form-note">Her nude model is not on the {bodyType} skeleton: she will be made, but scenes made for it will not move her correctly.</div>}
        </div>
      )}

      <div class="foot">
        {step > 0 && <button class="btn ghost" onClick={() => setStep(step - 1)}>Back</button>}
        {step < 3 && <button class="btn primary" disabled={(step === 0 && !whoReady) || (step === 1 && !bodyType) || (step === 2 && !modelsReady)} onClick={() => setStep(step + 1)}>Next</button>}
        {step === 3 && <button class="btn primary" disabled={busy} onClick={() => void make()}>{busy ? 'Making her…' : 'Make her'}</button>}
      </div>
    </div>
  );
}
