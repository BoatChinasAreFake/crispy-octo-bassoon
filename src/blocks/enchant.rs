//! Enchantments: stored in the high 16 bits of an item's wear (see
//! `inventory::Wear`).
//!
//! The first five took three bits each (bits 16..31). Unbreaking and Fortune
//! never go above III, so the top bit of each was always clear: those two
//! bits now hold **Mending** (27) and **Silk Touch** (30), and older items
//! read exactly as before. Three more share another's bits on the things only
//! they fit: **Looting** is Fortune's bits on a weapon, and **Frost Walker**
//! (boots) and **Riptide** (spears) are Efficiency's. On a book they're the
//! same bits, so a book reads as both ("Fortune/Looting III").

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
    /// Experience you pick up mends it instead.
    Mending,
    /// Blocks come away whole (glass as glass, ore as ore).
    SilkTouch,
    /// Weapons: more loot (Fortune's bits).
    Looting,
    /// Boots: water freezes underfoot (Efficiency's bits).
    FrostWalker,
    /// Spears: thrown in water or rain, it takes you with it (Efficiency's bits).
    Riptide,
}

impl Enchant {
    pub const ALL: [Enchant; 10] = [
        Enchant::Efficiency,
        Enchant::Sharpness,
        Enchant::Protection,
        Enchant::Unbreaking,
        Enchant::Fortune,
        Enchant::Mending,
        Enchant::SilkTouch,
        Enchant::Looting,
        Enchant::FrostWalker,
        Enchant::Riptide,
    ];

    /// Where it's kept: (first bit, mask).
    fn bits(self) -> (u32, u32) {
        match self {
            Enchant::Efficiency | Enchant::FrostWalker | Enchant::Riptide => (16, 7),
            Enchant::Sharpness => (19, 7),
            Enchant::Protection => (22, 7),
            Enchant::Unbreaking => (25, 3),
            Enchant::Mending => (27, 1),
            Enchant::Fortune | Enchant::Looting => (28, 3),
            Enchant::SilkTouch => (30, 1),
        }
    }

    pub fn max_level(self) -> u8 {
        match self {
            Enchant::Efficiency | Enchant::Sharpness => 5,
            Enchant::Protection => 4,
            Enchant::Unbreaking | Enchant::Fortune | Enchant::Looting | Enchant::Riptide => 3,
            Enchant::FrostWalker => 2,
            Enchant::Mending | Enchant::SilkTouch => 1,
        }
    }

    /// The enchantments with their own bits (each shared one counted once).
    pub const STORED: [Enchant; 7] = [Enchant::Efficiency, Enchant::Sharpness, Enchant::Protection, Enchant::Unbreaking, Enchant::Fortune, Enchant::Mending, Enchant::SilkTouch];

    /// Only from treasure (books in chests, Librarians), never the table.
    pub fn treasure(self) -> bool {
        self == Enchant::Mending
    }

    pub fn name(self) -> &'static str {
        match self {
            Enchant::Efficiency => "Efficiency (Faster Hitting of Rocks)",
            Enchant::Sharpness => "Sharpness (Pointier)",
            Enchant::Protection => "Protection (Extra Padding)",
            Enchant::Unbreaking => "Unbreaking (Mostly)",
            Enchant::Fortune => "Fortune (Rocks Pay Better)",
            Enchant::Mending => "Mending (Experience Fixes It)",
            Enchant::SilkTouch => "Silk Touch (Gentle Hands)",
            Enchant::Looting => "Looting (Monsters Pay Better)",
            Enchant::FrostWalker => "Frost Walker (Walk on Water, Sort Of)",
            Enchant::Riptide => "Riptide (Spear Express)",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Enchant::Efficiency => "Efficiency",
            Enchant::Sharpness => "Sharpness",
            Enchant::Protection => "Protection",
            Enchant::Unbreaking => "Unbreaking",
            Enchant::Fortune => "Fortune",
            Enchant::Mending => "Mending",
            Enchant::SilkTouch => "Silk Touch",
            Enchant::Looting => "Looting",
            Enchant::FrostWalker => "Frost Walker",
            Enchant::Riptide => "Riptide",
        }
    }

    /// Can `item` carry this enchantment? (Books carry anything.)
    pub fn fits(self, item: Id) -> bool {
        if item == BOOK || item == ENCHANTED_BOOK {
            return true;
        }
        let pick = pick_tier(item) > 0 || crate::tools::is_digger(item);
        let boots = armor_of(item).is_some_and(|(slot, _)| slot == BOOTS);
        let spear = crate::block::is_spear(item);
        match self {
            Enchant::Efficiency | Enchant::Fortune | Enchant::SilkTouch => pick,
            Enchant::Sharpness => is_sword(item),
            Enchant::Protection => armor_of(item).is_some() || item == SHIELD,
            Enchant::Unbreaking | Enchant::Mending => durability(item).is_some(),
            Enchant::Looting => is_sword(item) && !pick,
            Enchant::FrostWalker => boots,
            Enchant::Riptide => spear && !pick,
        }
    }
}

/// Enchantments an enchanting table (or a lucky chest) might give `item` at
/// power `power` (1..=30): one or two that fit it, stronger at higher power.
pub fn roll(item: Id, power: u8, rng: &mut crate::noise::Rng) -> Wear {
    // (A book rolls only the enchantments with their own bits, so it never names two at once.)
    let pool: &[Enchant] = if item == BOOK || item == ENCHANTED_BOOK { &Enchant::STORED } else { &Enchant::ALL };
    let fits: Vec<Enchant> = pool.iter().copied().filter(|e| e.fits(item) && !e.treasure()).collect();
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
    // Silk Touch and Fortune don't go together.
    if level(w, Enchant::SilkTouch) > 0 && level(w, Enchant::Fortune) > 0 && item != BOOK && item != ENCHANTED_BOOK {
        w = with_level(w, Enchant::Fortune, 0);
    }
    w
}

/// An enchanted book from a treasure chest: something strong (now and then Mending).
pub fn random_book(rng: &mut crate::noise::Rng) -> Wear {
    let w = roll(BOOK, rng.int(15, 30) as u8, rng);
    if rng.chance(0.2) { with_level(0, Enchant::Mending, 1) } else { w }
}

/// An enchantment's level on an item (0: none).
pub fn level(w: Wear, e: Enchant) -> u8 {
    let (shift, mask) = e.bits();
    ((w >> shift) & mask) as u8
}

pub fn with_level(w: Wear, e: Enchant, lvl: u8) -> Wear {
    let (shift, mask) = e.bits();
    (w & !(mask << shift)) | ((lvl.min(e.max_level()) as u32 & mask) << shift)
}

pub fn is_enchanted(w: Wear) -> bool {
    (w >> 16) & 0x7FFF != 0
}

/// Total levels of all its enchantments.
pub fn total_levels(w: Wear) -> u32 {
    Enchant::STORED.iter().map(|e| level(w, *e) as u32).sum()
}

/// `a`'s enchantments with `b`'s added, as far as they fit `item`: a level
/// both have goes up by one (to its maximum), otherwise the higher wins.
/// Only the enchantment bits are returned.
pub fn merge(item: Id, a: Wear, b: Wear) -> Wear {
    let mut w = a & 0x7FFF_0000;
    for e in Enchant::ALL {
        let (la, lb) = (level(a, e), level(b, e));
        // (Shared bits: only the one that fits this item counts, and only once.)
        let book = item == BOOK || item == ENCHANTED_BOOK;
        if lb == 0 || !e.fits(item) || (book && !Enchant::STORED.contains(&e)) {
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

/// What an item's enchantments are called, as they apply to that item:
/// "Efficiency III, Unbreaking I" (a book's shared bits read "Fortune/Looting III").
pub fn describe_for(item: Id, w: Wear) -> String {
    let roman = ["", "I", "II", "III", "IV", "V", "VI", "VII"];
    let book = item == BOOK || item == ENCHANTED_BOOK;
    let name = |e: Enchant| -> String {
        if !book {
            return e.short().to_string();
        }
        match e {
            Enchant::Fortune => "Fortune/Looting".into(),
            Enchant::Efficiency => "Efficiency/Frost Walker/Riptide".into(),
            e => e.short().to_string(),
        }
    };
    let list: Vec<Enchant> = if book { Enchant::STORED.to_vec() } else { Enchant::ALL.iter().copied().filter(|e| e.fits(item)).collect() };
    list.into_iter().filter(|e| level(w, *e) > 0).map(|e| format!("{} {}", name(e), roman[level(w, e) as usize])).collect::<Vec<_>>().join(", ")
}

/// What a block drops broken with Silk Touch: itself, when it's something a
/// player could place (None: Silk Touch makes no difference).
pub fn silk_drop(id: Id) -> Option<Id> {
    let b = block(id);
    (b.drop != id && b.hardness >= 0.0 && !is_liquid(id) && b.model == Model::Cube && placing_item(id) == Some(id)).then_some(id)
}

/// How long frozen water stays frozen (seconds), and how often boots look for water.
pub const FROST_SECS: f32 = 8.0;
const FROST_EVERY: f32 = 0.15;

/// Water sources in a ring of `radius` round `feet` (one block down), to freeze.
pub fn frost_cells(world: &crate::world::World, feet: Vec3, radius: i32) -> Vec<IVec3> {
    let c = (feet - Vec3::Y * 0.5).floor().as_ivec3();
    let mut v = Vec::new();
    for dz in -radius..=radius {
        for dx in -radius..=radius {
            let p = c + IVec3::new(dx, 0, dz);
            if dx * dx + dz * dz <= radius * radius && world.get_v(p) == WATER && world.get_v(p + IVec3::Y) == AIR {
                v.push(p);
            }
        }
    }
    v
}

/// Top enchantments need this many bookshelves around the table.
pub const MAX_SHELVES: u32 = 15;

/// An item's enchantments on their own (for the network and the ledger).
pub fn enchants(w: Wear) -> u16 {
    // (Bit 31 is an armour trim's, not an enchantment; see trims.rs.)
    ((w >> 16) & 0x7FFF) as u16
}

/// Can the table do anything with this?
pub fn enchantable(item: Id, w: Wear) -> bool {
    !is_enchanted(w) && Enchant::ALL.iter().any(|e| e.fits(item) && !e.treasure())
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
    /// Mending: picked-up experience mends whatever you're holding and wearing
    /// that has Mending (two uses a point). Returns the points it took.
    pub fn mend(&mut self, points: u32) -> u32 {
        let mut left = points;
        let inv = &mut self.inv;
        let sel = inv.selected;
        let mut spots: Vec<(Id, &mut Wear)> = Vec::new();
        if let Some((id, _)) = inv.slots[sel] {
            spots.push((id, &mut inv.wear[sel]));
        }
        if let Some((id, _)) = inv.offhand {
            spots.push((id, &mut inv.offhand_wear));
        }
        for (s, w) in inv.armor.iter().zip(inv.armor_wear.iter_mut()) {
            if let Some((id, _)) = s {
                spots.push((*id, w));
            }
        }
        for (_, w) in spots {
            if left == 0 {
                break;
            }
            let used = crate::inventory::uses(*w) as u32;
            if level(*w, Enchant::Mending) == 0 || used == 0 {
                continue;
            }
            let fix = used.min(left * 2);
            *w = crate::inventory::with_uses(*w, (used - fix) as u16);
            left -= fix.div_ceil(2);
        }
        points - left
    }

    /// Joined players: the host says our experience went up; Mending takes its share first.
    pub fn xp_update(&mut self, total: u32) {
        let gained = total.saturating_sub(self.xp);
        let used = self.mend(gained);
        self.xp = total.min(1 << 24) - used.min(total);
        if used > 0 {
            self.net_send_msg(Msg::Mend { points: used });
        }
    }

    /// The host: a joined player's Mending used some of their experience.
    pub fn host_mend(&mut self, from: u32, points: u32) {
        if let Some(p) = self.peers.get_mut(&from) {
            p.ledger.xp = p.ledger.xp.saturating_sub(points);
        }
    }

    /// Frost Walker boots freeze the water round your feet (the local player).
    pub fn frost_walk(&mut self, dt: f32) {
        self.frost_acc += dt;
        if self.frost_acc < FROST_EVERY {
            return;
        }
        self.frost_acc = 0.0;
        let lvl = self.inv.armor[BOOTS].map(|_| level(self.inv.armor_wear[BOOTS], Enchant::FrostWalker)).unwrap_or(0);
        let b = &self.player.body;
        if lvl == 0 || !b.on_ground || b.in_water || self.spectator {
            return;
        }
        if self.is_client() {
            if !frost_cells(&self.world, b.pos, 1 + lvl as i32).is_empty() {
                self.net_send_msg(Msg::FrostWalk);
            }
            return;
        }
        let at = b.pos;
        self.freeze_round(at, lvl);
    }

    /// The host: a joined player's Frost Walker boots (if their ledger says they have some).
    pub fn host_frost_walk(&mut self, from: u32) {
        let Some(p) = self.peers.get(&from) else { return };
        let at = p.target;
        let lvl = p.ledger.enchanted.keys().filter(|(item, _)| armor_of(*item).is_some_and(|(slot, _)| slot == BOOTS)).map(|(_, ench)| level((*ench as u32) << 16, Enchant::FrostWalker)).max().unwrap_or(0);
        if lvl > 0 {
            self.freeze_round(at, lvl);
        }
    }

    fn freeze_round(&mut self, at: Vec3, lvl: u8) {
        for p in frost_cells(&self.world, at, 1 + lvl as i32) {
            self.world.set_v(p, ICE);
            self.frosted.push((p, FROST_SECS + self.rng.range(0.0, 3.0)));
        }
    }

    /// Where the world lives: frozen water melts back after a while.
    pub fn frost_tick(&mut self, dt: f32) {
        if self.frosted.is_empty() {
            return;
        }
        let mut melt = Vec::new();
        self.frosted.retain_mut(|(p, t)| {
            *t -= dt;
            if *t <= 0.0 {
                melt.push(*p);
                false
            } else {
                true
            }
        });
        for p in melt {
            if self.world.get_v(p) == ICE {
                self.world.set_v(p, WATER);
            }
        }
    }

    /// Riptide: a spear thrown in water or the rain takes you with it (true if it did).
    pub fn riptide(&mut self, wear: Wear) -> bool {
        let lvl = level(wear, Enchant::Riptide);
        let p = self.player.body.pos;
        let wet = self.player.body.in_water || self.rained_on(p.x.floor() as i32, (p.y + 1.0).floor() as i32, p.z.floor() as i32);
        if lvl == 0 || !wet {
            return false;
        }
        self.player.body.vel = self.player.look_dir() * (12.0 + 5.0 * lvl as f32);
        self.player.body.on_ground = false;
        self.player.swing = 1.0;
        self.use_cd = 0.8;
        self.use_tool(1);
        self.sfx(Sfx::Gust, None);
        self.advance("riptide");
        true
    }

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
        ui.wear = (ui.wear & crate::inventory::NOT_ENCHANTS) | bits;
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
                *w = (*w & crate::inventory::NOT_ENCHANTS) | ((ench as u32) << 16);
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

#[cfg(test)]
mod tests {
    use super::*;
    use macroquad::math::ivec3;

    #[test]
    fn old_enchantments_read_the_same_and_new_ones_fit_in_the_spare_bits() {
        // An item from an older save: Unbreaking III and Fortune III use only two bits each.
        let old = with_level(with_level(0, Enchant::Unbreaking, 3), Enchant::Fortune, 3);
        assert_eq!(old, (3 << 25) | (3 << 28));
        assert_eq!(level(old, Enchant::Mending), 0);
        assert_eq!(level(old, Enchant::SilkTouch), 0);
        let w = with_level(with_level(old, Enchant::Mending, 1), Enchant::SilkTouch, 1);
        assert_eq!((level(w, Enchant::Unbreaking), level(w, Enchant::Fortune), level(w, Enchant::Mending), level(w, Enchant::SilkTouch)), (3, 3, 1, 1));
        assert_eq!(w & (1 << 31), 0, "the trim bit stays clear");
        // Shared bits name the right thing for the item.
        let boots = ARMOR_FIRST + 4 + BOOTS as Id;
        assert!(Enchant::FrostWalker.fits(boots) && !Enchant::FrostWalker.fits(PICK_IRON) && !Enchant::Efficiency.fits(boots));
        assert!(Enchant::Looting.fits(SWORD_IRON) && !Enchant::Fortune.fits(SWORD_IRON));
        assert!(Enchant::Riptide.fits(SPEAR_FIRST + 2));
        let fw = with_level(0, Enchant::FrostWalker, 2);
        assert_eq!(describe_for(boots, fw), "Frost Walker II");
        assert_eq!(describe_for(BOOK, with_level(0, Enchant::Looting, 3)), "Fortune/Looting III");
        assert_eq!(level(merge(boots, 0, fw), Enchant::FrostWalker), 2, "a book's Efficiency bits go onto boots as Frost Walker");
        // The table never gives Mending; Silk Touch and Fortune don't share a pick.
        let mut rng = crate::noise::Rng::new(5);
        for _ in 0..400 {
            let r = roll(PICK_DIAMOND, 30, &mut rng);
            assert_eq!(level(r, Enchant::Mending), 0);
            assert!(level(r, Enchant::SilkTouch) == 0 || level(r, Enchant::Fortune) == 0);
        }
        assert!((0..200).any(|_| level(random_book(&mut rng), Enchant::Mending) == 1), "Mending turns up in treasure");
    }

    #[test]
    fn mending_silk_touch_and_frost_walker_work() {
        let mut g = crate::game::tests::arena(181);
        // Mending: experience fixes a worn pick before it counts.
        g.inv.slots[0] = Some((PICK_IRON, 1));
        g.inv.selected = 0;
        g.inv.wear[0] = with_level(crate::inventory::with_uses(0, 10), Enchant::Mending, 1);
        g.add_xp(8);
        assert_eq!(crate::inventory::uses(g.inv.wear[0]), 0, "mended");
        assert_eq!(g.xp, 3, "five points went on mending");
        // Silk Touch: glass comes away whole.
        assert_eq!(silk_drop(GLASS), Some(GLASS));
        assert_eq!(silk_drop(STONE), Some(STONE));
        assert_eq!(silk_drop(COBBLE), None, "cobble drops itself anyway");
        g.inv.wear[0] = with_level(0, Enchant::SilkTouch, 1);
        let p = ivec3(2, 50, 2);
        g.world.set_v(p, GLASS);
        g.break_block(p, true);
        assert!(g.drops.iter().any(|d| d.item == GLASS), "glass, not nothing");
        // Frost Walker: the pond freezes round you, then melts.
        for x in -2..=2 {
            for z in -2..=2 {
                g.world.set_v(ivec3(x + 6, 49, z), WATER);
            }
        }
        g.world.set_v(ivec3(6, 49, 0), STONE);
        g.player.body.pos = Vec3::new(6.5, 50.0, 0.5);
        g.player.body.on_ground = true;
        g.inv.armor[BOOTS] = Some((ARMOR_FIRST + 4 + BOOTS as Id, 1));
        g.inv.armor_wear[BOOTS] = with_level(0, Enchant::FrostWalker, 1);
        g.frost_walk(1.0);
        assert_eq!(g.world.get_v(ivec3(7, 49, 0)), ICE);
        assert_eq!(g.world.get_v(ivec3(8, 49, 2)), WATER, "only a little way round");
        for _ in 0..30 {
            g.frost_tick(0.5);
        }
        assert_eq!(g.world.get_v(ivec3(7, 49, 0)), WATER, "melted again");
    }
}
