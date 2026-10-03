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
use crate::inventory::{click_stack, right_click_stack, Stack, Wear};
use crate::net::Msg;
use crate::sound::{Mat, Sfx};
use crate::world::World;
use macroquad::math::{IVec3, Vec3};

pub const CHEST_SLOTS: usize = 27;
pub const DISPENSER_SLOTS: usize = 9;
/// Furnace slots.
pub const INPUT: usize = 0;
pub const FUEL: usize = 1;
pub const OUTPUT: usize = 2;
/// Seconds to cook one item.
pub const COOK_SECS: f32 = 8.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Container {
    pub slots: Vec<Stack>,
    /// Wear of any tools or armour stored here (see `durability`).
    pub wear: Vec<Wear>,
    /// Furnace: seconds of fuel left, how long the current fuel lasts, and
    /// progress on the current item (seconds).
    pub burn: f32,
    pub burn_total: f32,
    pub cook: f32,
}

pub fn is_container(id: Id) -> bool {
    matches!(id, CHEST | FURNACE | FURNACE_LIT | BREWING_STAND | HOLLOW_BOX) || crate::contraptions::is_dispenser(id) || crate::contraptions::is_crafter(id) || crate::hoppers::is_hopper(id)
}

/// Furnaces and brewing stands: an input on top, a second slot below
/// (fuel, or the bottle being brewed), and a take-only output.
pub fn is_three_slot(id: Id) -> bool {
    is_furnace(id) || id == BREWING_STAND
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
        LOG | SPRUCE_LOG | JUNGLE_LOG | CHERRY_LOG | MANGROVE_LOG | PALE_OAK_LOG => COAL, // charcoal, legally distinct
        OLD_DEBRIS => SCORCHITE_SCRAP,
        COBBLED_DEEPSLATE => DEEPSLATE,
        // Mod recipes.
        _ => return reg().smelting.iter().find(|r| r.0 == id).map(|r| r.1),
    })
}

/// Seconds of burning one of `id` gives.
pub fn fuel_secs(id: Id) -> Option<f32> {
    Some(match id {
        COAL => 80.0,
        LOG | SPRUCE_LOG | JUNGLE_LOG | CHERRY_LOG | MANGROVE_LOG | PALE_OAK_LOG | PLANKS | CHERRY_PLANKS | MANGROVE_PLANKS | PALE_OAK_PLANKS | TABLE | BOOKSHELF | CHEST | SCARECROW => 15.0,
        HAY => 45.0,
        STICK | WHEAT => 5.0,
        DOOR => 10.0,
        id if slab_of(id).is_some() && made_of(id) == PLANKS => 7.5,
        id if stairs_of(id).is_some() && made_of(id) == PLANKS => 15.0,
        BOW | ROD | HOE | PICK_WOOD | SWORD_WOOD => 10.0,
        // Mod fuels.
        _ => return reg().fuels.iter().find(|f| f.0 == id).map(|f| f.1),
    })
}

/// Can `item` go into slot `slot` of a container of block `kind`?
pub fn accepts(kind: Id, slot: usize, item: Id) -> bool {
    if kind == BREWING_STAND {
        return match slot {
            INPUT => item == GUNPOWDER || crate::potions::BREWABLE.iter().any(|p| p.ingredient() == item),
            FUEL => crate::potions::is_bottle(item),
            _ => false,
        };
    }
    if crate::contraptions::is_dispenser(kind) || crate::contraptions::is_crafter(kind) {
        return slot < DISPENSER_SLOTS;
    }
    if crate::hoppers::is_hopper(kind) {
        return slot < crate::hoppers::SLOTS;
    }
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
        let n = if is_three_slot(id) {
            3
        } else if crate::contraptions::is_dispenser(id) || crate::contraptions::is_crafter(id) {
            DISPENSER_SLOTS
        } else if crate::hoppers::is_hopper(id) {
            crate::hoppers::SLOTS
        } else {
            CHEST_SLOTS
        };
        Container { slots: vec![None; n], wear: vec![0; n], burn: 0.0, burn_total: 0.0, cook: 0.0 }
    }

    /// Everything inside, with its wear (for spilling when it's broken).
    pub fn contents(&self) -> Vec<(Id, u8, Wear)> {
        self.slots.iter().zip(&self.wear).filter_map(|(s, w)| s.map(|(id, n)| (id, n, *w))).collect()
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

    /// Brew for `dt` seconds (a brewing stand: bottle and ingredient into a
    /// potion; `cook` is the progress). Returns true if anything visible changed.
    pub fn brew_tick(&mut self, dt: f32) -> bool {
        let before = (self.slots.clone(), (self.cook * 4.0) as i32);
        let result = match (self.slots[FUEL], self.slots[INPUT]) {
            (Some((bottle, 1)), Some((ingredient, _))) => crate::potions::brew(bottle, ingredient),
            _ => None,
        };
        match result {
            Some(potion) if self.slots[OUTPUT].is_none() => {
                self.cook += dt;
                if self.cook >= crate::potions::BREW_SECS {
                    self.cook = 0.0;
                    if let Some((ingredient, n)) = self.slots[INPUT] {
                        self.slots[INPUT] = if n > 1 { Some((ingredient, n - 1)) } else { None };
                    }
                    self.slots[FUEL] = None;
                    self.slots[OUTPUT] = Some((potion, 1));
                }
            }
            _ => self.cook = 0.0,
        }
        before != (self.slots.clone(), (self.cook * 4.0) as i32)
    }

    /// 0..1 fuel left and cooking progress, for the UI.
    pub fn gauges(&self) -> (f32, f32) {
        let burn = if self.burn_total > 0.0 { self.burn / self.burn_total } else { 0.0 };
        (burn.clamp(0.0, 1.0), (self.cook / COOK_SECS).clamp(0.0, 1.0))
    }
}

/// The item moves that turn slot `before` into `after`: (item, count, into the container?).
/// Tidies slots: plain stacks of the same thing merge, then everything is
/// ordered by item (worn, enchanted or labelled items keep their own slot),
/// with the gaps at the end. Nothing is gained or lost.
pub fn sort_slots(slots: &mut [Stack], wear: &mut [Wear]) {
    let mut items: Vec<(Id, u8, Wear)> = Vec::new();
    for (s, w) in slots.iter().zip(wear.iter()) {
        let Some((id, mut n)) = *s else { continue };
        if *w == 0 {
            for it in items.iter_mut().filter(|it| it.0 == id && it.2 == 0) {
                let room = max_stack(id).saturating_sub(it.1).min(n);
                it.1 += room;
                n -= room;
            }
        }
        if n > 0 {
            items.push((id, n, *w));
        }
    }
    items.sort_by_key(|&(id, n, w)| (id, w, std::cmp::Reverse(n)));
    for i in 0..slots.len() {
        let it = items.get(i);
        slots[i] = it.map(|&(id, n, _)| (id, n));
        wear[i] = it.map(|it| it.2).unwrap_or(0);
    }
}

#[cfg(test)]
mod sort_tests {
    use super::*;

    #[test]
    fn sorting_merges_plain_stacks_and_keeps_worn_ones_apart() {
        let mut slots: Vec<Stack> = vec![Some((DIRT, 40)), None, Some((STONE, 3)), Some((DIRT, 40)), Some((STONE, 1)), None];
        let mut wear: Vec<Wear> = vec![0, 0, 0, 0, 7, 0];
        let before: u32 = slots.iter().flatten().map(|s| s.1 as u32).sum();
        sort_slots(&mut slots, &mut wear);
        let after: u32 = slots.iter().flatten().map(|s| s.1 as u32).sum();
        assert_eq!(before, after);
        let mut want = vec![(DIRT, 64, 0), (DIRT, 16, 0), (STONE, 3, 0), (STONE, 1, 7)];
        want.sort_by_key(|&(id, n, w)| (id, w, std::cmp::Reverse(n)));
        let got: Vec<(Id, u8, Wear)> = slots.iter().zip(&wear).filter_map(|(s, w)| s.map(|(id, n)| (id, n, *w))).collect();
        assert_eq!(got, want);
        assert!(slots[4..].iter().all(|s| s.is_none()));
    }
}

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

/// Save format: v7 saves had no wear, v9 two bytes of it per slot, v11 four
/// (uses and enchantments).
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
        for (s, w) in c.slots.iter().zip(&c.wear) {
            let (id, n) = s.unwrap_or((0, 0));
            out.extend_from_slice(&id.to_le_bytes());
            out.push(n);
            out.extend_from_slice(&w.to_le_bytes());
        }
    }
    out
}

/// Unpack `encode`'s output (`wear_bytes` per slot: 0 before save v9, 2
/// before v11, then 4); stops at anything malformed.
pub fn decode(b: &[u8], wear_bytes: usize) -> std::collections::HashMap<IVec3, Container> {
    let per = 3 + wear_bytes;
    let mut map = std::collections::HashMap::new();
    let mut i = 0;
    let rd = |o: usize, b: &[u8]| -> Option<[u8; 4]> { b.get(o..o + 4)?.try_into().ok() };
    while i + 25 <= b.len() {
        let (Some(x), Some(y), Some(z)) = (rd(i, b), rd(i + 4, b), rd(i + 8, b)) else { break };
        let f = |o: usize| rd(o, b).map(f32::from_le_bytes).filter(|v| v.is_finite()).unwrap_or(0.0).clamp(0.0, 1000.0);
        let (burn, burn_total, cook) = (f(i + 12), f(i + 16), f(i + 20));
        let n = b[i + 24] as usize;
        i += 25;
        if n > CHEST_SLOTS || i + n * per > b.len() {
            break;
        }
        let slots = (0..n)
            .map(|k| {
                let o = i + k * per;
                let id = u16::from_le_bytes([b[o], b[o + 1]]);
                let c = b[o + 2];
                (c > 0 && valid_item(id)).then_some((id, c.min(64)))
            })
            .collect();
        let wear = (0..n)
            .map(|k| {
                let o = i + k * per + 3;
                match wear_bytes {
                    2 => u16::from_le_bytes([b[o], b[o + 1]]) as Wear,
                    4 => u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]),
                    _ => 0,
                }
            })
            .collect();
        i += n * per;
        map.insert(IVec3::new(i32::from_le_bytes(x), i32::from_le_bytes(y), i32::from_le_bytes(z)), Container { slots, wear, burn, burn_total, cook });
    }
    map
}

// ------------------------------------------------------------------ in the game

/// How close you must be to use a container (a little over reach, for lag).
const REACH: f32 = 10.0;

// Containers are found by position: a block's, or a loaded cart's key (see
// `vehicles::cart_key`), so chest and hopper carts work like chests.

/// The container at `p` (a block's, or a cart's).
pub fn store<'a>(world: &'a mut World, vehicles: &'a mut [crate::vehicles::Vehicle], p: IVec3) -> Option<&'a mut Container> {
    if let Some(k) = crate::stash::stash_of_key(p) {
        return world.stashes.get_mut(&k);
    }
    match crate::vehicles::cart_of_key(p) {
        Some(id) => vehicles.iter_mut().find(|v| v.id == id)?.contents.as_mut(),
        None => world.containers.get_mut(&p),
    }
}

pub fn store_ref<'a>(world: &'a World, vehicles: &'a [crate::vehicles::Vehicle], p: IVec3) -> Option<&'a Container> {
    if let Some(k) = crate::stash::stash_of_key(p) {
        return world.stashes.get(&k);
    }
    match crate::vehicles::cart_of_key(p) {
        Some(id) => vehicles.iter().find(|v| v.id == id)?.contents.as_ref(),
        None => world.containers.get(&p),
    }
}

/// The block a container behaves like (a cart's: a chest's or a hopper's).
pub fn store_kind(world: &World, vehicles: &[crate::vehicles::Vehicle], p: IVec3) -> Id {
    if crate::stash::stash_of_key(p).is_some() {
        return PERSONAL_CHEST;
    }
    match crate::vehicles::cart_of_key(p) {
        Some(id) => vehicles.iter().find(|v| v.id == id && v.contents.is_some()).map(|v| v.container_block()).unwrap_or(AIR),
        None => world.get_v(p),
    }
}

/// Where a container is (for reach checks).
pub fn store_centre(vehicles: &[crate::vehicles::Vehicle], p: IVec3) -> Vec3 {
    match crate::vehicles::cart_of_key(p) {
        Some(id) => vehicles.iter().find(|v| v.id == id).map(|v| v.pos + Vec3::Y * 0.5).unwrap_or(Vec3::splat(f32::MAX)),
        None => p.as_vec3() + Vec3::splat(0.5),
    }
}

impl Game {
    /// Right-clicked a chest or furnace: open it.
    pub fn open_container(&mut self, pos: IVec3) {
        if !self.is_client() {
            self.ensure_container(pos);
        }
        self.open = Some(pos);
        self.sfx(Sfx::Place(Mat::Wood), Some(store_centre(&self.vehicles, pos)));
        if self.is_client() {
            // The host has the contents; it sends them (and any changes) while it's open.
            self.net_send_msg(Msg::OpenContainer { x: pos.x, y: pos.y, z: pos.z });
        }
    }

    /// Containers the generator built (a hut's furnace) have no contents
    /// record until someone opens one: start it empty.
    pub fn ensure_container(&mut self, pos: IVec3) {
        if crate::vehicles::cart_of_key(pos).is_none() {
            let id = self.world.get_v(pos);
            if is_container(id) && !self.world.containers.contains_key(&pos) {
                self.world.containers.insert(pos, Container::for_block(id));
            }
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
        let kind = store_kind(&self.world, &self.vehicles, pos);
        let Some(c) = store(&mut self.world, &mut self.vehicles, pos) else { return };
        let Some(&before) = c.slots.get(slot) else { return };
        let mut s = before;
        let mut cursor = self.inv.cursor;
        let before_cursor = cursor;
        if shift {
            if let Some((id, n)) = s {
                let left = self.inv.add_worn(id, n, c.wear[slot]);
                s = if left > 0 { Some((id, left)) } else { None };
            }
        } else {
            // Only what the slot accepts may go in (the output is take-only).
            let put_ok = cursor.map(|(id, _)| accepts(kind, slot, id)).unwrap_or(true);
            let same = matches!((cursor, s), (Some((a, _)), Some((b, _))) if a == b);
            if !(put_ok || cursor.is_none() || (slot == OUTPUT && is_three_slot(kind))) {
                return;
            }
            if is_three_slot(kind) && slot == OUTPUT {
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
        if !shift {
            crate::inventory::wear_follow((before, before_cursor), (s, cursor), &mut c.wear[slot], &mut self.inv.cursor_wear);
        }
        let wear = c.wear[slot];
        self.dirty_containers.insert(pos);
        // Loading a chest cart.
        if s.is_some() && s != before && crate::vehicles::cart_of_key(pos).is_some() && kind == CHEST {
            self.advance("freight");
        }
        if self.is_client() {
            for (item, n, put) in moves(before, s) {
                self.net_send_msg(Msg::ContainerMove { x: pos.x, y: pos.y, z: pos.z, slot: slot as u8, item, n, put, wear });
            }
        } else if is_furnace(kind) && slot == OUTPUT
            && let Some((id, had)) = before
        {
            // Cooking pays a little experience when you take the results.
            let taken = had - s.map(|s| s.1).unwrap_or(0);
            let points = crate::xp::roll(crate::xp::smelt_xp(id) * taken as f32, &mut self.rng);
            self.add_xp(points);
        }
    }

    /// Shift-clicked an inventory slot while a container is open: send it in.
    pub fn container_quick_put(&mut self, inv_slot: usize) {
        let Some(pos) = self.open else { return };
        let Some((id, n)) = self.inv.slots.get(inv_slot).copied().flatten() else { return };
        let kind = store_kind(&self.world, &self.vehicles, pos);
        let Some(c) = store(&mut self.world, &mut self.vehicles, pos) else { return };
        let mut left = n;
        let mut changed: Vec<(usize, Stack)> = Vec::new();
        // Furnaces: fuel to the fuel slot (unless it's also cookable), the rest to the input.
        let order: Vec<usize> = if kind == BREWING_STAND {
            if crate::potions::is_bottle(id) { vec![FUEL] } else { vec![INPUT] }
        } else if is_furnace(kind) {
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
                if have == 0 {
                    c.wear[i] = self.inv.wear[inv_slot];
                }
                left -= put;
                changed.push((i, before));
            }
        }
        let after: Vec<(usize, Stack, Stack, Wear)> = changed.iter().map(|&(i, b)| (i, b, c.slots[i], c.wear[i])).collect();
        self.inv.slots[inv_slot] = if left > 0 { Some((id, left)) } else { None };
        self.dirty_containers.insert(pos);
        if self.is_client() {
            for (i, b, a, wear) in after {
                for (item, n, put) in moves(b, a) {
                    self.net_send_msg(Msg::ContainerMove { x: pos.x, y: pos.y, z: pos.z, slot: i as u8, item, n, put, wear });
                }
            }
        }
    }

    /// The Sort button: tidy the open chest.
    pub fn sort_container(&mut self) {
        let Some(pos) = self.open else { return };
        if is_three_slot(store_kind(&self.world, &self.vehicles, pos)) {
            return;
        }
        if let Some(c) = store(&mut self.world, &mut self.vehicles, pos) {
            sort_slots(&mut c.slots, &mut c.wear);
        }
        self.dirty_containers.insert(pos);
        if self.is_client() {
            self.net_send_msg(Msg::SortContainer { x: pos.x, y: pos.y, z: pos.z });
        }
    }

    pub fn host_sort(&mut self, from: u32, p: IVec3) {
        let open = self.viewers.get(&p).map(|v| v.contains(&from)).unwrap_or(false);
        if !open || !self.peer_near(from, p) || is_three_slot(store_kind(&self.world, &self.vehicles, p)) {
            return;
        }
        if let Some(c) = store(&mut self.world, &mut self.vehicles, p) {
            sort_slots(&mut c.slots, &mut c.wear);
            self.dirty_containers.insert(p);
        }
    }

    /// Joined players: close the screen if the container vanished under us.
    pub fn container_still_there(&self) -> bool {
        self.open
            .map(|p| {
                if crate::stash::stash_of_key(p).is_some() {
                    return crate::stash::near_personal_chest(&self.world, self.player.eye());
                }
                is_container(store_kind(&self.world, &self.vehicles, p)) && (crate::vehicles::cart_of_key(p).is_none() || store_centre(&self.vehicles, p).distance(self.player.eye()) < REACH)
            })
            .unwrap_or(false)
    }

    /// Breaking a container spills what was inside onto the ground.
    pub fn spill_container(&mut self, pos: IVec3) {
        // Hollow Boxes keep theirs (see boxes.rs).
        if self.pack_box(pos) {
            return;
        }
        let Some(c) = self.world.containers.get_mut(&pos) else { return };
        let contents = c.contents();
        c.slots.iter_mut().for_each(|s| *s = None);
        c.wear.iter_mut().for_each(|w| *w = 0);
        for (item, n, wear) in contents {
            self.pop_drop_worn(pos.as_vec3() + Vec3::splat(0.5), item, n, wear);
        }
    }

    // ------------------------------------------------------ the world's owner

    /// Furnaces cook, glow, and tell whoever's watching.
    pub fn container_tick(&mut self, dt: f32) {
        let stands: Vec<IVec3> = self.world.containers.keys().copied().filter(|p| self.world.is_loaded(p.x, p.z) && self.world.get_v(*p) == BREWING_STAND).collect();
        for p in stands {
            if let Some(c) = self.world.containers.get_mut(&p)
                && c.brew_tick(dt)
            {
                self.dirty_containers.insert(p);
            }
        }
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
        let Some(c) = store_ref(&self.world, &self.vehicles, p) else { return };
        let (burn, cook) = c.gauges();
        let slots = c.slots.iter().zip(&c.wear).map(|(s, w)| s.map(|(id, n)| (id, n, *w)).unwrap_or((AIR, 0, 0))).collect();
        let msg = Msg::Container { x: p.x, y: p.y, z: p.z, slots, burn, cook };
        let viewers: Vec<u32> = match only {
            Some(id) => vec![id],
            None => self.viewers.get(&p).map(|v| v.iter().copied().collect()).unwrap_or_default(),
        };
        for id in viewers {
            self.net_send_to(id, msg.clone());
        }
    }

    pub fn peer_near(&self, from: u32, p: IVec3) -> bool {
        // Someone's own Personal Chest storage: only theirs, and only by a Personal Chest.
        if crate::stash::stash_of_key(p).is_some() {
            return self.peers.get(&from).is_some_and(|q| crate::stash::stash_key(&q.name) == p && crate::stash::near_personal_chest(&self.world, q.target + Vec3::Y * 1.6));
        }
        self.peers.get(&from).map(|q| (q.target + Vec3::Y * 1.6).distance(store_centre(&self.vehicles, p)) <= REACH).unwrap_or(false)
    }

    pub fn host_open(&mut self, from: u32, p: IVec3) {
        let kind = store_kind(&self.world, &self.vehicles, p);
        if !self.peer_near(from, p) || !(is_container(kind) || kind == PERSONAL_CHEST) {
            return;
        }
        if let Some(k) = crate::stash::stash_of_key(p) {
            self.world.stashes.entry(k).or_insert_with(|| Container::for_block(CHEST));
        }
        self.ensure_container(p);
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
    #[allow(clippy::too_many_arguments)]
    pub fn host_container_move(&mut self, from: u32, p: IVec3, slot: usize, item: Id, n: u8, put: bool, wear: Wear) {
        let open = self.viewers.get(&p).map(|v| v.contains(&from)).unwrap_or(false);
        let kind = store_kind(&self.world, &self.vehicles, p);
        let ok = open && self.peer_near(from, p) && n > 0 && valid_item(item) && store_ref(&self.world, &self.vehicles, p).map(|c| slot < c.slots.len()).unwrap_or(false);
        let done = ok && if put { self.container_put(from, p, kind, slot, item, n, wear) } else { self.container_take(from, p, slot, item, n) };
        if done {
            // Everyone else looking sees it too.
            self.dirty_containers.insert(p);
        } else {
            self.send_container(p, Some(from));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn container_put(&mut self, from: u32, p: IVec3, kind: Id, slot: usize, item: Id, n: u8, wear: Wear) -> bool {
        if !accepts(kind, slot, item) {
            return false;
        }
        let fits = match store_ref(&self.world, &self.vehicles, p).and_then(|c| c.slots[slot]) {
            None => n <= max_stack(item),
            Some((id, have)) => id == item && have as u32 + n as u32 <= max_stack(item) as u32,
        };
        if !fits || !self.peer_take(from, item, n as u32) {
            return false;
        }
        let wear = self.launder(from, item, wear);
        let c = store(&mut self.world, &mut self.vehicles, p).expect("checked");
        let have = c.slots[slot].map(|s| s.1).unwrap_or(0);
        c.slots[slot] = Some((item, have + n));
        if have == 0 {
            // Their word for how worn it is (wear is only cosmetic to the host: see `host_wear`).
            c.wear[slot] = crate::inventory::sanitize_wear(item, wear);
        }
        true
    }

    fn container_take(&mut self, from: u32, p: IVec3, slot: usize, item: Id, n: u8) -> bool {
        let furnace = is_furnace(store_kind(&self.world, &self.vehicles, p));
        let Some(c) = store(&mut self.world, &mut self.vehicles, p) else { return false };
        let ench = (c.wear.get(slot).copied().unwrap_or(0) >> 16) as u16;
        match c.slots[slot] {
            Some((id, have)) if id == item && have >= n => {
                c.slots[slot] = if have > n { Some((id, have - n)) } else { None };
            }
            _ => return false,
        }
        if !self.peer_free(from)
            && let Some(peer) = self.peers.get_mut(&from)
        {
            peer.ledger.bag.add(item, n as u32);
            peer.ledger.add_enchanted(item, ench, n as u32);
        }
        // Cooking pays a little experience when the results are taken.
        if furnace && slot == OUTPUT {
            let points = crate::xp::roll(crate::xp::smelt_xp(item) * n as f32, &mut self.rng);
            self.give_peer_xp(from, points);
        }
        true
    }

    /// Joined players: the host's copy of what's in a container.
    pub fn apply_container(&mut self, p: IVec3, slots: Vec<(Id, u8, Wear)>, burn: f32, cook: f32) {
        let c = if let Some(k) = crate::stash::stash_of_key(p) {
            self.world.stashes.entry(k).or_insert_with(|| Container::for_block(CHEST))
        } else if crate::vehicles::cart_of_key(p).is_some() {
            let Some(c) = store(&mut self.world, &mut self.vehicles, p) else { return };
            c
        } else {
            let kind = self.world.get_v(p);
            self.world.containers.entry(p).or_insert_with(|| Container::for_block(kind))
        };
        c.wear = slots.iter().map(|s| s.2).collect();
        c.slots = slots.into_iter().map(|(id, n, _)| (n > 0 && valid_item(id)).then_some((id, n.min(64)))).collect();
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
    fn generated_furnaces_open_empty() {
        // A hut's furnace comes from the generator with no contents record.
        let mut g = crate::game::tests::arena(43);
        let p = IVec3::new(2, 50, 2);
        g.world.set_v(p, FURNACE);
        g.world.containers.remove(&p);
        g.open_container(p);
        assert!(g.container_still_there());
        assert!(store_ref(&g.world, &g.vehicles, p).is_some_and(|c| c.slots.len() == 3));
    }

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
        map.get_mut(&IVec3::new(-3, 60, 1_000_000)).unwrap().slots[5] = Some((PICK_IRON, 1));
        map.get_mut(&IVec3::new(-3, 60, 1_000_000)).unwrap().wear[5] = 123;
        assert_eq!(decode(&encode(&map), 4), map);
        assert!(decode(&[1, 2, 3], 4).is_empty());
    }

    #[test]
    fn a_furnace_smelts_mod_items_and_burns_mod_fuel() {
        // A mod item that smelts into a base item, and a mod item that burns as
        // fuel. The whole cooking path (smelt/fuel_secs/accepts/furnace_tick)
        // must honour mod ids just like base-game ones.
        let mod_txt = "\
[item ruby]
smelts_into = gold

[item fire_dust]
burns_for = 20
";
        crate::mods::with_mods(&[("gems", mod_txt)], |reg| {
            let ruby = reg.lookup("gems:ruby").unwrap();
            let fire_dust = reg.lookup("gems:fire_dust").unwrap();
            assert!(ruby >= FIRST_MOD_ITEM && fire_dust >= FIRST_MOD_ITEM);
            // The mod data reached the furnace lookups.
            assert_eq!(smelt(ruby), Some(GOLD_INGOT));
            assert_eq!(fuel_secs(fire_dust), Some(20.0));
            // The mod fuel is accepted in the fuel slot (and the input slot takes anything).
            assert!(accepts(FURNACE, FUEL, fire_dust));
            assert!(accepts(FURNACE, INPUT, ruby));

            let mut f = Container::for_block(FURNACE);
            f.slots[INPUT] = Some((ruby, 3));
            f.slots[FUEL] = Some((fire_dust, 1));
            // 20s of fuel cooks two rubies (8s each) and starts a third.
            let mut t = 0.0;
            while t < 20.0 {
                f.furnace_tick(0.1);
                t += 0.1;
            }
            assert_eq!(f.slots[OUTPUT], Some((GOLD_INGOT, 2)), "two rubies smelted into gold");
            assert_eq!(f.slots[INPUT], Some((ruby, 1)));
            assert_eq!(f.slots[FUEL], None, "the mod fuel was consumed");
            assert!(f.burn < 0.5, "and all but burnt out ({})", f.burn);
        });
    }
}
