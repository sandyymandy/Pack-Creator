import type { ClipData } from '../types';

/*
 * What a clip does in a scene, from its own name: `animation.<scene>.<role>[_n]`. This is the game's `SceneRole`, `AnimationNameParser` and `SceneClipCatalogue`, in the same
 * words, so that the simulator picks clips the way the game does. Keys are matched exactly, capitals included (`slow_L` is not `slow_l`), longest first.
 */

export type Role = 'intro' | 'slow' | 'slow_L' | 'slow_R' | 'slow_L_swap' | 'slow_R_swap' | 'fast' | 'cum' | 'lay_on_bed' | 'bed_idle' | 'loop' | 'solo_intro';

export const ROLES: Role[] = ['intro', 'slow', 'slow_L', 'slow_R', 'slow_L_swap', 'slow_R_swap', 'fast', 'cum', 'lay_on_bed', 'bed_idle', 'loop', 'solo_intro'];

export interface Parsed {
  sceneId: string;
  role: Role;
  /** Zero based: `fast` and `fast_1` are the first, `fast_2` the second. */
  index: number;
}

export function parseClipName(name: string): Parsed | null {
  if (!name.startsWith('animation.')) return null;

  const rest = name.slice('animation.'.length);
  const split = rest.indexOf('.');

  if (split <= 0 || split === rest.length - 1) return null;

  const sceneId = rest.slice(0, split);
  const roleText = rest.slice(split + 1);
  let best: Role | null = null;

  for (const role of ROLES) if (roleText.startsWith(role) && (best === null || role.length > best.length)) best = role;

  if (!best) return null;

  const suffix = roleText.slice(best.length);

  if (suffix === '') return { sceneId, role: best, index: 0 };

  const digits = suffix.startsWith('_') ? suffix.slice(1) : suffix;

  if (!/^\d+$/.test(digits) || Number(digits) < 1) return null;

  return { sceneId, role: best, index: Number(digits) - 1 };
}

/** An animation file of the pack, with its clips. */
export interface AnimationFile {
  path: string;
  clips: ClipData[];
}

const BASE = '';

/** The clips of one scene, by role, for each girl a scene file was made for (the shared file is under the empty name). */
export class SceneCatalogue {
  private readonly byGirl = new Map<string, Map<Role, string[]>>();
  private readonly sides = new Map<string, Role>();

  /**
   * A scene in the old layout is single files (`<scene>-<girl>`, `<scene>`). A scene in the new layout is a folder per role: the host's clips are in `<scene>/host/characters/<girl>`
   * (what she changes) and `<scene>/host/skeletons/<skeleton>` (what everyone on her skeleton plays), and each clip is looked up in her file first and in her skeleton's second.
   */
  constructor(files: AnimationFile[], readonly sceneId: string, readonly girlId: string, readonly replaceDefault: boolean) {
    const raw = new Map<string, Map<Role, Map<number, string>>>();
    const firstSides = new Map<string, Role>();
    let layered = false;

    const collect = (key: string, file: AnimationFile) => {
      const collected = raw.get(key) ?? new Map<Role, Map<number, string>>();

      for (const clip of file.clips) {
        const parsed = parseClipName(clip.name);

        if (!parsed || parsed.sceneId !== sceneId) continue;

        const variants = collected.get(parsed.role) ?? new Map<number, string>();

        variants.set(parsed.index, clip.name);
        collected.set(parsed.role, variants);

        if (!firstSides.has(key) && (parsed.role === 'slow_L' || parsed.role === 'slow_R')) firstSides.set(key, parsed.role);
      }

      raw.set(key, collected);
    };

    for (const file of files) {
      const name = file.path.slice(file.path.lastIndexOf('/') + 1).replace(/\.strutanim$/, '');

      if (!file.path.includes('/animations/scenes/')) continue;

      if (file.path.includes(`/animations/scenes/${sceneId}/host/`)) {
        layered = true;
        collect(file.path.includes('/host/characters/') || file.path.includes('/host/owners/') ? name : BASE, file);
        continue;
      }

      const girl = name === sceneId ? BASE : name.startsWith(sceneId + '-') ? name.slice(sceneId.length + 1) : null;

      if (girl !== null) collect(girl, file);
    }

    const finish = (variants: Map<Role, Map<number, string>>) => new Map([...variants].map(([role, byIndex]) => [role, [...byIndex.entries()].sort((a, b) => a[0] - b[0]).map(([, clipName]) => clipName)] as [Role, string[]]));

    for (const [key, variants] of raw) {
      this.byGirl.set(key, finish(variants));

      const side = firstSides.get(key);

      if (side) this.sides.set(key, side);
    }

    if (layered) {
      /* Her clips are her own file's, and the rest are her skeleton's: a clip of the same name in both is hers. */
      const merged = new Map<Role, Map<number, string>>();

      for (const source of [raw.get(BASE), raw.get(girlId)]) {
        for (const [role, variants] of source ?? []) {
          const into = merged.get(role) ?? new Map<number, string>();

          for (const [index, clipName] of variants) into.set(index, clipName);
          merged.set(role, into);
        }
      }

      this.byGirl.set(girlId, finish(merged));

      const side = firstSides.get(girlId) ?? firstSides.get(BASE);

      if (side) this.sides.set(girlId, side);
    }
  }

  hasGirlFile(): boolean {
    return this.byGirl.has(this.girlId);
  }

  /** `SceneClipCatalogue#animationsFor`: her own clips of the role, else (unless the scene replaces the defaults and she has a file) the shared ones. */
  of(role: Role): string[] {
    const hers = this.byGirl.get(this.girlId);

    if (hers?.has(role)) return hers.get(role)!;
    if (this.replaceDefault && this.hasGirlFile()) return [];

    return this.byGirl.get(BASE)?.get(role) ?? [];
  }

  startingSide(): Role {
    const hers = this.sides.get(this.girlId);

    if (hers) return hers;
    if (this.replaceDefault && this.hasGirlFile()) return 'slow_L';

    return this.sides.get(BASE) ?? 'slow_L';
  }

  /** Every role this scene has any clip for, to show what is missing. */
  roles(): Role[] {
    return ROLES.filter(role => this.of(role).length > 0);
  }
}
