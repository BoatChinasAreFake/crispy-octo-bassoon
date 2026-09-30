//! Host-checked inventories.
//!
//! Each player's inventory lives on their own machine (so using it feels
//! instant), but the host keeps a *ledger* of how many of each item every
//! joined player really has, built only from things the host saw happen:
//! blocks they broke (the host decides the drops), loot and catches it sent
//! them, recipes it let them craft, blocks they placed, arrows they shot, food
//! they ate. Anything a player tries that needs an item (placing a block,
//! planting, crafting, shooting, fishing, fertilising, even hitting harder
//! with a sword) is checked against the ledger, so a modified client can't
//! conjure items from nowhere.
//!
//! Every few seconds a client reports its counts. If they don't match, the
//! host sends the true counts back and the client adopts them, keeping any of
//! its own actions that were still on their way to the host (so nothing
//! flickers or duplicates when a check and an action cross in flight).
//!
//! Creative worlds skip all of this: everything is free there anyway.

use crate::block::*;
use crate::farming::{is_farmland, Crop};
use crate::game::Game;
use crate::net::Msg;
use std::collections::{BTreeMap, HashMap};

/// Seconds between a client's inventory checks.
pub const CHECK_SECS: f32 = 3.0;
/// Mining may run this much faster than the block's break time (lag, rounding).
const MINING_SLACK: f32 = 0.75;
/// Leftover mining time carried from one break to the next (batching over TCP).
const MINING_CARRY: f32 = 1.0;

/// What the host believes one player owns.
#[derive(Default, Debug, Clone, PartialEq)]
pub struct Bag(HashMap<Id, u32>);

impl Bag {
    pub fn count(&self, id: Id) -> u32 {
        self.0.get(&id).copied().unwrap_or(0)
    }
    pub fn has(&self, id: Id) -> bool {
        id != AIR && self.count(id) > 0
    }
    pub fn add(&mut self, id: Id, n: u32) {
        if id != AIR && n > 0 {
            *self.0.entry(id).or_insert(0) += n;
        }
    }
    /// Remove `n` if they're all there. Returns whether it could.
    pub fn take(&mut self, id: Id, n: u32) -> bool {
        match self.0.get_mut(&id) {
            Some(c) if *c >= n => {
                *c -= n;
                if *c == 0 {
                    self.0.remove(&id);
                }
                true
            }
            _ => n == 0,
        }
    }
    /// Remove up to `n` (used-up items: never an advantage, so no check needed).
    pub fn discard(&mut self, id: Id, n: u32) {
        let have = self.count(id);
        self.take(id, have.min(n));
    }
    pub fn from_items(items: &[(Id, u32)]) -> Bag {
        let mut b = Bag::default();
        for &(id, n) in items {
            if valid_item(id) {
                b.add(id, n);
            }
        }
        b
    }

    pub fn items(&self) -> Vec<(Id, u32)> {
        let mut v: Vec<(Id, u32)> = self.0.iter().map(|(&k, &n)| (k, n)).collect();
        v.sort_unstable();
        v
    }
    pub fn matches(&self, items: &[(Id, u32)]) -> bool {
        items.iter().filter(|(_, n)| *n > 0).count() == self.0.len() && items.iter().all(|&(id, n)| n == 0 || self.count(id) == n)
    }
}

/// Per joined player, on the host.
#[derive(Default)]
pub struct Ledger {
    pub bag: Bag,
    /// What they say they're holding, and its enchantments.
    pub held: Id,
    pub held_ench: u16,
    /// Game clock of their last block break, and spare mining time carried over.
    last_break: f32,
    /// Uses of each kind of tool (and enchantments) since one last wore out (see `host_wear`).
    uses: HashMap<(Id, u16), u32>,
    carry: f32,
    /// Their experience points (the host's word; see xp.rs).
    pub xp: u32,
    /// Enchanted things they own: (item, enchantments) -> how many (see enchant.rs).
    pub enchanted: HashMap<(Id, u16), u32>,
    /// How many times they've enchanted something (seeds the table's offers).
    pub enchant_count: u32,
}

impl Ledger {
    pub fn owns_enchanted(&self, item: Id, ench: u16) -> bool {
        ench == 0 || self.enchanted.get(&(item, ench)).is_some_and(|n| *n > 0)
    }

    pub fn add_enchanted(&mut self, item: Id, ench: u16, n: u32) {
        if ench != 0 {
            *self.enchanted.entry((item, ench)).or_insert(0) += n;
        }
    }

    /// One of these left them. False if they never had it.
    pub fn remove_enchanted(&mut self, item: Id, ench: u16) -> bool {
        match self.enchanted.get_mut(&(item, ench)) {
            Some(n) if *n > 0 => {
                *n -= 1;
                if *n == 0 {
                    self.enchanted.remove(&(item, ench));
                }
                true
            }
            _ => false,
        }
    }
}

/// Client side: bookkeeping for reconciling with the host's counts.
#[derive(Default)]
pub struct InvSync {
    timer: f32,
    /// Our counts when we sent the last check.
    at_check: BTreeMap<Id, u32>,
    /// Items the host gave (+) or took (-) since then.
    host_delta: BTreeMap<Id, i64>,
}

impl InvSync {
    /// Start counting afresh from these counts (after the host restored us).
    pub fn restart(&mut self, counts: BTreeMap<Id, u32>) {
        self.timer = 0.0;
        self.at_check = counts;
        self.host_delta.clear();
    }

    /// The host just gave or took items (they're already in its counts).
    pub fn note_host(&mut self, id: Id, delta: i64) {
        *self.host_delta.entry(id).or_insert(0) += delta;
    }
}

/// Is turning `old` into `new` breaking something (as opposed to placing, tilling, ...)?
pub fn is_break(old: Id, new: Id) -> bool {
    matches!(new, AIR | WATER) && targetable(old) && old != WATER
}

/// The seed item a crop block is planted from.
fn seed_of(crop: Crop) -> Id {
    match crop {
        Crop::Wheat => WHEAT_SEEDS,
        Crop::Carrot => CARROT,
        Crop::Potato => POTATO,
    }
}

impl Game {
    pub fn ledger(&mut self, from: u32) -> Option<&mut Ledger> {
        self.peers.get_mut(&from).map(|p| &mut p.ledger)
    }

    /// The item a player is holding, if they really own one (else bare hands).
    pub fn verified_held(&self, from: u32) -> Id {
        match self.peers.get(&from) {
            Some(p) if p.mode != crate::modes::GameMode::Survival || p.ledger.bag.has(p.ledger.held) => p.ledger.held,
            _ => AIR,
        }
    }

    /// The enchantments on what a player is holding, if the host knows they really have them.
    pub fn verified_ench(&self, from: u32) -> u16 {
        match self.peers.get(&from) {
            Some(p) if p.mode != crate::modes::GameMode::Survival || p.ledger.owns_enchanted(self.verified_held(from), p.ledger.held_ench) => p.ledger.held_ench,
            _ => 0,
        }
    }

    /// An item is leaving a joined player (thrown, dropped, put in a chest): its
    /// enchantments go with it only if the host knew about them.
    pub fn launder(&mut self, from: u32, item: Id, wear: crate::inventory::Wear) -> crate::inventory::Wear {
        let ench = (wear >> 16) as u16;
        if ench == 0 || self.peer_free(from) {
            return wear;
        }
        match self.ledger(from) {
            Some(l) if l.owns_enchanted(item, ench) => {
                l.remove_enchanted(item, ench);
                wear
            }
            _ => wear & 0xFFFF,
        }
    }

    /// Does this player own at least one? (Always yes in creative.)
    pub fn peer_has(&self, from: u32, id: Id) -> bool {
        self.peer_free(from) || self.peers.get(&from).map(|p| p.ledger.bag.has(id)).unwrap_or(false)
    }

    /// Use up one of the player's items; false if they don't have it. (Free in creative.)
    pub fn peer_take(&mut self, from: u32, id: Id, n: u32) -> bool {
        if self.peer_free(from) {
            return true;
        }
        self.ledger(from).map(|l| l.bag.take(id, n)).unwrap_or(false)
    }

    /// Give a joined player items: into the ledger, then over the network.
    pub fn give_peer(&mut self, from: u32, item: Id, n: u8) {
        self.give_peer_worn(from, item, n, 0);
    }

    /// The same, for a used tool or piece of armour.
    pub fn give_peer_worn(&mut self, from: u32, item: Id, n: u8, wear: crate::inventory::Wear) {
        if n == 0 || !valid_item(item) {
            return;
        }
        if let Some(l) = self.ledger(from) {
            l.bag.add(item, n as u32);
            l.add_enchanted(item, (wear >> 16) as u16, n as u32);
        }
        self.net_send_to(from, Msg::Give { item, n, wear });
    }

    /// Take items from a joined player (scripts' `take`): the ledger, then theirs.
    pub fn take_peer(&mut self, from: u32, item: Id, n: u8) {
        if let Some(l) = self.ledger(from) {
            l.bag.discard(item, n as u32);
        }
        self.net_send_to(from, Msg::Effect { heal: 0.0, teleport: None, launch: None, take: Some((item, n)) });
    }

    pub fn set_peer_held(&mut self, from: u32, held: Id, held_ench: u16) {
        if let Some(l) = self.ledger(from) {
            l.held = held;
            l.held_ench = held_ench;
        }
    }

    /// Check a joined player's block edit against what they own, and account
    /// for it: breaking pays out drops (at no more than mining speed), placing
    /// and planting cost the item, tilling needs a hoe. False: refuse the edit.
    pub fn ledger_edit(&mut self, from: u32, at: macroquad::math::IVec3, old: Id, new: Id) -> bool {
        if self.peer_mode(from) == crate::modes::GameMode::Spectator {
            return false;
        }
        if self.peer_free(from) {
            return true;
        }
        let held = self.verified_held(from);
        let ench = self.verified_ench(from);
        if is_break(old, new) {
            use crate::enchant::{level, Enchant};
            let (t, drops) = break_time_with(old, held, level((ench as u32) << 16, Enchant::Efficiency));
            if !t.is_finite() {
                return false;
            }
            let clock = self.clock;
            let Some(l) = self.ledger(from) else { return false };
            let available = (clock - l.last_break).max(0.0) + l.carry;
            let needed = t * MINING_SLACK;
            if available < needed {
                return false; // faster than their tools allow
            }
            l.last_break = clock;
            l.carry = (available - needed).min(MINING_CARRY);
            self.host_wear(from, held, dig_wear(held, old));
            // The drops land on the ground here; they get them by walking into them.
            if drops {
                let center = at.as_vec3() + macroquad::math::Vec3::splat(0.5);
                let roll = self.rng.f32();
                let n = fortune_count(old, level((ench as u32) << 16, Enchant::Fortune), roll);
                self.pop_drop(center, block(old).drop, n);
                let points = crate::xp::ore_xp(old, &mut self.rng);
                self.spawn_orbs(center, points);
                for (item, n) in crate::farming::random_drops(old, &mut self.rng) {
                    self.pop_drop(center, item, n);
                }
                let on_break: &'static [Action] = &block(old).on_break;
                for a in on_break {
                    if let Action::Give(item, n) = a {
                        self.give_peer(from, *item, *n);
                    }
                }
            }
            return true;
        }
        // Doors, gates and trapdoors swing for free; a slab onto a slab costs the second slab.
        if is_door(old) && is_door(new) {
            return true;
        }
        if let (Some(a), Some(b)) = (crate::carpentry::family(old), crate::carpentry::family(new))
            && a == b
        {
            return true;
        }
        if crate::contraptions::is_comparator(old) && crate::contraptions::is_comparator(new) {
            return true;
        }
        if crate::beacon::is_beacon(old) && crate::beacon::is_beacon(new) {
            return true;
        }
        // Goo waxes copper.
        if crate::copper::waxed(old) == Some(new) {
            return self.peer_take(from, GOO, 1);
        }
        // Fire comes from a Sparker (which wears a little).
        if new == FIRE {
            let ok = self.peer_has(from, SPARKER);
            if ok {
                self.host_wear(from, SPARKER, 1);
            }
            return ok;
        }
        if let Some((family, _)) = slab_of(old)
            && new == made_of(old)
            && new != AIR
        {
            return self.peer_take(from, family, 1);
        }
        if is_farmland(new) && !is_farmland(old) {
            let ok = self.peer_has(from, HOE);
            if ok {
                self.host_wear(from, HOE, 1);
            }
            return ok;
        }
        if let Some((crop, 0)) = Crop::of_block(new) {
            return self.peer_take(from, seed_of(crop), 1);
        }
        // Buckets: scooping up a source fills one, pouring one out empties it.
        // (A sponge nearby drinks water for free.)
        if matches!(old, WATER | LAVA) && new == AIR {
            let sponge = old == WATER && (-3..=3).any(|dy| (-3..=3).any(|dz| (-3..=3).any(|dx| self.world.get_v(at + macroquad::math::IVec3::new(dx, dy, dz)) == SPONGE)));
            if sponge {
                return true;
            }
            if !self.peer_take(from, BUCKET, 1) {
                return false;
            }
            let full = if old == WATER { WATER_BUCKET } else { LAVA_BUCKET };
            if let Some(l) = self.ledger(from) {
                l.bag.add(full, 1);
            }
            return true;
        }
        if matches!(new, WATER | LAVA) && old != ICE {
            let full = if new == WATER { WATER_BUCKET } else { LAVA_BUCKET };
            if !self.peer_take(from, full, 1) {
                return false;
            }
            if let Some(l) = self.ledger(from) {
                l.bag.add(BUCKET, 1);
            }
            return true;
        }
        if replaceable(old) && new != AIR && !is_liquid(new) {
            return match placing_item(new) {
                Some(item) => self.peer_take(from, item, 1),
                // A door's top half comes with the bottom (edit_allowed checked it's there).
                None => is_door(new),
            };
        }
        // Trampling, melting, water flowing, sponges drinking: nothing to pay.
        true
    }

    /// A repair at an anvil gave back `restored` uses of this kind of tool.
    pub fn host_unwear(&mut self, from: u32, item: Id, restored: u32) {
        let ench = if self.verified_held(from) == item { self.verified_ench(from) } else { 0 };
        if let Some(l) = self.ledger(from) {
            let u = l.uses.entry((item, ench)).or_insert(0);
            *u = u.saturating_sub(restored);
        }
    }

    /// A joined player used a tool `amount` times. The host can't see which of
    /// their pickaxes they held, so it counts uses per kind of tool (and its
    /// enchantments): every time they add up to one tool's durability (more
    /// with Unbreaking), one of that kind is worn out and leaves the ledger.
    /// Their own game breaks the same tool at the same moment; if a modified
    /// one doesn't, the next inventory check takes it anyway.
    pub fn host_wear(&mut self, from: u32, item: Id, amount: u16) {
        let ench = if self.verified_held(from) == item { self.verified_ench(from) } else { 0 };
        let Some(max) = crate::inventory::max_uses(item, (ench as u32) << 16) else { return };
        if amount == 0 || self.peer_free(from) {
            return;
        }
        let Some(l) = self.ledger(from) else { return };
        if !l.bag.has(item) {
            return;
        }
        let u = l.uses.entry((item, ench)).or_insert(0);
        *u += amount as u32;
        if *u >= max {
            *u -= max;
            l.bag.take(item, 1);
            l.remove_enchanted(item, ench);
        }
    }

    /// A joined player crafted: apply it to the ledger, as far as their items allow.
    pub fn host_craft(&mut self, from: u32, recipe: u16, times: u8) {
        if self.peer_free(from) {
            return;
        }
        let Some(r) = recipes().get(recipe as usize) else { return };
        let r = r.clone();
        let Some(l) = self.ledger(from) else { return };
        for _ in 0..times.min(64) {
            if !r.inputs.iter().all(|&(id, n)| l.bag.count(id) >= n as u32) {
                break;
            }
            for &(id, n) in &r.inputs {
                l.bag.take(id, n as u32);
            }
            l.bag.add(r.output.0, r.output.1 as u32);
        }
    }

    /// A joined player used things up (eating, yeeting, ...).
    pub fn host_consume(&mut self, from: u32, item: Id, n: u8) {
        if let Some(l) = self.ledger(from) {
            let had = l.bag.count(item);
            l.bag.discard(item, n as u32);
            // Drinking a potion leaves the bottle (thrown splash potions go via `host_splash`).
            if matches!(crate::potions::potion_of(item), Some((_, false))) {
                l.bag.add(GLASS_BOTTLE, had.min(n as u32));
            }
        }
    }

    /// A joined player's periodic "here's what I have": correct them if they're wrong.
    pub fn host_inventory_check(&mut self, from: u32, items: Vec<(Id, u32)>) {
        if self.peer_free(from) {
            return;
        }
        let Some(l) = self.ledger(from) else { return };
        if !l.bag.matches(&items) {
            let items = l.bag.items();
            self.net_send_to(from, Msg::Inventory { items });
        }
    }

    // ---------------------------------------------------------- client side

    /// Use up the held item (eating, throwing, yeeting). Joined players tell the
    /// host, which keeps its ledger in step.
    pub fn use_up_held(&mut self) {
        let held = self.inv.held();
        if held == AIR {
            return;
        }
        self.inv.consume_held();
        if self.is_client() && !self.creative {
            self.net_send_msg(Msg::Consume { item: held, n: 1 });
        }
    }

    /// Joined players in survival: send our counts every few seconds.
    pub fn inventory_sync_tick(&mut self, dt: f32) {
        if !self.is_client() || self.creative {
            return;
        }
        self.inv_sync.timer += dt;
        if self.inv_sync.timer < CHECK_SECS {
            return;
        }
        self.inv_sync.timer = 0.0;
        let counts = self.inv.counts();
        let items = counts.iter().map(|(&k, &n)| (k, n)).collect();
        self.inv_sync.at_check = counts;
        self.inv_sync.host_delta.clear();
        self.net_send_msg(Msg::InventoryCheck { items });
    }

    /// The host says our counts were wrong: adopt its numbers, plus whatever we've
    /// done ourselves since the check (still on its way to the host).
    pub fn apply_inventory(&mut self, items: Vec<(Id, u32)>) {
        let now = self.inv.counts();
        let mut target: BTreeMap<Id, u32> = items.into_iter().filter(|(id, _)| valid_item(*id)).collect();
        let keys: Vec<Id> = now.keys().chain(self.inv_sync.at_check.keys()).chain(self.inv_sync.host_delta.keys()).copied().collect();
        for id in keys {
            let ours = now.get(&id).copied().unwrap_or(0) as i64
                - self.inv_sync.at_check.get(&id).copied().unwrap_or(0) as i64
                - self.inv_sync.host_delta.get(&id).copied().unwrap_or(0);
            if ours != 0 {
                let e = target.entry(id).or_insert(0);
                *e = (*e as i64 + ours).max(0) as u32;
            }
        }
        target.retain(|_, n| *n > 0);
        self.inv.set_counts(&target);
        self.inv_sync.at_check = self.inv.counts();
        self.inv_sync.host_delta.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bags_count() {
        let mut b = Bag::default();
        b.add(COBBLE, 3);
        assert!(b.take(COBBLE, 2));
        assert!(!b.take(COBBLE, 2), "not enough");
        assert_eq!(b.count(COBBLE), 1);
        b.discard(COBBLE, 5);
        assert!(!b.has(COBBLE));
        b.add(STICK, 4);
        assert!(b.matches(&[(STICK, 4)]));
        assert!(b.matches(&[(STICK, 4), (DIRT, 0)]));
        assert!(!b.matches(&[(STICK, 4), (DIRT, 1)]));
        assert!(!b.matches(&[]));
        assert!(!b.has(AIR));
    }
}
