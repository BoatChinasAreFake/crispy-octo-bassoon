//! Hollow Cities: towers of Mauve stone on some of the Hollow's outer islands
//! (worlds from generator 9).
//!
//! - A city has a tall main tower and one or two smaller ones: square, with
//!   Mauve Pillars at the corners, floors every few blocks joined by ladders,
//!   windows, and a Glowrock lamp on each corner of the roof. Its treasure
//!   chest is on the main tower's top floor.
//! - Off the island's edge hangs a **flying ship**: a Mauve hull with a mast
//!   and a white sail, a lamp on the prow, and in the chest at its stern a
//!   **Glider** (always).
//! - **Sulkers** (see sulkers.rs) keep turning up on the towers' floors and
//!   the ship's deck while anyone is near, like Guardians round a monument.
//!
//! A city's island has no spire. Everything here is in the Hollow
//! generator's own coordinates (see dims.rs).

use crate::block::*;
use crate::dims::Dim;
use crate::entity::MobKind;
use crate::game::Game;
use crate::noise::hash2;
use crate::structures::Kind;
use crate::world::Generator;
use macroquad::math::{ivec3, IVec3, Vec3};

/// Floors this far apart.
const STOREY: i32 = 6;
/// Sulkers about a city at once, and how near someone must be for more to turn up.
pub const SULKERS: usize = 6;
pub const NEAR: f32 = 64.0;

/// Is the outer island at `c` (radius `r`) a city's?
pub fn is_city(seed: u32, c: IVec3, r: i32) -> bool {
    r >= 11 && hash2(seed ^ 0x0C17, c.x, c.z) < 0.4
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tower {
    /// The middle of its ground floor.
    pub base: IVec3,
    /// Half its width (the walls are at ±half).
    pub half: i32,
    pub height: i32,
}

impl Tower {
    /// Heights of its floors above the base (the ground one first).
    pub fn floors(&self) -> impl Iterator<Item = i32> {
        (0..self.height).step_by(STOREY as usize)
    }
    pub fn top_floor(&self) -> i32 {
        (self.height - 1) / STOREY * STOREY
    }
}

/// The flying ship: the middle of its deck, and which way it points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ship {
    pub deck: IVec3,
    /// Along x (or else along z).
    pub along_x: bool,
}

impl Ship {
    /// A point on the ship, `along` its length and `across` it, `up` from the deck.
    pub fn at(&self, along: i32, up: i32, across: i32) -> IVec3 {
        if self.along_x { self.deck + ivec3(along, up, across) } else { self.deck + ivec3(across, up, along) }
    }
    pub fn chest(&self) -> IVec3 {
        self.at(-5, 1, 0)
    }
}

/// Half the ship's width at a point along it (it narrows to the bow and the stern).
fn beam(along: i32) -> i32 {
    3 - (along.abs() - 4).max(0)
}

impl Generator {
    /// A city's towers (the main one first).
    pub fn city_towers(&self, c: IVec3, r: i32) -> Vec<Tower> {
        let s = self.seed ^ 0x0C18;
        let mut v = vec![Tower { base: c + IVec3::Y, half: 3, height: 22 + (hash2(s, c.x, c.z) * 10.0) as i32 }];
        let more = 1 + (hash2(s ^ 1, c.x, c.z) < 0.5) as i32;
        let turn = hash2(s ^ 2, c.x, c.z) * std::f32::consts::TAU;
        for k in 0..more {
            let a = turn + k as f32 * 2.4;
            let d = r as f32 * 0.55;
            let (x, z) = (c.x + (a.cos() * d) as i32, c.z + (a.sin() * d) as i32);
            let top = self.outer_top(x, z, c, r).unwrap_or(c.y);
            v.push(Tower { base: ivec3(x, top + 1, z), half: 2, height: 12 + (hash2(s ^ 3, x, z) * 7.0) as i32 });
        }
        v
    }

    /// A city's flying ship, off the island's edge and well up.
    pub fn city_ship(&self, c: IVec3, r: i32) -> Ship {
        let a = hash2(self.seed ^ 0x0C19, c.x, c.z) * std::f32::consts::TAU;
        let d = r as f32 + 14.0;
        let deck = ivec3(c.x + (a.cos() * d) as i32, c.y + 20, c.z + (a.sin() * d) as i32);
        // Broadside on to the island.
        Ship { deck, along_x: a.sin().abs() > a.cos().abs() }
    }

    /// Every block of a city: its towers and its ship.
    pub fn city_blocks(&self, c: IVec3, r: i32) -> Vec<(IVec3, Id)> {
        let mut v = Vec::new();
        for t in self.city_towers(c, r) {
            tower_blocks(&t, &mut v);
        }
        ship_blocks(&self.city_ship(c, r), &mut v);
        v
    }

    /// A city's chests: the main tower's treasure and the ship's (with the Glider).
    pub fn city_chests(&self, c: IVec3, r: i32) -> Vec<(IVec3, Kind)> {
        let main = self.city_towers(c, r)[0];
        vec![(treasure_chest(&main), Kind::HollowCity), (self.city_ship(c, r).chest(), Kind::SkyShip)]
    }

    /// Where Sulkers sit: in the corners of the towers' floors, and on the ship's deck.
    pub fn sulker_spots(&self, c: IVec3, r: i32) -> Vec<IVec3> {
        let mut v = Vec::new();
        for t in self.city_towers(c, r) {
            let k = t.half - 1;
            for y in t.floors() {
                v.extend([ivec3(-k, y + 1, -k), ivec3(k, y + 1, -k)].map(|d| t.base + d));
            }
            v.push(t.base + ivec3(0, t.height + 1, 0));
        }
        let ship = self.city_ship(c, r);
        v.extend([ship.at(3, 1, 1), ship.at(-2, 1, -1)]);
        v
    }

    /// The cities on outer islands that might reach (x, z).
    pub fn cities_near(&self, x: i32, z: i32) -> Vec<(IVec3, i32)> {
        if self.opts.version < 9 {
            return Vec::new();
        }
        crate::hollow::outer_islands_near(self.seed, x, z).into_iter().filter(|&(c, r, _)| is_city(self.seed, c, r)).map(|(c, r, _)| (c, r)).collect()
    }
}

fn treasure_chest(t: &Tower) -> IVec3 {
    t.base + ivec3(0, t.top_floor() + 1, -(t.half - 1))
}

fn tower_blocks(t: &Tower, v: &mut Vec<(IVec3, Id)>) {
    let h = t.half;
    let mut put = |dx: i32, dy: i32, dz: i32, id: Id| v.push((t.base + ivec3(dx, dy, dz), id));
    // Footings, so it doesn't hang off an uneven top.
    for dy in -4..0 {
        for dz in -h..=h {
            for dx in -h..=h {
                put(dx, dy, dz, MAUVE_BLOCK);
            }
        }
    }
    for dy in 0..=t.height {
        let floor = dy % STOREY == 0 || dy == t.height;
        for dz in -h..=h {
            for dx in -h..=h {
                let (edge_x, edge_z) = (dx.abs() == h, dz.abs() == h);
                let id = if edge_x && edge_z {
                    MAUVE_PILLAR
                } else if edge_x || edge_z {
                    let mid = if edge_x { dz } else { dx };
                    let window = mid == 0 && dy % STOREY >= 3 && dy % STOREY <= 4 && dy < t.height;
                    let door = dz == -h && dx == 0 && (1..=2).contains(&dy);
                    if door {
                        AIR
                    } else if window {
                        GLASS
                    } else {
                        MAUVE_BLOCK
                    }
                } else if dx == 0 && dz == h - 1 && dy < t.height {
                    // The way up: a ladder against the back wall, through every floor.
                    if dy == 0 { MAUVE_BLOCK } else { LADDER_FIRST + 2 }
                } else if floor {
                    MAUVE_BLOCK
                } else {
                    AIR
                };
                put(dx, dy, dz, id);
            }
        }
    }
    // A battlement round the roof, and lamps on its corners.
    for dz in -h..=h {
        for dx in -h..=h {
            if dx.abs() == h || dz.abs() == h {
                let corner = dx.abs() == h && dz.abs() == h;
                if corner {
                    put(dx, t.height + 1, dz, MAUVE_PILLAR);
                    put(dx, t.height + 2, dz, GLOWROCK);
                } else if (dx + dz) % 2 == 0 {
                    put(dx, t.height + 1, dz, MAUVE_BLOCK);
                }
            }
        }
    }
    put(0, t.top_floor() + 1, -(h - 1), CHEST);
}

fn ship_blocks(s: &Ship, v: &mut Vec<(IVec3, Id)>) {
    let mut put = |along: i32, up: i32, across: i32, id: Id| v.push((s.at(along, up, across), id));
    for along in -7..=7 {
        let w = beam(along);
        // The hull: narrower at the keel.
        for across in -w..=w {
            put(along, 0, across, MAUVE_BLOCK);
        }
        for across in -(w - 1)..=(w - 1) {
            put(along, -1, across, MAUVE_BLOCK);
        }
        for across in -(w - 2)..=(w - 2) {
            put(along, -2, across, MAUVE_PILLAR);
        }
        // The rails.
        for across in [-w, w] {
            put(along, 1, across, MAUVE_BLOCK);
        }
    }
    // The mast and its sail.
    for up in 1..=11 {
        put(0, up, 0, MAUVE_PILLAR);
    }
    for up in 4..=10 {
        for across in -2..=2 {
            if across != 0 {
                put(1, up, across, WOOL);
            }
        }
        put(1, up, 0, WOOL);
    }
    // A lamp on the bow, and the stern's chest.
    put(7, 2, 0, GLOWROCK);
    put(-5, 1, 0, CHEST);
}

impl Game {
    /// Sulkers turn up round the cities while anyone's near (where the world lives).
    pub fn city_tick(&mut self, dt: f32) {
        if self.is_client() || self.world.dim() != Dim::Hollow || !self.rules.difficulty.monsters() {
            return;
        }
        self.city_acc += dt;
        if self.city_acc < 2.0 {
            return;
        }
        self.city_acc = 0.0;
        let back = Dim::Hollow.gen_x();
        let mut people: Vec<Vec3> = self.peers.values().filter(|p| p.alive()).map(|p| p.target).collect();
        if !self.away() && self.dead.is_none() {
            people.push(self.player.body.pos);
        }
        for at in people {
            let (gx, gz) = (at.x.floor() as i32 + back, at.z.floor() as i32);
            for (c, r) in self.world.generator.cities_near(gx, gz) {
                let mid = (c - ivec3(back, 0, 0)).as_vec3();
                if mid.distance(at) > NEAR {
                    continue;
                }
                let here = self.mobs.iter().filter(|m| m.kind == MobKind::Sulker && m.body.pos.distance(mid) < NEAR).count();
                if here >= SULKERS || !self.rng.chance(0.4) {
                    continue;
                }
                let spots = self.world.generator.sulker_spots(c, r);
                let spot = spots[self.rng.int(0, spots.len() as i32 - 1) as usize] - ivec3(back, 0, 0);
                let taken = self.mobs.iter().any(|m| m.body.pos.floor().as_ivec3() == spot);
                if !taken && self.world.is_loaded(spot.x, spot.z) && self.world.get_v(spot) == AIR && is_solid(self.world.get_v(spot - IVec3::Y)) {
                    self.alloc_mob(MobKind::Sulker, spot.as_vec3() + Vec3::new(0.5, 0.0, 0.5));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{GenOptions, CW};

    fn a_city(g: &Generator) -> (IVec3, i32) {
        (1..14)
            .flat_map(|r: i32| (-r..=r).flat_map(move |i| [(i, -r), (i, r), (-r, i), (r, i)]))
            .filter_map(|(gx, gz)| crate::hollow::outer_island(g.seed, gx, gz))
            .find(|&(c, r, _)| is_city(g.seed, c, r))
            .map(|(c, r, _)| (c, r))
            .expect("a city")
    }

    fn block_at(g: &Generator, p: IVec3) -> Id {
        g.generate_hollow(p.x.div_euclid(CW), p.z.div_euclid(CW))[crate::world::idx(p.x.rem_euclid(CW), p.y, p.z.rem_euclid(CW))]
    }

    #[test]
    fn newer_hollows_have_cities_with_a_ship_and_a_glider() {
        let g = Generator::with_dim(12, GenOptions::DEFAULT, Dim::Hollow);
        let (c, r) = a_city(&g);
        let towers = g.city_towers(c, r);
        let main = towers[0];
        assert!(towers.len() >= 2);
        // The main tower: pillars at its corners, a roof lamp, and the treasure chest.
        assert_eq!(block_at(&g, main.base + ivec3(main.half, 5, main.half)), MAUVE_PILLAR);
        assert_eq!(block_at(&g, main.base + ivec3(main.half, main.height + 2, main.half)), GLOWROCK);
        let chests = g.city_chests(c, r);
        for (p, _) in &chests {
            assert_eq!(block_at(&g, *p), CHEST);
        }
        // No spire on a city's island.
        assert_ne!(block_at(&g, c + ivec3(0, 30, 0)), OBSIDIAN);
        // The ship floats off the edge, over nothing.
        let ship = g.city_ship(c, r);
        assert_eq!(block_at(&g, ship.deck), MAUVE_BLOCK);
        assert!((ship.deck.y - 30..ship.deck.y - 3).all(|y| block_at(&g, ivec3(ship.deck.x, y, ship.deck.z)) == AIR));
        // Its chest always has a Glider.
        let (p, kind) = chests[1];
        assert_eq!(kind, Kind::SkyShip);
        for seed in 0..10 {
            let loot = crate::structures::loot(kind, seed ^ p.x as u32);
            assert!(loot.slots.iter().flatten().any(|s| s.0 == GLIDER));
        }
        // And older worlds don't have any.
        let old = Generator::with_dim(12, GenOptions { version: 8, ..GenOptions::DEFAULT }, Dim::Hollow);
        assert!(old.cities_near(c.x, c.z).is_empty());
        assert_ne!(block_at(&old, ship.deck), MAUVE_BLOCK);
    }

    #[test]
    fn sulkers_turn_up_on_the_towers() {
        let mut g = Game::new_with(12, false, false, GenOptions::DEFAULT);
        g.ready = true;
        g.enter(Dim::Hollow);
        g.dim = Dim::Hollow;
        let (c, r) = a_city(&g.world.generator);
        let back = Dim::Hollow.gen_x();
        let at = (c - ivec3(back, 0, 0)).as_vec3() + Vec3::new(0.5, 2.0, -6.5);
        g.player.body.pos = at;
        let (cx, cz) = ((at.x as i32).div_euclid(CW), (at.z as i32).div_euclid(CW));
        for dz in -4..=4 {
            for dx in -4..=4 {
                g.world.load_now(cx + dx, cz + dz);
            }
        }
        for _ in 0..200 {
            g.city_tick(2.0);
        }
        let n = g.mobs.iter().filter(|m| m.kind == MobKind::Sulker).count();
        assert!(n > 0 && n <= SULKERS, "{n} Sulkers");
        let spots: Vec<IVec3> = g.world.generator.sulker_spots(c, r).into_iter().map(|p| p - ivec3(back, 0, 0)).collect();
        assert!(g.mobs.iter().filter(|m| m.kind == MobKind::Sulker).all(|m| spots.contains(&m.body.pos.floor().as_ivec3())));
    }
}
