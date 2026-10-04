//! The Pale Garden's Creakings.
//!
//! Now and then a pale oak grows with a **Creaking Heart** in its trunk. At
//! night the heart wakes and calls up a **Creaking** nearby: tall, made of
//! bark, and only able to move while nobody is looking at it. It can't be
//! hurt (hits just make it twitch); break its heart and it crumbles. At dawn
//! the heart sleeps again and its Creaking goes back into the ground.
//!
//! Hitting a Creaking does it no harm, but its heart oozes a little **Resin**
//! (a clump drops by the heart now and then). Nine make a block; bake them
//! into Resin Bricks.
//!
//! Hearts and their Creakings live where the world lives; joined players
//! looking at a Creaking freeze it too (the host knows where they look).

use crate::block::*;
use crate::entity::MobKind;
use crate::game::Game;
use crate::sound::Sfx;
use macroquad::math::{IVec3, Vec3};

/// How near someone must be for a heart to bother waking.
pub const WAKE_RANGE: f32 = 40.0;
/// A Creaking wanders no further than this from its heart.
pub const LEASH: f32 = 32.0;
/// How far a watching player can freeze one from.
pub const WATCH_RANGE: f32 = 48.0;

pub fn is_heart(id: Id) -> bool {
    id == CREAKING_HEART || id == CREAKING_HEART_AWAKE
}

/// Is someone at `eye` looking along `dir` at something at `at` (roughly: within
/// about 30 degrees of the middle of the screen)?
pub fn watching(eye: Vec3, dir: Vec3, at: Vec3) -> bool {
    let to = at - eye;
    let d = to.length();
    d > 0.1 && d < WATCH_RANGE && to.dot(dir) / d > 0.86
}

impl Game {
    /// Someone hit the Creaking `i`: its heart may ooze a resin clump (where the world lives).
    pub fn creaking_hit(&mut self, i: usize) {
        if self.is_client() || self.mobs[i].kind != MobKind::Creaking {
            return;
        }
        let Some(home) = self.mobs[i].home else { return };
        if self.rng.chance(0.5) {
            self.pop_drop(home + Vec3::Y * 0.6, RESIN_CLUMP, 1);
            self.sfx(Sfx::Break(crate::sound::Mat::Wood), Some(home));
            if !self.dedicated && self.player.body.pos.distance(home) < 24.0 {
                self.advance("resin_up");
            }
        }
    }

    /// Hearts near players wake at night, sleep by day; Creakings freeze when watched.
    pub fn creaking_tick(&mut self, dt: f32) {
        if self.is_client() {
            return;
        }
        self.creak_timer -= dt;
        if self.creak_timer > 0.0 {
            return;
        }
        self.creak_timer = 0.25;
        // Who's looking where (eye, direction).
        let mut eyes: Vec<(Vec3, Vec3)> = self
            .peers
            .values()
            .filter(|p| p.alive())
            .map(|p| (p.target + Vec3::Y * crate::player::EYE, Vec3::new(p.yaw.sin() * p.pitch.cos(), p.pitch.sin(), -p.yaw.cos() * p.pitch.cos())))
            .collect();
        if !self.dedicated && self.dead.is_none() {
            eyes.push((self.player.eye(), self.player.look_dir()));
        }
        // Frozen while watched (and in view: not through a wall).
        for i in 0..self.mobs.len() {
            if self.mobs[i].kind != MobKind::Creaking {
                continue;
            }
            let m = &self.mobs[i];
            let mid = m.body.pos + Vec3::Y * m.body.height * 0.6;
            let seen = eyes.iter().any(|&(eye, dir)| watching(eye, dir, mid) && self.world.raycast(eye, (mid - eye).normalize_or_zero(), mid.distance(eye) - 0.6).is_none());
            self.mobs[i].sitting = seen;
        }
        let night = self.is_night() && self.rules.difficulty.monsters();
        let players: Vec<Vec3> = eyes.iter().map(|e| e.0).collect();
        let hearts: Vec<IVec3> = self.world.cages.iter().copied().filter(|p| is_heart(self.world.get_v(*p))).collect();
        for p in hearts {
            let centre = p.as_vec3() + Vec3::splat(0.5);
            if !players.iter().any(|e| e.distance(centre) < WAKE_RANGE) {
                continue;
            }
            let id = self.world.get_v(p);
            let mine = self.mobs.iter().position(|m| m.kind == MobKind::Creaking && m.home == Some(centre));
            if night {
                if id != CREAKING_HEART_AWAKE {
                    self.world.set_v(p, CREAKING_HEART_AWAKE);
                }
                if mine.is_none()
                    && let Some(at) = crate::entity::warp_spot(&self.world, centre, 6.0, &mut self.rng)
                {
                    let mob = self.alloc_mob(MobKind::Creaking, at);
                    if let Some(m) = self.mobs.iter_mut().find(|m| m.id == mob) {
                        m.home = Some(centre);
                        m.goal = Some(centre);
                        m.persistent = true;
                    }
                    self.smoke(at + Vec3::Y, 10, 0.5);
                    self.sfx(Sfx::Groan, Some(at));
                }
            } else {
                if id != CREAKING_HEART {
                    self.world.set_v(p, CREAKING_HEART);
                }
                // Dawn: back into the ground (no drops).
                if let Some(i) = mine {
                    let at = self.mobs[i].body.pos;
                    self.mobs[i].health = -100.0;
                    self.smoke(at + Vec3::Y, 12, 0.5);
                }
            }
        }
        // A Creaking whose heart is gone crumbles (that counts as beating it).
        for m in self.mobs.iter_mut().filter(|m| m.kind == MobKind::Creaking && m.health > 0.0) {
            let Some(home) = m.home else {
                m.health = -100.0;
                continue;
            };
            let heart = home.floor().as_ivec3();
            if self.world.is_loaded(heart.x, heart.z) && !is_heart(self.world.get_v(heart)) {
                m.health = -1.0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::tests::arena;

    #[test]
    fn hitting_a_creaking_knocks_resin_out_of_its_heart() {
        let mut g = arena(63);
        let heart = Vec3::new(3.5, 50.5, 3.5);
        let id = g.alloc_mob(MobKind::Creaking, Vec3::new(0.5, 50.0, 0.5));
        let i = g.mobs.iter().position(|m| m.id == id).unwrap();
        g.mobs[i].home = Some(heart);
        for _ in 0..20 {
            g.creaking_hit(i);
        }
        let resin: u32 = g.drops.iter().filter(|d| d.item == RESIN_CLUMP).map(|d| d.n as u32).sum();
        assert!(resin > 0, "resin by the heart");
        assert_eq!(crate::containers::smelt(RESIN_CLUMP), Some(RESIN_BRICK));
    }

    #[test]
    fn creakings_wake_at_night_freeze_when_watched_and_die_with_their_heart() {
        let mut g = arena(61);
        let base = g.player.body.pos.floor().as_ivec3();
        let heart = base + IVec3::new(6, 0, 0);
        g.world.set_v(heart, CREAKING_HEART);
        assert!(g.world.cages.contains(&heart), "hearts are kept track of");
        g.time = 0.8;
        assert!(g.is_night());
        g.creaking_tick(0.3);
        assert_eq!(g.world.get_v(heart), CREAKING_HEART_AWAKE);
        let i = g.mobs.iter().position(|m| m.kind == MobKind::Creaking).expect("a Creaking came");
        // It can't be hurt.
        let hp = g.mobs[i].health;
        g.mobs[i].damage(50.0, g.player.body.pos);
        assert_eq!(g.mobs[i].health, hp);
        // Look straight at it: it freezes; look away: it doesn't.
        g.mobs[i].body.pos = g.player.body.pos + Vec3::new(0.0, 0.0, -5.0);
        g.player.yaw = 0.0;
        g.player.pitch = 0.0;
        g.creaking_tick(0.3);
        assert!(g.mobs[i].sitting, "watched");
        g.player.yaw = std::f32::consts::PI;
        g.creaking_tick(0.3);
        assert!(!g.mobs[i].sitting, "not watched");
        // Break the heart: it crumbles.
        g.world.set_v(heart, AIR);
        g.creaking_tick(0.3);
        assert!(g.mobs[i].health <= 0.0);
        // Hearts sleep by day.
        g.world.set_v(heart, CREAKING_HEART_AWAKE);
        g.time = 0.3;
        g.creaking_tick(0.3);
        assert_eq!(g.world.get_v(heart), CREAKING_HEART);
    }
}
