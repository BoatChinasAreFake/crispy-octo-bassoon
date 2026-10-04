//! Personal chests: a chest that opens to the same storage of your own
//! wherever you put one down (Minecraft's ender chest, legally distinct).
//!
//! Craft one from eight obsidian round a Staring Eye. Everyone who opens one
//! sees their own things, and nobody else's; breaking it loses nothing. What's
//! inside is kept with the world, per player (by name), so it's there the
//! next time you join.
//!
//! Inside, each player's storage is a container like any chest, found under
//! a key that isn't a real block position (like a chest cart's; see
//! containers.rs), so opening, moving items and the host's checks all work
//! the same way.

use crate::block::*;
use crate::containers::Container;
use crate::game::Game;
use crate::sound::{Mat, Sfx};
use crate::world::World;
use macroquad::math::{IVec3, Vec3};
use std::collections::HashMap;

const STASH_X: i32 = i32::MIN + 2;
const STASH_Y: i32 = -4097;
/// How near a Personal Chest a player must be to use theirs.
pub const REACH: i32 = 6;

/// The container key of `name`'s storage.
pub fn stash_key(name: &str) -> IVec3 {
    let key = crate::players::record_key(name);
    // FNV-1a: stable across runs and machines.
    let mut h: u32 = 0x811C_9DC5;
    for b in key.bytes() {
        h = (h ^ b as u32).wrapping_mul(0x0100_0193);
    }
    IVec3::new(STASH_X, STASH_Y, h as i32)
}

/// Whose storage a container key is, if it's anyone's.
pub fn stash_of_key(p: IVec3) -> Option<i32> {
    (p.x == STASH_X && p.y == STASH_Y).then_some(p.z)
}

/// Is there a Personal Chest within reach of `at`?
pub fn near_personal_chest(world: &World, at: Vec3) -> bool {
    let c = at.floor().as_ivec3();
    (-REACH..=REACH).any(|dy| (-REACH..=REACH).any(|dz| (-REACH..=REACH).any(|dx| world.get_v(c + IVec3::new(dx, dy, dz)) == PERSONAL_CHEST)))
}

/// Everyone's storage for the save.
pub fn encode(stashes: &HashMap<i32, Container>) -> Vec<u8> {
    crate::containers::encode(&stashes.iter().map(|(&k, c)| (IVec3::new(k, 0, 0), c.clone())).collect())
}

pub fn decode(b: &[u8]) -> HashMap<i32, Container> {
    crate::containers::decode(b, 4).into_iter().map(|(p, c)| (p.x, c)).collect()
}

impl Game {
    /// Right-clicked a Personal Chest: open our own storage.
    pub fn open_stash(&mut self, chest: IVec3) {
        let key = stash_key(&self.player_name);
        if !self.is_client() {
            self.world.stashes.entry(key.z).or_insert_with(|| Container::for_block(CHEST));
        }
        self.open = Some(key);
        self.sfx(Sfx::Place(Mat::Stone), Some(chest.as_vec3() + Vec3::splat(0.5)));
        if self.is_client() {
            self.net_send_msg(crate::net::Msg::OpenContainer { x: key.x, y: key.y, z: key.z });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::tests::arena;

    #[test]
    fn personal_chests_open_the_same_storage_anywhere() {
        let mut g = arena(91);
        let base = g.player.body.pos.floor().as_ivec3();
        let (a, b) = (base + IVec3::new(2, 0, 0), base + IVec3::new(-2, 0, 0));
        g.world.set_v(a, PERSONAL_CHEST);
        g.world.set_v(b, PERSONAL_CHEST);
        g.open_stash(a);
        let key = g.open.unwrap();
        assert_eq!(stash_of_key(key), Some(stash_key(&g.player_name).z));
        assert!(g.container_still_there());
        g.inv.slots[0] = Some((DIAMOND, 5));
        g.inv.click(0);
        g.container_click(3, false, false);
        g.close_container();
        // The other one has it too, and breaking a chest loses nothing.
        g.world.set_v(a, AIR);
        g.open_stash(b);
        let c = crate::containers::store_ref(&g.world, &g.vehicles, g.open.unwrap()).unwrap();
        assert_eq!(c.slots[3], Some((DIAMOND, 5)));
        // Someone else's is their own.
        assert_ne!(stash_key("Someone"), stash_key(&g.player_name));
        let back = decode(&encode(&g.world.stashes));
        assert_eq!(back.get(&key.z).and_then(|c| c.slots[3]), Some((DIAMOND, 5)));
    }
}
