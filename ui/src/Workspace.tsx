import { useEffect, useMemo, useRef, useState } from 'preact/hooks';
import { api, pickFolder, pickSavePath } from './api';
import { BlockbenchDialog } from './BlockbenchDialog';
import { SkeletonDialog } from './SkeletonDialog';
import { SplitDialog } from './SplitDialog';
import { FixesDialog } from './FixesDialog';
import { Dock } from './Dock';
import { arrange, loadPrefs, type DockItem, type DockPrefs } from './dockState';
import { DocsDialog } from './DocsDialog';
import { GameLogDialog } from './GameLogDialog';
import { HistoryDialog } from './HistoryDialog';
import { ImportDialog } from './ImportDialog';
import { ScopeDialog } from './ScopeDialog';
import { Confirm, ImportReport, NewFileDialog, Prompt, QuickOpen } from './Dialogs';
import * as ide from './editor';
import { FileIcon } from './icons';
import { GirlForm } from './GirlForm';
import { DialogueForm } from './DialogueForm';
import { SocialForm } from './SocialForm';
import { WearerForm } from './WearerForm';
import { ImageView } from './ImageView';
import { TraitForm } from './TraitForm';
import { SceneForm } from './SceneForm';
import { Viewer } from './viewer/Viewer';
import { messageOf, notify } from './notify';
import { Problems } from './Problems';
import { SimView } from './sim/SimView';
import { IconButton, ToolIcon } from './toolicons';
import { getSettings, onSettings } from './settings';
import { SettingsDialog } from './SettingsDialog';
import { SearchDialog } from './SearchDialog';
import { Splitter } from './Splitter';
import { Tour } from './Tour';
import { ALL_TOURS, markTours, tourSeen } from './tourState';
import { workspaceSteps } from './tourSteps';
import { collect, hasFiles } from './dropped';
import { Tree } from './Tree';
import type { Diagnostic, Entry, FileKind, ImportResult, Opened, Reference } from './types';

interface Tab {
  rel: string;
  kind: FileKind;
  view: 'text' | 'image' | 'binary' | 'big' | 'viewer' | 'sim';
  dirty: boolean;
  image?: string;
  size?: number;
}

type Dialog =
  | { type: 'new'; kind: FileKind }
  | { type: 'prompt-file'; parent: string }
  | { type: 'prompt-folder'; parent: string }
  | { type: 'rename'; entry: Entry }
  | { type: 'delete'; entry: Entry }
  | { type: 'quick' }
  | { type: 'import' }
  | { type: 'scope'; focus?: string }
  | { type: 'blockbench' }
  | { type: 'skeletons' }
  | { type: 'split' }
  | { type: 'fixes' }
  | { type: 'gamelog' }
  | { type: 'docs'; start?: string }
  | { type: 'settings' }
  | { type: 'search' }
  | { type: 'history'; rel: string }
  | { type: 'report'; result: ImportResult }
  | null;

const RECHECK_MS = 1200;

/** How long after the last keystroke a file is checked as it is typed. */
const LIVE_CHECK_MS = 450;

const nameOf = (rel: string) => rel.slice(rel.lastIndexOf('/') + 1);
const parentOf = (rel: string) => (rel.includes('/') ? rel.slice(0, rel.lastIndexOf('/')) : '');

function readSide(): 'left' | 'right' {
  try {
    return localStorage.getItem('tree-side') === 'left' ? 'left' : 'right';
  } catch {
    return 'right';
  }
}

const DEFAULT_SIZES = { side: 290, problems: 210 };
const clampSide = (width: number) => Math.max(200, Math.min(700, Math.round(width)));
const clampProblems = (height: number) => Math.max(70, Math.min(Math.round(window.innerHeight * 0.7), Math.round(height)));

/** The sizes of the file tree and the problems panel, as the person last dragged them. */
function readSizes(): { side: number; problems: number } {
  try {
    const stored = JSON.parse(localStorage.getItem('ph.layout') ?? '{}') as { side?: number; problems?: number };

    return { side: clampSide(stored.side ?? DEFAULT_SIZES.side), problems: clampProblems(stored.problems ?? DEFAULT_SIZES.problems) };
  } catch {
    return { ...DEFAULT_SIZES };
  }
}

function saveSizes(sizes: { side: number; problems: number }): void {
  try {
    localStorage.setItem('ph.layout', JSON.stringify(sizes));
  } catch {
    /* Only not remembered. */
  }
}

function readView(): 'form' | 'json' {
  try {
    return localStorage.getItem('girl-view') === 'json' ? 'json' : 'form';
  } catch {
    return 'form';
  }
}

export function Workspace({ opened, onHome, startImport }: { opened: Opened; onHome: () => void; startImport?: boolean }) {
  const [tabs, setTabs] = useState<Tab[]>([]);
  const [active, setActive] = useState<string | null>(null);
  const tabStrip = useRef<HTMLDivElement>(null);

  /* With more tabs than fit, the one in use is brought into view (the strip only, not the page around it), and the wheel moves along the strip. */
  useEffect(() => {
    const strip = tabStrip.current;
    const tab = strip?.querySelector('.tab.active') as HTMLElement | null;

    if (!strip || !tab) return;

    if (tab.offsetLeft < strip.scrollLeft) strip.scrollLeft = tab.offsetLeft;
    else if (tab.offsetLeft + tab.offsetWidth > strip.scrollLeft + strip.clientWidth) strip.scrollLeft = tab.offsetLeft + tab.offsetWidth - strip.clientWidth;
  }, [active, tabs.length]);
  const [problems, setProblems] = useState<Record<string, Diagnostic[]>>({});
  const [checking, setChecking] = useState(false);
  const [saveState, setSaveState] = useState<'saved' | 'saving' | 'idle'>('idle');
  const [version, setVersion] = useState(0);
  const [side, setSide] = useState<'left' | 'right'>(readSide);
  const [collapsed, setCollapsed] = useState(false);
  const [selected, setSelected] = useState<Entry | null>(null);
  const [collapseKey, setCollapseKey] = useState(0);
  const [tabMenu, setTabMenu] = useState<{ x: number; y: number; rel: string } | null>(null);
  const [sizes, setSizes] = useState(readSizes);
  const startSizes = useRef(sizes);
  const sizesRef = useRef(sizes);

  sizesRef.current = sizes;
  const [dialog, setDialog] = useState<Dialog>(startImport ? { type: 'import' } : null);

  /* A pack in an older format is offered its update as soon as it opens. */
  useEffect(() => { void api.migrationStatus().then(status => { if (status.needed) setDialog({ type: 'split' }); }).catch(() => undefined); }, []);
  const [menu, setMenu] = useState<{ x: number; y: number; entry: Entry | null } | null>(null);
  const [cursor, setCursor] = useState({ line: 1, column: 1 });
  const [view, setView] = useState<'form' | 'json'>(readView);
  const [reference, setReference] = useState<Reference | null>(null);
  const [packTraits, setPackTraits] = useState<string[]>([]);
  const [liveText, setLiveText] = useState('');

  const host = useRef<HTMLDivElement>(null);
  const editor = useRef<ReturnType<typeof ide.create> | null>(null);
  const tabsRef = useRef<Tab[]>([]);
  const problemsRef = useRef<Record<string, Diagnostic[]>>({});
  const saveTimers = useRef(new Map<string, number>());
  const listeners = useRef(new Map<string, { dispose(): void }>());
  const recheckTimer = useRef<number>();
  const pending = useRef<{ line: number; column: number } | null>(null);
  const stamps = useRef(new Map<string, string | null>());
  const knownFiles = useRef('');

  tabsRef.current = tabs;
  problemsRef.current = problems;

  /* ---------- Problems ---------- */

  const recheck = async () => {
    setChecking(true);

    try {
      const all = await api.checkProject();
      const grouped: Record<string, Diagnostic[]> = {};

      for (const problem of all) (grouped[problem.file] ??= []).push(problem);

      setProblems(grouped);
    } catch (error) {
      notify(messageOf(error));
    } finally {
      setChecking(false);
    }
  };

  const scheduleRecheck = () => {
    window.clearTimeout(recheckTimer.current);
    recheckTimer.current = window.setTimeout(() => void recheck(), RECHECK_MS);
  };

  /* The squiggles follow the problems of each open file. */
  useEffect(() => {
    for (const tab of tabs) if (tab.view === 'text') ide.showProblems(tab.rel, problems[tab.rel] ?? []);
  }, [problems, tabs.length]);

  /* ---------- Saving ---------- */

  const save = async (rel: string) => {
    window.clearTimeout(saveTimers.current.get(rel));
    saveTimers.current.delete(rel);

    const model = ide.monaco.editor.getModel(ide.uriOf(rel));

    if (!model) return;

    const text = model.getValue();

    setSaveState('saving');

    try {
      const written = await api.writeFile(rel, text);

      stamps.current.set(rel, written.stamp);
      setProblems(previous => ({ ...previous, [rel]: written.diagnostics }));
      setTabs(previous => previous.map(t => (t.rel === rel && model.getValue() === text ? { ...t, dirty: false } : t)));
      setSaveState('saved');
      scheduleRecheck();
    } catch (error) {
      setSaveState('idle');
      notify('Could not save ' + rel + ': ' + messageOf(error));
    }
  };

  /*
   * The problems of a file as it is typed, not only once it is saved: a short pause after the last keystroke, the text checked as it is, and the squiggles moved. The answer is
   * dropped if the text has moved on while it was coming back, so a slow answer never paints over a newer one.
   */
  const liveTimers = useRef(new Map<string, number>());

  const scheduleLiveCheck = (rel: string) => {
    window.clearTimeout(liveTimers.current.get(rel));
    liveTimers.current.set(rel, window.setTimeout(() => {
      const model = ide.monaco.editor.getModel(ide.uriOf(rel));

      if (!model) return;

      const text = model.getValue();

      void api.checkText(rel, text).then(found => {
        if (model.getValue() === text) setProblems(previous => ({ ...previous, [rel]: found }));
      }).catch(() => undefined);
    }, LIVE_CHECK_MS));
  };

  const scheduleSave = (rel: string) => {
    window.clearTimeout(saveTimers.current.get(rel));
    saveTimers.current.set(rel, window.setTimeout(() => void save(rel), getSettings().autosaveMs));
  };

  const saveAll = async () => {
    await Promise.all([...saveTimers.current.keys()].map(rel => save(rel)));
  };

  /* ---------- Opening and closing ---------- */

  /* The scene simulator is a tab of its own for a scene file, named `simulate:<the scene file>`. */
  const located = (file: string): FileKind => (/\/girls\/[^/]+\.json$/.test(file) ? 'girl' : 'scene');

  /* Her model in the viewer, from her girl file: the nude one, or the dressed one if that is all she has. */
  const previewGirl = async (girlRel: string) => {
    const id = girlRel.split('/').pop()?.replace(/\.json$/, '') ?? '';

    try {
      const found = await api.viewerFiles(`data/x/asset_sync/strut/models/entities/girls/nude/${id}.strutmodel`);
      const model = found.models.find(m => m.includes('/nude/')) ?? found.models[0];

      if (model) void openFile(model);
      else notify(`${id} has no model in this pack yet.`);
    } catch (error) {
      notify(messageOf(error));
    }
  };

  const openSim = (sceneRel: string) => {
    const rel = 'simulate:' + sceneRel;

    if (!tabsRef.current.some(t => t.rel === rel)) setTabs(previous => [...previous, { rel, kind: located(sceneRel), view: 'sim', dirty: false }]);

    setActive(rel);
  };

  const openFile = async (rel: string, at?: { line: number; column: number }) => {
    pending.current = at ?? null;

    if (!tabsRef.current.some(t => t.rel === rel)) {
      try {
        const located = await api.locateFile(rel);
        let tab: Tab;

        /* A model or an animation is shown in the viewer: it is read as a model, not as text. */
        const content = located.kind === 'model' || located.kind === 'animation' ? ({ type: 'viewer' } as const) : await api.readFile(rel);

        if (content.type === 'viewer') {
          tab = { rel, kind: located.kind, view: 'viewer', dirty: false };
        } else if (content.type === 'text') {
          const model = ide.modelFor(rel, content.text);

          listeners.current.get(rel)?.dispose();
          listeners.current.set(rel, model.onDidChangeContent(() => {
            setTabs(previous => previous.map(t => (t.rel === rel && !t.dirty ? { ...t, dirty: true } : t)));
            setSaveState('idle');
            scheduleSave(rel);
            scheduleLiveCheck(rel);
          }));
          void api.schemaFor(rel).then(schema => ide.setSchema(rel, schema)).catch(() => undefined);
          tab = { rel, kind: located.kind, view: 'text', dirty: false };
        } else if (content.type === 'binary' && rel.toLowerCase().endsWith('.png')) {
          tab = { rel, kind: located.kind, view: 'image', dirty: false, image: await api.readImage(rel), size: content.size };
        } else {
          tab = { rel, kind: located.kind, view: content.type === 'too_large' ? 'big' : 'binary', dirty: false, size: content.size };
        }

        stamps.current.set(rel, (await api.stamps([rel]))[0] ?? null);
        setTabs(previous => [...previous, tab]);
      } catch (error) {
        notify(messageOf(error));
        return;
      }
    }

    setActive(rel);
  };

  const closeTab = async (rel: string, discard = false) => {
    if (!discard && saveTimers.current.has(rel)) await save(rel);

    window.clearTimeout(saveTimers.current.get(rel));
    saveTimers.current.delete(rel);
    listeners.current.get(rel)?.dispose();
    listeners.current.delete(rel);
    ide.dropModel(rel);
    ide.forgetSchema(rel);

    const index = tabsRef.current.findIndex(t => t.rel === rel);
    const remaining = tabsRef.current.filter(t => t.rel !== rel);

    setTabs(remaining);
    setActive(current => (current === rel ? remaining[Math.min(index, remaining.length - 1)]?.rel ?? null : current));
  };

  /** Closes every tab at or under `rel` (a file that was renamed or deleted). */
  const closeUnder = async (rel: string, discard: boolean) => {
    for (const tab of tabsRef.current) if (tab.rel === rel || tab.rel.startsWith(rel + '/')) await closeTab(tab.rel, discard);
  };

  /* ---------- The editor ---------- */

  useEffect(() => {
    const instance = ide.create(host.current!);

    editor.current = instance;

    /* The editor follows the settings, now and when they change. */
    const applySettings = () => {
      const s = getSettings();

      instance.updateOptions({ fontSize: s.fontSize, minimap: { enabled: s.minimap }, wordWrap: s.wordWrap ? 'on' : 'off', tabSize: s.tabSize });
    };

    applySettings();

    const stopListening = onSettings(applySettings);

    instance.onDidChangeCursorPosition(event => setCursor({ line: event.position.lineNumber, column: event.position.column }));
    void recheck();

    return () => {
      stopListening();
      for (const timer of saveTimers.current.values()) window.clearTimeout(timer);
      instance.dispose();
      editor.current = null;
    };
  }, []);

  useEffect(() => {
    const instance = editor.current;
    const tab = tabs.find(t => t.rel === active);

    if (!instance) return;

    if (tab?.view === 'text') {
      const model = ide.monaco.editor.getModel(ide.uriOf(tab.rel));

      if (model) instance.setModel(model);

      if (pending.current) {
        const position = { lineNumber: pending.current.line, column: pending.current.column };

        instance.revealPositionInCenter(position);
        instance.setPosition(position);
        pending.current = null;
      }

      instance.focus();
    } else {
      instance.setModel(null);
    }
  }, [active, tabs.length]);

  /* ---------- The guided form ---------- */

  useEffect(() => {
    void api.reference().then(setReference).catch(error => notify(messageOf(error)));
  }, []);

  useEffect(() => {
    void api.listFiles().then(files => setPackTraits(files.map(f => /\/girl_traits\/([^/]+)\.json$/.exec(f)?.[1]).filter((n): n is string => !!n)));
  }, [version]);

  const activeTab = tabs.find(t => t.rel === active);
  const FORM_KINDS: FileKind[] = ['girl', 'scene', 'trait', 'dialogue', 'social', 'armour_wearer'];
  const formShown = activeTab?.view === 'text' && FORM_KINDS.includes(activeTab.kind) && view === 'form';

  /* The form shows the file's text as it is now, and follows every change to it. */
  useEffect(() => {
    if (!active || activeTab?.view !== 'text') return;

    const model = ide.monaco.editor.getModel(ide.uriOf(active));

    if (!model) return;

    setLiveText(model.getValue());

    const watch = model.onDidChangeContent(() => setLiveText(model.getValue()));

    return () => watch.dispose();
  }, [active, tabs.length]);

  /** The form wrote the file: the whole text is replaced as one edit, so undo still works. */
  const writeFromForm = (text: string) => {
    const model = active ? ide.monaco.editor.getModel(ide.uriOf(active)) : null;

    model?.pushEditOperations([], [{ range: model.getFullModelRange(), text }], () => null);
  };

  const chooseView = (next: 'form' | 'json') => {
    setView(next);

    try {
      localStorage.setItem('girl-view', next);
    } catch {
      /* Only not remembered. */
    }
  };

  /* ---------- Changes made by other programs ---------- */

  /* Every two seconds the open files and the list of files are looked at. A file that changed on disk is read again if there is nothing unsaved in it; if there is, the
     person is told, and the next save wins. A new, renamed or deleted file shows in the tree. */
  useEffect(() => {
    let running = false;

    const look = async () => {
      if (running) return;

      running = true;

      try {
        const open = tabsRef.current.filter(t => t.view === 'text');
        const now = await api.stamps(open.map(t => t.rel));

        for (let i = 0; i < open.length; i++) {
          const rel = open[i].rel;
          const before = stamps.current.get(rel);

          if (now[i] === before) continue;

          stamps.current.set(rel, now[i]);

          if (now[i] === null) {
            notify(`${rel} was deleted outside the Pack Creator. It stays open here; saving it makes it again.`);
            continue;
          }

          if (saveTimers.current.has(rel) || open[i].dirty) {
            notify(`${rel} was changed outside the Pack Creator while you have unsaved edits. Saving will replace the other change.`);
            continue;
          }

          const content = await api.readFile(rel);
          const model = ide.monaco.editor.getModel(ide.uriOf(rel));

          if (content.type === 'text' && model && model.getValue() !== content.text) {
            model.pushEditOperations([], [{ range: model.getFullModelRange(), text: content.text }], () => null);
            window.clearTimeout(saveTimers.current.get(rel));
            saveTimers.current.delete(rel);
            setTabs(previous => previous.map(t => (t.rel === rel ? { ...t, dirty: false } : t)));
            scheduleRecheck();
          }
        }

        const listing = (await api.listFiles()).join('\n');

        if (knownFiles.current !== '' && listing !== knownFiles.current) changed();

        knownFiles.current = listing;
      } catch {
        /* A failed look is tried again at the next one. */
      } finally {
        running = false;
      }
    };

    const timer = window.setInterval(() => void look(), 2000);

    return () => window.clearInterval(timer);
  }, []);

  /* ---------- Keys ---------- */

  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      /* F2 renames and Delete deletes what is selected in the tree, unless the person is typing somewhere. */
      const typing = (event.target as Element | null)?.closest?.('input, textarea, select, .monaco-editor') !== null && (event.target as Element | null)?.closest?.('input, textarea, select, .monaco-editor') !== undefined;

      if (!typing && !(event.ctrlKey || event.metaKey) && selected) {
        if (event.key === 'F2') { event.preventDefault(); setDialog({ type: 'rename', entry: selected }); return; }
        if (event.key === 'Delete') { event.preventDefault(); setDialog({ type: 'delete', entry: selected }); return; }
      }

      if (event.key === 'F1') { event.preventDefault(); setDialog({ type: 'docs' }); return; }

      if (!(event.ctrlKey || event.metaKey)) return;

      const key = event.key.toLowerCase();

      /* Ctrl+C and Ctrl+X take the selected file or folder, and Ctrl+V puts it in the selected folder; a paste of files from Explorer is handled by the paste event. Left alone
         when the person has text selected, which is theirs to copy. */
      if (!typing && selected && (key === 'c' || key === 'x') && !event.shiftKey && !window.getSelection()?.toString()) {
        event.preventDefault();
        treeClipboard.current = { entry: selected, cut: key === 'x' };

        /* On the Windows clipboard as well, so it can be pasted into Explorer or another program (a cut there moves the file out of the pack). */
        void api.copyToClipboard([selected.rel], key === 'x').catch(error => notify(messageOf(error)));
        notify(`${selected.name} ${key === 'x' ? 'cut' : 'copied'}. Select a folder and press Ctrl+V, or paste it into Explorer.`, true);
        return;
      }

      /* Ctrl+Z undoes the last move or rename of a file or folder (and Ctrl+Shift+Z or Ctrl+Y does it again), unless the person is typing: there Ctrl+Z is the editor's own. */
      if (!typing && (key === 'z' || key === 'y')) {
        event.preventDefault();
        void (key === 'y' || event.shiftKey ? redoFileMove() : undoFileMove());
        return;
      }

      /* Ctrl+Shift+F searches every file; Ctrl+Tab goes to the next tab and Ctrl+Shift+Tab to the one before. */
      if (key === 'j') { event.preventDefault(); setDockOpen(open => !open); return; }
      if (key === 'f' && event.shiftKey) { event.preventDefault(); setDialog({ type: 'search' }); return; }

      if (event.key === 'Tab' && tabsRef.current.length > 1) {
        event.preventDefault();

        const at = tabsRef.current.findIndex(t => t.rel === active);
        const next = (at + (event.shiftKey ? -1 : 1) + tabsRef.current.length) % tabsRef.current.length;

        setActive(tabsRef.current[next].rel);
        return;
      }

      if (key === 'p') { event.preventDefault(); setDialog({ type: 'quick' }); }
      else if (key === 's') { event.preventDefault(); void saveAll(); }
      else if (key === 'w' && active) { event.preventDefault(); void closeTab(active); }
      else if (key === 'n') { event.preventDefault(); setDialog({ type: 'new', kind: 'girl' }); }
    };

    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [active, selected]);

  /* Ctrl+V: files copied in Explorer (or another program) are copied into the pack, and otherwise what Ctrl+C or Ctrl+X took in the tree is pasted. Not while typing, where a
     paste is text. */
  useEffect(() => {
    const onPaste = (event: ClipboardEvent) => {
      if ((event.target as Element | null)?.closest?.('input, textarea, select, .monaco-editor')) return;

      /* What Ctrl+C put on the Windows clipboard comes back as a file too: when it is the one thing the tree took, it is pasted as that (a copy or a move inside the pack) and not
         imported again as if it came from outside. */
      const taken = treeClipboard.current;
      const ownFile = !!taken && event.clipboardData !== null && event.clipboardData.files.length === 1 && event.clipboardData.files[0].name === taken.entry.name;

      if (event.clipboardData && hasFiles(event.clipboardData) && !ownFile) {
        event.preventDefault();
        void importFromOutside(event.clipboardData, targetFolder());
      } else if (treeClipboard.current) {
        event.preventDefault();
        void pasteInTree();
      }
    };

    window.addEventListener('paste', onPaste);
    return () => window.removeEventListener('paste', onPaste);
  }, [selected]);

  /* A right click menu goes away when anything outside it is pressed (in the capture phase, so a part of the page that swallows its clicks, like the editor, cannot keep it up), on Escape, and when the window loses focus. */
  useEffect(() => {
    const closeAll = () => { setMenu(null); setTabMenu(null); };
    const outside = (event: Event) => { if (!(event.target instanceof Element) || !event.target.closest('.menu')) closeAll(); };
    const escape = (event: KeyboardEvent) => { if (event.key === 'Escape') closeAll(); };

    window.addEventListener('click', closeAll);
    window.addEventListener('mousedown', outside, true);
    window.addEventListener('contextmenu', outside, true);
    window.addEventListener('keydown', escape);
    window.addEventListener('blur', closeAll);

    return () => {
      window.removeEventListener('click', closeAll);
      window.removeEventListener('mousedown', outside, true);
      window.removeEventListener('contextmenu', outside, true);
      window.removeEventListener('keydown', escape);
      window.removeEventListener('blur', closeAll);
    };
  }, []);

  /* ---------- Tree actions ---------- */

  const changed = () => {
    setVersion(v => v + 1);
    scheduleRecheck();
  };

  const parentFolder = (entry: Entry | null) => (entry ? (entry.is_dir ? entry.rel : parentOf(entry.rel)) : '');

  /* The moves and renames made in the tree, so Ctrl+Z can take them back (and Ctrl+Y put them back). The editor's own undo only knows about text. */
  const fileMoves = useRef<{ done: { from: string; to: string }[]; undone: { from: string; to: string }[] }>({ done: [], undone: [] });

  const rememberMove = (from: string, to: string) => {
    fileMoves.current.done.push({ from, to });
    fileMoves.current.undone = [];
  };

  const undoFileMove = async () => {
    const move = fileMoves.current.done.pop();

    if (!move) { notify('Nothing to undo: only moves and renames made in the file tree can be undone here.', true); return; }

    try {
      await closeUnder(move.to, false);
      await api.renameEntry(move.to, move.from);
      fileMoves.current.undone.push(move);
      setSelected(null);
      changed();
      notify(`Undone: ${nameOf(move.to)} is back at ${move.from}. (Ctrl+Y puts it back.)`, true);
    } catch (error) {
      fileMoves.current.done.push(move);
      notify(messageOf(error));
    }
  };

  const redoFileMove = async () => {
    const move = fileMoves.current.undone.pop();

    if (!move) { notify('Nothing to redo.', true); return; }

    try {
      await closeUnder(move.from, false);
      await api.renameEntry(move.from, move.to);
      fileMoves.current.done.push(move);
      setSelected(null);
      changed();
    } catch (error) {
      fileMoves.current.undone.push(move);
      notify(messageOf(error));
    }
  };

  /* ---------- Files from outside, and copy / cut / paste ---------- */

  /** What Ctrl+C or Ctrl+X took in the tree: pasting copies it (or moves it, for a cut) into the selected folder. */
  const treeClipboard = useRef<{ entry: Entry; cut: boolean } | null>(null);

  /* Where a paste or an undropped import goes: the selected folder, or the folder of the selected file, or the top of the pack. */
  const targetFolder = () => (selected ? (selected.is_dir ? selected.rel : parentOf(selected.rel)) : '');

  /** Files dragged in from another program (or pasted) are copied into a folder of the pack. The transfer is read at once, because it is gone when the handler returns. */
  const importFromOutside = async (transfer: DataTransfer, folder: string) => {
    const reading = collect(transfer);

    try {
      const { files, skipped } = await reading;

      if (files.length === 0) {
        notify(skipped.length > 0 ? `Nothing was added: ${skipped.join(', ')} could not be read or is too big (200 MB is the most for one file).` : 'There was nothing to add.');
        return;
      }

      const made = await api.importFiles(folder, files);

      changed();
      notify(
        made.length === 1 ? `Added ${made[0]}.` : `Added ${made.length} items to ${folder || 'the top of the pack'}.` + (skipped.length > 0 ? ` Left out: ${skipped.join(', ')}.` : ''),
        skipped.length === 0,
      );
    } catch (error) {
      notify(messageOf(error));
    }
  };

  /** Pastes what Ctrl+C or Ctrl+X took in the tree. */
  const pasteInTree = async () => {
    const taken = treeClipboard.current;

    if (!taken) return;

    const folder = targetFolder();

    try {
      if (taken.cut) {
        const target = (folder ? folder + '/' : '') + taken.entry.name;

        if (target === taken.entry.rel) return;

        await closeUnder(taken.entry.rel, false);
        await api.renameEntry(taken.entry.rel, target);
        rememberMove(taken.entry.rel, target);
        treeClipboard.current = null;
        setSelected(null);
        changed();
        notify(`${taken.entry.name} was moved to ${folder || 'the top of the pack'}. (Ctrl+Z undoes it.)`, true);
      } else {
        const copy = await api.copyInto(taken.entry.rel, folder);

        changed();
        notify(`Copied to ${copy}.`, true);
      }
    } catch (error) {
      notify(messageOf(error));
    }
  };

  const doRename = async (entry: Entry, name: string) => {
    const target = (parentOf(entry.rel) ? parentOf(entry.rel) + '/' : '') + name;

    setDialog(null);

    try {
      await closeUnder(entry.rel, false);
      await api.renameEntry(entry.rel, target);
      rememberMove(entry.rel, target);
      changed();
    } catch (error) {
      notify(messageOf(error));
    }
  };

  /* An entry was dragged onto a folder (or the top of the pack): it moves there, keeping its name. */
  const doMove = async (entry: Entry, folder: string) => {
    const target = (folder ? folder + '/' : '') + entry.name;

    try {
      await closeUnder(entry.rel, false);
      await api.renameEntry(entry.rel, target);
      rememberMove(entry.rel, target);
      setSelected(null);
      changed();
      notify(`${entry.name} was moved to ${folder || 'the top of the pack'}. (Ctrl+Z undoes it.)`, true);
    } catch (error) {
      notify(messageOf(error));
    }
  };

  const doDelete = async (entry: Entry) => {
    setDialog(null);

    try {
      await closeUnder(entry.rel, true);
      await api.deleteEntry(entry.rel);
      changed();
      notify(`${entry.name} was moved to the Recycle Bin.`, true);
    } catch (error) {
      notify(messageOf(error));
    }
  };

  const doCreate = async (parent: string, name: string, isDir: boolean) => {
    const rel = (parent ? parent + '/' : '') + name;

    setDialog(null);

    try {
      await api.createEntry(rel, isDir);
      changed();

      if (!isDir) await openFile(rel);
    } catch (error) {
      notify(messageOf(error));
    }
  };

  /* ---------- Getting the pack out ---------- */

  const errorCount = () => Object.values(problemsRef.current).flat().filter(p => p.severity === 'error').length;

  const exportZip = async () => {
    await saveAll();

    const destination = await pickSavePath('Save the pack as a zip', opened.meta.name + '.zip');

    if (!destination) return;

    try {
      const count = await api.exportZip(destination);

      notify(`Saved ${count} files to ${destination}.` + (errorCount() > 0 ? ` The pack still has ${errorCount()} errors.` : ''), errorCount() === 0);
    } catch (error) {
      notify(messageOf(error));
    }
  };

  const tryInWorld = async () => {
    await saveAll();

    const world = await pickFolder("Choose the world's folder (the one with level.dat)", (await api.savesDir()) ?? undefined);

    if (!world) return;

    try {
      const target = await api.installPack(world);

      notify(`Copied to ${target}. In the game, run /reload (or open the world), then check the pack is listed with /datapack list.` + (errorCount() > 0 ? ` The pack still has ${errorCount()} errors.` : ''), errorCount() === 0);
    } catch (error) {
      notify(messageOf(error));
    }
  };

  /* The tour opens by itself the first time a project is opened, and after that only when asked for. Skipping it also silences the one on the home page. */
  const [touring, setTouring] = useState(false);
  const [dockOpen, setDockOpen] = useState(false);
  const [dockPrefs, setDockPrefs] = useState<DockPrefs>(loadPrefs);

  useEffect(() => {
    if (tourSeen('workspace')) return;

    const timer = window.setTimeout(() => setTouring(true), 700);

    return () => window.clearTimeout(timer);
  }, []);

  const endTour = (skipped: boolean) => {
    markTours(...(skipped ? ALL_TOURS : (['workspace'] as const)));
    setTouring(false);
  };

  const jump = (problem: Diagnostic) => void openFile(problem.file, { line: problem.line, column: problem.column });

  const setSideAndRemember = () => {
    const next = side === 'right' ? 'left' : 'right';

    setSide(next);

    try {
      localStorage.setItem('tree-side', next);
    } catch {
      /* The choice is only not remembered. */
    }
  };

  /* ---------- View ---------- */

  const all = useMemo(() => Object.values(problems).flat(), [problems]);
  const errors = all.filter(p => p.severity === 'error').length;
  const warnings = all.filter(p => p.severity === 'warning').length;
  const tab = tabs.find(t => t.rel === active);
  const showEditor = tab?.view === 'text';

  const dockItems: DockItem[] = [
    { id: 'play', icon: 'play', label: 'Try in a world', desc: 'Copy the pack into a world to test it in the game', group: 'Pack', tour: 'play', run: () => void tryInWorld() },
    { id: 'export', icon: 'export', label: 'Export as zip', desc: 'Write the pack as a zip to share', group: 'Pack', tour: 'export', run: () => void exportZip() },
    { id: 'update', icon: 'split', label: 'Update the pack', desc: 'One model per participant, in place (can be undone)', group: 'Pack', tour: 'update', run: () => setDialog({ type: 'split' }) },
    { id: 'fixes', icon: 'wrench', label: 'Fix the models', desc: 'Armour, arms and the old male rig: apply all or pick', group: 'Pack', tour: 'fixes', run: () => setDialog({ type: 'fixes' }) },
    { id: 'import', icon: 'import', label: 'Import an old girl', desc: 'From her resource pack (folder or zip) or profile', group: 'Girls and scenes', tour: 'import', run: () => setDialog({ type: 'import' }) },
    { id: 'scope', icon: 'scope', label: 'Who each scene is for', desc: 'One girl, or every girl on a skeleton', group: 'Girls and scenes', tour: 'scope', run: () => setDialog({ type: 'scope' }) },
    { id: 'rig', icon: 'rig', label: 'Skeleton ids', desc: 'Which rig each model and animation file is made for', group: 'Girls and scenes', tour: 'rig', run: () => setDialog({ type: 'skeletons' }) },
    { id: 'blockbench', icon: 'cube', label: 'Blockbench', desc: 'Install it, or open it with the Strut Toolkit', group: 'Models and Blockbench', tour: 'blockbench', run: () => setDialog({ type: 'blockbench' }) },
    { id: 'log', icon: 'log', label: 'Game log', desc: 'What the game said about this pack on its last run', group: 'Help', tour: 'log', run: () => setDialog({ type: 'gamelog' }) },
    { id: 'docs', icon: 'docs', label: 'Documentation', desc: 'How girls, scenes, traits, dialogue and assets work (F1)', group: 'Help', tour: 'docs', run: () => setDialog({ type: 'docs' }) },
    { id: 'tour', icon: 'help', label: 'Take the tour', desc: 'A short walk round this window', group: 'Help', tour: 'tour', run: () => setTouring(true) }
  ];
  const pinnedItems = arrange(dockItems, dockPrefs).filter(item => dockPrefs.pinned.includes(item.id));

  return (
    <div class="workspace">
      <div class="topbar">
        <IconButton left icon="home" tip="Back to the projects" onClick={() => void (async () => { await saveAll(); onHome(); })()} />
        <span class="name">{opened.meta.name}</span>
        <span class="chip" title="The namespace of this pack's data">{opened.meta.namespace}</span>
        <span class="grow" />
        <IconButton primary tour="new" icon="plus" tip="New girl, scene, trait, dialogue... (Ctrl+N)" onClick={() => setDialog({ type: 'new', kind: 'girl' })} />
        <IconButton tour="quick" icon="search" tip="Open a file by name (Ctrl+P)" onClick={() => setDialog({ type: 'quick' })} />
        <IconButton tour="find" icon="find" tip="Find and replace in all files (Ctrl+Shift+F)" onClick={() => setDialog({ type: 'search' })} />
        <span class="bar-sep" />
        <span class="bar-sep" />
        {pinnedItems.map(item => <IconButton key={item.id} icon={item.icon} tip={`${item.label}: ${item.desc}`} onClick={item.run} />)}
        {pinnedItems.length > 0 && <span class="bar-sep" />}
        <IconButton tour="dock" icon="dots" tip="Tools: every tool with its name (Ctrl+J). Customize it there" onClick={() => setDockOpen(open => !open)} />
        <IconButton tour="settings" icon="settings" tip="Settings" onClick={() => setDialog({ type: 'settings' })} />
        <IconButton icon={side === 'left' ? 'sideLeft' : 'sideRight'} tip={`File tree: on the ${side}. Click to move it to the ${side === 'left' ? 'right' : 'left'}`} onClick={setSideAndRemember} />
      </div>

      <div class={'body tree-' + side}>
        <aside class="sidebar" style={{ width: sizes.side }}>
          <div class="sidebar-head">
            <span>Files</span>
            <span class="spacer" />
            <button class="icon-btn" data-tip="Fold all folders" aria-label="Fold all folders" onClick={() => setCollapseKey(n => n + 1)}><ToolIcon name="collapse" /></button>
            <button class="btn ghost small" title={selected ? `New file in ${parentFolder(selected) || 'the pack'}` : 'New file'} onClick={() => setDialog({ type: 'prompt-file', parent: parentFolder(selected) })}>＋ File</button>
            <button class="btn ghost small" title={selected ? `New folder in ${parentFolder(selected) || 'the pack'}` : 'New folder'} onClick={() => setDialog({ type: 'prompt-folder', parent: parentFolder(selected) })}>＋ Folder</button>
          </div>
          <Tree version={version} collapseKey={collapseKey} active={active} selected={selected?.rel ?? null} problems={problems} onOpen={rel => void openFile(rel)} onSelect={setSelected} onMove={(entry, folder) => void doMove(entry, folder)} onDropFiles={(transfer, folder) => void importFromOutside(transfer, folder)} onContext={(entry, x, y) => { if (entry) setSelected(entry); setMenu({ entry, x, y }); }} />
        </aside>

        <Splitter
          direction="vertical"
          onStart={() => { startSizes.current = sizes; }}
          onDrag={dx => setSizes(s => ({ ...s, side: clampSide(startSizes.current.side + (side === 'right' ? -dx : dx)) }))}
          onEnd={() => saveSizes(sizesRef.current)}
          onReset={() => { const next = { ...sizes, side: DEFAULT_SIZES.side }; setSizes(next); saveSizes(next); }}
        />
        <main class="center">
          <div class="tabs" ref={tabStrip} onWheel={event => { const strip = tabStrip.current; if (strip && strip.scrollWidth > strip.clientWidth && Math.abs(event.deltaY) > Math.abs(event.deltaX)) { strip.scrollLeft += event.deltaY; event.preventDefault(); } }}>
            {tabs.map(t => (
              <div key={t.rel} class={'tab' + (t.rel === active ? ' active' : '')} title={t.rel} onContextMenu={event => { event.preventDefault(); event.stopPropagation(); setTabMenu({ x: event.clientX, y: event.clientY, rel: t.rel }); }} onClick={() => setActive(t.rel)} onAuxClick={event => { if (event.button === 1) void closeTab(t.rel); }}>
                <FileIcon kind={t.kind} />
                <span>{t.view === 'sim' ? '▶ ' + nameOf(t.rel.slice(9)) : nameOf(t.rel)}</span>
                {t.dirty ? <span class="dot" title="Saving soon" /> : null}
                <button class="close" title="Close (Ctrl+W)" onClick={event => { event.stopPropagation(); void closeTab(t.rel); }}>×</button>
              </div>
            ))}
          </div>

          {tab?.view === 'text' && (
            <div class="viewbar">
              {['girl', 'scene', 'trait', 'dialogue', 'social', 'armour_wearer'].includes(tab.kind) && (
                <>
                  <button class={'seg' + (view === 'form' ? ' on' : '')} onClick={() => chooseView('form')}>Form</button>
                  <button class={'seg' + (view === 'json' ? ' on' : '')} onClick={() => chooseView('json')}>JSON</button>
                </>
              )}
              <span class="grow" />
              <button class="btn ghost small" title="Tidy the file's layout (Shift+Alt+F)" onClick={() => void editor.current?.getAction('editor.action.formatDocument')?.run()}>Format</button>
              {tab.kind === 'girl' && <button class="btn small" title="See her model in the viewer, with her animations" onClick={() => void previewGirl(tab.rel)}>▶ Preview</button>}
              {tab.kind === 'scene' && <button class="btn small" title="Play this scene as the game does: strip, bed, join, thrust, cues" onClick={() => openSim(tab.rel)}>▶ Simulate</button>}
              <button class="btn ghost small" title="Earlier versions of this file" onClick={() => setDialog({ type: 'history', rel: tab.rel })}>History…</button>
            </div>
          )}

          <div class="editor-area">
            <div class="editor-host" ref={host} style={{ visibility: showEditor && !formShown ? 'visible' : 'hidden' }} />

            {formShown && reference && activeTab?.kind === 'girl' && (
              <GirlForm text={liveText} reference={reference} packTraits={packTraits} onChange={writeFromForm} onShowJson={() => chooseView('json')} />
            )}
            {formShown && activeTab?.kind === 'armour_wearer' && (
              <WearerForm rel={activeTab.rel} text={liveText} onChange={writeFromForm} onShowJson={() => chooseView('json')} />
            )}
            {formShown && reference && activeTab?.kind === 'trait' && (
              <TraitForm text={liveText} reference={reference} onChange={writeFromForm} onShowJson={() => chooseView('json')} />
            )}
            {formShown && reference && activeTab?.kind === 'dialogue' && (
              <DialogueForm text={liveText} reference={reference} onChange={writeFromForm} onShowJson={() => chooseView('json')} />
            )}
            {formShown && reference && activeTab?.kind === 'social' && (
              <SocialForm text={liveText} reference={reference} onChange={writeFromForm} onShowJson={() => chooseView('json')} />
            )}
            {formShown && reference && activeTab?.kind === 'scene' && (
              <SceneForm text={liveText} reference={reference} onChange={writeFromForm} onShowJson={() => chooseView('json')} />
            )}

            {!tab && (
              <div class="overlay">
                <div class="welcome">
                  <h2>{opened.meta.name}</h2>
                  <div>Open a file from the tree, or press <kbd>Ctrl</kbd> + <kbd>P</kbd> to find one.</div>
                  <div><kbd>Ctrl</kbd> + <kbd>N</kbd> makes a new girl, scene, trait or dialogue.</div>
                  <div>Everything is saved as you type.</div>
                </div>
              </div>
            )}
            {tab?.view === 'viewer' && <Viewer key={tab.rel} rel={tab.rel} />}
            {tab?.view === 'sim' && <SimView key={tab.rel} sceneRel={tab.kind === 'scene' ? tab.rel.slice(9) : ''} girlRel={tab.kind === 'girl' ? tab.rel.slice(9) : ''} />}
            {tab?.view === 'image' && tab.image && <ImageView key={tab.rel} src={tab.image} rel={tab.rel} size={tab.size} />}
            {(tab?.view === 'binary' || tab?.view === 'big') && (
              <div class="overlay">
                <div class="welcome">
                  <h2>{nameOf(tab.rel)}</h2>
                  <div>{tab.view === 'big' ? 'This file is too large to open here.' : 'This is not a text file, so it cannot be edited here.'}</div>
                  <div class="muted">{Math.round((tab.size ?? 0) / 1024)} KiB</div>
                </div>
              </div>
            )}
          </div>

          {!collapsed && (
            <Splitter
              direction="horizontal"
              onStart={() => { startSizes.current = sizes; }}
              onDrag={(_, dy) => setSizes(s => ({ ...s, problems: clampProblems(startSizes.current.problems - dy) }))}
              onEnd={() => saveSizes(sizesRef.current)}
              onReset={() => { const next = { ...sizes, problems: DEFAULT_SIZES.problems }; setSizes(next); saveSizes(next); }}
            />
          )}
          <Problems height={sizes.problems} problems={all} checking={checking} collapsed={collapsed} onToggle={() => setCollapsed(c => !c)} onJump={jump} onRecheck={() => void recheck()} />
        </main>
      </div>

      <div class="statusbar">
        <span class={saveState === 'saved' ? 'saved' : saveState === 'saving' ? 'saving' : ''}>{saveState === 'saved' ? 'All changes saved' : saveState === 'saving' ? 'Saving…' : tabs.some(t => t.dirty) ? 'Unsaved changes' : 'Autosave is on'}</span>
        <span>{errors} errors · {warnings} warnings</span>
        <span class="right">
          {showEditor && <span>Ln {cursor.line}, Col {cursor.column}</span>}
          <span>{opened.root}</span>
        </span>
      </div>

      {tabMenu && (
        <div class="menu" style={{ left: Math.min(tabMenu.x, window.innerWidth - 220), top: tabMenu.y }} onClick={event => event.stopPropagation()}>
          <button onClick={() => { const rel = tabMenu.rel; setTabMenu(null); void closeTab(rel); }}>Close</button>
          <button onClick={() => { const rel = tabMenu.rel; setTabMenu(null); for (const t of tabsRef.current) if (t.rel !== rel) void closeTab(t.rel); }}>Close the others</button>
          <button onClick={() => { setTabMenu(null); for (const t of tabsRef.current) void closeTab(t.rel); }}>Close all</button>
          {!tabMenu.rel.startsWith('simulate:') && (
            <>
              <hr />
              <button onClick={() => { const rel = tabMenu.rel; setTabMenu(null); void navigator.clipboard.writeText(rel).then(() => notify('Path copied.', true)); }}>Copy path</button>
              <button onClick={() => { const rel = tabMenu.rel; setTabMenu(null); void api.revealInExplorer(rel).catch(error => notify(messageOf(error))); }}>Show in Explorer</button>
            </>
          )}
        </div>
      )}

      {menu && (
        <div class="menu" style={{ left: Math.min(menu.x, window.innerWidth - 220), top: Math.min(menu.y, window.innerHeight - 260) }} onClick={event => event.stopPropagation()}>
          <div class="heading">New</div>
          {(['girl', 'scene', 'trait', 'dialogue', 'social'] as FileKind[]).map(kind => (
            <button key={kind} onClick={() => { setMenu(null); setDialog({ type: 'new', kind }); }}><FileIcon kind={kind} />{kind === 'social' ? 'Small talk' : kind[0].toUpperCase() + kind.slice(1)}…</button>
          ))}
          <button onClick={() => { setMenu(null); setDialog({ type: 'prompt-file', parent: parentFolder(menu.entry) }); }}>File…</button>
          <button onClick={() => { setMenu(null); setDialog({ type: 'prompt-folder', parent: parentFolder(menu.entry) }); }}>Folder…</button>
          {menu.entry && (
            <>
              <hr />
              {/^data\/[^/]+\/scenes\/[^/]+\.json$/.test(menu.entry.rel) && <button onClick={() => { const entry = menu.entry!; setMenu(null); setDialog({ type: 'scope', focus: entry.rel.slice(entry.rel.lastIndexOf('/') + 1).replace(/\.json$/, '') }); }}>Who this scene is for…</button>}
              <button onClick={() => { const entry = menu.entry!; setMenu(null); setDialog({ type: 'rename', entry }); }}>Rename…</button>
              <button onClick={() => { const entry = menu.entry!; setMenu(null); void api.duplicateEntry(entry.rel).then(copy => { changed(); notify(`Copied to ${copy}.`, true); }).catch(error => notify(messageOf(error))); }}>Duplicate</button>
              <button onClick={() => { const entry = menu.entry!; setMenu(null); void navigator.clipboard.writeText(entry.rel).then(() => notify('Path copied.', true)); }}>Copy path</button>
              <button onClick={() => { const entry = menu.entry!; setMenu(null); void api.revealInExplorer(entry.rel).catch(error => notify(messageOf(error))); }}>Show in Explorer</button>
              <button onClick={() => { const entry = menu.entry!; setMenu(null); setDialog({ type: 'delete', entry }); }}>Delete</button>
            </>
          )}
        </div>
      )}

      {dialog?.type === 'new' && (
        <NewFileDialog initialKind={dialog.kind} onClose={() => setDialog(null)} onCreated={rel => { setDialog(null); changed(); void openFile(rel); }} />
      )}
      {dialog?.type === 'prompt-file' && <Prompt title="New file" label={`In ${dialog.parent || 'the pack'}`} confirm="Create" onClose={() => setDialog(null)} onDone={name => void doCreate(dialog.parent, name, false)} />}
      {dialog?.type === 'prompt-folder' && <Prompt title="New folder" label={`In ${dialog.parent || 'the pack'}`} confirm="Create" onClose={() => setDialog(null)} onDone={name => void doCreate(dialog.parent, name, true)} />}
      {dialog?.type === 'rename' && <Prompt title="Rename" label="New name" initial={dialog.entry.name} confirm="Rename" onClose={() => setDialog(null)} onDone={name => void doRename(dialog.entry, name)} />}
      {dialog?.type === 'delete' && <Confirm title="Delete" text={`Move ${dialog.entry.name} to the Recycle Bin?`} confirm="Delete" onClose={() => setDialog(null)} onDone={() => void doDelete(dialog.entry)} />}
      {dialog?.type === 'scope' && <ScopeDialog focus={dialog.focus} onClose={() => setDialog(null)} onChanged={changed} />}
      {dialog?.type === 'import' && <ImportDialog onClose={() => setDialog(null)} onImported={result => { changed(); setDialog({ type: 'report', result }); }} />}
      {dialog?.type === 'history' && <HistoryDialog rel={dialog.rel} current={liveText} onRestore={text => { setDialog(null); writeFromForm(text); }} onClose={() => setDialog(null)} />}
      {dialog?.type === 'search' && <SearchDialog onClose={() => setDialog(null)} onOpen={(rel, at) => void openFile(rel, at)} onReplaced={changed} />}
      {dialog?.type === 'settings' && <SettingsDialog onClose={() => setDialog(null)} onResetLayout={() => { const next = { ...DEFAULT_SIZES }; setSizes(next); saveSizes(next); }} />}
      {dialog?.type === 'docs' && <DocsDialog start={dialog.start} onClose={() => setDialog(null)} onTour={() => { setDialog(null); setTouring(true); }} />}
      <Dock open={dockOpen || touring} items={dockItems} prefs={dockPrefs} onPrefs={setDockPrefs} onClose={() => setDockOpen(false)} />
      {touring && <Tour steps={workspaceSteps(page => { endTour(false); setDialog({ type: 'docs', start: page }); })} onDone={() => endTour(false)} onSkip={() => endTour(true)} />}
      {dialog?.type === 'gamelog' && <GameLogDialog onClose={() => setDialog(null)} onOpen={rel => void openFile(rel)} />}
      {dialog?.type === 'fixes' && <FixesDialog onClose={() => setDialog(null)} />}
      {dialog?.type === 'split' && <SplitDialog onClose={() => setDialog(null)} />}
      {dialog?.type === 'skeletons' && <SkeletonDialog onClose={() => setDialog(null)} />}
      {dialog?.type === 'blockbench' && <BlockbenchDialog onClose={() => setDialog(null)} />}
      {dialog?.type === 'report' && <ImportReport result={dialog.result} onClose={() => setDialog(null)} onOpen={() => { const rel = dialog.result.girl; setDialog(null); void openFile(rel); }} />}
      {dialog?.type === 'quick' && <QuickOpen onClose={() => setDialog(null)} onOpen={rel => { setDialog(null); void openFile(rel); }} />}
    </div>
  );
}
