//! Building blocks: **Concrete**, **Concrete Powder**, **Glazed Terracotta**,
//! **Candles**, **Chains**, **Scaffolding** and **hanging Lanterns**.
//!
//! Concrete Powder (four sand, four gravel and a dye) falls like sand, and
//! sets into Concrete the moment it touches water: placed beside it, landing
//! in it, or when water flows up to it later. Glazed Terracotta comes from
//! baking Concrete. Candles light with a Sparker (or a torch) and go out with
//! an empty hand. Scaffolding is climbed from inside, like a ladder you can
//! stand in. A Lantern put on the underside of a block hangs from it.

use crate::block::*;
use crate::game::Game;
use crate::sound::Sfx;
use crate::world::World;
use macroquad::math::{IVec3, Vec3};

pub fn is_powder(id: Id) -> bool {
    (CONCRETE_POWDER_FIRST..CONCRETE_POWDER_FIRST + 8).contains(&id)
}

pub fn is_concrete(id: Id) -> bool {
    (CONCRETE_FIRST..CONCRETE_FIRST + 8).contains(&id)
}

/// The concrete a powder sets into.
pub fn set_form(powder: Id) -> Id {
    powder - CONCRETE_POWDER_FIRST + CONCRETE_FIRST
}

/// Is there water touching `p`?
pub fn wet(world: &World, p: IVec3) -> bool {
    [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z].iter().any(|&d| is_water(world.get_v(p + d)))
}

impl Game {
    /// Where the world lives: Concrete Powder at `p` sets if water touches it.
    pub fn harden_powder(&mut self, p: IVec3) {
        let id = self.world.get_v(p);
        if is_powder(id) && wet(&self.world, p) {
            self.world.set_v(p, set_form(id));
            self.sfx(Sfx::Hiss, Some(p.as_vec3() + Vec3::splat(0.5)));
        }
    }

    /// Right-clicked a candle: light it with a Sparker or torch, put it out empty-handed.
    pub fn use_candle(&mut self, p: IVec3) -> bool {
        let held = self.inv.held();
        let new = match self.world.get_v(p) {
            CANDLE if matches!(held, SPARKER | TORCH) => CANDLE_LIT,
            CANDLE_LIT if held == AIR => CANDLE,
            _ => return false,
        };
        self.world.set_v(p, new);
        if held == SPARKER {
            self.use_tool(1);
        }
        self.sfx(if new == CANDLE_LIT { Sfx::Click } else { Sfx::Hiss }, Some(p.as_vec3() + Vec3::splat(0.5)));
        self.player.swing = 1.0;
        true
    }
}

/// Is a body in this cell range in scaffolding (so it can climb)?
pub fn in_scaffolding(world: &World, min: Vec3, max: Vec3) -> bool {
    let (x0, x1) = (min.x.floor() as i32, (max.x - 1e-3).floor() as i32);
    let (z0, z1) = (min.z.floor() as i32, (max.z - 1e-3).floor() as i32);
    let (y0, y1) = (min.y.floor() as i32, (min.y + 1.0).floor() as i32);
    (y0..=y1).any(|y| (z0..=z1).any(|z| (x0..=x1).any(|x| world.get(x, y, z) == SCAFFOLDING)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use macroquad::math::ivec3;

    #[test]
    fn concrete_powder_falls_and_sets_in_water() {
        let mut g = crate::game::tests::arena(171);
        assert!(crate::falling::is_gravity(CONCRETE_POWDER_FIRST + 2));
        // Dropped down a hole into a pool.
        let pool = ivec3(3, 49, 3);
        g.world.set_v(pool, WATER);
        g.world.set_v(pool - IVec3::Y, STONE);
        let top = pool + IVec3::Y * 4;
        g.world.set_v(top, CONCRETE_POWDER_FIRST + 2);
        g.world.fall_dirty.insert(top);
        for _ in 0..60 {
            g.falling_tick(0.05);
        }
        assert_eq!(g.world.get_v(pool), CONCRETE_FIRST + 2, "set where it landed");
        // Dry powder stays powder.
        let dry = ivec3(-3, 50, -3);
        g.world.set_v(dry, CONCRETE_POWDER_FIRST);
        g.harden_powder(dry);
        assert_eq!(g.world.get_v(dry), CONCRETE_POWDER_FIRST);
        g.world.set_v(dry + IVec3::X, WATER);
        g.harden_powder(dry);
        assert_eq!(g.world.get_v(dry), CONCRETE_FIRST);
    }

    #[test]
    fn candles_light_and_go_out_and_scaffolding_is_climbed() {
        let mut g = crate::game::tests::arena(172);
        let c = ivec3(1, 50, 1);
        g.world.set_v(c, CANDLE);
        assert!(!g.use_candle(c), "an empty hand can't light it");
        g.inv.slots[0] = Some((SPARKER, 1));
        g.inv.selected = 0;
        assert!(g.use_candle(c));
        assert_eq!(g.world.get_v(c), CANDLE_LIT);
        assert!(crate::light::emission(CANDLE_LIT) > 0);
        g.inv.slots[0] = None;
        assert!(g.use_candle(c));
        assert_eq!(g.world.get_v(c), CANDLE);
        g.world.set_v(ivec3(4, 50, 4), SCAFFOLDING);
        assert!(in_scaffolding(&g.world, Vec3::new(4.2, 50.0, 4.2), Vec3::new(4.8, 51.8, 4.8)));
        assert_eq!(placing_item(LANTERN_HANGING), Some(LANTERN));
        assert_eq!(placing_item(CANDLE_LIT), Some(CANDLE));
    }
}
