//! Clankers (legally distinct iron golems): each village's guard. And Copper
//! Golems, which tidy.
//!
//! One moves in when a village's square chest is first filled. It stays near
//! the square, goes after any monster that comes within 16 blocks and flattens
//! it (throwing it in the air for good measure), and otherwise stands about.
//! Hit one and it'll come after you until you get out of sight. Clankers are
//! kept with the world and drop iron. Everything here runs where the world lives.
//!
//! A **Copper Golem** is built by putting a pumpkin (or a Jack o'Lantern) on
//! a block of copper. It takes a stack out of a Copper Chest near it and
//! carries it to a chest that already holds that thing (or, failing that, an
//! empty chest), sixteen blocks at most. Nowhere to put it? It puts it back
//! and has a rest. What it carries is kept in its `seed` (item, and count
//! above bit 16), so saves and joined players see it too.

use crate::block::*;
use crate::containers::Container;
use crate::entity::{Mob, MobKind};
use crate::game::Game;
use crate::sound::Sfx;
use macroquad::math::{IVec3, Vec3};

/// How far a Clanker wanders from its square, and how far it looks for trouble.
pub const HOME_RANGE: f32 = 14.0;
pub const GUARD_RANGE: f32 = 16.0;
/// A punch.
pub const PUNCH: f32 = 12.0;
/// How far a Copper Golem looks for chests, how close it gets to use one,
/// how much it carries at a time, and how long it sulks with nowhere to put it.
pub const SORT_RANGE: f32 = 16.0;
const CHEST_REACH: f32 = 2.2;
const CARRY: u8 = 16;
const SULK: f32 = 15.0;

/// Any copper block (new to oxidized, waxed or not).
pub fn is_copper_block(id: Id) -> bool {
    crate::copper::is_copper(id) || (WAXED_COPPER_FIRST..WAXED_COPPER_FIRST + 4).contains(&id)
}

/// What a golem's `seed` says it carries.
pub fn cargo(seed: u32) -> Option<(Id, u8)> {
    (seed != 0).then_some(((seed & 0xFFFF) as Id, ((seed >> 16) & 0xFF) as u8))
}
fn pack(id: Id, n: u8) -> u32 {
    if n == 0 { 0 } else { id as u32 | (n as u32) << 16 }
}

/// Put up to `n` of `id` (unworn) into a chest; how many didn't fit.
fn put_into(c: &mut Container, id: Id, mut n: u8) -> u8 {
    for pass in 0..2 {
        for i in 0..c.slots.len() {
            if n == 0 {
                return 0;
            }
            let have = match c.slots[i] {
                Some((s, k)) if pass == 0 && s == id && c.wear[i] == 0 => k,
                None if pass == 1 => 0,
                _ => continue,
            };
            let put = n.min(max_stack(id).saturating_sub(have));
            if put > 0 {
                c.slots[i] = Some((id, have + put));
                if have == 0 {
                    c.wear[i] = 0;
                }
                n -= put;
            }
        }
    }
    n
}

impl Game {
    /// A pumpkin just went on top of `top`: if it's on copper, out steps a
    /// Copper Golem (where the world lives).
    pub fn try_build_copper_golem(&mut self, top: IVec3, who: &str) -> bool {
        let below = top - IVec3::Y;
        if !matches!(self.world.get_v(top), PUMPKIN | JACK) || !is_copper_block(self.world.get_v(below)) {
            return false;
        }
        self.world.set_v(top, AIR);
        self.world.set_v(below, AIR);
        let at = below.as_vec3() + Vec3::new(0.5, 0.0, 0.5);
        let id = self.alloc_mob(MobKind::CopperGolem, at);
        if let Some(m) = self.mobs.iter_mut().find(|m| m.id == id) {
            m.persistent = true;
            m.home = Some(at);
        }
        self.smoke(at + Vec3::Y * 0.6, 8, 0.3);
        self.sfx(Sfx::Place(crate::sound::Mat::Stone), Some(at));
        self.advance_for(who, "copper_golem");
        true
    }

    /// Chests of `kind` within reach of `from`, nearest first.
    fn chests_near(&self, from: Vec3, kind: Id) -> Vec<IVec3> {
        let mut found: Vec<(f32, IVec3)> = self
            .world
            .containers
            .keys()
            .filter(|p| p.y >= 0 && self.world.get_v(**p) == kind)
            .map(|p| (p.as_vec3().distance(from - Vec3::splat(0.5)), *p))
            .filter(|(d, _)| *d <= SORT_RANGE)
            .collect();
        found.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.to_array().cmp(&b.1.to_array())));
        found.into_iter().map(|f| f.1).collect()
    }

    /// Copper Golems fetch and carry.
    pub fn copper_golems_tick(&mut self, dt: f32) {
        if self.is_client() {
            return;
        }
        // (Every tick: wandering would otherwise forget where it was going.)
        let step = dt;
        for i in 0..self.mobs.len() {
            if self.mobs[i].kind != MobKind::CopperGolem || self.mobs[i].health <= 0.0 {
                continue;
            }
            let (pos, seed) = (self.mobs[i].body.pos, self.mobs[i].seed);
            self.mobs[i].warp_cd = (self.mobs[i].warp_cd - step).max(0.0);
            let near = |p: IVec3| (p.as_vec3() + Vec3::new(0.5, 0.0, 0.5)).distance(pos) < CHEST_REACH + 0.6;
            match cargo(seed) {
                None => {
                    // Something to fetch from a Copper Chest?
                    if self.mobs[i].warp_cd > 0.0 {
                        self.mobs[i].goal = None;
                        continue;
                    }
                    let full = self.chests_near(pos, COPPER_CHEST).into_iter().find(|p| self.world.containers.get(p).is_some_and(|c| c.slots.iter().zip(&c.wear).any(|(s, w)| s.is_some() && *w == 0)));
                    let Some(src) = full else {
                        self.mobs[i].goal = None;
                        continue;
                    };
                    if near(src) {
                        let Some(c) = self.world.containers.get_mut(&src) else { continue };
                        let Some(k) = (0..c.slots.len()).find(|&k| c.slots[k].is_some() && c.wear[k] == 0) else { continue };
                        let (id, n) = c.slots[k].unwrap();
                        let take = n.min(CARRY);
                        c.slots[k] = if n > take { Some((id, n - take)) } else { None };
                        self.dirty_containers.insert(src);
                        let m = &mut self.mobs[i];
                        m.seed = pack(id, take);
                        m.goal = None;
                        self.sfx(Sfx::Place(crate::sound::Mat::Stone), Some(pos));
                    } else {
                        self.mobs[i].goal = Some(src.as_vec3() + Vec3::new(0.5, 0.0, 0.5));
                    }
                }
                Some((id, n)) => {
                    // A chest that has some already, else an empty one, else put it back.
                    let chests = self.chests_near(pos, CHEST);
                    let has = |c: &Container| c.slots.iter().zip(&c.wear).any(|(s, w)| s.is_some_and(|s| s.0 == id) && *w == 0) && c.slots.iter().zip(&c.wear).any(|(s, w)| s.is_none() || (s.is_some_and(|s| s.0 == id && s.1 < max_stack(id)) && *w == 0));
                    let dest = chests
                        .iter()
                        .copied()
                        .find(|p| self.world.containers.get(p).is_some_and(has))
                        .or_else(|| chests.iter().copied().find(|p| self.world.containers.get(p).is_some_and(|c| c.slots.iter().all(|s| s.is_none()))));
                    let back = dest.is_none();
                    let target = match dest {
                        Some(d) => Some(d),
                        None => self.chests_near(pos, COPPER_CHEST).into_iter().find(|p| self.world.containers.get(p).is_some_and(|c| c.slots.iter().any(|s| s.is_none() || s.is_some_and(|s| s.0 == id)))),
                    };
                    let Some(to) = target else {
                        self.mobs[i].goal = None;
                        continue;
                    };
                    if near(to) {
                        let Some(c) = self.world.containers.get_mut(&to) else { continue };
                        let left = put_into(c, id, n);
                        self.dirty_containers.insert(to);
                        let m = &mut self.mobs[i];
                        m.seed = pack(id, left);
                        m.goal = None;
                        if back {
                            m.warp_cd = SULK;
                        } else if left < n && !self.dedicated && self.player.body.pos.distance(pos) < 32.0 {
                            self.advance("sorted");
                        }
                        self.sfx(Sfx::Place(crate::sound::Mat::Wood), Some(pos));
                    } else {
                        self.mobs[i].goal = Some(to.as_vec3() + Vec3::new(0.5, 0.0, 0.5));
                    }
                }
            }
        }
    }

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

    #[test]
    fn a_copper_golem_is_built_and_sorts_into_the_right_chest() {
        let mut g = crate::game::tests::arena(62);
        let base = IVec3::new(0, 50, 3);
        g.world.set_v(base, crate::block::COPPER_FIRST);
        g.world.set_v(base + IVec3::Y, PUMPKIN);
        assert!(g.try_build_copper_golem(base + IVec3::Y, ""));
        assert_eq!(g.world.get_v(base), AIR);
        assert!(g.mobs.iter().any(|m| m.kind == MobKind::CopperGolem && m.persistent));
        // A Copper Chest with cobble and dirt, a chest with some dirt, and an empty one.
        let (copper, dirt_chest, empty) = (IVec3::new(4, 50, 3), IVec3::new(-6, 50, 3), IVec3::new(0, 50, -7));
        for (p, kind) in [(copper, COPPER_CHEST), (dirt_chest, CHEST), (empty, CHEST)] {
            g.world.set_v(p, kind);
            g.ensure_container(p);
        }
        let c = g.world.containers.get_mut(&copper).unwrap();
        c.slots[0] = Some((DIRT, 10));
        c.slots[1] = Some((COBBLE, 5));
        g.world.containers.get_mut(&dirt_chest).unwrap().slots[4] = Some((DIRT, 1));
        for _ in 0..1600 {
            g.update_entities(0.05);
            if g.world.containers[&copper].slots.iter().all(|s| s.is_none()) && g.mobs.iter().all(|m| m.seed == 0) {
                break;
            }
        }
        let count = |p: IVec3, id: Id| g.world.containers[&p].slots.iter().flatten().filter(|s| s.0 == id).map(|s| s.1 as u32).sum::<u32>();
        assert_eq!(count(dirt_chest, DIRT), 11, "dirt went with the dirt");
        assert_eq!(count(empty, COBBLE), 5, "cobble went to the empty chest");
        assert_eq!(count(copper, DIRT) + count(copper, COBBLE), 0);
    }
}
