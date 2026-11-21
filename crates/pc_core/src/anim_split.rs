//! Splitting one scene's animation file in two: the host's (the girl) and the partner's. The channels of the partner's bones go to a file of their own, the rest stay with the
//! host, and every clip is in both with its name, length and loop mode, so the two halves stay one clip. Markers stay with the host. Every channel is copied byte for byte except
//! the partner root's position, which takes a shift (and is made, as a constant, in a clip that had none) so that the partner, rested at its own origin, is posed exactly where
//! the host's model had posed it.

use crate::strut_anim::{self, Key, Value3};
use std::collections::HashSet;

/// How to split a scene file.
pub struct SplitSpec<'a> {
    /// The bones of the partner's rig, the root included.
    pub partner_bones: &'a HashSet<String>,
    /// The partner's root bone.
    pub root: &'a str,
    /// What moves the partner's root from where the host's model had it to its own origin, added to the root's position in every clip (sixteenths of a block).
    pub shift: [f32; 3],
    pub partner_fingerprint: i32,
    pub host_id: Option<&'a str>,
    pub partner_id: Option<&'a str>,
}

pub struct Split {
    pub host: Vec<u8>,
    pub partner: Vec<u8>,
    /// What could not be carried over exactly.
    pub notes: Vec<String>,
}

fn index_of(strings: &mut Vec<String>, value: &str) -> u16 {
    match strings.iter().position(|s| s == value) {
        Some(at) => at as u16,
        None => {
            strings.push(value.to_string());
            (strings.len() - 1) as u16
        }
    }
}

fn put_key(out: &mut Vec<u8>, key: &Key, strings: &mut Vec<String>) {
    let mut flags: u16 = 0;

    if key.post.is_some() {
        flags |= 0x0001;
    }

    if key.easing.is_some() {
        flags |= 0x0002;
    }

    for c in 0..3 {
        if key.value.expressions[c].is_some() {
            flags |= 0x0004 << c;
        }

        if key.post.as_ref().map_or(false, |p| p.expressions[c].is_some()) {
            flags |= 0x0020 << c;
        }
    }

    out.extend_from_slice(&key.time.to_le_bytes());
    out.push(key.interpolation);
    out.extend_from_slice(&flags.to_le_bytes());

    for value in std::iter::once(&key.value).chain(key.post.iter()) {
        for c in 0..3 {
            match &value.expressions[c] {
                Some(e) => out.extend_from_slice(&index_of(strings, e).to_le_bytes()),
                None => out.extend_from_slice(&value.literals[c].to_le_bytes()),
            }
        }
    }

    if let Some(easing) = &key.easing {
        out.extend_from_slice(&index_of(strings, easing).to_le_bytes());
        out.push(key.easing_args.len() as u8);

        for a in &key.easing_args {
            out.extend_from_slice(&a.to_le_bytes());
        }
    }

    if let Some(handles) = &key.bezier {
        for h in handles {
            out.extend_from_slice(&h.to_le_bytes());
        }
    }
}

/// A channel record for a group bone's property with these keys.
fn put_channel(out: &mut Vec<u8>, target: &str, property: u8, interpolation: u8, keys: &[Key], strings: &mut Vec<String>) {
    out.extend_from_slice(&index_of(strings, target).to_le_bytes());
    out.push(0);
    out.push(property);
    out.push(interpolation);
    out.extend_from_slice(&(keys.len() as u16).to_le_bytes());

    for key in keys {
        put_key(out, key, strings);
    }
}

fn literal_key(time: f32, value: [f32; 3]) -> Key {
    Key { time, interpolation: 1, value: Value3 { literals: value, expressions: [None, None, None] }, post: None, easing: None, easing_args: vec![], bezier: None }
}

/// Splits one animation file in two.
pub fn split(bytes: &[u8], spec: &SplitSpec) -> Result<Split, String> {
    let layout = strut_anim::layout(bytes)?;
    let clips = strut_anim::read(bytes)?;
    let mut host_strings = layout.strings.clone();
    let mut partner_strings = layout.strings.clone();
    let mut host_body: Vec<u8> = Vec::new();
    let mut partner_body: Vec<u8> = Vec::new();
    let notes: Vec<String> = Vec::new();

    for (clip, span) in clips.iter().zip(&layout.clips) {
        let head = &bytes[span.start..span.channels_start - 4];
        let mut host_channels: Vec<u8> = Vec::new();
        let mut partner_channels: Vec<u8> = Vec::new();
        let (mut host_count, mut partner_count) = (0u16, 0u16);
        let mut root_position = false;

        for (channel, (from, to)) in clip.channels.iter().zip(&span.channel_spans) {
            let record = &bytes[*from..*to];

            if channel.kind == "GROUP_BONE" && spec.partner_bones.contains(&channel.target) {
                partner_count += 1;

                if channel.target == spec.root && channel.property == "POSITION" {
                    root_position = true;

                    let shifted: Vec<Key> = channel
                        .keys
                        .iter()
                        .map(|k| {
                            let mut k = k.clone();

                            for value in std::iter::once(&mut k.value).chain(k.post.iter_mut()) {
                                for c in 0..3 {
                                    /* An expression takes the shift added on, written out: `(expression) + (shift)`. */
                                    match &value.expressions[c] {
                                        Some(e) if spec.shift[c] != 0.0 => value.expressions[c] = Some(format!("({e}) + ({})", spec.shift[c])),
                                        Some(_) => {}
                                        None => value.literals[c] += spec.shift[c],
                                    }
                                }
                            }

                            k
                        })
                        .collect();

                    put_channel(&mut partner_channels, &channel.target, 1, record[4], &shifted, &mut partner_strings);
                } else {
                    /* The file keeps the string table, so the target's index means the same. */
                    partner_channels.extend_from_slice(record);
                }
            } else {
                host_count += 1;
                host_channels.extend_from_slice(record);
            }
        }

        if !root_position {
            partner_count += 1;
            put_channel(&mut partner_channels, spec.root, 1, 1, &[literal_key(0.0, spec.shift)], &mut partner_strings);
        }

        host_body.extend_from_slice(head);
        host_body.extend_from_slice(&host_count.to_le_bytes());
        host_body.extend_from_slice(&(clip.events.len() as u16).to_le_bytes());
        host_body.extend_from_slice(&host_channels);
        host_body.extend_from_slice(&bytes[span.events_start..span.end]);

        partner_body.extend_from_slice(head);
        partner_body.extend_from_slice(&partner_count.to_le_bytes());
        partner_body.extend_from_slice(&0u16.to_le_bytes());
        partner_body.extend_from_slice(&partner_channels);
    }

    if let Some(id) = spec.host_id {
        index_of(&mut host_strings, id);
    }

    if let Some(id) = spec.partner_id {
        index_of(&mut partner_strings, id);
    }

    let host = strut_anim::assemble(&layout, &host_strings, &host_body, clips.len(), None, spec.host_id);
    let partner = strut_anim::assemble(&layout, &partner_strings, &partner_body, clips.len(), Some(spec.partner_fingerprint), spec.partner_id);

    Ok(Split { host, partner, notes })
}
