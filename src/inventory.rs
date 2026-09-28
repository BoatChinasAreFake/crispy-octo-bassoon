//! 36-slot inventory (first 9 are the hotbar) and shapeless crafting.

use crate::block::*;

pub type Stack = Option<(u8, u8)>;

pub struct Inventory {
    pub slots: [Stack; 36],
    pub selected: usize,
    /// Stack held by the mouse cursor while the inventory screen is open.
    pub cursor: Stack,
}

impl Inventory {
    pub fn new() -> Self {
        Inventory { slots: [None; 36], selected: 0, cursor: None }
    }

    pub fn held(&self) -> u8 {
        self.slots[self.selected].map(|s| s.0).unwrap_or(AIR)
    }

    /// Add items; returns how many didn't fit.
    pub fn add(&mut self, item: u8, mut count: u8) -> u8 {
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

    pub fn count(&self, item: u8) -> u32 {
        self.slots.iter().flatten().filter(|s| s.0 == item).map(|s| s.1 as u32).sum()
    }

    pub fn remove(&mut self, item: u8, mut count: u32) {
        for s in self.slots.iter_mut().rev() {
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
        for &(item, n) in r.inputs {
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
        let slot = self.slots[i];
        match (self.cursor, slot) {
            (Some((a, an)), Some((b, bn))) if a == b => {
                let max = max_stack(a);
                let put = an.min(max - bn);
                self.slots[i] = Some((a, bn + put));
                self.cursor = if an - put > 0 { Some((a, an - put)) } else { None };
            }
            _ => {
                self.slots[i] = self.cursor;
                self.cursor = slot;
            }
        }
    }

    /// Right click: take half, or drop a single item.
    pub fn right_click(&mut self, i: usize) {
        match (self.cursor, self.slots[i]) {
            (None, Some((id, n))) => {
                let take = n.div_ceil(2);
                self.cursor = Some((id, take));
                self.slots[i] = if n - take > 0 { Some((id, n - take)) } else { None };
            }
            (Some((id, n)), None) => {
                self.slots[i] = Some((id, 1));
                self.cursor = if n > 1 { Some((id, n - 1)) } else { None };
            }
            (Some((id, n)), Some((b, bn))) if id == b && bn < max_stack(id) => {
                self.slots[i] = Some((id, bn + 1));
                self.cursor = if n > 1 { Some((id, n - 1)) } else { None };
            }
            _ => {}
        }
    }

    /// Put whatever the cursor holds back into the inventory.
    pub fn return_cursor(&mut self) {
        if let Some((id, n)) = self.cursor.take() {
            self.add(id, n);
        }
    }
}
