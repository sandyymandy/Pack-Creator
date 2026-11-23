//! A reader for Strut's binary animation files (`.strutanim`), as far as the checks need: the clips (their names, lengths and how they loop) and the events in them (the
//! cue markers, sounds and so on). The keyframes are read only to be skipped. The layout is Strut's `BinaryAnimationReader`; keep the two in step.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Event {
    pub time: f32,
    pub kind: String,
    pub payload: String,
    /// Who decides that it fires: 0 lets the event type decide, 1 the server, 2 the client, 3 both.
    pub authority: u8,
}

/// Three components, each a number or a Molang expression (the viewer reads an expression as 0).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Value3 {
    pub literals: [f32; 3],
    pub expressions: [Option<String>; 3],
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Key {
    pub time: f32,
    /// 0 step, 1 linear, 2 catmull-rom, 3 bezier.
    pub interpolation: u8,
    pub value: Value3,
    /// The outgoing value, where a key holds a different one on each side of its time.
    pub post: Option<Value3>,
    pub easing: Option<String>,
    pub easing_args: Vec<f32>,
    /// A bezier key's handles, twelve numbers: left time, left value, right time and right value, three axes each. None for any other kind of key.
    pub bezier: Option<Vec<f32>>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Channel {
    pub target: String,
    /// GROUP_BONE or ARMATURE_BONE: the same name can be both.
    pub kind: String,
    /// ROTATION, POSITION or SCALE.
    pub property: String,
    pub keys: Vec<Key>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub name: String,
    pub duration: f32,
    /// 0 plays once, 1 loops, 2 holds the last frame.
    pub loop_mode: u8,
    pub channels: Vec<Channel>,
    pub events: Vec<Event>,
}

const MAGIC: &[u8; 4] = b"STAM";
const HEADER_BYTES: usize = 32;
const FIRST_VERSION: u16 = 100;
const LAST_VERSION: u16 = 100;

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize, what: &str) -> Result<&'a [u8], String> {
        if self.at + count > self.bytes.len() {
            return Err(format!("the file ends inside {what} (the file is cut short or damaged)"));
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

    fn u32(&mut self, what: &str) -> Result<u32, String> {
        let b = self.take(4, what)?;

        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn f32(&mut self, what: &str) -> Result<f32, String> {
        Ok(f32::from_bits(self.u32(what)?))
    }

    fn string(&mut self, what: &str) -> Result<String, String> {
        let length = self.u16(what)? as usize;

        Ok(String::from_utf8_lossy(self.take(length, what)?).to_string())
    }
}

/* Where each clip sits in the file, so its events can be rewritten without touching a keyframe. */
struct Span {
    /* The start of the clip: its header with the name, length, loop mode and the two counts. */
    start: usize,
    channels_start: usize,
    events_start: usize,
    end: usize,
    /* Where each channel's record starts and ends. */
    channel_spans: Vec<(usize, usize)>,
}

struct Parsed {
    version_and_flags: [u8; 4],
    fingerprint_and_rest: Vec<u8>,
    strings: Vec<String>,
    clips: Vec<Clip>,
    spans: Vec<Span>,
}

/// Reads the clips of an animation file, or says what is wrong with it.
pub fn read(bytes: &[u8]) -> Result<Vec<Clip>, String> {
    parse(bytes).map(|parsed| parsed.clips)
}

fn parse(bytes: &[u8]) -> Result<Parsed, String> {
    if bytes.is_empty() {
        return Err("the file is empty".into());
    }

    let mut r = Reader { bytes, at: 0 };

    if r.take(4, "the start of the file")? != MAGIC {
        return Err("this is not a Strut binary animation (.strutanim): it does not start with STAM. Convert it with Strut's converter".into());
    }

    let version = r.u16("the version")?;

    if version < FIRST_VERSION || version > LAST_VERSION {
        return Err(format!("this is animation format version {version}, and the game reads versions {FIRST_VERSION} to {LAST_VERSION}. Convert it again with a matching Strut"));
    }

    let flags = r.u16("the flags")?;

    let string_count = r.u16("the string count")? as usize;

    r.u32("the string table size")?;

    let clip_count = r.u16("the clip count")? as usize;

    let rest = r.take(4 + 12, "the rest of the header")?.to_vec();

    if r.at != HEADER_BYTES {
        return Err("the header is the wrong size".into());
    }

    let mut strings = Vec::with_capacity(string_count);

    for _ in 0..string_count {
        strings.push(r.string("a string")?);
    }

    let name = |index: u16, strings: &[String]| -> Result<String, String> {
        strings.get(index as usize).cloned().ok_or_else(|| format!("a name points at string {index}, but the file has {} strings", strings.len()))
    };

    let mut clips = Vec::with_capacity(clip_count);
    let mut spans = Vec::with_capacity(clip_count);

    for _ in 0..clip_count {
        let start = r.at;
        let clip_name = name(r.u16("a clip name")?, &strings)?;
        let duration = r.f32("a clip length")?;
        let loop_mode = r.u8("a loop mode")?;
        let channel_count = r.u16("a channel count")?;
        let event_count = r.u16("an event count")?;
        let channels_start = r.at;

        let mut channels = Vec::with_capacity(channel_count as usize);
        let mut channel_spans = Vec::with_capacity(channel_count as usize);

        for _ in 0..channel_count {
            let channel_start = r.at;
            let target = name(r.u16("a channel target")?, &strings)?;

            let kind = if r.u8("a channel target kind")? == 1 { "ARMATURE_BONE" } else { "GROUP_BONE" }.to_string();

            let property = match r.u8("a channel property")? {
                0 => "ROTATION",
                1 => "POSITION",
                _ => "SCALE",
            }
            .to_string();

            r.u8("a channel interpolation")?;

            let count = r.u16("a keyframe count")?;
            let mut keys = Vec::with_capacity(count as usize);

            let value = |r: &mut Reader, flags: u16, bit: u16, strings: &[String]| -> Result<Value3, String> {
                let mut literals = [0.0f32; 3];
                let mut expressions: [Option<String>; 3] = [None, None, None];

                for component in 0..3 {
                    if flags & (bit << component) != 0 {
                        expressions[component] = Some(name(r.u16("an expression")?, strings)?);
                    } else {
                        literals[component] = r.f32("a keyframe value")?;
                    }
                }

                Ok(Value3 { literals, expressions })
            };

            for _ in 0..count {
                let time = r.f32("a keyframe time")?;
                let interpolation = r.u8("a keyframe interpolation")?;
                let flags = r.u16("keyframe flags")?;
                let current = value(&mut r, flags, 0x0004, &strings)?;
                let post = if flags & 0x0001 != 0 { Some(value(&mut r, flags, 0x0020, &strings)?) } else { None };
                let mut easing = None;
                let mut easing_args = Vec::new();

                if flags & 0x0002 != 0 {
                    easing = Some(name(r.u16("an easing name")?, &strings)?);

                    let args = r.u8("an easing argument count")?;

                    for _ in 0..args {
                        easing_args.push(r.f32("an easing argument")?);
                    }
                }

                let mut bezier = None;

                if interpolation == 3 {
                    let mut handles = Vec::with_capacity(12);

                    for _ in 0..12 {
                        handles.push(r.f32("a bezier handle")?);
                    }

                    bezier = Some(handles);
                }

                keys.push(Key { time, interpolation, value: current, post, easing, easing_args, bezier });
            }

            channels.push(Channel { target, kind, property, keys });
            channel_spans.push((channel_start, r.at));
        }

        let events_start = r.at;
        let mut events = Vec::with_capacity(event_count as usize);

        for _ in 0..event_count {
            let time = r.f32("an event time")?;
            let kind = name(r.u16("an event kind")?, &strings)?;
            let payload = name(r.u16("an event payload")?, &strings)?;

            let authority = r.u8("an event authority")?;

            events.push(Event { time, kind, payload, authority });
        }

        clips.push(Clip { name: clip_name, duration, loop_mode, channels, events });
        spans.push(Span { start, channels_start, events_start, end: r.at, channel_spans });
    }

    Ok(Parsed { version_and_flags: [(version & 0xFF) as u8, (version >> 8) as u8, (flags & 0xFF) as u8, (flags >> 8) as u8], fingerprint_and_rest: rest, strings, clips, spans })
}

/// The header flag that says the file names the skeleton its clips were made for.
const FLAG_SKELETON_ID: u16 = 1;
/// Where, in the file, the string-table index of that id sits (the first two of the twelve reserved header bytes, after the 4-byte fingerprint).
const SKELETON_INDEX_AT: usize = 20;

/// The skeleton id the clips of this file were made for (`mymod:creature_a`), or None when the file names none.
pub fn skeleton_id(bytes: &[u8]) -> Result<Option<String>, String> {
    let parsed = parse(bytes)?;
    let flags = u16::from_le_bytes([parsed.version_and_flags[2], parsed.version_and_flags[3]]);

    if flags & FLAG_SKELETON_ID == 0 {
        return Ok(None);
    }

    let index = u16::from_le_bytes([parsed.fingerprint_and_rest[SKELETON_INDEX_AT - 16], parsed.fingerprint_and_rest[SKELETON_INDEX_AT - 16 + 1]]) as usize;

    parsed.strings.get(index).cloned().map(Some).ok_or_else(|| format!("the file names its skeleton by string {index}, but it has {} strings", parsed.strings.len()))
}

/// Writes the file again with the skeleton id set (or taken away with None). The id is added to the string table (or found in it) and the header says where; every clip is
/// copied byte for byte.
pub fn set_skeleton_id(bytes: &[u8], id: Option<&str>) -> Result<Vec<u8>, String> {
    let parsed = parse(bytes)?;
    let mut strings = parsed.strings.clone();
    let mut flags = u16::from_le_bytes([parsed.version_and_flags[2], parsed.version_and_flags[3]]);
    let mut rest = parsed.fingerprint_and_rest.clone();

    match id {
        Some(id) => {
            if id.is_empty() || id.len() > 0xFFFF || strings.len() >= 0xFFFF {
                return Err("a skeleton id has to be between 1 and 65535 bytes".into());
            }

            let index = match strings.iter().position(|s| s == id) {
                Some(at) => at,
                None => {
                    strings.push(id.to_string());
                    strings.len() - 1
                }
            };

            flags |= FLAG_SKELETON_ID;
            rest[SKELETON_INDEX_AT - 16..SKELETON_INDEX_AT - 14].copy_from_slice(&(index as u16).to_le_bytes());
        }
        None => {
            flags &= !FLAG_SKELETON_ID;
            rest[SKELETON_INDEX_AT - 16..SKELETON_INDEX_AT - 14].copy_from_slice(&[0, 0]);
        }
    }

    let mut table: Vec<u8> = Vec::new();

    for value in &strings {
        table.extend_from_slice(&(value.len() as u16).to_le_bytes());
        table.extend_from_slice(value.as_bytes());
    }

    let mut out: Vec<u8> = Vec::with_capacity(bytes.len() + 64);

    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&parsed.version_and_flags[..2]);
    out.extend_from_slice(&flags.to_le_bytes());
    out.extend_from_slice(&(strings.len() as u16).to_le_bytes());
    out.extend_from_slice(&(table.len() as u32).to_le_bytes());
    out.extend_from_slice(&(parsed.clips.len() as u16).to_le_bytes());
    out.extend_from_slice(&rest);
    out.extend_from_slice(&table);

    if let Some(first) = parsed.spans.first() {
        out.extend_from_slice(&bytes[first.start..parsed.spans.last().map_or(first.start, |l| l.end)]);
    }

    Ok(out)
}

/// The file with only the named clips, each copied byte for byte, and the skeleton id given (or none). The clips keep their order in the file.
pub fn subset(bytes: &[u8], names: &[String], skeleton: Option<&str>) -> Result<Vec<u8>, String> {
    let parsed = parse(bytes)?;
    let mut strings = parsed.strings.clone();
    let mut body: Vec<u8> = Vec::new();
    let mut count = 0;

    for (clip, span) in parsed.clips.iter().zip(&parsed.spans) {
        if names.contains(&clip.name) {
            body.extend_from_slice(&bytes[span.start..span.end]);
            count += 1;
        }
    }

    if count == 0 {
        return Err("none of those clips is in the file".into());
    }

    if let Some(id) = skeleton {
        if !strings.iter().any(|s| s == id) {
            strings.push(id.to_string());
        }
    }

    let layout = layout(bytes)?;

    Ok(assemble(&layout, &strings, &body, count, None, skeleton))
}

/// The skeleton fingerprint in the file's header: the hash of the bone names of the model the clips were made for (the game compares it when the file names no skeleton id).
pub fn fingerprint(bytes: &[u8]) -> Result<i32, String> {
    parse(bytes)?;

    Ok(i32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]))
}

/// The file with another fingerprint in its header; every other byte is as it was.
pub fn set_fingerprint(bytes: &[u8], fingerprint: i32) -> Result<Vec<u8>, String> {
    parse(bytes)?;

    let mut out = bytes.to_vec();

    out[16..20].copy_from_slice(&fingerprint.to_le_bytes());
    Ok(out)
}

/// Where the clips and their channels sit in a file, for tools that copy them into another file.
pub struct ClipSpan {
    pub start: usize,
    pub channels_start: usize,
    pub events_start: usize,
    pub end: usize,
    pub channel_spans: Vec<(usize, usize)>,
}

pub struct Layout {
    pub strings: Vec<String>,
    pub clips: Vec<ClipSpan>,
    version_and_flags: [u8; 4],
    fingerprint_and_rest: Vec<u8>,
}

/// The string table and where every clip and channel is.
pub fn layout(bytes: &[u8]) -> Result<Layout, String> {
    let parsed = parse(bytes)?;

    Ok(Layout {
        strings: parsed.strings,
        clips: parsed.spans.into_iter().map(|s| ClipSpan { start: s.start, channels_start: s.channels_start, events_start: s.events_start, end: s.end, channel_spans: s.channel_spans }).collect(),
        version_and_flags: parsed.version_and_flags,
        fingerprint_and_rest: parsed.fingerprint_and_rest,
    })
}

/// Writes a file from a string table and clip records, with the header of the file `layout` was made from: a new fingerprint when one is given, and the skeleton id (which has
/// to be in the table) or none.
pub fn assemble(layout: &Layout, strings: &[String], body: &[u8], clips: usize, fingerprint: Option<i32>, skeleton: Option<&str>) -> Vec<u8> {
    let mut table: Vec<u8> = Vec::new();

    for value in strings {
        table.extend_from_slice(&(value.len() as u16).to_le_bytes());
        table.extend_from_slice(value.as_bytes());
    }

    let mut flags = u16::from_le_bytes([layout.version_and_flags[2], layout.version_and_flags[3]]) & !FLAG_SKELETON_ID;
    let mut rest = layout.fingerprint_and_rest.clone();

    if let Some(f) = fingerprint {
        rest[..4].copy_from_slice(&f.to_le_bytes());
    }

    rest[SKELETON_INDEX_AT - 16..SKELETON_INDEX_AT - 14].copy_from_slice(&[0, 0]);

    if let Some(id) = skeleton {
        flags |= FLAG_SKELETON_ID;
        rest[SKELETON_INDEX_AT - 16..SKELETON_INDEX_AT - 14].copy_from_slice(&(strings.iter().position(|s| s == id).unwrap_or(0) as u16).to_le_bytes());
    }

    let mut out: Vec<u8> = Vec::with_capacity(HEADER_BYTES + table.len() + body.len());

    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&layout.version_and_flags[..2]);
    out.extend_from_slice(&flags.to_le_bytes());
    out.extend_from_slice(&(strings.len() as u16).to_le_bytes());
    out.extend_from_slice(&(table.len() as u32).to_le_bytes());
    out.extend_from_slice(&(clips as u16).to_le_bytes());
    out.extend_from_slice(&rest);
    out.extend_from_slice(&table);
    out.extend_from_slice(body);
    out
}

/// The events one clip should have after an edit.
pub struct ClipEvents {
    pub clip: String,
    pub events: Vec<Event>,
}

/// Writes the file again with the events of the named clips replaced. Everything else is copied byte for byte: the keyframes, the loop modes and the strings already in
/// the table keep their places (new strings are added at the end), so an edit of a cue cannot disturb the motion.
pub fn edit_events(bytes: &[u8], edits: &[ClipEvents]) -> Result<Vec<u8>, String> {
    let parsed = parse(bytes)?;
    let mut strings = parsed.strings.clone();

    let index_of = |value: &str, strings: &mut Vec<String>| -> Result<u16, String> {
        if let Some(at) = strings.iter().position(|s| s == value) {
            return Ok(at as u16);
        }

        if value.len() > 0xFFFF || strings.len() >= 0xFFFF {
            return Err("a cue is too long for the format".into());
        }

        strings.push(value.to_string());
        Ok((strings.len() - 1) as u16)
    };

    for edit in edits {
        if !parsed.clips.iter().any(|c| c.name == edit.clip) {
            return Err(format!("this animation file has no clip called \"{}\"", edit.clip));
        }
    }

    let mut body: Vec<u8> = Vec::new();

    for (clip, span) in parsed.clips.iter().zip(&parsed.spans) {
        match edits.iter().find(|e| e.clip == clip.name) {
            None => body.extend_from_slice(&bytes[span.start..span.end]),
            Some(edit) => {
                /* The header keeps everything but the event count, which is its last two bytes. */
                body.extend_from_slice(&bytes[span.start..span.channels_start - 2]);
                body.extend_from_slice(&(edit.events.len() as u16).to_le_bytes());
                body.extend_from_slice(&bytes[span.channels_start..span.events_start]);

                let mut sorted: Vec<&Event> = edit.events.iter().collect();

                sorted.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal));

                for event in sorted {
                    let kind = index_of(&event.kind, &mut strings)?;
                    let payload = index_of(&event.payload, &mut strings)?;

                    body.extend_from_slice(&event.time.to_le_bytes());
                    body.extend_from_slice(&kind.to_le_bytes());
                    body.extend_from_slice(&payload.to_le_bytes());
                    body.push(event.authority);
                }
            }
        }
    }

    let mut table: Vec<u8> = Vec::new();

    for value in &strings {
        table.extend_from_slice(&(value.len() as u16).to_le_bytes());
        table.extend_from_slice(value.as_bytes());
    }

    let mut out: Vec<u8> = Vec::with_capacity(HEADER_BYTES + table.len() + body.len());

    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&parsed.version_and_flags);
    out.extend_from_slice(&(strings.len() as u16).to_le_bytes());
    out.extend_from_slice(&(table.len() as u32).to_le_bytes());
    out.extend_from_slice(&(parsed.clips.len() as u16).to_le_bytes());
    out.extend_from_slice(&parsed.fingerprint_and_rest);
    out.extend_from_slice(&table);
    out.extend_from_slice(&body);
    Ok(out)
}

/// Writes the file again without the named clips. Every clip that stays is copied byte for byte (keyframes, loop modes, cues), so removing one cannot disturb the others. A file
/// has to keep at least one clip: to remove them all, delete the file.
pub fn remove_clips(bytes: &[u8], names: &[String]) -> Result<Vec<u8>, String> {
    let parsed = parse(bytes)?;

    for name in names {
        if !parsed.clips.iter().any(|c| &c.name == name) {
            return Err(format!("this animation file has no clip called \"{name}\""));
        }
    }

    let kept: Vec<usize> = (0..parsed.clips.len()).filter(|i| !names.contains(&parsed.clips[*i].name)).collect();

    if kept.is_empty() {
        return Err("that would remove every clip of the file; delete the file instead".into());
    }

    let mut body: Vec<u8> = Vec::new();

    for i in &kept {
        body.extend_from_slice(&bytes[parsed.spans[*i].start..parsed.spans[*i].end]);
    }

    let mut table: Vec<u8> = Vec::new();

    for value in &parsed.strings {
        table.extend_from_slice(&(value.len() as u16).to_le_bytes());
        table.extend_from_slice(value.as_bytes());
    }

    let mut out: Vec<u8> = Vec::with_capacity(HEADER_BYTES + table.len() + body.len());

    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&parsed.version_and_flags);
    out.extend_from_slice(&(parsed.strings.len() as u16).to_le_bytes());
    out.extend_from_slice(&(table.len() as u32).to_le_bytes());
    out.extend_from_slice(&(kept.len() as u16).to_le_bytes());
    out.extend_from_slice(&parsed.fingerprint_and_rest);
    out.extend_from_slice(&table);
    out.extend_from_slice(&body);
    Ok(out)
}

/// Writes the file again with clips under other names (`renames` is old name, new name). The new names are added to the end of the string table and the clips are pointed at
/// them, so a name that is also used as something else in the file (a cue's text, a bone) is not changed there. Everything else is copied byte for byte.
pub fn rename_clips(bytes: &[u8], renames: &[(String, String)]) -> Result<Vec<u8>, String> {
    let parsed = parse(bytes)?;

    for (old, new) in renames {
        if !parsed.clips.iter().any(|c| &c.name == old) {
            return Err(format!("this animation file has no clip called \"{old}\""));
        }

        if new.trim().is_empty() || new.len() > 0xFFFF {
            return Err(format!("\"{old}\" cannot be renamed to nothing"));
        }
    }

    let after: Vec<String> = parsed.clips.iter().map(|c| renames.iter().find(|(old, _)| *old == c.name).map_or(c.name.clone(), |(_, new)| new.trim().to_string())).collect();

    for (i, name) in after.iter().enumerate() {
        if after.iter().skip(i + 1).any(|other| other == name) {
            return Err(format!("two clips would be called \"{name}\""));
        }
    }

    let mut strings = parsed.strings.clone();
    let first = parsed.spans.first().map(|s| s.start).ok_or("the file has no clips")?;
    let mut body = bytes[first..].to_vec();

    for (i, clip) in parsed.clips.iter().enumerate() {
        if after[i] == clip.name {
            continue;
        }

        let index = match strings.iter().position(|s| *s == after[i]) {
            Some(at) => at,
            None => {
                strings.push(after[i].clone());
                strings.len() - 1
            }
        };

        let at = parsed.spans[i].start - first;

        body[at..at + 2].copy_from_slice(&(index as u16).to_le_bytes());
    }

    let mut table: Vec<u8> = Vec::new();

    for value in &strings {
        table.extend_from_slice(&(value.len() as u16).to_le_bytes());
        table.extend_from_slice(value.as_bytes());
    }

    let mut out: Vec<u8> = Vec::with_capacity(HEADER_BYTES + table.len() + body.len());

    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&parsed.version_and_flags);
    out.extend_from_slice(&(strings.len() as u16).to_le_bytes());
    out.extend_from_slice(&(table.len() as u32).to_le_bytes());
    out.extend_from_slice(&(parsed.clips.len() as u16).to_le_bytes());
    out.extend_from_slice(&parsed.fingerprint_and_rest);
    out.extend_from_slice(&table);
    out.extend_from_slice(&body);
    read(&out).map_err(|e| format!("the renamed file did not read back: {e}"))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn refuses_what_is_not_an_animation() {
        assert!(read(b"").is_err());
        assert!(read(b"NOPE and more bytes here to be long enough......").unwrap_err().contains("STAM"));

        let mut short = b"STAM".to_vec();

        short.extend([100, 0, 0, 0]);
        assert!(read(&short).unwrap_err().contains("ends"));
    }

    fn games_animations() -> Vec<PathBuf> {
        let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../src/main/resources/assets/pleasurehorizons/strut/animations");
        let mut found = Vec::new();
        let mut stack = vec![folder];

        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(dir) else { continue };

            for entry in entries.flatten() {
                let path = entry.path();

                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().map_or(false, |e| e == "strutanim") {
                    found.push(path);
                }
            }
        }

        found
    }

    #[test]
    fn a_skeleton_id_is_written_read_and_removed_without_touching_the_clips() {
        let Some(path) = games_animations().into_iter().next() else { return };
        let bytes = std::fs::read(&path).unwrap();
        let clips = read(&bytes).unwrap();
        let with = set_skeleton_id(&bytes, Some("pleasurehorizons:jenny")).unwrap();

        assert_eq!(skeleton_id(&bytes).unwrap(), None);
        assert_eq!(skeleton_id(&with).unwrap().as_deref(), Some("pleasurehorizons:jenny"));
        assert_eq!(read(&with).unwrap(), clips);

        /* The fingerprint stays as it was, and a later cue edit keeps the id. */
        assert_eq!(&with[16..20], &bytes[16..20]);

        let edited = edit_events(&with, &[ClipEvents { clip: clips[0].name.clone(), events: clips[0].events.clone() }]).unwrap();

        assert_eq!(skeleton_id(&edited).unwrap().as_deref(), Some("pleasurehorizons:jenny"));

        let changed = set_skeleton_id(&with, Some("pleasurehorizons:bia")).unwrap();

        assert_eq!(skeleton_id(&changed).unwrap().as_deref(), Some("pleasurehorizons:bia"));
        assert_eq!(read(&changed).unwrap(), clips);
        assert_eq!(set_skeleton_id(&with, None).unwrap().len(), set_skeleton_id(&with, None).unwrap().len());
        assert_eq!(skeleton_id(&set_skeleton_id(&with, None).unwrap()).unwrap(), None);
        assert!(set_skeleton_id(&bytes, Some("")).is_err());
    }

    #[test]
    fn removing_clips_keeps_the_others_exactly() {
        let path = games_animations().into_iter().find(|p| read(&std::fs::read(p).unwrap()).map_or(false, |c| c.len() >= 3)).expect("a game file with three clips");
        let bytes = std::fs::read(&path).unwrap();
        let clips = read(&bytes).unwrap();
        let gone = vec![clips[1].name.clone()];
        let smaller = remove_clips(&bytes, &gone).unwrap();
        let after = read(&smaller).unwrap();

        assert_eq!(after.len(), clips.len() - 1);
        assert!(after.iter().all(|c| c.name != gone[0]));
        assert_eq!(after[0], clips[0]);
        assert_eq!(after.last().unwrap(), clips.last().unwrap());
        assert!(remove_clips(&bytes, &["nope".to_string()]).is_err());
        assert!(remove_clips(&bytes, &clips.iter().map(|c| c.name.clone()).collect::<Vec<_>>()).is_err());
    }

    #[test]
    fn rewriting_the_events_unchanged_gives_the_same_file() {
        for path in games_animations() {
            let bytes = std::fs::read(&path).unwrap();
            let edits: Vec<ClipEvents> = read(&bytes).unwrap().into_iter().map(|c| ClipEvents { clip: c.name, events: c.events }).collect();
            let again = edit_events(&bytes, &edits).unwrap();

            /* Events are written by time, so a file whose events are out of order would differ; the game's own files are in order. */
            assert!(again == bytes, "{} changed when its events were rewritten as they were", path.display());
        }
    }

    #[test]
    fn an_edit_changes_only_the_events() {
        let Some(path) = games_animations().into_iter().find(|p| read(&std::fs::read(p).unwrap()).unwrap().iter().any(|c| !c.channels.is_empty())) else { return };
        let bytes = std::fs::read(&path).unwrap();
        let before = read(&bytes).unwrap();
        let target = before.iter().find(|c| !c.channels.is_empty()).unwrap().name.clone();
        let mut events = before.iter().find(|c| c.name == target).unwrap().events.clone();

        events.push(Event { time: 0.25, kind: "instruction".into(), payload: "a-cue-nobody-has".into(), authority: 0 });

        let after = read(&edit_events(&bytes, &[ClipEvents { clip: target.clone(), events }]).unwrap()).unwrap();

        for (a, b) in before.iter().zip(&after) {
            assert_eq!(a.channels, b.channels);
            assert_eq!(a.duration, b.duration);
            assert_eq!(a.loop_mode, b.loop_mode);

            if a.name == target {
                assert_eq!(b.events.len(), a.events.len() + 1);
                assert!(b.events.iter().any(|e| e.payload == "a-cue-nobody-has" && e.time == 0.25));
            } else {
                assert_eq!(a.events, b.events);
            }
        }

        assert!(edit_events(&bytes, &[ClipEvents { clip: "no such clip".into(), events: vec![] }]).is_err());
    }

    #[test]
    fn reads_the_games_own_animations() {
        let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../src/main/resources/assets/pleasurehorizons/strut/animations");

        if !folder.is_dir() {
            return;
        }

        let mut seen = 0;
        let mut events = 0;
        let mut kinds = std::collections::BTreeSet::new();

        let mut stack = vec![folder];

        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();

                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().map_or(false, |e| e == "strutanim") {
                    let clips = read(&std::fs::read(&path).unwrap()).unwrap_or_else(|e| panic!("{}: {e}", path.display()));

                    seen += 1;

                    for clip in clips {
                        for event in clip.events {
                            events += 1;
                            kinds.insert(event.kind);
                        }
                    }
                }
            }
        }

        println!("read {seen} animation files with {events} events of kinds {kinds:?}");
        assert!(seen > 0);
    }
}

/// Writes out every cue name the game's own animations fire (for `reference.json`'s `cues.known`): run it with `cargo test -p pc_core -- --ignored --nocapture tokens`.
#[cfg(test)]
mod vocab {
    use std::collections::BTreeSet;
    #[test]
    #[ignore]
    fn tokens() {
        let assets = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../src/main/resources/assets/pleasurehorizons");
        if !assets.exists() { return; }
        let mut bound = BTreeSet::new();
        for d in ["strut/event_bindings/girls"] {
            for e in std::fs::read_dir(assets.join(d)).unwrap().flatten() {
                let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(e.path()).unwrap()).unwrap();
                if let Some(b) = v.get("bindings").and_then(|b| b.as_object()) { for k in b.keys() { bound.insert(k.to_lowercase()); } }
                if let Some(a) = v.get("events").and_then(|b| b.as_array()) { for x in a { if let Some(k) = x.get("key").and_then(|k| k.as_str()) { bound.insert(k.to_lowercase()); } } }
            }
        }
        let mut unbound = BTreeSet::new();
        let mut stack = vec![assets.join("strut/animations")];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() { stack.push(p); continue; }
                if p.extension().map_or(true, |x| x != "strutanim") { continue; }
                for clip in super::read(&std::fs::read(&p).unwrap()).unwrap() {
                    for ev in clip.events { if ev.kind == "instruction" { for t in ev.payload.to_lowercase().split('-') { unbound.insert(t.to_string()); } } }
                }
            }
        }
        println!("TOKENS {}", unbound.iter().cloned().collect::<Vec<_>>().join(" "));
        println!("BOUNDCOUNT {}", bound.len());
    }
}

#[cfg(test)]
mod rename_probe {
    /// `RENAME_FILE=<a .strutanim> cargo test -p pc_core --lib -- --ignored --nocapture rename_a_file`
    #[test]
    #[ignore]
    fn rename_a_file() {
        let bytes = std::fs::read(std::env::var("RENAME_FILE").unwrap()).unwrap();
        let before = super::read(&bytes).unwrap();
        let renames: Vec<(String, String)> = before.iter().map(|c| (c.name.clone(), format!("animation.{}", c.name.replace(".start", ".intro")))).collect();
        let out = super::rename_clips(&bytes, &renames).unwrap();
        let after = super::read(&out).unwrap();

        for (a, b) in before.iter().zip(&after) {
            println!("PROBE {} -> {} ({} channels, {} events, {} s)", a.name, b.name, b.channels.len(), b.events.len(), b.duration);
            assert_eq!(a.channels.len(), b.channels.len());
            println!("PROBE targets {:?}", a.channels.iter().map(|c| c.target.clone()).filter(|t| ["steve", "Torso2", "Head2", "cock", "nut", "shaft", "RightArm", "LeftLeg"].contains(&t.as_str())).collect::<Vec<_>>());
            assert_eq!(a.events.len(), b.events.len());
        }
    }
}
