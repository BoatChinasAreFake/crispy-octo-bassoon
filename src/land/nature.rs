//! Little touches on the ground: **Leaf Litter** drifts under forest trees,
//! **Wildflowers** fill the meadows, and **Firefly Bushes** grow in swamps
//! and mangroves. At night fireflies drift and blink around the bushes near
//! you (just a sight, so each player's game makes its own).

use crate::block::*;
use crate::entity::Particle;
use crate::game::Game;
use macroquad::math::{ivec3, Vec3};

/// How far around us bushes are looked for, and how often.
const RANGE: i32 = 14;
const EVERY: f32 = 0.15;
/// Bushes looked at each time (picked at random).
const LOOKS: usize = 60;
/// The glowing yellow dot (a firework spark).
pub const FIREFLY_TILE: u16 = crate::texture::T_SPARK_FIRST + 2;

impl Game {
    /// Fireflies around Firefly Bushes near us, after dark.
    pub fn fireflies_tick(&mut self, dt: f32) {
        if self.dedicated || self.menu || !self.is_night() {
            return;
        }
        self.firefly_acc += dt;
        if self.firefly_acc < EVERY {
            return;
        }
        self.firefly_acc = 0.0;
        let me = self.player.body.pos.floor().as_ivec3();
        for _ in 0..LOOKS {
            let p = me + ivec3(self.rng.int(-RANGE, RANGE), self.rng.int(-4, 4), self.rng.int(-RANGE, RANGE));
            if self.world.get_v(p) != FIREFLY_BUSH {
                continue;
            }
            let r = &mut self.rng;
            let at = p.as_vec3() + Vec3::new(r.range(-1.5, 2.5), r.range(0.3, 2.5), r.range(-1.5, 2.5));
            let vel = Vec3::new(r.range(-0.4, 0.4), r.range(-0.15, 0.25), r.range(-0.4, 0.4));
            self.particles.push(Particle { pos: at, vel, life: r.range(1.2, 2.6), tile: FIREFLY_TILE, uv: [0.25, 0.25], size: 0.06, gravity: 0.0 });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fireflies_come_out_at_night_near_their_bushes() {
        let mut g = crate::game::tests::arena(91);
        g.world.set_v(ivec3(2, 50, 2), FIREFLY_BUSH);
        g.time = 0.25; // noon: none
        for _ in 0..40 {
            g.fireflies_tick(0.2);
        }
        assert!(!g.particles.iter().any(|p| p.tile == FIREFLY_TILE));
        g.time = 0.75;
        for _ in 0..200 {
            g.fireflies_tick(0.2);
        }
        assert!(g.particles.iter().any(|p| p.tile == FIREFLY_TILE && p.pos.distance(Vec3::new(2.5, 51.0, 2.5)) < 4.0));
    }
}
