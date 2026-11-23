import { useEffect, useState } from 'preact/hooks';
import { api, pickFile, pickFolder } from './api';
import { ConvertPanel } from './ConvertPanel';
import { Dialog } from './Dialogs';
import { GirlReview, type Draft } from './GirlReview';
import { messageOf, notify } from './notify';
import type { Choices, ImportResult, ScenePlan } from './types';

interface GirlChoice {
  on: boolean;
  bodyType: string;
  profile: string;
  /** The scenes settled in the review, or null to use the ones her profile lists. */
  scenes: ScenePlan[] | null;
  reviewed: boolean;
}

interface CueChoice {
  count: number;
  action: 'marker' | 'sound' | 'custom' | 'leave_out';
  value: string;
  authority: number;
}

const ACTIONS: { id: CueChoice['action']; label: string }[] = [
  { id: 'marker', label: 'Keep as a cue (default)' },
  { id: 'sound', label: 'Play a sound' },
  { id: 'custom', label: 'A custom event' },
  { id: 'leave_out', label: 'Leave out' }
];

const AUTHORITIES = [
  { id: 0, label: 'Let the event type decide' },
  { id: 1, label: 'Server' },
  { id: 2, label: 'Client' },
  { id: 3, label: 'Both' }
];

/**
 * Brings old girls in. A resource pack (a folder or a zip) is opened to see the girls in it; any number of them can be chosen at once, each with her own body type and
 * her old profile (found by name in a profiles folder, or picked one by one). Her cues can be given a different meaning before the animations are converted, and a basic
 * physics file can be made for a model that came without one. With no pack, a profile alone brings in the girl file and her scenes.
 */
export function ImportDialog({ onImported, onClose }: { onImported: (result: ImportResult) => void; onClose: () => void }) {
  const [choices, setChoices] = useState<Choices | null>(null);
  const [pack, setPack] = useState('');
  const [girls, setGirls] = useState<Record<string, GirlChoice>>({});
  const [profilesFolder, setProfilesFolder] = useState('');
  const [profile, setProfile] = useState('');
  const [bodyType, setBodyType] = useState('');
  const [liquid, setLiquid] = useState(true);
  const [physics, setPhysics] = useState(true);
  const [cues, setCues] = useState<Record<string, CueChoice> | null>(null);
  const [showCues, setShowCues] = useState(false);
  const [busy, setBusy] = useState(false);
  const [mode, setMode] = useState<'girls' | 'file'>('girls');
  const [rewritten, setRewritten] = useState(false);
  const [filter, setFilter] = useState('');
  /** The girls the project already has (by the id her file is named with), who are left out of an import. */
  const [have, setHave] = useState<Set<string>>(new Set());
  const [reviewing, setReviewing] = useState(false);
  const [guessing, setGuessing] = useState('');

  useEffect(() => {
    void api.choices().then(setChoices);
  }, []);

  const hasAlready = (id: string) => have.has(id.toLowerCase());
  const chosen = Object.entries(girls).filter(([id, g]) => g.on && !hasAlready(id)).map(([id]) => id);

  /* A pack that is chosen is opened to see which girls are in it. */
  const look = async (path: string) => {
    setPack(path);
    setGirls({});
    setCues(null);
    setRewritten(false);

    if (!path.trim()) return;

    try {
      const found = await api.oldPackGirls(path.trim());
      const files = await api.listFiles().catch(() => [] as string[]);
      const existing = new Set(files.map(file => /^data\/[^/]+\/girls\/([^/]+)\.json$/.exec(file)?.[1]?.toLowerCase()).filter((id): id is string => !!id));
      const firstNew = found.girls.find(id => !existing.has(id.toLowerCase()));

      setHave(existing);
      setRewritten(found.rewritten);
      setGirls(Object.fromEntries(found.girls.map(id => [id, { on: id === firstNew, bodyType, profile: '', scenes: null, reviewed: false }])));
    } catch (error) {
      notify(messageOf(error));
    }
  };

  const browsePack = async () => {
    const picked = (await pickFolder('Choose the old resource pack folder')) ?? '';

    if (picked) await look(picked);
  };

  const browsePackZip = async () => {
    const picked = (await pickFile('Choose the old resource pack (.zip)', ['zip'])) ?? '';

    if (picked) await look(picked);
  };

  const setGirl = (id: string, change: Partial<GirlChoice>) => setGirls(previous => ({ ...previous, [id]: { ...previous[id], ...change } }));

  const shown = Object.keys(girls).filter(id => id.toLowerCase().includes(filter.trim().toLowerCase()));

  const selectAll = (on: boolean) => setGirls(previous => Object.fromEntries(Object.entries(previous).map(([id, g]) => [id, shown.includes(id) && !hasAlready(id) ? { ...g, on } : g])));

  /* Her body type guessed from how like the game's own skeletons her model's bones are, for each girl who has none yet. */
  const guessBodyTypes = async () => {
    const todo = (chosen.length > 0 ? chosen : shown.filter(id => !hasAlready(id))).filter(id => girls[id].bodyType === '');
    let done = 0;

    for (const id of todo) {
      setGuessing(`Looking at ${id} (${++done} of ${todo.length})…`);

      try {
        const found = await api.oldPackLook(pack.trim(), id, girls[id].profile.trim() || null);
        const guess = found.bones.length > 0 ? await api.guessBodyType(found.bones) : null;

        if (guess) setGirl(id, { bodyType: guess });
      } catch {
        /* A girl who cannot be looked at keeps no guess; the person can still choose. */
      }
    }

    setGuessing('');
  };

  const reviewIds = chosen.length > 0 ? chosen : shown.filter(id => !hasAlready(id));

  const finishReview = (drafts: Record<string, Draft>) => {
    setGirls(previous => {
      const next = { ...previous };

      for (const [id, d] of Object.entries(drafts)) {
        if (next[id]) next[id] = { ...next[id], on: d.include, bodyType: d.bodyType, profile: d.profile, scenes: d.scenes, reviewed: d.reviewed };
      }

      return next;
    });
    setReviewing(false);
  };

  /* A profiles folder holds one file for each girl, named for her. */
  const lookForProfiles = async (folder: string) => {
    setProfilesFolder(folder);

    if (!folder.trim()) return;

    const ids = Object.keys(girls);
    const found = await api.findProfiles(folder.trim(), ids).catch(() => [] as (string | null)[]);

    setGirls(previous => {
      const next = { ...previous };

      ids.forEach((id, i) => { if (found[i]) next[id] = { ...next[id], profile: found[i]! }; });
      return next;
    });
  };

  /* With one girl in the pack her profile is one file, so it is asked for as a file; with several, a folder holding one for each. */
  const onlyGirl = Object.keys(girls).length === 1 ? Object.keys(girls)[0] : null;

  const useProfilePath = async (path: string) => {
    if (onlyGirl && (path.trim() === '' || /\.json$/i.test(path.trim()))) {
      setProfilesFolder(path);
      setGirl(onlyGirl, { profile: path.trim() });
    } else await lookForProfiles(path);
  };

  const browseProfilesFolder = async () => {
    const picked = onlyGirl
      ? await pickFile(`Choose ${onlyGirl}'s old profile (.json)`, ['json'])
      : await pickFolder("Choose the folder with the old girls' profiles (one .json for each girl)");

    if (picked) await useProfilePath(picked);
  };

  const browseProfile = async (id: string | null) => {
    const picked = await pickFile("Choose the old girl's profile (.json)", ['json']);

    if (!picked) return;
    if (id) setGirl(id, { profile: picked });
    else setProfile(picked);
  };

  const setAllBodyTypes = (value: string) => {
    setBodyType(value);
    setGirls(previous => Object.fromEntries(Object.entries(previous).map(([id, g]) => [id, { ...g, bodyType: g.bodyType || value }])));
  };

  /* The cues of the chosen girls, with what each does now. */
  const loadCues = async () => {
    setShowCues(true);

    try {
      const counts = new Map<string, number>();

      for (const id of chosen) {
        for (const cue of await api.oldPackCues(pack.trim(), id)) counts.set(cue.token, (counts.get(cue.token) ?? 0) + cue.count);
      }

      setCues(previous => Object.fromEntries([...counts.entries()].sort().map(([token, count]) => [token, { action: 'marker', value: '', authority: 0, ...(previous?.[token] ?? {}), count } as CueChoice])));
    } catch (error) {
      notify(messageOf(error));
    }
  };

  const setCue = (token: string, change: Partial<CueChoice>) => setCues(previous => (previous ? { ...previous, [token]: { ...previous[token], ...change } } : previous));

  const run = async () => {
    setBusy(true);

    try {
      if (pack.trim()) {
        const rules = Object.entries(cues ?? {})
          .filter(([, c]) => c.action !== 'marker' || c.authority !== 0)
          .map(([token, c]) => ({ token, action: c.action, value: c.value.trim() || null, authority: c.authority }));
        const jobs = chosen.map(id => ({ girl: id, profile: girls[id].profile.trim() || null, body_type: girls[id].bodyType, scenes: girls[id].reviewed ? girls[id].scenes : null }));

        onImported(await api.importOldPack(pack.trim(), jobs, liquid, physics, rules));
      } else {
        onImported(await api.importProfile(profile.trim(), bodyType));
      }
    } catch (error) {
      notify(messageOf(error));
      setBusy(false);
    }
  };

  const bodyTypes = choices?.body_types ?? [];
  const ready = pack.trim() ? chosen.length > 0 && chosen.every(id => girls[id].bodyType !== '') : profile.trim() !== '' && bodyType !== '';

  const haveGirls = Object.keys(girls).length > 0;
  const leftOut = Object.keys(girls).filter(hasAlready).length;
  const needBody = chosen.filter(id => girls[id].bodyType === '').length;

  return (
    <>
    <Dialog title="Import and convert" onClose={onClose}>
      <div class="imp">
        <div class="imp-tabs">
          <button class={'seg' + (mode === 'girls' ? ' on' : '')} onClick={() => setMode('girls')}>Old girls from a resource pack</button>
          <button class={'seg' + (mode === 'file' ? ' on' : '')} onClick={() => setMode('file')}>Convert a model or animation file</button>
        </div>

        {mode === 'file' && <div class="imp-scroll"><ConvertPanel onDone={onImported} /></div>}

        {mode === 'girls' && <>
        <div class="imp-body">
          <section class="imp-main">
            <p class="muted imp-intro">Bring in old girls from their resource pack (models, animations, texture, sounds and cues are converted) and their old profiles. A profile alone brings in the girl file and her scenes. Your old files are not changed.</p>

            <div class="imp-sources">
              <div class="field">
                <label>Old resource pack <span class="muted">(folder or zip)</span></label>
                <div class="row">
                  <input class="input grow" value={pack} placeholder="C:\...\resourcepacks\Amber" onInput={e => void look((e.target as HTMLInputElement).value)} />
                  <button class="btn" onClick={browsePack}>Folder…</button>
                  <button class="btn" onClick={browsePackZip}>Zip…</button>
                </div>
              </div>
              <div class="field">
                <label>{onlyGirl ? 'Old profile' : 'Old profiles'} <span class="muted">{onlyGirl ? '(optional, her .json file)' : '(optional, one file per girl)'}</span></label>
                <div class="row">
                  <input class="input grow" value={profilesFolder} disabled={!haveGirls} placeholder={haveGirls ? (onlyGirl ? 'C:\\...\\config\\pleasurehorizons\\girl\\amber.json' : 'C:\\...\\config\\pleasurehorizons\\girl') : 'Open a pack first'} onInput={e => void useProfilePath((e.target as HTMLInputElement).value)} />
                  <button class="btn" disabled={!haveGirls} onClick={browseProfilesFolder}>Browse…</button>
                </div>
              </div>
            </div>

            {rewritten && <div class="hint">This pack was made for the old mod (one model, one animation file, a folder of sounds for each cue). It is converted as it is opened: one model becomes both of her models, scenes are found from the names of her clips, and each cue folder becomes a sound her animations play.</div>}

            {haveGirls && (
              <div class="imp-girls-wrap">
                <div class="imp-toolbar">
                  <input class="input grow" value={filter} placeholder="Find a girl by name…" onInput={e => setFilter((e.target as HTMLInputElement).value)} />
                  <button class="btn ghost small" onClick={() => selectAll(true)}>Select {filter.trim() ? 'these' : 'all'}</button>
                  <button class="btn ghost small" onClick={() => selectAll(false)}>Select none</button>
                </div>
                <div class="imp-count muted">{chosen.length} of {Object.keys(girls).length} chosen{leftOut > 0 ? ` · ${leftOut} already in the project and left out` : ''}{filter.trim() ? ` · showing ${shown.length}` : ''}</div>
                <div class="import-girls">
                  <div class="import-girl import-head">
                    <span class="ig-name">Girl</span>
                    <span class="ig-body">Body type</span>
                    <span class="ig-profile">Old profile</span>
                  </div>
                  {shown.length === 0 && <div class="muted" style="padding:10px">No girl matches that name.</div>}
                  {shown.map(id => [id, girls[id]] as const).map(([id, g]) => (
                    <div class={'import-girl' + (hasAlready(id) ? ' gone' : '') + (g.on && !hasAlready(id) ? ' on' : '')} key={id}>
                      <label class="check-inline ig-name" title={hasAlready(id) ? 'The project already has a girl file for her, so she is left out' : ''}>
                        <input type="checkbox" checked={g.on && !hasAlready(id)} disabled={hasAlready(id)} onChange={e => setGirl(id, { on: (e.target as HTMLInputElement).checked })} />
                        <b>{id}</b>
                        {hasAlready(id) && <span class="muted"> · in project</span>}
                      </label>
                      <span class="ig-body">
                        <select class={'input small-select' + (g.on && !hasAlready(id) && g.bodyType === '' ? ' need' : '')} value={g.bodyType} disabled={hasAlready(id)} onChange={e => setGirl(id, { bodyType: (e.target as HTMLSelectElement).value })} title="The skeleton her model was built from">
                          <option value="">Body type…</option>
                          {bodyTypes.map(b => <option key={b} value={b}>{b}</option>)}
                        </select>
                      </span>
                      <span class="ig-profile">
                        {g.reviewed && <span class="muted" title="Looked over in the review">✓ {g.scenes ? `${g.scenes.length} scene${g.scenes.length === 1 ? '' : 's'}` : 'seen'}</span>}
                        <span class="muted import-profile" title={g.profile || 'No old profile: a plain one is made'}>{g.profile ? g.profile.split(/[\\/]/).pop() : 'no profile'}</span>
                        <button class="btn ghost small" disabled={hasAlready(id)} onClick={() => void browseProfile(id)}>Profile…</button>
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {!pack.trim() && (
              <div class="imp-sources">
                <div class="field">
                  <label>Or just her old profile</label>
                  <div class="row">
                    <input class="input grow" value={profile} placeholder="C:\...\config\pleasurehorizons\girl\amber.json" onInput={e => setProfile((e.target as HTMLInputElement).value)} />
                    <button class="btn" onClick={() => void browseProfile(null)}>Browse…</button>
                  </div>
                </div>
                <div class="field">
                  <label>Body type</label>
                  <select class="input" value={bodyType} onChange={e => setBodyType((e.target as HTMLSelectElement).value)}>
                    <option value="">Choose…</option>
                    {bodyTypes.map(b => <option key={b} value={b}>{b}</option>)}
                  </select>
                </div>
              </div>
            )}
          </section>

          {haveGirls && (
            <aside class="imp-side">
              <div class="imp-card">
                <h4>Body types</h4>
                <p class="hint">The skeleton her model was built from. An old profile does not say, and a wrong one lets scenes play that do not fit her.</p>
                <div class="row">
                  <span class="muted">Fill in empty ones:</span>
                  <select class="input small-select grow" value={bodyType} onChange={e => setAllBodyTypes((e.target as HTMLSelectElement).value)}>
                    <option value="">Choose…</option>
                    {bodyTypes.map(b => <option key={b} value={b}>{b}</option>)}
                  </select>
                </div>
                <button class="btn small" disabled={guessing !== ''} title="Guess each chosen girl's body type from how like the game's own skeletons her model's bones are" onClick={() => void guessBodyTypes()}>{guessing || 'Guess body types'}</button>
              </div>

              <div class="imp-card">
                <h4>Review</h4>
                <p class="hint">Go through the girls one at a time: see her model, play her animations, and check or change which scenes they make up.</p>
                <button class="btn small" disabled={reviewIds.length === 0} onClick={() => setReviewing(true)}>Review {chosen.length > 0 ? 'the chosen' : 'these'} one by one…</button>
              </div>

              <div class="imp-card">
                <h4>Options</h4>
                <label class="check-inline"><input type="checkbox" checked={liquid} onChange={e => setLiquid((e.target as HTMLInputElement).checked)} /> <span>Show the fluid on the cum cue <span class="muted">(needs a "nut" bone in the nude model)</span></span></label>
                <label class="check-inline"><input type="checkbox" checked={physics} onChange={e => setPhysics((e.target as HTMLInputElement).checked)} /> <span>Make a basic physics file for a model that has none <span class="muted">(boobL, boobR, belly)</span></span></label>
              </div>

              <details class="more imp-card" open={showCues} onToggle={e => { if ((e.target as HTMLDetailsElement).open && !cues) void loadCues(); }}>
                <summary>What the cues do (optional)</summary>
                <p class="hint">By default every cue stays a cue and the sounds come from the girl's cue table. Change a cue to play a sound directly, fire a custom event, or leave it out.</p>
                {!cues && showCues && chosen.length > 0 && <div class="muted">Looking at the animations…</div>}
                {cues && Object.keys(cues).length === 0 && <div class="muted">The chosen girls' animations fire no cues.</div>}
                {cues && Object.entries(cues).map(([token, c]) => (
                  <div class="cue-rule" key={token}>
                    <span class="cue-rule-name"><b>{token}</b> <span class="muted">x{c.count}</span></span>
                    <select class="input small-select" value={c.action} onChange={e => setCue(token, { action: (e.target as HTMLSelectElement).value as CueChoice['action'] })}>
                      {ACTIONS.map(a => <option key={a.id} value={a.id}>{a.label}</option>)}
                    </select>
                    {(c.action === 'sound' || c.action === 'custom') && (
                      <input class="input grow" value={c.value} placeholder={c.action === 'sound' ? 'pleasurehorizons:mika.moan (empty: the cue name)' : 'pleasurehorizons:fade 1.5'} onInput={e => setCue(token, { value: (e.target as HTMLInputElement).value })} />
                    )}
                    {c.action !== 'leave_out' && (
                      <select class="input small-select" value={c.authority} title="Who decides that it fires" onChange={e => setCue(token, { authority: Number((e.target as HTMLSelectElement).value) })}>
                        {AUTHORITIES.map(a => <option key={a.id} value={a.id}>{a.label}</option>)}
                      </select>
                    )}
                  </div>
                ))}
              </details>
            </aside>
          )}
        </div>

        <div class="foot imp-foot">
          <span class="muted imp-status">{pack.trim() ? (chosen.length === 0 ? 'Choose at least one girl.' : needBody > 0 ? `${needBody} chosen ${needBody === 1 ? 'girl needs' : 'girls need'} a body type.` : `${chosen.length} ${chosen.length === 1 ? 'girl' : 'girls'} ready.`) : ''}</span>
          <button class="btn ghost" onClick={onClose}>Cancel</button>
          <button class="btn primary" disabled={!ready || busy} onClick={run}>{busy ? 'Importing…' : chosen.length > 1 ? `Import ${chosen.length} girls` : 'Import'}</button>
        </div>
        </>}
      </div>
    </Dialog>

    {reviewing && (
        <GirlReview
          pack={pack.trim()}
          ids={reviewIds}
          bodyTypes={bodyTypes}
          initial={Object.fromEntries(reviewIds.map(id => [id, { include: true, bodyType: girls[id].bodyType, profile: girls[id].profile, scenes: girls[id].scenes, reviewed: girls[id].reviewed } as Draft]))}
          onDone={finishReview}
          onClose={() => setReviewing(false)}
        />
      )}
    </>
  );
}
