//! Chests and furnaces: blocks that hold items.
//!
//! Contents live wherever the world lives (single player, host, server) in
//! `World::containers`, keyed by position, and are saved with the world.
//! Joined players see a copy of whatever they have open, and every move
//! between a container and their inventory is done by the host against its
//! ledger (see ledger.rs), so containers can't be used to conjure items.
//!
//! Furnaces cook on the world's owner whenever their chunk is loaded:
//! one item every `COOK_SECS` while there's fuel, glowing while lit.

use crate::block::*;
use crate::game::Game;
use crate::inventory::{click_stack, right_click_stack, Stack};
use crate::net::Msg;
use crate::sound::{Mat, Sfx};
use macroquad::math::{IVec3, Vec3};

pub const CHEST_SLOTS: usize = 27;
/// Furnace slots.
pub const INPUT: usize = 0;
pub const FUEL: usize = 1;
pub const OUTPUT: usize = 2;
/// Seconds to cook one item.
pub const COOK_SECS: f32 = 8.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Container {
    pub slots: Vec<Stack>,
    /// Furnace: seconds of fuel left, how long the current fuel lasts, and
    /// progress on the current item (seconds).
    pub burn: f32,
    pub burn_total: f32,
    pub cook: f32,
}

pub fn is_container(id: Id) -> bool {
    matches!(id, CHEST | FURNACE | FURNACE_LIT)
}

pub fn is_furnace(id: Id) -> bool {
    matches!(id, FURNACE | FURNACE_LIT)
}

/// What cooking `id` makes.
pub fn smelt(id: Id) -> Option<Id> {
    Some(match id {
        PORKCHOP => COOKED_CHOP,
        MUTTON => COOKED_MUTTON,
        CLUCKETS => COOKED_CLUCKETS,
        MOO_STEAK => STEAK,
        COD => COOKED_COD,
        SALMON => COOKED_SALMON,
        POTATO => BAKED_POTATO,
        PUFFER => COOKED_PUFFER,
        BOOT => COOKED_BOOT,
        SAND => GLASS,
        COBBLE => STONE,
        LOG => COAL, // charcoal, legally distinct
        _ => return None,
    })
}

/// Seconds of burning one of `id` gives.
pub fn fuel_secs(id: Id) -> Option<f32> {
    Some(match id {
        COAL => 80.0,
        LOG | PLANKS | TABLE | BOOKSHELF | CHEST | SCARECROW => 15.0,
        HAY => 45.0,
        STICK | WHEAT => 5.0,
        BOW | ROD | HOE | PICK_WOOD | SWORD_WOOD => 10.0,
        _ => return None,
    })
}

/// Can `item` go into slot `slot` of a container of block `kind`?
pub fn accepts(kind: Id, slot: usize, item: Id) -> bool {
    if !is_furnace(kind) {
        return slot < CHEST_SLOTS;
    }
    match slot {
        INPUT => true,
        FUEL => fuel_secs(item).is_some(),
        _ => false, // the output is take-only
    }
}

impl Container {
    pub fn for_block(id: Id) -> Container {
        let n = if is_furnace(id) { 3 } else { CHEST_SLOTS };
        Container { slots: vec![None; n], burn: 0.0, burn_total: 0.0, cook: 0.0 }
    }

    /// Everything inside (for spilling when it's broken).
    pub fn contents(&self) -> Vec<(Id, u8)> {
        self.slots.iter().flatten().copied().collect()
    }

    /// Furnace: can the input be cooked into the output right now?
    fn can_cook(&self) -> bool {
        let Some((input, _)) = self.slots[INPUT] else { return false };
        let Some(result) = smelt(input) else { return false };
        match self.slots[OUTPUT] {
            None => true,
            Some((out, n)) => out == result && n < max_stack(out),
        }
    }

    /// Run a furnace for `dt` seconds. Returns true if anything visible changed.
    pub fn furnace_tick(&mut self, dt: f32) -> bool {
        let before = (self.slots.clone(), self.burn > 0.0, (self.cook * 4.0) as i32, (self.burn * 2.0) as i32);
        let cookable = self.can_cook();
        if self.burn > 0.0 {
            self.burn = (self.burn - dt).max(0.0);
        }
        if self.burn <= 0.0 && cookable {
            // Light the next piece of fuel.
            if let Some((fuel, n)) = self.slots[FUEL]
                && let Some(secs) = fuel_secs(fuel)
            {
                self.slots[FUEL] = if n > 1 { Some((fuel, n - 1)) } else { None };
                self.burn = secs;
                self.burn_total = secs;
            }
        }
        if self.burn > 0.0 && cookable {
            self.cook += dt;
            if self.cook >= COOK_SECS {
                self.cook = 0.0;
                if let (Some((input, n)), Some(result)) = (self.slots[INPUT], self.slots[INPUT].and_then(|s| smelt(s.0))) {
                    self.slots[INPUT] = if n > 1 { Some((input, n - 1)) } else { None };
                    self.slots[OUTPUT] = Some((result, self.slots[OUTPUT].map(|o| o.1).unwrap_or(0) + 1));
                }
            }
        } else {
            // Unattended half-cooked food cools down.
            self.cook = (self.cook - dt * 2.0).max(0.0);
        }
        before != (self.slots.clone(), self.burn > 0.0, (self.cook * 4.0) as i32, (self.burn * 2.0) as i32)
    }

    /// 0..1 fuel left and cooking progress, for the UI.
    pub fn gauges(&self) -> (f32, f32) {
        let burn = if self.burn_total > 0.0 { self.burn / self.burn_total } else { 0.0 };
        (burn.clamp(0.0, 1.0), (self.cook / COOK_SECS).clamp(0.0, 1.0))
    }
}

/// The item moves that turn slot `before` into `after`: (item, count, into the container?).
pub fn moves(before: Stack, after: Stack) -> Vec<(Id, u8, bool)> {
    match (before, after) {
        (Some((a, an)), Some((b, bn))) if a == b => {
            if bn > an {
                vec![(a, bn - an, true)]
            } else if an > bn {
                vec![(a, an - bn, false)]
            } else {
                vec![]
            }
        }
        (b, a) => {
            let mut v = Vec::new();
            if let Some((id, n)) = b {
                v.push((id, n, false));
            }
            if let Some((id, n)) = a {
                v.push((id, n, true));
            }
            v
        }
    }
}

// ------------------------------------------------------------------ saving

pub fn encode(containers: &std::collections::HashMap<IVec3, Container>) -> Vec<u8> {
    let mut out = Vec::new();
    let mut entries: Vec<_> = containers.iter().collect();
    entries.sort_by_key(|(p, _)| (p.x, p.y, p.z));
    for (p, c) in entries {
        for v in [p.x, p.y, p.z] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for v in [c.burn, c.burn_total, c.cook] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.push(c.slots.len() as u8);
        for s in &c.slots {
            let (id, n) = s.unwrap_or((0, 0));
            out.extend_from_slice(&id.to_le_bytes());
            out.push(n);
        }
    }
    out
}

/// Unpack `encode`'s output; stops at anything malformed.
pub fn decode(b: &[u8]) -> std::collections::HashMap<IVec3, Container> {
    let mut map = std::collections::HashMap::new();
    let mut i = 0;
    let rd = |o: usize, b: &[u8]| -> Option<[u8; 4]> { b.get(o..o + 4)?.try_into().ok() };
    while i + 25 <= b.len() {
        let (Some(x), Some(y), Some(z)) = (rd(i, b), rd(i + 4, b), rd(i + 8, b)) else { break };
        let f = |o: usize| rd(o, b).map(f32::from_le_bytes).filter(|v| v.is_finite()).unwrap_or(0.0).clamp(0.0, 1000.0);
        let (burn, burn_total, cook) = (f(i + 12), f(i + 16), f(i + 20));
        let n = b[i + 24] as usize;
        i += 25;
        if n > CHEST_SLOTS || i + n * 3 > b.len() {
            break;
        }
        let slots = (0..n)
            .map(|k| {
                let o = i + k * 3;
                let id = u16::from_le_bytes([b[o], b[o + 1]]);
                let c = b[o + 2];
                (c > 0 && valid_item(id)).then_some((id, c.min(64)))
            })
            .collect();
        i += n * 3;
        map.insert(IVec3::new(i32::from_le_bytes(x), i32::from_le_bytes(y), i32::from_le_bytes(z)), Container { slots, burn, burn_total, cook });
    }
    map
}

// ------------------------------------------------------------------ in the game

/// How close you must be to use a container (a little over reach, for lag).
const REACH: f32 = 10.0;

impl Game {
    /// Right-clicked a chest or furnace: open it.
    pub fn open_container(&mut self, pos: IVec3) {
        self.open = Some(pos);
        self.sfx(Sfx::Place(Mat::Wood), Some(pos.as_vec3() + Vec3::splat(0.5)));
        if self.is_client() {
            // The host has the contents; it sends them (and any changes) while it's open.
            self.net_send_msg(Msg::OpenContainer { x: pos.x, y: pos.y, z: pos.z });
        }
    }

    pub fn close_container(&mut self) {
        self.inv.return_cursor();
        if let Some(p) = self.open.take()
            && self.is_client()
        {
            self.net_send_msg(Msg::CloseContainer { x: p.x, y: p.y, z: p.z });
        }
    }

    /// Clicked slot `slot` of the open container (right: one at a time / half;
    /// shift: move the whole stack to the inventory).
    pub fn container_click(&mut self, slot: usize, right: bool, shift: bool) {
        let Some(pos) = self.open else { return };
        let kind = self.world.get_v(pos);
        let Some(c) = self.world.containers.get_mut(&pos) else { return };
        let Some(&before) = c.slots.get(slot) else { return };
        let mut s = before;
        let mut cursor = self.inv.cursor;
        if shift {
            if let Some((id, n)) = s {
                let left = self.inv.add(id, n);
                s = if left > 0 { Some((id, left)) } else { None };
            }
        } else {
            // Only what the slot accepts may go in (the output is take-only).
            let put_ok = cursor.map(|(id, _)| accepts(kind, slot, id)).unwrap_or(true);
            let same = matches!((cursor, s), (Some((a, _)), Some((b, _))) if a == b);
            if !(put_ok || cursor.is_none() || (slot == OUTPUT && is_furnace(kind))) {
                return;
            }
            if is_furnace(kind) && slot == OUTPUT {
                // Take-only: grab it all (or top up a matching stack in the cursor).
                match (s, cursor) {
                    (Some(st), None) => {
                        cursor = Some(st);
                        s = None;
                    }
                    (Some((id, n)), Some((cid, cn))) if id == cid => {
                        let room = max_stack(id).saturating_sub(cn).min(n);
                        cursor = Some((id, cn + room));
                        s = if n - room > 0 { Some((id, n - room)) } else { None };
                    }
                    _ => return,
                }
            } else if right {
                right_click_stack(&mut s, &mut cursor);
            } else if put_ok || same {
                click_stack(&mut s, &mut cursor);
            }
        }
        c.slots[slot] = s;
        self.inv.cursor = cursor;
        self.dirty_containers.insert(pos);
        if self.is_client() {
            for (item, n, put) in moves(before, s) {
                self.net_send_msg(Msg::ContainerMove { x: pos.x, y: pos.y, z: pos.z, slot: slot as u8, item, n, put });
            }
        }
    }

    /// Shift-clicked an inventory slot while a container is open: send it in.
    pub fn container_quick_put(&mut self, inv_slot: usize) {
        let Some(pos) = self.open else { return };
        let Some((id, n)) = self.inv.slots.get(inv_slot).copied().flatten() else { return };
        let kind = self.world.get_v(pos);
        let Some(c) = self.world.containers.get_mut(&pos) else { return };
        let mut left = n;
        let mut changed: Vec<(usize, Stack)> = Vec::new();
        // Furnaces: fuel to the fuel slot (unless it's also cookable), the rest to the input.
        let order: Vec<usize> = if is_furnace(kind) {
            if fuel_secs(id).is_some() && smelt(id).is_none() { vec![FUEL] } else { vec![INPUT] }
        } else {
            // Chests: top up matching stacks first, then empty slots.
            let (same, empty): (Vec<usize>, Vec<usize>) = (0..c.slots.len()).filter(|&i| c.slots[i].map(|s| s.0 == id).unwrap_or(true)).partition(|&i| c.slots[i].is_some());
            same.into_iter().chain(empty).collect()
        };
        for i in order {
            if left == 0 || !accepts(kind, i, id) {
                continue;
            }
            let before = c.slots[i];
            let have = match before {
                Some((sid, sn)) if sid == id => sn,
                None => 0,
                _ => continue,
            };
            let put = left.min(max_stack(id).saturating_sub(have));
            if put > 0 {
                c.slots[i] = Some((id, have + put));
                left -= put;
                changed.push((i, before));
            }
        }
        let after: Vec<(usize, Stack, Stack)> = changed.iter().map(|&(i, b)| (i, b, c.slots[i])).collect();
        self.inv.slots[inv_slot] = if left > 0 { Some((id, left)) } else { None };
        self.dirty_containers.insert(pos);
        if self.is_client() {
            for (i, b, a) in after {
                for (item, n, put) in moves(b, a) {
                    self.net_send_msg(Msg::ContainerMove { x: pos.x, y: pos.y, z: pos.z, slot: i as u8, item, n, put });
                }
            }
        }
    }

    /// Joined players: close the screen if the container vanished under us.
    pub fn container_still_there(&self) -> bool {
        self.open.map(|p| is_container(self.world.get_v(p))).unwrap_or(false)
    }

    /// Breaking a container hands its contents to whoever broke it.
    pub fn spill_container(&mut self, pos: IVec3, to_peer: Option<u32>) {
        let Some(c) = self.world.containers.get_mut(&pos) else { return };
        let contents = c.contents();
        c.slots.iter_mut().for_each(|s| *s = None);
        for (item, n) in contents {
            match to_peer {
                Some(id) => self.give_peer(id, item, n),
                None => self.give(item, n),
            }
        }
    }

    // ------------------------------------------------------ the world's owner

    /// Furnaces cook, glow, and tell whoever's watching.
    pub fn container_tick(&mut self, dt: f32) {
        let furnaces: Vec<IVec3> = self.world.containers.keys().copied().filter(|p| self.world.is_loaded(p.x, p.z) && is_furnace(self.world.get_v(*p))).collect();
        for p in furnaces {
            let Some(c) = self.world.containers.get_mut(&p) else { continue };
            if !c.furnace_tick(dt) {
                continue;
            }
            let lit = c.burn > 0.0;
            let want = if lit { FURNACE_LIT } else { FURNACE };
            if self.world.get_v(p) != want {
                self.world.set_v(p, want);
            }
            self.dirty_containers.insert(p);
        }
        self.container_sync_timer += dt;
        if self.container_sync_timer >= 0.25 {
            self.container_sync_timer = 0.0;
            for p in std::mem::take(&mut self.dirty_containers) {
                self.send_container(p, None);
            }
        }
    }

    /// Send a container's contents to everyone looking at it (or just `only`).
    pub fn send_container(&mut self, p: IVec3, only: Option<u32>) {
        let Some(c) = self.world.containers.get(&p) else { return };
        let (burn, cook) = c.gauges();
        let slots = c.slots.iter().map(|s| s.unwrap_or((AIR, 0))).collect();
        let msg = Msg::Container { x: p.x, y: p.y, z: p.z, slots, burn, cook };
        let viewers: Vec<u32> = match only {
            Some(id) => vec![id],
            None => self.viewers.get(&p).map(|v| v.iter().copied().collect()).unwrap_or_default(),
        };
        for id in viewers {
            self.net_send_to(id, msg.clone());
        }
    }

    fn peer_near(&self, from: u32, p: IVec3) -> bool {
        self.peers.get(&from).map(|q| (q.target + Vec3::Y * 1.6).distance(p.as_vec3() + Vec3::splat(0.5)) <= REACH).unwrap_or(false)
    }

    pub fn host_open(&mut self, from: u32, p: IVec3) {
        if !self.peer_near(from, p) || !is_container(self.world.get_v(p)) {
            return;
        }
        self.viewers.entry(p).or_default().insert(from);
        self.send_container(p, Some(from));
    }

    pub fn host_close(&mut self, from: u32, p: IVec3) {
        if let Some(v) = self.viewers.get_mut(&p) {
            v.remove(&from);
            if v.is_empty() {
                self.viewers.remove(&p);
            }
        }
    }

    /// Forget a player who left.
    pub fn forget_viewer(&mut self, from: u32) {
        self.viewers.retain(|_, v| {
            v.remove(&from);
            !v.is_empty()
        });
    }

    /// A joined player moved items into or out of a container slot. Checked
    /// against the container and the player's ledger; on any refusal they get
    /// the true contents back (and the next inventory check fixes their side).
    pub fn host_container_move(&mut self, from: u32, p: IVec3, slot: usize, item: Id, n: u8, put: bool) {
        let open = self.viewers.get(&p).map(|v| v.contains(&from)).unwrap_or(false);
        let kind = self.world.get_v(p);
        let ok = open && self.peer_near(from, p) && n > 0 && valid_item(item) && self.world.containers.get(&p).map(|c| slot < c.slots.len()).unwrap_or(false);
        let done = ok && if put { self.container_put(from, p, kind, slot, item, n) } else { self.container_take(from, p, slot, item, n) };
        if done {
            // Everyone else looking sees it too.
            self.dirty_containers.insert(p);
        } else {
            self.send_container(p, Some(from));
        }
    }

    fn container_put(&mut self, from: u32, p: IVec3, kind: Id, slot: usize, item: Id, n: u8) -> bool {
        if !accepts(kind, slot, item) {
            return false;
        }
        let fits = match self.world.containers.get(&p).and_then(|c| c.slots[slot]) {
            None => n <= max_stack(item),
            Some((id, have)) => id == item && have as u32 + n as u32 <= max_stack(item) as u32,
        };
        if !fits || !self.peer_take(from, item, n as u32) {
            return false;
        }
        let c = self.world.containers.get_mut(&p).expect("checked");
        let have = c.slots[slot].map(|s| s.1).unwrap_or(0);
        c.slots[slot] = Some((item, have + n));
        true
    }

    fn container_take(&mut self, from: u32, p: IVec3, slot: usize, item: Id, n: u8) -> bool {
        let Some(c) = self.world.containers.get_mut(&p) else { return false };
        match c.slots[slot] {
            Some((id, have)) if id == item && have >= n => {
                c.slots[slot] = if have > n { Some((id, have - n)) } else { None };
            }
            _ => return false,
        }
        if !self.creative
            && let Some(peer) = self.peers.get_mut(&from)
        {
            peer.ledger.bag.add(item, n as u32);
        }
        true
    }

    /// Joined players: the host's copy of what's in a container.
    pub fn apply_container(&mut self, p: IVec3, slots: Vec<(Id, u8)>, burn: f32, cook: f32) {
        let kind = self.world.get_v(p);
        let c = self.world.containers.entry(p).or_insert_with(|| Container::for_block(kind));
        c.slots = slots.into_iter().map(|(id, n)| (n > 0 && valid_item(id)).then_some((id, n.min(64)))).collect();
        // Gauges come as fractions; show them against nominal totals.
        c.burn_total = 1.0;
        c.burn = burn.clamp(0.0, 1.0);
        c.cook = cook.clamp(0.0, 1.0) * COOK_SECS;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn furnaces_cook_with_fuel() {
        let mut f = Container::for_block(FURNACE);
        f.slots[INPUT] = Some((PORKCHOP, 3));
        assert!(!f.furnace_tick(1.0) || f.burn == 0.0, "no fuel, no fire");
        assert_eq!(f.cook, 0.0);
        f.slots[FUEL] = Some((PLANKS, 1));
        let mut t = 0.0;
        while t < 30.0 {
            f.furnace_tick(0.1);
            t += 0.1;
        }
        // 15s of plank cooks one chop (8s) and most of the next, which then cools.
        assert_eq!(f.slots[OUTPUT], Some((COOKED_CHOP, 1)));
        assert_eq!(f.slots[INPUT], Some((PORKCHOP, 2)));
        assert_eq!(f.slots[FUEL], None);
        assert_eq!(f.burn, 0.0);
        // Coal lasts 80s: the rest is done.
        f.slots[FUEL] = Some((COAL, 1));
        for _ in 0..400 {
            f.furnace_tick(0.1);
        }
        assert_eq!(f.slots[OUTPUT], Some((COOKED_CHOP, 3)));
        assert_eq!(f.slots[INPUT], None);
        assert!(f.burn > 0.0, "coal keeps burning a while after");
        // Nothing smeltable: fuel isn't wasted.
        let mut g = Container::for_block(FURNACE);
        g.slots[INPUT] = Some((DIAMOND, 1));
        g.slots[FUEL] = Some((COAL, 1));
        g.furnace_tick(1.0);
        assert_eq!(g.slots[FUEL], Some((COAL, 1)));
    }

    #[test]
    fn what_goes_where() {
        assert!(accepts(CHEST, 26, DIAMOND) && !accepts(CHEST, 27, DIAMOND));
        assert!(accepts(FURNACE, INPUT, DIAMOND));
        assert!(accepts(FURNACE, FUEL, COAL) && !accepts(FURNACE, FUEL, DIAMOND));
        assert!(!accepts(FURNACE, OUTPUT, COAL));
        assert_eq!(moves(None, Some((DIRT, 3))), vec![(DIRT, 3, true)]);
        assert_eq!(moves(Some((DIRT, 5)), Some((DIRT, 2))), vec![(DIRT, 3, false)]);
        assert_eq!(moves(Some((DIRT, 5)), Some((SAND, 1))), vec![(DIRT, 5, false), (SAND, 1, true)]);
        assert!(moves(Some((DIRT, 5)), Some((DIRT, 5))).is_empty());
    }

    #[test]
    fn containers_save() {
        let mut map = std::collections::HashMap::new();
        let mut c = Container::for_block(CHEST);
        c.slots[0] = Some((DIAMOND, 7));
        c.slots[26] = Some((STICK, 64));
        map.insert(IVec3::new(-3, 60, 1_000_000), c);
        let mut f = Container::for_block(FURNACE);
        f.slots[INPUT] = Some((SAND, 12));
        f.burn = 40.0;
        f.burn_total = 80.0;
        f.cook = 3.5;
        map.insert(IVec3::new(0, 1, 2), f);
        assert_eq!(decode(&encode(&map)), map);
        assert!(decode(&[1, 2, 3]).is_empty());
    }
}
