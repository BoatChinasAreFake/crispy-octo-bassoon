//! Slow changes to blocks near players, Minecraft's "random ticks": a few
//! random blocks in every 16-high slice of every nearby chunk get a look each
//! moment. That's what makes:
//!
//! - **Copper** weather: new, exposed, weathered, oxidized, about twenty
//!   minutes a stage. Right-click with Goo to wax it and keep it as it is
//!   (waxed copper never changes).
//! - **Bamboo** grow, a block at a time, up to sixteen tall.
//! - **Coral** die (turn grey) when no water touches it.
//! - **Torchflower** sprouts bloom (see archaeology.rs).
//!
//! Everything happens where the world lives; joined players see the edits.

use crate::block::*;
use crate::game::Game;
use crate::world::{CH, CW};
use macroquad::math::{ivec3, IVec3};
use std::collections::HashSet;

/// Random ticks per 16-high slice of a chunk, per second (Minecraft's 3 a tick).
const PER_SECTION: u32 = 60;
/// Chunks this far (in chunks) from a player get ticked.
const RADIUS: i32 = 6;
/// Chance a ticked copper block moves on a stage (so ~20 minutes a stage).
const WEATHER_CHANCE: f32 = 0.05;
const BAMBOO_CHANCE: f32 = 0.35;
pub const BAMBOO_MAX: i32 = 16;

pub fn is_copper(id: Id) -> bool {
    (COPPER_FIRST..COPPER_FIRST + 4).contains(&id)
}

/// The waxed version of an unwaxed copper block.
pub fn waxed(id: Id) -> Option<Id> {
    is_copper(id).then(|| id - COPPER_FIRST + WAXED_COPPER_FIRST)
}

pub fn is_coral(id: Id) -> bool {
    (CORAL_FIRST..CORAL_FIRST + 4).contains(&id)
}

impl Game {
    /// Pick random blocks near every player and let them change.
    pub fn random_ticks(&mut self, dt: f32) {
        if self.is_client() || self.menu {
            return;
        }
        self.random_tick_acc += dt;
        // Four rounds a second, a quarter of the ticks each.
        while self.random_tick_acc >= 0.25 {
            self.random_tick_acc -= 0.25;
            let mut centres: Vec<(i32, i32)> = self.peers.values().map(|p| (p.target.x.floor() as i32, p.target.z.floor() as i32)).collect();
            if !self.dedicated {
                let p = self.player.body.pos;
                centres.push((p.x.floor() as i32, p.z.floor() as i32));
            }
            let mut chunks = HashSet::new();
            for (x, z) in centres {
                let (cx, cz) = (x.div_euclid(CW), z.div_euclid(CW));
                for dz in -RADIUS..=RADIUS {
                    for dx in -RADIUS..=RADIUS {
                        chunks.insert((cx + dx, cz + dz));
                    }
                }
            }
            for (cx, cz) in chunks {
                if !self.world.chunks.contains_key(&(cx, cz)) {
                    continue;
                }
                for section in 0..CH / 16 {
                    for _ in 0..PER_SECTION / 4 {
                        let p = ivec3(cx * CW + self.rng.int(0, CW - 1), section * 16 + self.rng.int(0, 15), cz * CW + self.rng.int(0, CW - 1));
                        self.random_tick(p);
                    }
                }
            }
        }
    }

    fn random_tick(&mut self, p: IVec3) {
        let id = self.world.get_v(p);
        if is_copper(id) && id < COPPER_FIRST + 3 {
            if self.rng.chance(WEATHER_CHANCE) {
                self.world.set_v(p, id + 1);
                if id + 1 == COPPER_FIRST + 3 && !self.dedicated && p.as_vec3().distance(self.player.body.pos) < 24.0 {
                    self.advance("patina");
                }
            }
        } else if id == BAMBOO {
            let up = p + IVec3::Y;
            if self.world.get_v(up) == AIR && up.y < CH - 1 && self.rng.chance(BAMBOO_CHANCE) {
                let mut height = 1;
                while height < BAMBOO_MAX && self.world.get_v(p - IVec3::Y * height) == BAMBOO {
                    height += 1;
                }
                if height < BAMBOO_MAX {
                    self.world.set_v(up, BAMBOO);
                }
            }
        } else if id == TORCHFLOWER_SPROUT {
            // Ancient seeds take their time (and want a bit of light).
            if self.rng.chance(0.08) && self.world.sky_light(p.x, p.y, p.z) > 0.3 {
                self.world.set_v(p, TORCHFLOWER);
            }
        } else if is_coral(id) {
            let wet = [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z].iter().any(|&d| is_water(self.world.get_v(p + d)));
            if !wet {
                self.world.set_v(p, DEAD_CORAL);
            }
        }
    }

    /// Goo on a copper block waxes it. Returns whether it did.
    pub fn wax_copper(&mut self, pos: IVec3) -> bool {
        let Some(w) = waxed(self.world.get_v(pos)) else { return false };
        self.world.set_v(pos, w);
        self.sfx(crate::sound::Sfx::Place(crate::sound::Mat::Glass), Some(pos.as_vec3()));
        self.player.swing = 1.0;
        self.advance("waxed");
        // Joined players: the host takes the Goo when it sees the edit (see ledger.rs).
        if !self.creative {
            self.inv.consume_held();
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copper_waxes_and_coral_needs_water() {
        assert_eq!(waxed(COPPER_FIRST + 2), Some(WAXED_COPPER_FIRST + 2));
        assert_eq!(waxed(WAXED_COPPER_FIRST), None);
        assert!(is_coral(CORAL_FIRST + 3) && !is_coral(DEAD_CORAL));
        assert_eq!(block(WAXED_COPPER_FIRST + 3).tex, block(COPPER_FIRST + 3).tex, "waxed looks the same");
    }
}
