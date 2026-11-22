/** The shapes the Rust core sends. They mirror pc_core: keep the two in step. */

export type Severity = 'error' | 'warning' | 'info';

export interface QuickFix {
  title: string;
  replacement: string;
}

export interface Diagnostic {
  severity: Severity;
  code: string;
  message: string;
  file: string;
  line: number;
  column: number;
  end_line: number;
  end_column: number;
  fix: QuickFix | null;
}

export type FileKind =
  | 'pack_meta' | 'girl' | 'scene' | 'trait' | 'girl_ai' | 'dialogue' | 'social' | 'loot_table' | 'other_data'
  | 'model' | 'animation' | 'texture' | 'sound' | 'armour_wearer' | 'armour_set' | 'armour_look' | 'sounds_json' | 'lang' | 'event_bindings' | 'keyframe_events'
  | 'physics' | 'other_asset' | 'other';

export interface Entry {
  name: string;
  rel: string;
  is_dir: boolean;
  kind: FileKind;
  size: number;
}

export type Content =
  | { type: 'text'; text: string }
  | { type: 'binary'; size: number }
  | { type: 'too_large'; size: number };

export interface Located {
  kind: FileKind;
  namespace: string | null;
  id: string | null;
}

export interface ProjectMeta {
  format: number;
  name: string;
  namespace: string;
  author: string;
  description: string;
  contents: string[];
  created: number;
}

export interface Opened {
  root: string;
  meta: ProjectMeta;
}

export interface RecentView {
  path: string;
  name: string;
  last_opened: number;
  exists: boolean;
}

export interface NewProject {
  name: string;
  parent: string;
  namespace: string;
  author: string;
  description: string;
  contents: string[];
}

export interface Written {
  located: Located;
  diagnostics: Diagnostic[];
  stamp: string | null;
}

export interface Choices {
  archetypes: string[];
  body_types: string[];
  scene_types: string[];
  trait_categories: string[];
}

export interface NewFile {
  kind: FileKind;
  id: string;
  name?: string;
  archetype?: string;
  body_type?: string;
  tame_item?: string;
  option?: string;
}

/** The game's reference (what pc_core serialises): used by the guided forms. */
export interface RField {
  name: string;
  kind: string;
  required: boolean;
  description: string;
  min: number | null;
  max: number | null;
  options: string[];
  default: unknown;
  of: string | null;
}

export interface RComponent {
  id: string;
  description: string;
  required: boolean;
  fields: RField[];
}

export interface RTrait {
  id: string;
  category: string;
  description: string;
  params: RField[];
  stacks: string[];
}

export interface RCategory {
  name: string;
  description: string;
  several: boolean;
}

export interface Reference {
  components: Record<string, RComponent>;
  archetypes: Record<string, string[]>;
  traits: Record<string, RTrait>;
  categories: Record<string, RCategory>;
  scene_fields: RField[];
  goals: Record<string, RField[]>;
  goal_conditions: string[];
  capabilities: Record<string, RField>;
  taste_tags: string[];
  dialogue_topics: string[];
  dialogue_ids: string[];
  known_cues?: string[];
  sound_events?: string[];
}

export interface ImportNote {
  level: 'kept' | 'changed' | 'dropped' | 'review' | 'error';
  message: string;
}

export interface ImportResult {
  girl: string;
  written: string[];
  notes: ImportNote[];
}

/** What the Rust readers send for the viewer (pc_core's strut_model and strut_anim). */
export interface ModelTexture {
  name: string;
  width: number;
  height: number;
  uvWidth: number;
  uvHeight: number;
}

export interface ArmFixReport {
  fixed: { bone: string; oldRotation: number; cubes: { name: string; oldRotation: number; newRotation: number }[]; children: string[] }[];
  skipped: { bone: string; reason: string }[];
  removed: string[];
}

export interface ModelData {
  version: number;
  unitsPerBlock: number;
  boundsMin: [number, number, number];
  boundsMax: [number, number, number];
  textures: ModelTexture[];
  groupBones: { name: string; parentIndex: number; pivot: number[]; rotation: number[]; elements: { kind: number; index: number }[] }[];
  meshes: { name: string; textureIndex: number; origin: number[]; rotation: number[]; positions: number[]; faces: { vertices: number[]; uvs: number[] }[]; smooth: boolean; armatureIndex: number; skin: number[][][] }[];
  cubes: {
    name: string; textureIndex: number; origin: number[]; rotation: number[]; from: number[]; to: number[]; inflate: number; mirror: boolean; boxUv: boolean;
    faces: { faceId: number; u1: number; v1: number; u2: number; v2: number; uvRotation: number }[];
  }[];
  armatureCount: number;
  armatures: { name: string; parentGroupBone: number; bones: { name: string; parentIndex: number; head: number[]; rotation: number[]; length: number }[] }[];
  extensions: string[];
}

export interface Value3 {
  literals: [number, number, number];
  expressions: [string | null, string | null, string | null];
}

export interface KeyData {
  time: number;
  interpolation: number;
  value: Value3;
  post: Value3 | null;
  easing: string | null;
  easingArgs: number[];
  /** The handles of a bezier key, per axis: left time, left value, right time, right value (three numbers each). */
  bezier?: number[] | null;
}

export interface ClipData {
  name: string;
  duration: number;
  loopMode: number;
  channels: { target: string; kind: string; property: string; keys: KeyData[] }[];
  events: CueEvent[];
}

export interface CueEvent {
  time: number;
  kind: string;
  payload: string;
  authority: number;
}

export interface SearchHit {
  rel: string;
  line: number;
  column: number;
  preview: string;
  start: number;
  length: number;
}

export interface SimGirl {
  id: string;
  file: string;
  body_type: string | null;
  /** One of the game's own base girls, shipped as an example. */
  example: boolean;
  has_model: boolean;
}

export interface SimFiles {
  nude: string | null;
  dressed: string | null;
  texture: string | null;
  animations: string[];
  /** Her event bindings in layers (the skeleton's, then hers). */
  binding_layers: string[];
  /** The male's clip files, for a scene in the new layout. */
  male_animations: string[];
  bindings: string | null;
  sounds_json: string[];
}

export interface FileVersion {
  id: number;
  size: number;
}

export interface GameLogEntry {
  time: string;
  level: string;
  source: string;
  message: string;
  resource: string | null;
  count: number;
  file: string | null;
}

export interface GameLogView {
  path: string;
  ageSeconds: number | null;
  entries: GameLogEntry[];
  offer: { time: string; files: number; bytes: number } | null;
  packFiles: number;
}

export interface BlockbenchStatus {
  exe: string | null;
  version: string | null;
  versionOk: boolean;
  running: boolean;
  toolkitVersion: string;
  installing: string;
  /** Whether a Blockbench that is open can be reached without restarting it. */
  linked: boolean;
}

export interface ViewerFiles {
  id: string;
  models: string[];
  texture: string | null;
  animations: string[];
}

/** One scene as the person sees it in the review: its name and the clips (by the short names the pack writes) that play each part of it. */
export interface ScenePlan {
  name: string;
  parts: Record<string, string[]>;
  /** Where and how the scene plays (a scene file's type). Empty keeps what her profile says. */
  scene_type: string;
  /** How far the bed is moved, for a scene on a bed (null keeps her profile's). */
  bed_offset?: number | null;
}

/** What looking at one old girl found. */
export interface GirlLook {
  id: string;
  models: string[];
  texture: boolean;
  clips: string[];
  scenes: ScenePlan[];
  /** Where the scenes came from: her profile, the names of her clips, or nowhere. */
  scenes_from: 'profile' | 'names' | 'none';
  /** Her animations are already one file for each scene, imported as they are: the scenes are shown, not editable. */
  files_layout: boolean;
  bones: string[];
}

/** A scene, who its file says it is for, and its animation files (girl, skeleton or shared). */
export interface SceneScope {
  id: string;
  file: string;
  supported_girls: string[];
  body_types: string[];
  files: { rel: string; character_kind: 'girl' | 'skeleton' | 'shared'; character: string }[];
}

export interface SkeletonAssignment { rel: string; kind: 'model' | 'animation'; character: string; id: string; current: string | null }
export interface SkeletonGroup { id: string; models: string[]; animations: number }
export interface SkeletonPlan { assignments: SkeletonAssignment[]; groups: SkeletonGroup[]; unreadable: string[] }
export interface SkeletonWritten { written: number; already: number; failed: string[] }

export interface SplitReport { applied: boolean; stamp: string | null; scenes: number; files_written: number; files_removed: number; models_stripped: number; dropped_bones: [string, number][]; approximate: [string, number][]; skipped: string[]; findings: string[] }
export interface MigrationStatus { format: number; current: number; needed: boolean; reason: string | null; steps: { to: number; title: string; detail: string }[]; newer: boolean }
export interface ActorSummary { base: number; pack: number; folders: string[]; announced: boolean }
export interface AppUpdate { current: string; latest: string; newer: boolean; url: string; notes: string; asset: string | null; asset_name: string | null }
