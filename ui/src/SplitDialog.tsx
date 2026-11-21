import { useEffect, useState } from 'preact/hooks';
import { api } from './api';
import { Dialog } from './Dialogs';
import { messageOf, notify } from './notify';
import type { SplitReport } from './types';

/**
 * Splitting scenes: each girl's model carries the partner as a second rig that her scene animations also move. This takes that rig out into a model of its own, writes each scene as
 * a role folder (the girl's clips, the partner's clips) with a layout file, and removes the rig from the girl's models. It is done in place, only where every bone is proven to be
 * posed exactly as before, and the whole conversion can be undone.
 */
export function SplitDialog({ onClose }: { onClose: () => void }) {
  const [report, setReport] = useState<SplitReport | null>(null);
  const [done, setDone] = useState<SplitReport | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => { void api.splitScenes(false).then(setReport).catch(error => notify(messageOf(error))); }, []);

  const run = async () => {
    setBusy(true);

    try {
      setDone(await api.splitScenes(true));
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
      setReport(await api.splitScenes(false));
    } catch (error) {
      notify(messageOf(error));
    } finally {
      setBusy(false);
    }
  };

  const shown = done ?? report;

  return (
    <Dialog title="Split scenes" onClose={onClose}>
      {!shown && <p class="muted">Looking at the pack...</p>}
      {shown && (
        <div style="max-height:55vh;overflow:auto">
          <p style="margin:0 0 8px">
            {done ? 'Done: ' : 'This would convert '}
            <strong>{shown.scenes}</strong> scenes, write {shown.files_written} files, replace {shown.files_removed} old scene files and take the partner rig out of {shown.models_stripped} models.
          </p>
          {shown.dropped_bones.length > 0 && <details><summary>{shown.dropped_bones.length} old partner bones have no counterpart (their animation is dropped)</summary>{shown.dropped_bones.map(([bone, files]) => <p class="muted" key={bone} style="margin:2px 0">{bone}: animated in {files} files</p>)}</details>}
          {shown.approximate.length > 0 && <details><summary>{shown.approximate.length} partner bones are mapped as closely as the new model allows</summary>{shown.approximate.map(([bone, by]) => <p class="muted" key={bone} style="margin:2px 0">{bone}: up to {by.toFixed(2)} off</p>)}</details>}
          {shown.skipped.length > 0 && <details open><summary>{shown.skipped.length} files left as they are</summary>{shown.skipped.map(d => <p class="muted" key={d} style="margin:2px 0">{d}</p>)}</details>}
          {shown.findings.length > 0 && <details><summary>{shown.findings.length} notes</summary>{shown.findings.map(d => <p class="muted" key={d} style="margin:2px 0">{d}</p>)}</details>}
        </div>
      )}
      <div class="foot">
        <button class="btn ghost" onClick={onClose}>Close</button>
        {done?.stamp && <button class="btn" disabled={busy} onClick={() => void undo()}>Undo</button>}
        {!done && <button class="btn primary" disabled={busy || !report || report.scenes === 0} onClick={() => void run()}>Convert in place</button>}
      </div>
    </Dialog>
  );
}
