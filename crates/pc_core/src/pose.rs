//! Posing a model's group bones with a clip, in Rust: the same sampling and transform order the viewer uses (`ui/src/viewer/scene.ts`, `easing.ts`), so a conversion can prove that
//! it changes nothing on screen by comparing where every bone ends up before and after. Skinned meshes, armatures, Molang (an expression reads as 0) and physics are not posed:
//! only the group bones, which is what a scene's animation moves.

use crate::strut_anim::{Channel, Clip, Key, Value3};
use crate::strut_model::Model;
use std::collections::HashMap;

/// A 4x4 matrix, column-major, as three.js keeps it.
pub type Mat4 = [f64; 16];

const RAD: f64 = std::f64::consts::PI / 180.0;

pub fn identity() -> Mat4 {
    let mut m = [0.0; 16];

    m[0] = 1.0;
    m[5] = 1.0;
    m[10] = 1.0;
    m[15] = 1.0;
    m
}

pub fn multiply(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut out = [0.0; 16];

    for col in 0..4 {
        for row in 0..4 {
            let mut sum = 0.0;

            for k in 0..4 {
                sum += a[k * 4 + row] * b[col * 4 + k];
            }

            out[col * 4 + row] = sum;
        }
    }

    out
}

/// Translation, then rotation (Euler angles in degrees, applied Z then Y then X, three.js' order `ZYX`), then scale.
pub fn compose(position: [f64; 3], rotation: [f64; 3], scale: [f64; 3]) -> Mat4 {
    let (sx, cx) = (rotation[0] * RAD).sin_cos();
    let (sy, cy) = (rotation[1] * RAD).sin_cos();
    let (sz, cz) = (rotation[2] * RAD).sin_cos();

    /* R = Rz * Ry * Rx */
    let r = [
        [cz * cy, cz * sy * sx - sz * cx, cz * sy * cx + sz * sx],
        [sz * cy, sz * sy * sx + cz * cx, sz * sy * cx - cz * sx],
        [-sy, cy * sx, cy * cx],
    ];
    let mut m = identity();

    for row in 0..3 {
        for col in 0..3 {
            m[col * 4 + row] = r[row][col] * scale[col];
        }

        m[12 + row] = position[row];
    }

    m
}

type Ease = fn(f64) -> f64;

fn out(f: Ease, t: f64) -> f64 {
    1.0 - f(1.0 - t)
}

fn in_out(f: Ease, t: f64) -> f64 {
    if t < 0.5 { f(t * 2.0) / 2.0 } else { 1.0 - f((1.0 - t) * 2.0) / 2.0 }
}

/// The easing a keyframe names, as a function of how far along the segment it is. An unknown name is linear.
fn ease(name: &str, t: f64, args: &[f32]) -> f64 {
    let lower = name.to_lowercase();

    if lower == "linear" || lower == "none" {
        return t;
    }

    if lower == "step" {
        let steps = (args.first().copied().unwrap_or(2.0).trunc() as i64).max(2) as f64;
        let length = 1.0 / steps;

        return if t < 0.0 { 0.0 } else { ((t / length + 1e-9).floor()).min(steps - 1.0) * length };
    }

    let Some(rest) = lower.strip_prefix("ease") else { return t };
    let (direction, shape) = if let Some(s) = rest.strip_prefix("inout") { (2, s) } else if let Some(s) = rest.strip_prefix("in") { (0, s) } else if let Some(s) = rest.strip_prefix("out") { (1, s) } else { return t };
    let arg = args.first().map(|a| *a as f64);

    let base: Box<dyn Fn(f64) -> f64> = match shape {
        "quad" => Box::new(|t| t.powi(2)),
        "cubic" => Box::new(|t| t.powi(3)),
        "quart" => Box::new(|t| t.powi(4)),
        "quint" => Box::new(|t| t.powi(5)),
        "sine" => Box::new(|t| 1.0 - (t * std::f64::consts::PI / 2.0).cos()),
        "expo" => Box::new(|t| 2f64.powf(10.0 * (t - 1.0))),
        "circ" => Box::new(|t| 1.0 - (1.0 - t * t).sqrt()),
        "back" => {
            let s = arg.map_or(1.70158, |n| n * 1.70158);

            Box::new(move |t| t * t * ((s + 1.0) * t - s))
        }
        "elastic" => {
            let k = arg.unwrap_or(1.0);

            Box::new(move |t| 1.0 - (t * std::f64::consts::PI / 2.0).cos().powi(3) * (t * k * std::f64::consts::PI).cos())
        }
        "bounce" => {
            let k = arg.unwrap_or(0.5);

            Box::new(move |t| {
                ((121.0 / 16.0) * t * t).min((121.0 / 4.0) * k * (t - 6.0 / 11.0).powi(2) + 1.0 - k).min((121.0 * k * k * (t - 9.0 / 11.0).powi(2) + 1.0 - k * k).min(484.0 * k * k * k * (t - 10.5 / 11.0).powi(2) + 1.0 - k * k * k))
            })
        }
        _ => return t,
    };

    match direction {
        0 => base(t),
        1 => 1.0 - base(1.0 - t),
        _ => if t < 0.5 { base(t * 2.0) / 2.0 } else { 1.0 - base((1.0 - t) * 2.0) / 2.0 },
    }
}

fn spline(delta: f64, p0: f64, p1: f64, p2: f64, p3: f64) -> f64 {
    0.5 * (2.0 * p1 + (p2 - p0) * delta + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * delta * delta + (3.0 * p1 - p0 - 3.0 * p2 + p3) * delta * delta * delta)
}

fn cubic(s: f64, p0: f64, p1: f64, p2: f64, p3: f64) -> f64 {
    let m = 1.0 - s;

    m * m * m * p0 + 3.0 * m * m * s * p1 + 3.0 * m * s * s * p2 + s * s * s * p3
}

fn cubic_slope(s: f64, p0: f64, p1: f64, p2: f64, p3: f64) -> f64 {
    let m = 1.0 - s;

    3.0 * m * m * (p1 - p0) + 6.0 * m * s * (p2 - p1) + 3.0 * s * s * (p3 - p2)
}

#[allow(clippy::too_many_arguments)]
fn bezier(elapsed: f64, length: f64, start: f64, end: f64, right_time: f64, right_value: f64, left_time: f64, left_value: f64) -> f64 {
    let t1 = right_time.clamp(0.0, length);
    let t2 = length + left_time.clamp(-length, 0.0);
    let mut s = if length > 1e-6 { (elapsed / length).clamp(0.0, 1.0) } else { 0.0 };

    for _ in 0..8 {
        let slope = cubic_slope(s, 0.0, t1, t2, length);

        if slope.abs() < 1e-6 {
            break;
        }

        s = (s - (cubic(s, 0.0, t1, t2, length) - elapsed) / slope).clamp(0.0, 1.0);
    }

    cubic(s, start, start + right_value, end + left_value, end)
}

/// A key's value: a Molang expression reads as 0.
fn value(raw: &Value3) -> [f64; 3] {
    [0, 1, 2].map(|i| if raw.expressions[i].is_some() { 0.0 } else { raw.literals[i] as f64 })
}

/// A channel's value at a time (seconds), or the fallback when it has no keys.
pub fn sample(keys: &[Key], time: f64, fallback: [f64; 3]) -> [f64; 3] {
    if keys.is_empty() {
        return fallback;
    }

    if time <= keys[0].time as f64 {
        return value(&keys[0].value);
    }

    for i in 1..keys.len() {
        if time <= keys[i].time as f64 {
            let a = &keys[i - 1];
            let b = &keys[i];
            let from = value(a.post.as_ref().unwrap_or(&a.value));
            let to = value(&b.value);

            if b.time == a.time {
                return from;
            }

            let along = (time - a.time as f64) / (b.time - a.time) as f64;
            let name = if b.post.is_some() {
                "linear".to_string()
            } else {
                b.easing.clone().unwrap_or_else(|| ["step", "linear", "catmullrom", "bezier"].get(b.interpolation as usize).unwrap_or(&"linear").to_string())
            };

            if name == "catmullrom" {
                let before = if i < 2 { value(&a.value) } else { value(&keys[i - 2].value) };
                let after = if i + 1 >= keys.len() { to } else { value(&keys[i + 1].value) };

                return [0, 1, 2].map(|j| spline(along, before[j], from[j], to[j], after[j]));
            }

            if name == "bezier" {
                if let Some(left) = b.bezier.as_ref().filter(|h| h.len() >= 12) {
                    let length = (b.time - a.time) as f64;
                    let right = a.bezier.as_ref();

                    return [0, 1, 2].map(|j| {
                        bezier(time - a.time as f64, length, from[j], to[j], right.map_or(0.0, |r| r[6 + j] as f64), right.map_or(0.0, |r| r[9 + j] as f64), left[j] as f64, left[3 + j] as f64)
                    });
                }
            }

            let t = ease(&name, along, &b.easing_args);

            return [0, 1, 2].map(|j| from[j] + (to[j] - from[j]) * t);
        }
    }

    value(&keys[keys.len() - 1].value)
}

fn channel<'a>(clip: &'a Clip, bone: &str, property: &str) -> Option<&'a Channel> {
    clip.channels.iter().find(|c| c.kind == "GROUP_BONE" && c.target == bone && c.property == property)
}

/// Where every group bone of the model is with the clip at `time` seconds (no clip: at rest): its transform in the model's space, in blocks. A bone moves by its rest
/// offset from its parent's pivot plus the clip's position (in sixteenths of a block), turns by its rest rotation plus the clip's, and scales by the clip's.
pub fn world_matrices(model: &Model, clip: Option<&Clip>, time: f64) -> HashMap<String, Mat4> {
    let unit = model.units_per_block as f64;
    let mut worlds: Vec<Mat4> = Vec::with_capacity(model.group_bones.len());
    let mut by_name = HashMap::new();

    for bone in &model.group_bones {
        let parent = (bone.parent_index >= 0).then(|| &model.group_bones[bone.parent_index as usize]);
        let offset = [0, 1, 2].map(|i| (bone.pivot[i] as f64 - parent.map_or(0.0, |p| p.pivot[i] as f64)) / unit);
        let mut rotation = bone.rotation.map(|v| v as f64);
        let mut position = [0.0; 3];
        let mut scale = [1.0; 3];

        if let Some(clip) = clip {
            if let Some(c) = channel(clip, &bone.name, "ROTATION") {
                let r = sample(&c.keys, time, [0.0; 3]);

                rotation = [0, 1, 2].map(|i| r[i] + bone.rotation[i] as f64);
            }

            if let Some(c) = channel(clip, &bone.name, "POSITION") {
                position = sample(&c.keys, time, [0.0; 3]);
            }

            if let Some(c) = channel(clip, &bone.name, "SCALE") {
                scale = sample(&c.keys, time, [1.0; 3]);
            }
        }

        let local = compose([0, 1, 2].map(|i| offset[i] + position[i] / 16.0), rotation, scale);
        let world = if bone.parent_index >= 0 { multiply(&worlds[bone.parent_index as usize], &local) } else { local };

        by_name.insert(bone.name.clone(), world);
        worlds.push(world);
    }

    by_name
}

/// The largest difference between two matrices.
pub fn distance(a: &Mat4, b: &Mat4) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn euler_order_matches_three_js() {
        /* three.js: Euler(90deg, 0, 90deg, 'ZYX') turns the x axis to +y... checked by hand: Rz(90)*Rx(90) sends (1,0,0) to (0,1,0) and (0,1,0) to (0,0,1). */
        let m = compose([0.0; 3], [90.0, 0.0, 90.0], [1.0; 3]);

        assert!((m[1] - 1.0).abs() < 1e-9 && m[0].abs() < 1e-9 && m[2].abs() < 1e-9);
        assert!((m[6] - 1.0).abs() < 1e-9);
    }

    #[test]
    fn easing_and_splines_match_the_viewers_formulas() {
        assert!((ease("easeInQuad", 0.5, &[]) - 0.25).abs() < 1e-12);
        assert!((ease("easeOutQuad", 0.5, &[]) - 0.75).abs() < 1e-12);
        assert_eq!(ease("linear", 0.3, &[]), 0.3);
        assert!((spline(0.5, 0.0, 1.0, 2.0, 3.0) - 1.5).abs() < 1e-12);
        assert!((out(|t| t * t, 0.5) - 0.75).abs() < 1e-12 && (in_out(|t| t, 0.25) - 0.25).abs() < 1e-12);
    }

    #[test]
    fn a_pose_with_no_clip_is_the_rest_pose_and_a_clip_moves_it() {
        let folder = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test_pack/data/pleasurehorizons/asset_sync/strut");
        let Ok(model_bytes) = std::fs::read(folder.join("models/nude/jenny.strutmodel")) else { return };
        let model = crate::strut_model::read(&model_bytes).unwrap();
        let rest = world_matrices(&model, None, 0.0);

        assert_eq!(rest.len(), model.group_bones.len());

        let clips = crate::strut_anim::read(&std::fs::read(folder.join("animations/base.jenny.strutanim")).unwrap()).unwrap();
        let moving = clips.iter().find(|c| c.duration > 0.0 && !c.channels.is_empty()).unwrap();
        let posed = world_matrices(&model, Some(moving), moving.duration as f64 / 2.0);

        assert!(posed.iter().any(|(name, m)| distance(m, &rest[name]) > 1e-6), "the clip moved nothing");
        assert!(posed.values().all(|m| m.iter().all(|v| v.is_finite())));
    }
}
