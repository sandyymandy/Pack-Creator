import type { ActorSummary, AppUpdate, MigrationStatus, SplitReport, SkeletonPlan, SkeletonWritten, SceneScope, GirlLook, ScenePlan, ArmFixReport, SearchHit, SimFiles, SimGirl, BlockbenchStatus, FileVersion, GameLogView, ClipData, CueEvent, ModelData, ViewerFiles, ImportResult, Choices, Reference, Content, Diagnostic, Entry, Located, NewFile, NewProject, Opened, RecentView, Written } from './types';
import { mock } from './mock';

/** Whether the page runs inside the Pack Creator's window. In a plain browser (for developing the page) a small in-memory stand-in answers instead. */
export const inApp = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!inApp) return mock(command, args) as Promise<T>;

  const { invoke } = await import('@tauri-apps/api/core');

  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw new Error(typeof error === 'string' ? error : (error as Error).message ?? String(error));
  }
}

export const api = {
  listRecent: () => call<RecentView[]>('list_recent'),
  forgetRecent: (path: string) => call<void>('forget_recent', { path }),
  defaultProjectsDir: () => call<string>('default_projects_dir'),
  contentsOptions: () => call<{ id: string; label: string }[]>('contents_options'),
  namespaceProblem: (namespace: string) => call<string | null>('namespace_problem', { namespace }),
  createProject: (request: NewProject) => call<Opened>('create_project', { request }),
  openProject: (path: string) => call<Opened>('open_project', { path }),
  closeProject: () => call<void>('close_project'),
  listDir: (rel: string) => call<Entry[]>('list_dir', { rel }),
  listFiles: () => call<string[]>('list_files'),
  locateFile: (rel: string) => call<Located>('locate_file', { rel }),
  readFile: (rel: string) => call<Content>('read_file', { rel }),
  stamps: (rels: string[]) => call<(string | null)[]>('stamps', { rels }),
  readImage: (rel: string) => call<string>('read_image', { rel }),
  writeFile: (rel: string, textValue: string) => call<Written>('write_file', { rel, textValue }),
  /** Checks text that is not saved yet, as it is typed. */
  checkText: (rel: string, textValue: string) => call<Diagnostic[]>('check_text', { rel, textValue }),
  checkProject: () => call<Diagnostic[]>('check_project'),
  createEntry: (rel: string, isDir: boolean) => call<void>('create_entry', { rel, isDir }),
  renameEntry: (from: string, to: string) => call<void>('rename_entry', { from, to }),
  deleteEntry: (rel: string) => call<void>('delete_entry', { rel }),
  newFile: (request: NewFile) => call<string>('new_file', { request }),
  choices: () => call<Choices>('choices'),
  reference: () => call<Reference>('reference'),
  exportZip: (destination: string) => call<number>('export_zip', { destination }),
  installPack: (world: string) => call<string>('install_pack', { world }),
  savesDir: () => call<string | null>('saves_dir'),
  oldPackGirls: (path: string) => call<{ girls: string[]; namespace: string; rewritten: boolean }>('old_pack_girls', { path }),
  convertFile: (source: string, role: string, girl: string, scene: string | null, stamp: string | null) => call<ImportResult>('convert_file', { source, role, girl, scene, stamp }),
  importOldPack: (pack: string, jobs: { girl: string; profile: string | null; body_type: string; scenes: ScenePlan[] | null }[], liquid: boolean, physicsPreset: boolean, rules: { token: string; action: string; value: string | null; authority: number }[]) => call<ImportResult>('import_old_pack', { pack, jobs, liquid, physicsPreset, rules }),
  oldPackCues: (path: string, girl: string) => call<{ token: string; count: number }[]>('old_pack_cues', { path, girl }),
  oldPackLook: (path: string, girl: string, profile: string | null) => call<GirlLook>('old_pack_look', { path, girl, profile }),
  oldPackModel: (path: string, girl: string, kind: string) => call<ModelData>('old_pack_model', { path, girl, kind }),
  oldPackTexture: (path: string, girl: string) => call<string | null>('old_pack_texture', { path, girl }),
  oldPackClips: (path: string, girl: string) => call<ClipData[]>('old_pack_clips', { path, girl }),
  guessBodyType: (bones: string[]) => call<string | null>('guess_body_type', { bones }),
  detectScenes: (clips: string[]) => call<ScenePlan[]>('detect_scenes', { clips }),
  findProfiles: (folder: string, ids: string[]) => call<(string | null)[]>('find_profiles', { folder, ids }),
  viewerFiles: (rel: string) => call<ViewerFiles>('viewer_files', { rel }),
  readModel: (rel: string) => call<ModelData>('read_model', { rel }),
  skeletonPlan: (choices: Record<string, string>) => call<SkeletonPlan>('skeleton_plan', { choices }),
  skeletonApply: (choices: Record<string, string>) => call<SkeletonWritten>('skeleton_apply', { choices }),
  migrationStatus: () => call<MigrationStatus>('migration_status'),
  splitScenes: (apply: boolean) => call<SplitReport>('split_scenes', { apply }),
  undoSplitScenes: (stamp: string) => call<number>('undo_split_scenes', { stamp }),
  fixArms: (rel: string, apply: boolean) => call<ArmFixReport>('fix_arms', { rel, apply }),
  /** Puts the basic soft bones (chest, belly, cheeks, ass: those the model has, by name) in a model's physics file. */
  addBasicPhysics: (rel: string) => call<{ path: string; added: string[] }>('add_basic_physics', { rel }),
  readAnimation: (rel: string) => call<ClipData[]>('read_animation', { rel }),
  deleteClips: (rel: string, clips: string[]) => call<ClipData[]>('delete_clips', { rel, clips }),
  sceneScopes: () => call<SceneScope[]>('scene_scopes'),
  sceneMakeSkeletonWide: (scene: string, girl: string) => call<string[]>('scene_make_skeleton_wide', { scene, girl }),
  sceneMakeGirlSpecific: (scene: string, girl: string, keepOriginal: boolean) => call<string[]>('scene_make_girl_specific', { scene, girl, keepOriginal }),
  saveClipEvents: (rel: string, clip: string, events: CueEvent[]) => call<ClipData[]>('save_clip_events', { rel, clip, events }),
  cueNames: () => call<string[]>('cue_names'),
  bundledMale: () => call<ModelData>('bundled_male'),
  bundledBed: () => call<{ model: ModelData; texture: string }>('bundled_bed'),
  checkGirlModels: (bodyType: string, nude: string | null, dressed: string | null, texture: string | null) => call<unknown>('check_girl_models', { bodyType, nude, dressed, texture }),
  createGirl: (request: { id: string; name: string; archetype: string; body_type: string; tame_item: string; nude: string; dressed: string; texture: string; physics: boolean }) => call<{ girl: string; written: string[]; notes: string[] }>('create_girl', { request }),
  examplePreview: (id: string) => call<string | null>('example_preview', { id }),
  searchText: (query: string, caseSensitive: boolean) => call<SearchHit[]>('search_text', { query, caseSensitive }),
  replaceInFiles: (query: string, replacement: string, caseSensitive: boolean, files: string[]) => call<number>('replace_in_files', { query, replacement, caseSensitive, files }),
  duplicateEntry: (rel: string) => call<string>('duplicate_entry', { rel }),
  importFiles: (folder: string, files: { rel: string; data: string }[]) => call<string[]>('import_files', { folder, files }),
  copyInto: (rel: string, folder: string) => call<string>('copy_into', { rel, folder }),
  copyToClipboard: (rels: string[], cut: boolean) => call<void>('copy_to_clipboard', { rels, cut }),
  revealInExplorer: (rel: string) => call<void>('reveal_in_explorer', { rel }),
  simGirls: () => call<SimGirl[]>('sim_girls'),
  simFiles: (sceneId: string, girlId: string, example = false, bodyType: string | null = null) => call<SimFiles>('sim_files', { sceneId, girlId, example, bodyType }),
  exampleGirls: () => call<SimGirl[]>('example_girls'),
  exampleScenes: () => call<{ id: string; file: string }[]>('example_scenes'),
  readAsset: (rel: string) => call<string>('read_asset', { rel }),
  fileHistory: (rel: string) => call<FileVersion[]>('file_history', { rel }),
  readVersion: (rel: string, id: number) => call<string>('read_version', { rel, id }),
  gameLogDefault: () => call<string | null>('game_log_default'),
  readGameLog: (path: string) => call<GameLogView>('read_game_log', { path }),
  blockbenchStatus: () => call<BlockbenchStatus>('blockbench_status'),
  installBlockbench: () => call<void>('install_blockbench'),
  prepareBlockbench: () => call<void>('prepare_blockbench'),
  checkAppUpdate: () => call<AppUpdate>('check_app_update'),
  downloadAppUpdate: (url: string, name: string) => call<string>('download_app_update', { url, name }),
  openReleasePage: (url: string) => call<void>('open_release_page', { url }),
  registerActors: (girls: string[], withPack: boolean) => call<ActorSummary>('register_actors', { girls, withPack }),
  updateToolkit: () => call<string>('update_toolkit'),
  openInBlockbench: (model: string, texture: string | null, animation: string | null) => call<void>('open_in_blockbench', { model, texture, animation }),
  importProfile: (path: string, bodyType: string) => call<ImportResult>('import_profile', { path, bodyType }),
  schemaFor: (rel: string) => call<object | null>('schema_for', { rel })
};

/** Asks the user for a folder. */
export async function pickFolder(title: string, defaultPath?: string): Promise<string | null> {
  if (!inApp) return window.prompt(title, defaultPath ?? '') || null;

  const { open } = await import('@tauri-apps/plugin-dialog');
  const chosen = await open({ directory: true, multiple: false, title, defaultPath });

  return typeof chosen === 'string' ? chosen : null;
}

/** Asks the user for a file. */
export async function pickFile(title: string, extensions: string[]): Promise<string | null> {
  if (!inApp) return window.prompt(title, '') || null;

  const { open } = await import('@tauri-apps/plugin-dialog');
  const chosen = await open({ multiple: false, title, filters: [{ name: 'Files', extensions }] });

  return typeof chosen === 'string' ? chosen : null;
}

/** Asks where to save a file. */
export async function pickSavePath(title: string, defaultPath: string): Promise<string | null> {
  if (!inApp) return window.prompt(title, defaultPath) || null;

  const { save } = await import('@tauri-apps/plugin-dialog');

  return (await save({ title, defaultPath, filters: [{ name: 'Zip', extensions: ['zip'] }] })) ?? null;
}
