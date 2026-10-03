//! The anvil: repair a worn tool or piece of armour with the stuff it's made
//! of (each unit restores a quarter), or merge two of the same kind. Either
//! costs experience levels, and every use has a 12% chance to knock the anvil
//! down a stage (anvil, chipped, damaged, gone). "Drops Ominously" indeed.
//!
//! The anvil holds nothing between uses: what's on it goes back to your
//! inventory when you walk away. Joined players' repairs are checked by the
//! host against their ledger and their experience (see ledger.rs, xp.rs).

use crate::block::*;
use crate::enchant::{enchants, merge, total_levels};
use crate::game::Game;
use crate::inventory::{click_stack, right_click_stack, uses, wear_follow, with_uses, Stack, Wear};
use crate::net::Msg;
use crate::sound::{Mat, Sfx};
use crate::xp::{level_of, spend_levels};
use macroquad::math::{IVec3, Vec3};

/// Chance that a use knocks the anvil down a stage.
pub const WEAR_CHANCE: f32 = 0.12;
/// Levels to merge two of the same thing.
pub const COMBINE_COST: u32 = 3;

pub fn is_anvil(id: Id) -> bool {
    matches!(id, ANVIL | ANVIL_CHIPPED | ANVIL_DAMAGED)
}

/// What repairs `id` at an anvil.
pub fn repair_material(id: Id) -> Option<Id> {
    if id >= FIRST_MOD_ITEM {
        return Some(reg().items.get((id - FIRST_ITEM) as usize)?.repair).filter(|&m| m != AIR);
    }
    match id {
        GLIDER => return Some(FEATHER),
        SPEAR | PICK_COPPER | SWORD_COPPER => return Some(COPPER_INGOT),
        BRUSH => return Some(COPPER_INGOT),
        DIAMOND_BRUSH => return Some(DIAMOND),
        BEE_SMOKER => return Some(IRON),
        _ if crate::smithing::is_scorchite(id) => return Some(SCORCHITE_INGOT),
        _ => {}
    }
    if let Some(t) = crate::tools::axe_tier(id).or_else(|| crate::tools::shovel_tier(id)) {
        return Some([PLANKS, COBBLE, COPPER_INGOT, IRON, DIAMOND, SCORCHITE_INGOT][t]);
    }
    if armor_of(id).is_some_and(|(_, t)| t == COPPER_TIER) {
        return Some(COPPER_INGOT);
    }
    if let Some((_, tier)) = armor_of(id) {
        return Some([WOOL, IRON, GOLD_INGOT, DIAMOND][tier]);
    }
    Some(match id {
        PICK_WOOD | SWORD_WOOD | HOE => PLANKS,
        PICK_STONE | SWORD_STONE => COBBLE,
        PICK_IRON | SWORD_IRON => IRON,
        PICK_DIAMOND | SWORD_DIAMOND => DIAMOND,
        BOW | ROD => STRING,
        SHEARS => IRON,
        SHIELD => PLANKS,
        _ => return None,
    })
}

/// What an anvil would do with `item` (worn `wear`) and `other` in the second slot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Repair {
    /// Wear of the result (the left item's enchantments are kept).
    pub wear: Wear,
    /// Experience levels it costs.
    pub cost: u32,
    /// How many of the second slot it uses up.
    pub used: u8,
    /// Merging two of the same thing (rather than repairing with materials).
    pub combine: bool,
    /// Putting an enchanted book's enchantments on it.
    pub book: bool,
}

pub fn plan(item: Id, full_wear: Wear, other: Stack, other_wear: Wear) -> Option<Repair> {
    let (oid, on) = other?;
    // An enchanted book's enchantments go onto the item (or another book), as far as they fit.
    if oid == ENCHANTED_BOOK && item != BOOK {
        let merged = merge(item, full_wear, other_wear);
        let added = total_levels(merged).saturating_sub(total_levels(full_wear));
        if added == 0 {
            return None;
        }
        return Some(Repair { wear: (full_wear & crate::inventory::NOT_ENCHANTS) | merged, cost: added.max(1), used: 1, combine: false, book: true });
    }
    let max = durability(item)?;
    let (wear, other_uses) = (uses(full_wear), uses(other_wear));
    if oid == item {
        // Both uses left, plus a 12% bonus, as one; and both lots of enchantments.
        let left = (max - wear.min(max)) as u32 + (max - other_uses.min(max)) as u32 + max as u32 * 12 / 100;
        let new = max.saturating_sub(left.min(max as u32) as u16);
        let merged = merge(item, full_wear, other_wear);
        let added = total_levels(merged).saturating_sub(total_levels(full_wear));
        return Some(Repair { wear: with_uses(merged, new), cost: COMBINE_COST + added, used: 1, combine: true, book: false });
    }
    if repair_material(item) != Some(oid) || wear == 0 {
        return None;
    }
    let unit = max.div_ceil(4);
    let used = (wear.div_ceil(unit)).min(on as u16).min(4) as u8;
    Some(Repair { wear: with_uses(full_wear, wear.saturating_sub(unit * used as u16)), cost: used as u32, used, combine: false, book: false })
}

/// What's on an open anvil (only on the player's own screen).
pub struct AnvilUi {
    pub pos: IVec3,
    pub slots: [Stack; 2],
    pub wear: [Wear; 2],
}

impl Game {
    pub fn open_anvil(&mut self, pos: IVec3) {
        self.anvil = Some(AnvilUi { pos, slots: [None; 2], wear: [0; 2] });
        self.sfx(Sfx::Place(Mat::Stone), Some(pos.as_vec3() + Vec3::splat(0.5)));
    }

    /// Walked away: whatever's on it goes back in the inventory.
    pub fn close_anvil(&mut self) {
        self.inv.return_cursor();
        if let Some(ui) = self.anvil.take() {
            for (s, w) in ui.slots.into_iter().zip(ui.wear) {
                if let Some((id, n)) = s {
                    self.give_worn(id, n, w);
                }
            }
        }
    }

    pub fn anvil_still_there(&self) -> bool {
        self.anvil.as_ref().map(|a| is_anvil(self.world.get_v(a.pos))).unwrap_or(false)
    }

    /// Clicked one of the anvil's two input slots.
    pub fn anvil_click(&mut self, slot: usize, right: bool) {
        let Some(ui) = &mut self.anvil else { return };
        let before = (ui.slots[slot], self.inv.cursor);
        if right {
            right_click_stack(&mut ui.slots[slot], &mut self.inv.cursor);
        } else {
            click_stack(&mut ui.slots[slot], &mut self.inv.cursor);
        }
        wear_follow(before, (ui.slots[slot], self.inv.cursor), &mut ui.wear[slot], &mut self.inv.cursor_wear);
    }

    /// Shift-clicked an inventory slot with the anvil open: onto the first free input.
    pub fn anvil_quick_put(&mut self, inv_slot: usize) {
        let Some(ui) = &mut self.anvil else { return };
        let Some(free) = ui.slots.iter().position(|s| s.is_none()) else { return };
        ui.slots[free] = self.inv.slots[inv_slot].take();
        ui.wear[free] = std::mem::take(&mut self.inv.wear[inv_slot]);
    }

    /// What the anvil would make right now.
    pub fn anvil_plan(&self) -> Option<(Id, Repair)> {
        let ui = self.anvil.as_ref()?;
        let (item, _) = ui.slots[0]?;
        plan(item, ui.wear[0], ui.slots[1], ui.wear[1]).map(|r| (item, r))
    }

    /// Clicked the result: pay, and take it (onto the cursor).
    pub fn anvil_take(&mut self) {
        let Some((item, r)) = self.anvil_plan() else { return };
        if self.inv.cursor.is_some() {
            return;
        }
        let level = level_of(self.xp).0;
        if !self.creative && level < r.cost {
            self.msg(format!("Too expensive! That needs {} level{}, and you have {level}.", r.cost, if r.cost == 1 { "" } else { "s" }));
            return;
        }
        let Some(ui) = &mut self.anvil else { return };
        let pos = ui.pos;
        let material = ui.slots[1].map(|s| s.0).unwrap_or(AIR);
        let (ench, other_ench) = (enchants(ui.wear[0]), enchants(ui.wear[1]));
        ui.slots[0] = None;
        ui.wear[0] = 0;
        ui.slots[1] = match ui.slots[1] {
            Some((id, n)) if n > r.used => Some((id, n - r.used)),
            _ => None,
        };
        if ui.slots[1].is_none() {
            ui.wear[1] = 0;
        }
        self.inv.cursor = Some((item, 1));
        self.inv.cursor_wear = r.wear;
        if !self.creative {
            self.xp = spend_levels(self.xp, r.cost);
        }
        self.sfx(Sfx::Break(Mat::Stone), Some(pos.as_vec3() + Vec3::splat(0.5)));
        self.advance("good_as_new");
        if self.is_client() {
            // The host checks it's all real, charges the levels, and may chip the anvil.
            self.net_send_msg(Msg::Repair { x: pos.x, y: pos.y, z: pos.z, item, material, used: r.used, combine: r.combine, ench, other_ench });
        } else {
            self.anvil_wear_down(pos);
        }
    }

    /// Every use might knock the anvil down a stage (where the world lives).
    pub fn anvil_wear_down(&mut self, pos: IVec3) {
        if !self.rng.chance(WEAR_CHANCE) {
            return;
        }
        let next = match self.world.get_v(pos) {
            ANVIL => ANVIL_CHIPPED,
            ANVIL_CHIPPED => ANVIL_DAMAGED,
            _ => AIR,
        };
        self.world.set_v(pos, next);
        if next == AIR {
            self.sfx(Sfx::Break(Mat::Stone), Some(pos.as_vec3() + Vec3::splat(0.5)));
            self.msg("The anvil crumbled. It did say 'Drops Ominously'.");
            self.advance("ominous");
        }
    }

    /// A joined player repaired something: check it all, then charge them.
    /// `ench` and `other_ench` are the enchantments on the two things they used.
    #[allow(clippy::too_many_arguments)]
    pub fn host_repair(&mut self, from: u32, pos: IVec3, item: Id, material: Id, used: u8, combine: bool, ench: u16, other_ench: u16) {
        if !is_anvil(self.world.get_v(pos)) || !self.peer_near(from, pos) {
            return;
        }
        // Enchantments only move if the host knows they have them.
        let (ench, other_ench) = match self.peers.get(&from).map(|p| &p.ledger) {
            Some(l) if self.peer_free(from) || (l.owns_enchanted(item, ench) && l.owns_enchanted(material, other_ench)) => (ench, other_ench),
            _ => (0, 0),
        };
        if material == ENCHANTED_BOOK && item != BOOK {
            self.host_book_onto(from, pos, item, ench, other_ench);
            return;
        }
        let Some(max) = durability(item) else { return };
        let (cost, restored, take) = if combine {
            if material != item {
                return;
            }
            (COMBINE_COST, max as u32, (item, 1u32))
        } else {
            if repair_material(item) != Some(material) || !(1..=4).contains(&used) {
                return;
            }
            (used as u32, used as u32 * max.div_ceil(4) as u32, (material, used as u32))
        };
        if !self.peer_free(from) {
            let Some(l) = self.peers.get(&from).map(|p| &p.ledger) else { return };
            let enough_items = l.bag.has(item) && l.bag.count(take.0) >= take.1 + (take.0 == item) as u32;
            if level_of(l.xp).0 < cost || !enough_items {
                // Refused: put their levels straight (their items get fixed by the next check).
                let points = l.xp;
                self.net_send_to(from, Msg::Xp { points });
                return;
            }
            self.peer_take(from, take.0, take.1);
            let p = self.peers.get_mut(&from).expect("checked");
            p.ledger.xp = spend_levels(p.ledger.xp, cost);
            // Merging two: the second one's enchantments join the first's.
            if combine && (ench != 0 || other_ench != 0) {
                let merged = enchants(merge(item, (ench as u32) << 16, (other_ench as u32) << 16));
                p.ledger.remove_enchanted(item, ench);
                p.ledger.remove_enchanted(item, other_ench);
                p.ledger.add_enchanted(item, merged, 1);
            }
            let points = p.ledger.xp;
            self.net_send_to(from, Msg::Xp { points });
            self.host_unwear(from, item, restored);
        }
        self.anvil_wear_down(pos);
    }

    /// A joined player put an enchanted book's enchantments on something.
    fn host_book_onto(&mut self, from: u32, pos: IVec3, item: Id, ench: u16, book: u16) {
        let Some(r) = plan(item, (ench as u32) << 16, Some((ENCHANTED_BOOK, 1)), (book as u32) << 16) else { return };
        if !self.peer_free(from) {
            let Some(l) = self.peers.get_mut(&from).map(|p| &mut p.ledger) else { return };
            let books = if item == ENCHANTED_BOOK { 2 } else { 1 };
            if level_of(l.xp).0 < r.cost || !l.bag.has(item) || l.bag.count(ENCHANTED_BOOK) < books {
                let points = l.xp;
                self.net_send_to(from, Msg::Xp { points });
                return;
            }
            l.bag.take(ENCHANTED_BOOK, 1);
            l.remove_enchanted(ENCHANTED_BOOK, book);
            l.remove_enchanted(item, ench);
            l.add_enchanted(item, enchants(r.wear), 1);
            l.xp = spend_levels(l.xp, r.cost);
            let points = l.xp;
            self.net_send_to(from, Msg::Xp { points });
        }
        self.anvil_wear_down(pos);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anvil_rules() {
        // Iron pickaxe (250 uses), 200 worn: each iron chunk gives back 63.
        let r = plan(PICK_IRON, 200, Some((IRON, 10)), 0).unwrap();
        assert_eq!((r.used, r.cost, r.wear, r.combine, r.book), (4, 4, 0, false, false));
        let r = plan(PICK_IRON, 100, Some((IRON, 1)), 0).unwrap();
        assert_eq!((r.used, r.wear), (1, 37));
        // The wrong stuff, or nothing to fix, does nothing.
        assert!(plan(PICK_IRON, 100, Some((DIAMOND, 1)), 0).is_none());
        assert!(plan(PICK_IRON, 0, Some((IRON, 1)), 0).is_none());
        assert!(plan(DIRT, 0, Some((DIRT, 1)), 0).is_none());
        assert!(plan(PICK_IRON, 100, None, 0).is_none());
        // Two worn swords make one better one, with a bonus.
        let r = plan(SWORD_STONE, 100, Some((SWORD_STONE, 1)), 100).unwrap();
        assert_eq!((r.combine, r.cost, r.wear), (true, COMBINE_COST, 131 - (31 + 31 + 15)));
        // Armour is mended with its own material.
        assert_eq!(repair_material(ARMOR_FIRST + 2 * 4), Some(GOLD_INGOT));
        assert_eq!(repair_material(BOW), Some(STRING));
    }
}
