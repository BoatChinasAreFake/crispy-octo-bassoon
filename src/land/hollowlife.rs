//! Life in the Hollow: Leapstalks and their fruit, and mauve stone.
//!
//! - **Leapstalks** grow on the outer islands' Hollow Stone (worlds from
//!   generator 9): knobbly purple stems that branch and end in pale
//!   **Leapflowers**. Break a stalk and everything above it that's left
//!   hanging comes down too, dropping **Leapfruit**. Plant a Leapflower on
//!   Hollow Stone and it grows into a new stalk, branching as it goes.
//! - **Leapfruit**: eat one and you hop somewhere safe a few blocks away
//!   (handy, or not). Smelted, it pops; four **Popped Leapfruit** make four
//!   **Mauve Blocks**, and two of those make two **Mauve Pillars**: the
//!   stone the Hollow Cities are built from.

use crate::block::*;
use crate::game::Game;
use crate::noise::hash3;
use crate::sound::Sfx;
use macroquad::math::{ivec3, IVec3, Vec3};
use std::collections::{HashMap, HashSet, VecDeque};

/// How tall a Leapstalk's trunk grows before it stops.
const MAX_TRUNK: i32 = 6;
/// How far a Leapfruit can hop you.
const HOP: i32 = 8;

pub fn is_stalk(id: Id) -> bool {
    (LEAPSTALK_FIRST..LEAPSTALK_FIRST + 16).contains(&id)
}

/// Any part of a Leapstalk plant.
pub fn is_plant(id: Id) -> bool {
    is_stalk(id) || id == LEAPFLOWER
}

/// East, west, south, north: the bits of a join mask (as fences').
const SIDES: [(IVec3, u8); 4] = [(IVec3::X, 1), (IVec3::NEG_X, 2), (IVec3::Z, 4), (IVec3::NEG_Z, 8)];

/// The stalk block for `p`, joined to the plant beside it (`get` says what's where).
pub fn stalk_at(get: impl Fn(IVec3) -> Id, p: IVec3) -> Id {
    LEAPSTALK_FIRST + SIDES.iter().filter(|(d, _)| is_plant(get(p + *d))).fold(0, |m, (_, b)| m | b) as Id
}

/// A whole wild plant rooted on top of `base` (the Hollow Stone under it):
/// a trunk, a few branches that turn upward, and a flower at each tip.
pub fn plant_blocks(seed: u32, base: IVec3) -> Vec<(IVec3, Id)> {
    let h = |k: i32| hash3(seed, base.x, k, base.z);
    let mut parts: HashMap<IVec3, bool> = HashMap::new(); // true: a flower
    let trunk = 3 + (h(0) * 3.0) as i32;
    for y in 1..=trunk {
        parts.insert(base + IVec3::Y * y, false);
    }
    parts.insert(base + IVec3::Y * (trunk + 1), true);
    for (k, (d, _)) in SIDES.iter().enumerate() {
        if h(10 + k as i32) > 0.55 {
            continue;
        }
        let at = 2 + (h(20 + k as i32) * (trunk - 2) as f32) as i32;
        let out = 1 + (h(30 + k as i32) * 2.0) as i32;
        let up = 1 + (h(40 + k as i32) * 2.0) as i32;
        let mut p = base + IVec3::Y * at;
        for _ in 0..out {
            p += *d;
            parts.insert(p, false);
        }
        for _ in 0..up {
            p += IVec3::Y;
            parts.insert(p, false);
        }
        parts.insert(p + IVec3::Y, true);
    }
    let get = |q: IVec3| match parts.get(&q) {
        Some(true) => LEAPFLOWER,
        Some(false) => LEAPSTALK_FIRST,
        None => AIR,
    };
    let mut v: Vec<(IVec3, Id)> = parts.iter().map(|(&p, &flower)| (p, if flower { LEAPFLOWER } else { stalk_at(get, p) })).collect();
    v.sort_by_key(|(p, _)| (p.y, p.x, p.z));
    v
}

impl Game {
    /// A plant block went: anything of the plant left with nothing under it falls (where the world lives).
    pub fn plant_gone(&mut self, p: IVec3, old: Id) {
        if !is_plant(old) || self.is_client() {
            return;
        }
        // (`p` counts as gone even if the world hasn't caught up yet: a joined
        // player's break is checked before it's written.)
        let mut seen = HashSet::from([p]);
        for start in [p + IVec3::Y, p + IVec3::X, p - IVec3::X, p + IVec3::Z, p - IVec3::Z] {
            if seen.contains(&start) || !is_plant(self.world.get_v(start)) {
                continue;
            }
            // The part of the plant joined to this neighbour.
            let mut part = Vec::new();
            let mut queue = VecDeque::from([start]);
            seen.insert(start);
            while let Some(q) = queue.pop_front() {
                part.push(q);
                if part.len() > 256 {
                    break;
                }
                for d in [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z] {
                    let r = q + d;
                    if !seen.contains(&r) && is_plant(self.world.get_v(r)) {
                        seen.insert(r);
                        queue.push_back(r);
                    }
                }
            }
            // Still standing on something?
            let held_up = part.iter().any(|q| {
                let under = self.world.get_v(*q - IVec3::Y);
                *q - IVec3::Y != p && !is_plant(under) && is_solid(under)
            });
            if held_up {
                continue;
            }
            for q in part {
                let id = self.world.get_v(q);
                self.world.set_v(q, AIR);
                let drop = if id == LEAPFLOWER { Some(LEAPFLOWER) } else { self.rng.chance(0.5).then_some(LEAPFRUIT) };
                if let Some(item) = drop {
                    self.pop_drop(q.as_vec3() + Vec3::splat(0.5), item, 1);
                }
            }
        }
    }

    /// A Leapflower now and then grows up a block, leaving stalk under it, and
    /// sometimes sends a branch off to one side (where the world lives).
    pub fn leapflower_tick(&mut self, p: IVec3) {
        if !self.rng.chance(0.08) {
            return;
        }
        let up = p + IVec3::Y;
        if self.world.get_v(up) != AIR || up.y >= crate::world::CH - 1 {
            return;
        }
        // Count the stalk under it, down to what it grows from.
        let mut trunk = 0;
        while is_stalk(self.world.get_v(p - IVec3::Y * (trunk + 1))) && trunk < 32 {
            trunk += 1;
        }
        let root = self.world.get_v(p - IVec3::Y * (trunk + 1));
        // (A branch's flower isn't over a trunk; it's still growing if the stalk beside it is.)
        let on_branch = trunk == 0 && SIDES.iter().any(|(d, _)| is_stalk(self.world.get_v(p + *d)));
        if trunk >= MAX_TRUNK || !(root == HOLLOW_STONE || on_branch || is_stalk(root)) {
            return;
        }
        self.world.set_v(up, LEAPFLOWER);
        self.world.set_v(p, stalk_at(|q| if q == p { AIR } else { self.world.get_v(q) }, p));
        if trunk >= 2 && self.rng.chance(0.3) {
            let (d, _) = SIDES[self.rng.int(0, 3) as usize];
            let side = p + d;
            if self.world.get_v(side) == AIR && self.world.get_v(side - IVec3::Y) == AIR {
                self.world.set_v(side, LEAPFLOWER);
            }
        }
        self.world.reshape_joins(p);
    }

    /// Eat a Leapfruit: hop to somewhere safe nearby (a few tries; you stay put if there's nowhere).
    pub fn leapfruit_hop(&mut self) -> bool {
        let from = self.player.body.pos;
        let here = from.floor().as_ivec3();
        for _ in 0..16 {
            let (dx, dz) = (self.rng.int(-HOP, HOP), self.rng.int(-HOP, HOP));
            if dx.abs() + dz.abs() < 2 || !self.world.is_loaded(here.x + dx, here.z + dz) {
                continue;
            }
            // From a little above you down to a little below, the first place to stand.
            let to = (-HOP..=HOP).rev().map(|dy| here + ivec3(dx, dy, dz)).find(|&to| {
                to.y >= 2
                    && to.y < crate::world::CH - 2
                    && !is_solid(self.world.get_v(to))
                    && !is_solid(self.world.get_v(to + IVec3::Y))
                    && !is_liquid(self.world.get_v(to))
                    && is_solid(self.world.get_v(to - IVec3::Y))
            });
            if let Some(to) = to {
                let at = to.as_vec3() + Vec3::new(0.5, 0.0, 0.5);
                self.player.body.pos = at;
                self.player.body.vel = Vec3::ZERO;
                self.player.fall_start = at.y;
                self.sfx(Sfx::Warp, None);
                self.advance("hop_skip");
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wild_plants_branch_and_end_in_flowers() {
        let _lock = crate::mods::registry_test_lock();
        for seed in 0..20 {
            let v = plant_blocks(seed * 31, ivec3(5, 60, -3));
            let at: HashMap<IVec3, Id> = v.iter().copied().collect();
            assert!(v.iter().any(|(_, id)| *id == LEAPFLOWER));
            assert!(v.iter().all(|(p, id)| is_plant(*id) && p.y > 60));
            // Every stalk's joins match its neighbours.
            for (p, id) in &v {
                if is_stalk(*id) {
                    assert_eq!(*id, stalk_at(|q| at.get(&q).copied().unwrap_or(AIR), *p));
                }
            }
        }
    }

    #[test]
    fn breaking_a_stalk_brings_down_what_it_held_up() {
        let mut g = crate::game::tests::arena(901);
        let base = ivec3(3, 50, 3);
        g.world.set_v(base, HOLLOW_STONE);
        for (p, id) in plant_blocks(7, base) {
            g.world.set_v(p, id);
        }
        g.drops.clear();
        let first = base + IVec3::Y;
        let old = g.world.get_v(first);
        // (As the host does a joined player's break: checked, then written.)
        g.plant_gone(first, old);
        g.world.set_v(first, AIR);
        assert!(!(1..12).any(|y| (-3..=3).any(|dx| (-3..=3).any(|dz| is_plant(g.world.get_v(base + ivec3(dx, y, dz)))))), "nothing left hanging");
        assert!(!g.drops.is_empty());
    }

    #[test]
    fn a_planted_flower_grows_into_a_stalk() {
        let mut g = crate::game::tests::arena(902);
        let base = ivec3(0, 50, 4);
        g.world.set_v(base, HOLLOW_STONE);
        g.world.set_v(base + IVec3::Y, LEAPFLOWER);
        for _ in 0..2000 {
            let top = (1..20).map(|y| base + IVec3::Y * y).find(|p| g.world.get_v(*p) == LEAPFLOWER).unwrap();
            g.leapflower_tick(top);
        }
        assert!(is_stalk(g.world.get_v(base + IVec3::Y)));
        assert_eq!(g.world.get_v(base + IVec3::Y * (MAX_TRUNK + 1)), LEAPFLOWER, "grows to full height and stops");
    }

    #[test]
    fn leapfruit_hops_you_somewhere_safe() {
        let mut g = crate::game::tests::arena(903);
        let start = g.player.body.pos;
        assert!(g.leapfruit_hop());
        let at = g.player.body.pos;
        assert!(at.distance(start) > 0.5 && (at - start).abs().max_element() <= HOP as f32 + 1.0);
        let feet = at.floor().as_ivec3();
        assert!(is_solid(g.world.get_v(feet - IVec3::Y)) && !is_solid(g.world.get_v(feet)));
    }
}
