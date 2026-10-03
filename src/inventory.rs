//! 36-slot inventory (first 9 are the hotbar), four armour slots, and
//! shapeless crafting.

use crate::block::*;
use std::collections::BTreeMap;

pub type Stack = Option<(Id, u8)>;

/// What we know about one particular tool, weapon or piece of armour: uses
/// so far in the low 16 bits (see `durability`), enchantments in the high 16
/// (see enchant.rs). Zero for everything else. It travels with the item
/// wherever it goes: slots, the cursor, chests, the ground, saves.
pub type Wear = u32;

/// Everything in a wear but its enchantments (uses, and an armour trim).
pub const NOT_ENCHANTS: Wear = 0x8000_FFFF;

/// Uses so far (a trimmed piece of armour keeps its trim in the top of
/// the low half; see trims.rs).
pub fn uses(w: Wear) -> u16 {
    if w & crate::trims::TRIMMED != 0 { (w & 0xFFF) as u16 } else { w as u16 }
}

/// The same item with a different number of uses (enchantments and trim kept).
pub fn with_uses(w: Wear, uses: u16) -> Wear {
    if w & crate::trims::TRIMMED != 0 {
        (w & !0xFFF) | uses.min(0xFFF) as u32
    } else {
        (w & 0xFFFF_0000) | uses as u32
    }
}

/// Uses before `item` (with these enchantments) breaks.
pub fn max_uses(item: Id, w: Wear) -> Option<u32> {
    durability(item).map(|d| d as u32 * (1 + crate::enchant::level(w, crate::enchant::Enchant::Unbreaking) as u32))
}

/// Items that remember their wear: tools, weapons, armour, and enchanted books.
pub fn keeps_wear(item: Id) -> bool {
    durability(item).is_some() || tagged(item)
}

/// Items whose wear's high half is a label rather than damage: a Hollow Box's
/// number (see boxes.rs), a queen's temperament (bees.rs), a find's condition
/// (archaeology.rs), a book's enchantment.
pub fn tagged(item: Id) -> bool {
    item == ENCHANTED_BOOK || item == HOLLOW_BOX || item == BUNDLE || item == BOOK_AND_QUILL || item == WRITTEN_BOOK || item == BANNER || item == QUEEN_BEE || crate::archaeology::is_find(item)
}

/// Keep only what makes sense for `item` (saves and other players can't be trusted).
pub fn sanitize_wear(item: Id, w: Wear) -> Wear {
    if tagged(item) {
        return w & 0xFFFF_0000;
    }
    match max_uses(item, w) {
        Some(m) => with_uses(w, uses(w).min((m - 1).min(u16::MAX as u32) as u16)),
        None => 0,
    }
}

pub struct Inventory {
    pub slots: [Stack; 36],
    pub selected: usize,
    /// Stack held by the mouse cursor while the inventory screen is open.
    pub cursor: Stack,
    /// Worn armour: helmet, chestplate, leggings, boots.
    pub armor: [Stack; 4],
    /// Uses so far of whatever durable thing is in each slot (see `durability`).
    /// Only meaningful while the slot holds a tool, weapon or armour.
    pub wear: [Wear; 36],
    pub armor_wear: [Wear; 4],
    pub cursor_wear: Wear,
    /// The other hand: a shield, torches, rockets... (swap with Swap Hands).
    pub offhand: Stack,
    pub offhand_wear: Wear,
}

impl Inventory {
    pub fn new() -> Self {
        Inventory { slots: [None; 36], selected: 0, cursor: None, armor: [None; 4], wear: [0; 36], armor_wear: [0; 4], cursor_wear: 0, offhand: None, offhand_wear: 0 }
    }

    pub fn held(&self) -> Id {
        self.slots[self.selected].map(|s| s.0).unwrap_or(AIR)
    }

    /// Add items; returns how many didn't fit.
    pub fn add(&mut self, item: Id, mut count: u8) -> u8 {
        let max = max_stack(item);
        for s in self.slots.iter_mut() {
            if let Some((id, n)) = s
                && *id == item && *n < max {
                    let put = count.min(max - *n);
                    *n += put;
                    count -= put;
                    if count == 0 {
                        return 0;
                    }
                }
        }
        for (i, s) in self.slots.iter_mut().enumerate() {
            if s.is_none() {
                let put = count.min(max);
                *s = Some((item, put));
                self.wear[i] = 0;
                count -= put;
                if count == 0 {
                    return 0;
                }
            }
        }
        count
    }

    /// Add one used tool (or anything else, `wear` is ignored for things that don't wear).
    pub fn add_worn(&mut self, item: Id, n: u8, wear: Wear) -> u8 {
        if wear == 0 || !keeps_wear(item) {
            return self.add(item, n);
        }
        let Some(i) = self.slots.iter().position(|s| s.is_none()) else { return n };
        self.slots[i] = Some((item, 1));
        self.wear[i] = wear;
        n - 1
    }

    /// Use the held tool `amount` times. Returns the item if that broke it.
    pub fn wear_held(&mut self, amount: u16) -> Option<Id> {
        let held = self.held();
        let w = &mut self.wear[self.selected];
        let max = max_uses(held, *w)?;
        let used = uses(*w) as u32 + amount as u32;
        if used >= max {
            *w = 0;
            self.slots[self.selected] = None;
            return Some(held);
        }
        *w = with_uses(*w, used as u16);
        None
    }

    /// Worn armour takes a hit. Returns whatever broke.
    pub fn wear_armor(&mut self, amount: u16) -> Vec<Id> {
        let mut broke = Vec::new();
        for (s, w) in self.armor.iter_mut().zip(self.armor_wear.iter_mut()) {
            let Some((id, _)) = *s else { continue };
            // A Glider only wears out by gliding (see glider.rs).
            if id == GLIDER {
                continue;
            }
            let Some(max) = max_uses(id, *w) else { continue };
            let used = uses(*w) as u32 + amount as u32;
            if used >= max {
                *w = 0;
                *s = None;
                broke.push(id);
            } else {
                *w = with_uses(*w, used as u16);
            }
        }
        broke
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
        self.slots.iter().chain(std::iter::once(&self.offhand)).flatten().filter(|s| s.0 == item).map(|s| s.1 as u32).sum()
    }

    /// The Sort button: tidies the backpack (the hotbar stays as it is).
    pub fn sort_backpack(&mut self) {
        crate::containers::sort_slots(&mut self.slots[9..36], &mut self.wear[9..36]);
    }

    /// Swap what's in the hand for what's in the other hand.
    pub fn swap_hands(&mut self) {
        let i = self.selected;
        std::mem::swap(&mut self.slots[i], &mut self.offhand);
        std::mem::swap(&mut self.wear[i], &mut self.offhand_wear);
    }

    /// Take items away: from the inventory first, then (for the host's
    /// corrections and scripts) from what's worn.
    pub fn remove(&mut self, item: Id, mut count: u32) {
        let wear = self.wear.iter_mut().rev().chain(std::iter::once(&mut self.offhand_wear)).chain(self.armor_wear.iter_mut());
        for (s, w) in self.slots.iter_mut().rev().chain(std::iter::once(&mut self.offhand)).chain(self.armor.iter_mut()).zip(wear) {
            if let Some((id, n)) = s
                && *id == item {
                    let take = (count.min(*n as u32)) as u8;
                    *n -= take;
                    count -= take as u32;
                    if *n == 0 {
                        *s = None;
                        *w = 0;
                    }
                    if count == 0 {
                        return;
                    }
                }
        }
    }

    pub fn consume_held(&mut self) {
        if let Some((_, n)) = &mut self.slots[self.selected] {
            *n -= 1;
            if *n == 0 {
                self.slots[self.selected] = None;
                self.wear[self.selected] = 0;
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
        let before = (self.slots[i], self.cursor);
        click_stack(&mut self.slots[i], &mut self.cursor);
        wear_follow(before, (self.slots[i], self.cursor), &mut self.wear[i], &mut self.cursor_wear);
    }

    /// Right click: take half, or drop a single item.
    pub fn right_click(&mut self, i: usize) {
        let before = (self.slots[i], self.cursor);
        right_click_stack(&mut self.slots[i], &mut self.cursor);
        wear_follow(before, (self.slots[i], self.cursor), &mut self.wear[i], &mut self.cursor_wear);
    }

    /// How many of each item (the cursor included). Slot layout doesn't matter to the host.
    pub fn counts(&self) -> BTreeMap<Id, u32> {
        let mut c = BTreeMap::new();
        for (id, n) in self.slots.iter().chain(std::iter::once(&self.cursor)).chain(self.armor.iter()).chain(std::iter::once(&self.offhand)).flatten() {
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
        std::mem::swap(&mut self.wear[i], &mut self.armor_wear[slot]);
        true
    }

    /// Clicked armour slot `slot`: only the right kind of armour goes in.
    /// Clicked the other hand's slot: swap with (or merge into) the cursor.
    pub fn click_offhand(&mut self, right: bool) {
        let before = (self.offhand, self.cursor);
        if right {
            right_click_stack(&mut self.offhand, &mut self.cursor);
        } else {
            click_stack(&mut self.offhand, &mut self.cursor);
        }
        wear_follow(before, (self.offhand, self.cursor), &mut self.offhand_wear, &mut self.cursor_wear);
    }

    pub fn click_armor(&mut self, slot: usize) {
        if self.cursor.is_some_and(|(id, _)| armor_of(id).map(|(s, _)| s) != Some(slot)) {
            return;
        }
        std::mem::swap(&mut self.armor[slot], &mut self.cursor);
        std::mem::swap(&mut self.armor_wear[slot], &mut self.cursor_wear);
    }

    /// Worn tiers for other players to see: 4 bits per slot, tier + 1 (0 = nothing).
    pub fn armor_look(&self) -> u16 {
        self.armor.iter().enumerate().map(|(i, s)| s.and_then(|(id, _)| armor_of(id)).map(|(_, t)| (t as u16 + 1) << (i * 4)).unwrap_or(0)).sum()
    }

    /// Put whatever the cursor holds back into the inventory.
    pub fn return_cursor(&mut self) {
        if let Some((id, n)) = self.cursor.take() {
            self.add_worn(id, n, self.cursor_wear);
        }
        self.cursor_wear = 0;
    }
}

/// After a click moved stacks between a slot and the cursor, move their wear
/// with them: each side keeps its own if it holds the same thing as before,
/// takes the other's if it now holds what the other had, and is fresh otherwise.
pub fn wear_follow(before: (Stack, Stack), after: (Stack, Stack), slot_wear: &mut Wear, cursor_wear: &mut Wear) {
    let id = |s: Stack| s.map(|s| s.0);
    let (sw, cw) = (*slot_wear, *cursor_wear);
    let pick = |now: Stack, own: Stack, other: Stack, own_w: Wear, other_w: Wear| match id(now) {
        None => 0,
        n if n == id(own) => own_w,
        n if n == id(other) => other_w,
        _ => 0,
    };
    *slot_wear = pick(after.0, before.0, before.1, sw, cw);
    *cursor_wear = pick(after.1, before.1, before.0, cw, sw);
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
