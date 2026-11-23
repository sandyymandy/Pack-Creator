import type { ToolIconName } from './toolicons';

/** One tool in the dock: what it is called, what it does, and where it sits. */
export interface DockItem {
  id: string;
  icon: ToolIconName;
  label: string;
  desc: string;
  group: string;
  /** The tour looks for this on the row, so a step can point at it while the dock is open. */
  tour?: string;
  run: () => void;
}

/** What a person changed: the order of the tools and which of them also sit in the top bar. */
export interface DockPrefs {
  order: string[];
  pinned: string[];
}

export const GROUPS = ['Pack', 'Girls and scenes', 'Models and Blockbench', 'Help'];
export const DEFAULT_PINNED = ['play', 'export'];

const KEY = 'pc.dock';

export function loadPrefs(): DockPrefs {
  try {
    const stored = JSON.parse(localStorage.getItem(KEY) ?? 'null') as Partial<DockPrefs> | null;

    return {
      order: Array.isArray(stored?.order) ? stored.order.filter(x => typeof x === 'string') : [],
      pinned: Array.isArray(stored?.pinned) ? stored.pinned.filter(x => typeof x === 'string') : DEFAULT_PINNED
    };
  } catch {
    return { order: [], pinned: DEFAULT_PINNED };
  }
}

export function savePrefs(prefs: DockPrefs): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(prefs));
  } catch {
    /* no storage: the arrangement is not kept */
  }
}

/** The tools in the order the person chose; tools that are new (not in the saved order) come after, in their own order. */
export function arrange(items: DockItem[], prefs: DockPrefs): DockItem[] {
  const rank = (item: DockItem) => {
    const at = prefs.order.indexOf(item.id);

    return at < 0 ? prefs.order.length + items.indexOf(item) : at;
  };

  return [...items].sort((a, b) => rank(a) - rank(b));
}

/** Moves a tool one place up or down among the tools of its own group. */
export function move(items: DockItem[], prefs: DockPrefs, id: string, by: -1 | 1): DockPrefs {
  const all = arrange(items, prefs);
  const item = all.find(i => i.id === id);

  if (!item) return prefs;

  const same = all.filter(i => i.group === item.group);
  const other = same[same.indexOf(item) + by];

  if (!other) return prefs;

  const order = all.map(i => i.id);
  const a = order.indexOf(item.id);
  const b = order.indexOf(other.id);

  [order[a], order[b]] = [order[b], order[a]];

  return { ...prefs, order };
}
