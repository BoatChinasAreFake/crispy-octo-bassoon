//! Sand, gravel and anvils fall.
//!
//! Like liquids, nothing moves until something changes nearby: every edit
//! queues the cell above it (and a falling block placed in mid-air queues
//! itself), so a beach the generator left overhanging a cave stays put until
//! you dig under it. Then the block becomes a falling block that drops (through
//! air, water, lava and plants), and lands as a block again on whatever
//! stops it. An anvil that lands on someone hurts, as advertised.
//!
//! The world lives where the falling happens (single player, host, server);
//! joined players get the falling blocks with the mob snapshots and draw them.

use crate::block::*;
use crate::game::Game;
use crate::sound::{Mat, Sfx};
use macroquad::math::{IVec3, Mat4, Vec3};

/// How fast things fall, and how fast they're allowed to get.
const GRAVITY: f32 = 32.0;
const TERMINAL: f32 = 40.0;
/// Most queued cells looked at per tick (a long sand column cascades over a few).
const MAX_PER_TICK: usize = 512;

/// Does this block fall when there's nothing under it?
pub fn is_gravity(id: Id) -> bool {
    matches!(id, SAND | GRAVEL | RED_SAND | SUSPICIOUS_SAND | SUSPICIOUS_GRAVEL) || crate::anvil::is_anvil(id) || crate::masonry::is_powder(id)
}

/// Can a falling block drop through a cell holding `id`?
pub fn falls_through(id: Id) -> bool {
    id == AIR || id == FIRE || is_liquid(id) || block(id).model == Model::Cross
}

#[derive(Clone, Copy, Debug)]
pub struct FallingBlock {
    /// The block's bottom-north-west corner.
    pub pos: Vec3,
    pub vel: f32,
    pub id: Id,
    /// Where it started (an anvil hurts more the further it fell).
    pub from: f32,
}

impl Game {
    /// Start anything that's lost its footing falling, move what's falling, and land it.
    pub fn falling_tick(&mut self, dt: f32) {
        let host = !self.is_client();
        if host && !self.world.fall_dirty.is_empty() {
            let cells: Vec<IVec3> = self.world.fall_dirty.iter().copied().take(MAX_PER_TICK).collect();
            for p in cells {
                self.world.fall_dirty.remove(&p);
                let id = self.world.get_v(p);
                if p.y > 0 && is_gravity(id) && self.world.is_loaded(p.x, p.z) && falls_through(self.world.get_v(p - IVec3::Y)) {
                    // (The cell above hears about it, so a whole column follows.)
                    self.world.set_v(p, AIR);
                    self.falling.push(FallingBlock { pos: p.as_vec3(), vel: 0.0, id, from: p.y as f32 });
                }
            }
        }
        let mut landed: Vec<(usize, IVec3)> = Vec::new();
        for (i, f) in self.falling.iter_mut().enumerate() {
            f.vel = (f.vel - GRAVITY * dt).max(-TERMINAL);
            let next = f.pos.y + f.vel * dt;
            if !host {
                f.pos.y = next;
                continue;
            }
            let (x, z) = (f.pos.x.floor() as i32, f.pos.z.floor() as i32);
            let below = next.floor() as i32;
            if below < 0 {
                landed.push((i, IVec3::new(x, -1, z)));
            } else if !falls_through(self.world.get(x, below, z)) {
                landed.push((i, IVec3::new(x, below + 1, z)));
            } else {
                f.pos.y = next;
            }
        }
        for &(i, at) in landed.iter().rev() {
            let f = self.falling.swap_remove(i);
            if at.y < 0 {
                continue;
            }
            let here = self.world.get_v(at);
            let centre = at.as_vec3() + Vec3::new(0.5, 0.0, 0.5);
            if falls_through(here) {
                self.world.set_v(at, f.id);
                // Concrete Powder sets if it lands in (or by) water.
                if crate::masonry::is_powder(f.id) && is_water(here) {
                    self.world.set_v(at, crate::masonry::set_form(f.id));
                }
                self.harden_powder(at);
            } else {
                // Somewhere it can't sit (a slab, a torch post): it breaks into an item.
                self.pop_drop(centre + Vec3::Y * 0.3, f.id, 1);
            }
            if crate::anvil::is_anvil(f.id) {
                self.anvil_lands(centre, f.from - at.y as f32);
            } else {
                self.sfx(Sfx::Place(if f.id == GRAVEL || f.id == SUSPICIOUS_GRAVEL { Mat::Stone } else { Mat::Sand }), Some(centre));
            }
        }
    }

    /// Clang. Anyone underneath takes two points per block fallen (up to 40).
    fn anvil_lands(&mut self, at: Vec3, fell: f32) {
        self.sfx(Sfx::Thud, Some(at));
        let dmg = (fell.max(0.0) * 2.0).min(40.0);
        if dmg < 1.0 {
            return;
        }
        let under = |p: Vec3, h: f32| (p.x - at.x).abs() < 0.8 && (p.z - at.z).abs() < 0.8 && p.y <= at.y + 1.0 && p.y + h >= at.y;
        for m in self.mobs.iter_mut().filter(|m| under(m.body.pos, m.body.height)) {
            m.damage(dmg, at + Vec3::Y);
        }
        if under(self.player.body.pos, 1.8) {
            self.player.hurt = 0.0;
            self.hurt_player(dmg, "was flattened by a falling anvil. Ominous indeed.");
        }
        let peers: Vec<u32> = self.peers.iter().filter(|(_, p)| p.alive() && under(p.target, 1.8)).map(|(&id, _)| id).collect();
        for id in peers {
            self.hurt_peer(id, dmg, "was flattened by a falling anvil. Ominous indeed.", Vec3::ZERO);
        }
    }

    pub fn draw_falling(&self, g: &mut crate::render::DynGeo) {
        for f in &self.falling {
            let sky = self.world.shade_near(f.pos + Vec3::new(0.5, 0.5, 0.5));
            let root = Mat4::from_translation(f.pos + Vec3::new(0.5, 0.0, 0.5));
            crate::drops::draw_item(g, &root, f.id, 1.0, sky);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settle(g: &mut Game) {
        for _ in 0..200 {
            g.falling_tick(0.05);
        }
    }

    #[test]
    fn sand_falls_when_dug_under_and_columns_follow() {
        let mut g = crate::game::tests::arena(71);
        let base = IVec3::new(3, 50, 3);
        // A pillar: stone, with three sand on top.
        for y in 0..4 {
            g.world.set_v(base + IVec3::Y * y, if y == 0 { STONE } else { SAND });
        }
        settle(&mut g);
        assert_eq!(g.world.get_v(base + IVec3::Y * 3), SAND, "supported sand stays");
        // Take the stone away: the column drops one block.
        g.world.set_v(base, AIR);
        settle(&mut g);
        assert!(g.falling.is_empty());
        assert_eq!((0..3).map(|y| g.world.get_v(base + IVec3::Y * y)).collect::<Vec<_>>(), vec![SAND; 3]);
        assert_eq!(g.world.get_v(base + IVec3::Y * 3), AIR);
    }

    #[test]
    fn placed_in_mid_air_it_drops_and_anvils_hurt() {
        let mut g = crate::game::tests::arena(72);
        let up = IVec3::new(-3, 60, 2);
        g.world.set_v(up, GRAVEL);
        g.falling_tick(0.05);
        assert_eq!(g.world.get_v(up), AIR);
        assert_eq!(g.falling.len(), 1);
        settle(&mut g);
        assert_eq!(g.world.get_v(IVec3::new(-3, 50, 2)), GRAVEL, "on the floor");

        // An anvil dropped from ten blocks onto a Mooer.
        let mut rng = crate::noise::Rng::new(3);
        let mut cow = crate::entity::Mob::new(crate::entity::MobKind::Mooer, Vec3::new(5.5, 50.0, 5.5), &mut rng);
        cow.persistent = true;
        let before = cow.health;
        g.mobs.push(cow);
        g.world.set_v(IVec3::new(5, 60, 5), ANVIL);
        settle(&mut g);
        assert!(g.mobs[0].health < before, "the anvil landed on it");
    }
}
