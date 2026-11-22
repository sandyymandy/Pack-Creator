import { useEffect, useState } from 'preact/hooks';
import { api } from './api';
import { messageOf, notify } from './notify';
import type { AppUpdate } from './types';

const CHECKED = 'pc_update_checked';
const SKIPPED = 'pc_update_skipped';
const EVERY_MS = 12 * 60 * 60 * 1000;

function remembered(key: string): string {
  try { return localStorage.getItem(key) ?? ''; } catch { return ''; }
}

function remember(key: string, value: string): void {
  try { localStorage.setItem(key, value); } catch { /* the check is made again next time */ }
}

/** Looks on GitHub for a newer Pack Creator now, and returns what it found (the button in the settings). */
export async function checkForUpdate(): Promise<AppUpdate> {
  const update = await api.checkAppUpdate();

  remember(CHECKED, String(Date.now()));
  return update;
}

/**
 * Tells the person when a newer Pack Creator is out: a small note at the bottom right with what is new, a button that downloads the release's file into the Downloads folder, and
 * one that skips that version. The check is made when the program starts, and then at most twice a day; with no network it says nothing.
 */
export function UpdateBanner() {
  const [update, setUpdate] = useState<AppUpdate | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (Date.now() - Number(remembered(CHECKED) || 0) < EVERY_MS) return;

    void checkForUpdate().then(found => { if (found.newer && remembered(SKIPPED) !== found.latest) setUpdate(found); }).catch(() => undefined);
  }, []);

  if (!update) return null;

  const download = async () => {
    setBusy(true);

    try {
      if (update.asset && update.asset_name) {
        notify(`Downloaded to ${await api.downloadAppUpdate(update.asset, update.asset_name)}. Close the Pack Creator, then run the new one.`, true);
      } else {
        await api.openReleasePage(update.url);
      }

      setUpdate(null);
    } catch (error) {
      notify(messageOf(error));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div class="update-banner" role="status">
      <div><b>Pack Creator {update.latest} is out</b> <span class="muted">(this is {update.current})</span></div>
      {update.notes && <div class="update-notes muted">{update.notes}</div>}
      <div class="update-actions">
        <button class="btn primary small" disabled={busy} onClick={() => void download()}>{update.asset ? 'Download' : 'Open the page'}</button>
        <button class="btn ghost small" onClick={() => void api.openReleasePage(update.url).catch(error => notify(messageOf(error)))}>What is new</button>
        <button class="btn ghost small" onClick={() => { remember(SKIPPED, update.latest); setUpdate(null); }}>Skip this version</button>
        <button class="btn ghost small" onClick={() => setUpdate(null)}>Later</button>
      </div>
    </div>
  );
}
