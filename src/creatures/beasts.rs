//! The Scorchlands' biomes' own creatures (see wilds.rs for the biomes):
//!
//! - **Tuskers** roam the crimson forests (and the Snouts' pens in the
//!   bastions): big, bristly and cross. One that sees you charges and tosses
//!   you into the air. Feed two Crimson Fungus and they'll breed, but they
//!   can't abide Teal Fungus: put some down, or hold some, and they back off.
//! - **Sporelings** potter about the teal forests minding their own
//!   business. Hit one and it puffs a cloud of spores at you (you're slow
//!   and weak for a bit) and scurries off. They drop Teal Fungus.
//! - **Magma Bloops** bounce about the basalt deltas: Bloops, but hot. They
//!   jump higher, hurt more, split like Bloops, and the smallest leave
//!   Magma Cream (four make a Magma Block; it brews Fire Resistance too).
//! - **Wisps** drift over the soul sand valleys: blue flames that come
//!   at you and set you alight. Water does for them. They leave Soul Embers,
//!   which make Soul Lanterns.
//! - **Snout Brutes** guard the bastions (see bastion.rs): no gold will
//!   buy them off.
//!
//! All of it runs where the world lives; joined players see the mobs in the
//! snapshots like any other.

use crate::block::*;
use crate::entity::{Mob, MobEvent, MobKind};
use crate::noise::Rng;
use crate::world::World;
use macroquad::math::{IVec3, Vec3};

/// How close Teal Fungus has to be to put a Tusker off.
pub const FUNGUS_FEAR: i32 = 6;
/// How long a Sporeling's spores slow and weaken you.
pub const SPORE_SECS: f32 = 8.0;

/// The nearest Teal Fungus (placed) within reach of `at`, if any.
pub fn teal_fungus_near(world: &World, at: Vec3) -> Option<Vec3> {
    let c = at.floor().as_ivec3();
    let mut best: Option<(i32, IVec3)> = None;
    for dy in -2..=2 {
        for dz in -FUNGUS_FEAR..=FUNGUS_FEAR {
            for dx in -FUNGUS_FEAR..=FUNGUS_FEAR {
                let p = c + IVec3::new(dx, dy, dz);
                if world.get_v(p) == TEAL_FUNGUS {
                    let d = dx * dx + dz * dz + dy * dy;
                    if best.is_none_or(|b| d < b.0) {
                        best = Some((d, p));
                    }
                }
            }
        }
    }
    best.map(|(_, p)| p.as_vec3() + Vec3::splat(0.5))
}

/// One step of thinking for the beasts (`player`: whoever it's after).
#[allow(clippy::too_many_arguments)]
pub fn update(m: &mut Mob, dt: f32, world: &World, player: Vec3, visible: bool, rng: &mut Rng, want: &mut Option<(f32, f32)>, fly_vy: &mut Option<f32>, may_wander: &mut bool, ev: &mut Vec<MobEvent>) {
    let to_player = player - m.body.pos;
    let dist = to_player.length();
    let flat = Vec3::new(to_player.x, 0.0, to_player.z);
    let face = flat.x.atan2(-flat.z);
    let toward = |m: &Mob, g: Vec3| {
        let d = g - m.body.pos;
        (d.x.atan2(-d.z), Vec3::new(d.x, 0.0, d.z).length())
    };
    match m.kind {
        MobKind::Tusker => {
            // Teal Fungus on the ground (or in someone's hand, see `beasts_tick`): back right off.
            m.warp_cd = (m.warp_cd - dt).max(0.0);
            if rng.chance(dt * 2.0)
                && let Some(f) = teal_fungus_near(world, m.body.pos)
            {
                m.home = Some(f);
                m.warp_cd = 3.0;
            }
            if m.warp_cd > 0.0
                && let Some(f) = m.home
            {
                let (yaw, _) = toward(m, f);
                *want = Some((yaw + std::f32::consts::PI, 3.0));
                return;
            }
            // Babies, and grown-ups in love, are busy with their own.
            if m.baby > 0.0 || m.love > 0.0 {
                if let Some(g) = m.goal {
                    let (yaw, d) = toward(m, g);
                    if d > 1.4 {
                        *want = Some((yaw, 2.2));
                    } else {
                        *may_wander = false;
                    }
                }
                return;
            }
            if visible && dist < 16.0 {
                *want = Some((face, 3.4));
                if flat.length() < m.body.half + 1.0 && to_player.y.abs() < 2.0 && m.attack_cd <= 0.0 {
                    ev.push(MobEvent::Toss(6.0, "was tossed by a Tusker"));
                    m.attack_cd = 1.6;
                }
            } else if let Some(g) = m.goal {
                let (yaw, d) = toward(m, g);
                if d > 1.6 {
                    *want = Some((yaw, 2.0));
                }
            }
        }
        MobKind::Sporeling => {
            // Just hit: puff spores at whoever's close.
            if m.temper == 1 {
                m.temper = 0;
                ev.push(MobEvent::Spores(m.body.pos + Vec3::Y * 0.6));
                if visible && dist < 5.0 {
                    ev.push(MobEvent::Afflict(crate::potions::Potion::Slowness, SPORE_SECS));
                    ev.push(MobEvent::Afflict(crate::potions::Potion::Weakness, SPORE_SECS));
                }
            }
            if m.flee > 0.0 {
                *want = Some((face + std::f32::consts::PI, 3.6));
            } else if rng.chance(dt * 0.4) {
                ev.push(MobEvent::Spores(m.body.pos + Vec3::Y * 0.7));
            }
        }
        MobKind::Wisp => {
            // Drift at whoever it's after, low and quick; otherwise hang about over the ground.
            let bob = (m.wander_t * 2.0 + m.id as f32).sin() * 0.3;
            if visible && dist < 20.0 {
                *may_wander = false;
                *want = Some((face, 2.6));
                *fly_vy = Some(((player.y + 0.9 + bob - m.body.pos.y) * 2.0).clamp(-2.5, 2.5));
                if dist < m.body.half + 0.9 && m.attack_cd <= 0.0 {
                    ev.push(MobEvent::HurtPlayer(2.0, "was singed by a Wisp"));
                    ev.push(MobEvent::Ignite(4.0));
                    m.attack_cd = 1.2;
                }
            } else {
                let ground = crate::entity::ground_below(world, m.body.pos, 12);
                *fly_vy = Some(((ground + 1.8 + bob - m.body.pos.y) * 1.5).clamp(-1.5, 1.5));
            }
            m.wander_t += dt;
            // Water puts it out.
            if m.body.in_water {
                m.health -= dt * 20.0;
            }
            if rng.chance(dt * 6.0) {
                ev.push(MobEvent::Spores(m.body.pos + Vec3::Y * 0.3));
            }
        }
        MobKind::SnoutBrute => {
            if visible && dist < 24.0 {
                *want = Some((face, 3.3));
                if flat.length() < 1.6 && to_player.y.abs() < 1.8 && m.attack_cd <= 0.0 {
                    ev.push(MobEvent::HurtPlayer(9.0, "was cleaved by a Snout Brute"));
                    m.attack_cd = 1.1;
                }
            } else if let Some(h) = m.home {
                // Back to its post.
                let (yaw, d) = toward(m, h);
                if d > 6.0 {
                    *want = Some((yaw, 2.2));
                }
            }
        }
        _ => {}
    }
}

impl crate::game::Game {
    /// Tuskers back away from anyone holding Teal Fungus (where the world lives).
    pub fn beasts_tick(&mut self) {
        if self.is_client() {
            return;
        }
        let fungus: Vec<Vec3> = self.player_spots().into_iter().filter(|p| p.2 == TEAL_FUNGUS).map(|p| p.1).collect();
        if fungus.is_empty() {
            return;
        }
        for m in self.mobs.iter_mut().filter(|m| m.kind == MobKind::Tusker) {
            if let Some(&at) = fungus.iter().find(|f| f.distance(m.body.pos) < FUNGUS_FEAR as f32 + 2.0) {
                m.home = Some(at);
                m.warp_cd = m.warp_cd.max(1.0);
            }
        }
    }

    /// Spores (or a Wisp's flicker) drifting up from `at`.
    pub fn spores(&mut self, at: Vec3, kind: MobKind) {
        if self.dedicated {
            return;
        }
        let tile = if kind == MobKind::Wisp { crate::texture::T_SOUL_FIRE } else { crate::texture::T_TEAL_NYLIUM_TOP };
        let n = if kind == MobKind::Sporeling { 10 } else { 1 };
        for _ in 0..n {
            let r = &mut self.rng;
            let vel = Vec3::new(r.range(-0.8, 0.8), r.range(0.3, 1.2), r.range(-0.8, 0.8));
            self.particles.push(crate::entity::Particle { pos: at, vel, life: r.range(0.6, 1.4), tile, uv: [r.range(0.3, 0.6), r.range(0.3, 0.6)], size: 0.06, gravity: 0.0 });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tuskers_breed_on_crimson_fungus_and_shun_teal() {
        let mut g = crate::game::tests::arena(181);
        let me = crate::players::record_key(&g.player_name);
        let at = g.player.body.pos + Vec3::new(3.0, 0.0, 0.0);
        let a = g.alloc_mob(MobKind::Tusker, at);
        let b = g.alloc_mob(MobKind::Tusker, at + Vec3::new(0.8, 0.0, 0.0));
        assert_eq!(g.interact_mob(&me, g.player.body.pos, a, TEAL_FUNGUS), crate::animals::Interaction::Nothing);
        for id in [a, b] {
            assert_eq!(g.interact_mob(&me, g.player.body.pos, id, CRIMSON_FUNGUS), crate::animals::Interaction::Ate);
        }
        for _ in 0..100 {
            g.animals_tick(0.05);
            for m in g.mobs.iter_mut() {
                m.body.pos = at;
            }
        }
        assert!(g.mobs.iter().any(|m| m.kind == MobKind::Tusker && m.baby > 0.0), "a baby Tusker");
        // Teal Fungus close by: it backs off.
        let mut rng = Rng::new(3);
        let t = g.mobs.iter_mut().find(|m| m.id == a).unwrap();
        t.love = 0.0;
        t.goal = None;
        let pos = t.body.pos;
        g.world.set_v((pos + Vec3::new(2.0, 0.0, 0.0)).floor().as_ivec3(), TEAL_FUNGUS);
        let t = g.mobs.iter_mut().find(|m| m.id == a).unwrap();
        let (mut want, mut fly, mut wander, mut ev) = (None, None, true, Vec::new());
        for _ in 0..40 {
            update(t, 0.05, &g.world, pos + Vec3::new(4.0, 0.0, 0.0), true, &mut rng, &mut want, &mut fly, &mut wander, &mut ev);
        }
        let (yaw, _) = want.expect("going somewhere");
        assert!(yaw.sin() < 0.0, "away from the fungus (east), not at the player");
    }

    #[test]
    fn a_hit_sporeling_puffs_spores_and_runs() {
        let mut rng = Rng::new(4);
        let g = crate::game::tests::arena(182);
        let mut m = Mob::new(MobKind::Sporeling, Vec3::new(0.5, 50.0, 0.5), &mut rng);
        m.damage(1.0, Vec3::new(2.0, 50.0, 0.5));
        let (mut want, mut fly, mut wander, mut ev) = (None, None, true, Vec::new());
        update(&mut m, 0.05, &g.world, Vec3::new(2.0, 50.0, 0.5), true, &mut rng, &mut want, &mut fly, &mut wander, &mut ev);
        assert!(ev.iter().any(|e| matches!(e, MobEvent::Afflict(crate::potions::Potion::Slowness, _))));
        assert!(want.is_some_and(|(y, _)| y.sin() < 0.0), "running west, away");
    }
}
