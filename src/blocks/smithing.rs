//! Two workbenches for after you've got everything:
//!
//! - The **Grindstone** strips enchantments off a tool, weapon, piece of
//!   armour or enchanted book and gives some of the experience back (books
//!   go back to being plain books). Put two of the same worn thing on it and
//!   it grinds them into one, with their uses added up and a little bonus,
//!   but no enchantments.
//! - The **Smithing Table** upgrades Dimond gear to **Scorchite**: a Scorchite
//!   Upgrade Template (found in Hushed Cities), the Dimond tool or armour,
//!   and a Scorchite Ingot (four Scorchite Scrap smelted from Old Debris,
//!   deep in the Scorchlands, plus four gold). The result keeps its
//!   enchantments and wear. Scorchite hits harder, digs faster, lasts longer,
//!   and its armour shrugs off knockback; dropped Scorchite floats in lava.
//!   Templates can be copied: seven dimonds, a template and some cobbled
//!   deepslate make two.
//!
//! Like the anvil, neither keeps anything between uses. Joined players'
//! hosts check what they really had.

use crate::block::*;
use crate::enchant::{enchants, is_enchanted, total_levels};
use crate::game::Game;
use crate::inventory::{click_stack, right_click_stack, uses, wear_follow, with_uses, Stack, Wear};
use crate::net::Msg;
use crate::sound::{Mat, Sfx};
use macroquad::math::{IVec3, Vec3};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Bench {
    Grindstone,
    Smithing,
}

impl Bench {
    pub fn of_block(id: Id) -> Option<Bench> {
        match id {
            GRINDSTONE => Some(Bench::Grindstone),
            SMITHING_TABLE => Some(Bench::Smithing),
            _ => None,
        }
    }
    pub fn block(self) -> Id {
        match self {
            Bench::Grindstone => GRINDSTONE,
            Bench::Smithing => SMITHING_TABLE,
        }
    }
    pub fn slots(self) -> usize {
        match self {
            Bench::Grindstone => 2,
            Bench::Smithing => 3,
        }
    }
}

/// The Scorchite version of a Dimond thing.
pub fn upgraded(id: Id) -> Option<Id> {
    Some(match id {
        PICK_DIAMOND => PICK_SCORCHITE,
        SWORD_DIAMOND => SWORD_SCORCHITE,
        _ if id == AXE_FIRST + 4 => AXE_SCORCHITE,
        _ if id == SHOVEL_FIRST + 4 => SHOVEL_SCORCHITE,
        _ if (ARMOR_FIRST + 12..ARMOR_FIRST + 16).contains(&id) => SCORCHITE_ARMOR_FIRST + (id - ARMOR_FIRST - 12),
        _ => return None,
    })
}

pub fn is_scorchite(id: Id) -> bool {
    matches!(id, PICK_SCORCHITE | SWORD_SCORCHITE | AXE_SCORCHITE | SHOVEL_SCORCHITE | SCORCHITE_INGOT | SCORCHITE_SCRAP | OLD_DEBRIS) || (SCORCHITE_ARMOR_FIRST..SCORCHITE_ARMOR_FIRST + 4).contains(&id)
}

/// Experience points back for grinding off enchantments.
pub fn grind_xp(w: Wear) -> u32 {
    total_levels(w) * 3 + is_enchanted(w) as u32 * 2
}

/// What a bench would make from what's on it: (item, wear, xp back).
pub fn plan(bench: Bench, slots: &[Stack], wear: &[Wear]) -> Option<(Id, Wear, u32)> {
    match bench {
        Bench::Grindstone => {
            let (a, b) = (slots[0], slots[1]);
            match (a, b) {
                (Some((id, _)), None) | (None, Some((id, _))) => {
                    let w = if a.is_some() { wear[0] } else { wear[1] };
                    if !is_enchanted(w) {
                        return None;
                    }
                    let out = if id == ENCHANTED_BOOK { BOOK } else { id };
                    let keep = if id == ENCHANTED_BOOK { 0 } else { w & crate::inventory::NOT_ENCHANTS };
                    Some((out, keep, grind_xp(w)))
                }
                (Some((x, _)), Some((y, _))) if x == y => {
                    let max = durability(x)?;
                    let left = (max - uses(wear[0]).min(max)) as u32 + (max - uses(wear[1]).min(max)) as u32 + max as u32 * 5 / 100;
                    let new = max.saturating_sub(left.min(max as u32) as u16);
                    Some((x, with_uses(0, new), grind_xp(wear[0]) + grind_xp(wear[1])))
                }
                _ => None,
            }
        }
        Bench::Smithing => {
            let (Some((t, _)), Some((item, _)), Some((ingot, _))) = (slots[0], slots[1], slots[2]) else { return None };
            if crate::trims::is_template(t) {
                // A trim (see trims.rs).
                return Some((item, crate::trims::trimmed(t, item, wear[1], ingot)?, 0));
            }
            if t != UPGRADE_TEMPLATE || ingot != SCORCHITE_INGOT {
                return None;
            }
            Some((upgraded(item)?, wear[1], 0))
        }
    }
}

/// What's on an open grindstone or smithing table (only on the player's own screen).
pub struct BenchUi {
    pub pos: IVec3,
    pub bench: Bench,
    pub slots: [Stack; 3],
    pub wear: [Wear; 3],
}

impl Game {
    pub fn open_bench(&mut self, pos: IVec3, bench: Bench) {
        self.bench = Some(BenchUi { pos, bench, slots: [None; 3], wear: [0; 3] });
        self.sfx(Sfx::Place(Mat::Stone), Some(pos.as_vec3() + Vec3::splat(0.5)));
    }

    /// Walked away: whatever's on it goes back in the inventory.
    pub fn close_bench(&mut self) {
        self.inv.return_cursor();
        if let Some(ui) = self.bench.take() {
            for (s, w) in ui.slots.into_iter().zip(ui.wear) {
                if let Some((id, n)) = s {
                    self.give_worn(id, n, w);
                }
            }
        }
    }

    pub fn bench_still_there(&self) -> bool {
        self.bench.as_ref().is_some_and(|b| self.world.get_v(b.pos) == b.bench.block())
    }

    pub fn bench_click(&mut self, slot: usize, right: bool) {
        let Some(ui) = &mut self.bench else { return };
        let before = (ui.slots[slot], self.inv.cursor);
        if right {
            right_click_stack(&mut ui.slots[slot], &mut self.inv.cursor);
        } else {
            click_stack(&mut ui.slots[slot], &mut self.inv.cursor);
        }
        wear_follow(before, (ui.slots[slot], self.inv.cursor), &mut ui.wear[slot], &mut self.inv.cursor_wear);
    }

    /// Shift-clicked an inventory slot: onto the slot it belongs in.
    pub fn bench_quick_put(&mut self, inv_slot: usize) {
        let Some(ui) = &mut self.bench else { return };
        let Some((id, _)) = self.inv.slots[inv_slot] else { return };
        let want = match ui.bench {
            Bench::Smithing if id == UPGRADE_TEMPLATE || crate::trims::is_template(id) => Some(0),
            Bench::Smithing if id == SCORCHITE_INGOT || crate::trims::material_index(id).is_some() => Some(2),
            Bench::Smithing => Some(1),
            Bench::Grindstone => ui.slots[..2].iter().position(|s| s.is_none()),
        };
        let Some(free) = want.filter(|&i| ui.slots[i].is_none()) else { return };
        ui.slots[free] = self.inv.slots[inv_slot].take();
        ui.wear[free] = std::mem::take(&mut self.inv.wear[inv_slot]);
    }

    pub fn bench_plan(&self) -> Option<(Id, Wear, u32)> {
        let ui = self.bench.as_ref()?;
        let n = ui.bench.slots();
        plan(ui.bench, &ui.slots[..n], &ui.wear[..n])
    }

    /// Clicked the result: take it (onto the cursor).
    pub fn bench_take(&mut self) {
        let Some((item, wear, xp)) = self.bench_plan() else { return };
        if self.inv.cursor.is_some() {
            return;
        }
        let Some(ui) = &mut self.bench else { return };
        let (pos, bench) = (ui.pos, ui.bench);
        let trim = ui.slots[0].map(|s| s.0).filter(|&t| bench == Bench::Smithing && crate::trims::is_template(t)).zip(ui.slots[2].map(|s| s.0));
        let used: Vec<(Id, u16)> = ui.slots.iter().zip(ui.wear).map(|(s, w)| s.map(|(id, _)| (id, enchants(w))).unwrap_or((AIR, 0))).collect();
        for i in 0..bench.slots() {
            ui.slots[i] = match ui.slots[i] {
                Some((id, n)) if n > 1 => Some((id, n - 1)),
                _ => None,
            };
            if ui.slots[i].is_none() {
                ui.wear[i] = 0;
            }
        }
        self.inv.cursor = Some((item, 1));
        self.inv.cursor_wear = wear;
        let at = pos.as_vec3() + Vec3::splat(0.5);
        match bench {
            Bench::Grindstone => {
                self.sfx(Sfx::Grind, Some(at));
                if xp > 0 && !self.is_client() {
                    self.spawn_orbs(at + Vec3::Y * 0.6, xp);
                }
                self.advance("clean_slate");
            }
            Bench::Smithing if trim.is_some() => {
                self.sfx(Sfx::Place(Mat::Glass), Some(at));
                self.advance("dressed_up");
            }
            Bench::Smithing => {
                self.sfx(Sfx::Break(Mat::Stone), Some(at));
                self.advance("scorchite");
            }
        }
        if self.is_client() {
            // Grinding: the two things ground; smithing: the gear (the template and ingot are implied).
            let pick = |i: usize| used.get(i).copied().unwrap_or((AIR, 0));
            // (A trim: `b` is the template, and its "enchantments" the material.)
            let (a, b) = match trim {
                _ if bench == Bench::Grindstone => (pick(0), pick(1)),
                Some((template, material)) => (pick(1), (template, material)),
                None => (pick(1), (AIR, 0)),
            };
            self.net_send_msg(Msg::Smith { x: pos.x, y: pos.y, z: pos.z, grind: bench == Bench::Grindstone, a: a.0, a_ench: a.1, b: b.0, b_ench: b.1 });
        }
    }

    /// A joined player used a grindstone (`grind`) or smithing table. `a`/`b`:
    /// what they put on it (for smithing, `a` is the gear) with enchantments.
    #[allow(clippy::too_many_arguments)]
    pub fn host_smith(&mut self, from: u32, pos: IVec3, grind: bool, a: Id, a_ench: u16, b: Id, b_ench: u16) {
        let want = if grind { GRINDSTONE } else { SMITHING_TABLE };
        if self.world.get_v(pos) != want || !self.peer_near(from, pos) || self.peer_free(from) {
            return;
        }
        let Some(l) = self.peers.get_mut(&from).map(|p| &mut p.ledger) else { return };
        if grind {
            // One thing ground clean, or two of the same ground into one.
            let mut xp = 0;
            for (id, ench) in [(a, a_ench), (b, b_ench)] {
                if id == AIR || !l.bag.has(id) {
                    continue;
                }
                if ench != 0 && l.remove_enchanted(id, ench) {
                    xp += grind_xp((ench as u32) << 16);
                }
            }
            if a == ENCHANTED_BOOK && l.bag.has(ENCHANTED_BOOK) {
                l.bag.take(ENCHANTED_BOOK, 1);
                l.bag.add(BOOK, 1);
            }
            if b != AIR && b == a {
                l.bag.take(b, 1);
            }
            if xp > 0 {
                let at = pos.as_vec3() + Vec3::new(0.5, 1.1, 0.5);
                self.spawn_orbs(at, xp);
            }
        } else if crate::trims::is_template(b) {
            // A trim: the template and material are used up; the armour stays (trimmed).
            let material = b_ench;
            if crate::trims::material_index(material).is_none() || armor_of(a).is_none() || !l.bag.has(a) || !l.bag.has(b) || !l.bag.has(material) {
                return;
            }
            l.bag.take(b, 1);
            l.bag.take(material, 1);
        } else {
            // a: the Dimond gear; the template and ingot must be there too.
            let Some(up) = upgraded(a) else { return };
            if !l.bag.has(a) || !l.bag.has(UPGRADE_TEMPLATE) || !l.bag.has(SCORCHITE_INGOT) {
                return;
            }
            l.bag.take(a, 1);
            l.bag.take(UPGRADE_TEMPLATE, 1);
            l.bag.take(SCORCHITE_INGOT, 1);
            l.bag.add(up, 1);
            if a_ench != 0 && l.remove_enchanted(a, a_ench) {
                l.add_enchanted(up, a_ench, 1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enchant::{with_level, Enchant};

    #[test]
    fn grinding_and_smithing() {
        let sharp = with_level(40, Enchant::Sharpness, 3);
        let (out, wear, xp) = plan(Bench::Grindstone, &[Some((SWORD_IRON, 1)), None], &[sharp, 0]).unwrap();
        assert_eq!((out, wear, xp > 0), (SWORD_IRON, 40, true), "enchantments off, wear kept, xp back");
        assert!(plan(Bench::Grindstone, &[Some((SWORD_IRON, 1)), None], &[40, 0]).is_none(), "nothing to grind");
        let book = with_level(0, Enchant::Fortune, 2);
        assert_eq!(plan(Bench::Grindstone, &[None, Some((ENCHANTED_BOOK, 1))], &[0, book]).unwrap().0, BOOK);
        let (_, w, _) = plan(Bench::Grindstone, &[Some((PICK_IRON, 1)), Some((PICK_IRON, 1))], &[200, 200]).unwrap();
        assert!(uses(w) < 200, "two worn picks make a better one");
        let slots = [Some((UPGRADE_TEMPLATE, 1)), Some((PICK_DIAMOND, 1)), Some((SCORCHITE_INGOT, 1))];
        let (up, w, _) = plan(Bench::Smithing, &slots, &[0, sharp, 0]).unwrap();
        assert_eq!((up, w), (PICK_SCORCHITE, sharp), "upgrades keep enchantments and wear");
        assert!(plan(Bench::Smithing, &[slots[0], Some((PICK_IRON, 1)), slots[2]], &[0; 3]).is_none());
        assert_eq!(upgraded(ARMOR_FIRST + 12 + 1), Some(SCORCHITE_ARMOR_FIRST + 1));
        assert_eq!(upgraded(AXE_FIRST + 4), Some(AXE_SCORCHITE));
        assert!(durability(PICK_SCORCHITE).unwrap() > durability(PICK_DIAMOND).unwrap());
        assert!(break_time_with(STONE, PICK_SCORCHITE, 0).0 < break_time_with(STONE, PICK_DIAMOND, 0).0);
    }
}
