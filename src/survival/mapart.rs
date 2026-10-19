//! Map art (v0.8): maps that record what they show.
//!
//! Use an empty **Map** and it becomes a **Recorded Map** of the 64-by-64
//! block square you're standing in (squares line up on a 64-block grid, so
//! maps of neighbouring squares fit together on a wall of item frames). It
//! keeps the colours of the land from above, a pixel a block, and redraws
//! itself while someone's in its square, until it's **locked**: sneak and
//! use it with a Glass Pane to hand, and it never changes again. Use one
//! with a blank Map to hand to make a **copy** (copies are the same map).
//!
//! Maps live where the world lives; joined players are sent each map's
//! pixels when they join and whenever it's redrawn, so a held or framed
//! recorded map looks the same for everyone. An item's wear holds which map it is.

use crate::block::*;
use crate::game::Game;
use macroquad::math::Vec3;
use std::collections::BTreeMap;

/// A recorded map is this many blocks (and pixels) each way.
pub const ART: i32 = 64;
const PARCHMENT: [u8; 3] = [196, 178, 140];
/// How often unlocked maps with someone in their square are redrawn.
const REDRAW_SECS: f32 = 2.0;

/// What using a map does: what to say, the items taken and given (item, wear).
pub type MapUse = (String, Vec<(Id, u32)>, Vec<(Id, u32)>);

#[derive(Clone, Debug, PartialEq)]
pub struct MapArt {
    /// The north-west corner of its square.
    pub origin: (i32, i32),
    pub locked: bool,
    /// ART x ART colours, rows north to south.
    pub px: Vec<[u8; 3]>,
}

/// The square (its north-west corner) that `p` is in.
pub fn square_of(p: Vec3) -> (i32, i32) {
    ((p.x.floor() as i32).div_euclid(ART) * ART, (p.z.floor() as i32).div_euclid(ART) * ART)
}

/// What the land in the square looks like now (unloaded columns keep `old`'s colours).
pub fn record(world: &crate::world::World, origin: (i32, i32), colors: &[[u8; 3]], old: Option<&[[u8; 3]]>) -> Vec<[u8; 3]> {
    let mut px = Vec::with_capacity((ART * ART) as usize);
    for row in 0..ART {
        for col in 0..ART {
            let i = (row * ART + col) as usize;
            let keep = old.and_then(|o| o.get(i).copied()).unwrap_or(PARCHMENT);
            px.push(crate::navigation::column_colour(world, origin.0 + col, origin.1 + row, colors).unwrap_or(keep));
        }
    }
    px
}

pub fn encode(maps: &BTreeMap<u32, MapArt>) -> Vec<u8> {
    let mut v = Vec::new();
    for (id, m) in maps {
        v.extend_from_slice(&id.to_le_bytes());
        v.extend_from_slice(&m.origin.0.to_le_bytes());
        v.extend_from_slice(&m.origin.1.to_le_bytes());
        v.push(m.locked as u8);
        v.extend(m.px.iter().flatten());
    }
    v
}

pub fn decode(b: &[u8]) -> BTreeMap<u32, MapArt> {
    let n = 13 + (ART * ART * 3) as usize;
    b.chunks_exact(n)
        .map(|c| {
            let u = |i: usize| u32::from_le_bytes([c[i], c[i + 1], c[i + 2], c[i + 3]]);
            let px = c[13..].chunks_exact(3).map(|p| [p[0], p[1], p[2]]).collect();
            (u(0), MapArt { origin: (u(4) as i32, u(8) as i32), locked: c[12] != 0, px })
        })
        .collect()
}

impl Game {
    /// The network message carrying map `id` (host -> client).
    pub fn map_art_msg(&self, id: u32) -> Option<crate::net::Msg> {
        let m = self.map_art.get(&id)?;
        Some(crate::net::Msg::MapArt { id, x: m.origin.0, z: m.origin.1, locked: m.locked, px: m.px.iter().flatten().copied().collect() })
    }

    /// A joined player was sent a map.
    pub fn got_map_art(&mut self, id: u32, x: i32, z: i32, locked: bool, px: Vec<u8>) {
        if px.len() == (ART * ART * 3) as usize {
            let px = px.chunks_exact(3).map(|p| [p[0], p[1], p[2]]).collect();
            self.map_art.insert(id, MapArt { origin: (x, z), locked, px });
            self.map_art_version += 1;
        }
    }

    fn share_map(&mut self, id: u32) {
        if let Some(m) = self.map_art_msg(id) {
            self.net_broadcast_all(m);
        }
        self.map_art_version += 1;
    }

    /// Record a new map of the square at `at`; its id.
    pub fn new_map_art(&mut self, at: Vec3) -> u32 {
        let id = self.map_art.keys().next_back().map_or(1, |k| k + 1);
        let origin = square_of(at);
        let px = record(&self.world, origin, &self.map_colors, None);
        self.map_art.insert(id, MapArt { origin, locked: false, px });
        self.share_map(id);
        id
    }

    /// Where the world lives: what `who` (a player at `at`) does with the
    /// map in hand. A blank Map is recorded; a recorded one is copied (with a
    /// blank Map to hand) or, sneaking with a Glass Pane to hand, locked.
    /// Returns what to tell them, and the item changes: (take, give).
    pub fn use_map_art(&mut self, held: Id, wear: u32, at: Vec3, sneaking: bool, has: impl Fn(Id) -> bool) -> Option<MapUse> {
        match held {
            MAP => {
                let id = self.new_map_art(at);
                let (x, z) = self.map_art[&id].origin;
                Some((format!("You draw the land from {x}, {z} to {}, {}.", x + ART - 1, z + ART - 1), vec![(MAP, 0)], vec![(RECORDED_MAP, id)]))
            }
            RECORDED_MAP if self.map_art.contains_key(&wear) => {
                if sneaking && has(PANE_FIRST) {
                    let m = self.map_art.get_mut(&wear)?;
                    if m.locked {
                        return Some(("It's already locked.".into(), vec![], vec![]));
                    }
                    m.locked = true;
                    self.share_map(wear);
                    Some(("Locked: this map won't change again.".into(), vec![(PANE_FIRST, 0)], vec![]))
                } else if has(MAP) {
                    Some(("A copy, for the wall.".into(), vec![(MAP, 0)], vec![(RECORDED_MAP, wear)]))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Used a map item (the local player). Joined players ask the host.
    pub fn use_held_map(&mut self) -> bool {
        let held = self.inv.held();
        // (Used on an item frame, it goes in the frame instead.)
        if let Some(crate::game::Target::Block(h)) = &self.target
            && crate::decor::is_frame(self.world.get_v(h.pos))
        {
            return false;
        }
        if !matches!(held, MAP | RECORDED_MAP) || (held == MAP && !self.player.sneaking) {
            // (A blank Map is recorded by sneaking and using it; held, it's the live map.)
            return false;
        }
        let wear = self.inv.wear[self.inv.selected];
        if self.is_client() {
            self.net_send_msg(crate::net::Msg::UseMap { item: held, wear, sneaking: self.player.sneaking });
            self.player.swing = 1.0;
            return true;
        }
        let at = self.player.body.pos;
        let sneaking = self.player.sneaking;
        let counts: Vec<(Id, u32)> = self.inv.slots.iter().flatten().map(|s| (s.0, s.1 as u32)).collect();
        let has = |item: Id| counts.iter().any(|&(i, n)| i == item && (item != held || n > 1));
        let Some((line, take, give)) = self.use_map_art(held, wear, at, sneaking, has) else { return false };
        for (item, _) in take {
            if item == held {
                self.inv.consume_held();
            } else {
                self.inv.remove(item, 1);
            }
        }
        for (item, w) in give {
            self.inv.add_worn(item, 1, w);
        }
        self.msg(line);
        self.player.swing = 1.0;
        self.advance("cartographer_of_note");
        true
    }

    /// A joined player used a map (see `use_held_map`); the host checks they
    /// have what it takes and does it.
    pub fn peer_used_map(&mut self, from: u32, item: Id, wear: u32, sneaking: bool) {
        let Some(at) = self.peer_ref(from).map(|p| p.target) else { return };
        if !self.peer_has(from, item) || (item == MAP && !sneaking) {
            return;
        }
        let owned: Vec<Id> = [MAP, PANE_FIRST].into_iter().filter(|&i| self.peer_has(from, i)).collect();
        let has = |i: Id| owned.contains(&i);
        let Some((line, take, give)) = self.use_map_art(item, wear, at, sneaking, has) else { return };
        for (i, _) in take {
            self.take_peer(from, i, 1);
        }
        for (i, w) in give {
            self.give_peer_worn(from, i, 1, w);
        }
        self.system_message(Some(from), &line);
        let who = self.peer_name(from);
        self.advance_for(&crate::players::record_key(&who), "cartographer_of_note");
    }

    /// Where the world lives: redraw unlocked maps whose square has someone in it.
    pub fn map_art_tick(&mut self, dt: f32) {
        if self.is_client() || self.map_art.is_empty() {
            return;
        }
        self.map_art_timer -= dt;
        if self.map_art_timer > 0.0 {
            return;
        }
        self.map_art_timer = REDRAW_SECS;
        let mut here: Vec<(i32, i32)> = self.peers.values().map(|p| square_of(p.target)).collect();
        if !self.away() {
            here.push(square_of(self.player.body.pos));
        }
        let due: Vec<u32> = self.map_art.iter().filter(|(_, m)| !m.locked && here.contains(&m.origin)).map(|(&id, _)| id).collect();
        for id in due {
            let m = &self.map_art[&id];
            let px = record(&self.world, m.origin, &self.map_colors, Some(&m.px));
            if px != m.px {
                self.map_art.get_mut(&id).unwrap().px = px;
                self.share_map(id);
            }
        }
    }

    /// A recorded map's colour at a cell (for drawing), parchment if unknown.
    pub fn map_art_pixel(&self, id: u32, col: i32, row: i32) -> [u8; 3] {
        self.map_art.get(&id).and_then(|m| m.px.get((row * ART + col) as usize).copied()).unwrap_or(PARCHMENT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use macroquad::math::ivec3;

    fn game() -> Game {
        let mut g = crate::game::tests::arena(850);
        g.map_colors = (0..NUM_BLOCKS).map(|i| [(i % 251) as u8, 7, 9]).collect();
        g
    }

    #[test]
    fn a_map_records_its_square_and_locks() {
        let mut g = game();
        g.player.sneaking = true;
        g.inv.slots[g.inv.selected] = Some((MAP, 2));
        assert!(g.use_held_map());
        assert_eq!(g.inv.count(MAP), 1);
        let slot = g.inv.slots.iter().position(|s| s.is_some_and(|s| s.0 == RECORDED_MAP)).unwrap();
        let id = g.inv.wear[slot];
        let m = g.map_art[&id].clone();
        assert_eq!(m.origin, square_of(g.player.body.pos));
        // Something new on the land shows up on the next redraw...
        let p = ivec3(m.origin.0 + 10, 60, m.origin.1 + 20);
        g.world.set_v(p, GOLD_BLOCK);
        g.map_art_timer = 0.0;
        g.map_art_tick(0.1);
        let gold = crate::navigation::column_colour(&g.world, p.x, p.z, &g.map_colors).unwrap();
        assert_eq!(g.map_art_pixel(id, 10, 20), gold);
        // ...until it's locked.
        g.inv.selected = slot;
        g.inv.add(PANE_FIRST, 1);
        assert!(g.use_held_map());
        assert!(g.map_art[&id].locked);
        assert_eq!(g.inv.count(PANE_FIRST), 0);
        g.world.set_v(p, BRICK);
        g.map_art_timer = 0.0;
        g.map_art_tick(0.1);
        assert_eq!(g.map_art_pixel(id, 10, 20), gold, "locked maps keep their picture");
        // A copy, with the blank map left over: the same map.
        g.player.sneaking = false;
        assert!(g.use_held_map());
        assert_eq!(g.inv.count(RECORDED_MAP), 2);
        assert_eq!(g.inv.count(MAP), 0);
        assert!(g.advancements.has("cartographer_of_note"));
        // And it's kept with the world.
        assert_eq!(decode(&encode(&g.map_art)), g.map_art);
    }
}
