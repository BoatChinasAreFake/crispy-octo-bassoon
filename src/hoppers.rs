//! Hoppers: move items between containers, and pick up what lands on them.
//!
//! A hopper (five iron and a chest) holds five stacks. A few times a second
//! it takes one item from the container above it (only the output of a
//! furnace or brewing stand) or swallows items lying on top of it, and passes
//! one item on to the container its spout points at: down, or to the side it
//! was put against. Into a furnace from above goes the input, from the side
//! the fuel; a brewing stand likewise takes ingredients from above and
//! bottles from the side. Power switches a hopper off.
//!
//! Hoppers work where the world lives; joined players open them like chests.

use crate::block::*;
use crate::containers::{accepts, is_container, is_three_slot, Container, FUEL, INPUT, OUTPUT};
use crate::game::Game;
use crate::inventory::Wear;
use macroquad::math::{IVec3, Vec3};

pub const SLOTS: usize = 5;
/// Seconds between moves.
pub const STEP: f32 = 0.4;

pub fn is_hopper(id: Id) -> bool {
    (HOPPER_FIRST..HOPPER_FIRST + 5).contains(&id)
}

/// Where a hopper's spout points: down, or one of the four sides (contraptions::dir6 order).
pub fn spout(id: Id) -> IVec3 {
    match id - HOPPER_FIRST {
        0 => IVec3::NEG_Y,
        k => crate::contraptions::dir6((k - 1) as u8),
    }
}

/// Slots of `c` (a container of block `kind`) an item may be taken from by a hopper below.
fn takeable(kind: Id, c: &Container) -> Vec<usize> {
    if is_three_slot(kind) { vec![OUTPUT] } else { (0..c.slots.len()).collect() }
}

/// The slot an item arriving from direction `from` (the hopper's spout) goes into.
fn landing_slot(kind: Id, c: &Container, item: Id, wear: Wear, from_above: bool) -> Option<usize> {
    let fits = |i: usize| match c.slots[i] {
        None => accepts(kind, i, item),
        Some((id, n)) => id == item && n < max_stack(id) && c.wear[i] == wear && accepts(kind, i, item),
    };
    if is_three_slot(kind) {
        let i = if from_above { INPUT } else { FUEL };
        return fits(i).then_some(i);
    }
    // Top up a matching stack first, then an empty slot.
    (0..c.slots.len()).filter(|&i| c.slots[i].is_some()).find(|&i| fits(i)).or_else(|| (0..c.slots.len()).find(|&i| c.slots[i].is_none() && fits(i)))
}

fn take_one(c: &mut Container, i: usize) -> Option<(Id, Wear)> {
    let (id, n) = c.slots[i]?;
    let wear = c.wear[i];
    c.slots[i] = if n > 1 { Some((id, n - 1)) } else { None };
    if n == 1 {
        c.wear[i] = 0;
    }
    Some((id, wear))
}

fn put_one(c: &mut Container, i: usize, item: Id, wear: Wear) {
    c.slots[i] = Some((item, c.slots[i].map(|s| s.1).unwrap_or(0) + 1));
    c.wear[i] = wear;
}

impl Game {
    pub fn hoppers_tick(&mut self, dt: f32) {
        if self.is_client() {
            return;
        }
        self.hopper_timer += dt;
        if self.hopper_timer < STEP {
            return;
        }
        self.hopper_timer = 0.0;
        let hoppers: Vec<IVec3> = self.world.containers.keys().copied().filter(|p| self.world.is_loaded(p.x, p.z) && is_hopper(self.world.get_v(*p))).collect();
        for p in hoppers {
            if crate::wiring::powered(&self.world, p) {
                continue;
            }
            self.hopper_pull(p);
            self.hopper_push(p);
        }
        self.hopper_carts();
    }

    /// The container in a cell: its block's, or a loaded cart's standing in it.
    fn container_in(&self, cell: IVec3) -> Option<IVec3> {
        if is_container(self.world.get_v(cell)) {
            return Some(cell);
        }
        self.vehicles.iter().find(|v| v.contents.is_some() && v.cell() == cell).map(|v| crate::vehicles::cart_key(v.id))
    }

    /// Move one item from container `from` (only `slots` of it) to `to`
    /// (arriving from above or the side). True if one moved.
    fn move_one(&mut self, from: IVec3, slots: Option<Vec<usize>>, to: IVec3, from_above: bool) -> bool {
        use crate::containers::{store, store_kind, store_ref};
        let (src_kind, dst_kind) = (store_kind(&self.world, &self.vehicles, from), store_kind(&self.world, &self.vehicles, to));
        let (Some(src), Some(dst)) = (store_ref(&self.world, &self.vehicles, from), store_ref(&self.world, &self.vehicles, to)) else { return false };
        let slots = slots.unwrap_or_else(|| takeable(src_kind, src));
        let pick = slots.into_iter().find(|&i| src.slots.get(i).copied().flatten().is_some_and(|(id, _)| landing_slot(dst_kind, dst, id, src.wear[i], from_above).is_some()));
        let Some(i) = pick else { return false };
        let Some((item, wear)) = store(&mut self.world, &mut self.vehicles, from).and_then(|c| take_one(c, i)) else { return false };
        if let Some(c) = store(&mut self.world, &mut self.vehicles, to)
            && let Some(j) = landing_slot(dst_kind, c, item, wear, from_above)
        {
            put_one(c, j, item, wear);
        }
        self.dirty_containers.insert(from);
        self.dirty_containers.insert(to);
        true
    }

    /// Hopper carts take from containers above the track and swallow what they roll over.
    fn hopper_carts(&mut self) {
        let carts: Vec<(u32, IVec3, Vec3)> = self.vehicles.iter().filter(|v| v.kind == crate::vehicles::HOPPER_CART_KIND).map(|v| (v.id, v.cell(), v.pos)).collect();
        for (id, cell, pos) in carts {
            let key = crate::vehicles::cart_key(id);
            let above = cell + IVec3::Y;
            if is_container(self.world.get_v(above)) && self.move_one(above, None, key, true) {
                continue;
            }
            let Some(k) = self.drops.iter().position(|d| (d.body.pos - pos).abs().max_element() < 1.0) else { continue };
            let (item, wear) = (self.drops[k].item, self.drops[k].wear);
            let Some(c) = crate::containers::store(&mut self.world, &mut self.vehicles, key) else { continue };
            let Some(j) = landing_slot(HOPPER_FIRST, c, item, wear, true) else { continue };
            put_one(c, j, item, wear);
            self.drops[k].n -= 1;
            if self.drops[k].n == 0 {
                self.drops.remove(k);
            }
            self.dirty_containers.insert(key);
        }
    }

    /// Take one item from above (a container, or the ground).
    fn hopper_pull(&mut self, p: IVec3) {
        let hopper = self.world.get_v(p);
        let above = p + IVec3::Y;
        // A container above (or a loaded cart on a rail on top of it).
        if let Some(src) = self.container_in(above) {
            self.move_one(src, None, p, true);
            return;
        }
        // Items lying on top.
        let top = p.as_vec3() + Vec3::new(0.5, 1.0, 0.5);
        let Some(k) = self.drops.iter().position(|d| (d.body.pos - top).abs().max_element() < 0.7 && d.body.pos.y >= top.y - 0.3) else { return };
        let (item, wear) = (self.drops[k].item, self.drops[k].wear);
        let Some(c) = self.world.containers.get_mut(&p) else { return };
        let Some(j) = landing_slot(hopper, c, item, wear, true) else { return };
        put_one(c, j, item, wear);
        self.drops[k].n -= 1;
        if self.drops[k].n == 0 {
            self.drops.remove(k);
        }
        self.dirty_containers.insert(p);
    }

    /// Pass one item on through the spout.
    fn hopper_push(&mut self, p: IVec3) {
        let hopper = self.world.get_v(p);
        let d = spout(hopper);
        // Into whatever's there: a container, or a loaded cart.
        let Some(to) = self.container_in(p + d) else { return };
        let all = self.world.containers.get(&p).map(|c| (0..c.slots.len()).collect());
        self.move_one(p, all, to, d == IVec3::NEG_Y);
    }

    /// The hopper to place: spout down, or toward the block it was put against.
    pub fn hopper_facing(normal: IVec3) -> Id {
        if normal.y != 0 {
            return HOPPER_FIRST;
        }
        HOPPER_FIRST + 1 + crate::contraptions::facing_of(-normal) as Id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use macroquad::math::ivec3;

    #[test]
    fn a_chest_feeds_a_furnace_through_a_hopper() {
        let mut g = crate::game::tests::arena(81);
        let chest = ivec3(0, 52, 0);
        let hop = ivec3(0, 51, 0);
        let furnace = ivec3(0, 50, 0);
        g.world.set_v(furnace, FURNACE);
        g.world.set_v(hop, HOPPER_FIRST);
        g.world.set_v(chest, CHEST);
        g.world.containers.get_mut(&chest).unwrap().slots[3] = Some((COBBLE, 3));
        for _ in 0..40 {
            g.hoppers_tick(0.1);
        }
        assert_eq!(g.world.containers[&chest].slots[3], None);
        assert_eq!(g.world.containers[&furnace].slots[INPUT], Some((COBBLE, 3)));
        // Power stops it.
        g.world.containers.get_mut(&chest).unwrap().slots[0] = Some((COAL, 2));
        g.world.set_v(hop + IVec3::X, ZAP_BLOCK);
        for _ in 0..40 {
            g.hoppers_tick(0.1);
        }
        assert_eq!(g.world.containers[&chest].slots[0], Some((COAL, 2)));
    }

    #[test]
    fn carts_are_loaded_and_unloaded_by_hoppers() {
        use crate::vehicles::{cart_key, CHEST_CART_KIND, HOPPER_CART_KIND};
        let mut g = crate::game::tests::arena(83);
        // A hopper under the track, with a chest cart parked on a rail above it.
        let under = ivec3(3, 50, 3);
        g.world.set_v(under, HOPPER_FIRST);
        g.world.set_v(under + IVec3::Y, RAIL_FIRST);
        let cart = g.spawn_vehicle(CHEST_CART_KIND, (under + IVec3::Y).as_vec3() + Vec3::new(0.5, 0.06, 0.5), 0.0);
        let key = cart_key(cart);
        crate::containers::store(&mut g.world, &mut g.vehicles, key).unwrap().slots[4] = Some((COBBLE, 3));
        for _ in 0..20 {
            g.hoppers_tick(0.1);
        }
        assert_eq!(g.world.containers[&under].slots[0], Some((COBBLE, 3)), "unloaded into the hopper");
        // A hopper pointing sideways at the cart fills it back up.
        g.world.set_v(under, STONE);
        let side = under + IVec3::Y + IVec3::X;
        g.world.set_v(side, HOPPER_FIRST + 1 + crate::contraptions::facing_of(IVec3::NEG_X) as Id);
        g.world.containers.get_mut(&side).unwrap().slots[1] = Some((DIAMOND, 2));
        for _ in 0..20 {
            g.hoppers_tick(0.1);
        }
        let c = crate::containers::store_ref(&g.world, &g.vehicles, key).unwrap();
        assert!(c.slots.contains(&Some((DIAMOND, 2))), "{:?}", c.slots);
        // A hopper cart swallows what it's on top of.
        let hc = g.spawn_vehicle(HOPPER_CART_KIND, Vec3::new(-3.5, 50.06, -3.5), 0.0);
        g.spawn_drop(Vec3::new(-3.5, 50.2, -3.5), APPLE, 2, 0, Vec3::ZERO, 0.0);
        for _ in 0..20 {
            g.hoppers_tick(0.1);
        }
        assert_eq!(crate::containers::store_ref(&g.world, &g.vehicles, cart_key(hc)).unwrap().slots[0], Some((APPLE, 2)));
    }

    #[test]
    fn hoppers_swallow_items_on_top() {
        let mut g = crate::game::tests::arena(82);
        let hop = ivec3(2, 50, 2);
        g.world.set_v(hop, HOPPER_FIRST);
        g.spawn_drop(hop.as_vec3() + Vec3::new(0.5, 1.05, 0.5), DIAMOND, 2, 0, Vec3::ZERO, 0.0);
        for _ in 0..20 {
            g.hoppers_tick(0.1);
        }
        assert_eq!(g.world.containers[&hop].slots[0], Some((DIAMOND, 2)));
        assert!(g.drops.is_empty());
    }
}
