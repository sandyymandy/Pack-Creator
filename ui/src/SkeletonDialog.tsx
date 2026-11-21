import { useEffect, useState } from 'preact/hooks';
import { api } from './api';
import { Dialog } from './Dialogs';
import { messageOf, notify } from './notify';
import type { SkeletonPlan } from './types';

/**
 * Skeleton ids: the one string on a model, and on every animation file made for it, that says which rig it is (`pleasurehorizons:jenny`). The game compares these and
 * nothing else to decide who may play a clip or fill a scene role. The grouping is shown once: girls on one id share a rig; a girl whose model is built differently gets an id
 * of her own by typing one. Writing the ids keeps a copy of every file as it was and changes nothing else.
 */
export function SkeletonDialog({ onClose }: { onClose: () => void }) {
  const [choices, setChoices] = useState<Record<string, string>>({});
  const [plan, setPlan] = useState<SkeletonPlan | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    setPlan(null);
    void api.skeletonPlan(choices).then(setPlan).catch(error => notify(messageOf(error)));
  }, [choices]);

  const write = async () => {
    setBusy(true);

    try {
      const done = await api.skeletonApply(choices);

      notify(`Skeleton ids written to ${done.written} files (${done.already} already had theirs).${done.failed.length ? ` ${done.failed.length} failed: ${done.failed[0]}` : ''}`, done.failed.length === 0);
      setChoices({ ...choices });
    } catch (error) {
      notify(messageOf(error));
    } finally {
      setBusy(false);
    }
  };

  const models = plan?.assignments.filter(a => a.kind === 'model') ?? [];
  const pending = plan?.assignments.filter(a => a.current !== a.id).length ?? 0;

  return (
    <Dialog title="Skeleton ids" onClose={onClose}>
      <p class="muted" style="margin:0 0 8px">
        Each model and each animation file made for it carries the id of its rig. Girls on one id share a skeleton. Type another id next to a girl who has her own.
      </p>
      {!plan && <p class="muted">Looking at the pack...</p>}
      {plan && (
        <div style="max-height:50vh;overflow:auto">
          {plan.groups.map(group => (
            <div class="imp-card" key={group.id} style="margin-bottom:8px">
              <strong>{group.id}</strong> <span class="muted">{group.models.length} models, {group.animations} animation files</span>
              <div style="display:flex;flex-wrap:wrap;gap:6px;margin-top:6px">
                {models.filter(m => m.id === group.id).map(m => (
                  <label key={m.rel} style="display:flex;gap:4px;align-items:center">
                    {m.owner}
                    <input
                      class="input"
                      value={choices[m.owner] ?? ''}
                      placeholder={group.id}
                      style="width:190px"
                      onChange={event => {
                        const value = (event.currentTarget as HTMLInputElement).value.trim();

                        setChoices(value ? { ...choices, [m.owner]: value } : Object.fromEntries(Object.entries(choices).filter(([k]) => k !== m.owner)));
                      }}
                    />
                  </label>
                ))}
              </div>
            </div>
          ))}
          {plan.unreadable.length > 0 && <p class="muted">Left out, could not be read: {plan.unreadable.join('; ')}</p>}
        </div>
      )}
      <div class="foot">
        <button class="btn ghost" onClick={onClose}>Close</button>
        <button class="btn primary" disabled={busy || !plan || pending === 0} onClick={() => void write()}>
          {pending === 0 ? 'All ids are in place' : `Write ids to ${pending} files`}
        </button>
      </div>
    </Dialog>
  );
}
