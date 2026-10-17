//! Sulkers: the Hollow Cities' guards (see cities.rs).
//!
//! A Sulker sits in a purple shell stuck to a wall or a floor, and doesn't
//! move. Mostly it keeps its lid shut (and a shut shell shrugs off most of a
//! hit); when someone comes near it opens, peeks out, and shoots a slow,
//! homing bullet that sets them **Floating** for a while (and then, of
//! course, falling). Beaten, it sometimes leaves its **Sulker Shell**; two of
//! those and a chest make a Hollow Box.
//!
//! How open it is rides in `fuse` (seconds left before it shuts again), which
//! is sent to joined players, so everyone sees the lid lift.

use crate::block::{ProjectileAppearance, ProjectileEffect, ProjectileModel};
use crate::entity::{Arrow, Mob, MobEvent, MobKind};
use crate::game::Game;
use crate::noise::Rng;
use crate::potions::Potion;
use crate::sound::Sfx;
use macroquad::math::Vec3;

pub const HEALTH: f32 = 30.0;
/// How near someone must be for it to start shooting.
pub const RANGE: f32 = 16.0;
/// Seconds it stays open after a shot.
pub const OPEN_SECS: f32 = 2.5;
/// A shut shell lets this much of a hit through.
pub const SHUT_SOAK: f32 = 0.2;
pub const BULLET_SPEED: f32 = 6.0;
pub const BULLET_DAMAGE: f32 = 4.0;
/// How long a bullet's Floating lasts.
pub const FLOAT_SECS: f32 = 6.0;
/// How often a Sulker leaves its shell.
pub const SHELL_CHANCE: f32 = 0.5;

/// Shut tight (and so mostly armour)?
pub fn shut(m: &Mob) -> bool {
    m.kind == MobKind::Sulker && m.fuse <= 0.0
}

/// A Sulker's turn: stay put, peek now and then, and shoot whoever's near.
pub fn update(m: &mut Mob, dt: f32, target: Vec3, visible: bool, rng: &mut Rng, ev: &mut Vec<MobEvent>) {
    m.attack_cd = (m.attack_cd - dt).max(0.0);
    m.fuse = (m.fuse - dt).max(0.0);
    // It's stuck where it is.
    m.body.vel.x = 0.0;
    m.body.vel.z = 0.0;
    m.knock = Vec3::ZERO;
    m.anim += dt;
    let eye = m.body.pos + Vec3::Y * 0.6;
    let to = target + Vec3::Y * 0.9 - eye;
    if visible && to.length() < RANGE {
        m.yaw = to.x.atan2(-to.z);
        if m.attack_cd <= 0.0 {
            let vel = to.normalize_or_zero() * BULLET_SPEED;
            ev.push(MobEvent::SulkerShot(eye + vel.normalize_or_zero() * 0.6, vel));
            m.fuse = OPEN_SECS;
            m.attack_cd = rng.range(2.0, 4.0);
        }
    } else if m.fuse <= 0.0 && rng.chance(dt * 0.08) {
        // A peek out, for no reason in particular.
        m.fuse = rng.range(0.8, 2.0);
    }
}

impl Game {
    /// A Sulker's bullet: slow, homing, and it sets you Floating.
    pub fn fire_sulker_bullet(&mut self, from: Vec3, vel: Vec3) {
        let mut a = Arrow::new(from, vel, None, BULLET_DAMAGE);
        a.homing = 120.0;
        a.effect = Some(ProjectileEffect { kind: Potion::Floating, duration: FLOAT_SECS, amplifier: 0 });
        a.appearance = ProjectileAppearance { model: ProjectileModel::Billboard, tile: Some(crate::texture::T_SULKER_BULLET), scale: 0.35 };
        a.modded = true;
        a.life = 8.0;
        self.arrows.push(a);
        self.sfx(Sfx::Hiss, Some(from));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::noise::Rng;

    #[test]
    fn it_opens_to_shoot_and_shuts_again() {
        let mut rng = Rng::new(5);
        let mut m = Mob::new(MobKind::Sulker, Vec3::new(0.5, 60.0, 0.5), &mut rng);
        let mut ev = Vec::new();
        update(&mut m, 0.05, Vec3::new(30.5, 60.0, 0.5), true, &mut rng, &mut ev);
        assert!(ev.is_empty(), "too far to bother");
        update(&mut m, 0.05, Vec3::new(6.5, 60.0, 0.5), true, &mut rng, &mut ev);
        assert!(matches!(ev[..], [MobEvent::SulkerShot(..)]));
        assert!(!shut(&m));
        // Hits on an open one land in full; shut, mostly not.
        m.damage(5.0, Vec3::ZERO);
        assert!((m.health - (HEALTH - 5.0)).abs() < 1e-3);
        for _ in 0..200 {
            update(&mut m, 0.05, Vec3::new(30.5, 60.0, 0.5), false, &mut rng, &mut ev);
            if shut(&m) {
                break;
            }
        }
        assert!(shut(&m));
        m.hurt = 0.0;
        m.damage(5.0, Vec3::ZERO);
        assert!((m.health - (HEALTH - 5.0 - 5.0 * SHUT_SOAK)).abs() < 1e-3);
    }

    #[test]
    fn hollow_boxes_come_from_shells() {
        let ways: Vec<_> = crate::block::recipes().iter().filter(|r| r.output.0 == crate::block::HOLLOW_BOX).collect();
        assert_eq!(ways.len(), 1);
        assert!(ways[0].inputs.contains(&(crate::block::SULKER_SHELL, 2)));
    }

    #[test]
    fn a_bullet_sets_you_floating() {
        let mut g = crate::game::tests::arena(911);
        let me = g.player.body.pos;
        g.fire_sulker_bullet(me + Vec3::new(3.0, 1.0, 0.0), Vec3::new(-BULLET_SPEED, 0.0, 0.0));
        for _ in 0..60 {
            g.update_arrows(0.05);
            if g.has_effect(Potion::Floating) {
                break;
            }
        }
        assert!(g.has_effect(Potion::Floating));
        // And it lifts you.
        let y = g.player.body.pos.y;
        g.effects_tick(0.05);
        let still = crate::player::Input::default();
        for _ in 0..20 {
            g.player.update(0.05, &still, &g.world, false);
        }
        assert!(g.player.body.pos.y > y + 0.3, "floating up");
    }
}
