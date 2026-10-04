//! The Wilter: the second boss, built rather than found.
//!
//! - **Its makings.** **Sorrow Sand** lies in patches on the Scorchlands'
//!   shores (it slows you down), and **Charred Rattlers** walk the fortresses:
//!   tall, sooty and fond of their blades, which leave you **Wilting** (health
//!   drains, all the way down). Now and then one drops its **Charred Skull**.
//! - **Building it.** Four Sorrow Sand in a T (three across, one under the
//!   middle) with a Charred Skull on each of the top three. The last skull
//!   wakes it: for a few seconds it gathers itself (nothing hurts it), then
//!   it bursts out with a blast.
//! - **Fighting it.** It flies, keeps its distance, and throws wilting skulls
//!   that burst on landing. Below half health it's angrier: it throws faster,
//!   three at a time, and dives at whoever it's after. It heals slowly.
//! - **Its star.** Beaten, it leaves the **Wilter Star**. Use it on a beacon
//!   with its full three-layer pyramid and the beacon becomes a **Starred
//!   Beacon**: its effect at level II, Regeneration besides, and further reach.
//!
//! Like everything that moves, the Wilter lives where the world lives;
//! joined players see it in the mob snapshots, boss bar included.

use crate::block::*;
use crate::entity::{Mob, MobEvent, MobKind};
use crate::game::Game;
use crate::noise::Rng;
use crate::sound::Sfx;
use crate::world::World;
use macroquad::math::{IVec3, Vec3};

/// Its health, and how long it takes to wake.
pub const HEALTH: f32 = 300.0;
pub const WAKE_SECS: f32 = 8.0;
/// The chance a Charred Rattler drops its skull.
pub const SKULL_CHANCE: f32 = 0.08;
/// How close it likes to be, and how high above its target it hovers.
const KEEP_AWAY: f32 = 10.0;
const HOVER: f32 = 5.0;
/// Seconds between skulls (calm, then angry), and how fast they fly.
const SHOT_EVERY: [f32; 2] = [1.8, 0.9];
const SKULL_SPEED: f32 = 14.0;
/// Damage from a skull's burst, and from a dive.
pub const SKULL_DAMAGE: f32 = 5.0;
const DIVE_DAMAGE: f32 = 8.0;
/// Health a second it heals.
const REGEN: f32 = 0.5;

/// The T a Wilter is built from, for a skull just placed at `skull`: the
/// three skull cells and four sand cells, if they're all there.
pub fn frame(world: &World, skull: IVec3) -> Option<Vec<IVec3>> {
    for axis in [IVec3::X, IVec3::Z] {
        for k in -1..=1 {
            let mid = skull - axis * k;
            let skulls = [mid - axis, mid, mid + axis];
            let sand = [mid - axis - IVec3::Y, mid - IVec3::Y, mid + axis - IVec3::Y, mid - IVec3::Y * 2];
            if skulls.iter().all(|&p| world.get_v(p) == CHARRED_SKULL) && sand.iter().all(|&p| world.get_v(p) == SORROW_SAND) {
                return Some(skulls.into_iter().chain(sand).collect());
            }
        }
    }
    None
}

/// Is this Wilter still waking (and so not to be hurt)?
pub fn waking(m: &Mob) -> bool {
    m.kind == MobKind::Wilter && m.fuse > 0.0
}

/// Angry, past half health.
pub fn angry(m: &Mob) -> bool {
    m.health < HEALTH * 0.5
}

/// The Wilter's turn: wake, then hover, throw skulls, and (angry) dive.
pub fn update(m: &mut Mob, dt: f32, world: &World, target: Vec3, visible: bool, rng: &mut Rng, ev: &mut Vec<MobEvent>) {
    m.attack_cd = (m.attack_cd - dt).max(0.0);
    m.warp_cd = (m.warp_cd - dt).max(0.0);
    m.anim += dt * 4.0;
    if m.fuse > 0.0 {
        // Gathering itself: health fills, and then it bursts out.
        m.fuse -= dt;
        m.health = HEALTH * (1.0 - (m.fuse / WAKE_SECS).max(0.0)).max(0.05);
        m.body.vel = Vec3::ZERO;
        if m.fuse <= 0.0 {
            m.fuse = 0.0;
            m.health = HEALTH;
            ev.push(MobEvent::WilterWakes(m.body.pos + Vec3::Y * 1.5));
        }
        return;
    }
    m.health = (m.health + REGEN * dt).min(HEALTH);
    let mad = angry(m);
    let diving = m.flee > 0.0;
    // Where it wants to be: above and a little off from its target, or on top of it in a dive.
    let goal = if !visible {
        m.body.pos + Vec3::new((m.anim * 0.3).cos(), 0.0, (m.anim * 0.3).sin()) * 2.0
    } else if diving {
        target + Vec3::Y * 0.5
    } else {
        let off = Vec3::new(m.body.pos.x - target.x, 0.0, m.body.pos.z - target.z).normalize_or(Vec3::X);
        // (Circling slowly round.)
        let a = 0.4 * dt;
        let off = Vec3::new(off.x * a.cos() - off.z * a.sin(), 0.0, off.x * a.sin() + off.z * a.cos());
        target + off * KEEP_AWAY + Vec3::Y * HOVER
    };
    let speed = if diving { 14.0 } else if mad { 7.0 } else { 5.0 };
    let want = (goal - m.body.pos).normalize_or_zero() * speed * ((goal - m.body.pos).length() / 2.0).min(1.0);
    m.body.vel += (want - m.body.vel) * (dt * 3.0).min(1.0);
    let next = m.body.pos + m.body.vel * dt;
    // It doesn't go through the ground: up over it instead.
    let feet = next.floor().as_ivec3();
    m.body.pos = if is_solid(world.get_v(feet)) || is_solid(world.get_v(feet + IVec3::Y)) { m.body.pos + Vec3::Y * 4.0 * dt } else { next };
    if visible {
        let to = target - m.body.pos;
        m.yaw = to.x.atan2(-to.z);
    }
    if !visible {
        m.flee = 0.0;
        return;
    }
    // A dive lands on whoever it's after, then it backs off.
    if diving {
        m.flee -= dt;
        if m.body.pos.distance(target + Vec3::Y) < 2.0 {
            ev.push(MobEvent::HurtPlayer(DIVE_DAMAGE, "was flattened by the Wilter"));
            ev.push(MobEvent::Afflict(crate::potions::Potion::Wilting, 10.0));
            m.flee = 0.0;
        }
        return;
    }
    if mad && m.warp_cd <= 0.0 {
        m.flee = 2.5;
        m.warp_cd = rng.range(7.0, 10.0);
        return;
    }
    if m.attack_cd <= 0.0 && m.body.pos.distance(target) < 36.0 {
        // From the middle head (and, angry, the two side ones too).
        let side = Vec3::new(m.yaw.cos(), 0.0, m.yaw.sin()) * 0.85;
        let middle = m.body.pos + Vec3::Y * 2.3;
        let heads: &[Vec3] = if mad { &[middle, middle + side - Vec3::Y * 0.1, middle - side - Vec3::Y * 0.1] } else { &[middle] };
        for &from in heads {
            let vel = (target + Vec3::Y - from).normalize_or_zero() * SKULL_SPEED;
            ev.push(MobEvent::WiltSkull(from + vel.normalize_or_zero() * 0.6, vel));
        }
        m.attack_cd = SHOT_EVERY[mad as usize] * rng.range(0.8, 1.2);
    }
}

impl Game {
    /// A Charred Skull just went on at `skull`: if it finishes the T, the
    /// Wilter wakes (where the world lives).
    pub fn try_build_wilter(&mut self, skull: IVec3, who: &str) -> bool {
        if self.is_client() || crate::hollow::in_hollow(skull.x as f32) {
            return false;
        }
        let Some(cells) = frame(&self.world, skull) else { return false };
        for &p in &cells {
            self.world.set_v(p, AIR);
        }
        // It rises from where the T's stem was.
        let stem = cells[6];
        let at = stem.as_vec3() + Vec3::new(0.5, 0.0, 0.5);
        let id = self.alloc_mob(MobKind::Wilter, at);
        if let Some(m) = self.mobs.iter_mut().find(|m| m.id == id) {
            m.persistent = true;
            m.fuse = WAKE_SECS;
            m.health = HEALTH * 0.05;
        }
        self.smoke(at + Vec3::Y, 16, 0.5);
        self.sfx(Sfx::Roar, Some(at));
        let t = "The Wilter stirs...";
        self.msg(t);
        self.system_message(None, t);
        self.advance_for(who, "wilter_built");
        true
    }

    /// The Wilter bursts out of its waking: a blast all round it.
    pub fn wilter_wakes(&mut self, at: Vec3) {
        self.explode(at, 4.0, "was caught in the Wilter's first breath");
    }

    /// One of the Wilter's skulls: it bursts where it lands, wilting whoever's near.
    pub fn throw_wilt_skull(&mut self, from: Vec3, vel: Vec3) {
        let mut a = crate::entity::Arrow::new(from, vel, None, SKULL_DAMAGE);
        a.blast = 1.4;
        // (A little homing keeps them flying straight, and on target.)
        a.homing = 25.0;
        a.effect = Some(ProjectileEffect { kind: crate::potions::Potion::Wilting, duration: 10.0, amplifier: 0 });
        a.appearance = ProjectileAppearance { model: ProjectileModel::Billboard, tile: Some(crate::texture::T_CHARRED_SKULL_FACE), scale: 0.6 };
        a.modded = true;
        a.life = 4.0;
        self.arrows.push(a);
        self.sfx(Sfx::Hiss, Some(from));
    }

    /// The Wilter is beaten.
    pub fn wilter_defeated(&mut self, at: Vec3) {
        self.sfx(Sfx::Fanfare, Some(at));
        let t = "The Wilter has been beaten! It left a star behind.";
        self.msg(t);
        self.system_message(None, t);
        if !self.dedicated && self.player.body.pos.distance(at) < 48.0 {
            self.advance("wilt_under_pressure");
        }
        let near: Vec<String> = self.peers.values().filter(|p| p.target.distance(at) < 48.0).map(|p| crate::players::record_key(&p.name)).collect();
        for who in near {
            self.advance_for(&who, "wilt_under_pressure");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use macroquad::math::ivec3;

    /// A T of sand with two skulls on it, along x, at `mid` (the middle of the top row).
    fn t_frame(g: &mut Game, mid: IVec3) {
        for x in -1..=1 {
            g.world.set_v(mid + ivec3(x, -1, 0), SORROW_SAND);
        }
        g.world.set_v(mid + ivec3(0, -2, 0), SORROW_SAND);
        g.world.set_v(mid + ivec3(-1, 0, 0), CHARRED_SKULL);
        g.world.set_v(mid, CHARRED_SKULL);
    }

    #[test]
    fn the_last_skull_wakes_the_wilter() {
        let mut g = crate::game::tests::arena(301);
        let mid = ivec3(4, 53, 4);
        t_frame(&mut g, mid);
        // Two skulls aren't enough.
        assert!(!g.try_build_wilter(mid, ""));
        // The third, at either end, is.
        g.world.set_v(mid + IVec3::X, CHARRED_SKULL);
        assert!(frame(&g.world, mid + IVec3::X).is_some());
        assert!(g.try_build_wilter(mid + IVec3::X, ""));
        assert_eq!(g.world.get_v(mid), AIR, "the T is used up");
        let i = g.mobs.iter().position(|m| m.kind == MobKind::Wilter).expect("a Wilter");
        assert!(waking(&g.mobs[i]) && g.mobs[i].is_boss());
        // Nothing hurts it while it wakes.
        let hp = g.mobs[i].health;
        g.mobs[i].damage(50.0, mid.as_vec3());
        assert!(g.mobs[i].health >= hp, "it shrugs it off");
    }

    #[test]
    fn a_wrong_shape_does_nothing() {
        let mut g = crate::game::tests::arena(302);
        let mid = ivec3(4, 53, 4);
        t_frame(&mut g, mid);
        // No stem.
        g.world.set_v(mid + ivec3(0, -2, 0), STONE);
        g.world.set_v(mid + IVec3::X, CHARRED_SKULL);
        assert!(frame(&g.world, mid).is_none());
        assert!(!g.try_build_wilter(mid + IVec3::X, ""));
    }

    #[test]
    fn it_wakes_flies_and_throws_wilting_skulls() {
        let mut rng = Rng::new(4);
        let world = crate::game::tests::arena(303).world;
        let mut m = Mob::new(MobKind::Wilter, Vec3::new(0.5, 60.0, 0.5), &mut rng);
        m.fuse = WAKE_SECS;
        let target = Vec3::new(8.5, 51.0, 0.5);
        let mut ev = Vec::new();
        for _ in 0..((WAKE_SECS + 0.5) / 0.1) as usize {
            update(&mut m, 0.1, &world, target, true, &mut rng, &mut ev);
        }
        assert!(ev.iter().any(|e| matches!(e, MobEvent::WilterWakes(_))), "it burst out");
        assert_eq!(m.health, HEALTH);
        ev.clear();
        for _ in 0..40 {
            update(&mut m, 0.1, &world, target, true, &mut rng, &mut ev);
        }
        assert!(ev.iter().any(|e| matches!(e, MobEvent::WiltSkull(..))), "it threw a skull");
        // Angry, it throws three at once.
        m.health = HEALTH * 0.3;
        m.warp_cd = 100.0;
        m.attack_cd = 0.0;
        ev.clear();
        update(&mut m, 0.05, &world, target, true, &mut rng, &mut ev);
        assert_eq!(ev.iter().filter(|e| matches!(e, MobEvent::WiltSkull(..))).count(), 3);
    }

    #[test]
    fn a_skull_wilts_whoever_it_lands_on() {
        let mut g = crate::game::tests::arena(304);
        g.player.body.pos = Vec3::new(0.5, 50.0, 0.5);
        let from = g.player.body.pos + Vec3::new(0.0, 4.0, 0.0);
        g.throw_wilt_skull(from, Vec3::new(0.0, -SKULL_SPEED, 0.0));
        for _ in 0..30 {
            g.update_arrows(0.05);
        }
        assert!(g.has_effect(crate::potions::Potion::Wilting));
        // Wilting keeps draining.
        let hp = g.player.health;
        for _ in 0..80 {
            g.effects_tick(0.05);
        }
        assert!(g.player.health < hp);
    }
}
