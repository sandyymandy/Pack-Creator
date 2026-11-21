import { describe, expect, it } from 'vitest';
import type { ClipData } from '../types';
import { SceneCatalogue } from './roles';

const clip = (name: string): ClipData => ({ name, duration: 1, loopMode: 1, channels: [], events: [] });
const P = 'example:assets/pleasurehorizons/strut/animations/scenes/doggy/';

describe('a scene in the new layout', () => {
  const files = [
    { path: P + 'host/characters/momo.strutanim', clips: [clip('animation.doggy.slow_L'), clip('animation.doggy.fast_2')] },
    { path: P + 'host/skeletons/jenny.strutanim', clips: [clip('animation.doggy.intro'), clip('animation.doggy.slow_R'), clip('animation.doggy.fast'), clip('animation.doggy.fast_2')] }
  ];

  it('has her clips from her own file and the rest from her skeleton', () => {
    const catalogue = new SceneCatalogue(files, 'doggy', 'momo', false);

    expect(catalogue.hasGirlFile()).toBe(true);
    expect(catalogue.of('intro')).toEqual(['animation.doggy.intro']);
    expect(catalogue.of('fast')).toEqual(['animation.doggy.fast', 'animation.doggy.fast_2']);
    expect(catalogue.of('slow_L')).toEqual(['animation.doggy.slow_L']);
    expect(catalogue.startingSide()).toBe('slow_L');
    expect(catalogue.roles()).toContain('intro');
  });

  it('gives a girl with no file of her own her skeleton\'s clips', () => {
    const catalogue = new SceneCatalogue(files.slice(1), 'doggy', 'lucy', false);

    expect(catalogue.of('intro')).toEqual(['animation.doggy.intro']);
    expect(catalogue.startingSide()).toBe('slow_R');
  });

  it('still reads the old single files', () => {
    const old = [{ path: 'strut/animations/scenes/doggy-momo.strutanim', clips: [clip('animation.doggy.slow_L')] }, { path: 'strut/animations/scenes/doggy.strutanim', clips: [clip('animation.doggy.intro')] }];
    const catalogue = new SceneCatalogue(old, 'doggy', 'momo', false);

    expect(catalogue.of('slow_L')).toEqual(['animation.doggy.slow_L']);
    expect(catalogue.of('intro')).toEqual(['animation.doggy.intro']);
  });
});
