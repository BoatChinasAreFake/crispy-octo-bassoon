//! **Shop stalls** (an original): set out your wares, put a price on each
//! lot, and the Hmmers of a village nearby come by in the daytime and buy.
//!
//! - A Shop Stall has nine slots for what's for sale and a till. Each slot
//!   is a lot: a Hmmer buys the whole stack in it, for the price you set
//!   (in Gold Ingots, 0 for not for sale), and the gold goes in the till.
//! - The first player to open a new stall owns it; nobody else can open it.
//! - Every so often in the daytime, a Hmmer from within 24 blocks wanders
//!   over and looks at what's out. It buys a lot if the price is fair for
//!   what it's worth to the village now, and is likelier to the cheaper it is.
//! - **Demand** rises and falls with what the village needs: things on its
//!   Notice Board's Bring bounties are wanted more; food more the more
//!   Hmmers there are to feed; and each sale of a thing satisfies the
//!   village a little, for a while. The stall shows how each lot is doing.
//!
//! The stock is a container like any other (see containers.rs), so joined
//! players' moves are checked by the host; prices go through the host too.

use crate::block::*;
use crate::entity::MobKind;
use crate::game::Game;
use crate::net::Msg;
use macroquad::math::{IVec3, Vec3};
use std::collections::HashMap;

/// Lots for sale, and the till after them.
pub const LOTS: usize = 9;
pub const TILL: usize = LOTS;
pub const SLOTS: usize = LOTS + 1;
/// The most a lot can cost.
pub const MAX_PRICE: u8 = 64;
/// How far (blocks) Hmmers come from to shop.
pub const CATCHMENT: f32 = 24.0;
/// Seconds between customers (at least, and a random amount more).
const CUSTOMER_EVERY: f32 = 20.0;
const CUSTOMER_JITTER: f32 = 20.0;
/// Seconds a customer has to get to the stall before it gives up.
const CUSTOMER_PATIENCE: f32 = 25.0;
/// How much one sale satisfies a village's want of a thing, and how fast it comes back (a minute).
const GLUT_PER_SALE: f32 = 0.25;
const GLUT_RECOVERY: f32 = 0.05;
/// Gold sold for "Nest Egg".
pub const NEST_EGG: u32 = 64;

/// What a single one of a thing is worth to a village, in gold (0: not wanted at all).
pub fn value(id: Id) -> f32 {
    match id {
        WHEAT => 0.15,
        CARROT | POTATO => 0.2,
        BAKED_POTATO => 0.4,
        BREAD => 0.5,
        APPLE => 0.25,
        PUMPKIN => 0.5,
        MELON_SLICE => 0.12,
        CAKE => 3.0,
        COD | CLUCKETS | PORKCHOP | MUTTON => 0.3,
        COOKED_COD | COOKED_CLUCKETS | COOKED_MUTTON => 0.6,
        STEAK | COOKED_CHOP => 0.7,
        FISH_CHIPS => 1.0,
        WHEAT_SEEDS => 0.03,
        COAL => 0.25,
        IRON => 1.5,
        COPPER_INGOT => 0.4,
        DIAMOND => 6.0,
        LOG => 0.2,
        PLANKS => 0.05,
        COBBLE => 0.06,
        GLASS => 0.15,
        TORCH => 0.05,
        WOOL => 0.5,
        STRING => 0.3,
        FEATHER => 0.2,
        BONE => 0.3,
        GUNPOWDER => 1.0,
        BOOK => 1.5,
        _ => 0.0,
    }
}

/// Is this something people eat?
pub fn is_food(id: Id) -> bool {
    matches!(id, WHEAT | CARROT | POTATO | BAKED_POTATO | BREAD | APPLE | PUMPKIN | MELON_SLICE | CAKE | COD | CLUCKETS | PORKCHOP | MUTTON | COOKED_COD | COOKED_CLUCKETS | COOKED_MUTTON | STEAK | COOKED_CHOP | FISH_CHIPS)
}

/// How a lot is selling, in a word.
pub fn demand_word(demand: f32) -> &'static str {
    match demand {
        d if d >= 1.4 => "Wanted!",
        d if d >= 0.85 => "Steady",
        d if d >= 0.5 => "Slow",
        _ => "Glutted",
    }
}

/// Would a Hmmer pay `price` for a lot worth `fair` to it, and how likely is it to?
pub fn buy_chance(price: u8, fair: f32) -> f32 {
    if price == 0 || fair <= 0.0 {
        return 0.0;
    }
    let p = price as f32;
    if p <= fair * 0.8 {
        0.9
    } else if p <= fair {
        0.7
    } else if p <= fair * 1.25 {
        0.35
    } else {
        0.0
    }
}

/// A stall as its owner sees it: who owns it, the prices, and each lot's (demand, fair price).
pub type StallView = (String, [u8; LOTS], [(f32, f32); LOTS]);

/// One stall: whose it is, its prices, and how it's done.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stall {
    pub owner: String,
    pub prices: [u8; LOTS],
    /// Gold taken, ever.
    pub sold: u32,
    /// (Not saved.) Seconds to the next customer, and who's on their way (mob id, seconds left).
    pub next: f32,
    pub customer: Option<(u32, f32)>,
}

/// Every stall, and how satisfied each village is with what it's bought.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shops {
    pub stalls: HashMap<IVec3, Stall>,
    /// (village, item) -> how much it still wants it (0.25 to 1).
    pub glut: HashMap<(IVec3, Id), f32>,
    /// (Not saved.) Seconds to the next look round.
    pub look: f32,
    /// (A joined player's game.) What the host said about the stall we have open: its
    /// owner, prices, and how each lot would do (demand, fair price).
    pub seen: Option<(IVec3, StallView)>,
}

impl Shops {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = String::new();
        let mut v: Vec<_> = self.stalls.iter().collect();
        v.sort_by_key(|(p, _)| p.to_array());
        for (p, s) in v {
            let prices: Vec<String> = s.prices.iter().map(|n| n.to_string()).collect();
            out.push_str(&format!("stall\t{}\t{}\t{}\t{}\t{}\t{}\n", p.x, p.y, p.z, s.owner, s.sold, prices.join(",")));
        }
        let mut g: Vec<_> = self.glut.iter().collect();
        g.sort_by_key(|((p, id), _)| (p.to_array(), *id));
        for ((p, id), n) in g {
            out.push_str(&format!("glut\t{}\t{}\t{}\t{}\t{n}\n", p.x, p.y, p.z, reg().key_of(*id)));
        }
        out.into_bytes()
    }

    pub fn decode(b: &[u8]) -> Shops {
        let mut out = Shops::default();
        for line in String::from_utf8_lossy(b).lines() {
            let f: Vec<&str> = line.split('\t').collect();
            match f[..] {
                ["stall", x, y, z, owner, sold, prices] => {
                    let (Ok(x), Ok(y), Ok(z)) = (x.parse(), y.parse(), z.parse()) else { continue };
                    let mut s = Stall { owner: owner.to_string(), sold: sold.parse().unwrap_or(0), ..Default::default() };
                    for (i, n) in prices.split(',').take(LOTS).enumerate() {
                        s.prices[i] = n.parse::<u8>().unwrap_or(0).min(MAX_PRICE);
                    }
                    out.stalls.insert(IVec3::new(x, y, z), s);
                }
                ["glut", x, y, z, key, n] => {
                    let (Ok(x), Ok(y), Ok(z), Ok(n)) = (x.parse(), y.parse(), z.parse(), n.parse::<f32>()) else { continue };
                    if let Some(id) = reg().lookup(key) {
                        out.glut.insert((IVec3::new(x, y, z), id), n.clamp(0.25, 1.0));
                    }
                }
                _ => {}
            }
        }
        out
    }
}

impl Game {
    /// Right-clicked a Shop Stall: open it if it's ours (or nobody's yet).
    pub fn open_stall(&mut self, pos: IVec3) {
        if !self.is_client() {
            let me = crate::players::record_key(&self.player_name);
            if !self.claim_stall(pos, &me) {
                let owner = self.shops.stalls.get(&pos).map(|s| s.owner.clone()).unwrap_or_default();
                self.msg(format!("This is {owner}'s stall. (Look, don't touch.)"));
                return;
            }
        }
        self.shops.seen = None;
        self.open_container(pos);
    }

    /// `who` (a record key) may use the stall at `pos`: theirs, or nobody's yet (and now theirs).
    pub fn claim_stall(&mut self, pos: IVec3, who: &str) -> bool {
        let s = self.shops.stalls.entry(pos).or_default();
        if s.owner.is_empty() {
            s.owner = who.to_string();
        }
        s.owner == who
    }

    /// How much the village round a stall wants a thing now (1 is ordinary).
    pub fn demand_for(&self, village: IVec3, id: Id) -> f32 {
        let mut d = 1.0;
        // Wanted on the Notice Board (this posting's Bring bounties).
        let posting = crate::bounties::posting(self.day);
        if crate::bounties::bounties(village, posting).iter().any(|b| matches!(b.task, crate::bounties::Task::Bring(i, _) if i == id)) {
            d *= 1.6;
        }
        // Food: more mouths, more want.
        if is_food(id) {
            let centre = village.as_vec3();
            let mouths = self.mobs.iter().filter(|m| m.kind == MobKind::Hmmer && m.health > 0.0 && m.body.pos.distance(centre) < crate::raids::VILLAGE_RANGE).count();
            d *= (0.7 + 0.1 * mouths as f32).min(1.5);
        }
        d * self.shops.glut.get(&(village, id)).copied().unwrap_or(1.0)
    }

    /// What a lot (a stack of `n` of `id`) is fairly worth to the village round `pos`, and its demand.
    pub fn lot_worth(&self, pos: IVec3, id: Id, n: u8) -> (f32, f32) {
        let village = self.village_key(pos.as_vec3());
        let d = self.demand_for(village, id);
        (value(id) * n as f32 * d, d)
    }

    /// The stall we have open: its owner, prices, and each lot's (demand, fair price).
    pub fn stall_view(&self) -> Option<StallView> {
        let pos = self.open?;
        if self.world.get_v(pos) != SHOP_STALL {
            return None;
        }
        if self.is_client() {
            return self.shops.seen.as_ref().filter(|s| s.0 == pos).map(|s| s.1.clone());
        }
        let s = self.shops.stalls.get(&pos)?;
        let c = self.world.containers.get(&pos);
        let mut lots = [(1.0, 0.0); LOTS];
        for (i, lot) in lots.iter_mut().enumerate() {
            if let Some((id, n)) = c.and_then(|c| c.slots.get(i).copied().flatten()) {
                let (fair, d) = self.lot_worth(pos, id, n);
                *lot = (d, fair);
            }
        }
        Some((s.owner.clone(), s.prices, lots))
    }

    /// The owner sets lot `slot`'s price (a joined player asks the host).
    pub fn set_stall_price(&mut self, slot: usize, price: u8) {
        let Some(pos) = self.open else { return };
        let price = price.min(MAX_PRICE);
        if self.is_client() {
            if let Some(s) = self.shops.seen.as_mut().filter(|s| s.0 == pos)
                && let Some(p) = s.1.1.get_mut(slot)
            {
                *p = price;
            }
            self.net_send_msg(Msg::StallPrice { x: pos.x, y: pos.y, z: pos.z, slot: slot as u8, price });
            return;
        }
        if let Some(s) = self.shops.stalls.get_mut(&pos)
            && slot < LOTS
        {
            s.prices[slot] = price;
        }
        self.send_stall(pos);
    }

    /// A joined player sets a price at a stall they have open (theirs).
    pub fn host_stall_price(&mut self, from: u32, pos: IVec3, slot: u8, price: u8) {
        let open = self.viewers.get(&pos).is_some_and(|v| v.contains(&from));
        let who = crate::players::record_key(&self.peer_name(from));
        if open
            && slot < LOTS as u8
            && self.world.get_v(pos) == SHOP_STALL
            && let Some(s) = self.shops.stalls.get_mut(&pos).filter(|s| s.owner == who)
        {
            s.prices[slot as usize] = price.min(MAX_PRICE);
        }
        self.send_stall(pos);
    }

    /// Tell everyone looking at a stall how it stands.
    pub fn send_stall(&mut self, pos: IVec3) {
        if !self.is_host() {
            return;
        }
        let Some(s) = self.shops.stalls.get(&pos).cloned() else { return };
        let c = self.world.containers.get(&pos).cloned();
        let mut lots = Vec::new();
        for i in 0..LOTS {
            let (fair, d) = match c.as_ref().and_then(|c| c.slots.get(i).copied().flatten()) {
                Some((id, n)) => self.lot_worth(pos, id, n),
                None => (0.0, 1.0),
            };
            lots.push(((d * 100.0).round().clamp(0.0, 1000.0) as u16, (fair * 10.0).round().clamp(0.0, 65535.0) as u16));
        }
        let msg = Msg::Stall { x: pos.x, y: pos.y, z: pos.z, owner: s.owner.clone(), prices: s.prices.to_vec(), lots };
        let viewers: Vec<u32> = self.viewers.get(&pos).map(|v| v.iter().copied().collect()).unwrap_or_default();
        for id in viewers {
            self.net_send_to(id, msg.clone());
        }
    }

    /// A joined player's stall, from the host.
    pub fn stall_from_host(&mut self, pos: IVec3, owner: String, prices: Vec<u8>, lots: Vec<(u16, u16)>) {
        let mut p = [0; LOTS];
        let mut l = [(1.0, 0.0); LOTS];
        for i in 0..LOTS {
            p[i] = prices.get(i).copied().unwrap_or(0).min(MAX_PRICE);
            if let Some(&(d, f)) = lots.get(i) {
                l[i] = (d as f32 / 100.0, f as f32 / 10.0);
            }
        }
        self.shops.seen = Some((pos, (owner, p, l)));
    }

    /// Where the world lives: customers come and go, villages forget what they've bought.
    pub fn shops_tick(&mut self, dt: f32) {
        if self.is_client() || self.shops.stalls.is_empty() && self.shops.glut.is_empty() {
            return;
        }
        // Wants come back slowly.
        for g in self.shops.glut.values_mut() {
            *g = (*g + GLUT_RECOVERY * dt / 60.0).min(1.0);
        }
        self.shops.glut.retain(|_, g| *g < 1.0);
        self.shops.look -= dt;
        let looked = self.shops.look <= 0.0;
        if looked {
            self.shops.look = 1.0;
        }
        let daytime = crate::village_life::part_of_day(self.time) != crate::village_life::Part::Bed;
        let spots: Vec<IVec3> = self.shops.stalls.keys().copied().collect();
        for pos in spots {
            if !self.world.is_loaded(pos.x, pos.z) {
                continue;
            }
            // A stall that's gone is forgotten.
            if self.world.get_v(pos) != SHOP_STALL {
                self.shops.stalls.remove(&pos);
                continue;
            }
            let Some(s) = self.shops.stalls.get_mut(&pos) else { continue };
            s.next -= dt;
            if let Some((_, left)) = s.customer.as_mut() {
                *left -= dt;
            }
            if looked && daytime {
                self.serve_customer(pos);
            }
        }
    }

    /// One look at a stall: send for a customer, or see to the one who's come.
    fn serve_customer(&mut self, pos: IVec3) {
        let at = pos.as_vec3() + Vec3::new(0.5, 0.0, 0.5);
        let Some(s) = self.shops.stalls.get(&pos).cloned() else { return };
        match s.customer {
            None if s.next <= 0.0 => {
                // Anything worth coming for? (A Hmmer from round about, who isn't busy.)
                let open = s.prices.iter().enumerate().any(|(i, &p)| p > 0 && self.world.containers.get(&pos).is_some_and(|c| c.slots[i].is_some()));
                let free: Vec<u32> = self
                    .mobs
                    .iter()
                    .filter(|m| m.kind == MobKind::Hmmer && m.health > 0.0 && m.baby <= 0.0 && m.flee <= 0.0 && !m.sitting && m.leash.is_none() && m.body.pos.distance(at) < CATCHMENT)
                    .map(|m| m.id)
                    .collect();
                let jitter = self.rng.range(0.0, CUSTOMER_JITTER);
                let st = self.shops.stalls.get_mut(&pos).unwrap();
                st.next = CUSTOMER_EVERY + jitter;
                if !open || free.is_empty() {
                    return;
                }
                let who = free[self.rng.int(0, free.len() as i32 - 1) as usize];
                self.shops.stalls.get_mut(&pos).unwrap().customer = Some((who, CUSTOMER_PATIENCE));
            }
            Some((who, left)) => {
                let Some(m) = self.mobs.iter_mut().find(|m| m.id == who && m.health > 0.0) else {
                    self.shops.stalls.get_mut(&pos).unwrap().customer = None;
                    return;
                };
                if left <= 0.0 {
                    self.shops.stalls.get_mut(&pos).unwrap().customer = None;
                    return;
                }
                let flat = Vec3::new(m.body.pos.x - at.x, 0.0, m.body.pos.z - at.z).length();
                if flat > 2.5 {
                    m.goal = Some(at + Vec3::new(0.0, 0.0, 1.5));
                    return;
                }
                m.goal = None;
                self.shops.stalls.get_mut(&pos).unwrap().customer = None;
                self.customer_buys(pos, who);
            }
            _ => {}
        }
    }

    /// A Hmmer at the stall looks over what's out and buys a lot, if it likes one.
    pub fn customer_buys(&mut self, pos: IVec3, who: u32) -> bool {
        let Some(s) = self.shops.stalls.get(&pos).cloned() else { return false };
        let Some(c) = self.world.containers.get(&pos).cloned() else { return false };
        // The till must have room for the gold.
        let till = c.slots.get(TILL).copied().flatten();
        let room = match till {
            None => MAX_PRICE,
            Some((GOLD_INGOT, n)) => max_stack(GOLD_INGOT).saturating_sub(n),
            Some(_) => 0,
        };
        // The best bargain it can see.
        let mut best: Option<(usize, f32)> = None;
        for i in 0..LOTS {
            let (Some((id, n)), price) = (c.slots[i], s.prices[i]) else { continue };
            if price == 0 || price > room {
                continue;
            }
            let (fair, _) = self.lot_worth(pos, id, n);
            let chance = buy_chance(price, fair);
            if chance > 0.0 && best.is_none_or(|(_, b)| chance > b) {
                best = Some((i, chance));
            }
        }
        let at = pos.as_vec3() + Vec3::new(0.5, 1.2, 0.5);
        let Some((i, chance)) = best else {
            // Nothing it wants at those prices.
            self.sfx(crate::sound::Sfx::Hmm, Some(at));
            return false;
        };
        if !self.rng.chance(chance) {
            self.sfx(crate::sound::Sfx::Hmm, Some(at));
            return false;
        }
        let (id, n) = c.slots[i].unwrap();
        let price = s.prices[i];
        if let Some(c) = self.world.containers.get_mut(&pos) {
            c.slots[i] = None;
            c.wear[i] = 0;
            let had = c.slots[TILL].map_or(0, |t| t.1);
            c.slots[TILL] = Some((GOLD_INGOT, had + price));
        }
        self.dirty_containers.insert(pos);
        // The village wants a little less of it for a while.
        let village = self.village_key(pos.as_vec3());
        let g = self.shops.glut.entry((village, id)).or_insert(1.0);
        *g = (*g - GLUT_PER_SALE).max(0.25);
        let owner = {
            let st = self.shops.stalls.get_mut(&pos).unwrap();
            st.sold += price as u32;
            st.owner.clone()
        };
        let total = self.shops.stalls[&pos].sold;
        let seller = self.display_name(&owner);
        self.report(crate::paper::Event::Sale, &seller, "", price as u32);
        self.sfx(crate::sound::Sfx::Hmm, Some(at));
        self.hearts(at + Vec3::Y * 0.5, 2);
        let name = item_name(id);
        let name = name.split(" (").next().unwrap_or(name);
        self.tell(&owner, &format!("A Hmmer bought {n} {name} from your stall for {price} gold."));
        self.advance_for(&owner, "open_for_business");
        if total >= NEST_EGG {
            self.advance_for(&owner, "nest_egg");
        }
        self.send_stall(pos);
        if let Some(m) = self.mobs.iter_mut().find(|m| m.id == who) {
            m.goal = None;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fair_prices_sell_and_greedy_ones_dont() {
        assert_eq!(buy_chance(0, 10.0), 0.0, "not for sale");
        assert!(buy_chance(5, 10.0) > buy_chance(10, 10.0));
        assert!(buy_chance(12, 10.0) > 0.0);
        assert_eq!(buy_chance(13, 10.0), 0.0, "too dear");
        assert_eq!(value(DIRT), 0.0, "nobody buys dirt");
        assert_eq!(demand_word(1.6), "Wanted!");
        assert_eq!(demand_word(0.25), "Glutted");
    }

    #[test]
    fn shops_are_saved() {
        let mut s = Shops::default();
        s.stalls.insert(IVec3::new(4, 70, -2), Stall { owner: "ann".into(), prices: [3, 0, 64, 1, 0, 0, 0, 0, 9], sold: 12, ..Default::default() });
        s.glut.insert((IVec3::new(0, 71, 0), WHEAT), 0.5);
        let back = Shops::decode(&s.encode());
        assert_eq!(back.stalls[&IVec3::new(4, 70, -2)].prices, [3, 0, 64, 1, 0, 0, 0, 0, 9]);
        assert_eq!(back.stalls[&IVec3::new(4, 70, -2)].owner, "ann");
        assert_eq!(back.stalls[&IVec3::new(4, 70, -2)].sold, 12);
        assert_eq!(back.glut[&(IVec3::new(0, 71, 0), WHEAT)], 0.5);
    }

    #[test]
    fn hmmers_buy_bargains_pay_into_the_till_and_tire_of_a_thing() {
        let (mut g, key) = crate::village_growth::tests::village_with_square(31);
        g.time = 0.1;
        let stall = key + IVec3::new(5, 0, 5);
        g.world.set_v(stall, SHOP_STALL);
        let me = crate::players::record_key(&g.player_name);
        g.player.body.pos = stall.as_vec3() + Vec3::new(2.0, 0.0, 0.5);
        g.open_stall(stall);
        assert_eq!(g.shops.stalls[&stall].owner, me, "the first to open it owns it");
        // Bread, cheap; iron, far too dear.
        {
            let c = g.world.containers.get_mut(&stall).unwrap();
            c.slots[0] = Some((BREAD, 10));
            c.slots[1] = Some((IRON, 2));
        }
        g.open = Some(stall);
        g.set_stall_price(0, 2);
        g.set_stall_price(1, 50);
        let (fair, d) = g.lot_worth(stall, BREAD, 10);
        assert!(fair > 2.0 && d > 0.0, "{fair}");
        let hmmer = g.alloc_mob(MobKind::Hmmer, stall.as_vec3() + Vec3::new(0.5, 0.0, 1.5));
        let mut sold = false;
        for _ in 0..20 {
            if g.customer_buys(stall, hmmer) {
                sold = true;
                break;
            }
        }
        assert!(sold, "a bargain sells");
        let c = &g.world.containers[&stall];
        assert_eq!(c.slots[0], None);
        assert_eq!(c.slots[TILL], Some((GOLD_INGOT, 2)));
        assert_eq!(c.slots[1], Some((IRON, 2)), "too dear");
        assert!(g.advancements.earned.iter().any(|a| a == "open_for_business"));
        // The village has had its fill of bread for now.
        assert!(g.demand_for(key, BREAD) < d);
        // Someone else can't open it.
        g.close_container();
        g.player_name = "Bob".into();
        g.open_stall(stall);
        assert!(g.open.is_none(), "it's Ann's stall");
    }

    #[test]
    fn hmmers_walk_over_in_the_daytime_to_shop() {
        let (mut g, key) = crate::village_growth::tests::village_with_square(31);
        g.time = 0.1;
        let stall = key + IVec3::new(5, 0, 5);
        g.world.set_v(stall, SHOP_STALL);
        g.open_stall(stall);
        g.close_container();
        {
            let c = g.world.containers.get_mut(&stall).unwrap();
            c.slots[0] = Some((BREAD, 10));
        }
        g.shops.stalls.get_mut(&stall).unwrap().prices[0] = 1;
        let hmmer = g.alloc_mob(MobKind::Hmmer, stall.as_vec3() + Vec3::new(6.5, 0.0, 0.5));
        if let Some(m) = g.mobs.iter_mut().find(|m| m.id == hmmer) {
            m.persistent = true;
        }
        g.shops_tick(1.0);
        assert_eq!(g.shops.stalls[&stall].customer.map(|c| c.0), Some(hmmer), "it sends for a customer");
        g.shops_tick(1.0);
        assert!(g.mobs.iter().find(|m| m.id == hmmer).unwrap().goal.is_some(), "the customer heads over");
    }
}
