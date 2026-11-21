import { useEffect, useState } from 'preact/hooks';
import { api } from './api';
import { Dialog } from './Dialogs';
import { messageOf, notify } from './notify';
import type { SceneScope, SimGirl } from './types';

/**
 * Who each scene is for. A scene made for one girl (her animation file `scene-girl`, and her name in the scene file) is made into one for her whole skeleton (`scene.body`, and no girl
 * named), or the other way round. The animation file is moved and the scene file changed together, so the two never disagree.
 */
export function ScopeDialog({ focus, onClose, onChanged }: { focus?: string; onClose: () => void; onChanged: () => void }) {
  const [scenes, setScenes] = useState<SceneScope[]>([]);
  const [girls, setGirls] = useState<SimGirl[]>([]);
  const [pick, setPick] = useState<Record<string, string>>({});
  const [keep, setKeep] = useState(false);
  const [filter, setFilter] = useState(focus ?? '');
  const [busy, setBusy] = useState(false);

  const load = async () => {
    try {
      setScenes(await api.sceneScopes());
    } catch (error) {
      notify(messageOf(error));
    }
  };

  useEffect(() => {
    void load();
    void api.simGirls().then(list => setGirls(list.filter(g => !g.example))).catch(() => undefined);
  }, []);

  const run = async (work: () => Promise<string[]>) => {
    setBusy(true);

    try {
      const notes = await work();

      notify(notes.join(' '), true);
      onChanged();
      await load();
    } catch (error) {
      notify(messageOf(error));
    } finally {
      setBusy(false);
    }
  };

  /* Every scene that is one girl's, made skeleton-wide; the ones that cannot be (a file already in the way) are said, not stopped at. */
  const everything = async () => {
    setBusy(true);

    let done = 0;
    const problems: string[] = [];

    for (const scene of scenes) {
      for (const file of scene.files.filter(f => f.character_kind === 'girl')) {
        try {
          await api.sceneMakeSkeletonWide(scene.id, file.character);
          done++;
        } catch (error) {
          problems.push(`${scene.id} (${file.character}): ${messageOf(error)}`);
        }
      }
    }

    notify(`${done} scene file${done === 1 ? '' : 's'} made skeleton-wide.${problems.length ? ' Left alone: ' + problems.join(' ') : ''}`, problems.length === 0);
    onChanged();
    await load();
    setBusy(false);
  };

  const shown = scenes.filter(s => s.id.includes(filter.trim().toLowerCase()));
  const aloneCount = scenes.reduce((n, s) => n + s.files.filter(f => f.character_kind === 'girl').length, 0);

  return (
    <Dialog title="Who each scene is for" onClose={onClose}>
      <div class="scope">
        <p class="muted" style="margin-top:0">A scene can be for one girl (her animation file and her name in the scene file) or for every girl on a skeleton. Making it skeleton-wide moves her animation file to <code>scene.skeleton.strutanim</code> and takes her name out of the scene file; the other way puts it back.</p>
        <div class="row" style="gap:6px;margin-bottom:8px;flex-wrap:wrap">
          <input class="input grow" value={filter} placeholder="Find a scene…" onInput={e => setFilter((e.target as HTMLInputElement).value)} />
          <button class="btn small" disabled={busy || aloneCount === 0} title="Every scene animation file that belongs to one girl becomes a skeleton one" onClick={() => void everything()}>Make all {aloneCount} skeleton-wide</button>
          <label class="check-inline" title="When giving a scene back to one girl, copy the skeleton file instead of moving it, so the other girls keep theirs"><input type="checkbox" checked={keep} onChange={e => setKeep((e.target as HTMLInputElement).checked)} /> Keep the skeleton file when going back</label>
        </div>
        <div class="scope-list">
          {shown.length === 0 && <span class="muted">No scenes.</span>}
          {shown.map(scene => (
            <div class="scope-card" key={scene.id}>
              <div class="row" style="justify-content:space-between;gap:8px">
                <b>{scene.id}</b>
                <span class="muted">{scene.supported_girls.length > 0 ? `only ${scene.supported_girls.join(', ')}` : `every girl${scene.body_types.length ? ' on ' + scene.body_types.join(', ') : ''}`}</span>
              </div>
              {scene.files.length === 0 && <span class="muted">No animation file in this pack.</span>}
              {scene.files.map(file => (
                <div class="scope-file" key={file.rel}>
                  <span class="row-name" title={file.rel}>{file.rel.slice(file.rel.lastIndexOf('/') + 1)}</span>
                  <span class="muted">{file.character_kind === 'girl' ? `${file.character} only` : file.character_kind === 'skeleton' ? `every ${file.character} girl` : 'everyone'}</span>
                  {file.character_kind === 'girl' && <button class="btn small" disabled={busy} onClick={() => void run(() => api.sceneMakeSkeletonWide(scene.id, file.character))}>Make skeleton-wide</button>}
                  {file.character_kind !== 'girl' && (
                    <>
                      <select class="input small-select" value={pick[file.rel] ?? ''} onChange={e => setPick(previous => ({ ...previous, [file.rel]: (e.target as HTMLSelectElement).value }))}>
                        <option value="">Only for…</option>
                        {girls.filter(g => file.character_kind === 'shared' || g.body_type === file.character).map(g => <option key={g.id} value={g.id}>{g.id}</option>)}
                      </select>
                      <button class="btn small" disabled={busy || !pick[file.rel]} onClick={() => void run(() => api.sceneMakeGirlSpecific(scene.id, pick[file.rel], keep))}>Make hers only</button>
                    </>
                  )}
                </div>
              ))}
            </div>
          ))}
        </div>
      </div>
      <div class="foot"><button class="btn primary" onClick={onClose}>Close</button></div>
    </Dialog>
  );
}
