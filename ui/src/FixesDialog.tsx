import { useEffect, useState } from 'preact/hooks';
import { api } from './api';
import { Dialog } from './Dialogs';
import { messageOf, notify } from './notify';
import type { FixItem, FixOutcome } from './types';

const key = (item: FixItem) => `${item.fix}:${item.girl}`;

/**
 * Fixes for the models of a pack: the armour cut out into models of its own, the arms built with a rotated bone, and the old male rig. An import applies what matters on its own;
 * this is for a pack that was not imported, or was imported before a fix existed. Everything can be applied at once or picked, and can be undone.
 */
export function FixesDialog({ onClose }: { onClose: () => void }) {
  const [items, setItems] = useState<FixItem[] | null>(null);
  const [on, setOn] = useState<Set<string>>(new Set());
  const [done, setDone] = useState<FixOutcome | null>(null);
  const [busy, setBusy] = useState(false);

  const look = async () => {
    try {
      const found = await api.modelFixesScan();

      setItems(found);
      setOn(new Set(found.map(key)));
    } catch (error) {
      notify(messageOf(error));
    }
  };

  useEffect(() => { void look(); }, []);

  const toggle = (item: FixItem) => setOn(previous => {
    const next = new Set(previous);

    if (!next.delete(key(item))) next.add(key(item));

    return next;
  });

  const run = async (everything: boolean) => {
    const chosen = (items ?? []).filter(item => everything || on.has(key(item))).map(item => ({ fix: item.fix, girl: item.girl }));

    if (chosen.length === 0) return;

    setBusy(true);

    try {
      setDone(await api.modelFixesApply(chosen));
      await look();
    } catch (error) {
      notify(messageOf(error));
    } finally {
      setBusy(false);
    }
  };

  const undo = async () => {
    if (!done?.stamp) return;

    setBusy(true);

    try {
      notify(`Put back ${await api.undoSplitScenes(done.stamp)} files.`, true);
      setDone(null);
      await look();
    } catch (error) {
      notify(messageOf(error));
    } finally {
      setBusy(false);
    }
  };

  const groups = [...new Set((items ?? []).map(item => item.fix))];
  const titles: Record<string, string> = { male: 'Old male rig', arms: 'Arms', armour: 'Armour' };

  return (
    <Dialog title="Fix the models" onClose={onClose}>
      {!items && <p class="muted">Looking at the models...</p>}
      {items && items.length === 0 && <p class="muted">Nothing to fix: no armour left in the girls' models, no rotated arm bones, no old male rig.</p>}
      {items && items.length > 0 && (
        <div style="max-height:50vh;overflow:auto">
          <p class="muted" style="margin:0 0 8px">Pick what to fix, or fix everything. Your files are kept, and Undo puts them back.</p>
          {groups.map(group => (
            <div key={group} style="margin-bottom:8px">
              <strong>{titles[group] ?? group}</strong>
              {items.filter(item => item.fix === group).map(item => (
                <label key={key(item)} style="display:flex;gap:8px;align-items:flex-start;margin:4px 0">
                  <input type="checkbox" checked={on.has(key(item))} onChange={() => toggle(item)} />
                  <span>{item.girl && <b>{item.girl}: </b>}<span class="muted">{item.detail}</span></span>
                </label>
              ))}
            </div>
          ))}
        </div>
      )}
      {done && (
        <div style="max-height:25vh;overflow:auto;margin-top:8px">
          <p style="margin:0 0 4px">Done: {done.written.length} files written.</p>
          {done.notes.map(note => <p class="muted" key={note} style="margin:2px 0">{note}</p>)}
        </div>
      )}
      <div class="foot">
        <button class="btn ghost" onClick={onClose}>Close</button>
        {done?.stamp && <button class="btn" disabled={busy} onClick={() => void undo()}>Undo</button>}
        <button class="btn" disabled={busy || !items || on.size === 0} onClick={() => void run(false)}>Apply picked</button>
        <button class="btn primary" disabled={busy || !items || items.length === 0} onClick={() => void run(true)}>Apply all</button>
      </div>
    </Dialog>
  );
}
