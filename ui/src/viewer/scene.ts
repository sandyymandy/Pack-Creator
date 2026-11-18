/**
 * The 3D scene (three.js): draws a model in the authoring space the converter writes (X is already mirrored, the camera looks from behind it), with the bones turned by
 * the clip being played. Cubes and free-form meshes are drawn; skinning is not applied, so a skinned mesh shows in its rest shape.
 */
import * as THREE from 'three';
import { evaluate } from './molang';
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js';
import type { ClipData, KeyData, ModelData, ModelTexture, Value3 } from '../types';
import { bezierPoint, ease, splinePoint } from './easing';
import { entityAt, SpringSim, type BoneFrame, type Motion, type SpringParams, type Vec3 } from './physics';

const FACE_NAMES = ['west', 'east', 'north', 'south', 'up', 'down'];
const RAD = Math.PI / 180;
/** Bones that are the scene rig (the partner's body, items, armour, cameras) and not part of the girl. */
const RIG = /^(steve|items|itemrenderer|weapon|armor.*|girlcam|boycam|boobwindow)$/i;
/** Names a girl's own bones can have too (an old model's `chest` holds her head and arms): they are the partner's only at the top of the model, as `steve`'s children are. */
const RIG_ROOT = /^(torso2|rightleg|leftleg|chest)$/i;
const isRigName = (name: string, root: boolean) => RIG.test(name) || (root && RIG_ROOT.test(name));

type Corner = [number, number, number];

/** The four corners of a face as seen from outside the cube: top left, top right, bottom right, bottom left of its image. */
function faceCorners(name: string, x0: number, y0: number, z0: number, x1: number, y1: number, z1: number): Corner[] {
  switch (name) {
    case 'north': return [[x1, y1, z0], [x0, y1, z0], [x0, y0, z0], [x1, y0, z0]];
    case 'south': return [[x0, y1, z1], [x1, y1, z1], [x1, y0, z1], [x0, y0, z1]];
    case 'east': return [[x1, y1, z1], [x1, y1, z0], [x1, y0, z0], [x1, y0, z1]];
    case 'west': return [[x0, y1, z0], [x0, y1, z1], [x0, y0, z1], [x0, y0, z0]];
    case 'up': return [[x0, y1, z0], [x1, y1, z0], [x1, y1, z1], [x0, y1, z1]];
    default: return [[x0, y0, z1], [x1, y0, z1], [x1, y0, z0], [x0, y0, z0]];
  }
}

/** The grid a model's UVs are measured on: its own when it has one (a 4096 pixel skin can be laid out on a 64 grid), else the texture's size. */
function uvGrid(texture: ModelTexture): [number, number] {
  return [texture.uvWidth || texture.width || 64, texture.uvHeight || texture.height || 64];
}

function geometryOf(positions: number[], uvs: number[], index: number[]): THREE.BufferGeometry {
  const geometry = new THREE.BufferGeometry();

  geometry.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3));
  geometry.setAttribute('uv', new THREE.Float32BufferAttribute(uvs, 2));
  geometry.setIndex(index);
  /* Every face has its own corners, so the normals are flat, as the game's are. */
  geometry.computeVertexNormals();
  return geometry;
}

/** A model with nothing animated, drawn as it stands: the bed. Bones whose name matches `hide` (and what is under them) are left out. */
function buildStatic(model: ModelData, textureUrl: string, hide: RegExp): THREE.Group {
  const unit = model.unitsPerBlock || 16;
  const texture = model.textures[0] ?? { name: '', width: 64, height: 64, uvWidth: 64, uvHeight: 64 };
  const map = textureUrl ? new THREE.TextureLoader().load(textureUrl) : null;

  if (map) {
    map.colorSpace = THREE.SRGBColorSpace;
    map.magFilter = THREE.NearestFilter;
    map.minFilter = THREE.NearestFilter;
  }

  const material = new THREE.MeshLambertMaterial({ map, side: THREE.DoubleSide, transparent: true, alphaTest: 0.02 });
  const root = new THREE.Group();
  const nodes: THREE.Group[] = [];
  const hidden: boolean[] = [];

  model.groupBones.forEach(bone => {
    const node = new THREE.Group();
    const parent = bone.parentIndex >= 0 ? model.groupBones[bone.parentIndex] : null;

    node.position.set(...([0, 1, 2].map(i => (bone.pivot[i] - (parent ? parent.pivot[i] : 0)) / unit) as [number, number, number]));
    setEuler(node, bone.rotation);
    nodes.push(node);
    hidden.push(hide.test(bone.name) || (bone.parentIndex >= 0 && hidden[bone.parentIndex] === true));
    node.visible = !hidden[hidden.length - 1];
    (bone.parentIndex >= 0 && nodes[bone.parentIndex] ? nodes[bone.parentIndex] : root).add(node);

    for (const element of bone.elements) {
      if (element.kind !== 1 || !model.cubes[element.index]) continue;

      const cube = model.cubes[element.index];
      const piece = new THREE.Mesh(buildCube(cube, texture, unit), material);

      piece.position.set((cube.origin[0] - bone.pivot[0]) / unit, (cube.origin[1] - bone.pivot[1]) / unit, (cube.origin[2] - bone.pivot[2]) / unit);
      setEuler(piece, cube.rotation);
      node.add(piece);
    }
  });

  return root;
}

/** How the model is drawn: lit by a light, flat (the texture as it is), its normals, or as wires. */
export type Shading = 'lit' | 'flat' | 'normals' | 'wire';

/** Whether a mesh blends its normals across shared vertices: as each mesh says it ("shade smooth" in Blockbench), or all of them smooth, or all flat. */
export type Smoothing = 'authored' | 'smooth' | 'flat';

/**
 * The normals of a mesh, one for each corner in the order the geometry lists them. Smooth: the faces that share a vertex (by its index, not its place in the UV
 * layout) are averaged there, each weighted by its size. Flat: each face keeps its own.
 */
/** What a mesh's normals are worked out from, which does not change while it is posed: the vertex of each corner and the triangles made of corners. Kept so that a skinned
 *  mesh, whose normals are worked out again every frame, allocates nothing. */
interface NormalPlan {
  corners: Int32Array;
  triangles: Int32Array;
  vertices: number;
  sums: Float32Array;
}

const normalPlans = new WeakMap<object, NormalPlan>();

function normalPlanOf(mesh: ModelData['meshes'][number]): NormalPlan {
  let plan = normalPlans.get(mesh);

  if (plan) return plan;

  const corners: number[] = [];
  const triangles: number[] = [];
  let vertices = 0;

  for (const face of mesh.faces) {
    const first = corners.length;

    for (const v of face.vertices) {
      corners.push(v);
      if (v + 1 > vertices) vertices = v + 1;
    }

    for (let i = 1; i < face.vertices.length - 1; i++) triangles.push(first, first + i, first + i + 1);
  }

  plan = { corners: Int32Array.from(corners), triangles: Int32Array.from(triangles), vertices, sums: new Float32Array(vertices * 3) };
  normalPlans.set(mesh, plan);

  return plan;
}

/**
 * The normals of a mesh, one for each corner in the order the geometry lists them. Smooth: the faces that share a vertex (by its index, not its place in the UV
 * map) share one normal, as the game blends them. Flat: each corner takes the normal of its face. Written into `out` (made when it is not given).
 */
function meshNormals(mesh: ModelData['meshes'][number], positions: ArrayLike<number>, smooth: boolean, out?: Float32Array): Float32Array {
  const plan = normalPlanOf(mesh);
  const { corners, triangles, sums } = plan;
  const result = out && out.length === corners.length * 3 ? out : new Float32Array(corners.length * 3);

  if (smooth) sums.fill(0);
  else result.fill(0);

  for (let t = 0; t < triangles.length; t += 3) {
    const a = corners[triangles[t]] * 3;
    const b = corners[triangles[t + 1]] * 3;
    const c = corners[triangles[t + 2]] * 3;
    const ux = positions[b] - positions[a];
    const uy = positions[b + 1] - positions[a + 1];
    const uz = positions[b + 2] - positions[a + 2];
    const vx = positions[c] - positions[a];
    const vy = positions[c + 1] - positions[a + 1];
    const vz = positions[c + 2] - positions[a + 2];
    const nx = uy * vz - uz * vy;
    const ny = uz * vx - ux * vz;
    const nz = ux * vy - uy * vx;

    if (smooth) {
      for (const at of [a, b, c]) {
        sums[at] += nx;
        sums[at + 1] += ny;
        sums[at + 2] += nz;
      }
    } else {
      for (let k = 0; k < 3; k++) {
        const o = triangles[t + k] * 3;

        result[o] = nx;
        result[o + 1] = ny;
        result[o + 2] = nz;
      }
    }
  }

  for (let corner = 0; corner < corners.length; corner++) {
    const o = corner * 3;
    let x: number;
    let y: number;
    let z: number;

    if (smooth) {
      const v = corners[corner] * 3;

      x = sums[v];
      y = sums[v + 1];
      z = sums[v + 2];
    } else {
      x = result[o];
      y = result[o + 1];
      z = result[o + 2];
    }

    const length = Math.hypot(x, y, z) || 1;

    result[o] = x / length;
    result[o + 1] = y / length;
    result[o + 2] = z / length;
  }

  return result;
}

function buildCube(cube: ModelData['cubes'][number], texture: ModelTexture, unit: number): THREE.BufferGeometry {
  const [uw, uh] = uvGrid(texture);
  const lo = cube.from.map((v, i) => Math.min(v, cube.to[i]) - cube.inflate);
  const hi = cube.from.map((v, i) => Math.max(v, cube.to[i]) + cube.inflate);
  const positions: number[] = [];
  const uvs: number[] = [];
  const index: number[] = [];

  for (const face of cube.faces) {
    const corners = faceCorners(FACE_NAMES[face.faceId] ?? 'down', lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]);
    let uv: [number, number][] = [[face.u1, face.v1], [face.u2, face.v1], [face.u2, face.v2], [face.u1, face.v2]];

    /* In Blockbench and the game the top face's image has its bottom edge on the north side and the bottom face's has its top edge there (the cross layout of a box UV puts the
       top above the north face), with the image's left to right still running west to east. The corners above are listed north-first for both, so their rows are swapped. */
    if (face.faceId === 4 || face.faceId === 5) uv = [uv[3], uv[2], uv[1], uv[0]];

    if (cube.mirror) uv = [uv[1], uv[0], uv[3], uv[2]];

    for (let r = 0; r < Math.max(0, face.uvRotation); r++) uv = [uv[3], uv[0], uv[1], uv[2]];

    const base = positions.length / 3;

    for (let i = 0; i < 4; i++) {
      positions.push((corners[i][0] - cube.origin[0]) / unit, (corners[i][1] - cube.origin[1]) / unit, (corners[i][2] - cube.origin[2]) / unit);
      uvs.push(uv[i][0] / uw, 1 - uv[i][1] / uh);
    }

    index.push(base, base + 1, base + 2, base, base + 2, base + 3);
  }

  return geometryOf(positions, uvs, index);
}

function buildMesh(mesh: ModelData['meshes'][number], texture: ModelTexture, positionsByVertex: ArrayLike<number>): THREE.BufferGeometry {
  const [uw, uh] = uvGrid(texture);
  const positions: number[] = [];
  const uvs: number[] = [];
  const index: number[] = [];

  for (const face of mesh.faces) {
    const base = positions.length / 3;

    face.vertices.forEach((v, i) => {
      positions.push(positionsByVertex[v * 3], positionsByVertex[v * 3 + 1], positionsByVertex[v * 3 + 2]);
      uvs.push(face.uvs[i * 2] / uw, 1 - face.uvs[i * 2 + 1] / uh);
    });

    for (let i = 1; i < face.vertices.length - 1; i++) index.push(base, base + i, base + i + 1);
  }

  return geometryOf(positions, uvs, index);
}

const eulerMatrix = (rotation: number[]): THREE.Matrix4 => new THREE.Matrix4().makeRotationFromEuler(new THREE.Euler(rotation[0] * RAD, rotation[1] * RAD, rotation[2] * RAD, 'ZYX'));

/**
 * A skinned mesh's vertices in model space (blocks), as the game bakes them: the frame of its armature (the pivot of the group it hangs off), plus the mesh's own origin,
 * plus its own rotation turning the vertices. The armature's bones then move them.
 */
function skinnedRest(mesh: ModelData['meshes'][number], frame: number[], unit: number): Float32Array {
  const matrix = eulerMatrix(mesh.rotation).setPosition((frame[0] + mesh.origin[0]) / unit, (frame[1] + mesh.origin[1]) / unit, (frame[2] + mesh.origin[2]) / unit);
  const out = new Float32Array(mesh.positions.length);
  const point = new THREE.Vector3();

  for (let v = 0; v < mesh.positions.length / 3; v++) {
    point.set(mesh.positions[v * 3] / unit, mesh.positions[v * 3 + 1] / unit, mesh.positions[v * 3 + 2] / unit).applyMatrix4(matrix);
    out.set([point.x, point.y, point.z], v * 3);
  }

  return out;
}

interface ArmatureRuntime {
  names: string[];
  parents: number[];
  bindLocal: THREE.Matrix4[];
  inverseBind: THREE.Matrix4[];
  /** The matrix of each bone that moves a vertex from its rest place to where the animation puts it. */
  skin: THREE.Matrix4[];
}

function buildArmature(armature: ModelData['armatures'][number], model: ModelData, unit: number): ArmatureRuntime {
  const pivot = armature.parentGroupBone >= 0 ? model.groupBones[armature.parentGroupBone]?.pivot ?? [0, 0, 0] : [0, 0, 0];
  const frame = new THREE.Matrix4().makeTranslation(pivot[0] / unit, pivot[1] / unit, pivot[2] / unit);
  const runtime: ArmatureRuntime = { names: [], parents: [], bindLocal: [], inverseBind: [], skin: [] };
  const bindModel: THREE.Matrix4[] = [];

  armature.bones.forEach((bone, i) => {
    const local = new THREE.Matrix4().makeTranslation(bone.head[0] / unit, bone.head[1] / unit, bone.head[2] / unit).multiply(eulerMatrix(bone.rotation));
    const bindLocal = bone.parentIndex < 0 ? frame.clone().multiply(local) : local;

    bindModel[i] = bone.parentIndex < 0 ? bindLocal.clone() : bindModel[bone.parentIndex].clone().multiply(bindLocal);
    runtime.names.push(bone.name);
    runtime.parents.push(bone.parentIndex);
    runtime.bindLocal.push(bindLocal);
    runtime.inverseBind.push(bindModel[i].clone().invert());
    runtime.skin.push(new THREE.Matrix4());
  });

  return runtime;
}

type Channels = Partial<Record<'ROTATION' | 'POSITION' | 'SCALE', KeyData[]>>;

/** Where each bone of an armature is now: its rest transform, then the animated offset within it (position, then rotation Z, Y, X, then scale), as the game does. */
function poseArmature(runtime: ArmatureRuntime, channels: Map<string, Channels>, time: number): void {
  const pose: THREE.Matrix4[] = [];

  runtime.names.forEach((name, i) => {
    const matrix = runtime.parents[i] < 0 ? runtime.bindLocal[i].clone() : pose[runtime.parents[i]].clone().multiply(runtime.bindLocal[i]);
    const own = channels.get(name);

    if (own) {
      const position = own.POSITION ? sample(own.POSITION, time, [0, 0, 0]) : [0, 0, 0];
      const rotation = own.ROTATION ? sample(own.ROTATION, time, [0, 0, 0]) : [0, 0, 0];
      const scale = own.SCALE ? sample(own.SCALE, time, [1, 1, 1]) : [1, 1, 1];
      const delta = new THREE.Matrix4().makeTranslation(position[0] / 16, position[1] / 16, position[2] / 16).multiply(eulerMatrix(rotation)).multiply(new THREE.Matrix4().makeScale(scale[0], scale[1], scale[2]));

      matrix.multiply(delta);
    }

    pose[i] = matrix;
    runtime.skin[i].copy(matrix).multiply(runtime.inverseBind[i]);
  });
}

function setEuler(object: THREE.Object3D, rotation: number[]): void {
  object.rotation.order = 'ZYX';
  object.rotation.set(rotation[0] * RAD, rotation[1] * RAD, rotation[2] * RAD);
}

/** A value of three components: each a number, or a Molang expression worked out at this moment of the clip. */
function valueOf(raw: Value3 | null, time: number): [number, number, number] {
  if (!raw) return [0, 0, 0];

  const context = { animTime: time, lifeTime: time };

  return [0, 1, 2].map(i => (raw.expressions[i] !== null ? evaluate(raw.expressions[i] as string, context) : raw.literals[i])) as [number, number, number];
}

function sample(keys: KeyData[], time: number, fallback: [number, number, number]): [number, number, number] {
  if (keys.length === 0) return fallback;
  if (time <= keys[0].time) return valueOf(keys[0].value, time);

  /* The first key at or after this moment, found by halving (a long clip has many keys, and every bone is sampled every frame). */
  let low = 1;
  let high = keys.length;

  while (low < high) {
    const middle = (low + high) >> 1;

    if (time <= keys[middle].time) high = middle;
    else low = middle + 1;
  }

  for (let i = low; i < keys.length; i++) {
    if (time <= keys[i].time) {
      const a = keys[i - 1];
      const b = keys[i];
      const from = valueOf(a.post ?? a.value, time);
      const to = valueOf(b.value, time);

      if (b.time === a.time) return from;

      const along = (time - a.time) / (b.time - a.time);

      /* What shapes the segment is the easing the key it arrives at names; with none named, its interpolation decides (step, linear, catmull-rom, bezier). A key that holds a different
         value on each side of its time gives its easing to the instant between them, so the segment arriving at it is linear (Strut does the same). */
      const name = b.post ? 'linear' : b.easing ?? ['step', 'linear', 'catmullrom', 'bezier'][b.interpolation] ?? 'linear';

      if (name === 'catmullrom') {
        /* A spline through this segment's two ends, with the keys either side of them as its guides. */
        const before = i < 2 ? valueOf(a.value, time) : valueOf(keys[i - 2].value, time);
        const after = i + 1 >= keys.length ? to : valueOf(keys[i + 1].value, time);

        return [0, 1, 2].map(j => splinePoint(along, before[j], from[j], to[j], after[j])) as [number, number, number];
      }

      if (name === 'bezier' && b.bezier && b.bezier.length >= 12) {
        const length = b.time - a.time;
        const left = b.bezier;
        const right = a.bezier;

        return [0, 1, 2].map(j => bezierPoint(time - a.time, length, from[j], to[j], right ? right[6 + j] : 0, right ? right[9 + j] : 0, left[j], left[3 + j])) as [number, number, number];
      }

      const t = ease(name, along, b.easingArgs);

      return [0, 1, 2].map(j => from[j] + (to[j] - from[j]) * t) as [number, number, number];
    }
  }

  return valueOf(keys[keys.length - 1].value, time);
}

interface Bone {
  name: string;
  node: THREE.Group;
  /** Whether this bone itself is part of the scene rig (its children follow it). */
  rigSelf: boolean;
  rest: { rotation: number[]; offset: number[] };
}

export interface Viewer {
  loadModel(model: ModelData, textureUrl: string | null, keepCamera?: boolean): void;
  setClip(clip: ClipData | null): void;
  play(): void;
  pause(): void;
  isPlaying(): boolean;
  seek(time: number): void;
  setSpeed(speed: number): void;
  setLoop(loop: boolean): void;
  /** `frame` says whether the camera is put back to show everything (the viewer does, the simulator keeps the camera where the person put it). */
  setRigHidden(hidden: boolean, frame?: boolean): void;
  setShading(shading: Shading): void;
  /** 0.2 to 2: how bright the model is (the light's strength when lit, the texture's colour when flat). */
  setBrightness(brightness: number): void;
  setSmoothing(smoothing: Smoothing): void;
  setGrid(visible: boolean): void;
  onTick(callback: (time: number, playing: boolean) => void): void;
  /** Plays a clip from its start: repeating, or once and then held on its last frame (the end is reported). Used by the scene simulator. */
  playClip(clip: ClipData | null, loop: boolean): void;
  /** Hears each event of the playing clip as its time passes. */
  onEvent(callback: ((event: { time: number; kind: string; payload: string }) => void) | null): void;
  /** Hears that a clip that does not repeat has played out. */
  onClipEnd(callback: (() => void) | null): void;
  /** Hears a click on the model: on the girl, or on the scene rig (her partner). */
  onPick(callback: ((target: 'girl' | 'rig' | 'none') => void) | null): void;
  /** The skin the scene rig is drawn with (the player's), or null to draw it with the model's texture. */
  setRigSkin(url: string | null): void;
  /** The game's bed under the girl, or none: its model and texture, and how many blocks in front of her the middle of its foot block is. */
  setBed(bed: { model: ModelData; texture: string; forward: number } | null): void;
  getTime(): number;
  /** Where the camera is and what it looks at, so a second view can be kept looking at the same place. */
  getView(): { position: [number, number, number]; target: [number, number, number] };
  setView(view: { position: [number, number, number]; target: [number, number, number] }): void;
  /** The soft bones to simulate (null for none). Kept across model loads. */
  setPhysics(bones: Map<string, SpringParams> | null): void;
  /** Hears, each time the soft bones are matched to a model, every bone name the model has and which of the soft bones' names it does not. */
  onPhysicsBound(callback: ((bound: { names: string[]; missing: string[] }) => void) | null): void;
  /** Whether the soft bones run, how the model is moved to wake them, and a steady wind in blocks per second squared. */
  setPhysicsRun(run: { on: boolean; motion: Motion; wind: number }): void;
  /** Gives the model a quick shove forward and back. */
  bumpPhysics(): void;
  /** Puts the soft bones back to rest. */
  resetPhysics(): void;
  resize(): void;
  dispose(): void;
}

export function createViewer(container: HTMLElement): Viewer {
  const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });

  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
  container.append(renderer.domElement);

  const scene = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(35, 1, 0.05, 100);
  const controls = new OrbitControls(camera, renderer.domElement);
  const grid = new THREE.GridHelper(4, 16, 0x6b7280, 0x3a3c42);

  scene.add(grid);
  camera.position.set(0, 1.2, -4);
  controls.target.set(0, 1, 0);
  controls.update();

  let root: THREE.Group | null = null;
  let bones: Bone[] = [];
  let clip: ClipData | null = null;
  let channelsByBone = new Map<string, Partial<Record<'ROTATION' | 'POSITION' | 'SCALE', KeyData[]>>>();
  let playing = false;
  let time = 0;
  let last = performance.now();
  let speed = 1;
  let loop = true;
  let disposed = false;
  let tick: ((time: number, playing: boolean) => void) | null = null;
  let rigHidden = true;
  let eventListener: ((event: { time: number; kind: string; payload: string }) => void) | null = null;
  let endListener: (() => void) | null = null;
  let pickListener: ((target: 'girl' | 'rig' | 'none') => void) | null = null;
  /** The time up to which the events of the playing clip have been heard. */
  let heardUntil = -1;
  const textureCache = new Map<string, THREE.Texture>();
  let rigMap: THREE.Texture | null = null;
  let bedGroup: THREE.Group | null = null;
  let shading: Shading = 'lit';
  let brightness = 1;
  const pieces: THREE.Mesh[] = [];
  /** The free-form meshes, which are the ones with normals to blend. */
  interface FreeMesh {
    piece: THREE.Mesh;
    data: ModelData['meshes'][number];
    /** The vertices in the bone's own frame, in blocks: where they are drawn now. */
    positions: Float32Array;
    /** For a skinned mesh: where they rest in model space, the pivot of the bone it sits under, and the armature that moves them. */
    rest?: Float32Array;
    pivot?: number[];
    armature?: number;
    /** The vertex each corner of the geometry is. */
    corners: number[];
  }

  const freeMeshes: FreeMesh[] = [];
  let armatures: ArmatureRuntime[] = [];
  let armatureChannels = new Map<string, Channels>();
  let smoothing: Smoothing = 'authored';

  /* Soft bones: the physics file's bones, the simulation built from them for the loaded model, and how it is being run. */
  let physicsParams: Map<string, SpringParams> | null = null;
  let sim: SpringSim | null = null;
  let physicsOn = false;
  let motion: Motion = 'walk';
  let wind = 0;
  let unitsPerBlock = 16;
  /** When the last bump began, on the simulation's clock; far in the past for none. */
  let bumpStart = -10;
  const boneByName = new Map<string, Bone>();

  let boundListener: ((bound: { names: string[]; missing: string[] }) => void) | null = null;

  /** Builds the simulation for the soft bones the loaded model has, and reports the names it lacks. */
  function bindPhysics(): void {
    sim = physicsParams && physicsParams.size > 0 ? new SpringSim(physicsParams) : null;
    bumpStart = -10;
    restoreRoot();
    boundListener?.({ names: [...boneByName.keys()], missing: physicsParams && boneByName.size > 0 ? [...physicsParams.keys()].filter(name => !boneByName.has(name)) : [] });
  }

  function restoreRoot(): void {
    root?.position.set(0, 0, 0);
    root?.rotation.set(0, 0, 0);
  }

  /** Where a bone is in the model right now (its pivot in blocks and its axes), read before the physics touches the pose. */
  function frameOf(name: string): BoneFrame | null {
    const bone = boneByName.get(name);

    if (!bone) return null;

    const e = bone.node.matrixWorld.elements;
    const axis = (i: number): Vec3 => {
      const size = Math.hypot(e[i * 4], e[i * 4 + 1], e[i * 4 + 2]) || 1;

      return [e[i * 4] / size, e[i * 4 + 1] / size, e[i * 4 + 2] / size];
    };

    return { origin: [e[12], e[13], e[14]], axes: [axis(0), axis(1), axis(2)] };
  }

  /** Runs the soft bones for `dt` seconds on the pose that was just set, and adds what they do on top of it. */
  function simulate(dt: number): void {
    if (!sim || !root) return;

    const running = sim;
    const entity = (t: number) => entityAt(motion, t, t - bumpStart);

    /* The model is read where it stands at the origin, then moved to where its motion has it, so the bones' places are the model's own. */
    restoreRoot();
    root.updateMatrixWorld(true);
    running.advance(dt, entity, frameOf, [wind, 0, 0]);

    const offsets = new Map<string, ReturnType<SpringSim['offset']>>();

    for (const name of running.params.keys()) offsets.set(name, running.offset(name));

    for (const [name, offset] of offsets) {
      const bone = boneByName.get(name);

      if (!bone || !offset) continue;

      const size = Math.hypot(...offset.rot);

      if (size > 1e-7) bone.node.quaternion.multiply(new THREE.Quaternion().setFromAxisAngle(new THREE.Vector3(offset.rot[0] / size, offset.rot[1] / size, offset.rot[2] / size), size));

      bone.node.position.x += offset.pos[0] / unitsPerBlock;
      bone.node.position.y += offset.pos[1] / unitsPerBlock;
      bone.node.position.z += offset.pos[2] / unitsPerBlock;
    }

    const where = entity(running.now);

    root.position.set(...where.position);
    root.rotation.y = (where.yaw * Math.PI) / 180;
  }

  /* The light stays with the camera, so the side you look at is the side that is lit. */
  const ambient = new THREE.AmbientLight(0xffffff, 0.55);
  const sun = new THREE.DirectionalLight(0xffffff, 0.9);

  sun.position.set(0.4, 0.8, -1);
  camera.add(sun);
  scene.add(camera, ambient);

  const materials = {
    lit: new THREE.MeshLambertMaterial({ side: THREE.DoubleSide, transparent: true, alphaTest: 0.02, color: 0xcccccc }),
    flat: new THREE.MeshBasicMaterial({ side: THREE.DoubleSide, transparent: true, alphaTest: 0.02, color: 0xcccccc }),
    normals: new THREE.MeshNormalMaterial({ side: THREE.DoubleSide }),
    wire: new THREE.MeshBasicMaterial({ side: THREE.DoubleSide, wireframe: true, color: 0x9fb4d8 })
  };

  /* The scene rig is drawn with the player's skin, so it looks as it will in the game; they share the lighting of the girl's materials. */
  const rigMaterials = {
    lit: new THREE.MeshLambertMaterial({ side: THREE.DoubleSide, transparent: true, alphaTest: 0.02, color: 0xffffff }),
    flat: new THREE.MeshBasicMaterial({ side: THREE.DoubleSide, transparent: true, alphaTest: 0.02, color: 0xcccccc })
  };

  const materialFor = (piece: THREE.Object3D) => (piece.userData.rig && rigMap && (shading === 'lit' || shading === 'flat') ? rigMaterials[shading] : materials[shading]);

  function refreshNormals(mesh: FreeMesh): void {
    const smooth = smoothing === 'smooth' || (smoothing === 'authored' && mesh.data.smooth);

    const existing = mesh.piece.geometry.getAttribute('normal') as THREE.BufferAttribute | undefined;

    /* The buffer is written in place when it is there already: made anew every frame, a posed model spends its time on the allocations. */
    if (existing && existing.count === mesh.corners.length) {
      meshNormals(mesh.data, mesh.positions, smooth, existing.array as Float32Array);
      existing.needsUpdate = true;
    } else {
      mesh.piece.geometry.setAttribute('normal', new THREE.BufferAttribute(meshNormals(mesh.data, mesh.positions, smooth), 3));
    }
  }

  function applySmoothing(): void {
    for (const mesh of freeMeshes) refreshNormals(mesh);
  }

  /** Moves the vertices of the skinned meshes to where their armatures' bones put them now. */
  function updateSkins(): void {
    const any = freeMeshes.some(m => m.armature !== undefined);

    if (!any) return;

    for (const armature of armatures) poseArmature(armature, armatureChannels, time);

    const rest = new THREE.Vector3();
    const sum = new THREE.Vector3();
    const moved = new THREE.Vector3();

    for (const mesh of freeMeshes) {
      if (mesh.armature === undefined || !mesh.rest || !mesh.pivot) continue;

      const armature = armatures[mesh.armature];
      const pivot = mesh.pivot;

      for (let v = 0; v < mesh.positions.length / 3; v++) {
        const influences = mesh.data.skin[v] ?? [];

        rest.set(mesh.rest[v * 3], mesh.rest[v * 3 + 1], mesh.rest[v * 3 + 2]);
        sum.set(0, 0, 0);

        let total = 0;

        for (const [bone, weight] of influences) {
          const matrix = armature?.skin[bone];

          if (!matrix || weight === 0) continue;

          sum.addScaledVector(moved.copy(rest).applyMatrix4(matrix), weight);
          total += weight;
        }

        if (total === 0) sum.copy(rest);

        mesh.positions[v * 3] = sum.x - pivot[0];
        mesh.positions[v * 3 + 1] = sum.y - pivot[1];
        mesh.positions[v * 3 + 2] = sum.z - pivot[2];
      }

      const attribute = mesh.piece.geometry.getAttribute('position') as THREE.BufferAttribute;

      mesh.corners.forEach((v, corner) => attribute.setXYZ(corner, mesh.positions[v * 3], mesh.positions[v * 3 + 1], mesh.positions[v * 3 + 2]));
      attribute.needsUpdate = true;
      refreshNormals(mesh);
    }
  }

  function applyShading(): void {
    const material = materials[shading];

    for (const piece of pieces) piece.material = piece.userData.rig ? materialFor(piece) : material;

    rigMaterials.flat.color.setScalar(Math.min(1, 0.8 * brightness));
    ambient.intensity = 0.55 * brightness;
    sun.intensity = 0.9 * brightness;
    materials.flat.color.setScalar(Math.min(1, 0.8 * brightness));
    materials.lit.color.set(0xffffff);
  }
  function applyRig(): void {
    for (const bone of bones) bone.node.visible = !(bone.rigSelf && rigHidden);
  }

  function frameModel(): void {
    if (!root) return;

    const box = new THREE.Box3();

    root.updateMatrixWorld(true);
    root.traverse(o => {
      if (!(o as THREE.Mesh).isMesh) return;

      for (let p: THREE.Object3D | null = o; p; p = p.parent) if (p.visible === false) return;

      box.expandByObject(o);
    });

    if (!box.isEmpty()) {
      const size = box.getSize(new THREE.Vector3());
      const center = box.getCenter(new THREE.Vector3());
      const distance = Math.max(size.y, size.x, size.z) * 2.2 + 0.5;

      controls.target.copy(center);
      camera.position.set(center.x, center.y + size.y * 0.1, center.z - distance);
      controls.update();
      grid.position.y = box.min.y;
    }
  }

  function resize(): void {
    const w = Math.max(10, container.clientWidth);
    const h = Math.max(10, container.clientHeight);

    /* Nothing to draw into yet (the panel has not been laid out): the next resize does it. */
    if (container.clientWidth < 10 || container.clientHeight < 10) return;

    renderer.setSize(w, h, false);
    renderer.domElement.style.width = '100%';
    renderer.domElement.style.height = '100%';
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
  }

  function pose(): void {
    for (const bone of bones) {
      const channels = channelsByBone.get(bone.name);
      let rotation = bone.rest.rotation;
      let position: number[] = [0, 0, 0];
      let scale: number[] = [1, 1, 1];

      if (channels) {
        if (channels.ROTATION) rotation = sample(channels.ROTATION, time, [0, 0, 0]).map((v, i) => v + bone.rest.rotation[i]);
        if (channels.POSITION) position = sample(channels.POSITION, time, [0, 0, 0]);
        if (channels.SCALE) scale = sample(channels.SCALE, time, [1, 1, 1]);
      }

      setEuler(bone.node, rotation);
      bone.node.position.set(bone.rest.offset[0] + position[0] / 16, bone.rest.offset[1] + position[1] / 16, bone.rest.offset[2] + position[2] / 16);
      bone.node.scale.set(scale[0], scale[1], scale[2]);
    }

    updateSkins();
  }

  /** Tells the listener of the events whose time has been reached since the last time. */
  function hear(until: number): void {
    if (!clip || !eventListener) { heardUntil = until; return; }

    for (const event of clip.events) {
      if (event.time > heardUntil && event.time <= until) eventListener({ time: event.time, kind: event.kind, payload: event.payload });
    }

    heardUntil = until;
  }

  let lastTick = 0;

  function frame(): void {
    if (disposed) return;

    requestAnimationFrame(frame);

    const now = performance.now();
    const dt = (now - last) / 1000;

    last = now;

    let posed = false;

    if (clip && playing) {
      posed = true;
      time += dt * speed;

      let ended = false;

      if (time > clip.duration) {
        if (loop && clip.duration > 0) {
          hear(clip.duration);
          time %= clip.duration;
          heardUntil = -1;
        } else {
          hear(clip.duration);
          time = clip.duration;
          playing = false;
          ended = true;
        }
      }

      hear(time);
      pose();
      /* The page around the view (the time bar) is told about 15 times a second, not on every frame: redrawing it each frame costs more than the animation does. */
      if (ended || now - lastTick > 66) {
        lastTick = now;
        tick?.(time, playing);
      }

      if (ended) endListener?.();
    }

    /* The soft bones move even when the clip is paused or there is none, so the pose is made again each frame under them. */
    if (physicsOn && sim) {
      if (!posed) pose();

      simulate(dt);
    }

    controls.update();
    renderer.render(scene, camera);
  }

  /* A click (not a drag of the camera) on the model is reported, saying whether it was the girl or the scene rig. */
  const raycaster = new THREE.Raycaster();
  let pressedAt: { x: number; y: number } | null = null;

  function pointerDown(event: PointerEvent): void {
    pressedAt = { x: event.clientX, y: event.clientY };
  }

  function pointerUp(event: PointerEvent): void {
    if (!pressedAt || !pickListener) return;

    const moved = Math.hypot(event.clientX - pressedAt.x, event.clientY - pressedAt.y);

    pressedAt = null;

    if (moved > 5) return;

    const rect = renderer.domElement.getBoundingClientRect();

    raycaster.setFromCamera(new THREE.Vector2(((event.clientX - rect.left) / rect.width) * 2 - 1, -((event.clientY - rect.top) / rect.height) * 2 + 1), camera);

    const visible = pieces.filter(piece => { for (let p: THREE.Object3D | null = piece; p; p = p.parent) if (p.visible === false) return false; return true; });
    const hit = raycaster.intersectObjects(visible, false)[0];

    pickListener(hit ? (hit.object.userData.rig ? 'rig' : 'girl') : 'none');
  }

  renderer.domElement.addEventListener('pointerdown', pointerDown);
  renderer.domElement.addEventListener('pointerup', pointerUp);

  /* The panel's size is known only once it is laid out, and changes with the window and with the panels around it: watch it, not just the window. */
  const observer = new ResizeObserver(() => resize());

  observer.observe(container);
  window.addEventListener('resize', resize);
  resize();
  requestAnimationFrame(frame);

  return {
    loadModel(model, textureUrl, keepCamera) {
      if (root) scene.remove(root);

      root = new THREE.Group();
      bones = [];
      boneByName.clear();
      pieces.length = 0;
      freeMeshes.length = 0;
      armatures = model.armatures.map(a => buildArmature(a, model, model.unitsPerBlock || 16));

      const unit = model.unitsPerBlock || 16;
      const texture = model.textures[0] ?? { name: '', width: 64, height: 64, uvWidth: 64, uvHeight: 64 };
      const nodes: THREE.Group[] = [];
      materials.lit.map = materials.flat.map = null;

      const useMap = (loaded: THREE.Texture) => {
        materials.lit.map = loaded;
        materials.flat.map = loaded;
        materials.lit.needsUpdate = true;
        materials.flat.needsUpdate = true;
        applyShading();
      };

      if (textureUrl) {
        const cached = textureCache.get(textureUrl);

        if (cached) {
          useMap(cached);
        } else {
          new THREE.TextureLoader().load(textureUrl, loaded => {
            /* A picture is in sRGB: said so, three.js does not brighten it a second time. */
            loaded.colorSpace = THREE.SRGBColorSpace;
            loaded.magFilter = THREE.NearestFilter;
            loaded.minFilter = THREE.NearestFilter;
            textureCache.set(textureUrl, loaded);
            useMap(loaded);
          });
        }
      }

      const rigBone: boolean[] = [];

      model.groupBones.forEach(bone => {
        const node = new THREE.Group();
        const parent = bone.parentIndex >= 0 ? model.groupBones[bone.parentIndex] : null;
        const offset = [0, 1, 2].map(i => (bone.pivot[i] - (parent ? parent.pivot[i] : 0)) / unit);

        node.position.set(offset[0], offset[1], offset[2]);
        setEuler(node, bone.rotation);
        node.name = bone.name;
        nodes.push(node);
        rigBone.push(isRigName(bone.name, bone.parentIndex < 0) || (bone.parentIndex >= 0 && rigBone[bone.parentIndex] === true));
        (bone.parentIndex >= 0 && nodes[bone.parentIndex] ? nodes[bone.parentIndex] : root!).add(node);
        bones.push({ name: bone.name, node, rigSelf: isRigName(bone.name, bone.parentIndex < 0), rest: { rotation: [...bone.rotation], offset } });
        boneByName.set(bone.name, bones[bones.length - 1]);

        for (const element of bone.elements) {
          const place = (piece: THREE.Mesh, origin: number[], rotation: number[]) => {
            piece.userData.rig = rigBone[rigBone.length - 1];
            pieces.push(piece);
            piece.position.set((origin[0] - bone.pivot[0]) / unit, (origin[1] - bone.pivot[1]) / unit, (origin[2] - bone.pivot[2]) / unit);
            setEuler(piece, rotation);
            node.add(piece);
          };

          if (element.kind === 0 && model.meshes[element.index]) {
            const free = model.meshes[element.index];

            const corners = free.faces.flatMap(f => f.vertices);

            if (free.armatureIndex >= 0 && model.armatures[free.armatureIndex]) {
              /* Skinned: drawn from where it rests in model space, relative to this bone, and moved by its armature. The mesh's own origin and rotation are already in it. */
              const armature = model.armatures[free.armatureIndex];
              const frame = armature.parentGroupBone >= 0 ? model.groupBones[armature.parentGroupBone]?.pivot ?? [0, 0, 0] : [0, 0, 0];
              const rest = skinnedRest(free, frame, unit);
              const pivotBlocks = [bone.pivot[0] / unit, bone.pivot[1] / unit, bone.pivot[2] / unit];
              const positions = new Float32Array(rest.length);

              for (let v = 0; v < rest.length / 3; v++) positions.set([rest[v * 3] - pivotBlocks[0], rest[v * 3 + 1] - pivotBlocks[1], rest[v * 3 + 2] - pivotBlocks[2]], v * 3);

              const piece = new THREE.Mesh(buildMesh(free, texture, positions), materials[shading]);

              freeMeshes.push({ piece, data: free, positions, rest, pivot: pivotBlocks, armature: free.armatureIndex, corners });
              piece.userData.rig = rigBone[rigBone.length - 1];
              pieces.push(piece);
              node.add(piece);
            } else {
              const positions = Float32Array.from(free.positions, p => p / unit);
              const piece = new THREE.Mesh(buildMesh(free, texture, positions), materials[shading]);

              freeMeshes.push({ piece, data: free, positions, corners });
              place(piece, free.origin, free.rotation);
            }
          } else if (element.kind === 1 && model.cubes[element.index]) {
            const cube = model.cubes[element.index];

            place(new THREE.Mesh(buildCube(cube, texture, unit), materials[shading]), cube.origin, cube.rotation);
          }
        }
      });

      scene.add(root);
      applySmoothing();
      applyShading();
      applyRig();
      unitsPerBlock = unit;
      bindPhysics();
      pose();
      if (!keepCamera) frameModel();
    },

    setClip(next) {
      clip = next;
      time = 0;
      channelsByBone = new Map();
      armatureChannels = new Map();

      if (clip) {
        for (const channel of clip.channels) {
          const store = channel.kind === 'ARMATURE_BONE' ? armatureChannels : channelsByBone;
          const entry = store.get(channel.target) ?? {};

          entry[channel.property as 'ROTATION' | 'POSITION' | 'SCALE'] = channel.keys;
          store.set(channel.target, entry);
        }

        loop = clip.loopMode !== 0;
      }

      for (const bone of bones) {
        setEuler(bone.node, bone.rest.rotation);
        bone.node.position.set(bone.rest.offset[0], bone.rest.offset[1], bone.rest.offset[2]);
        bone.node.scale.set(1, 1, 1);
      }

      pose();
    },

    playClip(next, repeat) {
      this.setClip(next);
      loop = repeat;
      heardUntil = -1;

      if (next) { playing = true; last = performance.now(); }
    },
    onEvent(callback) { eventListener = callback; },
    onClipEnd(callback) { endListener = callback; },
    onPick(callback) { pickListener = callback; },
    getTime() { return time; },
    getView() {
      return { position: [camera.position.x, camera.position.y, camera.position.z], target: [controls.target.x, controls.target.y, controls.target.z] };
    },
    setView(view) {
      camera.position.set(...view.position);
      controls.target.set(...view.target);
      controls.update();
    },
    setPhysics(next) {
      physicsParams = next;
      bindPhysics();

      if (!physicsOn || !sim) pose();
    },
    onPhysicsBound(callback) { boundListener = callback; },
    setPhysicsRun(run) {
      const turnedOff = physicsOn && !run.on;

      physicsOn = run.on;
      motion = run.motion;
      wind = run.wind;

      if (turnedOff) { restoreRoot(); sim?.reset(); pose(); }
    },
    bumpPhysics() { if (sim) bumpStart = sim.now; },
    resetPhysics() { sim?.reset(); bumpStart = -10; },
    setRigSkin(url) {
      if (!url) { rigMap = null; rigMaterials.lit.map = rigMaterials.flat.map = null; applyShading(); return; }

      new THREE.TextureLoader().load(url, loaded => {
        loaded.colorSpace = THREE.SRGBColorSpace;
        loaded.magFilter = THREE.NearestFilter;
        loaded.minFilter = THREE.NearestFilter;
        rigMap = loaded;
        rigMaterials.lit.map = rigMaterials.flat.map = loaded;
        rigMaterials.lit.needsUpdate = rigMaterials.flat.needsUpdate = true;
        applyShading();
      });
    },
    setBed(bed) {
      if (bedGroup) { scene.remove(bedGroup); bedGroup = null; }

      if (!bed) return;

      /* The game's bed: its own model and texture, the single bed (the bones of the double bed are hidden, as the game hides them), its foot block's middle where the
         game puts it from her: in front of her, as she faces (the model's head end is its -Z). */
      const group = buildStatic(bed.model, bed.texture, /^(doubleBed[LR]|headRest[LR]|headrest[LR])$/i);

      group.position.set(0, 0, -bed.forward);
      scene.add(group);
      bedGroup = group;
    },

    play() { if (clip) { if (time >= clip.duration) time = 0; playing = true; last = performance.now(); } },
    pause() { playing = false; },
    isPlaying() { return playing; },
    seek(t) { time = Math.max(0, Math.min(t, clip ? clip.duration : 0)); pose(); },
    setSpeed(v) { speed = v; },
    setLoop(v) { loop = v; },
    setRigHidden(hidden, frame = true) {
      if (rigHidden === hidden && !frame) return;

      rigHidden = hidden;
      applyRig();
      if (frame) frameModel();
    },
    setShading(next) { shading = next; applyShading(); },
    setBrightness(next) { brightness = next; applyShading(); },
    setSmoothing(next) { smoothing = next; applySmoothing(); },
    setGrid(visible) { grid.visible = visible; },
    onTick(callback) { tick = callback; },
    resize,
    dispose() {
      renderer.domElement.removeEventListener('pointerdown', pointerDown);
      renderer.domElement.removeEventListener('pointerup', pointerUp);
      disposed = true;
      observer.disconnect();
      window.removeEventListener('resize', resize);
      renderer.dispose();
      container.replaceChildren();
    }
  };
}
