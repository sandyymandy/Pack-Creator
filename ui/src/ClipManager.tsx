import { useState } from 'preact/hooks';
import { api } from './api';
import { Dialog } from './Dialogs';
import { messageOf, notify } from './notify';
import type { ClipData } from './types';

/**
 * The name the game looks for: `animation.<scene>.<part>`, where the part is intro, slow, fast, cum... (a clip called `sagittarius.start` is not found; `animation.sagittarius.intro`
 * is). Shown as a suggestion, never applied by itself.
 */
export function suggest(name: string): string {
  const full = name.startsWith('animation.') ? name : `animation.${name}`;
  const parts = full.split('.');

  if (parts.length >= 3 && /^start(_\d+)?$/.test(parts[parts.length - 1])) parts[parts.length - 1] = parts[parts.length - 1].replace('start', 'intro');

  return parts.join('.');
}

/**
 * Lists the clips of an animation file so they can be renamed or the unwanted ones removed. Renaming follows into the scene layouts that list the clip. A copy of the file as it was
 * is kept in the project's backups.
 */
export function ClipManager({ rel, clips, onClose, onDeleted }: { rel: string; clips: ClipData[]; onClose: () => void; onDeleted: () => void }) {
  const [marked, setMarked] = useState<Set<string>>(new Set());
  const [names, setNames] = useState<Record<string, string>>({});
  const [filter, setFilter] = useState('');
  const [busy, setBusy] = useState(false);

  const shown = clips.filter(c => c.name.toLowerCase().includes(filter.trim().toLowerCase()));
  const toggle = (name: string) => setMarked(previous => { const next = new Set(previous); if (next.has(name)) next.delete(name); else next.add(name); return next; });
  const all = marked.size === clips.length;
  const nameOf = (c: ClipData) => names[c.name] ?? c.name;
  const changes = clips.filter(c => nameOf(c).trim() !== '' && nameOf(c).trim() !== c.name);
  const after = clips.map(c => nameOf(c).trim());
  const clash = (c: ClipData) => after.filter(n => n === nameOf(c).trim()).length > 1;
  const clashing = clips.some(clash);

  const remove = async () => {
    setBusy(true);

    try {
      await api.deleteClips(rel, [...marked]);
      notify(`Removed ${marked.size} animation${marked.size === 1 ? '' : 's'}. A copy of the file as it was is in the project's .packcreator/backups folder.`, true);
      onDeleted();
    } catch (error) {
      notify(messageOf(error));
      setBusy(false);
    }
  };

  const rename = async () => {
    setBusy(true);

    try {
      await api.renameClips(rel, changes.map(c => [c.name, nameOf(c).trim()] as [string, string]));
      notify(`Renamed ${changes.length} animation${changes.length === 1 ? '' : 's'}. A copy of the file as it was is in the project's .packcreator/backups folder.`, true);
      onDeleted();
    } catch (error) {
      notify(messageOf(error));
      setBusy(false);
    }
  };

  const fixNames = () => setNames(Object.fromEntries(clips.map(c => [c.name, suggest(c.name)]).filter(([from, to]) => from !== to)));

  return (
    <Dialog title="Animations in this file" onClose={onClose}>
      <p class="muted" style="margin-top:0">{rel.slice(rel.lastIndexOf('/') + 1)}: {clips.length} animations. Edit a name to rename it, or tick the ones to remove. The game looks for <code>animation.&lt;scene&gt;.&lt;part&gt;</code> (the parts are intro, slow, fast, cum...).</p>
      <div class="row" style="gap:6px;margin-bottom:6px">
        <input class="input grow" value={filter} placeholder="Search…" onInput={e => setFilter((e.target as HTMLInputElement).value)} />
        <button class="btn ghost small" title="Writes the names the game looks for: adds animation. in front and calls start intro" onClick={fixNames}>Suggest names</button>
        <button class="btn ghost small" onClick={() => setMarked(new Set(shown.map(c => c.name)))}>Tick {filter.trim() ? 'these' : 'all'}</button>
        <button class="btn ghost small" onClick={() => setMarked(new Set())}>Tick none</button>
      </div>
      <div class="clip-manager">
        {shown.map(c => (
          <div class={'clip-manager-row' + (marked.has(c.name) ? ' marked' : '')} key={c.name}>
            <input type="checkbox" title="Remove this animation" checked={marked.has(c.name)} onChange={() => toggle(c.name)} />
            <input class="input row-name" style={clash(c) ? 'border-color:var(--error)' : ''} value={nameOf(c)} onInput={e => setNames({ ...names, [c.name]: (e.target as HTMLInputElement).value })} />
            <span class="muted">{c.duration.toFixed(2)} s{c.events.length > 0 ? `, ${c.events.length} cue${c.events.length === 1 ? '' : 's'}` : ''}</span>
          </div>
        ))}
        {shown.length === 0 && <span class="muted">Nothing matches.</span>}
      </div>
      {all && <div class="hint" style="color:var(--warning)">A file keeps at least one animation. To remove them all, delete the file instead.</div>}
      {clashing && <div class="hint" style="color:var(--error)">Two animations have the same name.</div>}
      <div class="foot">
        <button class="btn ghost" onClick={onClose}>Close</button>
        <button class="btn" disabled={changes.length === 0 || clashing || busy} onClick={() => void rename()}>{busy ? 'Working…' : `Rename ${changes.length}`}</button>
        <button class="btn primary" disabled={marked.size === 0 || all || busy} onClick={() => void remove()}>{`Remove ${marked.size} animation${marked.size === 1 ? '' : 's'}`}</button>
      </div>
    </Dialog>
  );
}
