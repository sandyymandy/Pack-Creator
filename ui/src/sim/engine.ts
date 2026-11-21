import { ScenePlanner, SceneStateData, clearScene, clipFinished, playPhase, readyToCum, showsSceneRig, snapshotsEqual, thrusted, type Choice, type Clips, type Hooks, type Phase, type Snapshot } from './planner';
import { BEAT_WINDOW, DEFAULT_PACE, PaceGame, type Ending, type PaceSettings } from './pace';
import { SceneCatalogue, type Role } from './roles';

/*
 * A scene as the game runs it, step by step, over a stage that draws it. It follows `GirlSceneEntity#startScene`, `SceneDirector`, `StripScene` and `SceneAnimator`: she is
 * dressed to begin with, strips if the scene needs it (the model changes on the `becomenude` cue, as in the game), goes to the bed and lies down, idles until she is clicked,
 * and then the planner chooses her clips from the cues and the thrusting. Nothing here draws: the stage does, and tells the engine the cues and the clips that ended.
 */

export type SceneType = 'on_player' | 'on_bed' | 'stationary_contact' | 'stationary_intro' | 'stationary' | 'downed' | 'player_downed' | 'player_animation';

export interface SceneInfo {
  id: string;
  type: SceneType;
  needsToStrip: boolean;
  cumThreshold: number;
  useKeyFrameEvents: boolean;
  replaceDefault: boolean;
  bedOffset: number;
  amountOfLoops: number;
  /** `classic` is the game's: hold until the bar is full. `pace` is the prototype: keep her pace in her sweet spot. */
  minigame: 'classic' | 'pace';
  pace: PaceSettings;
  /** What the scene's author set in its `minigame` block; it wins over the feel read from the clips. */
  paceOverrides?: Partial<PaceSettings>;
}

/** What draws the scene. */
export interface Stage {
  hasClip(name: string): boolean;
  /** Plays a clip from its start: repeating, or once and held on its last frame (the stage reports the end). A name that is not a clip of hers stops the body (the rest pose). */
  playClip(name: string | null, repeat: boolean): void;
  setModel(kind: 'nude' | 'dressed'): void;
  /** Whether she has clothes to take off: false for a girl with only a nude model, who is always nude and is never undressed. */
  readonly wearsClothes: boolean;
  setRigVisible(visible: boolean): void;
  setBed(visible: boolean, offset: number): void;
}

export type Director = 'IDLE' | 'STRIPPING' | 'MOVING_TO_BED' | 'MOVING_TO_PLAYER' | 'WAITING_FOR_PLAYER' | 'IN_SCENE' | 'DONE';

export interface CueLog {
  /** Ticks since the start. */
  age: number;
  /** The marker as it fired (it may name several cues). */
  marker: string;
  phase: Phase;
  clip: string | null;
}

/** A tick is a twentieth of a second, as in the game. */
export const TICK_MS = 50;

const PROGRESS_STEP = 0.1;
const MARKER_STRIP = 'becomenude';
const STRIP_CLIP = 'animation.base.strip';
const IDLE_CLIP = 'animation.base.idle';
const NONE_CLIP = 'animation.base.null';
const WALK_TICKS = 24;

export class SceneSim {
  readonly state = new SceneStateData();
  director: Director = 'IDLE';
  stripped = false;
  age = 0;
  /** The clip that is playing and whether it repeats. */
  current: string | null = null;
  currentLoop: boolean | null = null;
  note = '';
  finished: 'complete' | 'stopped' | null = null;
  /** How the pace game ended (only once it has). */
  ending: Ending | null = null;
  /** The last time a beat was hit, in ticks, so the screen can flash it. */
  hitAt = -100;
  /** What the pace game is worth once it has ended (see `PaceGame.reward`). */
  reward = 0;
  private lastPressMs = -1e9;
  private lastBeatMs = -1e9;
  private pressUsed = true;
  private beatUsed = true;
  private misses = 0;

  /** The pace mini game (only when the scene plays that way). */
  readonly pace: PaceGame;
  private planner = new ScenePlanner();
  private cues = new Set<string>();
  private lastAsked: Snapshot | null = null;
  private stripApplied = false;
  private timer = 0;
  private startedStep = false;

  constructor(readonly info: SceneInfo, private readonly clips: SceneCatalogue, private readonly stage: Stage, private readonly onCue: (cue: CueLog) => void, private readonly onChange: () => void) {
    this.pace = new PaceGame(info.pace ?? DEFAULT_PACE);
  }

  // =================================================================
  // Starting
  // =================================================================

  start(): void {
    this.planner = new ScenePlanner();
    this.cues.clear();
    this.lastAsked = null;
    this.age = 0;
    this.stripped = !this.stage.wearsClothes;
    this.stripApplied = false;
    this.finished = null;
    this.current = null;
    this.currentLoop = null;
    this.startedStep = false;
    clearScene(this.state);
    this.state.cumThreshold = this.info.cumThreshold;
    this.state.sexAnim = null;
    this.state.thrusting = false;
    this.pace.reset();
    this.ending = null;
    this.reward = 0;
    this.misses = 0;
    this.stage.setBed(false, 0);
    this.stage.setRigVisible(false);
    this.stage.setModel(this.stage.wearsClothes ? 'dressed' : 'nude');
    this.director = 'IDLE';
    this.everyday();

    /* A girl with no clothes has nothing to take off: the scene just goes on. */
    if (this.info.needsToStrip && this.stage.wearsClothes) this.requestStrip();
    else this.afterStrip();

    this.onChange();
  }

  private everyday(): void {
    this.play(this.stage.hasClip(IDLE_CLIP) ? IDLE_CLIP : null, true);
  }

  private play(clip: string | null, repeat: boolean): void {
    this.current = clip;
    this.currentLoop = repeat;
    this.stage.playClip(clip, repeat);
  }

  // =================================================================
  // The director: stripping, the walk, the wait
  // =================================================================

  private requestStrip(): void {
    this.director = 'STRIPPING';
    this.note = 'She is undressing.';
    this.stripApplied = false;

    if (this.stage.hasClip(STRIP_CLIP)) {
      this.play(STRIP_CLIP, false);
    } else {
      this.note = 'She has no animation.base.strip clip, so she is changed at once.';
      this.swap();
      this.afterStrip();
    }
  }

  /** The swap itself, once: the model changes, as `StripScene.Run#swap`. */
  private swap(): void {
    if (this.stripApplied) return;

    this.stripApplied = true;
    this.stripped = !this.stripped;
    this.stage.setModel(this.stripped ? 'nude' : 'dressed');
  }

  private afterStrip(): void {
    switch (this.info.type) {
      case 'on_bed':
        this.director = 'MOVING_TO_BED';
        this.timer = WALK_TICKS;
        this.note = 'She walks to the bed.';
        this.everyday();
        break;
      case 'on_player':
      case 'player_animation':
      case 'downed':
      case 'player_downed':
        this.director = 'MOVING_TO_PLAYER';
        this.timer = this.info.type === 'on_player' || this.info.type === 'player_animation' ? WALK_TICKS : 1;
        this.note = 'She walks up to you.';
        this.everyday();
        break;
      case 'stationary_contact':
        this.director = 'WAITING_FOR_PLAYER';
        this.note = 'She waits for you. Click on her to start.';
        this.everyday();
        break;
      case 'stationary_intro':
        this.startSolo('STATIONARY_INTRO');
        break;
      default:
        this.startSolo('STATIONARY');
        break;
    }

    this.onChange();
  }

  private startSolo(phase: Phase): void {
    this.state.inScene = true;
    this.state.progress = 0;
    this.state.stationaryIndex = 0;
    this.state.stationaryLoop = 0;
    this.state.stationaryLoopThreshold = this.info.amountOfLoops;
    this.state.phase = phase;
    this.director = 'IN_SCENE';
    this.note = '';
    this.stage.setRigVisible(false);
    this.lastAsked = null;
  }

  private arriveAtBed(): void {
    this.stage.setBed(true, 0);
    this.state.inScene = true;
    this.state.phase = 'LAYING_DOWN';
    this.director = 'IN_SCENE';
    this.note = '';
    this.stage.setRigVisible(showsSceneRig(this.state.phase));
    this.lastAsked = null;
  }

  /** Whether she is waiting to be clicked: lying on the bed, or standing where a contact scene starts. */
  waitingForClick(): boolean {
    return (this.director === 'IN_SCENE' && this.state.phase === 'BED_IDLE') || this.director === 'WAITING_FOR_PLAYER';
  }

  /** The player clicked on her: the scene begins with the player on her (`GirlSceneEntity#startRidingScene`). */
  join(): void {
    if (!this.waitingForClick()) return;

    this.state.progress = 0;
    this.state.cumThreshold = this.info.cumThreshold;
    this.state.thrusting = false;
    this.pace.reset();
    this.state.introIndex = 0;
    this.state.sexAnim = null;
    /* One animation, the number of times the scene says (at least once): no intro, no thrusting. */
    if (this.info.type === 'player_animation') {
      this.state.stationaryLoop = 0;
      this.state.stationaryLoopThreshold = Math.max(1, this.info.amountOfLoops);
      playPhase(this.state, 'STATIONARY');
    } else {
      playPhase(this.state, 'INTRO');
    }

    this.state.inScene = true;
    this.director = 'IN_SCENE';
    this.note = '';
    this.stage.setRigVisible(true);
    this.stage.setBed(this.info.type === 'on_bed', this.info.bedOffset);
    this.lastAsked = null;
    this.onChange();
  }

  /** The bar is full: she can be finished. In the game the player does it (a button), so the scene does not end by itself. */
  canCum(): boolean {
    return this.director === 'IN_SCENE' && this.state.phase === 'HAVING_SEX' && readyToCum(this.state);
  }

  /** The player finishes (`GirlSceneEntity#tryTriggerCum`). */
  cum(): void {
    if (!this.canCum()) return;

    this.finishNow(this.info.minigame === 'pace' ? this.pace.ending(this.state.progress / Math.max(0.0001, this.state.cumThreshold)) : 'good');
  }

  private finishNow(ending: Ending): void {
    this.ending = ending;
    this.reward = this.info.minigame === 'pace' ? this.pace.reward(ending) : 1;
    playPhase(this.state, 'CUM');
    this.state.thrusting = false;
    this.pace.held = false;
    this.lastAsked = null;
    this.note = ending === 'early' ? 'You could not hold on.' : '';
    this.onChange();
  }

  /** The input (a key, a button) is held or let go. In the classic game that is the thrusting; in the pace game it speeds the pace up or lets it fall. */
  /** An analog input (a trigger or a stick) says the pace outright; 0 hands control back to the key. */
  setAnalog(value: number): void {
    if (this.info.minigame !== 'pace') return;

    const was = this.pace.analog !== null;

    if (value < 0.03) {
      this.pace.analog = null;
      return;
    }

    if (!was && value > 0.15) this.pressed();

    this.pace.analog = value;
  }

  /** Whether there is something to skip: she is still undressing, walking, lying down, waiting or in the intro. */
  canSkip(): boolean {
    if (this.director === 'DONE' || this.director === 'IDLE') return false;
    if (this.director !== 'IN_SCENE') return true;

    return this.state.phase === 'LAYING_DOWN' || this.state.phase === 'BED_IDLE' || this.state.phase === 'INTRO' || this.state.phase === 'STATIONARY_INTRO';
  }

  /** Straight to the main part: undressed if the scene undresses her, on the bed, the male there, and the sex (or the solo loop) going. For testing the part that matters. */
  skip(): void {
    if (!this.canSkip()) return;

    const single = this.info.type === 'player_animation';
    const solo = this.info.type === 'stationary' || this.info.type === 'stationary_intro' || single;

    /* She is as she would be by now: undressed if the scene undresses her. */
    if (this.info.needsToStrip && !this.stripped) {
      this.stripped = true;
      this.stripApplied = true;
      this.stage.setModel('nude');
    }

    this.state.inScene = true;
    this.state.progress = 0;
    this.state.cumThreshold = this.info.cumThreshold;
    this.state.thrusting = false;
    this.state.sexAnim = null;
    this.state.introIndex = 0;
    this.state.stationaryIndex = 0;
    this.state.stationaryLoop = 0;
    this.state.stationaryLoopThreshold = single ? Math.max(1, this.info.amountOfLoops) : this.info.amountOfLoops;
    this.pace.reset();
    this.ending = null;
    this.director = 'IN_SCENE';
    this.note = '';
    playPhase(this.state, solo ? 'STATIONARY' : 'HAVING_SEX');
    this.stage.setRigVisible(!solo || single);
    this.stage.setBed(this.info.type === 'on_bed', this.info.type === 'on_bed' ? this.info.bedOffset : 0);
    this.current = null;
    this.lastAsked = null;
    this.onChange();
  }

  setThrusting(held: boolean): void {
    if (this.info.minigame === 'pace') {
      if (held && !this.pace.held) this.pressed();

      this.pace.held = held;
    } else {
      this.state.thrusting = held;
    }
  }

  // =================================================================
  // Every tick
  // =================================================================

  tick(): void {
    this.age++;

    if (this.director === 'MOVING_TO_BED' || this.director === 'MOVING_TO_PLAYER') {
      if (--this.timer <= 0) {
        if (this.director === 'MOVING_TO_BED') this.arriveAtBed();
        else this.join2();
      }
    }

    if (this.director === 'IN_SCENE' && this.state.inScene) {
      if (this.info.minigame === 'pace') this.stepPace();

      const now = this.snapshot();

      if (!(snapshotsEqual(this.lastAsked, now) && this.cues.size === 0 && !this.planner.timeoutDue(now, this.age))) this.ask(false);
    }

    this.onChange();
  }

  /** A press of the input: on the beat when a thrust cue fired a moment ago (the cue, if it comes a moment later, finds this press). */
  private pressed(): void {
    const now = performance.now();

    this.lastPressMs = now;
    this.pressUsed = false;

    if (!this.beatUsed && now - this.lastBeatMs <= BEAT_WINDOW * 1000) this.hit();
  }

  /** A thrust cue fired: it is on the beat when the player pressed a moment ago; two cues in a row with no press end the combo. */
  private beatCue(): void {
    const now = performance.now();

    this.lastBeatMs = now;
    this.beatUsed = false;
    this.pace.cues++;

    if (!this.pressUsed && now - this.lastPressMs <= BEAT_WINDOW * 1000) {
      this.hit();
    } else if (this.state.phase === 'HAVING_SEX') {
      /* A cue that comes and goes with no press counts against the combo, but only the second one in a row. */
      setTimeout(() => {
        if (!this.beatUsed && this.misses++ >= 1) this.pace.combo = 0;
      }, BEAT_WINDOW * 1000);
    }
  }

  private hit(): void {
    this.pressUsed = true;
    this.beatUsed = true;
    this.misses = 0;
    this.pace.beat(0);
    this.hitAt = this.age;
  }

  /** The pace mini game, a tick: the pace follows the input, the clip follows the pace, and her pleasure follows how well the pace suits her. */
  private stepPace(): void {
    const sex = this.state.phase === 'HAVING_SEX';
    const gain = this.pace.step(TICK_MS / 1000, sex, (this.age * TICK_MS) / 1000, this.state.cumThreshold);

    this.state.thrusting = sex && this.pace.fast;

    if (!sex) {
      this.pace.pace = 0;
      this.pace.target = 0;
      return;
    }

    /* Once her bar is full it stays full, whatever the pace does after. */
    this.state.progress = this.state.progress >= this.state.cumThreshold ? this.state.cumThreshold : Math.min(this.state.cumThreshold, Math.max(0, this.state.progress + gain));

    /* The player cannot hold on: it ends before she is ready. */
    if (this.pace.edge >= 1) this.finishNow(this.state.progress >= this.state.cumThreshold ? 'good' : 'early');
  }

  /** The walk to the player is over: the scene starts (an on-player scene has no wait). */
  private join2(): void {
    this.director = 'WAITING_FOR_PLAYER';
    this.join();
  }

  private snapshot(): Snapshot {
    const s = this.state;

    return { phase: s.phase, introIndex: s.introIndex, stationaryIndex: s.stationaryIndex, stationaryLoop: s.stationaryLoop, stationaryLoopThreshold: s.stationaryLoopThreshold, thrusting: s.thrusting, sexAnim: s.sexAnim, useKeyFrameEvents: this.info.useKeyFrameEvents };
  }

  // =================================================================
  // Cues and clips ending, from the stage
  // =================================================================

  /** A marker of the playing clip fired. One marker can name several cues (`lipsound, thrust`). */
  cue(marker: string): void {
    const lower = marker.toLowerCase();

    this.onCue({ age: this.age, marker, phase: this.state.phase, clip: this.current });

    if (this.director === 'STRIPPING' && lower.includes(MARKER_STRIP)) this.swap();

    if (this.director !== 'IN_SCENE' || !this.state.inScene) return;

    for (const part of lower.split(',')) {
      const name = part.trim();

      if (name) this.cues.add(name);
    }

    /* In the pace game the pleasure comes from the pace, not from the thrust cues. */
    if (this.info.minigame !== 'pace') thrusted(this.state, lower, PROGRESS_STEP);
    else if (lower.includes('thrust')) this.beatCue();
  }

  /** A clip that does not repeat has played out. */
  clipEnded(): void {
    if (this.director === 'STRIPPING') {
      this.swap();
      this.afterStrip();
      return;
    }

    if (this.director !== 'IN_SCENE' || !this.state.inScene) return;

    this.startedStep = false;
    this.advanceAfterClip();

    if (!this.state.inScene) return;

    this.ask(true);
  }

  private advanceAfterClip(): void {
    const phase = this.state.phase;

    switch (phase) {
      case 'INTRO':
      case 'STATIONARY_INTRO': {
        const role: Role = phase === 'INTRO' ? 'intro' : 'solo_intro';
        const index = phase === 'INTRO' ? this.state.introIndex : this.state.stationaryIndex;

        clipFinished(this.state, index >= this.clips.of(role).length - 1, this.hooks);
        break;
      }
      case 'LAYING_DOWN':
      case 'CUM':
        clipFinished(this.state, false, this.hooks);
        break;
      case 'STATIONARY':
        clipFinished(this.state, false, this.hooks);
        /* The same clip plays again for the next loop, as a fresh start. */
        this.current = null;
        break;
      default:
        break;
    }
  }

  private readonly hooks: Hooks = {
    cumFinished: () => this.stop('complete'),
    soloFinished: () => this.stop('complete')
  };

  /** The scene is over: complete, or cut short. */
  stop(how: 'complete' | 'stopped'): void {
    clearScene(this.state);
    this.finished = how;
    this.director = 'DONE';
    const rhythm = this.info.minigame === 'pace' && this.pace.cues > 0 ? ` Rhythm ${this.pace.hits} of ${this.pace.cues}, best combo x${this.pace.best}. Worth x${this.reward}.` : '';

    this.note = how === 'stopped' ? 'The scene was stopped.' : (this.ending === 'together' ? 'You finished together.' : this.ending === 'early' ? 'You finished too soon.' : 'The scene is complete.') + rhythm;
    this.stage.setRigVisible(false);
    this.stage.setBed(false, 0);
    this.everyday();
    this.onChange();
  }

  // =================================================================
  // Asking the planner
  // =================================================================

  private clipsOf(): Clips {
    return {
      of: role => this.clips.of(role),
      orNone: role => this.clips.of(role)[0] ?? NONE_CLIP,
      startingSide: () => this.clips.startingSide()
    };
  }

  private ask(stepEnded: boolean): void {
    let plan = this.planner.plan(this.snapshot(), this.clipsOf(), this.cues, this.age, stepEnded);

    /* A phase that has no clips moves on at once, asked again a few times at most. */
    for (let i = 0; i < 3 && plan.moveTo; i++) {
      playPhase(this.state, plan.moveTo);
      plan = this.planner.plan(this.snapshot(), this.clipsOf(), this.cues, this.age, stepEnded);
    }

    this.cues.clear();

    if (plan.sexAnimChanged) this.state.sexAnim = plan.sexAnim;

    if (plan.stop) {
      this.stop('complete');
      return;
    }

    const choice: Choice | null = plan.choice;

    if (choice && (plan.restart || choice.clip !== this.current || (choice.loop === 'LOOP') !== this.currentLoop)) {
      this.play(this.stage.hasClip(choice.clip) ? choice.clip : null, choice.loop === 'LOOP');
      this.current = choice.clip;
      this.startedStep = true;
    }

    this.stage.setRigVisible(showsSceneRig(this.state.phase));
    this.lastAsked = this.snapshot();
  }

  /** For the screen: what role the playing clip has, or what she is doing. */
  describe(): string {
    if (this.director === 'DONE') return this.note;
    if (this.director !== 'IN_SCENE') return this.note;

    return this.state.phase;
  }

  get playingStep(): boolean {
    return this.startedStep;
  }
}
