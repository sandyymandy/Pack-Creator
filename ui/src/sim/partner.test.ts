import { describe, expect, it } from 'vitest';
import type { ClipData, ModelData } from '../types';
import { withPartner, withPartnerClips } from './partner';

const model = (bones: string[], texture: string): ModelData => ({
  version: 101, unitsPerBlock: 16, boundsMin: [0, 0, 0], boundsMax: [1, 1, 1],
  textures: [{ name: texture, width: 64, height: 64, uvWidth: 64, uvHeight: 64 }],
  groupBones: bones.map((name, i) => ({ name, parentIndex: i === 0 ? -1 : 0, pivot: [0, 0, 0], rotation: [0, 0, 0], elements: [{ kind: 1, index: i }] })),
  meshes: [],
  cubes: bones.map(name => ({ name, textureIndex: 0, origin: [0, 0, 0], rotation: [0, 0, 0], from: [0, 0, 0], to: [1, 1, 1], inflate: 0, mirror: false, boxUv: true, faces: [] })),
  armatureCount: 0, armatures: [], extensions: []
});

const clip = (name: string, targets: string[]): ClipData => ({
  name, duration: 1, loopMode: 1, events: [{ time: 0.5, kind: 'instruction', payload: 'x', authority: 0 }],
  channels: targets.map(target => ({ target, kind: 'GROUP_BONE', property: 'ROTATION', keys: [] }))
});

describe('the partner joined to the girl for drawing', () => {
  const girl = model(['body', 'Head'], 'girl.png');
  const partner = model(['player', 'head', 'torso'], 'player.png');

  it('adds the partner as a steve rig whose bones cannot meet hers', () => {
    const both = withPartner(girl, partner);

    expect(both.groupBones.map(b => b.name)).toEqual(['body', 'Head', 'steve', 'partner:head', 'partner:torso']);
    expect(both.groupBones[3].parentIndex).toBe(2);
    expect(both.groupBones[2].parentIndex).toBe(-1);
    expect(both.cubes).toHaveLength(5);
    expect(both.groupBones[4].elements[0].index).toBe(4);
    expect(both.cubes[3].textureIndex).toBe(1);
    expect(both.textures[1].name).toBe('player.png');
  });

  it('leaves a model that still has the old rig alone', () => {
    const old = model(['body', 'steve'], 'girl.png');

    expect(withPartner(old, partner)).toBe(old);
  });

  it('joins the partner clip of the same name and keeps the girl length, loop and events', () => {
    const files = withPartnerClips([{ path: 'a', clips: [clip('idle', ['body']), clip('other', ['body'])] }], [{ path: 'b', clips: [clip('idle', ['player', 'head'])] }], partner);
    const [idle, other] = files[0].clips;

    expect(idle.channels.map(c => c.target)).toEqual(['body', 'steve', 'partner:head']);
    expect(idle.events).toHaveLength(1);
    expect(other.channels.map(c => c.target)).toEqual(['body']);
  });
});
