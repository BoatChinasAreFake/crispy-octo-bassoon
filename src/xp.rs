//! Experience: points from mobs, ores, smelting and fishing, counted into
//! levels with Minecraft's curve, and spent at the anvil (see anvil.rs).
//!
//! Points come as little green orbs that float toward the nearest player.
//! Like items on the ground, orbs live where the world lives; the host also
//! keeps every joined player's total and tells them what it is, so levels
//! (and what they buy) can't be made up.

use crate::block::*;
use crate::entity::{move_body, Body};
use crate::game::Game;
use crate::net::Msg;
use crate::noise::Rng;
use crate::render::{DynGeo, Pass};
use crate::sound::Sfx;
use crate::texture::T_XP_ORB;
use crate::world::World;
use macroquad::math::Vec3;

/// Orbs drift toward a player this close.
const ATTRACT: f32 = 8.0;
const PICKUP: f32 = 1.2;
const DESPAWN_SECS: f32 = 300.0;
const MAX_ORBS: usize = 512;

/// Points needed to go from `level` to the next (Minecraft's curve).
pub fn level_cost(level: u32) -> u32 {
    match level {
        0..=15 => 2 * level + 7,
        16..=30 => 5 * level - 38,
        _ => 9 * level - 158,
    }
}

/// Total points at the start of `level`.
pub fn points_for_level(level: u32) -> u32 {
    (0..level).map(level_cost).sum()
}

/// (level, progress through it 0..1) for a points total.
pub fn level_of(points: u32) -> (u32, f32) {
    let mut level = 0;
    let mut left = points;
    while left >= level_cost(level) {
        left -= level_cost(level);
        level += 1;
    }
    (level, left as f32 / level_cost(level) as f32)
}

/// Take `levels` off a points total, keeping the progress through the current level.
pub fn spend_levels(points: u32, levels: u32) -> u32 {
    let (level, _) = level_of(points);
    let into = points - points_for_level(level);
    let new_level = level.saturating_sub(levels);
    points_for_level(new_level) + into.min(level_cost(new_level).saturating_sub(1))
}

/// Points dropped on death: 7 per level, at most 100 (the rest is lost).
pub fn death_drop(points: u32) -> u32 {
    (level_of(points).0 * 7).min(100)
}

/// Points from breaking an ore.
pub fn ore_xp(id: Id, rng: &mut Rng) -> u32 {
    match id {
        COAL_ORE => rng.int(0, 2) as u32,
        IRON_ORE | GOLD_ORE => 1,
        DIAMOND_ORE => rng.int(3, 7) as u32,
        ZAP_ORE => rng.int(1, 5) as u32,
        _ => 0,
    }
}

/// Points per item taken out of a furnace (fractions are rolled).
pub fn smelt_xp(output: Id) -> f32 {
    match output {
        GLASS | STONE | COOKED_BOOT => 0.1,
        COAL => 0.15,
        _ => 0.35,
    }
}

/// Round `amount` up or down at random, keeping the average.
pub fn roll(amount: f32, rng: &mut Rng) -> u32 {
    let whole = amount.floor();
    whole as u32 + rng.chance(amount - whole) as u32
}

pub struct XpOrb {
    pub id: u32,
    pub value: u16,
    pub body: Body,
    pub age: f32,
    /// Joined players: the host's latest position.
    pub net_pos: Vec3,
}

impl XpOrb {
    fn draw(&self, g: &mut DynGeo, world: &World, clock: f32, eye: Vec3) {
        let s = 0.12 + (self.value as f32).sqrt() * 0.025;
        let at = self.body.pos + Vec3::Y * (0.15 + (clock * 3.0 + self.id as f32).sin() * 0.05);
        let sky = world.sky_shade(at.x.floor() as i32, at.y.floor() as i32 + 1, at.z.floor() as i32);
        // A camera-facing square that pulses a little.
        let to_eye = (eye - at).normalize_or(Vec3::Z);
        let right = Vec3::Y.cross(to_eye).normalize_or(Vec3::X);
        let up = to_eye.cross(right);
        let k = s * (1.0 + (clock * 6.0 + self.id as f32).sin() * 0.12);
        let c = [at - right * k - up * k, at + right * k - up * k, at + right * k + up * k, at - right * k + up * k];
        g.quad(c, T_XP_ORB, [0.0, 0.0, 1.0, 1.0], [1.0, sky.max(0.8)]);
    }
}

/// Split points into orbs the way Minecraft does (big ones first).
fn split(mut points: u32) -> Vec<u16> {
    let mut v = Vec::new();
    for size in [149, 73, 37, 17, 7, 3, 1] {
        while points >= size && v.len() < 32 {
            v.push(size as u16);
            points -= size;
        }
    }
    v
}

impl Game {
    /// The local player's level and progress through it.
    pub fn level(&self) -> (u32, f32) {
        level_of(self.xp)
    }

    /// Points for the local player (single player or host; joined players are told by the host).
    pub fn add_xp(&mut self, points: u32) {
        if points == 0 {
            return;
        }
        let before = level_of(self.xp).0;
        self.xp = self.xp.saturating_add(points).min(1 << 24);
        let after = level_of(self.xp).0;
        self.sfx(Sfx::Pop, None);
        if after > before && after.is_multiple_of(5) {
            self.msg(format!("Level {after}! Your anvil is impressed. Mildly."));
        }
        if after >= 30 {
            self.advance("level_30");
        }
    }

    /// Points for a joined player (kept on the host, which tells them their total).
    pub fn give_peer_xp(&mut self, from: u32, points: u32) {
        let Some(p) = self.peers.get_mut(&from) else { return };
        p.ledger.xp = p.ledger.xp.saturating_add(points).min(1 << 24);
        let total = p.ledger.xp;
        self.net_send_to(from, Msg::Xp { points: total });
    }

    /// Orbs worth `points` in total, popping out at `at` (where the world lives).
    pub fn spawn_orbs(&mut self, at: Vec3, points: u32) {
        if self.is_client() || self.creative {
            return;
        }
        for value in split(points) {
            self.next_orb_id = self.next_orb_id.wrapping_add(1).max(1);
            let r = &mut self.rng;
            let mut body = Body::new(at, 0.125, 0.25);
            body.vel = Vec3::new(r.range(-2.0, 2.0), r.range(2.0, 5.0), r.range(-2.0, 2.0));
            self.orbs.push(XpOrb { id: self.next_orb_id, value, body, age: 0.0, net_pos: at });
        }
        if self.orbs.len() > MAX_ORBS {
            let extra = self.orbs.len() - MAX_ORBS;
            self.orbs.drain(..extra);
        }
    }

    /// Where the world lives: orbs fall, drift toward the nearest player and are collected.
    pub fn orbs_tick(&mut self, dt: f32) {
        // Everyone who can collect: (None = the local player, or a joined player's id, and their middle).
        let mut players: Vec<(Option<u32>, Vec3)> = Vec::new();
        if !self.dedicated && !self.menu && self.dead.is_none() {
            players.push((None, self.player.body.pos + Vec3::Y * 0.9));
        }
        players.extend(self.peers.iter().filter(|(_, p)| p.alive()).map(|(id, p)| (Some(*id), p.target + Vec3::Y * 0.9)));
        let mut collected: Vec<(Option<u32>, u32)> = Vec::new();
        for o in self.orbs.iter_mut() {
            o.age += dt;
            let p = o.body.pos;
            if !self.world.is_loaded(p.x.floor() as i32, p.z.floor() as i32) {
                continue;
            }
            let nearest = players.iter().map(|(who, at)| (*who, *at, at.distance(p))).min_by(|a, b| a.2.total_cmp(&b.2));
            match nearest {
                Some((who, _, d)) if d < PICKUP && o.age > 0.4 => {
                    collected.push((who, o.value as u32));
                    o.age = f32::INFINITY;
                    continue;
                }
                Some((_, at, d)) if d < ATTRACT => {
                    let pull = (at - p).normalize_or_zero() * (1.0 - d / ATTRACT) * 30.0;
                    o.body.vel += pull * dt;
                    o.body.vel *= (1.0 - 2.0 * dt).max(0.0);
                }
                _ => {
                    o.body.vel.y -= 12.0 * dt;
                    if o.body.on_ground {
                        let k = (1.0 - 8.0 * dt).max(0.0);
                        o.body.vel.x *= k;
                        o.body.vel.z *= k;
                    }
                }
            }
            move_body(&self.world, &mut o.body, dt, false);
        }
        self.orbs.retain(|o| o.age < DESPAWN_SECS && o.body.pos.y > -16.0);
        for (who, points) in collected {
            match who {
                None => self.add_xp(points),
                Some(id) => self.give_peer_xp(id, points),
            }
        }
    }

    /// The host tells joined players where the orbs are.
    pub fn send_orbs(&mut self, dt: f32) {
        self.orb_sync += dt;
        if self.orb_sync < 0.1 {
            return;
        }
        self.orb_sync = 0.0;
        if self.orbs.is_empty() && self.orbs_sent_empty {
            return;
        }
        self.orbs_sent_empty = self.orbs.is_empty();
        let list = self.orbs.iter().map(|o| (o.id, o.body.pos, o.value)).collect();
        self.net_broadcast(Msg::Orbs(list));
    }

    /// Joined players: the host's orbs.
    pub fn apply_orbs(&mut self, list: Vec<(u32, Vec3, u16)>) {
        let old = std::mem::take(&mut self.orbs);
        for (id, pos, value) in list {
            if !pos.is_finite() {
                continue;
            }
            let at = old.iter().find(|o| o.id == id).map(|o| o.body.pos).unwrap_or(pos);
            self.orbs.push(XpOrb { id, value, body: Body::new(at, 0.125, 0.25), age: 0.0, net_pos: pos });
        }
    }

    /// Joined players: glide toward the host's positions.
    pub fn client_orbs(&mut self, dt: f32) {
        let k = (dt * 15.0).min(1.0);
        for o in self.orbs.iter_mut() {
            o.body.pos += (o.net_pos - o.body.pos) * k;
        }
    }

    pub fn draw_orbs(&self, g: &mut DynGeo, eye: Vec3, range: f32) {
        if self.orbs.is_empty() {
            return;
        }
        g.begin(Pass::Opaque, [1.0; 4], true);
        for o in self.orbs.iter().filter(|o| o.body.pos.distance(eye) < range) {
            o.draw(g, &self.world, self.clock, eye);
        }
    }

    /// Where the world lives: a joined player died. Their points fall out
    /// (some of them), unless the world keeps inventories.
    pub fn peer_died(&mut self, from: u32) {
        if self.rules.keep_inventory {
            return;
        }
        let Some((points, at)) = self.peers.get_mut(&from).map(|p| (std::mem::take(&mut p.ledger.xp), p.target)) else { return };
        self.spawn_orbs(at + Vec3::Y * 0.5, death_drop(points));
        self.net_send_to(from, Msg::Xp { points: 0 });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_follow_minecrafts_curve() {
        assert_eq!(level_of(0), (0, 0.0));
        assert_eq!(level_of(7).0, 1);
        assert_eq!(points_for_level(16), 352);
        assert_eq!(points_for_level(31), 1507);
        assert_eq!(level_of(1507).0, 31);
        assert_eq!(level_of(1506).0, 30);
        // Spending levels keeps progress into the level.
        let p = points_for_level(10) + 5;
        assert_eq!(spend_levels(p, 3), points_for_level(7) + 5);
        assert_eq!(spend_levels(p, 50), 5);
        assert_eq!(death_drop(points_for_level(5)), 35);
        assert_eq!(death_drop(points_for_level(40)), 100);
        assert_eq!(split(60).iter().map(|v| *v as u32).sum::<u32>(), 60);
        let mut rng = Rng::new(1);
        let total: u32 = (0..1000).map(|_| roll(0.35, &mut rng)).sum();
        assert!((250..450).contains(&total), "{total}");
    }
}
