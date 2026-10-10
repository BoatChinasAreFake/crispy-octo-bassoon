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

/// How tall a spire stands (worlds from version 3 have the taller, round ones).
pub fn spire_height(tall: bool) -> i32 {
    if tall { 18 } else { 9 }
}

/// A taller spire, from version 3: a round obsidian tower banded with Hollow
/// Stone, with glowing windows, buttresses at its foot and a pointed roof
/// with a glowing tip; the chest stands inside on a pedestal, as before.
pub fn tall_spire_block(dx: i32, dy: i32, dz: i32) -> Option<Id> {
    if dx.abs() > 3 || dz.abs() > 3 || !(1..=18).contains(&dy) {
        return None;
    }
    let d = ((dx * dx + dz * dz) as f32).sqrt();
    // Buttresses on the diagonals at the foot.
    if dx.abs() == dz.abs() && dx.abs() == 3 {
        return (dy <= 4).then_some(OBSIDIAN);
    }
    let r = match dy {
        1..=11 => 2.9,
        12..=14 => 2.2,
        15 => 1.6,
        16 => 1.0,
        _ => 0.5,
    };
    if dy == 18 {
        return (dx == 0 && dz == 0).then_some(GLOWROCK);
    }
    // (Nothing outside the tower: None. Inside, even air is part of it.)
    if d > r {
        return None;
    }
    let wall = d > r - 1.0 || dy >= 15;
    let axis = dx == 0 || dz == 0;
    Some(match () {
        _ if dy == 1 => HOLLOW_STONE,
        _ if dx == 0 && dz == 0 && dy == 2 => CHEST,
        // The door, facing +z.
        _ if dz > 0 && dx == 0 && (2..=4).contains(&dy) => AIR,
        _ if !wall => AIR,
        _ if axis && matches!(dy, 6 | 7 | 12) => GLOWROCK,
        _ if dy % 5 == 0 => HOLLOW_STONE,
        _ => OBSIDIAN,
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

/// How far a Crypt's rooms reach from its middle (worlds from version 3;
/// older ones have the single room, 6 out).
pub const CRYPT_REACH: i32 = 27;

/// A hollow box of stone brick (mossy here and there) from `lo` to `hi`, inclusive.
fn crypt_box(v: &mut Vec<(IVec3, Id)>, seed: u32, lo: IVec3, hi: IVec3) {
    for y in lo.y..=hi.y {
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                let p = ivec3(x, y, z);
                let wall = x == lo.x || x == hi.x || y == lo.y || y == hi.y || z == lo.z || z == hi.z;
                let id = if !wall {
                    AIR
                } else if hash3(seed ^ 0xC419, x, y, z) < 0.2 {
                    MOSSY_COBBLE
                } else {
                    STONE_BRICKS
                };
                v.push((p, id));
            }
        }
    }
}

/// A Crypt's blocks. The portal room: a ring of frames round a pit. In
/// worlds from version 3 (`big`), corridors lead off it to four side rooms:
/// a library, a store, cells and a flooded well room.
pub fn crypt_blocks(c: IVec3, seed: u32, big: bool) -> Vec<(IVec3, Id)> {
    if !big {
        return crypt_room(c, seed);
    }
    let mut v = Vec::new();
    // The corridors and rooms first, then the portal room over them, then the doorways.
    let dirs = [ivec3(0, 0, -1), ivec3(1, 0, 0), ivec3(0, 0, 1), ivec3(-1, 0, 0)];
    // Which room is down which corridor (by the seed and the spot).
    let turn = (hash2(seed ^ 0xC420, c.x, c.z) * 4.0) as usize;
    let mut doors = Vec::new();
    for (k, d) in dirs.iter().enumerate() {
        let side = ivec3(d.z.abs(), 0, d.x.abs());
        // A corridor 3 wide and 3 tall, from the room's wall out 10.
        let (a, b) = (c + *d * 6, c + *d * 17);
        let lo = a.min(b) - side * 2 + ivec3(0, -1, 0);
        let hi = a.max(b) + side * 2 + ivec3(0, 3, 0);
        crypt_box(&mut v, seed, lo, hi);
        // A room at the end, 11 across and 6 tall.
        let mid = c + *d * 22;
        crypt_box(&mut v, seed, mid - ivec3(5, 1, 5), mid + ivec3(5, 5, 5));
        // Torches along the corridor, cobwebs in its corners.
        for t in [8, 12, 16] {
            v.push((c + *d * t + side * 2 + ivec3(0, 1, 0), TORCH));
            if hash3(seed ^ 0xC421, k as i32, t, 0) < 0.5 {
                v.push((c + *d * t - side + ivec3(0, 2, 0), COBWEB));
            }
        }
        doors.push((c + *d * 6, c + *d * 17, side));
        match (k + turn) % 4 {
            0 => {
                // The library: shelves along the walls, a lectern, a chest.
                for i in -4..=4 {
                    for y in 0..=3 {
                        for q in [mid + side * 4 + *d * i, mid - side * 4 + *d * i] {
                            v.push((q + ivec3(0, y, 0), BOOKSHELF));
                        }
                    }
                }
                v.push((mid + *d * 2, LECTERN));
                v.push((mid + *d * 4, CHEST));
            }
            1 => {
                // The store: barrels and chests.
                for i in -3..=3 {
                    let q = mid + side * 4 + *d * i;
                    v.push((q, if i % 3 == 0 { CHEST } else { BARREL }));
                    v.push((mid - side * 4 + *d * i, BARREL));
                }
            }
            2 => {
                // The cells: little stone bays, webbed, one with a chest.
                for i in [-3, 0, 3] {
                    for y in 0..=3 {
                        v.push((mid + side * 2 + *d * (i + 1) + ivec3(0, y, 0), STONE_BRICKS));
                        v.push((mid - side * 2 + *d * (i + 1) + ivec3(0, y, 0), STONE_BRICKS));
                    }
                    v.push((mid + side * 4 + *d * i + ivec3(0, 2, 0), COBWEB));
                }
                v.push((mid - side * 4 + *d * 3, CHEST));
            }
            _ => {
                // The well room: a pool in the middle, lanterns round it.
                for z in -2..=2 {
                    for x in -2..=2 {
                        v.push((mid + ivec3(x, -1, z), WATER));
                        v.push((mid + ivec3(x, -2, z), STONE_BRICKS));
                    }
                }
                for q in [ivec3(4, 0, 4), ivec3(-4, 0, 4), ivec3(4, 0, -4), ivec3(-4, 0, -4)] {
                    v.push((mid + q, LANTERN));
                }
            }
        }
    }
    v.extend(crypt_room(c, seed));
    // Doorways through the walls at each end of each corridor.
    for (inner, outer, side) in doors {
        for s in -1..=1 {
            for y in 0..=2 {
                v.push((inner + side * s + ivec3(0, y, 0), AIR));
                v.push((outer + side * s + ivec3(0, y, 0), AIR));
            }
        }
    }
    // (The portal room's own torches sit in its corners, clear of the doorways.)
    v
}

/// The portal room: a stone brick room with the ring of frames around a pit.
fn crypt_room(c: IVec3, seed: u32) -> Vec<(IVec3, Id)> {
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
                let tall = self.opts.version >= 3;
                for (c, r, spire) in outer_islands_near(s, x, z) {
                    if tall {
                        self.outer_island_column(x, z, c, r, &mut b, lx, lz);
                        if spire {
                            for dy in 1..=spire_height(true) {
                                if let Some(id) = tall_spire_block(x - c.x, dy, z - c.z) {
                                    b[i(c.y + dy)] = id;
                                }
                            }
                        }
                        continue;
                    }
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
    /// One column of an outer island, from version 3: two to four lobes at
    /// slightly different heights, a craggy underside, and a few rocks
    /// drifting nearby.
    #[allow(clippy::too_many_arguments)]
    fn outer_island_column(&self, x: i32, z: i32, c: IVec3, r: i32, b: &mut [Id], lx: i32, lz: i32) {
        let s = self.seed ^ 0x0157;
        let i = |y: i32| crate::world::idx(lx, y, lz);
        let key = (c.x, c.z);
        let mut lobes = vec![(c, r as f32)];
        for k in 0..3 {
            if hash3(s, key.0, k, key.1) < 0.65 {
                let a = hash3(s ^ 1, key.0, k, key.1) * std::f32::consts::TAU;
                let off = r as f32 * (0.55 + hash3(s ^ 2, key.0, k, key.1) * 0.35);
                let rise = (hash3(s ^ 3, key.0, k, key.1) * 5.0) as i32 - 2;
                let at = c + ivec3((a.cos() * off) as i32, rise, (a.sin() * off) as i32);
                lobes.push((at, r as f32 * (0.45 + hash3(s ^ 4, key.0, k, key.1) * 0.3)));
            }
        }
        // Rocks drifting round it.
        for k in 0..3 {
            if hash3(s ^ 5, key.0, k, key.1) < 0.6 {
                let a = hash3(s ^ 6, key.0, k, key.1) * std::f32::consts::TAU;
                let off = r as f32 + 6.0 + hash3(s ^ 7, key.0, k, key.1) * 8.0;
                let up = (hash3(s ^ 8, key.0, k, key.1) * 16.0) as i32 - 6;
                lobes.push((c + ivec3((a.cos() * off) as i32, up, (a.sin() * off) as i32), 2.0 + hash3(s ^ 9, key.0, k, key.1) * 2.5));
            }
        }
        for (m, rr) in lobes {
            let d = ((x - m.x) as f32).hypot((z - m.z) as f32);
            let edge = rr + self.scorch.noise3(x as f32 / 7.0, m.y as f32, z as f32 / 7.0) * (rr * 0.3).min(3.0);
            if d >= edge {
                continue;
            }
            let k = 1.0 - d / edge;
            // A craggy underside: deeper in the middle, in uneven teeth.
            let crag = (self.scorch.noise3(x as f32 / 3.0, 40.0, z as f32 / 3.0) + 1.0) * 3.0;
            let depth = (k.sqrt() * (rr * 0.9 + 2.0) + crag * k) as i32 + 1;
            // A gently uneven top (flat under the spire, which stands on the middle).
            let bump = if d < 4.0 { 0.0 } else { self.scorch.noise3(x as f32 / 11.0, 9.0, z as f32 / 11.0) * 1.6 * k };
            let top = m.y + bump as i32;
            for y in (top - depth).max(1)..=top {
                if b[i(y)] == AIR {
                    b[i(y)] = HOLLOW_STONE;
                }
            }
        }
    }

    /// Chests in the Crypts' side rooms that fall in chunk (cx, cz) (filled like a dungeon's).
    pub fn crypt_chests(&self, cx: i32, cz: i32) -> Vec<IVec3> {
        if self.opts.version < 3 || self.dim != Dim::Over {
            return Vec::new();
        }
        let (rx, rz) = ((cx * CW).div_euclid(CRYPT_REGION), (cz * CW).div_euclid(CRYPT_REGION));
        let mut v = Vec::new();
        for dz in -1..=1 {
            for dx in -1..=1 {
                let Some(c) = crypt_in_region(self.seed, rx + dx, rz + dz) else { continue };
                if (c.x - (cx * CW + 8)).abs() > CRYPT_REACH + 8 || (c.z - (cz * CW + 8)).abs() > CRYPT_REACH + 8 {
                    continue;
                }
                v.extend(crypt_blocks(c, self.seed, true).into_iter().filter(|&(p, id)| id == CHEST && p.x.div_euclid(CW) == cx && p.z.div_euclid(CW) == cz).map(|(p, _)| p));
            }
        }
        v.sort_unstable_by_key(|p| (p.x, p.y, p.z));
        v.dedup();
        v
    }

    /// Every chest in the Crypt at `c`.
    #[cfg(test)]
    pub fn crypt_chests_all(&self, c: IVec3) -> Vec<IVec3> {
        let mut v: Vec<IVec3> = crypt_blocks(c, self.seed, self.opts.version >= 3).into_iter().filter(|&(_, id)| id == CHEST).map(|(p, _)| p).collect();
        v.sort_unstable_by_key(|p| (p.x, p.y, p.z));
        v.dedup();
        v
    }

    /// Crypts whose rooms reach into chunk (cx, cz).
    pub fn place_crypts(&self, cx: i32, cz: i32, b: &mut [Id]) {
        let (rx, rz) = ((cx * CW).div_euclid(CRYPT_REGION), (cz * CW).div_euclid(CRYPT_REGION));
        for dz in -1..=1 {
            for dx in -1..=1 {
                let Some(c) = crypt_in_region(self.seed, rx + dx, rz + dz) else { continue };
                let big = self.opts.version >= 3;
                let reach = if big { CRYPT_REACH + 8 } else { 16 };
                if (c.x - (cx * CW + 8)).abs() > reach || (c.z - (cz * CW + 8)).abs() > reach {
                    continue;
                }
                for (p, id) in crypt_blocks(c, self.seed, big) {
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
    fn newer_spires_are_tall_round_and_open() {
        use crate::world::GenOptions;
        let g = Generator::with_dim(9, GenOptions::DEFAULT, Dim::Hollow);
        let (c, _, _) = (1..12).flat_map(|r: i32| (-r..=r).flat_map(move |i| [(i, -r), (i, r), (-r, i), (r, i)])).find_map(|(gx, gz)| outer_island(9, gx, gz).filter(|i| i.2)).expect("a spire");
        let at = |p: IVec3| g.generate_hollow(p.x.div_euclid(CW), p.z.div_euclid(CW))[crate::world::idx(p.x.rem_euclid(CW), p.y, p.z.rem_euclid(CW))];
        assert_eq!(at(c + ivec3(0, 2, 0)), CHEST, "the chest where the loot goes");
        assert_eq!(at(c + ivec3(0, spire_height(true), 0)), GLOWROCK, "a glowing tip");
        // Round: the corners of its square are open, its sides are wall; the door is open.
        assert_eq!(at(c + ivec3(3, 8, 3)), AIR);
        assert_eq!(at(c + ivec3(2, 8, 0)), OBSIDIAN);
        assert!((2..=4).all(|y| at(c + ivec3(0, y, 2)) == AIR));
        assert!(spire_chests(9, (c.x).div_euclid(CW), (c.z).div_euclid(CW)).iter().any(|(p, _)| *p == c + ivec3(0, 2, 0)));
    }

    #[test]
    fn newer_crypts_have_corridors_and_side_rooms() {
        use crate::world::GenOptions;
        let g = Generator::with(77, GenOptions::DEFAULT);
        let c = nearest_crypt(77, Vec3::ZERO).expect("a crypt");
        let mut w = crate::world::World::with_options(77, GenOptions::DEFAULT);
        w.structure_loot = true;
        let reach = (CRYPT_REACH + 8) / CW + 1;
        for dz in -reach..=reach {
            for dx in -reach..=reach {
                w.load_now(c.x.div_euclid(CW) + dx, c.z.div_euclid(CW) + dz);
            }
        }
        // The ring is there, and so are the four corridors and the rooms at their ends.
        assert!(ring(c).iter().all(|&p| matches!(w.get_v(p), EYE_FRAME | EYE_FRAME_FULL)));
        for d in [ivec3(0, 0, -1), ivec3(1, 0, 0), ivec3(0, 0, 1), ivec3(-1, 0, 0)] {
            for t in [6, 10, 17, 19, 22] {
                let p = c + d * t;
                assert!(w.get_v(p) == AIR || !is_solid(w.get_v(p)), "{t} along {d}: {}", w.get_v(p));
            }
        }
        // Loot in the side rooms' chests.
        let chests = g.crypt_chests_all(c);
        assert!(chests.len() >= 3, "{} chests", chests.len());
        assert!(chests.iter().any(|p| w.containers.get(p).is_some_and(|ch| ch.slots.iter().any(|s| s.is_some()))), "the chests were filled");
        // Older worlds keep the single room.
        assert!(crypt_blocks(c, 77, false).iter().all(|(p, _)| (p.x - c.x).abs() <= 6 && (p.z - c.z).abs() <= 6));
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
