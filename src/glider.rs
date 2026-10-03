//! Gliders: wear one in the chest slot, jump off something high, and press
//! Jump again mid-fall to spread it. Look down to dive and gain speed, look up
//! to trade speed for height. A Boom Rocket used while gliding pushes you
//! along the way you're looking. Flying into a wall at speed hurts, the
//! Glider wears a little every second in the air, and landing (or water)
//! folds it. Found in the Hollow's outer spires (see hollow.rs).
//!
//! The flight model is Minecraft's elytra, run at its 20 steps a second.

use crate::block::*;
use crate::game::Game;
use crate::sound::Sfx;
use macroquad::math::Vec3;

/// Seconds of push from one rocket.
pub const ROCKET_SECS: f32 = 1.2;
const STEP: f32 = 1.0 / 20.0;

/// One of Minecraft's 20-a-second glide steps. `v` is in blocks per step;
/// `pitch` is positive looking up (our convention); `look` is where we face.
pub fn glide_step(v: &mut Vec3, look: Vec3, pitch: f32, boosting: bool) {
    let down = -pitch; // Minecraft's pitch: positive looking down
    let flat = (look.x * look.x + look.z * look.z).sqrt();
    let speed = (v.x * v.x + v.z * v.z).sqrt();
    let lift = down.cos().powi(2);
    v.y += -0.08 + lift * 0.06;
    if v.y < 0.0 && flat > 0.0 {
        // Falling turns into forward speed.
        let d = v.y * -0.1 * lift;
        v.y += d;
        v.x += look.x * d / flat;
        v.z += look.z * d / flat;
    }
    if down < 0.0 && flat > 0.0 {
        // Pulling up trades speed for height.
        let d = speed * -down.sin() * 0.04;
        v.y += d * 3.2;
        v.x -= look.x * d / flat;
        v.z -= look.z * d / flat;
    }
    if flat > 0.0 {
        // Steer toward where we look.
        v.x += (look.x / flat * speed - v.x) * 0.1;
        v.z += (look.z / flat * speed - v.z) * 0.1;
    }
    if boosting {
        *v += look * 0.1 + (look * 1.5 - *v) * 0.5;
    }
    v.x *= 0.99;
    v.y *= 0.98;
    v.z *= 0.99;
}

/// Advance a glide by `dt` seconds (in whole steps, carrying the remainder).
/// `vel` is in blocks per second.
pub fn glide(vel: &mut Vec3, look: Vec3, pitch: f32, boost: &mut f32, acc: &mut f32, dt: f32) {
    *acc += dt;
    let mut v = *vel * STEP;
    while *acc >= STEP {
        *acc -= STEP;
        glide_step(&mut v, look, pitch, *boost > 0.0);
        *boost = (*boost - STEP).max(0.0);
    }
    *vel = v / STEP;
}

/// Damage for hitting a wall: how much horizontal speed (blocks a second) the
/// crash took off, as Minecraft does it.
pub fn crash_damage(before: f32, after: f32) -> f32 {
    ((before - after) / 20.0 * 10.0 - 3.0).max(0.0)
}

impl Game {
    /// Is the player wearing a Glider that still works?
    pub fn wearing_glider(&self) -> bool {
        self.inv.armor[CHESTPLATE].is_some_and(|(id, _)| id == GLIDER)
    }

    /// Each second of gliding wears the Glider a little.
    pub fn glider_tick(&mut self, dt: f32) {
        if !self.player.gliding {
            self.glide_wear = 0.0;
            return;
        }
        self.advance("wings");
        if self.creative {
            return;
        }
        self.glide_wear += dt;
        while self.glide_wear >= 1.0 {
            self.glide_wear -= 1.0;
            let w = self.inv.armor_wear[CHESTPLATE];
            let Some(max) = crate::inventory::max_uses(GLIDER, w) else { return };
            let used = crate::inventory::uses(w) as u32 + 1;
            if used >= max {
                self.inv.armor[CHESTPLATE] = None;
                self.inv.armor_wear[CHESTPLATE] = 0;
                self.player.gliding = false;
                self.sfx(Sfx::Break(crate::sound::Mat::Glass), None);
                self.msg("Your Glider tore. Gravity says hello.");
                return;
            }
            self.inv.armor_wear[CHESTPLATE] = crate::inventory::with_uses(w, used as u16);
        }
    }

    /// Using a rocket: a push while gliding, a harmless bang otherwise.
    pub fn use_rocket(&mut self) {
        if !self.player.gliding {
            // On foot: it's a firework (see fireworks.rs).
            self.set_off_firework();
            return;
        }
        let at = self.player.eye() + self.player.look_dir() * 0.8;
        self.sfx(Sfx::Explode, Some(at));
        for _ in 0..12 {
            let v = Vec3::new(self.rng.range(-3.0, 3.0), self.rng.range(1.0, 5.0), self.rng.range(-3.0, 3.0));
            let colour = [crate::texture::T_FLAME, crate::texture::T_WHITE][self.rng.int(0, 1) as usize];
            self.particles.push(crate::entity::Particle { pos: at, vel: v, life: 0.7, tile: colour, uv: [0.3, 0.3], size: 0.12, gravity: 4.0 });
        }
        if self.player.gliding {
            self.player.boost = ROCKET_SECS;
            self.advance("rocket_man");
        }
        if !self.creative {
            self.use_up_held();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diving_gains_speed_and_pulling_up_climbs() {
        // Level flight slowly sinks; diving builds speed.
        let mut v = Vec3::new(0.0, -0.5, -0.5);
        let ahead = Vec3::new(0.0, 0.0, -1.0);
        for _ in 0..40 {
            glide_step(&mut v, (ahead + Vec3::new(0.0, -1.0, 0.0)).normalize(), -0.78, false);
        }
        let dive_speed = v.length();
        assert!(dive_speed > 1.0, "a dive gets fast: {dive_speed}");
        // Now pull up: the climb eats the speed.
        let up = (ahead + Vec3::new(0.0, 0.6, 0.0)).normalize();
        for _ in 0..10 {
            glide_step(&mut v, up, 0.54, false);
        }
        assert!(v.y > 0.0, "pulling up climbs: {v:?}");
        assert!(v.length() < dive_speed);
    }

    #[test]
    fn rockets_push_along_the_look() {
        let mut v = Vec3::ZERO;
        let look = Vec3::new(1.0, 0.0, 0.0);
        for _ in 0..20 {
            glide_step(&mut v, look, 0.0, true);
        }
        assert!(v.x > 1.2, "{v:?}");
    }

    #[test]
    fn stepping_is_steady_whatever_the_frame_rate() {
        let look = Vec3::new(0.0, -0.3, -1.0).normalize();
        let run = |dt: f32| {
            let (mut vel, mut boost, mut acc) = (Vec3::new(0.0, -2.0, -8.0), 0.0, 0.0);
            let mut t = 0.0;
            while t < 2.0 {
                glide(&mut vel, look, -0.3, &mut boost, &mut acc, dt);
                t += dt;
            }
            vel
        };
        assert!((run(1.0 / 30.0) - run(1.0 / 240.0)).length() < 1.0);
    }

    #[test]
    fn crashes_hurt_only_when_fast() {
        assert_eq!(crash_damage(6.0, 0.0), 0.0);
        assert!(crash_damage(30.0, 0.0) > 10.0);
    }
}
