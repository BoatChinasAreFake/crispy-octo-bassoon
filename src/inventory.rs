//! 36-slot inventory (first 9 are the hotbar), four armour slots, and
//! shapeless crafting.

use crate::block::*;
use std::collections::BTreeMap;

pub type Stack = Option<(Id, u8)>;

pub struct Inventory {
    pub slots: [Stack; 36],
    pub selected: usize,
    /// Stack held by the mouse cursor while the inventory screen is open.
    pub cursor: Stack,
    /// Worn armour: helmet, chestplate, leggings, boots.
    pub armor: [Stack; 4],
}

impl Inventory {
    pub fn new() -> Self {
        Inventory { slots: [None; 36], selected: 0, cursor: None, armor: [None; 4] }
    }

    pub fn held(&self) -> Id {
        self.slots[self.selected].map(|s| s.0).unwrap_or(AIR)
    }

    /// Add items; returns how many didn't fit.
    pub fn add(&mut self, item: Id, mut count: u8) -> u8 {
        let max = max_stack(item);
        for s in self.slots.iter_mut() {
            if let Some((id, n)) = s {
                if *id == item && *n < max {
                    let put = count.min(max - *n);
                    *n += put;
                    count -= put;
                    if count == 0 {
                        return 0;
                    }
                }
            }
        }
        for s in self.slots.iter_mut() {
            if s.is_none() {
                let put = count.min(max);
                *s = Some((item, put));
                count -= put;
                if count == 0 {
                    return 0;
                }
            }
        }
        count
    }

    /// How many more of `item` would fit.
    pub fn room_for(&self, item: Id) -> u32 {
        let max = max_stack(item) as u32;
        self.slots
            .iter()
            .map(|s| match s {
                None => max,
                Some((id, n)) if *id == item => max.saturating_sub(*n as u32),
                _ => 0,
            })
            .sum()
    }

    pub fn count(&self, item: Id) -> u32 {
        self.slots.iter().flatten().filter(|s| s.0 == item).map(|s| s.1 as u32).sum()
    }

    /// Take items away: from the inventory first, then (for the host's
    /// corrections and scripts) from what's worn.
    pub fn remove(&mut self, item: Id, mut count: u32) {
        for s in self.slots.iter_mut().rev().chain(self.armor.iter_mut()) {
            if let Some((id, n)) = s {
                if *id == item {
                    let take = (count.min(*n as u32)) as u8;
                    *n -= take;
                    count -= take as u32;
                    if *n == 0 {
                        *s = None;
                    }
                    if count == 0 {
                        return;
                    }
                }
            }
        }
    }

    pub fn consume_held(&mut self) {
        if let Some((_, n)) = &mut self.slots[self.selected] {
            *n -= 1;
            if *n == 0 {
                self.slots[self.selected] = None;
            }
        }
    }

    pub fn can_craft(&self, r: &Recipe) -> bool {
        r.inputs.iter().all(|&(item, n)| self.count(item) >= n as u32)
    }

    pub fn craft(&mut self, r: &Recipe) -> bool {
        if !self.can_craft(r) {
            return false;
        }
        for &(item, n) in &r.inputs {
            self.remove(item, n as u32);
        }
        let left = self.add(r.output.0, r.output.1);
        if left > 0 {
            // No room: put it on the cursor if possible, else refund is lost to the void. Parody physics.
            if self.cursor.is_none() {
                self.cursor = Some((r.output.0, left));
            }
        }
        true
    }

    /// Left click on a slot: pick up, put down, merge or swap with the cursor.
    pub fn click(&mut self, i: usize) {
        click_stack(&mut self.slots[i], &mut self.cursor);
    }

    /// Right click: take half, or drop a single item.
    pub fn right_click(&mut self, i: usize) {
        right_click_stack(&mut self.slots[i], &mut self.cursor);
    }

    /// How many of each item (the cursor included). Slot layout doesn't matter to the host.
    pub fn counts(&self) -> BTreeMap<Id, u32> {
        let mut c = BTreeMap::new();
        for (id, n) in self.slots.iter().chain(std::iter::once(&self.cursor)).chain(self.armor.iter()).flatten() {
            *c.entry(*id).or_insert(0) += *n as u32;
        }
        c
    }

    /// Add or remove items until the counts match `target` (the host's word).
    /// Existing stacks stay where they are as far as possible.
    pub fn set_counts(&mut self, target: &BTreeMap<Id, u32>) {
        self.return_cursor();
        let have = self.counts();
        for (&id, &n) in &have {
            let want = target.get(&id).copied().unwrap_or(0);
            if n > want {
                self.remove(id, n - want);
            }
        }
        for (&id, &want) in target {
            let n = have.get(&id).copied().unwrap_or(0);
            let mut missing = want.saturating_sub(n);
            while missing > 0 && valid_item(id) {
                let batch = missing.min(64) as u8;
                if self.add(id, batch) > 0 {
                    break; // full
                }
                missing -= batch as u32;
            }
        }
    }

    /// Total armour points being worn.
    pub fn armor_points(&self) -> u32 {
        self.armor.iter().flatten().map(|(id, _)| armor_points(*id) as u32).sum()
    }

    /// Wear the armour in inventory slot `i` (swapping out whatever was worn there).
    /// Returns whether it was armour.
    pub fn equip(&mut self, i: usize) -> bool {
        let Some((id, _)) = self.slots[i] else { return false };
        let Some((slot, _)) = armor_of(id) else { return false };
        std::mem::swap(&mut self.slots[i], &mut self.armor[slot]);
        true
    }

    /// Clicked armour slot `slot`: only the right kind of armour goes in.
    pub fn click_armor(&mut self, slot: usize) {
        if self.cursor.is_some_and(|(id, _)| armor_of(id).map(|(s, _)| s) != Some(slot)) {
            return;
        }
        std::mem::swap(&mut self.armor[slot], &mut self.cursor);
    }

    /// Worn tiers for other players to see: 4 bits per slot, tier + 1 (0 = nothing).
    pub fn armor_look(&self) -> u16 {
        self.armor.iter().enumerate().map(|(i, s)| s.and_then(|(id, _)| armor_of(id)).map(|(_, t)| (t as u16 + 1) << (i * 4)).unwrap_or(0)).sum()
    }

    /// Put whatever the cursor holds back into the inventory.
    pub fn return_cursor(&mut self) {
        if let Some((id, n)) = self.cursor.take() {
            self.add(id, n);
        }
    }
}

/// Left click on any slot (inventory, chest, furnace): pick up, put down,
/// merge or swap with the cursor.
pub fn click_stack(slot: &mut Stack, cursor: &mut Stack) {
    match (*cursor, *slot) {
        (Some((a, an)), Some((b, bn))) if a == b => {
            let max = max_stack(a);
            let put = an.min(max.saturating_sub(bn));
            *slot = Some((a, bn + put));
            *cursor = if an - put > 0 { Some((a, an - put)) } else { None };
        }
        _ => std::mem::swap(slot, cursor),
    }
}

/// Right click on any slot: take half, or drop a single item.
pub fn right_click_stack(slot: &mut Stack, cursor: &mut Stack) {
    match (*cursor, *slot) {
        (None, Some((id, n))) => {
            let take = n.div_ceil(2);
            *cursor = Some((id, take));
            *slot = if n - take > 0 { Some((id, n - take)) } else { None };
        }
        (Some((id, n)), None) => {
            *slot = Some((id, 1));
            *cursor = if n > 1 { Some((id, n - 1)) } else { None };
        }
        (Some((id, n)), Some((b, bn))) if id == b && bn < max_stack(id) => {
            *slot = Some((id, bn + 1));
            *cursor = if n > 1 { Some((id, n - 1)) } else { None };
        }
        _ => {}
    }
}
