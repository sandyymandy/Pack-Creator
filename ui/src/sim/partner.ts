import type { ClipData, ModelData } from '../types';
import type { AnimationFile } from './roles';

/**
 * A scene in the new layout plays two models: the girl and the base mod's partner. The viewer draws one model with one clip, and already knows how to draw the partner as the
 * second rig of a model (a root called `steve`, drawn with the player's skin, hidden or picked as a whole), so the two are put together here for drawing: the partner's bones are
 * added to the girl's model (the root as `steve`, the others with a `partner:` in front so they cannot meet a bone of hers with the same name), and each clip of the girl is
 * joined by the partner's clip of the same name. The game keeps them apart; this is only how the simulator shows them.
 */
const ROOT = 'steve';
const PREFIX = 'partner:';

const nameOf = (bone: { name: string; parentIndex: number }) => (bone.parentIndex < 0 ? ROOT : PREFIX + bone.name);
const targetOf = (partner: ModelData, name: string): string => {
  const bone = partner.groupBones.find(b => b.name === name);

  return bone ? nameOf(bone) : PREFIX + name;
};

/** The girl's model with the partner's bones and cubes added. A model that already has a `steve` rig (an old one) is returned as it is. */
export function withPartner(girl: ModelData, partner: ModelData): ModelData {
  if (girl.groupBones.some(b => b.name === ROOT)) return girl;

  const bones = girl.groupBones.length;
  const cubes = girl.cubes.length;
  const texture = girl.textures.length;
  const source = partner.textures[0] ?? { name: 'player.png', width: 64, height: 64, uvWidth: 64, uvHeight: 64 };

  return {
    ...girl,
    textures: [...girl.textures, source],
    groupBones: [
      ...girl.groupBones,
      ...partner.groupBones.map(b => ({
        ...b,
        name: nameOf(b),
        parentIndex: b.parentIndex < 0 ? -1 : b.parentIndex + bones,
        elements: b.elements.map(e => ({ ...e, index: e.index + cubes }))
      }))
    ],
    cubes: [...girl.cubes, ...partner.cubes.map(c => ({ ...c, textureIndex: texture }))]
  };
}

/** The clip with the partner's clip of the same name joined to it (the girl's length, loop and events are kept). */
function joined(clip: ClipData, partner: ModelData, theirs: ClipData | undefined): ClipData {
  if (!theirs) return clip;

  return { ...clip, channels: [...clip.channels, ...theirs.channels.filter(c => c.kind === 'GROUP_BONE').map(c => ({ ...c, target: targetOf(partner, c.target) }))] };
}

/** The girl's animation files with the partner's clips joined to each clip they have a partner for; the partner's files are looked in in the order given. */
export function withPartnerClips(files: AnimationFile[], partnerFiles: AnimationFile[], partner: ModelData): AnimationFile[] {
  const find = (name: string) => {
    for (const file of partnerFiles) {
      const clip = file.clips.find(c => c.name === name);

      if (clip) return clip;
    }

    return undefined;
  };

  return files.map(file => ({ ...file, clips: file.clips.map(clip => joined(clip, partner, find(clip.name))) }));
}
