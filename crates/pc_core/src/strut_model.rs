//! A reader for Strut's binary models (`.strutmodel`), for the viewer and the checks: bones, cubes and free-form meshes with their UVs, and the textures they name. Skinning
//! weights and armatures are read past, not kept (the viewer draws a model in its rest pose). The layout is Strut's `BinaryModelReader`; keep the two in step.

use serde::Serialize;
use std::collections::BTreeMap;

pub type Vec3 = [f32; 3];

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Texture {
    pub name: String,
    pub width: u16,
    pub height: u16,
    pub uv_width: u16,
    pub uv_height: u16,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Element {
    /// 0 is a mesh, 1 is a cube.
    pub kind: u8,
    pub index: u16,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GroupBone {
    pub name: String,
    /// -1 for a root.
    pub parent_index: i16,
    pub pivot: Vec3,
    pub rotation: Vec3,
    pub elements: Vec<Element>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CubeFace {
    pub face_id: u8,
    pub u1: f32,
    pub v1: f32,
    pub u2: f32,
    pub v2: f32,
    pub uv_rotation: u8,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Cube {
    pub name: String,
    pub texture_index: u16,
    pub origin: Vec3,
    pub rotation: Vec3,
    pub from: Vec3,
    pub to: Vec3,
    pub inflate: f32,
    pub mirror: bool,
    pub box_uv: bool,
    pub faces: Vec<CubeFace>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MeshFace {
    pub vertices: Vec<u32>,
    pub uvs: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Mesh {
    pub name: String,
    pub texture_index: u16,
    pub origin: Vec3,
    pub rotation: Vec3,
    pub positions: Vec<f32>,
    pub faces: Vec<MeshFace>,
    /// Adjacent faces blend their normals at a shared vertex ("shade smooth") instead of each being flat.
    pub smooth: bool,
    /// The armature that deforms it, or -1 when it is not skinned.
    pub armature_index: i16,
    /// For each vertex, the bones that move it and how much: `[bone, weight]` pairs. Empty when the mesh is not skinned.
    pub skin: Vec<Vec<[f32; 2]>>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ArmatureBone {
    pub name: String,
    /// -1 for a root.
    pub parent_index: i16,
    pub head: Vec3,
    pub rotation: Vec3,
    pub length: f32,
}

/// A skeleton of its own that deforms skinned meshes. It hangs off a group bone (its frame is that bone's pivot) or off the model.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Armature {
    pub name: String,
    pub parent_group_bone: i16,
    pub bones: Vec<ArmatureBone>,
}

/// Where the numbers of a bone sit in the file (byte offsets of the first of three floats), so a tool can change a few of them without writing the whole model again.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BoneAt {
    /// Where the whole record starts and ends (so one can be cut out), and where its parent index sits.
    pub start: usize,
    pub end: usize,
    pub parent: usize,
    pub pivot: usize,
    pub rotation: usize,
}

/// The same for a cube: its origin (pivot), rotation, and the two corners of its box.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CubeAt {
    pub start: usize,
    pub end: usize,
    pub origin: usize,
    pub rotation: usize,
    pub from: usize,
    pub to: usize,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Offsets {
    pub bones: Vec<BoneAt>,
    pub cubes: Vec<CubeAt>,
    /// Where each armature's parent group bone index sits.
    pub armature_parents: Vec<usize>,
    /// Where the extension sections start (the count of them when the file has any), which is the end of the file when it has none.
    pub extensions_at: usize,
    /// Where the sections of bones, meshes, cubes and armatures start (a section ends where the next starts).
    pub bones_start: usize,
    pub meshes_start: usize,
    pub cubes_start: usize,
    pub armatures_start: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    pub version: u16,
    pub units_per_block: f32,
    pub bounds_min: Vec3,
    pub bounds_max: Vec3,
    pub textures: Vec<Texture>,
    pub group_bones: Vec<GroupBone>,
    pub meshes: Vec<Mesh>,
    pub cubes: Vec<Cube>,
    pub armature_count: u16,
    pub armatures: Vec<Armature>,
    /// The names of the extension sections the file carries (`strut_physics:physics`).
    pub extensions: Vec<String>,
    /// The skeleton id (`mymod:creature_a`) from the `strut:skeleton` extension: which rig this model is. None when the model has none.
    pub skeleton_id: Option<String>,
    /// Where each bone's and cube's numbers are in the file. Not part of what the viewer is sent.
    #[serde(skip)]
    pub offsets: Offsets,
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize, what: &str) -> Result<&'a [u8], String> {
        if self.at + count > self.bytes.len() {
            return Err(format!("the file ends inside {what} (it is cut short or damaged)"));
        }

        let slice = &self.bytes[self.at..self.at + count];

        self.at += count;
        Ok(slice)
    }

    fn u8(&mut self, what: &str) -> Result<u8, String> {
        Ok(self.take(1, what)?[0])
    }

    fn u16(&mut self, what: &str) -> Result<u16, String> {
        let b = self.take(2, what)?;

        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    fn i16(&mut self, what: &str) -> Result<i16, String> {
        Ok(self.u16(what)? as i16)
    }

    fn u32(&mut self, what: &str) -> Result<u32, String> {
        let b = self.take(4, what)?;

        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn f32(&mut self, what: &str) -> Result<f32, String> {
        Ok(f32::from_bits(self.u32(what)?))
    }

    fn vec3(&mut self, what: &str) -> Result<Vec3, String> {
        Ok([self.f32(what)?, self.f32(what)?, self.f32(what)?])
    }

    fn string(&mut self, what: &str) -> Result<String, String> {
        let length = self.u16(what)? as usize;

        Ok(String::from_utf8_lossy(self.take(length, what)?).to_string())
    }
}

const HEADER_BYTES: usize = 64;

fn name_of(strings: &[String], index: u16, what: &str) -> Result<String, String> {
    strings.get(index as usize).cloned().ok_or_else(|| format!("{what} points at string {index}, but the file has {} strings", strings.len()))
}

/// Reads a model, or says what is wrong with the file.
pub fn read(bytes: &[u8]) -> Result<Model, String> {
    if bytes.is_empty() {
        return Err("the file is empty".into());
    }

    let mut r = Reader { bytes, at: 0 };

    if r.take(4, "the start of the file")? != b"STMD" {
        return Err("this is not a Strut binary model (.strutmodel): it does not start with STMD. Convert it with Strut's converter".into());
    }

    let version = r.u16("the version")?;

    if !(100..=101).contains(&version) {
        return Err(format!("this is model format version {version}, and the game reads versions 100 and 101. Convert it again with a matching Strut"));
    }

    let flags = r.u16("the flags")?;
    let string_count = r.u16("the string count")?;
    let texture_count = r.u16("the texture count")?;
    let bone_count = r.u16("the bone count")?;
    let mesh_count = r.u16("the mesh count")?;
    let cube_count = r.u16("the cube count")?;
    let armature_count = r.u16("the armature count")?;

    r.u32("the string table size")?;

    let units = r.f32("the units per block")?;
    let bounds_min = r.vec3("the bounds")?;
    let bounds_max = r.vec3("the bounds")?;

    r.take(12, "the rest of the header")?;

    if r.at != HEADER_BYTES {
        return Err("the header is the wrong size".into());
    }

    if !(units > 0.0 && units.is_finite()) {
        return Err(format!("the model says it has {units} units per block, which cannot be right"));
    }

    let mut strings = Vec::with_capacity(string_count as usize);

    for _ in 0..string_count {
        strings.push(r.string("a string")?);
    }

    let mut textures = Vec::new();

    for _ in 0..texture_count {
        let name = name_of(&strings, r.u16("a texture name")?, "a texture")?;

        textures.push(Texture { name, width: r.u16("a texture width")?, height: r.u16("a texture height")?, uv_width: r.u16("a texture uv width")?, uv_height: r.u16("a texture uv height")? });
    }

    let mut group_bones = Vec::new();
    let mut offsets = Offsets::default();

    offsets.bones_start = r.at;

    for _ in 0..bone_count {
        let start = r.at;
        let name = name_of(&strings, r.u16("a bone name")?, "a bone")?;
        let parent_at = r.at;
        let parent_index = r.i16("a bone parent")?;
        let pivot_at = r.at;
        let pivot = r.vec3("a bone pivot")?;
        let rotation_at = r.at;
        let rotation = r.vec3("a bone rotation")?;

        r.u8("a bone's flags")?;

        let count = r.u16("a bone's element count")?;
        let mut elements = Vec::new();

        for _ in 0..count {
            elements.push(Element { kind: r.u8("an element kind")?, index: r.u16("an element index")? });
        }

        offsets.bones.push(BoneAt { start, end: r.at, parent: parent_at, pivot: pivot_at, rotation: rotation_at });
        group_bones.push(GroupBone { name, parent_index, pivot, rotation, elements });
    }

    let mut meshes = Vec::new();

    offsets.meshes_start = r.at;

    for _ in 0..mesh_count {
        let name = name_of(&strings, r.u16("a mesh name")?, "a mesh")?;
        let texture_index = r.u16("a mesh texture")?;
        let origin = r.vec3("a mesh origin")?;
        let rotation = r.vec3("a mesh rotation")?;
        let mesh_flags = r.u8("a mesh's flags")?;

        let armature_index = r.i16("a mesh armature")?;

        let vertex_count = r.u32("a vertex count")? as usize;
        let face_count = r.u32("a face count")? as usize;

        if vertex_count.saturating_mul(12) > bytes.len() || face_count > bytes.len() {
            return Err("a mesh claims more vertices or faces than the file could hold".into());
        }

        let mut positions = Vec::with_capacity(vertex_count * 3);

        for _ in 0..vertex_count * 3 {
            positions.push(r.f32("a vertex")?);
        }

        let mut faces = Vec::new();

        for _ in 0..face_count {
            let corners = r.u8("a face's corner count")?;
            let mut vertices = Vec::new();
            let mut uvs = Vec::new();

            for _ in 0..corners {
                vertices.push(r.u32("a corner's vertex")?);
                uvs.push(r.f32("a corner's u")?);
                uvs.push(r.f32("a corner's v")?);
            }

            faces.push(MeshFace { vertices, uvs });
        }

        let mut skin = Vec::new();

        if mesh_flags & 4 != 0 {
            for _ in 0..vertex_count {
                let influences = r.u8("an influence count")?;
                let mut list = Vec::new();

                for _ in 0..influences {
                    let bone = r.u16("an influence bone")?;

                    list.push([bone as f32, r.f32("an influence weight")?]);
                }

                skin.push(list);
            }
        }

        meshes.push(Mesh { name, texture_index, origin, rotation, positions, faces, smooth: mesh_flags & 8 != 0, armature_index: if mesh_flags & 4 != 0 { armature_index } else { -1 }, skin });
    }

    let mut cubes = Vec::new();

    offsets.cubes_start = r.at;

    for _ in 0..cube_count {
        let cube_start = r.at;
        let name = name_of(&strings, r.u16("a cube name")?, "a cube")?;
        let texture_index = r.u16("a cube texture")?;
        let origin_at = r.at;
        let origin = r.vec3("a cube origin")?;
        let rotation_at = r.at;
        let rotation = r.vec3("a cube rotation")?;
        let from_at = r.at;
        let from = r.vec3("a cube's from")?;
        let to_at = r.at;
        let to = r.vec3("a cube's to")?;

        let cube_slot = offsets.cubes.len();

        offsets.cubes.push(CubeAt { start: cube_start, end: 0, origin: origin_at, rotation: rotation_at, from: from_at, to: to_at });
        let inflate = r.f32("a cube's inflate")?;
        let cube_flags = r.u8("a cube's flags")?;
        let face_count = r.u8("a cube's face count")?;
        let mut faces = Vec::new();

        for _ in 0..face_count {
            faces.push(CubeFace { face_id: r.u8("a face id")?, u1: r.f32("a face u1")?, v1: r.f32("a face v1")?, u2: r.f32("a face u2")?, v2: r.f32("a face v2")?, uv_rotation: r.u8("a face's uv rotation")? });
        }

        offsets.cubes[cube_slot].end = r.at;
        cubes.push(Cube { name, texture_index, origin, rotation, from, to, inflate, mirror: cube_flags & 2 != 0, box_uv: cube_flags & 4 != 0, faces });
    }

    let mut armatures = Vec::new();

    offsets.armatures_start = r.at;

    for _ in 0..armature_count {
        let name = name_of(&strings, r.u16("an armature name")?, "an armature")?;

        r.vec3("an armature origin")?;

        offsets.armature_parents.push(r.at);

        let parent_group_bone = r.i16("an armature parent")?;

        r.u8("an armature's flags")?;

        let count = r.u16("an armature's bone count")?;
        let mut bones = Vec::new();

        for _ in 0..count {
            let bone_name = name_of(&strings, r.u16("an armature bone name")?, "an armature bone")?;
            let parent_index = r.i16("an armature bone parent")?;
            let head = r.vec3("an armature bone head")?;
            let rotation = r.vec3("an armature bone rotation")?;
            let length = r.f32("an armature bone length")?;

            if version >= 101 {
                r.f32("an armature bone width")?;
                r.u8("an armature bone colour")?;
            }

            r.u8("an armature bone's flags")?;
            bones.push(ArmatureBone { name: bone_name, parent_index, head, rotation, length });
        }

        armatures.push(Armature { name, parent_group_bone, bones });
    }

    let mut extensions = Vec::new();
    let mut skeleton_id = None;

    offsets.extensions_at = r.at;

    if flags & 1 != 0 {
        let count = r.u16("the extension count")?;

        for _ in 0..count {
            let name = r.string("an extension name")?;
            let length = r.u32("an extension length")? as usize;
            let payload = r.take(length, "an extension")?;

            if name == SKELETON_EXTENSION {
                skeleton_id = skeleton_from(payload);
            }

            extensions.push(name);
        }
    }

    Ok(Model { version, units_per_block: units, bounds_min, bounds_max, textures, group_bones, meshes, cubes, armature_count, armatures, extensions, skeleton_id, offsets })
}

/// Writes the model again without the group bones at these indices and the cubes only they hold. Every other byte is copied as it was (meshes, skinning, armatures, extensions and
/// the strings), and the indices that point at bones and cubes are renumbered. A removed bone that holds a mesh, or that an armature hangs off, is refused.
pub fn remove_bones(bytes: &[u8], remove: &std::collections::HashSet<usize>) -> Result<Vec<u8>, String> {
    edit_bones(bytes, remove, &std::collections::HashSet::new())
}

/// Like `remove_bones`, and the bones in `empty` stay but lose their cubes (and the cubes only they held). A bone that is both removed and emptied is removed.
pub fn edit_bones(bytes: &[u8], remove: &std::collections::HashSet<usize>, empty: &std::collections::HashSet<usize>) -> Result<Vec<u8>, String> {
    let model = read(bytes)?;
    let holds = |i: usize, element: &Element| !(empty.contains(&i) && element.kind == 1);
    let o = &model.offsets;
    let mut new_bone = vec![-1i32; model.group_bones.len()];
    let mut kept = 0;

    for i in 0..model.group_bones.len() {
        if !remove.contains(&i) {
            new_bone[i] = kept;
            kept += 1;
        }
    }

    for bone in model.group_bones.iter().enumerate().filter(|(i, _)| remove.contains(i)).map(|(_, b)| b) {
        if bone.elements.iter().any(|e| e.kind == 0) {
            return Err(format!("the bone {} holds a mesh", bone.name));
        }
    }

    for armature in &model.armatures {
        if armature.parent_group_bone >= 0 && remove.contains(&(armature.parent_group_bone as usize)) {
            return Err(format!("the armature {} hangs off a bone that would be removed", armature.name));
        }
    }

    /* The cubes of the bones that stay keep their order. */
    let mut new_cube: BTreeMap<u16, u16> = BTreeMap::new();
    let mut cube_order: Vec<usize> = Vec::new();

    for (i, bone) in model.group_bones.iter().enumerate().filter(|(i, _)| !remove.contains(i)) {
        for element in bone.elements.iter().filter(|e| e.kind == 1 && holds(i, e)) {
            if !new_cube.contains_key(&element.index) {
                new_cube.insert(element.index, cube_order.len() as u16);
                cube_order.push(element.index as usize);
            }
        }
    }

    let mut out = bytes[..o.bones_start].to_vec();

    for (i, bone) in model.group_bones.iter().enumerate().filter(|(i, _)| !remove.contains(i)) {
        let at = &o.bones[i];
        let parent = if bone.parent_index >= 0 { new_bone[bone.parent_index as usize] } else { -1 };

        if parent < 0 && bone.parent_index >= 0 {
            return Err(format!("the bone {} hangs off a bone that would be removed", bone.name));
        }

        out.extend_from_slice(&bytes[at.start..at.parent]);
        out.extend_from_slice(&(parent as i16).to_le_bytes());
        out.extend_from_slice(&bytes[at.parent + 2..at.rotation + 12 + 1]);
        out.extend_from_slice(&(bone.elements.iter().filter(|e| holds(i, e)).count() as u16).to_le_bytes());

        for element in bone.elements.iter().filter(|e| holds(i, e)) {
            out.push(element.kind);
            out.extend_from_slice(&(if element.kind == 1 { new_cube[&element.index] } else { element.index }).to_le_bytes());
        }
    }

    out.extend_from_slice(&bytes[o.meshes_start..o.cubes_start]);

    for cube in &cube_order {
        out.extend_from_slice(&bytes[o.cubes[*cube].start..o.cubes[*cube].end]);
    }

    let armatures_at = out.len();

    out.extend_from_slice(&bytes[o.armatures_start..]);

    for (armature, at) in model.armatures.iter().zip(&o.armature_parents) {
        if armature.parent_group_bone >= 0 {
            let place = armatures_at + (at - o.armatures_start);

            out[place..place + 2].copy_from_slice(&(new_bone[armature.parent_group_bone as usize] as i16).to_le_bytes());
        }
    }

    out[12..14].copy_from_slice(&(kept as u16).to_le_bytes());
    out[16..18].copy_from_slice(&(cube_order.len() as u16).to_le_bytes());

    let back = read(&out).map_err(|e| format!("the model without those bones did not read back: {e}"))?;

    if back.group_bones.len() != kept as usize || back.cubes.len() != cube_order.len() || back.meshes.len() != model.meshes.len() {
        return Err("the model without those bones lost something it should have kept".into());
    }

    Ok(out)
}

/// Sets the size of every texture (the pixels, not the UV space the faces are written in).
pub fn set_texture_size(bytes: &[u8], width: u16, height: u16) -> Result<Vec<u8>, String> {
    let model = read(bytes)?;
    let count = model.textures.len();
    let first = model.offsets.bones_start - 10 * count;
    let mut out = bytes.to_vec();

    for i in 0..count {
        let at = first + 10 * i;

        out[at + 2..at + 4].copy_from_slice(&width.to_le_bytes());
        out[at + 4..at + 6].copy_from_slice(&height.to_le_bytes());
    }

    read(&out)?;
    Ok(out)
}

/// The model without any extension section (physics, the skeleton id): they name bones of the body it was made for.
pub fn drop_extensions(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let model = read(bytes)?;
    let mut out = bytes[..model.offsets.extensions_at].to_vec();
    let flags = u16::from_le_bytes([bytes[6], bytes[7]]) & !1;

    out[6..8].copy_from_slice(&flags.to_le_bytes());
    Ok(out)
}

impl Model {
    /// The hash the game recomputes from a skeleton: every group bone's name in order, each followed by a NUL, hashed as one string (Java's `String.hashCode`). Animation files made for
    /// the model carry it.
    pub fn fingerprint(&self) -> i32 {
        let mut names = String::new();

        for bone in &self.group_bones {
            names.push_str(&bone.name);
            names.push('\0');
        }

        names.encode_utf16().fold(0i32, |hash, unit| hash.wrapping_mul(31).wrapping_add(unit as i32))
    }
}

/// The extension section that holds a model's skeleton id.
pub const SKELETON_EXTENSION: &str = "strut:skeleton";

/// The id in the section's payload: a `uint16` length and that many UTF-8 bytes. An empty or cut-short payload is no id, as in Strut.
fn skeleton_from(payload: &[u8]) -> Option<String> {
    if payload.len() < 2 {
        return None;
    }

    let length = u16::from_le_bytes([payload[0], payload[1]]) as usize;

    if length == 0 || payload.len() < 2 + length {
        return None;
    }

    Some(String::from_utf8_lossy(&payload[2..2 + length]).to_string())
}

/// Writes the model again with its skeleton id set (or taken away with None). Only the extension sections change: every other byte is copied as it was, and the other
/// sections (physics, for one) keep their order and contents.
pub fn set_skeleton_id(bytes: &[u8], id: Option<&str>) -> Result<Vec<u8>, String> {
    let model = read(bytes)?;
    let flags = u16::from_le_bytes([bytes[6], bytes[7]]);
    let mut sections: Vec<(String, Vec<u8>)> = Vec::new();

    if flags & 1 != 0 {
        let mut r = Reader { bytes, at: model.offsets.extensions_at };
        let count = r.u16("the extension count")?;

        for _ in 0..count {
            let name = r.string("an extension name")?;
            let length = r.u32("an extension length")? as usize;

            sections.push((name, r.take(length, "an extension")?.to_vec()));
        }
    }

    sections.retain(|(name, _)| name != SKELETON_EXTENSION);

    if let Some(id) = id {
        if id.is_empty() || id.len() > 0xFFFF {
            return Err("a skeleton id has to be between 1 and 65535 bytes".into());
        }

        let mut payload = (id.len() as u16).to_le_bytes().to_vec();

        payload.extend_from_slice(id.as_bytes());
        sections.insert(0, (SKELETON_EXTENSION.to_string(), payload));
    }

    let mut out = bytes[..model.offsets.extensions_at].to_vec();
    let flags = if sections.is_empty() { flags & !1 } else { flags | 1 };

    out[6..8].copy_from_slice(&flags.to_le_bytes());

    if !sections.is_empty() {
        out.extend_from_slice(&(sections.len() as u16).to_le_bytes());

        for (name, payload) in &sections {
            out.extend_from_slice(&(name.len() as u16).to_le_bytes());
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            out.extend_from_slice(payload);
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn refuses_what_is_not_a_model() {
        assert!(read(b"").is_err());
        assert!(read(b"STAM and more bytes").unwrap_err().contains("STMD"));

        let mut short = b"STMD".to_vec();

        short.extend([101, 0]);
        assert!(read(&short).unwrap_err().contains("ends"));
    }

    #[test]
    fn a_skeleton_id_is_written_read_and_removed_without_touching_the_rest() {
        let Some(path) = games_model() else { return };
        let bytes = std::fs::read(&path).unwrap();
        let before = read(&bytes).unwrap();
        let with = set_skeleton_id(&bytes, Some("pleasurehorizons:jenny")).unwrap();
        let after = read(&with).unwrap();

        assert_eq!(after.skeleton_id.as_deref(), Some("pleasurehorizons:jenny"));
        assert_eq!(after.group_bones, before.group_bones);
        assert_eq!(after.cubes, before.cubes);
        assert_eq!(after.meshes, before.meshes);
        assert!(after.extensions.iter().filter(|e| *e == SKELETON_EXTENSION).count() == 1);

        let changed = set_skeleton_id(&with, Some("pleasurehorizons:bia")).unwrap();

        assert_eq!(read(&changed).unwrap().skeleton_id.as_deref(), Some("pleasurehorizons:bia"));
        assert_eq!(read(&changed).unwrap().extensions.len(), after.extensions.len());

        let without = set_skeleton_id(&with, None).unwrap();

        assert_eq!(read(&without).unwrap().skeleton_id, None);
        assert_eq!(without, set_skeleton_id(&bytes, None).unwrap());
        assert!(set_skeleton_id(&bytes, Some("")).is_err());
    }

    fn games_model() -> Option<PathBuf> {
        let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../src/main/resources/assets/pleasurehorizons/strut/models");
        let mut stack = vec![folder];

        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(dir).ok()?.flatten() {
                let path = entry.path();

                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().map_or(false, |e| e == "strutmodel") {
                    return Some(path);
                }
            }
        }

        None
    }

    #[test]
    fn reads_the_games_own_models() {
        let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../src/main/resources/assets/pleasurehorizons/strut/models");

        if !folder.is_dir() {
            return;
        }

        let mut seen = 0;
        let mut stack = vec![folder];

        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();

                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().map_or(false, |e| e == "strutmodel") {
                    let model = read(&std::fs::read(&path).unwrap()).unwrap_or_else(|e| panic!("{}: {e}", path.display()));

                    assert!(!model.group_bones.is_empty() || model.cubes.is_empty(), "{}", path.display());

                    for bone in &model.group_bones {
                        for element in &bone.elements {
                            let count = if element.kind == 0 { model.meshes.len() } else { model.cubes.len() };

                            assert!((element.index as usize) < count, "{}: bone {} names an element that is not there", path.display(), bone.name);
                        }
                    }

                    seen += 1;
                }
            }
        }

        assert!(seen > 10, "read {seen} models");
    }

    #[test]
    fn what_the_converter_writes_reads_back() {
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../asset_sources/geo/nude/lucy.geo.json");

        if !source.exists() {
            return;
        }

        let converted = crate::convert::model::convert_geo(&serde_json::from_str(&std::fs::read_to_string(source).unwrap()).unwrap(), None);
        let model = read(&crate::convert::model::encode(&converted.model).unwrap()).unwrap();

        assert_eq!(model.group_bones.len(), converted.model.group_bones.len());
        assert_eq!(model.cubes.len(), converted.model.cubes.len());
        assert_eq!(model.group_bones[0].name, converted.model.group_bones[0].name);
        assert_eq!(model.cubes[10].faces.len(), converted.model.cubes[10].faces.len());
        assert_eq!(model.units_per_block, 16.0);
    }
}

#[cfg(test)]
mod survey {
    #[test]
    #[ignore]
    fn which_models_have_meshes() {
        let folder = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../src/main/resources/assets/pleasurehorizons/strut/models");
        let mut stack = vec![folder];

        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();

                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().map_or(false, |e| e == "strutmodel") {
                    let model = super::read(&std::fs::read(&path).unwrap()).unwrap();

                    let referenced: std::collections::HashSet<u16> = model.group_bones.iter().flat_map(|b| b.elements.iter().filter(|e| e.kind == 0).map(|e| e.index)).collect();

                    println!("SURVEY unreferenced={} {} bones={} cubes={} meshes={} armatures={} faces_in_meshes={}", model.meshes.len() - referenced.len(), path.file_name().unwrap().to_string_lossy(), model.group_bones.len(), model.cubes.len(), model.meshes.len(), model.armature_count, model.meshes.iter().map(|m| m.faces.len()).sum::<usize>());
                }
            }
        }
    }
}
