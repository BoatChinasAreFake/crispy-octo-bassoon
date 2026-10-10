//! Floaties (legally distinct happy ghasts): big, gentle, cloud-white fliers.
//!
//! - A **Dried Floaty** (found in Scorch Fortress chests) put down next to water
//!   slowly soaks it up and wakes as a baby Floaty. Babies grow up in twenty
//!   minutes; feed one flowers to hurry it along.
//! - Put a **Harness** (wool, string and glass) on a grown Floaty and it waits
//!   where it is. Right-click to climb aboard: four seats, the first one aboard
//!   steers. It flies where the driver looks, W to go, Jump to rise; sneak to
//!   get off. Shears take the harness back off.
//! - A wild grown-up comes over to see anyone holding a Harness.
//!
//! Where the world lives decides who sits where; the driver's game moves it
//! (like a Galloper's, see horses.rs).

use crate::animals::Interaction;
use crate::block::*;
use crate::entity::MobKind;
use crate::game::Game;
use crate::sound::Sfx;
use macroquad::math::{IVec3, Vec3};

/// How long a baby takes to grow up, and how much a flower takes off.
pub const GROW_SECS: f32 = 1200.0;
const FLOWER_SECS: f32 = 60.0;
/// Chance a random tick wakes a Dried Floaty that's next to water.
pub const HATCH_CHANCE: f32 = 0.05;
/// Top speed, and how fast it rises (Jump) or sinks (looking down).
pub const FLY_SPEED: f32 = 6.0;
pub const CLIMB: f32 = 3.5;
/// How far a wild Floaty notices someone holding a Harness.
const CURIOUS: f32 = 16.0;

/// Next to water on any side.
pub fn soaked(world: &crate::world::World, p: IVec3) -> bool {
    [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z].iter().any(|&d| is_water(world.get_v(p + d)))
}

impl Game {
    /// A Dried Floaty wakes up (where the world lives).
    pub fn hatch_floaty(&mut self, p: IVec3) {
        self.world.set_v(p, AIR);
        let at = p.as_vec3() + Vec3::new(0.5, 0.0, 0.5);
        let id = self.alloc_mob(MobKind::Floaty, at);
        if let Some(m) = self.mobs.iter_mut().find(|m| m.id == id) {
            m.set_baby(GROW_SECS);
            m.persistent = true;
        }
        self.smoke(at + Vec3::Y * 0.5, 6, 0.3);
        self.sfx(Sfx::Splash, Some(at));
        if !self.away() && self.player.body.pos.distance(at) < 24.0 {
            self.advance("floaty_born");
        }
    }

    /// Right-click on a Floaty by `who` (player id + 1 = `rider`) holding `item`.
    pub fn floaty_interact(&mut self, who: &str, i: usize, item: Id, rider: u32) -> Interaction {
        let pos = self.mobs[i].body.pos + Vec3::Y * 2.0;
        let m = &mut self.mobs[i];
        if m.baby > 0.0 {
            if item == FLOWER {
                m.baby = (m.baby - FLOWER_SECS).max(0.5);
                m.persistent = true;
                self.hearts(pos, 3);
                return Interaction::Ate;
            }
            return Interaction::Nothing;
        }
        if item == HARNESS && !m.saddled {
            m.saddled = true;
            m.persistent = true;
            self.sfx(Sfx::Place(crate::sound::Mat::Wood), Some(pos));
            self.advance_for(who, "harnessed");
            return Interaction::Ate;
        }
        if item == SHEARS && m.saddled && m.rider == 0 && m.passenger == 0 && m.crew == [0, 0] {
            m.saddled = false;
            self.pop_drop(pos, HARNESS, 1);
            self.sfx(Sfx::Snip, Some(pos));
            return Interaction::Sheared;
        }
        if !m.saddled || rider == 0 || m.rider == rider || m.passenger == rider || m.crew.contains(&rider) {
            return Interaction::Nothing;
        }
        let seat = if m.rider == 0 {
            m.rider = rider;
            0
        } else if m.passenger == 0 {
            m.passenger = rider;
            1
        } else if let Some(k) = m.crew.iter().position(|&c| c == 0) {
            m.crew[k] = rider;
            2 + k as u32
        } else {
            return Interaction::Nothing;
        };
        let (id, full) = (m.id, m.rider != 0 && m.passenger != 0 && m.crew.iter().all(|&c| c != 0));
        if full {
            self.advance_for(who, "full_flight");
        }
        Interaction::Mounted(id | seat << crate::horses::SEAT_SHIFT)
    }

    /// Wild grown-ups drift over to anyone holding a Harness (after animals_tick).
    pub fn floaties_tick(&mut self) {
        if self.is_client() {
            return;
        }
        let spots = self.player_spots();
        for m in self.mobs.iter_mut().filter(|m| m.kind == MobKind::Floaty && !m.saddled && m.baby <= 0.0) {
            m.goal = spots.iter().filter(|p| p.2 == HARNESS && p.1.distance(m.body.pos) < CURIOUS).map(|p| p.1).next();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dried_floaty_wakes_grows_takes_a_harness_and_four_riders() {
        let mut g = crate::game::tests::arena(71);
        let p = IVec3::new(2, 50, 2);
        g.world.set_v(p, DRIED_FLOATY);
        g.world.set_v(p + IVec3::X, WATER);
        assert!(soaked(&g.world, p));
        g.hatch_floaty(p);
        let i = g.mobs.iter().position(|m| m.kind == MobKind::Floaty).unwrap();
        assert!(g.mobs[i].baby > 0.0 && g.world.get_v(p) == AIR);
        // Too young for a harness; flowers help it grow.
        assert_eq!(g.floaty_interact("", i, HARNESS, 1), Interaction::Nothing);
        let before = g.mobs[i].baby;
        assert_eq!(g.floaty_interact("", i, FLOWER, 1), Interaction::Ate);
        assert!(g.mobs[i].baby < before);
        g.mobs[i].set_baby(0.0);
        assert_eq!(g.floaty_interact("", i, HARNESS, 1), Interaction::Ate);
        let id = g.mobs[i].id;
        let shift = crate::horses::SEAT_SHIFT;
        assert_eq!(g.floaty_interact("", i, AIR, 1), Interaction::Mounted(id));
        assert_eq!(g.floaty_interact("", i, AIR, 2), Interaction::Mounted(id | 1 << shift));
        assert_eq!(g.floaty_interact("", i, AIR, 3), Interaction::Mounted(id | 2 << shift));
        assert_eq!(g.floaty_interact("", i, AIR, 4), Interaction::Mounted(id | 3 << shift));
        assert_eq!(g.floaty_interact("", i, AIR, 5), Interaction::Nothing, "four seats");
        assert_eq!(g.floaty_interact("", i, AIR, 2), Interaction::Nothing, "already aboard");
    }

    #[test]
    fn the_driver_flies_it_up_and_along() {
        let mut g = crate::game::tests::arena(72);
        let id = g.alloc_mob(MobKind::Floaty, Vec3::new(0.5, 51.0, 0.5));
        let i = g.mobs.iter().position(|m| m.id == id).unwrap();
        g.mobs[i].saddled = true;
        let me = g.my_id + 1;
        assert_eq!(g.floaty_interact("", i, AIR, me), Interaction::Mounted(id));
        g.mount_mob(id);
        g.player.yaw = 0.0;
        g.player.pitch = 0.0;
        let start = g.mobs[i].body.pos;
        for _ in 0..60 {
            g.ride_tick(0.05, 1.0, 0.0, true, false);
        }
        let now = g.mobs.iter().find(|m| m.id == id).unwrap().body.pos;
        assert!(now.y > start.y + 3.0, "it climbs: {start} -> {now}");
        assert!(start.z - now.z > 5.0, "and goes where we look (north): {start} -> {now}");
        assert!(g.player.body.pos.y > now.y + 3.0, "we sit on top");
    }
}
