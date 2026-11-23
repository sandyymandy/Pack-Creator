import { useEffect, useState } from 'preact/hooks';
import { arrange, DEFAULT_PINNED, GROUPS, move, savePrefs, type DockItem, type DockPrefs } from './dockState';
import { ToolIcon } from './toolicons';

/**
 * The dock: every tool of the pack with its name and what it does, in groups, in a panel that slides in from the right (the three dots in the top bar, or Ctrl+J). It can be
 * customised: tools move up and down inside their group, and any tool can be pinned to the top bar as a button too.
 */
export function Dock({ open, items, prefs, onPrefs, onClose }: { open: boolean; items: DockItem[]; prefs: DockPrefs; onPrefs: (next: DockPrefs) => void; onClose: () => void }) {
  const [customizing, setCustomizing] = useState(false);

  useEffect(() => {
    if (!open) return;

    const key = (event: KeyboardEvent) => { if (event.key === 'Escape') { event.stopPropagation(); onClose(); } };

    window.addEventListener('keydown', key, true);
    return () => window.removeEventListener('keydown', key, true);
  }, [open, onClose]);

  const change = (next: DockPrefs) => { onPrefs(next); savePrefs(next); };
  const ordered = arrange(items, prefs);
  const pin = (id: string) => change({ ...prefs, pinned: prefs.pinned.includes(id) ? prefs.pinned.filter(p => p !== id) : [...prefs.pinned, id] });

  return (
    <>
      {open && <div class="dock-backdrop" onClick={onClose} />}
      <aside class={'dock' + (open ? ' open' : '')} aria-hidden={!open} aria-label="Tools">
        <div class="dock-head">
          <strong>Tools</strong>
          <span class="spacer" />
          <button class={'btn ghost small' + (customizing ? ' on' : '')} title="Move tools around and pin them to the top bar" onClick={() => setCustomizing(c => !c)}>{customizing ? 'Done' : 'Customize'}</button>
          <button class="btn ghost small" title="Close (Esc)" onClick={onClose}>✕</button>
        </div>
        <div class="dock-body">
          {GROUPS.map(group => {
            const here = ordered.filter(item => item.group === group);

            return here.length === 0 ? null : (
              <section key={group} class="dock-group">
                <h3>{group}</h3>
                {here.map((item, at) => (
                  <div key={item.id} class="dock-row" data-tour={item.tour}>
                    <button class="dock-item" title={item.desc} onClick={() => { if (!customizing) { onClose(); item.run(); } }}>
                      <span class="dock-icon"><ToolIcon name={item.icon} /></span>
                      <span class="dock-text"><span class="dock-label">{item.label}</span><span class="dock-desc">{item.desc}</span></span>
                    </button>
                    {customizing && (
                      <span class="dock-edit">
                        <button class="btn ghost small" disabled={at === 0} title="Move up" onClick={() => change(move(items, prefs, item.id, -1))}>▲</button>
                        <button class="btn ghost small" disabled={at === here.length - 1} title="Move down" onClick={() => change(move(items, prefs, item.id, 1))}>▼</button>
                        <button class={'btn ghost small' + (prefs.pinned.includes(item.id) ? ' on' : '')} title="Show this tool as a button in the top bar" onClick={() => pin(item.id)}>{prefs.pinned.includes(item.id) ? 'Pinned' : 'Pin'}</button>
                      </span>
                    )}
                  </div>
                ))}
              </section>
            );
          })}
        </div>
        {customizing && (
          <div class="dock-foot">
            <button class="btn ghost small" onClick={() => change({ order: [], pinned: DEFAULT_PINNED })}>Reset to the default</button>
          </div>
        )}
      </aside>
    </>
  );
}
