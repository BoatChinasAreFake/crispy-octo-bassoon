//! **Backpacks**: a Bundle grown up. Hold one and right-click to open your
//! pack, a chest you carry with you.
//!
//! - A **Backpack** (an empty Bundle, four Wool and two String) reaches 27
//!   slots of it; a **Big Backpack** (a Backpack, four Iron, two Wool) 45;
//!   a **Huge Backpack** (a Big one, four Gold and two Diamonds) all 72.
//! - The pack is yours, not the bag's: every backpack you hold opens the
//!   same one (like a Personal Chest), so upgrading loses nothing and a
//!   dropped backpack doesn't spill your things. What's in it is kept with
//!   the world, per player.
//!
//! Inside, a pack is a container under a key that isn't a real block (like a
//! Personal Chest's; see stash.rs), so the chest screen, moving things and
//! the host's checks for joined players all work as they do for chests.

use crate::block::*;
use crate::containers::Container;
use crate::game::Game;
use crate::sound::{Mat, Sfx};
use macroquad::math::IVec3;
use std::collections::HashMap;

const PACK_X: i32 = i32::MIN + 3;

/// Is this a backpack, and which (1 to 3)?
pub fn tier(item: Id) -> Option<u8> {
    match item {
        BACKPACK => Some(1),
        BIG_BACKPACK => Some(2),
        HUGE_BACKPACK => Some(3),
        _ => None,
    }
}

/// The chest a pack opened with a backpack of tier `t` works like (its size).
pub fn chest_of(t: u8) -> Id {
    match t {
        1 => CHEST,
        2 => IRON_CHEST,
        _ => DIAMOND_CHEST,
    }
}

/// The container key of `name`'s pack, opened with a backpack of tier `t`.
pub fn pack_key(name: &str, t: u8) -> IVec3 {
    IVec3::new(PACK_X, t as i32, crate::stash::stash_key(name).z)
}

/// Whose pack a key is (and through which tier of backpack).
pub fn pack_of_key(p: IVec3) -> Option<(i32, u8)> {
    (p.x == PACK_X && (1..=3).contains(&p.y)).then_some((p.z, p.y as u8))
}

/// A fresh pack: room for the biggest backpack.
pub fn fresh() -> Container {
    Container::for_block(DIAMOND_CHEST)
}

/// Everyone's packs for the save.
pub fn encode(packs: &HashMap<i32, Container>) -> Vec<u8> {
    crate::containers::encode(&packs.iter().map(|(&k, c)| (IVec3::new(k, 0, 0), c.clone())).collect())
}

pub fn decode(b: &[u8]) -> HashMap<i32, Container> {
    crate::containers::decode(b, 4).into_iter().map(|(p, mut c)| {
        c.slots.resize(72, None);
        c.wear.resize(72, 0);
        (p.x, c)
    }).collect()
}

impl Game {
    /// Right-click with a backpack in hand: open your pack.
    pub fn open_backpack(&mut self) -> bool {
        let Some(t) = tier(self.inv.held()) else { return false };
        let key = pack_key(&self.player_name, t);
        if !self.is_client() {
            self.world.backpacks.entry(key.z).or_insert_with(fresh);
        }
        self.open = Some(key);
        self.player.swing = 1.0;
        self.sfx(Sfx::Place(Mat::Grass), None);
        if self.is_client() {
            self.net_send_msg(crate::net::Msg::OpenContainer { x: key.x, y: key.y, z: key.z });
        }
        true
    }

    /// Is the pack at `key` still ours to look in (a backpack still in hand)?
    pub fn backpack_held(&self, key: IVec3) -> bool {
        pack_of_key(key).is_some_and(|(_, t)| tier(self.inv.held()).is_some_and(|h| h >= t)) && key == pack_key(&self.player_name, key.y as u8)
    }

    /// The host: may joined player `from` use the pack at `key`? (It must be
    /// theirs, and they must really be holding a big enough backpack.)
    pub fn peer_backpack(&self, from: u32, key: IVec3) -> bool {
        let Some((_, t)) = pack_of_key(key) else { return false };
        self.peers.get(&from).is_some_and(|q| pack_key(&q.name, t) == key) && tier(self.verified_held(from)).is_some_and(|h| h >= t)
    }

    /// Crafting a Backpack takes an empty Bundle (so nothing in it is lost).
    pub fn bundles_empty(&self) -> bool {
        (0..self.inv.slots.len()).all(|i| self.inv.slots[i].map(|s| s.0) != Some(BUNDLE) || self.bundle_contents(self.inv.wear[i]).is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backpacks_open_your_own_pack_and_bigger_ones_reach_more() {
        let mut g = crate::game::tests::arena(243);
        g.inv.slots[0] = Some((BACKPACK, 1));
        g.inv.selected = 0;
        assert!(g.open_backpack());
        let key = g.open.unwrap();
        assert_eq!(pack_of_key(key).map(|p| p.1), Some(1));
        assert!(g.backpack_held(key) && g.container_still_there());
        let kind = crate::containers::store_kind(&g.world, &g.vehicles, key);
        assert_eq!(crate::chests::slots(kind), 27);
        assert!(crate::containers::accepts(kind, 26, COBBLE) && !crate::containers::accepts(kind, 27, COBBLE));
        // Put something in, then look again through a Huge Backpack: same pack, more room.
        crate::containers::store(&mut g.world, &mut g.vehicles, key).unwrap().slots[3] = Some((DIAMOND, 5));
        g.inv.slots[0] = Some((HUGE_BACKPACK, 1));
        assert!(g.open_backpack());
        let big = g.open.unwrap();
        assert_eq!(crate::chests::slots(crate::containers::store_kind(&g.world, &g.vehicles, big)), 72);
        assert_eq!(crate::containers::store_ref(&g.world, &g.vehicles, big).unwrap().slots[3], Some((DIAMOND, 5)));
        // Put it away and the pack closes.
        g.inv.slots[0] = None;
        assert!(!g.container_still_there());
        // Kept with the world.
        let back = crate::game::Game::from_save(g.to_save());
        assert_eq!(back.world.backpacks[&big.z].slots[3], Some((DIAMOND, 5)));
        // Recipes: Bundle -> Backpack -> Big -> Huge.
        let makes = |out: Id| crate::block::recipes().iter().any(|r| r.output.0 == out);
        assert!(makes(BACKPACK) && makes(BIG_BACKPACK) && makes(HUGE_BACKPACK));
    }
}
