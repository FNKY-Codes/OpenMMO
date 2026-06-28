use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use gltf::animation::Property;

use crate::math::Mat4;

pub const MAX_JOINTS: usize = 64;

pub const FROG_IDLE: &str = "Frog_Idle";
pub const FROG_JUMP: &str = "Frog_Jump";
pub const FROG_ATTACK: &str = "Frog_Attack";
pub const FROG_DEATH: &str = "Frog_Death";

pub const PLAYER_IDLE: &str = "Idle_Loop";
pub const PLAYER_WALK: &str = "Walk_Loop";
pub const PLAYER_ATTACK: &str = "Punch_Jab";
pub const PLAYER_DEATH: &str = "Death01";

/// Root node index in the shared superhero / UAL1 skeleton (strip horizontal drift).
pub const PLAYER_ROOT_NODE: usize = 64;

#[derive(Debug, Clone)]
pub struct DeathCorpse {
    pub npc_id: openmmo_common::NpcId,
    pub base: [f32; 3],
    pub yaw: f32,
    pub player: AnimationPlayer,
}

#[derive(Debug, Clone, Copy)]
struct Quat {
    x: f32,
    y: f32,
    z: f32,
    w: f32,
}

impl Quat {
    fn identity() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        }
    }

    fn from_gltf(v: [f32; 4]) -> Self {
        Self {
            x: v[0],
            y: v[1],
            z: v[2],
            w: v[3],
        }
    }

    fn normalize(self) -> Self {
        let len = (self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w).sqrt();
        if len < 1e-8 {
            return Self::identity();
        }
        Self {
            x: self.x / len,
            y: self.y / len,
            z: self.z / len,
            w: self.w / len,
        }
    }

    fn slerp(self, other: Self, t: f32) -> Self {
        let mut dot = self.x * other.x + self.y * other.y + self.z * other.z + self.w * other.w;
        let mut b = other;
        if dot < 0.0 {
            dot = -dot;
            b = Self {
                x: -b.x,
                y: -b.y,
                z: -b.z,
                w: -b.w,
            };
        }
        if dot > 0.9995 {
            return Self {
                x: self.x + t * (b.x - self.x),
                y: self.y + t * (b.y - self.y),
                z: self.z + t * (b.z - self.z),
                w: self.w + t * (b.w - self.w),
            }
            .normalize();
        }
        let theta = dot.clamp(-1.0, 1.0).acos();
        let sin_theta = theta.sin();
        let w1 = (theta * (1.0 - t)).sin() / sin_theta;
        let w2 = (theta * t).sin() / sin_theta;
        Self {
            x: self.x * w1 + b.x * w2,
            y: self.y * w1 + b.y * w2,
            z: self.z * w1 + b.z * w2,
            w: self.w * w1 + b.w * w2,
        }
        .normalize()
    }

    fn to_mat4(self) -> Mat4 {
        let (x, y, z, w) = (self.x, self.y, self.z, self.w);
        let xx = x * x;
        let yy = y * y;
        let zz = z * z;
        let xy = x * y;
        let xz = x * z;
        let yz = y * z;
        let wx = w * x;
        let wy = w * y;
        let wz = w * z;
        Mat4 {
            cols: [
                [1.0 - 2.0 * (yy + zz), 2.0 * (xy + wz), 2.0 * (xz - wy), 0.0],
                [2.0 * (xy - wz), 1.0 - 2.0 * (xx + zz), 2.0 * (yz + wx), 0.0],
                [2.0 * (xz + wy), 2.0 * (yz - wx), 1.0 - 2.0 * (xx + yy), 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }
}

#[derive(Debug, Clone)]
enum ChannelValues {
    Translations(Vec<[f32; 3]>),
    Rotations(Vec<[f32; 4]>),
    Scales(Vec<[f32; 3]>),
}

#[derive(Debug, Clone)]
struct AnimChannel {
    node: usize,
    property: Property,
    times: Vec<f32>,
    values: ChannelValues,
}

#[derive(Debug, Clone)]
pub struct AnimationClip {
    pub name: String,
    pub duration: f32,
    channels: Vec<AnimChannel>,
}

#[derive(Debug, Clone, Default)]
pub struct AnimationSet {
    pub clips: HashMap<String, AnimationClip>,
}

#[derive(Debug, Clone)]
pub struct Skeleton {
    pub joint_count: usize,
    joint_nodes: Vec<usize>,
    inverse_bind: Vec<Mat4>,
    node_parents: Vec<Option<usize>>,
    rest_local: Vec<Mat4>,
}

#[derive(Debug, Clone)]
pub struct AnimationPlayer {
    current: Option<String>,
    time: f32,
    looping: bool,
    pub finished: bool,
    idle_clip: String,
    pending_clip: Option<(String, bool)>,
}

impl AnimationPlayer {
    pub fn new(idle_clip: impl Into<String>) -> Self {
        Self {
            current: None,
            time: 0.0,
            looping: true,
            finished: false,
            idle_clip: idle_clip.into(),
            pending_clip: None,
        }
    }

    pub fn play(&mut self, name: impl Into<String>, looping: bool) {
        let name = name.into();
        if self.current.as_deref() == Some(name.as_str()) && !self.finished {
            return;
        }
        self.current = Some(name);
        self.time = 0.0;
        self.looping = looping;
        self.finished = false;
        self.pending_clip = None;
    }

    pub fn queue_after_current(&mut self, name: impl Into<String>, looping: bool) {
        self.pending_clip = Some((name.into(), looping));
    }

    pub fn current_clip(&self) -> Option<&str> {
        self.current.as_deref()
    }

    pub fn is_playing(&self, name: &str) -> bool {
        self.current.as_deref() == Some(name) && !self.finished
    }

    pub fn advance(&mut self, dt: f32, clips: &AnimationSet) {
        let Some(current) = self.current.clone() else {
            self.play(self.idle_clip.clone(), true);
            return;
        };
        let Some(clip) = clips.clips.get(&current) else {
            self.play(self.idle_clip.clone(), true);
            return;
        };
        self.time += dt;
        if self.looping {
            if clip.duration > 0.0 {
                self.time %= clip.duration;
            }
            return;
        }
        if self.time >= clip.duration {
            self.finished = true;
            if let Some((next, looping)) = self.pending_clip.take() {
                self.play(next, looping);
            } else if current != self.idle_clip {
                self.play(self.idle_clip.clone(), true);
            }
        }
    }

    pub fn bone_matrices(
        &self,
        skeleton: &Skeleton,
        clips: &AnimationSet,
        root_motion_node: Option<usize>,
    ) -> Vec<Mat4> {
        let clip_name = self
            .current
            .as_deref()
            .unwrap_or(self.idle_clip.as_str());
        let clip = clips
            .clips
            .get(clip_name)
            .or_else(|| clips.clips.get(&self.idle_clip));
        let Some(clip) = clip else {
            return skeleton.bind_pose();
        };
        let time = if self.looping && clip.duration > 0.0 {
            self.time % clip.duration
        } else {
            self.time.min(clip.duration)
        };
        if let Some(root_node) = root_motion_node {
            skeleton.sample_clip_in_place(clip, time, root_node)
        } else {
            skeleton.sample_clip(clip, time)
        }
    }
}

pub fn load_animation_set(path: &Path) -> Result<(Skeleton, AnimationSet)> {
    let (document, buffers, _images) = gltf::import(path).context("failed to import glb")?;
    load_animation_from_document(&document, &buffers)
}

/// Load animation clips only (mesh/skin from the same file is ignored).
pub fn load_animation_clips(path: &Path) -> Result<AnimationSet> {
    let (document, buffers, _images) = gltf::import(path).context("failed to import animation glb")?;
    let mut clips = AnimationSet::default();
    for anim in document.animations() {
        let clip = parse_animation(anim, &buffers)?;
        clips.clips.insert(clip.name.clone(), clip);
    }
    Ok(clips)
}

pub fn align_skeleton_to_clip(
    skeleton: &mut Skeleton,
    clips: &AnimationSet,
    clip_name: &str,
) {
    if let Some(clip) = clips.clips.get(clip_name) {
        skeleton.align_rest_pose_to_clip(clip);
    }
}

pub fn load_animation_from_document(
    document: &gltf::Document,
    buffers: &[gltf::buffer::Data],
) -> Result<(Skeleton, AnimationSet)> {
    let mut skeleton = parse_skeleton(document, buffers)?;
    let mut clips = AnimationSet::default();
    for anim in document.animations() {
        let clip = parse_animation(anim, buffers)?;
        clips.clips.insert(clip.name.clone(), clip);
    }
    if let Some(idle) = clips.clips.get(FROG_IDLE) {
        skeleton.align_rest_pose_to_clip(idle);
    }
    Ok((skeleton, clips))
}

impl Skeleton {
    pub fn bind_pose(&self) -> Vec<Mat4> {
        let globals = self.global_transforms(&self.rest_local);
        self.skin_matrices(&globals)
    }

    /// Rebase node rest pose and inverse bind matrices to match a clip at t=0.
    pub fn align_rest_pose_to_clip(&mut self, clip: &AnimationClip) {
        self.rest_local = self.sample_locals_from_base(&self.rest_local, clip, 0.0);
        self.rebuild_inverse_bind_from_rest();
    }

    pub fn sample_clip(&self, clip: &AnimationClip, time: f32) -> Vec<Mat4> {
        let locals = self.sample_locals(clip, time);
        let globals = self.global_transforms(&locals);
        self.skin_matrices(&globals)
    }

    /// Sample a clip with horizontal root translation removed (in-place locomotion).
    pub fn sample_clip_in_place(
        &self,
        clip: &AnimationClip,
        time: f32,
        root_node: usize,
    ) -> Vec<Mat4> {
        let mut locals = self.sample_locals(clip, time);
        let locals_at_start = self.sample_locals(clip, 0.0);
        if root_node < locals.len() && root_node < locals_at_start.len() {
            // Edit translation directly; TRS decompose/recompose can flip orientations
            // on the UAL root joint (-90° X) even when only X/Z should change.
            locals[root_node].cols[3][0] = locals_at_start[root_node].cols[3][0];
            locals[root_node].cols[3][2] = locals_at_start[root_node].cols[3][2];
        }
        let globals = self.global_transforms(&locals);
        self.skin_matrices(&globals)
    }

    fn sample_locals(&self, clip: &AnimationClip, time: f32) -> Vec<Mat4> {
        self.sample_locals_from_base(&self.rest_local, clip, time)
    }

    fn sample_locals_from_base(
        &self,
        base_locals: &[Mat4],
        clip: &AnimationClip,
        time: f32,
    ) -> Vec<Mat4> {
        let mut locals = base_locals.to_vec();
        for node_idx in 0..locals.len() {
            let (mut t, mut r, mut s) = mat4_trs(locals[node_idx]);
            let mut touched = false;
            for channel in &clip.channels {
                if channel.node != node_idx {
                    continue;
                }
                touched = true;
                match (&channel.property, &channel.values) {
                    (Property::Translation, ChannelValues::Translations(vals)) => {
                        t = sample_vec3(&channel.times, vals, time);
                    }
                    (Property::Rotation, ChannelValues::Rotations(vals)) => {
                        r = sample_quat(&channel.times, vals, time);
                    }
                    (Property::Scale, ChannelValues::Scales(vals)) => {
                        s = sample_vec3(&channel.times, vals, time);
                    }
                    _ => {}
                }
            }
            if touched {
                locals[node_idx] = mat4_from_trs(t, r, s);
            }
        }
        locals
    }

    fn rebuild_inverse_bind_from_rest(&mut self) {
        let globals = self.global_transforms(&self.rest_local);
        for (joint_idx, node) in self.joint_nodes.iter().enumerate() {
            self.inverse_bind[joint_idx] = globals
                .get(*node)
                .and_then(|m| m.inverse())
                .unwrap_or(Mat4::identity());
        }
    }

    fn global_transforms(&self, locals: &[Mat4]) -> Vec<Mat4> {
        let mut globals = vec![Mat4::identity(); locals.len()];
        let mut computed = vec![false; locals.len()];
        for idx in 0..locals.len() {
            self.compute_global(idx, locals, &mut globals, &mut computed);
        }
        globals
    }

    fn compute_global(
        &self,
        idx: usize,
        locals: &[Mat4],
        globals: &mut [Mat4],
        computed: &mut [bool],
    ) -> Mat4 {
        if computed[idx] {
            return globals[idx];
        }
        let global = match self.node_parents.get(idx).copied().flatten() {
            Some(parent) => {
                let parent_global = self.compute_global(parent, locals, globals, computed);
                parent_global.mul(locals[idx])
            }
            None => locals[idx],
        };
        globals[idx] = global;
        computed[idx] = true;
        global
    }

    fn skin_matrices(&self, globals: &[Mat4]) -> Vec<Mat4> {
        let mut out = vec![Mat4::identity(); self.joint_count];
        for (joint_idx, node) in self.joint_nodes.iter().enumerate() {
            let global = globals.get(*node).copied().unwrap_or(Mat4::identity());
            out[joint_idx] = global.mul(self.inverse_bind[joint_idx]);
        }
        out
    }
}

fn parse_skeleton(document: &gltf::Document, buffers: &[gltf::buffer::Data]) -> Result<Skeleton> {
    let node_count = document.nodes().len();
    let mut node_parents = vec![None; node_count];
    for node in document.nodes() {
        for child in node.children() {
            node_parents[child.index()] = Some(node.index());
        }
    }

    let mut rest_local = vec![Mat4::identity(); node_count];
    for node in document.nodes() {
        rest_local[node.index()] = Mat4::from_gltf(node.transform().matrix());
    }

    let skin = document
        .skins()
        .next()
        .context("skinned model missing skin")?;

    let mut joint_nodes = Vec::new();
    let mut inverse_bind = Vec::new();
    for joint in skin.joints() {
        joint_nodes.push(joint.index());
    }

    if let Some(reader) = skin.reader(|buffer| Some(&buffers[buffer.index()])).read_inverse_bind_matrices()
    {
        for m in reader {
            inverse_bind.push(Mat4::from_gltf(m));
        }
    } else {
        inverse_bind = vec![Mat4::identity(); joint_nodes.len()];
    }

    Ok(Skeleton {
        joint_count: joint_nodes.len(),
        joint_nodes,
        inverse_bind,
        node_parents,
        rest_local,
    })
}

fn parse_animation(anim: gltf::Animation, buffers: &[gltf::buffer::Data]) -> Result<AnimationClip> {
    let name = anim
        .name()
        .map(str::to_string)
        .unwrap_or_else(|| format!("anim_{}", anim.index()));
    let mut channels = Vec::new();
    let mut duration = 0.0f32;

    for channel in anim.channels() {
        let reader = channel.reader(|buffer| Some(&buffers[buffer.index()]));
        let times: Vec<f32> = reader
            .read_inputs()
            .context("animation channel missing inputs")?
            .collect();
        if let Some(&last) = times.last() {
            duration = duration.max(last);
        }
        let node = channel.target().node().index();
        let property = channel.target().property();
        let values = match reader.read_outputs().context("animation channel missing outputs")? {
            gltf::animation::util::ReadOutputs::Translations(iter) => {
                ChannelValues::Translations(iter.collect())
            }
            gltf::animation::util::ReadOutputs::Rotations(iter) => {
                let rotations: Vec<[f32; 4]> = match iter {
                    gltf::animation::util::Rotations::F32(r) => {
                        r.map(|q| [q[0], q[1], q[2], q[3]]).collect()
                    }
                    gltf::animation::util::Rotations::I16(r) => r
                        .map(|q| [q[0] as f32, q[1] as f32, q[2] as f32, q[3] as f32])
                        .collect(),
                    gltf::animation::util::Rotations::U16(r) => r
                        .map(|q| [q[0] as f32, q[1] as f32, q[2] as f32, q[3] as f32])
                        .collect(),
                    gltf::animation::util::Rotations::U8(r) => r
                        .map(|q| [q[0] as f32, q[1] as f32, q[2] as f32, q[3] as f32])
                        .collect(),
                    gltf::animation::util::Rotations::I8(r) => r
                        .map(|q| [q[0] as f32, q[1] as f32, q[2] as f32, q[3] as f32])
                        .collect(),
                };
                ChannelValues::Rotations(rotations)
            }
            gltf::animation::util::ReadOutputs::Scales(iter) => {
                ChannelValues::Scales(iter.collect())
            }
            gltf::animation::util::ReadOutputs::MorphTargetWeights(_) => continue,
        };
        channels.push(AnimChannel {
            node,
            property,
            times,
            values,
        });
    }

    Ok(AnimationClip {
        name,
        duration,
        channels,
    })
}

fn sample_vec3(times: &[f32], values: &[[f32; 3]], time: f32) -> [f32; 3] {
    if times.is_empty() {
        return [0.0, 0.0, 0.0];
    }
    if time <= times[0] {
        return values[0];
    }
    if time >= *times.last().unwrap() {
        return *values.last().unwrap();
    }
    let idx = times.partition_point(|&t| t <= time).saturating_sub(1);
    let t0 = times[idx];
    let t1 = times[idx + 1];
    let alpha = if (t1 - t0).abs() < 1e-8 {
        0.0
    } else {
        (time - t0) / (t1 - t0)
    };
    let a = values[idx];
    let b = values[idx + 1];
    [
        a[0] + (b[0] - a[0]) * alpha,
        a[1] + (b[1] - a[1]) * alpha,
        a[2] + (b[2] - a[2]) * alpha,
    ]
}

fn sample_quat(times: &[f32], values: &[[f32; 4]], time: f32) -> Quat {
    if times.is_empty() {
        return Quat::identity();
    }
    if time <= times[0] {
        return Quat::from_gltf(values[0]);
    }
    if time >= *times.last().unwrap() {
        return Quat::from_gltf(*values.last().unwrap());
    }
    let idx = times.partition_point(|&t| t <= time).saturating_sub(1);
    let t0 = times[idx];
    let t1 = times[idx + 1];
    let alpha = if (t1 - t0).abs() < 1e-8 {
        0.0
    } else {
        (time - t0) / (t1 - t0)
    };
    let a = Quat::from_gltf(values[idx]);
    let b = Quat::from_gltf(values[idx + 1]);
    a.slerp(b, alpha)
}

fn mat4_from_trs(t: [f32; 3], r: Quat, s: [f32; 3]) -> Mat4 {
    let rot = r.to_mat4();
    let scale = Mat4::scale(s[0], s[1], s[2]);
    let trans = Mat4::translation(t[0], t[1], t[2]);
    trans.mul(rot.mul(scale))
}

fn mat4_trs(m: Mat4) -> ([f32; 3], Quat, [f32; 3]) {
    let t = [m.cols[3][0], m.cols[3][1], m.cols[3][2]];
    let sx = (m.cols[0][0] * m.cols[0][0]
        + m.cols[0][1] * m.cols[0][1]
        + m.cols[0][2] * m.cols[0][2])
        .sqrt();
    let sy = (m.cols[1][0] * m.cols[1][0]
        + m.cols[1][1] * m.cols[1][1]
        + m.cols[1][2] * m.cols[1][2])
        .sqrt();
    let sz = (m.cols[2][0] * m.cols[2][0]
        + m.cols[2][1] * m.cols[2][1]
        + m.cols[2][2] * m.cols[2][2])
        .sqrt();
    let s = [sx.max(1e-8), sy.max(1e-8), sz.max(1e-8)];
    let inv_sx = 1.0 / s[0];
    let inv_sy = 1.0 / s[1];
    let inv_sz = 1.0 / s[2];
    let m00 = m.cols[0][0] * inv_sx;
    let m01 = m.cols[0][1] * inv_sx;
    let m02 = m.cols[0][2] * inv_sx;
    let m10 = m.cols[1][0] * inv_sy;
    let m11 = m.cols[1][1] * inv_sy;
    let m12 = m.cols[1][2] * inv_sy;
    let m20 = m.cols[2][0] * inv_sz;
    let m21 = m.cols[2][1] * inv_sz;
    let m22 = m.cols[2][2] * inv_sz;
    let trace = m00 + m11 + m22;
    let q = if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        Quat {
            x: (m21 - m12) / s,
            y: (m02 - m20) / s,
            z: (m10 - m01) / s,
            w: 0.25 * s,
        }
    } else if m00 > m11 && m00 > m22 {
        let s = (1.0 + m00 - m11 - m22).sqrt() * 2.0;
        Quat {
            x: 0.25 * s,
            y: (m01 + m10) / s,
            z: (m02 + m20) / s,
            w: (m21 - m12) / s,
        }
    } else if m11 > m22 {
        let s = (1.0 + m11 - m00 - m22).sqrt() * 2.0;
        Quat {
            x: (m01 + m10) / s,
            y: 0.25 * s,
            z: (m12 + m21) / s,
            w: (m02 - m20) / s,
        }
    } else {
        let s = (1.0 + m22 - m00 - m11).sqrt() * 2.0;
        Quat {
            x: (m02 + m20) / s,
            y: (m12 + m21) / s,
            z: 0.25 * s,
            w: (m10 - m01) / s,
        }
    };
    (t, q.normalize(), s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn frog_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/models/Frog.glb")
    }

    #[test]
    fn in_place_root_strip_preserves_matrix_at_t0() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/models/UAL1_Standard.glb");
        let (skel, clips) = load_animation_set(&path).expect("load UAL");
        let idle = &clips.clips[PLAYER_IDLE];
        let at_zero = skel.sample_clip(idle, 0.0);
        let in_place = skel.sample_clip_in_place(idle, 0.0, PLAYER_ROOT_NODE);
        let mut max_diff = 0.0f32;
        for (a, b) in at_zero.iter().zip(in_place.iter()) {
            for c in 0..4 {
                for r in 0..4 {
                    max_diff = max_diff.max((a.cols[c][r] - b.cols[c][r]).abs());
                }
            }
        }
        assert!(
            max_diff < 1e-4,
            "in-place strip at t=0 should not alter bone matrices (max diff {max_diff})"
        );
    }

    #[test]
    fn frog_idle_at_zero_matches_bind_pose_after_align() {
        let path = frog_path();
        let (skel, clips) = load_animation_set(&path).expect("load frog");
        let idle = &clips.clips[FROG_IDLE];
        let bind = skel.bind_pose();
        let at_zero = skel.sample_clip(idle, 0.0);
        let mut max_diff = 0.0f32;
        for (a, b) in bind.iter().zip(at_zero.iter()) {
            for c in 0..4 {
                for r in 0..4 {
                    max_diff = max_diff.max((a.cols[c][r] - b.cols[c][r]).abs());
                }
            }
        }
        assert!(
            max_diff < 1e-3,
            "idle at t=0 should match bind pose after align (max diff {max_diff})"
        );
    }

    #[test]
    fn frog_has_four_clips() {
        let path = frog_path();
        assert!(path.is_file(), "missing Frog.glb at {path:?}");
        let (_skel, clips) = load_animation_set(&path).expect("load frog");
        assert_eq!(clips.clips.len(), 4);
        assert!(clips.clips.contains_key(FROG_IDLE));
        assert!(clips.clips.contains_key(FROG_JUMP));
        assert!(clips.clips.contains_key(FROG_ATTACK));
        assert!(clips.clips.contains_key(FROG_DEATH));
    }

    #[test]
    fn frog_clip_durations() {
        let path = frog_path();
        let (_skel, clips) = load_animation_set(&path).expect("load frog");
        assert!((clips.clips[FROG_IDLE].duration - 2.5).abs() < 0.01);
        assert!((clips.clips[FROG_JUMP].duration - 0.875).abs() < 0.01);
        assert!((clips.clips[FROG_ATTACK].duration - 0.75).abs() < 0.01);
        assert!((clips.clips[FROG_DEATH].duration - 0.833).abs() < 0.02);
    }

    #[test]
    fn idle_animation_changes_bones_over_time() {
        let path = frog_path();
        let (skel, clips) = load_animation_set(&path).expect("load frog");
        let clip = &clips.clips[FROG_IDLE];
        let at_zero = skel.sample_clip(clip, 0.0);
        let at_mid = skel.sample_clip(clip, 1.25);
        assert_eq!(at_zero.len(), skel.joint_count);
        let changed = at_zero
            .iter()
            .zip(at_mid.iter())
            .any(|(a, b)| (a.cols[3][0] - b.cols[3][0]).abs() > 1e-4);
        assert!(changed, "idle clip should move bones");
    }
}
