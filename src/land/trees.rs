//! Trees that come back: saplings grow into trees, and leaves cut off from
//! their tree wither away.
//!
//! - **Saplings** drop from leaves (one in twenty, more as leaves decay) and
//!   go on grass or dirt. In the light (sky or torchlight 9 or more) and with
//!   room above, one grows into a tree after a few minutes. Bone Dust makes it
//!   happen sooner (often straight away).
//! - **Leaf decay.** When a log or leaf is removed, leaves nearby check
//!   whether they're still within four steps (through leaves) of a log. Those
//!   that aren't wither over the next few seconds and drop what leaves drop:
//!   sticks, saplings, and now and then an Apple.
//!
//! Everything here happens where the world lives; the blocks it changes reach
//! joined players as ordinary edits.

use crate::block::*;
use crate::game::Game;
use crate::noise::Rng;
use crate::sound::{Mat, Sfx};
use crate::world::{TreeKind, World, CH};
use macroquad::math::{ivec3, IVec3, Vec3};
use std::collections::VecDeque;

/// Chance a sapling grows in a given second (so about four minutes on average).
pub const GROW_CHANCE: f32 = 1.0 / 240.0;
/// Light a sapling needs.
pub const GROW_LIGHT: u8 = 9;
/// Leaves this many steps (through leaves) from a log stay.
pub const LEAF_REACH: i32 = 4;
/// Leaf checks handled per tick, so a felled forest doesn't stall the game.
const CHECKS_PER_TICK: usize = 64;

/// Where a tree of `kind` grown at `base` (the sapling's cell) puts its
/// blocks, and whether each pushes through what's there.
pub fn tree_shape(kind: TreeKind, base: IVec3, trunk: i32, rng: &mut Rng) -> Vec<(IVec3, Id, bool)> {
    let ground = base - IVec3::Y;
    kind.shape(trunk, |_| rng.f32()).into_iter().map(|(o, id, force)| (ground + o, id, force)).collect()
}

/// Is this leaf within reach of a log, stepping through leaves?
pub fn leaf_supported(world: &World, p: IVec3) -> bool {
    let mut seen = vec![p];
    let mut queue = VecDeque::from([(p, 0)]);
    while let Some((q, d)) = queue.pop_front() {
        for n in [q + IVec3::X, q - IVec3::X, q + IVec3::Y, q - IVec3::Y, q + IVec3::Z, q - IVec3::Z] {
            let id = world.get_v(n);
            if is_log(id) {
                return true;
            }
            if is_leaves(id) && d + 1 < LEAF_REACH && !seen.contains(&n) {
                seen.push(n);
                queue.push_back((n, d + 1));
            }
        }
    }
    false
}

impl World {
    /// A log or leaf went away: leaves around it should check they still hang on.
    pub fn wake_leaves(&mut self, p: IVec3) {
        let r = LEAF_REACH;
        for dy in -r..=r {
            for dz in -r..=r {
                for dx in -r..=r {
                    let q = p + ivec3(dx, dy, dz);
                    if is_leaves(self.get_v(q)) {
                        self.leaf_checks.insert(q);
                    }
                }
            }
        }
    }
}

impl Game {
    /// Saplings grow, leaves wither (where the world lives).
    pub fn trees_tick(&mut self, dt: f32) {
        if self.is_client() {
            return;
        }
        // Leaves that were told to check.
        let batch: Vec<IVec3> = self.world.leaf_checks.iter().take(CHECKS_PER_TICK).copied().collect();
        for p in batch {
            self.world.leaf_checks.remove(&p);
            if is_leaves(self.world.get_v(p)) && !leaf_supported(&self.world, p) && !self.decaying.iter().any(|d| d.0 == p) {
                let delay = self.rng.range(0.5, 6.0);
                self.decaying.push((p, delay));
            }
        }
        let mut wither = Vec::new();
        self.decaying.retain_mut(|d| {
            d.1 -= dt;
            if d.1 <= 0.0 {
                wither.push(d.0);
                false
            } else {
                true
            }
        });
        for p in wither {
            let id = self.world.get_v(p);
            if !is_leaves(id) || leaf_supported(&self.world, p) {
                continue;
            }
            self.world.set_v(p, AIR);
            self.block_particles(p, 4);
            let center = p.as_vec3() + Vec3::splat(0.5);
            for (item, n) in crate::farming::random_drops(id, &mut self.rng) {
                self.pop_drop(center, item, n);
            }
        }
        // Saplings, once a second.
        self.sapling_timer += dt;
        if self.sapling_timer < 1.0 {
            return;
        }
        self.sapling_timer = 0.0;
        let saplings: Vec<IVec3> = self.world.saplings.iter().copied().filter(|p| self.world.is_loaded(p.x, p.z)).collect();
        for p in saplings {
            if self.rng.chance(GROW_CHANCE) {
                self.grow_sapling(p);
            }
        }
    }

    /// Enough light for a sapling at `p`?
    fn sapling_lit(&self, p: IVec3) -> bool {
        self.world.sky_level(p.x, p.y, p.z) >= GROW_LIGHT || self.world.block_level(p.x, p.y, p.z) >= GROW_LIGHT
    }

    /// Try to turn the sapling at `p` into a tree. Returns whether it grew.
    pub fn grow_sapling(&mut self, p: IVec3) -> bool {
        if self.world.get_v(p) != SAPLING || !self.sapling_lit(p) {
            return false;
        }
        // Whatever grows around here: spruce in the cold, jungle trees in the jungle...
        let kind = self.world.generator.column(p.x, p.z).1.tree();
        let trunk = kind.trunk(self.rng.f32());
        if p.y + trunk + 2 >= CH {
            return false;
        }
        // Room for the trunk and the crown's middle.
        let room = (1..trunk + 2).all(|dy| {
            let id = self.world.get_v(p + IVec3::Y * dy);
            id == AIR || is_leaves(id) || block(id).model == Model::Cross
        });
        if !room {
            return false;
        }
        for (q, id, force) in tree_shape(kind, p, trunk, &mut self.rng) {
            let here = self.world.get_v(q);
            if force || here == AIR || block(here).model == Model::Cross && here != SAPLING {
                self.world.set_v(q, id);
            }
        }
        self.world.set_v(p, kind.log());
        self.sfx(Sfx::Place(Mat::Wood), Some(p.as_vec3() + Vec3::splat(0.5)));
        true
    }

    /// Bone Dust on a sapling: a good chance it grows right now.
    pub fn bone_sapling(&mut self, p: IVec3) -> String {
        if !self.sapling_lit(p) {
            return "Too dark. Saplings need light (sun or torches).".into();
        }
        if self.rng.chance(0.45) && self.grow_sapling(p) {
            self.advance("lumberjack_reforms");
            "Whoosh. A tree.".into()
        } else {
            "The sapling perks up. Maybe more Bone Dust?".into()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_shape_has_a_trunk_and_a_crown() {
        let mut rng = Rng::new(3);
        let blocks = tree_shape(TreeKind::Oak, ivec3(0, 10, 0), 5, &mut rng);
        let logs: Vec<_> = blocks.iter().filter(|b| b.1 == LOG).collect();
        assert_eq!(logs.len(), 5);
        assert!(blocks.iter().filter(|b| b.1 == LEAVES).count() > 20);
        assert!(blocks.iter().all(|b| b.0.y >= 10));
    }

    #[test]
    fn every_kind_of_tree_holds_on_to_all_its_leaves() {
        // Built on a stone floor, no leaf should start out too far from the trunk to stay.
        for kind in [TreeKind::Oak, TreeKind::Spruce, TreeKind::Jungle, TreeKind::Swamp] {
            for seed in 0..6 {
                let mut g = crate::game::tests::arena(40 + seed);
                let at = ivec3(0, 50, 0);
                let mut rng = Rng::new(seed as u64);
                let trunk = kind.trunk(rng.f32());
                let blocks = tree_shape(kind, at, trunk, &mut rng);
                for &(q, id, _) in &blocks {
                    g.world.set_v(q, id);
                }
                for &(q, id, _) in &blocks {
                    if is_leaves(id) && g.world.get_v(q) == id {
                        assert!(leaf_supported(&g.world, q), "{kind:?} leaf at {} (trunk {trunk}) would rot", q - at);
                    }
                }
                assert!(blocks.iter().any(|b| b.1 == kind.log()) && blocks.iter().filter(|b| b.1 == kind.leaves()).count() > 15, "{kind:?}");
            }
        }
    }

    #[test]
    fn saplings_grow_and_orphaned_leaves_decay() {
        let mut g = crate::game::tests::arena(31);
        let at = ivec3(4, 50, 4);
        let ground = at - IVec3::Y;
        g.world.set_v(ground, GRASS);
        for dy in 0..9 {
            g.world.set_v(at + IVec3::Y * dy, AIR);
        }
        g.world.set_v(at, SAPLING);
        assert!(g.world.saplings.contains(&at));
        assert!(g.grow_sapling(at));
        assert!(is_log(g.world.get_v(at)));
        assert!(!g.world.saplings.contains(&at));
        // Chop the trunk: the crown withers, dropping things.
        let mut y = at.y;
        while is_log(g.world.get_v(ivec3(at.x, y, at.z))) {
            g.world.set_v(ivec3(at.x, y, at.z), AIR);
            y += 1;
        }
        assert!(!g.world.leaf_checks.is_empty());
        for _ in 0..600 {
            g.trees_tick(0.05);
        }
        let left = (-3..=3).flat_map(|dx| (-3..=3).flat_map(move |dz| (0..16).map(move |dy| ivec3(dx, dy, dz)))).filter(|&o| is_leaves(g.world.get_v(at + o))).count();
        assert_eq!(left, 0);
    }

    #[test]
    fn leaves_next_to_a_log_stay() {
        let mut g = crate::game::tests::arena(32);
        let at = ivec3(-5, 53, 2);
        g.world.set_v(at, LOG);
        g.world.set_v(at + IVec3::X, LEAVES);
        g.world.set_v(at + IVec3::X * 2, LEAVES);
        assert!(leaf_supported(&g.world, at + IVec3::X * 2));
        g.world.set_v(at, AIR);
        assert!(!leaf_supported(&g.world, at + IVec3::X * 2));
    }
}
