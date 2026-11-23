import { useEffect, useState } from 'preact/hooks';
import { api } from './api';
import { Labeled } from './GirlForm';

type Json = Record<string, unknown>;

const isObject = (value: unknown): value is Json => typeof value === 'object' && value !== null && !Array.isArray(value);

const SLOTS: { id: string; label: string; hint: string }[] = [
  { id: 'head', label: 'Head', hint: 'Helmet' },
  { id: 'chest', label: 'Chest', hint: 'Chestplate: usually hides the breasts so the armour is drawn instead' },
  { id: 'legs', label: 'Legs', hint: 'Leggings: usually hides what the pants cover' },
  { id: 'feet', label: 'Feet', hint: 'Boots' }
];

/** The bones of her models, for suggesting what a slot can hide. */
function useBones(rel: string): string[] {
  const [bones, setBones] = useState<string[]>([]);

  useEffect(() => {
    let current = true;
    const girl = rel.split('/').pop()?.replace(/\.json$/, '') ?? '';

    void (async () => {
      try {
        const files = await api.viewerFiles(`data/x/asset_sync/strut/models/entities/girls/nude/${girl}.strutmodel`);
        const names = new Set<string>();

        for (const model of files.models) {
          const data = await api.readModel(model).catch(() => null);

          data?.groupBones.forEach(b => names.add(b.name));
        }

        if (current) setBones([...names].sort());
      } catch {
        if (current) setBones([]);
      }
    })();

    return () => { current = false; };
  }, [rel]);

  return bones;
}

/**
 * An armour wearer as a form: which armour set she wears (or the wearer it starts from), and for each slot the bones of hers that the armour hides while it is worn, and how
 * much of her soft bones' movement it damps. Anything else in the file (fit, bones, trims, glint) is kept as it is.
 */
export function WearerForm({ rel, text, onChange, onShowJson }: { rel: string; text: string; onChange: (text: string) => void; onShowJson: () => void }) {
  const bones = useBones(rel);
  const [adding, setAdding] = useState<Record<string, string>>({});
  let root: Json | null = null;

  try {
    const parsed: unknown = JSON.parse(text);

    root = isObject(parsed) ? parsed : null;
  } catch {
    root = null;
  }

  if (!root) {
    return (
      <div class="form-pane">
        <div class="form-note">
          This file is not valid JSON right now, or is not an object, so the form cannot read it. Fix it in the JSON view, and the form comes back.
          <div style="margin-top:10px"><button class="btn" onClick={onShowJson}>Show the JSON</button></div>
        </div>
      </div>
    );
  }

  const file = root;
  const write = (next: Json) => onChange(JSON.stringify(next, null, 2) + '\n');
  const slots = isObject(file.slots) ? file.slots : {};

  const setSlot = (slot: string, change: (value: Json) => Json) => {
    const next = change(isObject(slots[slot]) ? { ...(slots[slot] as Json) } : {});
    const all = { ...slots };

    if (Object.keys(next).length === 0) delete all[slot];
    else all[slot] = next;

    const out = { ...file };

    if (Object.keys(all).length === 0) delete out.slots;
    else out.slots = all;

    write(out);
  };

  const coversOf = (slot: string): string[] => {
    const value = isObject(slots[slot]) ? (slots[slot] as Json).covers : undefined;

    return Array.isArray(value) ? value.map(String) : [];
  };

  const dampsOf = (slot: string): [string, number][] => {
    const value = isObject(slots[slot]) ? (slots[slot] as Json).damps : undefined;

    return isObject(value) ? Object.entries(value).map(([k, v]) => [k, Number(v)] as [string, number]) : [];
  };

  const setCovers = (slot: string, list: string[]) => setSlot(slot, value => { const next = { ...value }; if (list.length) next.covers = list; else delete next.covers; return next; });
  const setDamps = (slot: string, list: [string, number][]) => setSlot(slot, value => { const next = { ...value }; if (list.length) next.damps = Object.fromEntries(list); else delete next.damps; return next; });

  return (
    <div class="form-pane">
      <div class="form-inner">
      <div class="form-card">
      <h3>Armour wearer</h3>
      <Labeled label="Starts from" hint="Another wearer, such as pleasurehorizons:girl. What this file lists replaces what that one has for the same slot.">
        <input class="input" value={String(file.extends ?? '')} placeholder="pleasurehorizons:girl" onInput={e => { const v = (e.target as HTMLInputElement).value.trim(); const out = { ...file }; if (v) out.extends = v; else delete out.extends; write(out); }} />
      </Labeled>
      </div>
      <datalist id="wearer-bones">{bones.map(b => <option key={b} value={b} />)}</datalist>

      {SLOTS.map(slot => (
        <div key={slot.id} class="form-card">
          <h3>{slot.label}</h3>
          <div class="hint">{slot.hint}</div>
          <Labeled label="Bones it hides" hint="Her own bones that are not drawn while this armour is worn.">
            <div class="tagrow">
              {coversOf(slot.id).map(bone => (
                <button key={bone} class="tag on" title="Remove" onClick={() => setCovers(slot.id, coversOf(slot.id).filter(b => b !== bone))}>{bone} ✕</button>
              ))}
            </div>
            <div class="row">
              <input class="input grow" list="wearer-bones" placeholder="Add a bone" value={adding[slot.id] ?? ''} onInput={e => setAdding({ ...adding, [slot.id]: (e.target as HTMLInputElement).value })}
                onKeyDown={e => { if (e.key === 'Enter') { const v = (adding[slot.id] ?? '').trim(); if (v && !coversOf(slot.id).includes(v)) setCovers(slot.id, [...coversOf(slot.id), v]); setAdding({ ...adding, [slot.id]: '' }); } }} />
              <button class="btn" onClick={() => { const v = (adding[slot.id] ?? '').trim(); if (v && !coversOf(slot.id).includes(v)) setCovers(slot.id, [...coversOf(slot.id), v]); setAdding({ ...adding, [slot.id]: '' }); }}>Add</button>
            </div>
          </Labeled>
          <Labeled label="Damps" hint="How much of a group of soft bones' movement is kept under this armour: 0 holds them still, 1 leaves them free.">
            {dampsOf(slot.id).map(([name, amount]) => (
              <div class="row" key={name}>
                <input class="input" style="width:140px" value={name} onChange={e => setDamps(slot.id, dampsOf(slot.id).map(d => d[0] === name ? [(e.target as HTMLInputElement).value.trim() || name, d[1]] : d))} />
                <input class="input" type="number" min={0} max={1} step={0.05} style="width:90px" value={amount} onInput={e => setDamps(slot.id, dampsOf(slot.id).map(d => d[0] === name ? [d[0], Number((e.target as HTMLInputElement).value)] : d))} />
                <button class="btn ghost small" onClick={() => setDamps(slot.id, dampsOf(slot.id).filter(d => d[0] !== name))}>Remove</button>
              </div>
            ))}
            <button class="btn ghost small" onClick={() => { let n = 'group'; while (dampsOf(slot.id).some(d => d[0] === n)) n += '_2'; setDamps(slot.id, [...dampsOf(slot.id), [n, 0]]); }}>Add a damp</button>
          </Labeled>
        </div>
      ))}
      </div>
    </div>
  );
}
