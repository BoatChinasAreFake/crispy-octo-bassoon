//! Handy things: the **world border**, **sign colours** and glowing signs,
//! and **hotbar presets** in creative.
//!
//! - Where you last died shows on a held map.
//! - The **world border** (World Settings) stops everyone that far from the
//!   middle of the world, and glows red as you come up to it. It only
//!   applies in the ordinary world.
//! - Right-click a sign holding a **dye** to colour its words, or a
//!   **Glowshroom** to make them glow (they show from further off). The
//!   style is kept with the world and joined players are told.
//! - Creative hotbar presets live in settings.txt (see screens/inventory.rs).

use crate::block::*;
use crate::game::Game;
use crate::net::Msg;
use crate::render::{DynGeo, Pass};
use macroquad::math::{IVec3, Vec3};

/// How near the border it starts to glow.
const BORDER_GLOW: f32 = 16.0;
/// A sign style's glow bit (the low bits are 1 + a dye colour; 0: plain).
pub const GLOW: u8 = 16;

/// What a sign's style becomes with `item` (None: it does nothing).
pub fn restyle(style: u8, item: Id) -> Option<u8> {
    if (DYE_FIRST..DYE_FIRST + 8).contains(&item) {
        let c = (item - DYE_FIRST) as u8 + 1;
        return (style & 15 != c).then_some((style & GLOW) | c);
    }
    (item == GLOWSHROOM && style & GLOW == 0).then_some(style | GLOW)
}

/// A sign's text colour (white unless dyed).
pub fn sign_colour(style: u8) -> [u8; 3] {
    match style & 15 {
        0 => [255, 255, 255],
        c => crate::carpentry::colour_rgb((c - 1) as usize),
    }
}

/// Sign styles for the save: (x, y, z, style) each.
pub fn encode_styles(s: &std::collections::HashMap<IVec3, u8>) -> Vec<u8> {
    let mut out = Vec::new();
    let mut keys: Vec<_> = s.iter().collect();
    keys.sort_by_key(|(p, _)| (p.x, p.y, p.z));
    for (p, st) in keys {
        for v in [p.x, p.y, p.z] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.push(*st);
    }
    out
}

pub fn decode_styles(b: &[u8]) -> std::collections::HashMap<IVec3, u8> {
    b.chunks_exact(13).map(|c| (IVec3::new(i32::from_le_bytes([c[0], c[1], c[2], c[3]]), i32::from_le_bytes([c[4], c[5], c[6], c[7]]), i32::from_le_bytes([c[8], c[9], c[10], c[11]])), c[12])).collect()
}

impl Game {
    /// Keep the local player inside the world border (true if it stopped them).
    pub fn border_tick(&mut self, dt: f32) -> bool {
        self.border_note = (self.border_note - dt).max(0.0);
        if self.rules.border == 0 || self.elsewhere() {
            return false;
        }
        let b = self.rules.border as f32;
        let p = &mut self.player.body;
        let (cx, cz) = (p.pos.x.clamp(-b, b), p.pos.z.clamp(-b, b));
        if cx == p.pos.x && cz == p.pos.z {
            return false;
        }
        if cx != p.pos.x {
            p.vel.x = 0.0;
        }
        if cz != p.pos.z {
            p.vel.z = 0.0;
        }
        p.pos.x = cx;
        p.pos.z = cz;
        if self.border_note <= 0.0 {
            self.border_note = 4.0;
            self.msg("The world border. The world ends here (the owner said so).");
        }
        true
    }

    /// The border's red glow, when we're near it.
    pub fn draw_border(&self, g: &mut DynGeo, eye: Vec3) {
        if self.rules.border == 0 || self.elsewhere() {
            return;
        }
        let b = self.rules.border as f32;
        let walls = [(Vec3::X, b), (Vec3::NEG_X, b), (Vec3::Z, b), (Vec3::NEG_Z, b)];
        for (n, d) in walls {
            let along = if n.x != 0.0 { eye.x * n.x } else { eye.z * n.z };
            let gap = d - along;
            if gap > BORDER_GLOW {
                continue;
            }
            let alpha = 0.35 * (1.0 - gap / BORDER_GLOW).clamp(0.0, 1.0);
            g.begin(Pass::Blend, [1.0, 1.0, 1.0, alpha], true);
            let side = Vec3::new(n.z.abs(), 0.0, n.x.abs());
            for k in -6..6 {
                for up in -2..3 {
                    let base = n * d + side * ((if n.x != 0.0 { eye.z } else { eye.x }).floor() + k as f32 * 2.0) + Vec3::new(0.0, eye.y.floor() + up as f32 * 3.0, 0.0);
                    let c = [base, base + side * 2.0, base + side * 2.0 + Vec3::Y * 3.0, base + Vec3::Y * 3.0];
                    g.quad_tinted(c, crate::texture::T_WHITE, [1.0, 1.0], [255, 70, 70]);
                }
            }
        }
    }

    /// Right-clicked a sign: a dye colours its words, a Glowshroom makes them glow.
    pub fn style_sign(&mut self, pos: IVec3) -> bool {
        let held = self.inv.held();
        let style = self.world.sign_styles.get(&pos).copied().unwrap_or(0);
        let Some(new) = restyle(style, held) else { return false };
        self.world.sign_styles.insert(pos, new);
        if !self.creative {
            self.inv.consume_held();
        }
        self.player.swing = 1.0;
        self.sfx(crate::sound::Sfx::Brush, Some(pos.as_vec3() + Vec3::splat(0.5)));
        let item = if self.is_client() { held } else { AIR };
        let m = Msg::SignStyle { x: pos.x, y: pos.y, z: pos.z, style: new, item };
        if self.is_client() {
            self.net_send_msg(m);
        } else {
            self.net_broadcast(m);
        }
        true
    }

    /// The host: a joined player restyled a sign with `item` (they must have it, and be near).
    pub fn host_sign_style(&mut self, from: u32, pos: IVec3, item: Id) {
        let near = self.peers.get(&from).is_some_and(|p| p.target.distance(pos.as_vec3()) < 8.0);
        let style = self.world.sign_styles.get(&pos).copied().unwrap_or(0);
        let ok = near && crate::decor::is_sign(self.world.get_v(pos)) && valid_item(item) && restyle(style, item).is_some() && self.peer_take(from, item, 1);
        let new = if ok { restyle(style, item).unwrap_or(style) } else { style };
        if ok {
            self.world.sign_styles.insert(pos, new);
        }
        self.net_broadcast(Msg::SignStyle { x: pos.x, y: pos.y, z: pos.z, style: new, item: AIR });
    }

    /// Joined players: the host's word on a sign's style.
    pub fn apply_sign_style(&mut self, pos: IVec3, style: u8) {
        if style == 0 {
            self.world.sign_styles.remove(&pos);
        } else {
            self.world.sign_styles.insert(pos, style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use macroquad::math::ivec3;

    #[test]
    fn the_border_stops_you_and_signs_take_colour_and_glow() {
        let mut g = crate::game::tests::arena(212);
        g.rules.border = 1000;
        g.player.body.pos = Vec3::new(1003.0, 50.0, -1001.0);
        assert!(g.border_tick(0.05));
        assert_eq!((g.player.body.pos.x, g.player.body.pos.z), (1000.0, -1000.0));
        assert!(!g.border_tick(0.05), "inside now");
        // Dye, then a Glowshroom.
        let sign = ivec3(1, 50, 1);
        g.world.set_v(sign, SIGN_FIRST);
        g.inv.slots[0] = Some((DYE_FIRST + 2, 2));
        g.inv.selected = 0;
        assert!(g.style_sign(sign));
        assert!(!g.style_sign(sign), "already that colour");
        g.inv.slots[0] = Some((GLOWSHROOM, 1));
        assert!(g.style_sign(sign));
        let st = g.world.sign_styles[&sign];
        assert_eq!((st & 15, st & GLOW), (3, GLOW));
        assert_eq!(sign_colour(st), crate::carpentry::colour_rgb(2));
        assert_eq!(decode_styles(&encode_styles(&g.world.sign_styles)), g.world.sign_styles);
    }
}
