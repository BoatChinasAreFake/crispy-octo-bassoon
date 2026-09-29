//! Fire: it spreads over things that burn, eats them, and goes out.
//!
//! - A **Sparker** lights the block you point at (portals first, if it's an
//!   obsidian frame). Lava sets fire to flammable things beside it as it
//!   flows, and lightning starts fires where it strikes.
//! - Every half second each fire may spread to air next to something
//!   flammable within a block, and may burn up the flammable block it sits
//!   on or beside (turning it into more fire). TNT near a fire goes off.
//! - Fire with nothing to burn dies in a few seconds; on Scorchrock it
//!   burns forever. Rain puts out fires under open sky.
//! - Walking through fire sets you alight (water puts it out). Punch fire
//!   to put it out.
//!
//! Fire lives where the world lives; joined players light it with a
//! Sparker (the host checks they have one) and see it as ordinary edits.

use crate::block::*;
use crate::game::Game;
use crate::sound::{Mat, Sfx};
use crate::world::{World, CH};
use macroquad::math::{ivec3, IVec3, Vec3};

/// Seconds between fire updates.
pub const STEP: f32 = 0.5;
/// Most fires looked at per update (a burning forest is plenty).
const MAX_FIRES: usize = 2048;

const SIDES: [IVec3; 6] = [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z];

/// How readily a block catches (0: never), and how fast it burns away.
pub fn flammability(id: Id) -> (f32, f32) {
    use crate::carpentry::{is_fence, is_gate, is_ladder, is_trapdoor};
    if matches!(id, LEAVES | TALL_GRASS | FLOWER | WEEDS | SAPLING | HAY | SCARECROW) || id == WOOL || (DYED_WOOL..DYED_WOOL + 7).contains(&id) {
        (0.6, 0.6)
    } else if matches!(id, PLANKS | BOOKSHELF | TABLE) || is_fence(id) || is_gate(id) || is_ladder(id) || is_trapdoor(id) || is_door(id) || ((slab_of(id).is_some() || stairs_of(id).is_some()) && made_of(id) == PLANKS) {
        (0.25, 0.2)
    } else if id == LOG {
        (0.1, 0.08)
    } else if id == TNT {
        (1.0, 1.0)
    } else {
        (0.0, 0.0)
    }
}

pub fn flammable(id: Id) -> bool {
    flammability(id).0 > 0.0
}

/// Can fire sit in this cell? (Empty, with something to stand on or to burn.)
pub fn can_burn_at(world: &World, p: IVec3) -> bool {
    if !(0..CH).contains(&p.y) || world.get_v(p) != AIR {
        return false;
    }
    is_solid(world.get_v(p - IVec3::Y)) || SIDES.iter().any(|d| flammable(world.get_v(p + *d)))
}

impl Game {
    /// Light a fire at `p` if it can burn there.
    pub fn ignite(&mut self, p: IVec3) -> bool {
        if !can_burn_at(&self.world, p) {
            return false;
        }
        self.world.set_v(p, FIRE);
        self.sfx(Sfx::Place(Mat::Wood), Some(p.as_vec3() + Vec3::splat(0.5)));
        true
    }

    /// The Sparker used on a block face (after portals): set it alight.
    pub fn spark(&mut self, hit: IVec3, normal: IVec3) -> bool {
        let at = hit + normal;
        if !can_burn_at(&self.world, at) {
            return false;
        }
        // Joined players' fire is lit by the host (which checks the Sparker); their edit goes over.
        self.world.set_v(at, FIRE);
        self.sfx(Sfx::Place(Mat::Wood), Some(at.as_vec3() + Vec3::splat(0.5)));
        self.use_tool(1);
        self.player.swing = 1.0;
        true
    }

    /// Fire spreads, burns and goes out (where the world lives).
    pub fn fire_tick(&mut self, dt: f32) {
        if self.is_client() {
            return;
        }
        self.fire_timer += dt;
        if self.fire_timer < STEP {
            return;
        }
        self.fire_timer = 0.0;
        let raining = self.weather.kind.wet();
        let fires: Vec<IVec3> = self.world.fires.iter().copied().filter(|p| self.world.is_loaded(p.x, p.z)).take(MAX_FIRES).collect();
        let mut out = Vec::new();
        let mut burn = Vec::new();
        let mut spread = Vec::new();
        for p in fires {
            if self.world.get_v(p) != FIRE {
                continue;
            }
            let below = self.world.get_v(p - IVec3::Y);
            let eternal = below == crate::block::SCORCHROCK;
            let fuel: Vec<IVec3> = SIDES.iter().map(|d| p + *d).filter(|q| flammable(self.world.get_v(*q))).collect();
            if !eternal && !is_solid(below) && fuel.is_empty() {
                out.push(p);
                continue;
            }
            if raining && self.world.sky_level(p.x, p.y, p.z) >= 15 && self.rng.chance(0.4) {
                out.push(p);
                continue;
            }
            if !eternal && fuel.is_empty() && self.rng.chance(0.15) {
                out.push(p);
                continue;
            }
            for q in fuel {
                let (catch, eat) = flammability(self.world.get_v(q));
                if self.rng.chance(eat * STEP) {
                    burn.push(q);
                } else if self.rng.chance(catch * 0.12) {
                    // Catch the air beside it too.
                    let side = q + SIDES[self.rng.int(0, 5) as usize];
                    spread.push(side);
                }
            }
            // Leap a little, to air near fuel.
            if self.rng.chance(0.05) {
                let jump = p + ivec3(self.rng.int(-1, 1), self.rng.int(-1, 1), self.rng.int(-1, 1));
                if SIDES.iter().any(|d| flammable(self.world.get_v(jump + *d))) {
                    spread.push(jump);
                }
            }
        }
        for p in out {
            if self.world.get_v(p) == FIRE {
                self.world.set_v(p, AIR);
            }
        }
        for q in burn {
            let id = self.world.get_v(q);
            if id == TNT {
                self.world.set_v(q, AIR);
                self.tnts.push(crate::entity::PrimedTnt { pos: q.as_vec3(), fuse: 2.0 });
            } else if flammable(id) {
                // It burns up: fire where it stood, or nothing if nothing can hold fire there.
                if is_door(id) {
                    self.remove_door_partner(q, id);
                }
                self.world.set_v(q, AIR);
                if can_burn_at(&self.world, q) && self.rng.chance(0.7) {
                    self.world.set_v(q, FIRE);
                }
            }
        }
        for p in spread {
            if can_burn_at(&self.world, p) && SIDES.iter().any(|d| flammable(self.world.get_v(p + *d))) {
                self.world.set_v(p, FIRE);
            }
        }
    }

    /// Lava flowing into `p` sets flammable things around it alight.
    pub fn lava_ignites(&mut self, p: IVec3) {
        for d in SIDES {
            let q = p + d;
            if flammable(self.world.get_v(q)) {
                for e in SIDES {
                    if self.rng.chance(0.2) && can_burn_at(&self.world, q + e) {
                        self.world.set_v(q + e, FIRE);
                        return;
                    }
                }
            }
        }
    }
}

/// Is any part of this box in fire?
pub fn touches_fire(world: &World, min: Vec3, max: Vec3) -> bool {
    (min.y.floor() as i32..=max.y.floor() as i32).any(|y| (min.z.floor() as i32..=max.z.floor() as i32).any(|z| (min.x.floor() as i32..=max.x.floor() as i32).any(|x| world.get(x, y, z) == FIRE)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fire_spreads_through_wood_and_burns_out() {
        let mut g = crate::game::tests::arena(51);
        g.weather.kind = crate::weather::Weather::Clear;
        // A little plank hut floor with a wooden wall.
        for x in 0..6 {
            g.world.set_v(ivec3(x, 50, 3), PLANKS);
            g.world.set_v(ivec3(x, 51, 3), PLANKS);
        }
        assert!(g.ignite(ivec3(0, 50, 2)));
        assert_eq!(g.world.get_v(ivec3(0, 50, 2)), FIRE);
        for _ in 0..2400 {
            g.fire_tick(0.05);
        }
        let planks = (0..6).flat_map(|x| [50, 51].map(|y| ivec3(x, y, 3))).filter(|&p| g.world.get_v(p) == PLANKS).count();
        assert!(planks < 12, "nothing burned");
        // With the fuel gone, every fire dies out.
        for _ in 0..4000 {
            g.fire_tick(0.05);
        }
        assert!(g.world.fires.is_empty(), "{} fires still going", g.world.fires.len());
    }

    #[test]
    fn stone_doesnt_burn_and_fire_needs_a_floor() {
        let mut g = crate::game::tests::arena(52);
        assert!(!g.ignite(ivec3(0, 55, 0)), "fire in mid-air");
        assert!(g.ignite(ivec3(0, 50, 0)));
        for _ in 0..400 {
            g.fire_tick(0.05);
        }
        assert_eq!(g.world.get_v(ivec3(0, 50, 0)), AIR);
        assert_eq!(g.world.get_v(ivec3(0, 49, 0)), STONE);
    }
}
