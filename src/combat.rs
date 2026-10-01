//! Combat polish: attacks that charge up, sweeping sword blows, shields,
//! and armour that steadies you against being knocked about.
//!
//! - **Charge.** After a swing, a weapon takes a moment to be ready again
//!   (swords 0.6 s, pickaxes 0.8 s, hands 0.3 s); a bar under the crosshair
//!   fills up. Hitting early does less damage (a fifth, rising with the
//!   square of the charge), and only a fully charged blow counts as a
//!   falling critical or knocks back hard when sprinting.
//! - **Sweep.** A fully charged sword blow on the ground also catches mobs
//!   right next to the target for a point of damage.
//! - **Shields.** Hold right-click with a shield to block: you move slowly,
//!   and hits from in front (mobs, arrows, half of an explosion) are stopped;
//!   the shield takes the damage as wear instead.
//! - **Steadiness.** Every armour point takes 2.5% off knockback.

use crate::block::*;
use crate::game::Game;
use crate::sound::Sfx;
use macroquad::math::Vec3;

/// Seconds for `held` to be fully ready after a swing.
pub fn attack_speed(held: Id) -> f32 {
    if is_sword(held) {
        0.6
    } else if crate::tools::axe_tier(held).is_some() {
        // Axes hit hard but slowly.
        1.0
    } else if pick_tier(held) > 0 || crate::tools::shovel_tier(held).is_some() {
        0.8
    } else if held == HOE {
        0.5
    } else {
        0.3
    }
}

/// Damage multiplier for a blow at this charge (0..1).
pub fn charge_scale(charge: f32) -> f32 {
    let c = charge.clamp(0.0, 1.0);
    0.2 + 0.8 * c * c
}

/// How much of a hit a shield stops, from where it came (None: nowhere in particular).
pub fn shield_blocks(look: Vec3, me: Vec3, from: Option<Vec3>) -> bool {
    let Some(from) = from else { return false };
    let to = from - me;
    let flat_to = Vec3::new(to.x, 0.0, to.z).normalize_or_zero();
    let flat_look = Vec3::new(look.x, 0.0, look.z).normalize_or_zero();
    flat_to.dot(flat_look) > 0.25
}

impl Game {
    /// 0..1: how ready the held weapon is.
    pub fn attack_charge(&self) -> f32 {
        (self.since_attack / attack_speed(self.inv.held())).min(1.0)
    }

    /// Hurt the local player by something at `from`: a raised shield in the way stops it.
    pub fn hurt_player_from(&mut self, amount: f32, cause: &str, from: Option<Vec3>, explosion: bool) {
        let me = self.player.body.pos + Vec3::Y * 0.9;
        if self.blocking && shield_blocks(self.player.look_dir(), me, from) {
            let stopped = if explosion { amount * 0.5 } else { amount };
            self.use_tool(stopped.ceil().max(1.0) as u16);
            self.sfx(Sfx::Thunk, None);
            self.advance("not_today");
            if amount - stopped > 0.0 {
                self.hurt_player_armored(amount - stopped, cause);
            }
            return;
        }
        self.hurt_player_armored(amount, cause);
    }

    /// Knockback after armour's steadiness.
    pub fn steadied(&self, push: Vec3) -> Vec3 {
        push * (1.0 - self.inv.armor_points() as f32 * 0.025).max(0.3)
    }

    /// A full-strength sword blow catches mobs next to the one hit (where the world lives).
    pub fn sweep(&mut self, target: usize, from: Vec3) {
        let center = self.mobs[target].body.pos;
        let id = self.mobs[target].id;
        let mut hit = false;
        for m in self.mobs.iter_mut() {
            if m.id != id && m.owner.is_none() && m.body.pos.distance(center) < 1.6 {
                m.damage(1.0, from);
                m.last_attacker = self.my_id;
                hit = true;
            }
        }
        if hit {
            self.smoke(center + Vec3::Y * 0.8, 6, 0.6);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn charge_and_blocking() {
        assert!(attack_speed(SWORD_IRON) < attack_speed(PICK_IRON));
        assert_eq!(charge_scale(1.0), 1.0);
        assert!((charge_scale(0.0) - 0.2).abs() < 1e-6);
        assert!(charge_scale(0.5) < 0.5);
        let look = Vec3::new(0.0, 0.0, -1.0);
        assert!(shield_blocks(look, Vec3::ZERO, Some(Vec3::new(0.3, 0.0, -3.0))), "in front");
        assert!(!shield_blocks(look, Vec3::ZERO, Some(Vec3::new(0.0, 0.0, 3.0))), "behind");
        assert!(!shield_blocks(look, Vec3::ZERO, None));
    }
}
