//! Woodland Mansions: great dark oak houses deep in the dark forests, where
//! the Pilferers and their Invoicers live.
//!
//! - Three storeys of dark oak on a cobblestone footing, with log pillars,
//!   rows of windows and a stepped roof. The door's in the middle of the
//!   south side.
//! - Inside, each floor is a cross of corridors with eight rooms off it: a
//!   library, a storeroom with chests, a bedroom, a dining room, a mess for
//!   the Pilferers. Ladders up the east and west ends go between floors.
//! - **Secret rooms**: two rooms upstairs have no door at all. Look for the
//!   gap in the rooms along a corridor, and dig in: a chest of the good
//!   stuff (diamonds, enchanted books, a Totem-ish golden thing) and a block
//!   of gold or two.
//! - **Residents**: Pilferers on every floor and an Invoicer or three, who
//!   move in the first time someone comes near in a session (like a Snout
//!   Bastion's).
//! - **Finding one**: they only grow in dark forests, and they're rare.
//!   Cartographers sell a **Woodland Explorer Map** marked with the nearest.
//!
//! (New worlds only: older ones' dark forests stay as they were.)

use crate::block::*;
use crate::entity::MobKind;
use crate::game::Game;
use crate::noise::{hash2, hash3};
use crate::structures::{Kind, Site};
use crate::world::{Biome, Generator, CW};
use macroquad::math::{ivec3, IVec3, Vec3};

/// Half the mansion's width (x) and depth (z), and a storey's height.
pub const W: i32 = 15;
pub const D: i32 = 11;
pub const STOREY: i32 = 6;
pub const FLOORS: i32 = 3;
/// Mansions start on a grid of their own: one chunk in each GRID x GRID.
pub const GRID: i32 = 8;
/// How far (grid cells) a Woodland Explorer Map looks for one.
pub const MAP_CELLS: i32 = 20;
/// How close someone must come for the residents to move in.
const NEAR: f32 = 56.0;

/// What a room is for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Room {
    Library,
    Storeroom,
    Bedroom,
    Dining,
    Mess,
    /// No door: dig in.
    Secret,
}

/// One room: its floor, its inside (x0..=x1, z0..=z1, relative to the
/// origin), and what it is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RoomAt {
    pub floor: i32,
    pub x: (i32, i32),
    pub z: (i32, i32),
    pub kind: Room,
}

/// The rooms' insides along x (west to east) and z (north, south).
const XS: [(i32, i32); 4] = [(-14, -9), (-7, -3), (3, 7), (9, 14)];
const ZS: [(i32, i32); 2] = [(-10, -3), (3, 10)];

/// The first block of the floor (y, relative to the origin) of storey `k`.
pub fn floor_y(k: i32) -> i32 {
    k * STOREY
}

/// Every room in a mansion, floor by floor.
pub fn rooms(site: &Site) -> Vec<RoomAt> {
    let s = site.seed;
    // Two secret rooms upstairs (never the same one).
    let a = (hash2(s ^ 0x5EC, 1, 0) * 16.0) as i32 % 16;
    let b = (a + 1 + (hash2(s ^ 0x5EC, 2, 0) * 15.0) as i32 % 15) % 16;
    let mut v = Vec::new();
    for floor in 0..FLOORS {
        for (zi, &z) in ZS.iter().enumerate() {
            for (xi, &x) in XS.iter().enumerate() {
                let n = (floor - 1) * 8 + zi as i32 * 4 + xi as i32;
                let kind = if floor >= 1 && (n == a || n == b) {
                    Room::Secret
                } else {
                    match (hash3(s ^ 0x200, floor, zi as i32, xi as i32) * 5.0) as i32 {
                        0 => Room::Library,
                        1 => Room::Storeroom,
                        2 => Room::Bedroom,
                        3 => Room::Dining,
                        _ => Room::Mess,
                    }
                };
                v.push(RoomAt { floor, x, z, kind });
            }
        }
    }
    v
}

/// The chests in the secret rooms.
pub fn secret_chests(site: &Site) -> Vec<IVec3> {
    rooms(site).into_iter().filter(|r| r.kind == Room::Secret).map(|r| site.origin + ivec3((r.x.0 + r.x.1) / 2, floor_y(r.floor), r.z.0)).collect()
}

/// Every block of a mansion (in world coordinates; the door faces south).
pub fn mansion_blocks(site: &Site) -> Vec<(IVec3, Id)> {
    let o = site.origin;
    let s = site.seed;
    let planks = crate::woods::id(2, crate::woods::part::PLANKS);
    let stairs = crate::woods::id(2, crate::woods::part::STAIRS);
    let mut out: Vec<(IVec3, Id)> = Vec::new();
    let mut put = |x: i32, y: i32, z: i32, id: Id| out.push((o + ivec3(x, y, z), id));
    let top = floor_y(FLOORS);
    // Clear the plot (the forest grew right up to it).
    for z in -D - 2..=D + 2 {
        for x in -W - 2..=W + 2 {
            for y in 0..top + D + 4 {
                put(x, y, z, AIR);
            }
        }
    }
    // The footing, and a cobbled path out of the door.
    for z in -D..=D {
        for x in -W..=W {
            let edge = x.abs() == W || z.abs() == D;
            put(x, -1, z, if edge { if hash3(s, x, -1, z) < 0.3 { MOSSY_COBBLE } else { COBBLE } } else { planks });
        }
    }
    for z in D + 1..=D + 2 {
        for x in -1..=1 {
            put(x, -1, z, COBBLE);
        }
    }
    // Walls: log pillars every five blocks and at the corners, log beams
    // along each floor, planks between, windows on every storey.
    let pillar = |x: i32, z: i32| (x.abs() == W && (z.abs() == D || z % 5 == 0)) || (z.abs() == D && (x.abs() == W || x % 5 == 0));
    for y in 0..top {
        let beam = y % STOREY == STOREY - 1;
        let wy = y % STOREY;
        for z in -D..=D {
            for x in -W..=W {
                if x.abs() != W && z.abs() != D {
                    continue;
                }
                let id = if pillar(x, z) || beam {
                    DARK_OAK_LOG
                } else if (1..=2).contains(&wy) && ((z.abs() == D && (2..=3).contains(&x.rem_euclid(5))) || (x.abs() == W && (2..=3).contains(&z.rem_euclid(5)))) {
                    // A window (a pane joined along the wall).
                    PANE_FIRST + if z.abs() == D { 3 } else { 12 }
                } else {
                    planks
                };
                put(x, y, z, id);
            }
        }
    }
    // The front door: a wide opening in the middle of the south wall.
    for x in -1..=1 {
        for y in 0..=2 {
            put(x, y, D, AIR);
        }
    }
    // Floors and ceilings between storeys, and the top storey's ceiling.
    for k in 1..=FLOORS {
        for z in -D + 1..D {
            for x in -W + 1..W {
                put(x, floor_y(k) - 1, z, planks);
            }
        }
    }
    let rooms = rooms(site);
    for k in 0..FLOORS {
        let y0 = floor_y(k);
        // Room walls: along the corridors (z = ±2), between rooms (x = ±2, ±8).
        for y in y0..y0 + STOREY - 1 {
            for x in -W + 1..W {
                for z in [-2, 2] {
                    if x.abs() > 1 {
                        put(x, y, z, planks);
                    }
                }
            }
            for x in [-8, -2, 2, 8] {
                for z in (-D + 1..=-3).chain(3..D) {
                    put(x, y, z, planks);
                }
            }
        }
        // A red runner down the ground floor's hall (upstairs, it would show
        // through the ceiling below).
        if k == 0 {
            for x in -W + 2..W - 1 {
                put(x, y0 - 1, 0, DYED_WOOL + 1);
            }
        }
        // Lanterns along the corridors.
        for x in [-11, -5, 5, 11] {
            put(x, y0, -1, LANTERN);
            put(x, y0, 1, LANTERN);
        }
        // Ladders up the east and west ends (with a hole in the floor above).
        if k + 1 < FLOORS {
            for (x, facing) in [(W - 1, 1u8), (-W + 1, 3u8)] {
                for y in y0..floor_y(k + 1) + 1 {
                    put(x, y, 0, LADDER_FIRST + facing as Id);
                }
            }
        }
        for r in rooms.iter().filter(|r| r.floor == k) {
            let (x0, x1) = r.x;
            let (z0, z1) = r.z;
            let mid = (x0 + x1) / 2;
            // A doorway onto the corridor (not into a secret room).
            let door_z = if z0 < 0 { -2 } else { 2 };
            if r.kind != Room::Secret {
                for y in y0..y0 + 2 {
                    put(mid, y, door_z, AIR);
                }
            }
            // Against the far wall, away from the door.
            let back = if z0 < 0 { z0 } else { z1 };
            match r.kind {
                Room::Library => {
                    for x in x0..=x1 {
                        for y in y0..y0 + 3 {
                            put(x, y, back, BOOKSHELF);
                        }
                    }
                    put(mid, y0, (z0 + z1) / 2, LANTERN);
                }
                Room::Storeroom => {
                    put(x0, y0, back, CHEST);
                    put(x1, y0, back, CHEST);
                    put(mid, y0, back, LANTERN);
                }
                Room::Bedroom => {
                    // A red woollen bed, and a lamp.
                    put(x0, y0, back, DYED_WOOL + 1);
                    put(x0 + 1, y0, back, DYED_WOOL + 1);
                    put(x1, y0, back, LANTERN);
                }
                Room::Dining => {
                    // A long table down the middle.
                    for x in x0 + 1..x1 {
                        put(x, y0, (z0 + z1) / 2, planks);
                    }
                    put(mid, y0 + 1, (z0 + z1) / 2, LANTERN);
                }
                Room::Mess => {
                    put(x0, y0, back, CHEST);
                    put(x1, y0, back, TABLE);
                    put(mid, y0, back, LANTERN);
                }
                Room::Secret => {
                    // The good stuff, with gold piled up behind it.
                    put(mid, y0, z0, CHEST);
                    put(x0, y0, back, GOLD_BLOCK);
                    if hash2(s ^ 0xD1A, k, x0) < 0.5 {
                        put(x1, y0, back, GOLD_BLOCK);
                    }
                    put(x1, y0, (z0 + z1) / 2, LANTERN);
                }
            }
        }
    }
    // A stepped roof of dark oak, overhanging a block.
    for i in 0.. {
        let (wx, wz) = (W + 1 - i, D + 1 - i);
        if wz < 0 {
            break;
        }
        let y = top + i;
        for z in -wz..=wz {
            for x in -wx..=wx {
                let (ex, ez) = (x.abs() == wx, z.abs() == wz);
                if !ex && !ez {
                    continue;
                }
                let id = if ex && ez || wz == 0 {
                    planks
                } else if ez {
                    stairs + if z > 0 { 0 } else { 2 }
                } else {
                    stairs + if x > 0 { 3 } else { 1 }
                };
                put(x, y, z, id);
            }
        }
    }
    out
}

/// Who lives in a mansion, and where (relative to its origin).
pub fn residents(site: &Site) -> Vec<(MobKind, IVec3)> {
    let mut v = Vec::new();
    for k in 0..FLOORS {
        let y = floor_y(k);
        for x in [-9, 0, 9] {
            v.push((MobKind::Pilferer, ivec3(x, y, 0)));
        }
    }
    // Invoicers hold court in the rooms (never the secret ones).
    for r in rooms(site).iter().filter(|r| matches!(r.kind, Room::Dining | Room::Mess)).take(3) {
        v.push((MobKind::Invoicer, ivec3((r.x.0 + r.x.1) / 2, floor_y(r.floor), (r.z.0 + r.z.1) / 2 + 1)));
    }
    v
}

impl Generator {
    /// Is chunk (cx, cz) where a mansion may start?
    pub fn mansion_spot(&self, cx: i32, cz: i32) -> bool {
        let s = self.seed ^ 0x3A_2510;
        self.opts.version >= 4 && self.opts.structures > 0 && (cx.rem_euclid(GRID), cz.rem_euclid(GRID)) == (3, 5) && hash2(s, cx.div_euclid(GRID), cz.div_euclid(GRID)) < [0.0, 0.5, 0.85, 1.0][self.opts.structures as usize % 4]
    }

    /// A mansion here, if the spot's in a dark forest on fairly level ground.
    pub fn mansion_site(&self, ox: i32, oz: i32, h: i32, biome: Biome, facing: u8, seed: u32) -> Option<Site> {
        if biome != Biome::DarkForest || h <= self.sea() + 1 || h > self.sea() + 50 {
            return None;
        }
        let level = [(-W, -D), (W, -D), (-W, D), (W, D), (0, D), (0, -D), (W, 0), (-W, 0)].iter().all(|&(dx, dz)| {
            let (hh, b) = self.column(ox + dx, oz + dz);
            (hh - h).abs() <= 6 && !b.is_ocean()
        });
        level.then_some(Site { kind: Kind::Mansion, origin: ivec3(ox, h, oz), facing, seed })
    }

    /// The nearest mansion to `from` (on the Overworld's grid), as far as a map looks.
    pub fn nearest_mansion(&self, from: Vec3) -> Option<IVec3> {
        let (gx, gz) = ((from.x as i32).div_euclid(CW).div_euclid(GRID), (from.z as i32).div_euclid(CW).div_euclid(GRID));
        for r in 0..=MAP_CELLS {
            let mut best: Option<(f32, IVec3)> = None;
            for dz in -r..=r {
                for dx in -r..=r {
                    if dx.abs().max(dz.abs()) != r {
                        continue;
                    }
                    let (cx, cz) = ((gx + dx) * GRID + 3, (gz + dz) * GRID + 5);
                    if let Some(site) = self.site(cx, cz).filter(|s| s.kind == Kind::Mansion) {
                        let d = site.origin.as_vec3().distance(from);
                        if best.is_none_or(|(b, _)| d < b) {
                            best = Some((d, site.origin));
                        }
                    }
                }
            }
            if let Some((_, p)) = best {
                return Some(p);
            }
        }
        None
    }
}

impl Game {
    /// Move the residents into any mansion someone's come near (where the world lives).
    pub fn mansions_tick(&mut self, dt: f32) {
        if self.is_client() || !self.rules.difficulty.monsters() || self.world.dim() != crate::dims::Dim::Over {
            return;
        }
        self.mansion_timer -= dt;
        if self.mansion_timer > 0.0 {
            return;
        }
        self.mansion_timer = 2.0;
        for (_, at, _) in self.player_spots() {
            let (cx, cz) = ((at.x as i32).div_euclid(CW), (at.z as i32).div_euclid(CW));
            let (gx, gz) = (cx.div_euclid(GRID), cz.div_euclid(GRID));
            for (dx, dz) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
                let Some(site) = self.world.site((gx + dx) * GRID + 3, (gz + dz) * GRID + 5).filter(|s| s.kind == Kind::Mansion) else { continue };
                let o = site.origin;
                if self.mansions_peopled.contains(&o) || o.as_vec3().distance(at) > NEAR || !self.world.is_loaded(o.x - W, o.z - D) || !self.world.is_loaded(o.x + W, o.z + D) {
                    continue;
                }
                self.mansions_peopled.insert(o);
                for (kind, p) in residents(&site) {
                    if is_solid(self.world.get_v(o + p)) {
                        continue;
                    }
                    let pos = (o + p).as_vec3() + Vec3::new(0.5, 0.0, 0.5);
                    let id = self.alloc_mob(kind, pos);
                    if let Some(m) = self.mobs.iter_mut().find(|m| m.id == id) {
                        m.home = Some(pos);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::GenOptions;

    fn site() -> Site {
        Site { kind: Kind::Mansion, origin: ivec3(100, 70, -40), facing: 2, seed: 12345 }
    }

    #[test]
    fn a_mansion_has_three_floors_secret_rooms_and_a_way_up() {
        let s = site();
        let rooms = rooms(&s);
        assert_eq!(rooms.len(), 24);
        let secret: Vec<_> = rooms.iter().filter(|r| r.kind == Room::Secret).collect();
        assert_eq!(secret.len(), 2, "two secret rooms");
        assert!(secret.iter().all(|r| r.floor >= 1), "upstairs");
        let blocks: std::collections::HashMap<IVec3, Id> = mansion_blocks(&s).into_iter().collect();
        // The secret rooms: a chest, and no way in (walls all round).
        for r in &secret {
            let c = s.origin + ivec3((r.x.0 + r.x.1) / 2, floor_y(r.floor), r.z.0);
            assert_eq!(blocks.get(&c), Some(&CHEST));
            let door_z = if r.z.0 < 0 { -2 } else { 2 };
            for x in r.x.0..=r.x.1 {
                assert!(blocks.get(&(s.origin + ivec3(x, floor_y(r.floor), door_z))).is_some_and(|&b| is_solid(b)), "a wall where a door would be");
            }
        }
        assert_eq!(secret_chests(&s).len(), 2);
        // Ordinary rooms have a doorway, and the front door is open.
        let open = rooms.iter().find(|r| r.kind != Room::Secret).unwrap();
        let dz = if open.z.0 < 0 { -2 } else { 2 };
        assert_eq!(blocks.get(&(s.origin + ivec3((open.x.0 + open.x.1) / 2, floor_y(open.floor), dz))), Some(&AIR));
        assert_eq!(blocks.get(&(s.origin + ivec3(0, 0, D))), Some(&AIR));
        // Ladders go up a floor.
        assert!(matches!(blocks.get(&(s.origin + ivec3(W - 1, floor_y(1) - 1, 0))), Some(&b) if b == LADDER_FIRST + 1));
        // A roof on top.
        assert!(blocks.get(&(s.origin + ivec3(0, floor_y(FLOORS) + D + 1, 0))).is_some_and(|&b| b != AIR));
        // Pilferers on every floor, and Invoicers.
        let who = residents(&s);
        assert!(who.iter().filter(|r| r.0 == MobKind::Pilferer).count() >= 9);
        assert!(who.iter().any(|r| r.0 == MobKind::Invoicer));
    }

    #[test]
    fn mansions_grow_in_dark_forests_and_maps_find_them() {
        let mut found = 0;
        for seed in [1u32, 2, 3, 4, 5, 6] {
            let g = Generator::with(seed, GenOptions::DEFAULT);
            if let Some(p) = g.nearest_mansion(Vec3::ZERO) {
                found += 1;
                assert_eq!(g.column(p.x, p.z).1, Biome::DarkForest);
            }
        }
        assert!(found >= 4, "mansions near the middle of {found} of 6 worlds");
        // Older worlds don't get them.
        let old = Generator::with(1, GenOptions { version: 3, ..GenOptions::DEFAULT });
        assert!(old.nearest_mansion(Vec3::ZERO).is_none());
    }
}
