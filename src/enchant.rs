//! Enchantments: stored in the high 16 bits of an item's wear (see
//! `inventory::Wear`), three bits per enchantment.

use crate::block::*;
use crate::game::Game;
use crate::inventory::{click_stack, right_click_stack, wear_follow, Stack, Wear};
use crate::net::Msg;
use crate::sound::Sfx;
use crate::xp::{level_of, spend_levels};
use macroquad::math::{IVec3, Vec3};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Enchant {
    /// Pickaxes dig faster.
    Efficiency,
    /// Swords hit harder.
    Sharpness,
    /// Armour blocks more.
    Protection,
    /// Anything that wears out lasts longer.
    Unbreaking,
    /// Pickaxes get more from ores.
    Fortune,
}

impl Enchant {
    pub const ALL: [Enchant; 5] = [Enchant::Efficiency, Enchant::Sharpness, Enchant::Protection, Enchant::Unbreaking, Enchant::Fortune];

    fn shift(self) -> u32 {
        16 + 3 * self as u32
    }

    pub fn max_level(self) -> u8 {
        match self {
            Enchant::Efficiency | Enchant::Sharpness => 5,
            Enchant::Protection => 4,
            Enchant::Unbreaking | Enchant::Fortune => 3,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Enchant::Efficiency => "Efficiency (Faster Hitting of Rocks)",
            Enchant::Sharpness => "Sharpness (Pointier)",
            Enchant::Protection => "Protection (Extra Padding)",
            Enchant::Unbreaking => "Unbreaking (Mostly)",
            Enchant::Fortune => "Fortune (Rocks Pay Better)",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Enchant::Efficiency => "Efficiency",
            Enchant::Sharpness => "Sharpness",
            Enchant::Protection => "Protection",
            Enchant::Unbreaking => "Unbreaking",
            Enchant::Fortune => "Fortune",
        }
    }

    /// Can `item` carry this enchantment? (Books carry anything.)
    pub fn fits(self, item: Id) -> bool {
        if item == BOOK || item == ENCHANTED_BOOK {
            return true;
        }
        let pick = pick_tier(item) > 0 || crate::tools::is_digger(item);
        match self {
            Enchant::Efficiency | Enchant::Fortune => pick,
            Enchant::Sharpness => is_sword(item),
            Enchant::Protection => armor_of(item).is_some() || item == SHIELD,
            Enchant::Unbreaking => durability(item).is_some(),
        }
    }
}

/// Enchantments an enchanting table (or a lucky chest) might give `item` at
/// power `power` (1..=30): one or two that fit it, stronger at higher power.
pub fn roll(item: Id, power: u8, rng: &mut crate::noise::Rng) -> Wear {
    let fits: Vec<Enchant> = Enchant::ALL.iter().copied().filter(|e| e.fits(item)).collect();
    if fits.is_empty() || power == 0 {
        return 0;
    }
    let mut w = 0;
    let count = 1 + rng.chance(power as f32 / 40.0) as usize;
    for _ in 0..count {
        let e = fits[rng.int(0, fits.len() as i32 - 1) as usize];
        let top = e.max_level() as f32;
        let lvl = (1.0 + (power as f32 / 30.0) * top * rng.range(0.6, 1.1)).floor().clamp(1.0, top) as u8;
        w = with_level(w, e, lvl.max(level(w, e)));
    }
    w
}

/// An enchanted book from a treasure chest: something strong.
pub fn random_book(rng: &mut crate::noise::Rng) -> Wear {
    roll(BOOK, rng.int(15, 30) as u8, rng)
}

/// An enchantment's level on an item (0: none).
pub fn level(w: Wear, e: Enchant) -> u8 {
    ((w >> e.shift()) & 7) as u8
}

pub fn with_level(w: Wear, e: Enchant, lvl: u8) -> Wear {
    (w & !(7 << e.shift())) | ((lvl.min(e.max_level()) as u32) << e.shift())
}

pub fn is_enchanted(w: Wear) -> bool {
    w >> 16 != 0
}

/// Total levels of all its enchantments.
pub fn total_levels(w: Wear) -> u32 {
    Enchant::ALL.iter().map(|e| level(w, *e) as u32).sum()
}

/// `a`'s enchantments with `b`'s added, as far as they fit `item`: a level
/// both have goes up by one (to its maximum), otherwise the higher wins.
/// Only the enchantment bits are returned.
pub fn merge(item: Id, a: Wear, b: Wear) -> Wear {
    let mut w = a & 0xFFFF_0000;
    for e in Enchant::ALL {
        let (la, lb) = (level(a, e), level(b, e));
        if lb == 0 || !e.fits(item) {
            continue;
        }
        let new = if la == lb { (la + 1).min(e.max_level()) } else { la.max(lb) };
        w = with_level(w, e, new);
    }
    w
}

/// What the table turns an item into (a book becomes an enchanted book).
pub fn enchanted_form(item: Id) -> Id {
    if item == BOOK { ENCHANTED_BOOK } else { item }
}

/// "Efficiency III, Unbreaking I".
pub fn describe(w: Wear) -> String {
    let roman = ["", "I", "II", "III", "IV", "V", "VI", "VII"];
    Enchant::ALL.iter().filter(|e| level(w, **e) > 0).map(|e| format!("{} {}", e.short(), roman[level(w, *e) as usize])).collect::<Vec<_>>().join(", ")
}

/// Top enchantments need this many bookshelves around the table.
pub const MAX_SHELVES: u32 = 15;

/// An item's enchantments on their own (for the network and the ledger).
pub fn enchants(w: Wear) -> u16 {
    (w >> 16) as u16
}

/// Can the table do anything with this?
pub fn enchantable(item: Id, w: Wear) -> bool {
    !is_enchanted(w) && Enchant::ALL.iter().any(|e| e.fits(item))
}

/// What seeds a player's offers: the same on their screen and on the host.
pub fn offer_seed(item: Id, count: u32) -> u64 {
    (item as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ ((count as u64) << 24) ^ 0x5EED
}

/// The table's three offers for `item` with `shelves` bookshelves around it:
/// (level needed, enchantments), cheapest first. Minecraft's formula: a base
/// of 1..8 plus up to the shelf count, then a third of it, two thirds, and all
/// of it (at least twice the shelves) for the three buttons. Costs are 1, 2
/// and 3 levels (and as many gold ingots) whatever the level needed.
pub fn offers(item: Id, w: Wear, shelves: u32, seed: u64) -> Option<[(u32, Wear); 3]> {
    if !enchantable(item, w) {
        return None;
    }
    let mut rng = crate::noise::Rng::new(seed);
    let b = shelves.min(MAX_SHELVES) as i32;
    let base = rng.int(1, 8) + b / 2 + rng.int(0, b);
    let levels = [(base / 3).max(1), base * 2 / 3 + 1, base.max(b * 2)];
    let mut out = [(0, 0); 3];
    for (i, lvl) in levels.into_iter().enumerate() {
        let mut r = crate::noise::Rng::new(seed ^ (0xABCD_0000 + i as u64 * 7919));
        out[i] = (lvl as u32, roll(item, lvl.clamp(1, 30) as u8, &mut r));
    }
    Some(out)
}

/// What's on an open enchanting table (only on the player's own screen).
pub struct EnchantUi {
    pub pos: IVec3,
    pub item: Stack,
    pub wear: Wear,
    pub gold: Stack,
    /// Joined players: the item and enchantments we gave it last, until the host confirms.
    pub predicted: Option<(Id, u16)>,
}

impl Game {
    /// An enchantment's level on the item in hand.
    pub fn held_level(&self, e: Enchant) -> u8 {
        level(self.inv.wear[self.inv.selected], e)
    }

    pub fn open_enchanting(&mut self, pos: IVec3) {
        self.enchanting = Some(EnchantUi { pos, item: None, wear: 0, gold: None, predicted: None });
        self.sfx(Sfx::Chime, Some(pos.as_vec3() + Vec3::splat(0.5)));
    }

    /// Walked away: the item and the gold go back in the inventory.
    pub fn close_enchanting(&mut self) {
        self.inv.return_cursor();
        if let Some(ui) = self.enchanting.take() {
            if let Some((id, n)) = ui.item {
                self.give_worn(id, n, ui.wear);
            }
            if let Some((id, n)) = ui.gold {
                self.give_worn(id, n, 0);
            }
        }
    }

    pub fn enchanting_still_there(&self) -> bool {
        self.enchanting.as_ref().map(|e| self.world.get_v(e.pos) == ENCHANTING_TABLE).unwrap_or(false)
    }

    /// Bookshelves in the ring two blocks out from the table, on its level and the one above.
    pub fn bookshelves(&self, pos: IVec3) -> u32 {
        let mut n = 0;
        for dy in 0..=1 {
            for dx in -2..=2i32 {
                for dz in -2..=2i32 {
                    if dx.abs().max(dz.abs()) == 2 && self.world.get_v(pos + IVec3::new(dx, dy, dz)) == BOOKSHELF {
                        n += 1;
                    }
                }
            }
        }
        n.min(MAX_SHELVES)
    }

    /// Clicked the item slot (0) or the gold slot (1).
    pub fn enchant_click(&mut self, slot: usize, right: bool) {
        let Some(ui) = &mut self.enchanting else { return };
        if slot == 1 {
            // Only gold goes in the gold slot.
            if self.inv.cursor.is_some_and(|(id, _)| id != GOLD_INGOT) {
                return;
            }
            if right {
                right_click_stack(&mut ui.gold, &mut self.inv.cursor);
            } else {
                click_stack(&mut ui.gold, &mut self.inv.cursor);
            }
            return;
        }
        let before = (ui.item, self.inv.cursor);
        if right {
            right_click_stack(&mut ui.item, &mut self.inv.cursor);
        } else {
            click_stack(&mut ui.item, &mut self.inv.cursor);
        }
        // One thing at a time on the table.
        if let Some((id, n)) = ui.item
            && n > 1
        {
            ui.item = Some((id, 1));
            self.inv.cursor = Some((id, n - 1 + self.inv.cursor.map(|c| c.1).unwrap_or(0)));
        }
        wear_follow(before, (ui.item, self.inv.cursor), &mut ui.wear, &mut self.inv.cursor_wear);
    }

    /// Shift-clicked an inventory slot with the table open.
    pub fn enchant_quick_put(&mut self, inv_slot: usize) {
        let Some(ui) = &mut self.enchanting else { return };
        let Some((id, n)) = self.inv.slots[inv_slot] else { return };
        if id == GOLD_INGOT {
            let have = ui.gold.map(|g| g.1).unwrap_or(0);
            let moved = n.min(64 - have);
            if moved > 0 {
                ui.gold = Some((GOLD_INGOT, have + moved));
                self.inv.slots[inv_slot] = if n > moved { Some((id, n - moved)) } else { None };
            }
        } else if ui.item.is_none() && n == 1 {
            ui.item = self.inv.slots[inv_slot].take();
            ui.wear = std::mem::take(&mut self.inv.wear[inv_slot]);
        } else if ui.item.is_none() && id == BOOK {
            // One book off the stack.
            ui.item = Some((BOOK, 1));
            ui.wear = 0;
            self.inv.slots[inv_slot] = Some((BOOK, n - 1));
        }
    }

    /// What the table offers right now: (level needed, enchantments) for each button.
    pub fn enchant_offers(&self) -> Option<[(u32, Wear); 3]> {
        let ui = self.enchanting.as_ref()?;
        let (item, _) = ui.item?;
        offers(item, ui.wear, self.bookshelves(ui.pos), offer_seed(item, self.enchant_count))
    }

    /// Why offer `choice` can't be taken, if it can't.
    pub fn enchant_blocked(&self, choice: usize) -> Option<String> {
        let (need, _) = self.enchant_offers()?[choice];
        let cost = choice as u32 + 1;
        if self.creative {
            return None;
        }
        let level = level_of(self.xp).0;
        if level < need.max(cost) {
            return Some(format!("Needs level {}", need.max(cost)));
        }
        let gold = self.enchanting.as_ref().and_then(|u| u.gold).map(|g| g.1 as u32).unwrap_or(0);
        if gold < cost {
            return Some(format!("Needs {cost} gold"));
        }
        None
    }

    /// Clicked one of the three offers: pay, and the item glows.
    pub fn enchant_pick(&mut self, choice: usize) {
        let Some(offers) = self.enchant_offers() else { return };
        if let Some(why) = self.enchant_blocked(choice) {
            self.msg(format!("The table hums disapprovingly. {why}."));
            return;
        }
        let (_, bits) = offers[choice];
        let cost = choice as u32 + 1;
        let creative = self.creative;
        let Some(ui) = &mut self.enchanting else { return };
        let Some((item, _)) = ui.item else { return };
        ui.wear = (ui.wear & 0xFFFF) | bits;
        ui.item = Some((enchanted_form(item), 1));
        ui.predicted = Some((enchanted_form(item), enchants(bits)));
        if !creative {
            ui.gold = match ui.gold {
                Some((g, n)) if n as u32 > cost => Some((g, n - cost as u8)),
                _ => None,
            };
        }
        let pos = ui.pos;
        if !creative {
            self.xp = spend_levels(self.xp, cost);
        }
        self.enchant_count += 1;
        self.sfx(Sfx::Chime, Some(pos.as_vec3() + Vec3::splat(0.5)));
        self.advance("enchanter");
        if self.is_client() {
            // The host checks it's real, charges the levels and gold, and remembers the enchantments.
            self.net_send_msg(Msg::Enchant { x: pos.x, y: pos.y, z: pos.z, item, choice: choice as u8 });
        }
    }

    /// A joined player enchanted something: check it all, then charge them.
    pub fn host_enchant(&mut self, from: u32, pos: IVec3, item: Id, choice: u8) {
        if choice > 2 || self.world.get_v(pos) != ENCHANTING_TABLE || !self.peer_near(from, pos) || !valid_item(item) {
            return;
        }
        let shelves = self.bookshelves(pos);
        let creative = self.peer_free(from);
        let Some(p) = self.peers.get_mut(&from) else { return };
        let l = &mut p.ledger;
        let count = l.enchant_count;
        let cost = choice as u32 + 1;
        let offer = offers(item, 0, shelves, offer_seed(item, count)).map(|o| o[choice as usize]);
        let ok = offer.is_some_and(|(need, _)| creative || (level_of(l.xp).0 >= need.max(cost) && l.bag.has(item) && l.bag.count(GOLD_INGOT) >= cost));
        let Some((_, bits)) = offer.filter(|_| ok) else {
            // Refused: put their levels and seed straight (items get fixed by the next check).
            let points = l.xp;
            self.net_send_to(from, Msg::Xp { points });
            self.net_send_to(from, Msg::Enchanted { item, ench: 0, count });
            return;
        };
        if !creative {
            l.bag.take(GOLD_INGOT, cost);
            l.xp = spend_levels(l.xp, cost);
        }
        // A book goes in, an enchanted book comes out.
        let result = enchanted_form(item);
        if result != item {
            l.bag.take(item, 1);
            l.bag.add(result, 1);
        }
        l.enchant_count += 1;
        l.add_enchanted(result, enchants(bits), 1);
        let (points, count) = (l.xp, l.enchant_count);
        self.net_send_to(from, Msg::Xp { points });
        self.net_send_to(from, Msg::Enchanted { item: result, ench: enchants(bits), count });
        self.sfx(Sfx::Chime, Some(pos.as_vec3() + Vec3::splat(0.5)));
    }

    /// Joined players: the host's word on our last enchanting.
    pub fn apply_enchanted(&mut self, item: Id, ench: u16, count: u32) {
        self.enchant_count = count;
        let Some(ui) = &mut self.enchanting else { return };
        let Some((want_item, guess)) = ui.predicted.take() else { return };
        if want_item != item || guess == ench {
            return;
        }
        // We guessed differently (the host saw other shelves, or refused): trust it.
        let fix = |w: &mut Wear| {
            if enchants(*w) == guess {
                *w = (*w & 0xFFFF) | ((ench as u32) << 16);
                true
            } else {
                false
            }
        };
        if ui.item.is_some_and(|s| s.0 == item) && fix(&mut ui.wear) {
            return;
        }
        if self.inv.cursor.is_some_and(|s| s.0 == item) && fix(&mut self.inv.cursor_wear) {
            return;
        }
        for i in 0..36 {
            if self.inv.slots[i].is_some_and(|s| s.0 == item) && fix(&mut self.inv.wear[i]) {
                return;
            }
        }
    }
}
