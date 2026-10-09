//! The Scorchlands, filled in.
//!
//! - **Fortresses** of Scorch Bricks stand over the lava sea: a hall in the
//!   middle and four long bridges out to little towers, cut straight through
//!   any rock in the way. **Sizzler Cages** in the hall (and at the end of a
//!   bridge) keep producing **Sizzlers** while anyone's near: they hover
//!   just above you and throw fireballs three at a time. They drop **Sizzle
//!   Rods**: one makes two **Sizzle Powder**, and powder and a Stare Pearl make
//!   two Staring Eyes (better than the Ember Shroom way), or brew Strength.
//! - **Weepers** drift over the open caverns: huge, slow and miserable, they
//!   open their eyes and cry a fireball that explodes where it lands. Hit a
//!   fireball (any fireball) and it goes back the way it came. A Weeper's
//!   **Tear** brews Regeneration.
//! - **Strutters** walk on lava like it's a pavement (and shiver blue when
//!   they're off it). Saddle one, hold an **Ember Shroom on a Stick** (a
//!   Fishing Stick and a shroom) and ride across the lava sea; feed them
//!   Ember Shrooms to breed them.
//! - **Snouts** live in camps of gilded rock with gold piled up and a chest.
//!   They leave you alone unless you hit one (or take their gold: break a
//!   gold block near them). Throw a gold ingot near one and it'll pick it up,
//!   admire it for a while, then throw something back: barter. Usually
//!   something useful, now and then a Stare Pearl or a potion, and very
//!   rarely the Music Disc "Oinkstep".
//!
//! Fireballs, cages and bartering run where the world lives; joined players
//! see the fireballs with the mob snapshots and can knock them back too.

use crate::block::*;
use crate::entity::MobKind;
use crate::game::Game;
use crate::noise::{hash2, Rng};
use crate::scorch::{LAVA_SEA, SCORCH_X};
use crate::sound::Sfx;
use crate::structures::{Kind, Site};
use crate::world::{Generator, CW};
use macroquad::math::{ivec3, IVec3, Mat4, Vec3};

/// Bridge level, and the Snouts' camps.
pub const FORTRESS_Y: i32 = LAVA_SEA + 10;
pub const CAMP_Y: i32 = LAVA_SEA + 5;
/// How far a fortress's bridges reach from the hall.
const ARM: i32 = 26;
/// ...and in a big one (newer worlds), which stays within three chunks of its own.
const BIG_ARM: i32 = 39;
/// Rolls under this (and over the camps') are a Snout Bastion.
const BASTION_ROLL: f32 = 0.034;
/// Rolls under this are a fortress, in newer worlds.
const BIG_FORT_ROLL: f32 = 0.004;
/// Seconds a Snout admires gold before it pays up.
pub const ADMIRE_SECS: f32 = 6.0;
/// Most Sizzlers a cage keeps around it.
const CAGE_CAP: usize = 5;

#[derive(Clone, Copy, Debug)]
pub struct Fireball {
    pub pos: Vec3,
    pub vel: Vec3,
    /// A Weeper's: explodes.
    pub big: bool,
    pub life: f32,
    /// The mob that threw it (it doesn't hit its thrower).
    pub shooter: u32,
    /// Knocked back by this player: it's theirs now.
    pub returned: Option<u32>,
}

impl Generator {
    /// Fortresses and Snout camps, out past the Scorchlands' edge.
    pub fn scorch_site(&self, cx: i32, cz: i32) -> Option<Site> {
        if cx * CW < SCORCH_X + 96 {
            return None;
        }
        let s = self.seed ^ 0xF07;
        let r = hash2(s, cx, cz);
        let ox = cx * CW + 5 + (hash2(s ^ 1, cx, cz) * 6.0) as i32;
        let oz = cz * CW + 5 + (hash2(s ^ 2, cx, cz) * 6.0) as i32;
        let seed = (hash2(s ^ 3, cx, cz) * u32::MAX as f32) as u32;
        let facing = (hash2(s ^ 4, cx, cz) * 4.0) as u8 % 4;
        if self.opts.version >= 2 && r < BIG_FORT_ROLL {
            // Rarer, and never two close together.
            let crowded = (-6..=6).any(|dz| (-6..=6).any(|dx| (dx, dz) != (0, 0) && hash2(s, cx + dx, cz + dz) < BIG_FORT_ROLL));
            (!crowded).then_some(Site { kind: Kind::Fortress, origin: ivec3(ox, FORTRESS_Y, oz), facing, seed })
        } else if self.opts.version >= 2 && r < 0.012 {
            None
        } else if r < 0.012 {
            Some(Site { kind: Kind::Fortress, origin: ivec3(ox, FORTRESS_Y, oz), facing, seed })
        } else if r < 0.024 {
            Some(Site { kind: Kind::SnoutCamp, origin: ivec3(ox, CAMP_Y, oz), facing, seed })
        } else if r < BASTION_ROLL {
            // A bastion is big: only where nothing else starts within three chunks (see bastion.rs).
            let crowded = (-3..=3).any(|dz| (-3..=3).any(|dx| (dx, dz) != (0, 0) && hash2(s, cx + dx, cz + dz) < BASTION_ROLL));
            (!crowded).then_some(Site { kind: Kind::Bastion, origin: ivec3(ox, crate::bastion::BASTION_Y, oz), facing, seed })
        } else {
            None
        }
    }
}

/// A fortress: the hall, four bridges on pillars, and a tower at each end.
/// `big` (newer worlds) is the full-size one: a two-storey hall, bridges half
/// as long again with arches under them and a roofed walk on two of them, and
/// taller towers with a lookout on top.
pub fn fortress_blocks(site: &Site, big: bool) -> Vec<(IVec3, Id)> {
    let mut out = Vec::new();
    let o = site.origin;
    let mut put = |x: i32, y: i32, z: i32, id: Id| out.push((o + ivec3(x, y, z), id));
    let (hh, top, arm, th, ttop): (i32, i32, i32, i32, i32) = if big { (9, 11, BIG_ARM, 3, 9) } else { (6, 6, ARM, 2, 4) };
    // The hall.
    for x in -hh..=hh {
        for z in -hh..=hh {
            for y in -1..=top {
                let edge = x.abs() == hh || z.abs() == hh;
                let door = (x.abs() <= 1 && z.abs() == hh || z.abs() <= 1 && x.abs() == hh) && (1..=3).contains(&y);
                // Upstairs: a gallery round the walls, open over the middle.
                let gallery = big && y == 6 && (x.abs() >= hh - 3 || z.abs() >= hh - 3);
                let window = big && edge && (7..=9).contains(&y) && (x + z).rem_euclid(4) == 0 && !(x.abs() == hh && z.abs() == hh);
                let id = if y <= 0 || y == top || gallery {
                    SCORCH_BRICKS
                } else if edge && !door && !window {
                    if y == 3 && (x + z) % 4 == 0 { GLOWROCK } else { SCORCH_BRICKS }
                } else {
                    AIR
                };
                put(x, y, z, id);
            }
            // Battlements.
            if big && (x.abs() == hh || z.abs() == hh) && (x + z).rem_euclid(2) == 0 {
                put(x, top + 1, z, SCORCH_BRICKS);
            }
        }
    }
    put(0, 1, 0, SIZZLER_CAGE);
    put(-4, 1, -4, CHEST);
    put(4, 1, 4, CHEST);
    if big {
        // Stairs up to the gallery in one corner, a second cage and chest up there.
        for s in 0..5 {
            put(-hh + 1, 1 + s, hh - 3 - s, SCORCH_BRICKS);
            put(-hh + 2, 1 + s, hh - 3 - s, SCORCH_BRICKS);
            put(-hh + 1, 6, hh - 4 - s, AIR);
            put(-hh + 2, 6, hh - 4 - s, AIR);
        }
        put(hh - 2, 7, -hh + 2, SIZZLER_CAGE);
        put(-hh + 2, 7, -hh + 2, CHEST);
        for (x, z) in [(-3, -3), (3, -3), (-3, 3), (3, 3)] {
            for y in 1..top {
                put(x, y, z, SCORCH_BRICKS);
            }
            put(x, top - 1, z + 1, LANTERN_HANGING);
        }
    }
    // Bridges, north, east, south, west.
    for (k, (dx, dz)) in [(0i32, -1i32), (1, 0), (0, 1), (-1, 0)].into_iter().enumerate() {
        let roofed = big && site.seed.wrapping_add(k as u32).is_multiple_of(2);
        for d in hh + 1..=arm {
            for w in -2..=2i32 {
                let (x, z) = (dx * d + dz.abs() * w, dz * d + dx.abs() * w);
                put(x, 0, z, SCORCH_BRICKS);
                if w.abs() == 2 {
                    put(x, 1, z, SCORCH_BRICKS);
                } else {
                    put(x, 1, z, AIR);
                }
                for y in 2..=4 {
                    // A roofed walk has walls with slits, posts and a roof.
                    let wall = roofed && w.abs() == 2 && !(y == 3 && d % 3 == 0);
                    put(x, y, z, if wall { SCORCH_BRICKS } else { AIR });
                }
                if roofed {
                    put(x, 5, z, SCORCH_BRICKS);
                    if w == 0 && d % 6 == 3 {
                        put(x, 4, z, LANTERN_HANGING);
                    }
                }
                // Pillars down into the lava every six blocks (with arches between, on the big ones).
                if d % 6 == 0 && w.abs() <= 1 {
                    for y in (LAVA_SEA - 3 - FORTRESS_Y)..0 {
                        put(x, y, z, SCORCH_BRICKS);
                    }
                } else if big && w.abs() <= 1 {
                    let m = (d % 6) as f32 - 3.0;
                    let depth = 1 + (3.0 - m.abs()).max(0.0).powi(2) as i32 / 3;
                    for y in -(4 - depth).max(1)..0 {
                        put(x, y, z, SCORCH_BRICKS);
                    }
                }
            }
        }
        // A tower at the end.
        let (tx, tz) = (dx * (arm + th + 1), dz * (arm + th + 1));
        for a in -th..=th {
            for b in -th..=th {
                for y in -1..=ttop {
                    let edge = a.abs() == th || b.abs() == th;
                    // The way in, from the bridge.
                    let toward = (dx != 0 && a * dx == -th && b.abs() <= 1) || (dz != 0 && b * dz == -th && a.abs() <= 1);
                    let slit = big && edge && y == 6 && (a == 0 || b == 0);
                    let id = if y <= 0 || y == ttop || (big && y == 5 && !(a.abs() <= 1 && b == th - 1)) || (edge && !(toward && y <= 2) && !slit) { SCORCH_BRICKS } else { AIR };
                    put(tx + a, y, tz + b, id);
                }
                if big {
                    // A lookout on top, with a crenellated wall.
                    if (a.abs() == th || b.abs() == th) && (a + b).rem_euclid(2) == 0 {
                        put(tx + a, ttop + 1, tz + b, SCORCH_BRICKS);
                    }
                    // Down to the lava.
                    if a.abs() == th && b.abs() == th {
                        for y in (LAVA_SEA - 3 - FORTRESS_Y)..-1 {
                            put(tx + a, y, tz + b, SCORCH_BRICKS);
                        }
                    }
                }
            }
        }
        if big {
            // A ladder-less way up: steps inside to the upper room.
            for s in 0..4 {
                put(tx - th + 1 + s, 1 + s, tz - th + 1, SCORCH_BRICKS);
            }
            put(tx - th + 4, 5, tz - th + 1, AIR);
            put(tx, ttop - 1, tz, LANTERN_HANGING);
        }
        // Two of the towers have a chest; one has a cage.
        match (site.seed.wrapping_add(k as u32)) % 4 {
            0 => put(tx, 1, tz, SIZZLER_CAGE),
            1 | 2 => put(tx, 1, tz, CHEST),
            _ => put(tx, 1, tz, GLOWROCK),
        }
    }
    out
}

/// A Snout camp: a gilded platform on pillars, low walls, gold piled up, a chest.
pub fn camp_blocks(site: &Site) -> Vec<(IVec3, Id)> {
    let mut out = Vec::new();
    let o = site.origin;
    let mut put = |x: i32, y: i32, z: i32, id: Id| out.push((o + ivec3(x, y, z), id));
    for x in -6..=6i32 {
        for z in -6..=6i32 {
            put(x, 0, z, if (x + z) % 3 == 0 { GILDED_SCORCHROCK } else { SCORCHROCK });
            let edge = x.abs() == 6 || z.abs() == 6;
            let gap = x.abs() <= 1 || z.abs() <= 1;
            for y in 1..=5 {
                put(x, y, z, if edge && !gap && y <= 2 { SCORCH_BRICKS } else { AIR });
            }
            if x.abs() == 6 && z.abs() == 6 {
                for y in (LAVA_SEA - 2 - CAMP_Y)..0 {
                    put(x, y, z, SCORCH_BRICKS);
                }
                put(x, 3, z, GLOWROCK);
            }
        }
    }
    // Gold, piled up the way Snouts like it.
    let mut rng = Rng::new(site.seed as u64 | 1);
    for _ in 0..5 {
        let (x, z) = (rng.int(-4, 4), rng.int(-4, 4));
        if x.abs() > 1 || z.abs() > 1 {
            put(x, 1, z, GOLD_BLOCK);
            if rng.chance(0.4) {
                put(x, 2, z, GOLD_BLOCK);
            }
        }
    }
    put(0, 1, 0, CHEST);
    out
}

/// A cage that spawns things: a fortress's Sizzler Cage or a dungeon's Monster Cage.
/// Cages, spawners and Trial Spawners (all kept track of in `World::cages`).
pub fn is_cage(id: Id) -> bool {
    id == SIZZLER_CAGE || id == SPAWNER || crate::trial::is_trial_spawner(id) || crate::creaking::is_heart(id)
}

/// What comes out of the cage at `p`: Sizzlers, or (in a dungeon) one kind of monster, the same each time.
pub fn cage_kind(id: Id, p: IVec3) -> MobKind {
    if id == SIZZLER_CAGE {
        return MobKind::Sizzler;
    }
    let r = crate::noise::hash3(0xCA6E, p.x, p.y, p.z);
    if r < 0.5 {
        MobKind::Groaner
    } else if r < 0.75 {
        MobKind::Rattler
    } else {
        MobKind::Webber
    }
}

/// What a Snout hands over for a gold ingot.
pub fn barter(rng: &mut Rng) -> (Id, u8) {
    // (item, fewest, most, weight)
    let table: [(Id, u8, u8, u32); 11] = [
        (OBSIDIAN, 1, 1, 40),
        (GRAVEL, 8, 16, 40),
        (SCORCHROCK, 8, 16, 40),
        (STRING, 3, 9, 20),
        (EMBER_SHROOM, 2, 6, 20),
        (ARROW, 6, 12, 20),
        (IRON, 2, 5, 10),
        (PEARL, 2, 4, 10),
        (crate::potions::potion_item(crate::potions::Potion::FireResistance, false), 1, 1, 8),
        (SHROOM_STICK, 1, 1, 5),
        (DISC_FIRST + 7, 1, 1, 1),
    ];
    let total: u32 = table.iter().map(|t| t.3).sum();
    let mut roll = rng.int(0, total as i32 - 1) as u32;
    for (item, lo, hi, w) in table {
        if roll < w {
            return (item, rng.int(lo as i32, hi as i32) as u8);
        }
        roll -= w;
    }
    (GRAVEL, 8)
}

impl Game {
    pub fn spawn_fireball(&mut self, from: Vec3, vel: Vec3, big: bool, shooter: u32) {
        self.fireballs.push(Fireball { pos: from, vel, big, life: 8.0, shooter, returned: None });
        self.sfx(if big { Sfx::Shriek } else { Sfx::Hiss }, Some(from));
    }

    /// Fly, and burst on whatever they hit (where the world lives; joined players just watch).
    pub fn fireballs_tick(&mut self, dt: f32) {
        for f in self.fireballs.iter_mut() {
            f.pos += f.vel * dt;
            f.life -= dt;
        }
        if self.is_client() {
            self.fireballs.retain(|f| f.life > 0.0);
            return;
        }
        let mut i = 0;
        while i < self.fireballs.len() {
            let f = self.fireballs[i];
            if f.life <= 0.0 {
                self.fireballs.swap_remove(i);
                continue;
            }
            let r = if f.big { 1.4 } else { 0.7 };
            let solid = is_solid(self.world.get_v(f.pos.floor().as_ivec3()));
            // Anywhere on someone, head to toe.
            let touches = |feet: Vec3| Vec3::new(feet.x - f.pos.x, 0.0, feet.z - f.pos.z).length() < r && f.pos.y > feet.y - r * 0.5 && f.pos.y < feet.y + 1.8 + r * 0.5;
            let me = !self.dedicated && self.dead.is_none() && !self.creative && Some(self.my_id) != f.returned && touches(self.player.body.pos);
            let peer = self.peers.iter().find(|&(&id, p)| p.alive() && Some(id) != f.returned && touches(p.target)).map(|(&id, _)| id);
            let mob = self.mobs.iter().position(|m| (m.id != f.shooter || f.returned.is_some()) && (m.body.pos + Vec3::Y * m.body.height * 0.5).distance(f.pos) < m.body.half + r * 0.6 + 0.2);
            if !(solid || me || peer.is_some() || mob.is_some()) {
                i += 1;
                continue;
            }
            self.fireballs.swap_remove(i);
            let at = f.pos - f.vel.normalize_or_zero() * 0.4;
            let cause = if f.big { "was fireballed by a Weeper. It was very upset" } else { "was fireballed by a Sizzler" };
            if f.big {
                // A Weeper's own fireball, sent back: one-shots it.
                if let Some(j) = mob
                    && self.mobs[j].kind == MobKind::Weeper
                    && let Some(who) = f.returned
                {
                    self.mobs[j].damage(1000.0, at);
                    self.mobs[j].last_attacker = who;
                    let name = if who == self.my_id { self.player_name.clone() } else { self.peers.get(&who).map(|p| p.name.clone()).unwrap_or_default() };
                    self.advance_for(&name, "return_to_sender");
                }
                self.explode(at, 1.3, cause);
                for _ in 0..4 {
                    let p = (at + Vec3::new(self.rng.range(-1.5, 1.5), self.rng.range(-1.0, 1.0), self.rng.range(-1.5, 1.5))).floor().as_ivec3();
                    if crate::fire::can_burn_at(&self.world, p) {
                        self.world.set_v(p, FIRE);
                    }
                }
                continue;
            }
            if me {
                self.hurt_player_from(5.0, cause, Some(at), false);
                if !self.has_effect(crate::potions::Potion::FireResistance) {
                    self.on_fire = self.on_fire.max(4.0);
                }
            }
            if let Some(id) = peer {
                self.hurt_peer(id, 5.0, cause, f.vel.normalize_or_zero() * 2.0);
            }
            if let Some(j) = mob {
                let m = &mut self.mobs[j];
                if !m.kind.fireproof() {
                    m.damage(5.0, at);
                    m.on_fire = m.on_fire.max(4.0);
                }
                if let Some(who) = f.returned {
                    m.last_attacker = who;
                }
            }
            if solid {
                let p = at.floor().as_ivec3();
                if crate::fire::can_burn_at(&self.world, p) {
                    self.world.set_v(p, FIRE);
                }
            }
            self.smoke(at, 4, 0.2);
        }
    }

    /// Swat at a fireball near where `who` is looking: it goes back the way they're facing.
    pub fn deflect_fireball(&mut self, eye: Vec3, dir: Vec3, who: u32) -> bool {
        let best = self
            .fireballs
            .iter()
            .enumerate()
            .filter_map(|(i, f)| {
                let to = f.pos - eye;
                let along = to.dot(dir);
                let off = (to - dir * along).length();
                (along > 0.0 && along < 4.5 && off < if f.big { 1.4 } else { 0.8 }).then_some((i, along))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let Some((i, _)) = best else { return false };
        if self.is_client() {
            let at = self.fireballs[i].pos;
            self.net_send_msg(crate::net::Msg::Deflect { at, dir });
            self.fireballs[i].vel = dir * self.fireballs[i].vel.length().max(10.0) * 1.2;
            return true;
        }
        let f = &mut self.fireballs[i];
        f.vel = dir.normalize_or_zero() * f.vel.length().max(10.0) * 1.2;
        f.returned = Some(who);
        f.life = 8.0;
        let at = f.pos;
        self.sfx(Sfx::Thunk, Some(at));
        true
    }

    /// A joined player swatted at a fireball (believable ones only).
    pub fn host_deflect(&mut self, from: u32, at: Vec3, dir: Vec3) {
        let Some(eye) = self.peers.get(&from).map(|p| p.target + Vec3::Y * crate::player::EYE) else { return };
        if !at.is_finite() || !dir.is_finite() || eye.distance(at) > 6.0 {
            return;
        }
        let dir = dir.normalize_or_zero();
        if let Some(f) = self.fireballs.iter_mut().filter(|f| f.pos.distance(at) < 2.5).min_by(|a, b| a.pos.distance(at).total_cmp(&b.pos.distance(at))) {
            f.vel = dir * f.vel.length().max(10.0) * 1.2;
            f.returned = Some(from);
            f.life = 8.0;
        }
    }

    pub fn draw_fireballs(&self, g: &mut crate::render::DynGeo) {
        for f in &self.fireballs {
            let s = if f.big { 1.0 } else { 0.32 };
            g.begin(crate::render::Pass::Opaque, [2.2, 2.0, 1.6, 1.0], false);
            let spin = Mat4::from_rotation_y(f.life * 6.0) * Mat4::from_rotation_x(f.life * 4.0);
            let m = Mat4::from_translation(f.pos) * spin * Mat4::from_scale(Vec3::splat(s)) * Mat4::from_translation(Vec3::splat(-0.5));
            g.cube(&m, [crate::texture::T_FIREBALL; 6], 1.0, [0.0, 0.0, 1.0, 1.0]);
        }
    }

    /// Cages near a player keep a few Sizzlers about (where the world lives).
    pub fn cages_tick(&mut self, dt: f32) {
        if self.is_client() || !self.rules.difficulty.monsters() {
            return;
        }
        self.cage_timer -= dt;
        if self.cage_timer > 0.0 {
            return;
        }
        self.cage_timer = 1.0;
        let players = self.player_spots();
        let cages: Vec<IVec3> = self.world.cages.iter().copied().filter(|p| matches!(self.world.get_v(*p), SIZZLER_CAGE | SPAWNER) && players.iter().any(|(_, at, _)| at.distance(p.as_vec3()) < 16.0)).collect();
        for c in cages {
            let kind = cage_kind(self.world.get_v(c), c);
            let centre = c.as_vec3() + Vec3::splat(0.5);
            self.smoke(centre, 2, 0.3);
            if !self.rng.chance(0.12) {
                continue;
            }
            let near = self.mobs.iter().filter(|m| m.kind == kind && m.body.pos.distance(centre) < 12.0).count();
            if near >= CAGE_CAP {
                continue;
            }
            for _ in 0..self.rng.int(1, 3) {
                let p = centre + Vec3::new(self.rng.range(-2.5, 2.5), self.rng.range(0.0, 1.5), self.rng.range(-2.5, 2.5));
                if (0..2).all(|h| self.world.get_v((p + Vec3::Y * h as f32).floor().as_ivec3()) == AIR) {
                    self.alloc_mob(kind, p);
                    self.smoke(p + Vec3::Y, 8, 0.4);
                }
            }
        }
    }

    /// Snouts go for thrown gold, admire it, and pay up (where the world lives).
    pub fn snouts_tick(&mut self, dt: f32) {
        if self.is_client() {
            return;
        }
        let players = self.player_spots();
        let mut pickups: Vec<(usize, usize)> = Vec::new();
        let mut pays: Vec<usize> = Vec::new();
        for (i, m) in self.mobs.iter_mut().enumerate().filter(|(_, m)| m.kind == MobKind::Snout) {
            if m.seed == 1 {
                m.restock -= dt;
                if m.restock <= 0.0 {
                    pays.push(i);
                }
                continue;
            }
            if m.angry {
                continue;
            }
            let pos = m.body.pos;
            let gold = self.drops.iter().enumerate().filter(|(_, d)| d.item == GOLD_INGOT && d.age > d.delay && d.body.pos.distance(pos) < 8.0).min_by(|a, b| a.1.body.pos.distance(pos).total_cmp(&b.1.body.pos.distance(pos))).map(|(j, d)| (j, d.body.pos));
            match gold {
                Some((j, at)) if at.distance(pos) < 1.4 => pickups.push((i, j)),
                Some((_, at)) => m.goal = Some(at),
                None => m.goal = None,
            }
        }
        pickups.sort_by(|a, b| b.1.cmp(&a.1));
        for (i, j) in pickups {
            if j >= self.drops.len() {
                continue;
            }
            if self.drops[j].n > 1 {
                self.drops[j].n -= 1;
            } else {
                self.drops.remove(j);
            }
            let m = &mut self.mobs[i];
            m.seed = 1;
            m.restock = ADMIRE_SECS;
            m.goal = None;
            let at = m.body.pos + Vec3::Y * 1.6;
            self.sfx(Sfx::Oink, Some(at));
        }
        for i in pays {
            let (from, nearest) = {
                let m = &mut self.mobs[i];
                m.seed = 0;
                let from = m.body.pos + Vec3::Y * 1.3;
                let nearest = players.iter().min_by(|a, b| a.1.distance(from).total_cmp(&b.1.distance(from))).map(|p| (p.0.clone(), p.1));
                (from, nearest)
            };
            let (item, n) = barter(&mut self.rng);
            let to = nearest.as_ref().map(|p| p.1).unwrap_or(from + Vec3::X * 2.0);
            self.fling_to(from, to, item, n);
            self.sfx(Sfx::Oink, Some(from));
            if let Some((who, at)) = nearest
                && at.distance(from) < 16.0
            {
                self.advance_for(&who, "fair_trade");
                if item == DISC_FIRST + 7 {
                    self.advance_for(&who, "oinkstep");
                }
            }
        }
    }

    /// Breaking a Snout's gold makes every Snout nearby cross.
    pub fn gold_taken(&mut self, pos: IVec3, old: Id) {
        if matches!(old, GOLD_BLOCK | GILDED_SCORCHROCK | CHEST) && crate::scorch::in_scorch(pos.x as f32) {
            let at = pos.as_vec3();
            for m in self.mobs.iter_mut().filter(|m| m.kind == MobKind::Snout && m.body.pos.distance(at) < 16.0) {
                m.angry = true;
                m.seed = 0;
            }
        }
    }

    /// Ride a saddled Strutter (where the world lives): saddle it first, like a Galloper (no taming needed).
    pub fn strutter_interact(&mut self, i: usize, item: Id, rider: u32) -> crate::animals::Interaction {
        use crate::animals::Interaction;
        let pos = self.mobs[i].body.pos + Vec3::Y * 1.6;
        let m = &mut self.mobs[i];
        if item == SADDLE && !m.saddled && m.baby <= 0.0 {
            m.saddled = true;
            m.persistent = true;
            self.sfx(Sfx::Place(crate::sound::Mat::Wood), Some(pos));
            return Interaction::Ate;
        }
        if m.kind.breed_food().contains(&item) && m.ready_to_breed() {
            m.love = crate::animals::LOVE_SECS;
            m.persistent = true;
            self.hearts(pos, 4);
            return Interaction::Ate;
        }
        if m.saddled && m.rider == 0 && m.baby <= 0.0 {
            m.rider = rider;
            return Interaction::Mounted(m.id);
        }
        Interaction::Nothing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fortresses_have_cages_chests_and_bridges() {
        let site = Site { kind: Kind::Fortress, origin: ivec3(SCORCH_X + 500, FORTRESS_Y, 40), facing: 0, seed: 7 };
        let b = fortress_blocks(&site, false);
        assert!(b.iter().filter(|x| x.1 == SIZZLER_CAGE).count() >= 1);
        assert!(b.iter().filter(|x| x.1 == CHEST).count() >= 3);
        assert!(b.iter().any(|x| x.0 == site.origin + ivec3(0, 0, -ARM)), "a bridge reaches out north");
        // The big one is bigger, and stays within three chunks of the one it starts in.
        let big = fortress_blocks(&site, true);
        assert!(big.len() > b.len() * 2);
        assert!(big.iter().any(|x| x.0 == site.origin + ivec3(0, 0, -BIG_ARM)));
        let reach = big.iter().map(|x| (x.0 - site.origin).x.abs().max((x.0 - site.origin).z.abs())).max().unwrap();
        assert!(reach + 11 < 4 * CW && reach > 5 + 2 * CW, "{reach}");
        let camp = camp_blocks(&Site { kind: Kind::SnoutCamp, ..site });
        assert!(camp.iter().any(|x| x.1 == GOLD_BLOCK) && camp.iter().any(|x| x.1 == CHEST));
    }

    #[test]
    fn bartering_is_mostly_rubbish_and_sometimes_oinkstep() {
        let mut rng = Rng::new(5);
        let mut seen = std::collections::HashSet::new();
        for _ in 0..4000 {
            let (item, n) = barter(&mut rng);
            assert!(n >= 1);
            seen.insert(item);
        }
        assert!(seen.contains(&OBSIDIAN) && seen.contains(&PEARL));
        assert!(seen.contains(&(DISC_FIRST + 7)), "Oinkstep turns up eventually");
    }

    #[test]
    fn snouts_take_gold_and_pay_up() {
        let mut g = crate::game::tests::arena(92);
        let at = g.player.body.pos + Vec3::new(2.0, 0.0, 0.0);
        let id = g.alloc_mob(MobKind::Snout, at);
        g.pop_drop(at + Vec3::new(0.3, 0.2, 0.0), GOLD_INGOT, 1);
        for d in g.drops.iter_mut() {
            d.age = 10.0;
        }
        g.snouts_tick(0.1);
        let m = g.mobs.iter().find(|m| m.id == id).unwrap();
        assert_eq!(m.seed, 1, "admiring the gold");
        assert!(!g.drops.iter().any(|d| d.item == GOLD_INGOT));
        for _ in 0..70 {
            g.snouts_tick(0.1);
        }
        assert_eq!(g.mobs.iter().find(|m| m.id == id).unwrap().seed, 0);
        assert!(!g.drops.is_empty(), "something thrown back");
        // Take their gold and they're cross.
        g.gold_taken(IVec3::new(SCORCH_X + 100, 50, 0), GOLD_BLOCK);
    }

    #[test]
    fn fireballs_burn_and_can_be_knocked_back() {
        let mut g = crate::game::tests::arena(91);
        g.creative = false;
        let eye = g.player.eye();
        let dir = g.player.look_dir();
        g.spawn_fireball(eye + dir * 3.0, -dir * 10.0, false, 0);
        assert!(g.deflect_fireball(eye, dir, g.my_id));
        assert!(g.fireballs[0].vel.dot(dir) > 0.0, "it's going back");
        g.fireballs.clear();
        let h = g.player.health;
        g.spawn_fireball(eye + dir * 3.0, -dir * 10.0, false, 0);
        for _ in 0..20 {
            g.fireballs_tick(0.05);
        }
        assert!(g.player.health < h, "it hit");
        assert!(g.fireballs.is_empty());
    }
}
