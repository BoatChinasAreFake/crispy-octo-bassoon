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
//!
//! **Waypoints**: places you name with `/waypoint add <name>` (or `/wp`).
//! Each shows as a coloured label over the spot, with how far off it is,
//! and as a coloured diamond on a held map. `/wp list` says where they
//! all are and `/wp remove <name>` forgets one.
//!
//! They're yours: typed commands are handled on your own game, not sent to
//! the host. They're kept with the world you made them in (a joined player's
//! last until they leave).

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
    ((item == GLOWSHROOM || item == GLOW_INK_SAC) && style & GLOW == 0).then_some(style | GLOW)
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

    /// Right-clicked a sign: a dye colours its words, a Glowshroom (or a Glow Ink Sac) makes them glow.
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

// ---- waypoints

/// At most this many waypoints, with names at most `NAME_LEN` long.
pub const MAX_WAYPOINTS: usize = 32;
pub const NAME_LEN: usize = 24;
/// Labels in the world show this far off (blocks).
pub const SHOW_RANGE: f32 = 2000.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Waypoint {
    pub name: String,
    pub pos: Vec3,
}

/// A waypoint's colour (by its place in the list).
pub fn colour(i: usize) -> [u8; 3] {
    const COLOURS: [[u8; 3]; 8] = [[255, 90, 90], [90, 200, 255], [255, 210, 70], [120, 230, 110], [230, 120, 255], [255, 150, 60], [90, 255, 210], [250, 250, 250]];
    COLOURS[i % COLOURS.len()]
}

/// Which world a spot is in (the Scorchlands and the Hollow live far off along x).
pub fn realm(x: f32) -> u8 {
    if crate::hollow::in_hollow(x) {
        2
    } else if crate::scorch::in_scorch(x) {
        1
    } else {
        0
    }
}

pub fn encode(w: &[Waypoint]) -> Vec<u8> {
    let mut out = Vec::new();
    for p in w {
        out.push(p.name.len() as u8);
        out.extend_from_slice(p.name.as_bytes());
        for v in p.pos.to_array() {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    out
}

pub fn decode(mut b: &[u8]) -> Vec<Waypoint> {
    let mut out = Vec::new();
    while let Some((&n, rest)) = b.split_first() {
        let n = n as usize;
        if rest.len() < n + 12 {
            break;
        }
        let name = String::from_utf8_lossy(&rest[..n]).into_owned();
        let f = |o: usize| f32::from_le_bytes(rest[n + o..n + o + 4].try_into().unwrap());
        let pos = Vec3::new(f(0), f(4), f(8));
        if pos.is_finite() && !name.is_empty() && out.len() < MAX_WAYPOINTS {
            out.push(Waypoint { name, pos });
        }
        b = &rest[n + 12..];
    }
    out
}

/// Is `text` a waypoint command (handled here, not sent anywhere)?
pub fn is_command(text: &str) -> bool {
    matches!(text.split_whitespace().next(), Some("/waypoint" | "/waypoints" | "/wp"))
}

impl Game {
    /// `/waypoint add|remove|list [name]`: what to say back.
    pub fn waypoint_command(&mut self, text: &str) -> Vec<String> {
        let mut words = text.split_whitespace().skip(1);
        let verb = words.next().unwrap_or("").to_ascii_lowercase();
        let name: String = words.collect::<Vec<_>>().join(" ").chars().take(NAME_LEN).collect();
        let me = self.player.body.pos;
        let find = |w: &[Waypoint], n: &str| w.iter().position(|p| p.name.eq_ignore_ascii_case(n));
        match verb.as_str() {
            "add" | "set" if !name.is_empty() => {
                let at = Vec3::new(me.x.floor() + 0.5, me.y.floor(), me.z.floor() + 0.5);
                if let Some(i) = find(&self.waypoints, &name) {
                    self.waypoints[i].pos = at;
                    return vec![format!("Moved waypoint \"{name}\" here.")];
                }
                if self.waypoints.len() >= MAX_WAYPOINTS {
                    return vec![format!("That's {MAX_WAYPOINTS} waypoints already. Remove one first (/wp remove <name>).")];
                }
                self.waypoints.push(Waypoint { name: name.clone(), pos: at });
                vec![format!("Waypoint \"{name}\" set at {:.0} {:.0} {:.0}.", at.x.floor(), at.y, at.z.floor())]
            }
            "remove" | "delete" | "del" | "rm" if !name.is_empty() => match find(&self.waypoints, &name) {
                Some(i) => {
                    let w = self.waypoints.remove(i);
                    vec![format!("Forgot waypoint \"{}\".", w.name)]
                }
                None => vec![format!("No waypoint called \"{name}\". /wp list shows them.")],
            },
            "list" | "" if !self.waypoints.is_empty() || verb == "list" => {
                if self.waypoints.is_empty() {
                    return vec!["No waypoints yet. /wp add <name> marks where you're standing.".into()];
                }
                let mut out = vec![format!("{} waypoint{}:", self.waypoints.len(), if self.waypoints.len() == 1 { "" } else { "s" })];
                for w in &self.waypoints {
                    let d = w.pos - me;
                    let far = if realm(w.pos.x) != realm(me.x) {
                        "in another world".to_string()
                    } else {
                        format!("{:.0} blocks {}", Vec3::new(d.x, 0.0, d.z).length(), crate::archaeology::compass_word(d))
                    };
                    out.push(format!("  {}: {:.0} {:.0} {:.0} ({far})", w.name, w.pos.x.floor(), w.pos.y, w.pos.z.floor()));
                }
                out
            }
            _ => vec!["Waypoints: /wp add <name>, /wp remove <name>, /wp list.".into()],
        }
    }

    /// The waypoints in the world you're in, with their colours.
    pub fn waypoints_here(&self) -> impl Iterator<Item = (&Waypoint, [u8; 3])> {
        let here = realm(self.player.body.pos.x);
        self.waypoints.iter().enumerate().filter(move |(_, w)| realm(w.pos.x) == here).map(|(i, w)| (w, colour(i)))
    }
}

#[cfg(test)]
mod waypoints_tests {
    use super::*;

    #[test]
    fn waypoints_are_added_listed_removed_and_saved() {
        let mut g = crate::game::tests::arena(234);
        g.player.body.pos = Vec3::new(10.3, 50.0, -4.7);
        assert!(is_command("/wp add Home") && is_command("/waypoint list") && !is_command("/w hi"));
        assert!(g.waypoint_command("/wp add Home")[0].contains("Home"));
        assert_eq!(g.waypoints[0].pos, Vec3::new(10.5, 50.0, -4.5));
        g.player.body.pos = Vec3::new(100.0, 60.0, 0.0);
        g.waypoint_command("/waypoint add The Mine");
        assert!(g.waypoint_command("/wp add home")[0].starts_with("Moved"), "names ignore case");
        assert_eq!(g.waypoints.len(), 2);
        let list = g.waypoint_command("/wp list");
        assert_eq!(list.len(), 3);
        assert!(list[2].contains("The Mine"));
        assert_eq!(decode(&encode(&g.waypoints)), g.waypoints);
        assert!(g.waypoint_command("/wp remove the mine")[0].contains("Forgot"));
        assert_eq!(g.waypoints.len(), 1);
        assert!(g.waypoint_command("/wp remove nowhere")[0].contains("No waypoint"));
        assert_eq!(g.waypoints_here().count(), 1);
        // Kept with the world.
        let back = Game::from_save(g.to_save());
        assert_eq!(back.waypoints, g.waypoints);
    }
}
