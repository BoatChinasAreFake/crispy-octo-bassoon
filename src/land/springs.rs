//! Hot springs, and the chill of a long night out.
//!
//! - **The chill.** Out in the cold at night (snowy land, taigas, the
//!   peaks, frozen seas, or just very high up), or wet in the cold, you get
//!   chilled. A fire, a warm lamp, a roof and the daytime warm you up again,
//!   slowly. Chilly (over 40): your health stops healing by itself. Freezing
//!   (over 75): you slow down and get hungry faster, and at the very worst it
//!   starts to hurt (never below half a heart). A hot meal (see cooking.rs)
//!   warms you through.
//! - **Hot springs.** Steaming pools in snowy mountains, on a bed of warm
//!   **Spring Rock**. Bathing in one quickly gets rid of the chill and slowly
//!   heals you.
//!
//! The chill is each player's own (their game keeps it, like hunger); the
//! springs are part of the land (newer worlds only, see `GenOptions`).

use crate::block::*;
use crate::game::Game;
use crate::noise::{hash2, hash3};
use crate::structures::{Kind, Site};
use crate::world::{Biome, Generator};
use macroquad::math::{ivec3, IVec3, Vec3};

pub const MAX_CHILL: f32 = 100.0;
/// No healing by yourself over this; slowed and hungrier over `FREEZING`.
pub const CHILLY: f32 = 40.0;
pub const FREEZING: f32 = 75.0;
/// Chill a second out in the cold at night (and how much more wet).
const COLD_RATE: f32 = 0.8;
/// How fast a spring, a fire and anywhere else warm you (a second).
const SPRING_WARM: f32 = 15.0;
const FIRE_WARM: f32 = 3.0;
const EASE: f32 = 0.5;

/// Is this a cold place (at height `y`)?
pub fn cold(biome: Biome, y: i32, sea: i32) -> bool {
    matches!(biome, Biome::Snowy | Biome::IceSpikes | Biome::Taiga | Biome::StonyPeaks | Biome::FrozenOcean) || y > sea + 60
}

/// How much chill builds a second: out in the cold (`outside`), at night,
/// wet, and with a fire near (`warm`) or in a spring. Negative: warming up.
pub fn chill_rate(cold: bool, night: bool, outside: bool, wet: bool, warm: bool, spring: bool) -> f32 {
    if spring {
        return -SPRING_WARM;
    }
    if warm {
        return -FIRE_WARM;
    }
    let mut r = 0.0;
    if cold {
        r += COLD_RATE * if night { 1.0 } else { 0.25 } * if outside { 1.0 } else { 0.2 };
    }
    if wet && (cold || night) {
        r += COLD_RATE * 0.6;
    }
    if r <= 0.0 { -EASE } else { r }
}

/// "Chilly", "Freezing" or nothing.
pub fn chill_words(chill: f32) -> Option<&'static str> {
    if chill >= FREEZING {
        Some("Freezing")
    } else if chill >= CHILLY {
        Some("Chilly")
    } else {
        None
    }
}

/// Every block of a hot spring (the pool's surface is at the origin's height).
pub fn spring_blocks(site: &Site) -> Vec<(IVec3, Id)> {
    let (o, s) = (site.origin, site.seed);
    let r = 3 + (s % 2) as i32;
    let mut out = Vec::new();
    for dz in -r - 1..=r + 1 {
        for dx in -r - 1..=r + 1 {
            let d = ((dx * dx + dz * dz) as f32).sqrt();
            let edge = d > r as f32 - 0.5;
            if d > r as f32 + 1.2 {
                continue;
            }
            // Clear the snow and plants above.
            for y in 1..=4 {
                out.push((o + ivec3(dx, y, dz), AIR));
            }
            if edge {
                // A crusty rim of warm rock (and snow melted round it).
                out.push((o + ivec3(dx, 0, dz), if hash3(s, dx, 0, dz) < 0.6 { SPRING_ROCK } else { STONE }));
                // (On a footing, where the ground falls away.)
                for y in -3..0 {
                    out.push((o + ivec3(dx, y, dz), STONE));
                }
                continue;
            }
            let depth = if d < r as f32 - 1.5 { 3 } else { 2 };
            for y in -depth + 1..=0 {
                out.push((o + ivec3(dx, y, dz), WATER));
            }
            out.push((o + ivec3(dx, -depth, dz), SPRING_ROCK));
            out.push((o + ivec3(dx, -depth - 1, dz), SPRING_ROCK));
        }
    }
    out
}

impl Generator {
    /// Might a hot spring start in chunk (cx, cz)?
    pub fn spring_spot(&self, cx: i32, cz: i32) -> bool {
        self.opts.version >= 5 && self.opts.structures > 0 && (cx.rem_euclid(3), cz.rem_euclid(3)) == (1, 1) && hash2(self.seed ^ 0x5_9416, cx.div_euclid(3), cz.div_euclid(3)) < 0.45
    }

    /// A hot spring here, if it's snowy mountains and roughly level.
    pub fn spring_site(&self, ox: i32, oz: i32, h: i32, biome: Biome, facing: u8, seed: u32) -> Option<Site> {
        let snowy = matches!(biome, Biome::Snowy | Biome::IceSpikes) || (matches!(biome, Biome::StonyPeaks | Biome::Taiga) && h > self.sea() + 20);
        if !snowy || h <= self.sea() + 6 {
            return None;
        }
        let level = [(-4, -4), (4, -4), (-4, 4), (4, 4)].iter().all(|&(dx, dz)| (self.column(ox + dx, oz + dz).0 - h).abs() <= 2);
        level.then_some(Site { kind: Kind::HotSpring, origin: ivec3(ox, h, oz), facing, seed })
    }
}

impl Game {
    /// Bathing in a hot spring: in water with Spring Rock under it.
    pub fn in_hot_spring(&self) -> bool {
        let b = &self.player.body;
        if !b.in_water {
            return false;
        }
        let feet = b.pos.floor().as_ivec3();
        (0..=3).any(|d| self.world.get_v(feet - IVec3::Y * d) == SPRING_ROCK)
    }

    /// The local player's chill, a frame at a time.
    pub fn chill_tick(&mut self, dt: f32) {
        if self.creative || self.spectator || self.dead.is_some() || self.away() {
            self.chill = 0.0;
            return;
        }
        let p = self.player.body.pos;
        let (x, y, z) = (p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
        let biome = self.world.generator.column(x, z).1;
        let open = self.world.dim().open_sky();
        let cold_here = open && cold(biome, y, self.world.sea());
        let outside = self.world.sky_light(x, y + 1, z) >= 0.5;
        let wet = (self.player.body.in_water && !self.in_hot_spring()) || self.rained_on(x, y + 1, z);
        let warm = !open || self.world.block_level(x, y, z) >= 10 || self.world.block_level(x, y + 1, z) >= 10;
        let spring = self.in_hot_spring();
        let before = self.chill;
        let rate = chill_rate(cold_here, self.is_night(), outside, wet, warm, spring);
        self.chill = (self.chill + rate * dt).clamp(0.0, MAX_CHILL);
        if spring {
            // A soak heals, slowly.
            self.spring_heal += dt;
            if self.spring_heal >= 2.0 {
                self.spring_heal = 0.0;
                self.player.health = (self.player.health + 0.5).min(crate::player::MAX_HEALTH);
            }
            self.advance("hot_tub");
            if before >= FREEZING && self.chill < CHILLY {
                self.advance("thawed");
            }
        }
        if self.chill >= FREEZING {
            self.timed_effect(crate::potions::Potion::Slowness, 1.0);
            self.player.hunger.exhaust(dt * 0.02);
            if before < FREEZING {
                self.msg("You're freezing. Find a fire, a roof, a hot meal or a hot spring.");
            }
        } else if self.chill >= CHILLY && before < CHILLY {
            self.msg("You're getting chilly. Your health won't come back by itself until you warm up.");
        }
        // At the very worst it bites (but never past half a heart).
        if self.chill >= MAX_CHILL {
            self.chill_bite += dt;
            if self.chill_bite >= 4.0 && self.player.health > 1.0 {
                self.chill_bite = 0.0;
                self.player.hurt = 0.0;
                self.hurt_player(1.0f32.min(self.player.health - 1.0), "froze. A long night out, that");
            }
        }
    }

    /// Warm up by `amount` (a hot meal; see cooking.rs).
    pub fn warm_up(&mut self, amount: f32) {
        self.chill = (self.chill - amount).max(0.0);
    }

    /// Steam rising off hot springs near the eye (everyone sees it from the land).
    pub fn spring_steam(&mut self, dt: f32) {
        if self.away() || !self.rng.chance((dt * 4.0).min(1.0)) {
            return;
        }
        let eye = self.player.eye();
        let Some(o) = self.world.nearest_site(Kind::HotSpring, eye, 2) else { return };
        if o.as_vec3().distance(eye) > 40.0 {
            return;
        }
        let (dx, dz) = (self.rng.range(-3.0, 3.0), self.rng.range(-3.0, 3.0));
        let at = o.as_vec3() + Vec3::new(0.5 + dx, 1.0, 0.5 + dz);
        if crate::block::is_water(self.world.get_v(at.floor().as_ivec3() - IVec3::Y)) {
            self.smoke(at, 1, 0.2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::GenOptions;

    #[test]
    fn the_cold_chills_and_warmth_thaws() {
        // A cold night out: about two minutes to freezing.
        let r = chill_rate(true, true, true, false, false, false);
        assert!(r > 0.0 && FREEZING / r < 150.0 && FREEZING / r > 60.0, "{r}");
        assert!(chill_rate(true, true, true, true, false, false) > r, "wet is worse");
        assert!(chill_rate(true, false, true, false, false, false) < r, "day is milder");
        assert!(chill_rate(false, false, true, false, false, false) < 0.0, "warm places ease it");
        assert!(chill_rate(true, true, true, true, true, false) < 0.0, "a fire wins");
        assert!(chill_rate(true, true, true, true, false, true) < chill_rate(true, true, true, true, true, false), "a spring beats a fire");
        assert_eq!(chill_words(10.0), None);
        assert_eq!(chill_words(50.0), Some("Chilly"));
        assert_eq!(chill_words(90.0), Some("Freezing"));
    }

    #[test]
    fn springs_steam_in_snowy_mountains_and_bathing_warms() {
        let s = Site { kind: Kind::HotSpring, origin: ivec3(10, 90, 10), facing: 0, seed: 7 };
        let b: std::collections::HashMap<IVec3, Id> = spring_blocks(&s).into_iter().collect();
        assert_eq!(b.get(&s.origin), Some(&WATER));
        assert!(b.values().any(|&id| id == SPRING_ROCK));
        // Newer worlds have them, in the cold.
        let mut found = 0;
        for seed in 1..6u32 {
            let g = Generator::with(seed, GenOptions::DEFAULT);
            if let Some(p) = g.nearest_site(Kind::HotSpring, Vec3::ZERO, 40) {
                found += 1;
                assert!(matches!(g.column(p.x, p.z).1, Biome::Snowy | Biome::IceSpikes | Biome::StonyPeaks | Biome::Taiga));
            }
        }
        assert!(found >= 3, "springs in {found} of 5 worlds");
        assert!(Generator::with(1, GenOptions { version: 4, ..GenOptions::DEFAULT }).nearest_site(Kind::HotSpring, Vec3::ZERO, 40).is_none(), "not in older worlds");
        // In the water over Spring Rock: the chill goes and health comes back.
        let mut g = crate::game::tests::arena(931);
        let pool = IVec3::new(4, 51, 4);
        g.world.set_v(pool - IVec3::Y, SPRING_ROCK);
        g.world.set_v(pool, WATER);
        g.player.body.pos = pool.as_vec3() + Vec3::new(0.5, 0.1, 0.5);
        g.player.body.in_water = true;
        assert!(g.in_hot_spring());
        g.chill = 90.0;
        g.player.health = 10.0;
        for _ in 0..80 {
            g.player.body.in_water = true;
            g.chill_tick(0.1);
        }
        assert!(g.chill < CHILLY, "{}", g.chill);
        assert!(g.player.health > 10.0);
    }
}
