//! The Hollow: the end of the road. A floating island in a starry void, far
//! west, guarded by the Hollow Wyrm.
//!
//! Getting there:
//! 1. Make **Staring Eyes** (a Stare Pearl and an Ember Shroom). Throw one
//!    (right-click) and it drifts off toward the nearest **Crypt**, a buried
//!    room of stone bricks; follow, and pick it up again (usually).
//! 2. Every Crypt has a ring of twelve **Eye Frames** around a dark pit. Put
//!    an eye in every frame and the pit opens into a portal.
//! 3. Jump in. You arrive on the edge of the island.
//!
//! The **Hollow Wyrm** circles the island and swoops at whoever is there,
//! healing from the **Wyrm Crystals** on top of the obsidian pillars (break
//! them first: they blow up). Beat it for a pile of experience, the **Wyrm
//! Egg**, and a portal home in the middle of the island. Fall off the island
//! and the void gets you.
//!
//! Like the Scorchlands, the Hollow lives in the same world as everything
//! else (west of `HOLLOW_X`, behind a band of bedrock), so saving,
//! multiplayer and the rest work unchanged. Portals are walked into like
//! Scorchlands ones (`Msg::UsePortal`).

use crate::block::*;
use crate::dims::Dim;
use crate::entity::{Mob, MobEvent, MobKind};
use crate::game::Game;
use crate::noise::{hash2, hash3};
use crate::sound::Sfx;
use crate::world::{Generator, World, CH, CW};
use macroquad::math::{ivec3, IVec3, Vec3};

/// Where the Hollow used to start, when it shared the ordinary world's map
/// (blocks west; see dims.rs), and its island's middle then: the Hollow is
/// still generated there (`GEN_ORIGIN`, in generator coordinates).
pub const HOLLOW_X: i32 = -32_768;
pub const GEN_ORIGIN: IVec3 = IVec3::new(HOLLOW_X - 1024, 50, 0);
/// The island's middle, in the Hollow's own coordinates.
pub const ORIGIN: IVec3 = IVec3::new(0, 50, 0);
/// The island's radius, and the pillars around its middle.
pub const ISLAND: f32 = 64.0;
pub const PILLARS: usize = 8;
pub const PILLAR_RING: f32 = 34.0;
/// Crypts: one somewhere in each region this big (never right by spawn).
const CRYPT_REGION: i32 = 640;
/// Where Crypts sit, underground.
const CRYPT_Y: i32 = 18;
/// The Wyrm's reach when it swoops, and how hard it hits.
const WYRM_REACH: f32 = 3.5;
const WYRM_DAMAGE: f32 = 7.0;

/// The pillars' spots (x, z) and heights.
pub fn pillars(seed: u32) -> Vec<(i32, i32, i32)> {
    (0..PILLARS)
        .map(|i| {
            let a = i as f32 / PILLARS as f32 * std::f32::consts::TAU;
            let x = GEN_ORIGIN.x + (a.cos() * PILLAR_RING) as i32;
            let z = GEN_ORIGIN.z + (a.sin() * PILLAR_RING) as i32;
            let h = 62 + (hash2(seed ^ 0x9111, i as i32, 0) * 14.0) as i32;
            (x, z, h)
        })
        .collect()
}

/// Outer islands: at most one per cell this big, beyond the main island.
const OUTER_CELL: i32 = 80;
const OUTER_NEAR: f32 = 110.0;
const OUTER_FAR: f32 = 700.0;

/// The outer island in cell (gx, gz) (cells counted from `ORIGIN`), if any:
/// its middle (the surface), radius, and whether a spire with loot stands on it.
pub fn outer_island(seed: u32, gx: i32, gz: i32) -> Option<(IVec3, i32, bool)> {
    if hash2(seed ^ 0x0A7E, gx, gz) > 0.55 {
        return None;
    }
    let x = GEN_ORIGIN.x + gx * OUTER_CELL + 16 + (hash2(seed ^ 0x0A7F, gx, gz) * (OUTER_CELL - 32) as f32) as i32;
    let z = GEN_ORIGIN.z + gz * OUTER_CELL + 16 + (hash2(seed ^ 0x0A80, gx, gz) * (OUTER_CELL - 32) as f32) as i32;
    let d = ((x - GEN_ORIGIN.x) as f32).hypot((z - GEN_ORIGIN.z) as f32);
    if !(OUTER_NEAR..OUTER_FAR).contains(&d) {
        return None;
    }
    let y = GEN_ORIGIN.y - 6 + (hash2(seed ^ 0x0A81, gx, gz) * 14.0) as i32;
    let r = 9 + (hash2(seed ^ 0x0A82, gx, gz) * 8.0) as i32;
    Some((ivec3(x, y, z), r, hash2(seed ^ 0x0A83, gx, gz) < 0.6))
}

/// Outer islands that might reach (x, z).
fn outer_islands_near(seed: u32, x: i32, z: i32) -> Vec<(IVec3, i32, bool)> {
    let (gx, gz) = ((x - GEN_ORIGIN.x).div_euclid(OUTER_CELL), (z - GEN_ORIGIN.z).div_euclid(OUTER_CELL));
    let mut v = Vec::new();
    for dz in -1..=1 {
        for dx in -1..=1 {
            v.extend(outer_island(seed, gx + dx, gz + dz));
        }
    }
    v
}

/// A spire's blocks relative to its island's middle: a hollow obsidian tower
/// with a Hollow Stone floor, a glowing top, and the chest inside.
pub fn spire_block(dx: i32, dy: i32, dz: i32) -> Option<Id> {
    let (ax, az) = (dx.abs(), dz.abs());
    if ax > 2 || az > 2 || !(1..=9).contains(&dy) {
        return None;
    }
    let wall = ax == 2 || az == 2;
    let door = dz == 2 && dx == 0 && (1..=2).contains(&dy);
    Some(match dy {
        9 => {
            if ax <= 1 && az <= 1 { GLOWROCK } else { OBSIDIAN }
        }
        1 if dx == 0 && dz == 0 => HOLLOW_STONE,
        2 if dx == 0 && dz == 0 => CHEST,
        _ if door => AIR,
        _ if wall && !(ax == 2 && az == 2 && dy % 3 == 0) => OBSIDIAN,
        _ if wall => GLOWROCK,
        _ => AIR,
    })
}

/// The chest in each outer spire in chunk (cx, cz), with a seed for its loot
/// (all in generator coordinates).
pub fn spire_chests(seed: u32, cx: i32, cz: i32) -> Vec<(IVec3, u32)> {
    let mid = ivec3(cx * CW + CW / 2, 0, cz * CW + CW / 2);
    outer_islands_near(seed, mid.x, mid.z)
        .into_iter()
        .filter(|(_, _, spire)| *spire)
        .map(|(c, _, _)| c + ivec3(0, 2, 0))
        .filter(|p| p.x.div_euclid(CW) == cx && p.z.div_euclid(CW) == cz)
        .map(|p| (p, seed ^ (p.x as u32).wrapping_mul(2_654_435_761) ^ p.z as u32))
        .collect()
}

/// Where you arrive on the island.
pub fn arrival() -> Vec3 {
    Vec3::new(ORIGIN.x as f32 + ISLAND - 8.5, (ORIGIN.y + 1) as f32, ORIGIN.z as f32 + 0.5)
}

/// Is the portal home open (the Wyrm beaten)?
pub fn exit_open(world: &World) -> bool {
    world.get_v(ORIGIN + IVec3::Y) == HOLLOW_PORTAL
}

/// The Crypt in the region around (x, z), if any: its middle.
pub fn crypt_in_region(seed: u32, rx: i32, rz: i32) -> Option<IVec3> {
    if rx == 0 && rz == 0 {
        // Not right under spawn: one region out, at least.
        return None;
    }
    let x = rx * CRYPT_REGION + 64 + (hash2(seed ^ 0xC417, rx, rz) * (CRYPT_REGION - 128) as f32) as i32;
    let z = rz * CRYPT_REGION + 64 + (hash2(seed ^ 0xC418, rx, rz) * (CRYPT_REGION - 128) as f32) as i32;
    // Only in the ordinary world.
    // (Within where the old shared map's ordinary world was, so old worlds keep theirs.)
    (x > HOLLOW_X + 2048 + 64 && x < crate::scorch::SCORCH_X - 2048 - 64).then(|| ivec3(x, CRYPT_Y, z))
}

/// The nearest Crypt to `p` (looking a few regions around).
pub fn nearest_crypt(seed: u32, p: Vec3) -> Option<IVec3> {
    let (rx, rz) = ((p.x as i32).div_euclid(CRYPT_REGION), (p.z as i32).div_euclid(CRYPT_REGION));
    let mut best: Option<IVec3> = None;
    for dz in -2..=2 {
        for dx in -2..=2 {
            if let Some(c) = crypt_in_region(seed, rx + dx, rz + dz) {
                let d = |q: IVec3| (q.x as f32 - p.x).hypot(q.z as f32 - p.z);
                if best.is_none_or(|b| d(c) < d(b)) {
                    best = Some(c);
                }
            }
        }
    }
    best
}

/// The twelve Eye Frames around a portal's 3x3 middle at `c`.
pub fn ring(c: IVec3) -> Vec<IVec3> {
    let mut v = Vec::new();
    for k in -1..=1 {
        v.extend([c + ivec3(k, 0, -2), c + ivec3(k, 0, 2), c + ivec3(-2, 0, k), c + ivec3(2, 0, k)]);
    }
    v
}

/// A Crypt's blocks: a stone brick room with the ring of frames around a pit.
pub fn crypt_blocks(c: IVec3, seed: u32) -> Vec<(IVec3, Id)> {
    let mut v = Vec::new();
    let (r, h): (i32, i32) = (6, 6);
    for dy in -1..=h {
        for dz in -r..=r {
            for dx in -r..=r {
                let wall = dx.abs() == r || dz.abs() == r || dy == -1 || dy == h;
                let p = c + ivec3(dx, dy, dz);
                let id = if wall {
                    if hash3(seed ^ 0xC419, p.x, p.y, p.z) < 0.2 { MOSSY_COBBLE } else { STONE_BRICKS }
                } else {
                    AIR
                };
                v.push((p, id));
            }
        }
    }
    // The pit (lava at the bottom, like it means it), framed.
    for dz in -1..=1 {
        for dx in -1..=1 {
            v.push((c + ivec3(dx, -1, dz), LAVA));
            v.push((c + ivec3(dx, -2, dz), STONE_BRICKS));
        }
    }
    for p in ring(c) {
        // A few frames come with their eye already in.
        let full = hash3(seed ^ 0xC41A, p.x, p.y, p.z) < 0.1;
        v.push((p, if full { EYE_FRAME_FULL } else { EYE_FRAME }));
    }
    // Torches on the walls, and a way up (a ladder shaft to the surface is up to you).
    for (dx, dz) in [(-5, -5), (5, -5), (-5, 5), (5, 5)] {
        v.push((c + ivec3(dx, 0, dz), TORCH));
    }
    v
}

impl Generator {
    /// The Hollow's chunks: void, the island, its pillars and crystals.
    pub fn generate_hollow(&self, cx: i32, cz: i32) -> Vec<Id> {
        let mut b = vec![AIR; (CW * CW * CH) as usize];
        let s = self.seed;
        let pillars = pillars(s);
        for lz in 0..CW {
            for lx in 0..CW {
                let (x, z) = (cx * CW + lx, cz * CW + lz);
                let i = |y: i32| crate::world::idx(lx, y, lz);
                let d = ((x - GEN_ORIGIN.x) as f32).hypot((z - GEN_ORIGIN.z) as f32);
                let edge = ISLAND + self.scorch.noise3(x as f32 / 20.0, 3.0, z as f32 / 20.0) * 8.0;
                if d < edge {
                    // A lens of Hollow Stone: thick in the middle, thin at the rim.
                    let depth = ((1.0 - d / edge).sqrt() * 26.0) as i32 + 2;
                    for y in (GEN_ORIGIN.y - depth).max(1)..=GEN_ORIGIN.y {
                        b[i(y)] = HOLLOW_STONE;
                    }
                }
                for &(px, pz, h) in &pillars {
                    if (x - px).pow(2) + (z - pz).pow(2) <= 9 {
                        for y in GEN_ORIGIN.y + 1..h {
                            b[i(y)] = OBSIDIAN;
                        }
                    }
                    if x == px && z == pz {
                        b[i(h)] = WYRM_CRYSTAL;
                    }
                }
                // Outer islands, some with a spire.
                for (c, r, spire) in outer_islands_near(s, x, z) {
                    let d = ((x - c.x) as f32).hypot((z - c.z) as f32);
                    let edge = r as f32 + self.scorch.noise3(x as f32 / 9.0, c.y as f32, z as f32 / 9.0) * 3.0;
                    if d < edge {
                        let depth = ((1.0 - d / edge).sqrt() * 10.0) as i32 + 1;
                        for y in (c.y - depth).max(1)..=c.y {
                            b[i(y)] = HOLLOW_STONE;
                        }
                    }
                    if spire {
                        for dy in 1..=9 {
                            if let Some(id) = spire_block(x - c.x, dy, z - c.z)
                                && id != AIR
                            {
                                b[i(c.y + dy)] = id;
                            }
                        }
                    }
                }
                // A little obsidian landing where you arrive.
                let a = arrival() + Vec3::new(GEN_ORIGIN.x as f32, 0.0, 0.0);
                if (x as f32 - a.x).abs() < 2.5 && (z as f32 - a.z).abs() < 2.5 {
                    b[i(GEN_ORIGIN.y)] = OBSIDIAN;
                }
            }
        }
        b
    }
}

impl Generator {
    /// Crypts whose rooms reach into chunk (cx, cz).
    pub fn place_crypts(&self, cx: i32, cz: i32, b: &mut [Id]) {
        let (rx, rz) = ((cx * CW).div_euclid(CRYPT_REGION), (cz * CW).div_euclid(CRYPT_REGION));
        for dz in -1..=1 {
            for dx in -1..=1 {
                let Some(c) = crypt_in_region(self.seed, rx + dx, rz + dz) else { continue };
                if (c.x - (cx * CW + 8)).abs() > 16 || (c.z - (cz * CW + 8)).abs() > 16 {
                    continue;
                }
                for (p, id) in crypt_blocks(c, self.seed) {
                    let (lx, lz) = (p.x - cx * CW, p.z - cz * CW);
                    if (0..CW).contains(&lx) && (0..CW).contains(&lz) && (1..CH).contains(&p.y) {
                        b[crate::world::idx(lx, p.y, lz)] = id;
                    }
                }
            }
        }
    }
}

impl Game {
    pub fn in_hollow(&self) -> bool {
        !self.menu && self.world.is_hollow()
    }

    /// Right-click with a Staring Eye: into a frame, or thrown to point the way.
    pub fn use_eye(&mut self) -> bool {
        if let Some(crate::game::Target::Block(h)) = &self.target {
            let pos = h.pos;
            if self.world.get_v(pos) == EYE_FRAME {
                self.world.set_v(pos, EYE_FRAME_FULL);
                self.sfx(Sfx::Warp, Some(pos.as_vec3() + Vec3::splat(0.5)));
                self.player.swing = 1.0;
                if !self.creative {
                    self.use_up_held();
                }
                if !self.is_client() {
                    self.try_open_crypt(pos);
                }
                return true;
            }
            if self.world.get_v(pos) == EYE_FRAME_FULL {
                return true;
            }
        }
        // Thrown: a trail of sparkles toward the nearest Crypt.
        let eye = self.player.eye();
        let Some(c) = nearest_crypt(self.world.seed(), eye) else {
            self.msg("The eye just... sits there. Nothing to find around here.");
            return true;
        };
        let flat = Vec3::new(c.x as f32 - eye.x, 0.0, c.z as f32 - eye.z);
        let dir = flat.normalize_or_zero();
        for k in 0..24 {
            let p = eye + dir * (k as f32 * 0.5) + Vec3::Y * (k as f32 * 0.08);
            self.smoke(p, 1, 0.05);
        }
        let far = flat.length();
        let hint = if far < 24.0 { "It dives into the ground. Dig here!".to_string() } else { format!("The eye drifts {} toward something about {} blocks away.", compass_word(dir), far as i32) };
        self.msg(hint);
        self.player.swing = 1.0;
        // Usually you get it back.
        if !self.creative && self.rng.chance(0.2) {
            self.use_up_held();
            self.msg("The eye shattered. Rude.");
        }
        self.sfx(Sfx::Warp, None);
        true
    }

    /// An eye went in at `p`: if its whole ring is full, open the portal.
    pub fn try_open_crypt(&mut self, p: IVec3) {
        for dz in -2..=2 {
            for dx in -2..=2 {
                let c = p + ivec3(dx, 0, dz);
                let frames = ring(c);
                if frames.contains(&p) && frames.iter().all(|&f| self.world.get_v(f) == EYE_FRAME_FULL) {
                    for z in -1..=1 {
                        for x in -1..=1 {
                            self.world.set_v(c + ivec3(x, 0, z), HOLLOW_PORTAL);
                        }
                    }
                    self.sfx(Sfx::Fanfare, Some(c.as_vec3()));
                    self.advance("eye_spy");
                    return;
                }
            }
        }
    }

    /// Where a Hollow portal at `at` takes you (leaving that realm active).
    pub fn hollow_destination(&mut self, at: IVec3) -> (Dim, Vec3) {
        let _ = at;
        if self.world.is_hollow() {
            // Home: the world's spawn.
            self.enter(Dim::Over);
            let s = self.spawn;
            self.world.load_now(s.x.floor() as i32 >> 4, s.z.floor() as i32 >> 4);
            return (Dim::Over, s);
        }
        self.enter(Dim::Hollow);
        let a = arrival();
        let (cx, cz) = ((a.x as i32).div_euclid(CW), (a.z as i32).div_euclid(CW));
        for dz in -1..=1 {
            for dx in -1..=1 {
                self.world.load_now(cx + dx, cz + dz);
            }
        }
        (Dim::Hollow, a)
    }

    /// Standing in a Hollow portal (the local player) takes you through at once.
    /// Returns whether it did.
    pub fn hollow_portal_tick(&mut self) -> bool {
        let b = &self.player.body;
        let feet = IVec3::new(b.pos.x.floor() as i32, (b.pos.y + 0.1).floor() as i32, b.pos.z.floor() as i32);
        if self.world.get_v(feet) != HOLLOW_PORTAL || self.portal_cooldown > 0.0 || self.dead.is_some() {
            return false;
        }
        self.portal_cooldown = 4.0;
        if self.is_client() {
            self.net_send_msg(crate::net::Msg::UsePortal { x: feet.x, y: feet.y, z: feet.z });
            return true;
        }
        let (dim, to) = self.hollow_destination(feet);
        self.move_local_player(dim, to);
        self.sfx(Sfx::Warp, None);
        if dim == Dim::Hollow {
            self.advance("hollow");
            self.msg("The Hollow. Something big is circling.");
        }
        true
    }

    /// The Hollow's resident, and its reward.
    pub fn hollow_tick(&mut self, dt: f32) {
        if self.is_client() {
            return;
        }
        // (Players here: the local one, or any joined ones in this realm.)
        let anyone = self.in_hollow() && (!self.away() || !self.peers.is_empty());
        if !anyone {
            return;
        }
        // The Wyrm appears when someone's there and it hasn't been beaten.
        let wyrm = self.mobs.iter().any(|m| m.kind == MobKind::Wyrm);
        if !wyrm && self.world.is_loaded(ORIGIN.x, ORIGIN.z) && !exit_open(&self.world) {
            self.alloc_mob(MobKind::Wyrm, ORIGIN.as_vec3() + Vec3::new(0.0, 30.0, 0.0));
            if let Some(m) = self.mobs.last_mut() {
                m.persistent = true;
            }
            self.msg("A roar echoes across the void.");
        }
        // Crystals heal it.
        let crystals: Vec<Vec3> = pillars(self.world.seed())
            .into_iter()
            .map(|(x, z, h)| ivec3(x - GEN_ORIGIN.x, h, z))
            .filter(|p| self.world.get_v(*p) == WYRM_CRYSTAL)
            .map(|p| p.as_vec3() + Vec3::splat(0.5))
            .collect();
        for m in self.mobs.iter_mut().filter(|m| m.kind == MobKind::Wyrm) {
            if crystals.iter().any(|c| c.distance(m.body.pos) < 70.0) {
                m.health = (m.health + dt * 2.0).min(m.kind.max_health());
            }
        }
        // Void: fall off and it's over.
        if !self.away() && self.in_hollow() && self.player.body.pos.y < -8.0 && self.dead.is_none() {
            self.hurt_player(4.0 * dt * 10.0, "fell into the void. It's not a pool either");
        }
    }

    /// The Wyrm is beaten: the portal home, the egg, and a lot of experience.
    pub fn wyrm_defeated(&mut self, at: Vec3) {
        for z in -1..=1 {
            for x in -1..=1 {
                self.world.set_v(ORIGIN + ivec3(x, 0, z), OBSIDIAN);
                self.world.set_v(ORIGIN + ivec3(x, 1, z), HOLLOW_PORTAL);
            }
        }
        self.world.set_v(ORIGIN + ivec3(0, 2, 3), WYRM_EGG);
        self.spawn_orbs(at, 500);
        self.sfx(Sfx::Fanfare, Some(at));
        self.msg("The Hollow Wyrm is beaten! A portal home opens in the middle of the island.");
        self.advance("wyrm_slayer");
    }

    /// A Wyrm Crystal was broken: it blows up.
    pub fn crystal_broken(&mut self, p: IVec3) {
        if !self.is_client() {
            self.explode(p.as_vec3() + Vec3::splat(0.5), 3.0, "was too close to a Wyrm Crystal");
        }
    }
}

/// Words for a direction.
fn compass_word(d: Vec3) -> &'static str {
    let a = d.x.atan2(-d.z).to_degrees().rem_euclid(360.0);
    ["north", "north-east", "east", "south-east", "south", "south-west", "west", "north-west"][((a + 22.5) / 45.0) as usize % 8]
}

/// The Wyrm flies: circling the island, now and then swooping at the nearest target.
pub fn wyrm_update(m: &mut Mob, dt: f32, target: Vec3, has_target: bool, ev: &mut Vec<MobEvent>) {
    m.attack_cd = (m.attack_cd - dt).max(0.0);
    m.warp_cd -= dt;
    let center = ORIGIN.as_vec3() + Vec3::Y * 28.0;
    let swooping = m.fuse > 0.0;
    let goal = if swooping && has_target {
        target + Vec3::Y
    } else {
        // Round and round.
        let a = m.anim * 0.05;
        center + Vec3::new(a.cos() * 40.0, (a * 3.0).sin() * 6.0, a.sin() * 40.0)
    };
    let to = goal - m.body.pos;
    let speed = if swooping { 16.0 } else { 11.0 };
    let want = to.normalize_or_zero() * speed;
    let k = (dt * 1.5).min(1.0);
    m.body.vel += (want - m.body.vel) * k;
    m.body.pos += m.body.vel * dt;
    m.anim += dt * 20.0;
    if m.body.vel.length() > 0.1 {
        m.yaw = m.body.vel.x.atan2(-m.body.vel.z);
    }
    // Decide to swoop, or give up.
    if !swooping && has_target && m.warp_cd <= 0.0 {
        m.fuse = 6.0;
        m.warp_cd = 10.0;
    }
    if swooping {
        m.fuse -= dt;
        if has_target && m.attack_cd <= 0.0 && m.body.pos.distance(target + Vec3::Y) < WYRM_REACH {
            ev.push(MobEvent::HurtPlayer(WYRM_DAMAGE, "was flattened by the Hollow Wyrm"));
            m.attack_cd = 1.5;
            m.fuse = 0.0; // back up for another pass
        }
        if m.fuse <= 0.0 {
            m.fuse = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hollow_is_an_island_with_pillars() {
        let g = Generator::with_dim(5, crate::world::GenOptions::LEGACY, crate::dims::Dim::Hollow);
        let (cx, cz) = (ORIGIN.x.div_euclid(CW), ORIGIN.z.div_euclid(CW));
        let b = g.generate(cx, cz);
        let (lx, lz) = (ORIGIN.x.rem_euclid(CW), ORIGIN.z.rem_euclid(CW));
        assert_eq!(b[crate::world::idx(lx, ORIGIN.y, lz)], HOLLOW_STONE);
        assert_eq!(b[crate::world::idx(lx, ORIGIN.y + 5, lz)], AIR);
        // Far out: nothing at all.
        let far = g.generate(cx + 12, cz + 12);
        assert!(far.iter().all(|&id| id == AIR));
        for (x, z, h) in pillars(5) {
            let x = x - GEN_ORIGIN.x; // (generator coordinates to the Hollow's own)
            let c = g.generate(x.div_euclid(CW), z.div_euclid(CW));
            assert_eq!(c[crate::world::idx(x.rem_euclid(CW), h, z.rem_euclid(CW))], WYRM_CRYSTAL);
        }
    }

    #[test]
    fn eyes_find_crypts_and_a_full_ring_opens() {
        let mut g = crate::game::tests::arena(101);
        let c = nearest_crypt(g.world.seed(), Vec3::ZERO).expect("a crypt nearby");
        assert!(c.x.abs() > 64 || c.z.abs() > 64);
        // Build a ring in the arena and fill it.
        let mid = ivec3(0, 50, -6);
        for p in ring(mid) {
            g.world.set_v(p, EYE_FRAME);
        }
        let frames = ring(mid);
        for (i, &p) in frames.iter().enumerate() {
            g.world.set_v(p, EYE_FRAME_FULL);
            g.try_open_crypt(p);
            let open = g.world.get_v(mid) == HOLLOW_PORTAL;
            assert_eq!(open, i == frames.len() - 1);
        }
    }

    #[test]
    fn the_wyrm_swoops() {
        let mut rng = crate::noise::Rng::new(1);
        let mut m = Mob::new(MobKind::Wyrm, ORIGIN.as_vec3() + Vec3::Y * 20.0, &mut rng);
        let target = ORIGIN.as_vec3() + Vec3::new(10.0, 1.0, 0.0);
        let mut hurt = false;
        for _ in 0..1200 {
            let mut ev = Vec::new();
            wyrm_update(&mut m, 1.0 / 30.0, target, true, &mut ev);
            hurt |= ev.iter().any(|e| matches!(e, MobEvent::HurtPlayer(..)));
        }
        assert!(hurt, "never hit");
    }
}
