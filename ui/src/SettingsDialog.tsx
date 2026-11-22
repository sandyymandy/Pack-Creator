import { useEffect, useState } from 'preact/hooks';
import { Dialog } from './Dialogs';
import { checkForUpdate } from './UpdateBanner';
import { messageOf, notify } from './notify';
import { api } from './api';
import { DEFAULTS, getSettings, keyName, setSettings, type Settings } from './settings';

/** The Pack Creator's settings: how files are saved, how the editor looks, and how the scene simulator is played. Every change applies at once. */
export function SettingsDialog({ onClose, onResetLayout }: { onClose: () => void; onResetLayout: () => void }) {
  const [settings, setLocal] = useState<Settings>(getSettings());
  const [capturing, setCapturing] = useState<'thrustKey' | 'cumKey' | null>(null);

  const change = (patch: Partial<Settings>) => {
    setSettings(patch);
    setLocal(getSettings());
  };

  /* The next key pressed becomes the thrust key. */
  useEffect(() => {
    if (!capturing) return;

    const take = (event: KeyboardEvent) => {
      event.preventDefault();
      event.stopPropagation();

      if (event.code !== 'Escape' && capturing) change({ [capturing]: event.code });

      setCapturing(null);
    };

    window.addEventListener('keydown', take, true);

    return () => window.removeEventListener('keydown', take, true);
  }, [capturing]);

  const number = (value: number, min: number, max: number, set: (n: number) => void) => (
    <input class="input" style="width:90px" type="number" min={min} max={max} value={value} onChange={e => { const n = Number((e.target as HTMLInputElement).value); if (Number.isFinite(n)) set(Math.max(min, Math.min(max, n))); }} />
  );

  return (
    <Dialog title="Settings" onClose={onClose}>
      <h4 class="set-head">Updates</h4>
      <div class="set-row">
        <label>The Pack Creator looks for a newer version on GitHub when it starts.</label>
        <button class="btn small" onClick={() => void checkForUpdate().then(found => {
          if (!found.newer) { notify(`You have the newest version (${found.current}).`, true); return; }
          notify(`${found.latest} is out (this is ${found.current}). Opening its page.`, true);
          void api.openReleasePage(found.url).catch(error => notify(messageOf(error)));
        }).catch(error => notify(messageOf(error)))}>Check now</button>
      </div>

      <h4 class="set-head">Window</h4>
      <div class="set-row"><label>Zoom the whole window ({Math.round(settings.uiZoom * 100)}%; also Ctrl + and Ctrl - , Ctrl 0 resets)</label><input type="range" min={0.7} max={2} step={0.05} value={settings.uiZoom} onInput={e => change({ uiZoom: Number((e.target as HTMLInputElement).value) })} /></div>

      <h4 class="set-head">Saving</h4>
      <div class="set-row">
        <label>Save a file this long after the last edit (milliseconds)</label>
        {number(settings.autosaveMs, 200, 10000, n => change({ autosaveMs: n }))}
      </div>

      <h4 class="set-head">The editor</h4>
      <div class="set-row"><label>Text size</label>{number(settings.fontSize, 9, 28, n => change({ fontSize: n }))}</div>
      <div class="set-row"><label>Show the minimap</label><input type="checkbox" checked={settings.minimap} onChange={e => change({ minimap: (e.target as HTMLInputElement).checked })} /></div>
      <div class="set-row"><label>Spaces in a tab</label>{number(settings.tabSize, 1, 8, n => change({ tabSize: Math.round(n) }))}</div>
      <div class="set-row"><label>Wrap long lines</label><input type="checkbox" checked={settings.wordWrap} onChange={e => change({ wordWrap: (e.target as HTMLInputElement).checked })} /></div>

      <h4 class="set-head">The scene simulator</h4>
      <div class="set-row">
        <label>Thrust key</label>
        <button class="btn small" onClick={() => setCapturing('thrustKey')}>{capturing === 'thrustKey' ? 'Press a key…' : keyName(settings.thrustKey)}</button>
      </div>
      <div class="set-row">
        <label>Finish key (when the bar is full)</label>
        <button class="btn small" onClick={() => setCapturing('cumKey')}>{capturing === 'cumKey' ? 'Press a key…' : keyName(settings.cumKey)}</button>
      </div>
      <div class="set-row"><label>Play the sounds of cues</label><input type="checkbox" checked={settings.simSound} onChange={e => change({ simSound: (e.target as HTMLInputElement).checked })} /></div>
      <div class="set-row"><label>Volume</label><input type="range" min={0} max={1} step={0.05} value={settings.simVolume} onInput={e => change({ simVolume: Number((e.target as HTMLInputElement).value) })} /></div>

      <h4 class="set-head">Layout</h4>
      <div class="set-row"><label>The sizes of the file tree and the problems list</label><button class="btn small" onClick={onResetLayout}>Reset</button></div>

      <div class="foot">
        <button class="btn ghost" onClick={() => change({ ...DEFAULTS })}>Reset all to the defaults</button>
        <button class="btn primary" onClick={onClose}>Done</button>
      </div>
    </Dialog>
  );
}
