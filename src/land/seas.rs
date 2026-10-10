//! Busier seas: kelp forests, seagrass meadows and glowing sea pickles, and
//! oceans that come warm, lukewarm, ordinary or frozen.
//!
//! - **Kelp** stands in tall columns on cool and ordinary sea floors and
//!   grows a little taller now and then, up to just under the surface.
//!   Break any of it and everything above comes away too. Dry it in a
//!   furnace for **Dried Kelp** (a crunchy snack).
//! - **Seagrass** carpets shallow floors; **sea pickles** glow in clusters
//!   on warm reefs.
//! - All three sit in water (they're waterlogged): you swim through them, and
//!   breaking one leaves water behind.
//! - **Warm oceans** are clear and full of coral; **lukewarm** ones have a
//!   little coral and plenty of seagrass; **frozen** ones are capped with ice
//!   and dotted with icebergs.

use crate::block::*;
use crate::game::Game;
use crate::noise::hash2;
use crate::world::{Biome, Generator};
use macroquad::math::{IVec3, Vec3};

/// The tallest kelp gets, and how often it grows.
pub const KELP_MAX: i32 = 16;
pub const KELP_CHANCE: f32 = 0.08;

/// What grows on this patch of sea floor, if anything: the plant, and (for
/// kelp) how tall. `depth` is how much water there is above the floor.
pub fn floor_plant(g: &Generator, biome: Biome, x: i32, z: i32, depth: i32) -> Option<(Id, i32)> {
    if depth < 2 || biome == Biome::FrozenOcean {
        return None;
    }
    let s = g.seed;
    let r = hash2(s ^ 0x5EA1, x, z);
    // Kelp forests in cool and ordinary seas, in patches.
    if matches!(biome, Biome::Ocean | Biome::LukewarmOcean) && depth >= 4 && hash2(s ^ 0x5EA2, x >> 3, z >> 3) < 0.4 && r < 0.18 {
        let tall = (2 + (hash2(s ^ 0x5EA3, x, z) * (depth as f32 - 1.0)) as i32).min(depth - 1).min(KELP_MAX);
        return Some((KELP, tall));
    }
    // Sea pickles on warm floors.
    if biome == Biome::WarmOcean && r < 0.025 {
        return Some((SEA_PICKLE, 1));
    }
    // Seagrass in shallow-ish water, thicker in warmer seas.
    let grass = match biome {
        Biome::WarmOcean | Biome::LukewarmOcean => 0.3,
        _ => 0.12,
    };
    (depth <= 14 && r < grass + 0.18 && r >= 0.18).then_some((SEAGRASS, 1))
}

impl Generator {
    /// How far an iceberg rises above a frozen sea here (0: none). Bergs sit
    /// on a grid, one per 24 blocks or so, each a lumpy mound of packed ice.
    pub fn iceberg(&self, x: i32, z: i32) -> i32 {
        const GRID: i32 = 24;
        let s = self.seed ^ 0x1CEB;
        let (gx, gz) = (x.div_euclid(GRID), z.div_euclid(GRID));
        let mut best = 0;
        for dz in -1..=1 {
            for dx in -1..=1 {
                let (cx, cz) = (gx + dx, gz + dz);
                if hash2(s, cx, cz) > 0.35 {
                    continue;
                }
                let mid = (cx * GRID + 4 + (hash2(s ^ 1, cx, cz) * 16.0) as i32, cz * GRID + 4 + (hash2(s ^ 2, cx, cz) * 16.0) as i32);
                let d = (((x - mid.0).pow(2) + (z - mid.1).pow(2)) as f32).sqrt();
                let radius = 3.0 + hash2(s ^ 3, cx, cz) * 4.0;
                let height = 3.0 + hash2(s ^ 4, cx, cz) * 9.0;
                let here = height * (1.0 - (d / radius).powi(2));
                if here >= 1.0 {
                    best = best.max(here as i32);
                }
            }
        }
        best
    }
}

impl Game {
    /// A waterlogged block was broken (where the world lives): the water
    /// stays, and broken kelp takes the kelp above it with it.
    pub fn sea_block_gone(&mut self, p: IVec3, old: Id) {
        if !waterlogged(old) {
            return;
        }
        self.refill.push(p);
        if old == KELP {
            let mut q = p + IVec3::Y;
            while self.world.get_v(q) == KELP {
                self.world.set_v(q, WATER);
                self.pop_drop(q.as_vec3() + Vec3::splat(0.5), KELP, 1);
                q += IVec3::Y;
            }
            if !self.away() && p.as_vec3().distance(self.player.body.pos) < 8.0 {
                self.advance("kelp_me");
            }
        }
    }

    /// Water back into broken waterlogged blocks (a tick later, once they're gone).
    pub fn refill_tick(&mut self) {
        for p in std::mem::take(&mut self.refill) {
            // (Running water may have got there first.)
            let here = self.world.get_v(p);
            if here == AIR || (is_water(here) && here != WATER) {
                self.world.set_v(p, WATER);
            }
        }
    }

    /// Kelp grows: a random tick on the top of a column.
    pub fn kelp_grows(&mut self, p: IVec3) {
        let up = p + IVec3::Y;
        // (Only into water, and always leaving a block of it on top.)
        if self.world.get_v(up) != WATER || !is_water(self.world.get_v(up + IVec3::Y)) || !self.rng.chance(KELP_CHANCE) {
            return;
        }
        let mut height = 1;
        while height < KELP_MAX && self.world.get_v(p - IVec3::Y * height) == KELP {
            height += 1;
        }
        if height < KELP_MAX {
            self.world.set_v(up, KELP);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use macroquad::math::ivec3;

    /// A pool, 8 deep, over the arena's floor.
    fn pool(g: &mut Game) {
        for y in 50..58 {
            for z in -3..=3 {
                for x in -3..=3 {
                    g.world.set_v(ivec3(x, y, z), WATER);
                }
            }
        }
    }

    #[test]
    fn kelp_grows_and_comes_away_whole_leaving_water() {
        let mut g = crate::game::tests::arena(701);
        pool(&mut g);
        for y in 50..53 {
            g.world.set_v(ivec3(0, y, 0), KELP);
        }
        for _ in 0..400 {
            g.kelp_grows(ivec3(0, (50..58).rev().find(|&y| g.world.get(0, y, 0) == KELP).unwrap(), 0));
        }
        let top = (50..58).rev().find(|&y| g.world.get(0, y, 0) == KELP).unwrap();
        assert_eq!(top, 56, "it grows to just under the surface");
        // Break the bottom: it all comes away, the water stays, and each piece drops.
        g.break_block(ivec3(0, 50, 0), true);
        g.refill_tick();
        assert!((50..58).all(|y| g.world.get(0, y, 0) == WATER), "water where the kelp was");
        assert_eq!(g.drops.iter().filter(|d| d.item == KELP).map(|d| d.n as u32).sum::<u32>(), 7);
        // Dried in a furnace, it's a snack.
        assert_eq!(crate::containers::smelt(KELP), Some(DRIED_KELP));
        assert!(food_value(DRIED_KELP).is_some());
    }

    #[test]
    fn you_swim_through_sea_plants() {
        let mut g = crate::game::tests::arena(702);
        pool(&mut g);
        g.world.set_v(ivec3(1, 50, 1), SEAGRASS);
        g.world.set_v(ivec3(1, 51, 1), SEAGRASS);
        g.player.body.pos = Vec3::new(1.5, 50.1, 1.5);
        crate::entity::move_body(&g.world, &mut g.player.body, 0.05, false);
        assert!(g.player.body.in_water, "seagrass is wet");
        assert!(is_wet(SEA_PICKLE) && !is_wet(STONE));
    }

    #[test]
    fn running_water_leaves_sea_plants_be_and_they_feed_it() {
        let p = ivec3(0, 50, 0);
        // A seagrass under water, with air beside it: it stays put (it isn't washed out).
        let get = |q: IVec3| if q == p { SEAGRASS } else if q.y > p.y { WATER } else if q.y < p.y { STONE } else { AIR };
        assert_eq!(crate::liquids::settle(&get, p), None);
        // A gap between two kelp on the floor fills with a new source, as between two waters.
        let get = |q: IVec3| if q.y < p.y { STONE } else if q == p { AIR } else if q.y == p.y && (q.x - p.x).abs() == 1 && q.z == p.z { KELP } else { AIR };
        assert_eq!(crate::liquids::settle(&get, p), Some(WATER));
    }

    #[test]
    fn seas_come_warm_lukewarm_ordinary_and_frozen_with_their_own_floors() {
        let g = Generator::new(2024);
        let mut seen = std::collections::HashSet::new();
        let mut plants = std::collections::HashSet::new();
        for z in (-4000..4000).step_by(32) {
            for x in (-4000..4000).step_by(32) {
                let (h, b) = g.column(x, z);
                if b.is_ocean() {
                    seen.insert(format!("{b:?}"));
                    if let Some((p, _)) = floor_plant(&g, b, x, z, g.sea() - h) {
                        plants.insert(p);
                    }
                }
            }
        }
        for b in ["Ocean", "WarmOcean", "LukewarmOcean", "FrozenOcean"] {
            assert!(seen.contains(b), "no {b}");
        }
        assert!(plants.contains(&KELP) && plants.contains(&SEAGRASS) && plants.contains(&SEA_PICKLE));
    }
}
