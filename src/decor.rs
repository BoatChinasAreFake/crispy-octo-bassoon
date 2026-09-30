//! Signs (four lines of text, readable from a distance) and item frames
//! (show off one thing on a wall).
//!
//! What's written on a sign and what's in a frame live in the world (like a
//! chest's contents) and are saved with it. Joined players edit them through
//! the host: `SignText` both ways, `FrameUse` asks, `FrameItem` tells.

use crate::block::*;
use crate::game::Game;
use crate::inventory::Wear;
use crate::net::Msg;
use crate::render::{DynGeo, Pass};
use crate::sound::{Mat, Sfx};
use macroquad::math::{IVec3, Mat4, Vec3};

/// Lines on a sign, and how long each can be.
pub const LINES: usize = 4;
pub const LINE_LEN: usize = 15;

pub fn is_sign(id: Id) -> bool {
    (SIGN_FIRST..SIGN_FIRST + 4).contains(&id)
}

pub fn is_frame(id: Id) -> bool {
    (FRAME_FIRST..FRAME_FIRST + 4).contains(&id)
}

/// Keep sign text tidy: four lines, short, printable.
pub fn clean_lines(lines: &[String]) -> [String; LINES] {
    let mut out: [String; LINES] = Default::default();
    for (i, l) in lines.iter().take(LINES).enumerate() {
        out[i] = l.chars().filter(|c| !c.is_control()).take(LINE_LEN).collect();
    }
    out
}

/// Which side of its cell a frame hangs on, from the face that was clicked.
pub fn frame_facing(normal: IVec3) -> Option<u8> {
    match (normal.x, normal.z) {
        (0, 1) => Some(0),
        (-1, 0) => Some(1),
        (0, -1) => Some(2),
        (1, 0) => Some(3),
        _ => None,
    }
}

/// Outward from the wall, for a frame hung on side `facing`.
fn outward(facing: u8) -> Vec3 {
    match facing % 4 {
        0 => Vec3::Z,
        1 => Vec3::NEG_X,
        2 => Vec3::NEG_Z,
        _ => Vec3::X,
    }
}

impl Game {
    /// Put text on a sign (ours, or the host's word).
    pub fn set_sign(&mut self, pos: IVec3, lines: &[String]) {
        // Where the world lives it must really be a sign (joined players' chunks may not be here yet).
        if !self.is_client() && !is_sign(self.world.get_v(pos)) {
            return;
        }
        let lines = clean_lines(lines);
        self.world.signs.insert(pos, lines.clone());
        let m = Msg::SignText { x: pos.x, y: pos.y, z: pos.z, lines: lines.to_vec() };
        if self.is_client() {
            self.net_send_msg(m);
        } else {
            self.net_broadcast(m);
        }
    }

    /// The host: a joined player wrote on a sign.
    pub fn host_sign(&mut self, from: u32, pos: IVec3, lines: Vec<String>) {
        let near = self.peers.get(&from).is_some_and(|p| p.target.distance(pos.as_vec3()) < 10.0);
        if near {
            self.set_sign(pos, &lines);
        }
    }

    /// Right-click on a frame holding something (true), or putting something in it.
    pub fn use_frame(&mut self, pos: IVec3) -> bool {
        if !is_frame(self.world.get_v(pos)) {
            return false;
        }
        let held = self.inv.held();
        if held == AIR || self.world.frames.contains_key(&pos) {
            return true;
        }
        let wear = self.inv.wear[self.inv.selected];
        self.world.frames.insert(pos, (held, wear));
        if !self.creative {
            self.inv.consume_held();
        }
        self.sfx(Sfx::Place(Mat::Wood), Some(pos.as_vec3() + Vec3::splat(0.5)));
        self.player.swing = 1.0;
        let m = Msg::FrameUse { x: pos.x, y: pos.y, z: pos.z, item: held, wear, put: true };
        if self.is_client() {
            self.net_send_msg(m);
        } else {
            self.net_broadcast(Msg::FrameItem { x: pos.x, y: pos.y, z: pos.z, item: held, wear });
        }
        true
    }

    /// Hit a frame with something in it: the thing pops out (true); an empty frame breaks as usual.
    pub fn hit_frame(&mut self, pos: IVec3) -> bool {
        if !is_frame(self.world.get_v(pos)) || !self.world.frames.contains_key(&pos) {
            return false;
        }
        if self.is_client() {
            self.world.frames.remove(&pos);
            self.net_send_msg(Msg::FrameUse { x: pos.x, y: pos.y, z: pos.z, item: AIR, wear: 0, put: false });
        } else {
            self.spill_frame(pos);
        }
        self.sfx(Sfx::Pop, Some(pos.as_vec3() + Vec3::splat(0.5)));
        true
    }

    /// Where the world lives: a frame's item falls out (it was hit, or the frame broke).
    pub fn spill_frame(&mut self, pos: IVec3) {
        if let Some((item, wear)) = self.world.frames.remove(&pos) {
            self.pop_drop_worn(pos.as_vec3() + Vec3::splat(0.5), item, 1, wear);
            self.net_broadcast(Msg::FrameItem { x: pos.x, y: pos.y, z: pos.z, item: AIR, wear: 0 });
        }
    }

    /// The host: a joined player put something in a frame (they must own it) or knocked it out.
    #[allow(clippy::too_many_arguments)]
    pub fn host_frame_use(&mut self, from: u32, pos: IVec3, item: Id, wear: Wear, put: bool) {
        let near = self.peers.get(&from).is_some_and(|p| p.target.distance(pos.as_vec3()) < 8.0);
        if !near || !is_frame(self.world.get_v(pos)) {
            return;
        }
        if !put {
            self.spill_frame(pos);
            return;
        }
        if self.world.frames.contains_key(&pos) || !valid_item(item) || !self.peer_take(from, item, 1) {
            let (i, w) = self.world.frames.get(&pos).copied().unwrap_or((AIR, 0));
            self.net_send_to(from, Msg::FrameItem { x: pos.x, y: pos.y, z: pos.z, item: i, wear: w });
            return;
        }
        let wear = self.launder(from, item, crate::inventory::sanitize_wear(item, wear));
        self.world.frames.insert(pos, (item, wear));
        self.net_broadcast(Msg::FrameItem { x: pos.x, y: pos.y, z: pos.z, item, wear });
    }

    /// Joined players: the host's word on a frame.
    pub fn apply_frame(&mut self, pos: IVec3, item: Id, wear: Wear) {
        if item == AIR || !valid_item(item) {
            self.world.frames.remove(&pos);
        } else {
            self.world.frames.insert(pos, (item, wear));
        }
    }

    /// Everything hung in frames nearby.
    pub fn draw_frames(&self, g: &mut DynGeo, eye: Vec3, range: f32) {
        g.begin(Pass::Opaque, [1.0; 4], false);
        for (&pos, &(item, _)) in &self.world.frames {
            let center = pos.as_vec3() + Vec3::splat(0.5);
            if center.distance(eye) > range {
                continue;
            }
            let Some(facing) = self.world.get_v(pos).checked_sub(FRAME_FIRST).filter(|f| *f < 4) else { continue };
            let out = outward(facing as u8);
            let sky = self.world.sky_shade(pos.x, pos.y, pos.z).max(0.3);
            // Just off the wall.
            let at = center - out * 0.4;
            let yaw = out.x.atan2(out.z);
            let root = Mat4::from_translation(at) * Mat4::from_rotation_y(yaw);
            if is_block_item(item) {
                let m = root * Mat4::from_translation(Vec3::new(-0.2, -0.2, 0.0)) * Mat4::from_scale(Vec3::new(0.4, 0.4, 0.08));
                let t = block(item).tex;
                g.cube(&m, [t[1], t[1], t[0], t[2], t[1], t[1]], sky, [0.0, 0.0, 1.0, 1.0]);
            } else {
                let s = 0.3;
                let c = [Vec3::new(-s, -s, 0.02), Vec3::new(s, -s, 0.02), Vec3::new(s, s, 0.02), Vec3::new(-s, s, 0.02)].map(|p| root.transform_point3(p));
                g.quad(c, item_tile(item), [0.0, 1.0, 1.0, 0.0], [1.0, sky]);
            }
        }
    }
}

/// Pack signs and frames for the save file.
pub fn encode(signs: &std::collections::HashMap<IVec3, [String; LINES]>, frames: &std::collections::HashMap<IVec3, (Id, Wear)>) -> Vec<u8> {
    let mut out = Vec::new();
    let put_pos = |out: &mut Vec<u8>, p: &IVec3| {
        for v in [p.x, p.y, p.z] {
            out.extend_from_slice(&v.to_le_bytes());
        }
    };
    out.extend_from_slice(&(signs.len() as u32).to_le_bytes());
    for (p, lines) in signs {
        put_pos(&mut out, p);
        for l in lines {
            let b = &l.as_bytes()[..l.len().min(64)];
            out.push(b.len() as u8);
            out.extend_from_slice(b);
        }
    }
    out.extend_from_slice(&(frames.len() as u32).to_le_bytes());
    for (p, &(item, wear)) in frames {
        put_pos(&mut out, p);
        out.extend_from_slice(&item.to_le_bytes());
        out.extend_from_slice(&wear.to_le_bytes());
    }
    out
}

#[allow(clippy::type_complexity)]
pub fn decode(b: &[u8]) -> (std::collections::HashMap<IVec3, [String; LINES]>, std::collections::HashMap<IVec3, (Id, Wear)>) {
    let (mut signs, mut frames) = (std::collections::HashMap::new(), std::collections::HashMap::new());
    let mut i = 0usize;
    let mut take = |n: usize| -> Option<&[u8]> {
        let s = b.get(i..i + n)?;
        i += n;
        Some(s)
    };
    let u32_of = |s: &[u8]| u32::from_le_bytes([s[0], s[1], s[2], s[3]]);
    let pos_of = |s: &[u8]| IVec3::new(u32_of(&s[0..4]) as i32, u32_of(&s[4..8]) as i32, u32_of(&s[8..12]) as i32);
    let Some(n) = take(4).map(u32_of) else { return (signs, frames) };
    'signs: for _ in 0..n.min(100_000) {
        let Some(p) = take(12).map(pos_of) else { break };
        let mut lines: [String; LINES] = Default::default();
        for l in lines.iter_mut() {
            let Some(len) = take(1).map(|s| s[0] as usize) else { break 'signs };
            let Some(t) = take(len) else { break 'signs };
            *l = String::from_utf8_lossy(t).into_owned();
        }
        signs.insert(p, clean_lines(&lines));
    }
    let Some(n) = take(4).map(u32_of) else { return (signs, frames) };
    for _ in 0..n.min(100_000) {
        let (Some(p), Some(item), Some(wear)) = (take(12).map(pos_of), take(2).map(|s| u16::from_le_bytes([s[0], s[1]])), take(4).map(u32_of)) else { break };
        if valid_item(item) {
            frames.insert(p, (item, crate::inventory::sanitize_wear(item, wear)));
        }
    }
    (signs, frames)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signs_and_frames_basics() {
        let long = "a very long line that goes on and on".to_string();
        let lines = clean_lines(&[long, "hi\tthere".into(), "".into(), "four".into(), "five".into()]);
        assert_eq!(lines[0].chars().count(), LINE_LEN);
        assert_eq!(lines[1], "hithere");
        assert_eq!(lines[3], "four");
        assert_eq!(frame_facing(IVec3::Z), Some(0));
        assert_eq!(frame_facing(IVec3::Y), None);
        assert!(is_sign(SIGN_FIRST + 3) && is_frame(FRAME_FIRST) && !is_frame(SIGN_FIRST));
    }
}
