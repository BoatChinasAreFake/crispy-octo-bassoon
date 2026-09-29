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
    }

    /// Take one item from above (a container, or the ground).
    fn hopper_pull(&mut self, p: IVec3) {
        let hopper = self.world.get_v(p);
        let above = p + IVec3::Y;
        let above_id = self.world.get_v(above);
        if is_container(above_id) {
            let Some(src) = self.world.containers.get(&above) else { return };
            let Some(dst) = self.world.containers.get(&p) else { return };
            let pick = takeable(above_id, src).into_iter().find(|&i| src.slots[i].is_some_and(|(id, _)| landing_slot(hopper, dst, id, src.wear[i], true).is_some()));
            let Some(i) = pick else { return };
            let Some((item, wear)) = self.world.containers.get_mut(&above).and_then(|c| take_one(c, i)) else { return };
            if let Some(c) = self.world.containers.get_mut(&p)
                && let Some(j) = landing_slot(hopper, c, item, wear, true)
            {
                put_one(c, j, item, wear);
            }
            self.dirty_containers.insert(above);
            self.dirty_containers.insert(p);
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
        let to = p + d;
        let kind = self.world.get_v(to);
        if !is_container(kind) {
            return;
        }
        let (Some(src), Some(dst)) = (self.world.containers.get(&p), self.world.containers.get(&to)) else { return };
        let from_above = d == IVec3::NEG_Y;
        let pick = (0..src.slots.len()).find(|&i| src.slots[i].is_some_and(|(id, _)| landing_slot(kind, dst, id, src.wear[i], from_above).is_some()));
        let Some(i) = pick else { return };
        let Some((item, wear)) = self.world.containers.get_mut(&p).and_then(|c| take_one(c, i)) else { return };
        if let Some(c) = self.world.containers.get_mut(&to)
            && let Some(j) = landing_slot(kind, c, item, wear, from_above)
        {
            put_one(c, j, item, wear);
        }
        self.dirty_containers.insert(p);
        self.dirty_containers.insert(to);
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
