//! Clankers (legally distinct iron golems): each village's guard.
//!
//! One moves in when a village's square chest is first filled. It stays near
//! the square, goes after any monster that comes within 16 blocks and flattens
//! it (throwing it in the air for good measure), and otherwise stands about.
//! Hit one and it'll come after you until you get out of sight. Clankers are
//! kept with the world and drop iron. Everything here runs where the world lives.

use crate::entity::{Mob, MobKind};
use crate::game::Game;
use crate::sound::Sfx;
use macroquad::math::Vec3;

/// How far a Clanker wanders from its square, and how far it looks for trouble.
pub const HOME_RANGE: f32 = 14.0;
pub const GUARD_RANGE: f32 = 16.0;
/// A punch.
pub const PUNCH: f32 = 12.0;

impl Game {
    /// Where the world lives: a new village gets its Clanker.
    pub fn house_clankers(&mut self) {
        for at in std::mem::take(&mut self.world.new_clankers) {
            let mut m = Mob::new(MobKind::Clanker, at, &mut self.rng);
            m.id = self.next_mob_id;
            self.next_mob_id += 1;
            m.home = Some(at);
            m.persistent = true;
            self.mobs.push(m);
        }
    }

    /// Clankers pick fights with monsters and stay near home.
    pub fn clankers_tick(&mut self, _dt: f32) {
        if self.is_client() {
            return;
        }
        let mut punches = Vec::new();
        for i in 0..self.mobs.len() {
            if self.mobs[i].kind != MobKind::Clanker || self.mobs[i].health <= 0.0 {
                continue;
            }
            let pos = self.mobs[i].body.pos;
            let home = self.mobs[i].home.unwrap_or(pos);
            // The nearest monster near it (and not too far from home to chase).
            let prey = self
                .mobs
                .iter()
                .filter(|o| o.menacing() && o.health > 0.0 && o.body.pos.distance(pos) < GUARD_RANGE && o.body.pos.distance(home) < GUARD_RANGE + HOME_RANGE)
                .min_by(|a, b| a.body.pos.distance(pos).total_cmp(&b.body.pos.distance(pos)))
                .map(|o| (o.id, o.body.pos));
            let m = &mut self.mobs[i];
            m.prey = prey.map(|p| p.0);
            m.goal = match prey {
                Some((_, at)) => Some(at),
                None if pos.distance(home) > HOME_RANGE => Some(home),
                None => None,
            };
            if let Some((id, at)) = prey {
                let d = at - pos;
                if Vec3::new(d.x, 0.0, d.z).length() < 2.2 && d.y.abs() < 2.5 && m.attack_cd <= 0.0 && !m.angry {
                    m.attack_cd = 1.2;
                    punches.push((id, pos));
                }
            }
        }
        for (id, from) in punches {
            if let Some(o) = self.mobs.iter_mut().find(|o| o.id == id) {
                o.hurt = 0.0;
                o.damage(PUNCH, from);
                o.body.vel.y = 10.0;
                let at = o.body.pos;
                if !self.dedicated && self.player.body.pos.distance(at) < GUARD_RANGE {
                    self.advance("clank_you");
                }
                self.sfx(Sfx::Thud, Some(at));
                self.sfx(Sfx::MobHurt, Some(at));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clanker_moves_in_and_sees_off_monsters() {
        let mut g = crate::game::tests::arena(61);
        let square = Vec3::new(0.5, 50.0, 0.5);
        g.world.new_clankers.push(square);
        g.house_clankers();
        assert!(g.mobs.iter().any(|m| m.kind == MobKind::Clanker && m.persistent && m.home == Some(square)));
        g.alloc_mob(MobKind::Groaner, Vec3::new(6.5, 50.0, 0.5));
        let groaner = g.mobs.last().unwrap().id;
        for _ in 0..600 {
            g.update_entities(0.05);
            if !g.mobs.iter().any(|m| m.id == groaner) {
                break;
            }
        }
        assert!(!g.mobs.iter().any(|m| m.id == groaner), "the Groaner is still about");
        // Its job done, it stays near home.
        let c = g.mobs.iter().find(|m| m.kind == MobKind::Clanker).unwrap();
        assert!(c.body.pos.distance(square) < HOME_RANGE + 4.0);
    }
}
