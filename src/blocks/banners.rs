//! Banners, the Loom, and your banner on your shield.
//!
//! - A **Banner** (six wool and a stick) starts plain white. Put it up and it
//!   stands on a pole, facing you.
//! - At a **Loom** (two planks and two string), hold a banner and pick a
//!   pattern and a dye you have: the dye is used up and the pattern goes on
//!   (a background colour, then up to two patterns; a third replaces the
//!   second). Six patterns: Stripe, Pale, Cross, Border, Chevron and Chief.
//! - Hold a Shield at the Loom and pick one of your banners: it's painted on
//!   your shield (yours to keep, like your skin, see settings.rs).
//!
//! A banner's design lives in its item's wear and, once put up, in
//! `Game::banners` with the way it faces; joined players hear about them with
//! `Msg::Banner`, and their hosts check the dyes.

use crate::block::*;
use crate::carpentry::{colour_rgb, COLOURS};
use crate::game::Game;
use crate::net::Msg;
use crate::render::{DynGeo, Pass};
use crate::sound::{Mat, Sfx};
use crate::texture::T_WHITE;
use macroquad::math::{IVec3, Mat4, Vec3};
use std::collections::HashMap;

/// Patterns (0 is "none"). A loom also offers "Base": the background colour.
pub const PATTERNS: [&str; 7] = ["Base", "Stripe", "Pale", "Cross", "Border", "Chevron", "Chief"];
/// The cloth, in cells.
pub const W: usize = 8;
pub const H: usize = 12;

/// A banner's design: background colour, then up to two (pattern, colour) layers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Design {
    pub base: u8,
    pub layers: [(u8, u8); 2],
}

impl Design {
    pub fn from_bits(b: u16) -> Design {
        Design { base: (b & 7) as u8, layers: [(((b >> 3) & 7) as u8 % 7, ((b >> 6) & 7) as u8), (((b >> 9) & 7) as u8 % 7, ((b >> 12) & 7) as u8)] }
    }

    pub fn bits(self) -> u16 {
        (self.base as u16 & 7) | (self.layers[0].0 as u16 & 7) << 3 | (self.layers[0].1 as u16 & 7) << 6 | (self.layers[1].0 as u16 & 7) << 9 | (self.layers[1].1 as u16 & 7) << 12
    }

    /// The design with `pattern` (0: the background) in `colour` added.
    pub fn with(mut self, pattern: u8, colour: u8) -> Design {
        let colour = colour % 8;
        if pattern == 0 {
            self.base = colour;
        } else if self.layers[0].0 == 0 {
            self.layers[0] = (pattern % 7, colour);
        } else {
            self.layers[1] = (pattern % 7, colour);
        }
        self
    }

    /// The colour of cell (x, y) (y = 0 at the top).
    pub fn cell(self, x: usize, y: usize) -> u8 {
        let mut c = self.base;
        for (p, col) in self.layers {
            if p != 0 && covers(p, x, y) {
                c = col;
            }
        }
        c
    }

    /// "Red banner: White Stripe, Black Border".
    pub fn describe(self) -> String {
        let mut s = format!("{} banner", COLOURS[self.base as usize % 8].1);
        let layers: Vec<String> = self.layers.iter().filter(|l| l.0 != 0).map(|&(p, c)| format!("{} {}", COLOURS[c as usize % 8].1, PATTERNS[p as usize % 7])).collect();
        if !layers.is_empty() {
            s += ": ";
            s += &layers.join(", ");
        }
        s
    }
}

/// Does pattern `p` cover cell (x, y)?
pub fn covers(p: u8, x: usize, y: usize) -> bool {
    let (fx, fy) = (x as f32 + 0.5 - W as f32 / 2.0, y as f32 + 0.5);
    match p {
        1 => (5..7).contains(&y),
        2 => (3..5).contains(&x),
        3 => (3..5).contains(&x) || (5..7).contains(&y),
        4 => x == 0 || x == W - 1 || y == 0 || y == H - 1,
        5 => fy > H as f32 - 3.5 - (3.5 - fx.abs()),
        6 => y < 3,
        _ => false,
    }
}

/// Draw a banner's cloth: `root` maps the cloth's (0..1, 0..1) square (x right, y up) into the world.
pub fn draw_cloth(g: &mut DynGeo, root: &Mat4, design: Design, light: f32, pass: Pass) {
    for colour in 0..8u8 {
        let rgb = colour_rgb(colour as usize);
        let mut any = false;
        for y in 0..H {
            for x in 0..W {
                if design.cell(x, y) != colour {
                    continue;
                }
                if !any {
                    g.begin(pass, [rgb[0] as f32 / 255.0, rgb[1] as f32 / 255.0, rgb[2] as f32 / 255.0, 1.0], false);
                    any = true;
                }
                let (x0, x1) = (x as f32 / W as f32, (x + 1) as f32 / W as f32);
                let (y0, y1) = (1.0 - (y + 1) as f32 / H as f32, 1.0 - y as f32 / H as f32);
                let c = [Vec3::new(x0, y0, 0.0), Vec3::new(x1, y0, 0.0), Vec3::new(x1, y1, 0.0), Vec3::new(x0, y1, 0.0)].map(|p| root.transform_point3(p));
                g.quad(c, T_WHITE, [0.0, 1.0, 1.0, 0.0], [1.0, light]);
                // And the back.
                g.quad([c[1], c[0], c[3], c[2]], T_WHITE, [0.0, 1.0, 1.0, 0.0], [1.0, light * 0.8]);
            }
        }
    }
}

/// Which way (0 north .. 3 west) something faces someone looking along `yaw`.
pub fn facing_from_yaw(yaw: f32) -> u8 {
    // Toward the one who put it up.
    (((yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) / std::f32::consts::FRAC_PI_2).round() as u8) % 4
}

/// Banners for the save: position, design, facing.
pub fn encode(banners: &HashMap<IVec3, (u16, u8)>) -> Vec<u8> {
    let mut out = (banners.len() as u32).to_le_bytes().to_vec();
    for (p, (d, f)) in banners {
        for c in [p.x, p.y, p.z] {
            out.extend(c.to_le_bytes());
        }
        out.extend(d.to_le_bytes());
        out.push(*f);
    }
    out
}

pub fn decode(b: &[u8]) -> HashMap<IVec3, (u16, u8)> {
    let mut m = HashMap::new();
    let Some(n) = b.get(..4).map(|s| u32::from_le_bytes(s.try_into().unwrap())) else { return m };
    for k in 0..n.min(1_000_000) as usize {
        let Some(s) = b.get(4 + k * 15..4 + (k + 1) * 15) else { break };
        let i = |o: usize| i32::from_le_bytes(s[o..o + 4].try_into().unwrap());
        m.insert(IVec3::new(i(0), i(4), i(8)), (u16::from_le_bytes([s[12], s[13]]), s[14] % 4));
    }
    m
}

impl Game {
    /// A banner went up at `p` (where the world lives): remember its design and tell everyone.
    pub fn put_up_banner(&mut self, p: IVec3, design: u16, facing: u8) {
        if self.world.get_v(p) != BANNER {
            return;
        }
        self.banners.insert(p, (design, facing % 4));
        self.net_broadcast(Msg::Banner { x: p.x, y: p.y, z: p.z, design, facing, up: true });
    }

    /// A banner came down: it drops (with its design).
    pub fn spill_banner(&mut self, p: IVec3) {
        if let Some((design, _)) = self.banners.remove(&p) {
            self.pop_drop_worn(p.as_vec3() + Vec3::splat(0.5), BANNER, 1, (design as u32) << 16);
            self.net_broadcast(Msg::Banner { x: p.x, y: p.y, z: p.z, design: 0, facing: 0, up: false });
        }
    }

    /// The host's word on a banner.
    pub fn banner_msg(&mut self, p: IVec3, design: u16, facing: u8, up: bool) {
        if up {
            self.banners.insert(p, (design, facing % 4));
        } else {
            self.banners.remove(&p);
        }
    }

    /// Hold a banner (or a shield) at a Loom: the screen opens.
    pub fn use_loom(&mut self, p: IVec3) {
        let held = self.inv.held();
        if held == BANNER || held == SHIELD {
            self.loom = Some(p);
        } else {
            self.msg("Hold a Banner to pattern it (or a Shield to paint one of your banners on it).");
        }
    }

    /// Put `pattern` in `colour` on the held banner, using up a dye.
    pub fn loom_apply(&mut self, pattern: u8, colour: u8) -> bool {
        let i = self.inv.selected;
        if self.inv.slots[i].map(|s| s.0) != Some(BANNER) {
            return false;
        }
        let dye = DYE_FIRST + colour as Id % 8;
        if !self.creative && self.inv.count(dye) == 0 {
            return false;
        }
        let old = crate::enchant::enchants(self.inv.wear[i]);
        let new = Design::from_bits(old).with(pattern, colour).bits();
        if !self.creative {
            self.inv.remove(dye, 1);
        }
        self.inv.wear[i] = (self.inv.wear[i] & 0xFFFF) | (new as u32) << 16;
        if self.is_client() {
            self.net_send_msg(Msg::Loom { design: new, dye });
        }
        self.sfx(Sfx::Place(Mat::Grass), None);
        self.advance("loomed");
        true
    }

    /// A joined player patterned the banner they're holding: check the dye, and track the new design.
    pub fn host_loom(&mut self, from: u32, design: u16, dye: Id) {
        if self.verified_held(from) != BANNER || !(DYE_FIRST..DYE_FIRST + 8).contains(&dye) {
            return;
        }
        let old = self.verified_ench(from);
        // Only one layer's change at a time, in the dye they used.
        let (a, b) = (Design::from_bits(old), Design::from_bits(design));
        let colour = (dye - DYE_FIRST) as u8;
        let ok = (0..PATTERNS.len() as u8).any(|p| a.with(p, colour) == b);
        if !ok || !self.peer_take(from, dye, 1) {
            return;
        }
        if let Some(l) = self.peers.get_mut(&from).map(|p| &mut p.ledger) {
            if old != 0 {
                l.remove_enchanted(BANNER, old);
            }
            if design != 0 {
                l.add_enchanted(BANNER, design, 1);
            }
        }
        self.set_peer_held(from, BANNER, design);
    }

    /// Draw the banners that are up near `eye`: a pole, a crossbar, and the cloth.
    pub fn draw_banners(&self, g: &mut DynGeo, eye: Vec3, range: f32) {
        for (&p, &(design, facing)) in &self.banners {
            let base = p.as_vec3() + Vec3::new(0.5, 0.0, 0.5);
            if base.distance(eye) > range {
                continue;
            }
            let light = self.world.sky_shade(p.x, p.y, p.z).max(0.3);
            let yaw = facing as f32 * std::f32::consts::FRAC_PI_2;
            let root = Mat4::from_translation(base) * Mat4::from_rotation_y(-yaw);
            g.begin(Pass::Opaque, [1.0; 4], false);
            let pole = root * Mat4::from_translation(Vec3::new(-0.05, 0.0, -0.05)) * Mat4::from_scale(Vec3::new(0.1, 2.0, 0.1));
            g.cube(&pole, [crate::texture::T_PLANKS; 6], light, [0.0, 0.0, 0.2, 1.0]);
            let bar = root * Mat4::from_translation(Vec3::new(-0.5, 1.88, 0.05)) * Mat4::from_scale(Vec3::new(1.0, 0.08, 0.08));
            g.cube(&bar, [crate::texture::T_PLANKS; 6], light, [0.0, 0.0, 1.0, 0.1]);
            // The cloth hangs in front of the pole.
            let cloth = root * Mat4::from_translation(Vec3::new(-0.45, 0.45, 0.14)) * Mat4::from_scale(Vec3::new(0.9, 1.42, 1.0));
            draw_cloth(g, &cloth, Design::from_bits(design), light, Pass::Opaque);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::tests::arena;

    #[test]
    fn designs_layer_up_and_round_trip() {
        let d = Design::default().with(0, 2).with(1, 0).with(4, 1);
        assert_eq!(d.base, 2);
        assert_eq!(d.layers, [(1, 0), (4, 1)]);
        assert_eq!(Design::from_bits(d.bits()), d);
        assert_eq!(d.cell(3, 0), 1, "the border is on top");
        assert_eq!(d.cell(3, 5), 0, "the stripe");
        assert_eq!(d.cell(3, 9), 2, "the background");
        assert_eq!(d.describe(), "Red banner: White Stripe, Black Border");
        // A third pattern replaces the second.
        assert_eq!(d.with(6, 4).layers[1], (6, 4));
        let mut m = HashMap::new();
        m.insert(IVec3::new(1, -2, 3), (d.bits(), 2));
        assert_eq!(decode(&encode(&m)), m);
    }

    #[test]
    fn looms_use_dye_and_banners_drop_their_design() {
        let mut g = arena(81);
        g.inv.slots[0] = Some((BANNER, 1));
        g.inv.slots[1] = Some((DYE_FIRST + 2, 2));
        g.inv.selected = 0;
        assert!(g.loom_apply(0, 2));
        assert!(g.loom_apply(3, 2));
        assert!(!g.loom_apply(1, 2), "out of red dye");
        let design = crate::enchant::enchants(g.inv.wear[0]);
        assert_eq!(Design::from_bits(design).layers[0], (3, 2));
        let p = g.player.body.pos.floor().as_ivec3() + IVec3::new(2, 0, 0);
        g.world.set_v(p, BANNER);
        g.put_up_banner(p, design, 1);
        assert_eq!(g.banners.get(&p), Some(&(design, 1)));
        g.spill_banner(p);
        assert!(g.drops.iter().any(|d| d.item == BANNER && crate::enchant::enchants(d.wear) == design));
    }
}
