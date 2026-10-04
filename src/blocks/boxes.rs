//! Hollow Boxes: chests that keep what's inside when you break them.
//!
//! Breaking a box that holds anything tucks its contents away under a number,
//! and the Hollow Box item that drops carries that number in its wear (where
//! enchantments usually go, so joined players' hosts already track it; see
//! ledger.rs). Placing the item again unpacks the contents into the new box.
//! Boxes can go in chests, on the ground and in other boxes like any item.

use crate::block::*;
use crate::containers::Container;
use crate::game::Game;
use crate::inventory::Wear;
use macroquad::math::{IVec3, Vec3};
use std::collections::HashMap;

/// The packed contents' number carried by a Hollow Box item (0: empty).
pub fn box_id(wear: Wear) -> u16 {
    (wear >> 16) as u16
}

pub fn box_wear(id: u16) -> Wear {
    (id as Wear) << 16
}

/// Saved like chests, keyed by (number, 0, 0).
pub fn encode(boxes: &HashMap<u16, Container>) -> Vec<u8> {
    crate::containers::encode(&boxes.iter().map(|(&id, c)| (IVec3::new(id as i32, 0, 0), c.clone())).collect())
}

pub fn decode(b: &[u8]) -> HashMap<u16, Container> {
    crate::containers::decode(b, 4).into_iter().filter(|(p, _)| (1..=u16::MAX as i32).contains(&p.x)).map(|(p, c)| (p.x as u16, c)).collect()
}

impl Game {
    /// A Hollow Box at `pos` is being broken: pack its contents and drop it as
    /// an item carrying them. Returns false if it wasn't a Hollow Box.
    pub fn pack_box(&mut self, pos: IVec3) -> bool {
        if self.world.get_v(pos) != HOLLOW_BOX {
            return false;
        }
        let at = pos.as_vec3() + Vec3::splat(0.5);
        let c = self.world.containers.remove(&pos);
        let wear = match c.filter(|c| !c.contents().is_empty()) {
            Some(c) => {
                // Next free number (unused ones get reused once their box is placed again).
                let id = (1..=u16::MAX).find(|i| !self.boxes.contains_key(i)).unwrap_or(1);
                self.boxes.insert(id, c);
                box_wear(id)
            }
            None => 0,
        };
        self.pop_drop_worn(at, HOLLOW_BOX, 1, wear);
        true
    }

    /// A Hollow Box item carrying `wear` was placed at `pos`: unpack it.
    pub fn unpack_box(&mut self, pos: IVec3, wear: Wear) {
        let id = box_id(wear);
        if id == 0 || self.world.get_v(pos) != HOLLOW_BOX {
            return;
        }
        if let Some(c) = self.boxes.remove(&id) {
            self.world.containers.insert(pos, c);
            self.dirty_containers.insert(pos);
        }
    }

    /// How many things a Hollow Box item holds.
    #[cfg(test)]
    pub fn box_count(&self, wear: Wear) -> usize {
        self.boxes.get(&box_id(wear)).map(|c| c.contents().iter().map(|s| s.1 as usize).sum()).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boxes_keep_their_contents() {
        let mut g = Game::new(4, false, false);
        let start = std::time::Instant::now();
        while !g.world.chunks.contains_key(&(0, 0)) && start.elapsed().as_secs() < 20 {
            g.world.stream(&[(Vec3::ZERO, 1)]);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let p = IVec3::new(3, 90, 3);
        g.world.set_v(p, HOLLOW_BOX);
        let c = g.world.containers.get_mut(&p).expect("a box is a container");
        c.slots[5] = Some((DIAMOND, 12));
        c.slots[9] = Some((TORCH, 3));
        assert!(g.pack_box(p));
        g.world.set_v(p, AIR);
        let drop = g.drops.iter().find(|d| d.item == HOLLOW_BOX).expect("the box dropped");
        let wear = drop.wear;
        assert_ne!(box_id(wear), 0);
        assert_eq!(g.box_count(wear), 15);
        // Saved and loaded.
        let back = decode(&encode(&g.boxes));
        assert_eq!(back.len(), 1);
        // Placed somewhere else: everything's there.
        let q = IVec3::new(8, 90, 2);
        g.world.set_v(q, HOLLOW_BOX);
        g.unpack_box(q, wear);
        let c = &g.world.containers[&q];
        assert_eq!(c.slots[5], Some((DIAMOND, 12)));
        assert_eq!(c.slots[9], Some((TORCH, 3)));
        assert!(g.boxes.is_empty());
        // An empty box drops a plain one.
        g.world.set_v(IVec3::new(0, 90, 0), HOLLOW_BOX);
        g.pack_box(IVec3::new(0, 90, 0));
        assert!(g.drops.iter().any(|d| d.item == HOLLOW_BOX && d.wear == 0));
    }
}
