//! Sniffers: huge, gentle and very old.
//!
//! - A **Sniffer Egg** turns up now and then in the deepest layer of a
//!   Trailfolk dig site. Put it down and it hatches after a while (sooner on
//!   moss) into a baby Sniffer.
//! - Sniffers wander about, and every minute or so stop, sniff, and dig up an
//!   ancient seed from grass, dirt, moss or mud: **Torchflower Seeds** or a
//!   **Pitcher Pod**.
//! - Plant a Pitcher Pod on grass, dirt or farmland and it slowly grows into
//!   a **Pitcher Plant**. Feed Sniffers Torchflower Seeds to breed them.

use crate::block::*;
use crate::noise::Rng;

/// Ground a Sniffer can dig in.
pub fn diggable(id: Id) -> bool {
    matches!(id, GRASS | DIRT | MUD | PALE_MOSS | SNOW_GRASS | FARMLAND | FARMLAND_WET)
}

/// What a Sniffer digs up.
pub fn dig_up(rng: &mut Rng) -> Id {
    if rng.chance(0.5) { TORCHFLOWER_SEEDS } else { PITCHER_POD }
}

/// Ground an ancient seed can be planted on, and what it grows from.
pub fn sprout_of(seed: Id) -> Option<Id> {
    match seed {
        TORCHFLOWER_SEEDS => Some(TORCHFLOWER_SPROUT),
        PITCHER_POD => Some(PITCHER_CROP),
        _ => None,
    }
}

/// Chance per random tick that an egg hatches (and on moss).
pub const HATCH_CHANCE: f32 = 0.02;
pub const HATCH_CHANCE_MOSS: f32 = 0.05;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{Mob, MobEvent, MobKind};
    use crate::world::World;
    use macroquad::math::Vec3;

    #[test]
    fn sniffers_dig_up_ancient_seeds() {
        let mut rng = Rng::new(8);
        let mut world = World::new(3);
        world.load_now(0, 0);
        let h = world.surface_y(8, 8);
        world.set(8, h, 8, GRASS);
        let mut s = Mob::new(MobKind::Sniffer, Vec3::new(8.5, h as f32 + 1.0, 8.5), &mut rng);
        s.body.on_ground = true;
        let mut found = Vec::new();
        for _ in 0..4000 {
            for e in s.update(0.05, &world, Vec3::new(40.0, 80.0, 40.0), false, 1.0, &mut rng) {
                if let MobEvent::DropItem(_, item) = e {
                    found.push(item);
                }
            }
            s.body.pos = Vec3::new(8.5, h as f32 + 1.0, 8.5);
            s.body.on_ground = true;
        }
        assert!(!found.is_empty(), "it dug something up");
        assert!(found.iter().all(|&i| i == TORCHFLOWER_SEEDS || i == PITCHER_POD));
        assert_eq!(sprout_of(PITCHER_POD), Some(PITCHER_CROP));
    }
}
