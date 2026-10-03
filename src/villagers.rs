//! Hmmers: legally distinct villagers who live in the huts and trade.
//!
//! The first time a hut's chunk loads where the world lives, a Hmmer moves
//! in. Each has a job, from its hut: Farmers, Librarians, Smiths and
//! Fishers, each with their own list of trades. Gold ingots are the money
//! (finally, a use for them). Every trade can be made a few times before the
//! Hmmer runs out; they restock every in-game day. Joined players ask the
//! host (`Msg::Trade`), which checks they really have what they're giving.
//!
//! Hmmers remember their **regulars**: after 5 trades with one you get 10%
//! off, after 15 a fifth off, after 30 nearly a third (on top of a Hero of
//! the Village's discount). Where the world lives keeps count for everyone
//! and tells joined players theirs (`Msg::Regular`).

use crate::block::*;
use crate::enchant::roll;
use crate::entity::{Mob, MobKind};
use crate::game::Game;
use crate::inventory::Wear;
use crate::net::Msg;
use crate::noise::Rng;
use crate::sound::Sfx;

/// Times each trade can be made before a restock, and seconds between restocks (a day).
pub const STOCK: u8 = 6;
pub const RESTOCK_SECS: f32 = crate::game::DAY_SECONDS;
/// Hmmers wander this far from home at most.
const HOME_RANGE: f32 = 10.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Job {
    Farmer,
    Librarian,
    Smith,
    Fisher,
}

impl Job {
    pub fn of(seed: u32) -> Job {
        [Job::Farmer, Job::Librarian, Job::Smith, Job::Fisher][(seed % 4) as usize]
    }
    pub fn name(self) -> &'static str {
        match self {
            Job::Farmer => "Farmer (Knows a Lot About Wheat)",
            Job::Librarian => "Librarian (Shh)",
            Job::Smith => "Smith (Hits Things Professionally)",
            Job::Fisher => "Fisher (Smells Faintly of Cod)",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Trade {
    /// What you give (the second can be AIR).
    pub give: [(Id, u8); 2],
    /// What you get, and its wear (enchantments, for books and gear).
    pub get: (Id, u8),
    pub wear: Wear,
}

/// A Hmmer's trades, always the same for the same seed.
pub fn trades(seed: u32) -> Vec<Trade> {
    let mut rng = Rng::new(seed as u64 ^ 0x7ADE);
    let t = |give: (Id, u8), give2: (Id, u8), get: (Id, u8)| Trade { give: [give, give2], get, wear: 0 };
    let none = (AIR, 0);
    let mut v = match Job::of(seed) {
        Job::Farmer => vec![
            t((WHEAT, 20), none, (GOLD_INGOT, 1)),
            t((CARROT, 15), none, (GOLD_INGOT, 1)),
            t((GOLD_INGOT, 1), none, (BREAD, 6)),
            t((GOLD_INGOT, 1), none, (COOKED_CHOP, 4)),
            t((GOLD_INGOT, 2), none, (CAKE, 1)),
        ],
        Job::Librarian => vec![
            t((FEATHER, 24), none, (GOLD_INGOT, 1)),
            t((GOLD_INGOT, 1), none, (BOOK, 2)),
            t((GOLD_INGOT, 3), none, (BOOKSHELF, 1)),
            t((GOLD_INGOT, 5), (BOOK, 1), (ENCHANTED_BOOK, 1)),
            t((GOLD_INGOT, 2), none, (LANTERN, 2)),
        ],
        Job::Smith => vec![
            t((COAL, 12), none, (GOLD_INGOT, 1)),
            t((IRON, 4), none, (GOLD_INGOT, 1)),
            t((GOLD_INGOT, 4), none, (PICK_IRON, 1)),
            t((GOLD_INGOT, 7), none, (SWORD_IRON, 1)),
            t((GOLD_INGOT, 10), none, (ARMOR_FIRST + 4 + CHESTPLATE as Id, 1)),
        ],
        Job::Fisher => vec![
            t((COD, 6), none, (GOLD_INGOT, 1)),
            t((STRING, 12), none, (GOLD_INGOT, 1)),
            t((GOLD_INGOT, 2), none, (ROD, 1)),
            t((GOLD_INGOT, 1), none, (COOKED_COD, 4)),
            t((GOLD_INGOT, 1), none, (BAIT, 8)),
        ],
    };
    // Books, swords and picks from a trader come enchanted.
    for tr in v.iter_mut() {
        if matches!(tr.get.0, ENCHANTED_BOOK | SWORD_IRON | PICK_IRON) {
            let item = if tr.get.0 == ENCHANTED_BOOK { BOOK } else { tr.get.0 };
            tr.wear = roll(item, rng.int(8, 20) as u8, &mut rng);
        }
    }
    v
}

/// Who a mob is as a trader, and its trades: a Hmmer's job and list, or a
/// mod-defined trader's name and the `trade` lines from its mod. None: it
/// doesn't trade (or, being a monster on the loose, won't).
pub fn trades_of(m: &Mob) -> Option<(String, Vec<Trade>)> {
    if m.kind == MobKind::Hmmer {
        return Some((format!("Hmmer: {}", Job::of(m.seed).name()), trades(m.seed)));
    }
    let d = m.kind.mod_def()?;
    (!d.trades.is_empty() && !m.menacing()).then(|| (d.name.clone(), d.trades.clone()))
}

/// How much a regular gets off, after `n` trades with the same Hmmer.
pub fn regular_discount(n: u16) -> f32 {
    match n {
        30.. => 0.3,
        15.. => 0.2,
        5.. => 0.1,
        _ => 0.0,
    }
}

/// A trade's price for someone who's traded `n` times here before.
pub fn regular_price(mut t: Trade, n: u16) -> Trade {
    let d = regular_discount(n);
    if d > 0.0 {
        for g in t.give.iter_mut().filter(|g| g.0 != AIR && g.1 > 1) {
            g.1 = ((g.1 as f32 * (1.0 - d)).round() as u8).max(1);
        }
    }
    t
}

/// Save format for the regulars: (mob, who) -> trades.
pub fn encode_regulars(r: &std::collections::HashMap<(u32, String), u16>) -> Vec<u8> {
    let mut keys: Vec<_> = r.iter().collect();
    keys.sort();
    let mut out = (keys.len() as u32).to_le_bytes().to_vec();
    for ((mob, who), n) in keys {
        out.extend_from_slice(&mob.to_le_bytes());
        out.extend_from_slice(&n.to_le_bytes());
        let name = &who.as_bytes()[..who.len().min(255)];
        out.push(name.len() as u8);
        out.extend_from_slice(name);
    }
    out
}

pub fn decode_regulars(b: &[u8]) -> std::collections::HashMap<(u32, String), u16> {
    let mut map = std::collections::HashMap::new();
    let Some(n) = b.get(..4).map(|x| u32::from_le_bytes(x.try_into().unwrap())) else { return map };
    let mut at = 4;
    for _ in 0..n {
        let Some(head) = b.get(at..at + 7) else { break };
        let mob = u32::from_le_bytes(head[..4].try_into().unwrap());
        let trades = u16::from_le_bytes(head[4..6].try_into().unwrap());
        let len = head[6] as usize;
        let Some(name) = b.get(at + 7..at + 7 + len) else { break };
        map.insert((mob, String::from_utf8_lossy(name).into_owned()), trades);
        at += 7 + len;
    }
    map
}

/// Does this inventory (item counts) have what a trade asks for?
pub fn can_afford(counts: impl Fn(Id) -> u32, t: &Trade) -> bool {
    t.give.iter().all(|&(id, n)| id == AIR || counts(id) >= n as u32)
}

impl Game {
    /// Where the world lives: a new hut gets its Hmmer.
    pub fn house_hmmers(&mut self) {
        for (at, seed) in std::mem::take(&mut self.world.new_huts) {
            let mut m = Mob::new(MobKind::Hmmer, at, &mut self.rng);
            m.id = self.next_mob_id;
            self.next_mob_id += 1;
            m.seed = seed & 0xFF_FFFF;
            m.home = Some(at);
            m.persistent = true;
            self.mobs.push(m);
        }
    }

    /// Hmmers stay near home and restock over time.
    pub fn hmmers_tick(&mut self, dt: f32) {
        for m in self.mobs.iter_mut().filter(|m| m.kind == MobKind::Hmmer || m.kind.mod_def().is_some_and(|d| !d.trades.is_empty())) {
            m.restock -= dt;
            if m.restock <= 0.0 {
                m.restock = RESTOCK_SECS;
                m.trades_used = [0; 8];
            }
            if let Some(home) = m.home
                && m.goal.is_none()
                && m.body.pos.distance(home) > HOME_RANGE
            {
                m.goal = Some(home);
            }
        }
    }

    /// How many times the local player has traded with this Hmmer.
    pub fn my_trades_with(&self, mob: u32) -> u16 {
        self.regulars.get(&(mob, crate::players::record_key(&self.player_name))).copied().unwrap_or(0)
    }

    /// A trade went through: count it (where the world lives, or our copy).
    fn note_regular(&mut self, mob: u32, who: String) -> u16 {
        let n = self.regulars.entry((mob, who)).or_insert(0);
        *n = n.saturating_add(1);
        *n
    }

    /// The local player's regular advancements, after a trade with `mob`.
    fn regular_advancements(&mut self, mob: u32) {
        if self.my_trades_with(mob) >= 15 {
            self.advance("the_usual");
        }
        let me = crate::players::record_key(&self.player_name);
        let mut jobs: Vec<u32> = self
            .regulars
            .keys()
            .filter(|(_, who)| *who == me)
            .filter_map(|(id, _)| self.mobs.iter().find(|m| m.id == *id && m.kind == MobKind::Hmmer).map(|m| m.seed % 4))
            .collect();
        jobs.sort_unstable();
        jobs.dedup();
        if jobs.len() >= 4 {
            self.advance("trading_hall");
        }
    }

    /// A joined player is told how many times they've traded with `mob`.
    pub fn regular_state(&mut self, mob: u32, trades: u16) {
        let me = crate::players::record_key(&self.player_name);
        self.regulars.insert((mob, me), trades);
    }

    /// Right-click on a Hmmer: open its trades.
    pub fn open_trade(&mut self, mob_id: u32) {
        self.trading = Some(mob_id);
        if self.is_client() {
            // Ask how well it knows us (the host keeps count).
            self.net_send_msg(Msg::RegularAsk { mob: mob_id });
        }
        let at = self.mobs.iter().find(|m| m.id == mob_id).map(|m| m.body.pos);
        self.sfx(Sfx::Hmm, at);
    }

    /// The trades of the Hmmer (or modded trader) we're talking to, under its
    /// title (and whether it's still there).
    pub fn trade_list(&self) -> Option<(String, Vec<Trade>)> {
        let id = self.trading?;
        let m = self.mobs.iter().find(|m| m.id == id)?;
        if m.body.pos.distance(self.player.body.pos) > 8.0 {
            return None;
        }
        let (title, list) = trades_of(m)?;
        let hero = self.has_effect(crate::potions::Potion::Hero);
        let regular = self.my_trades_with(id);
        let off = regular_discount(regular);
        let title = if off > 0.0 { format!("{title}  (a regular: {:.0}% off)", off * 100.0) } else { title };
        Some((title, list.into_iter().map(|t| regular_price(if hero { crate::raids::hero_price(t) } else { t }, regular)).collect()))
    }

    /// The local player makes trade `index` with the Hmmer they're talking to.
    pub fn make_trade(&mut self, index: usize) {
        let Some(id) = self.trading else { return };
        let Some((_, list)) = self.trade_list() else { return };
        let Some(&t) = list.get(index) else { return };
        if !self.creative && !can_afford(|i| self.inv.count(i), &t) {
            self.msg("Hmm. (You can't afford that.)");
            return;
        }
        if self.is_client() {
            // The host checks the stock and what we have, then sends the goods.
            for (i, n) in t.give {
                if i != AIR && !self.creative {
                    self.inv.remove(i, n as u32);
                }
            }
            self.net_send_msg(Msg::Trade { mob: id, index: index as u8 });
            self.sfx(Sfx::Hmm, None);
            self.advance("what_a_deal");
            return;
        }
        let Some(m) = self.mobs.iter_mut().find(|m| m.id == id) else { return };
        if m.trades_used[index.min(7)] >= STOCK {
            self.msg("Hmm. (Out of stock until tomorrow.)");
            return;
        }
        m.trades_used[index.min(7)] += 1;
        let at = m.body.pos;
        if !self.creative {
            for (i, n) in t.give {
                if i != AIR {
                    self.inv.remove(i, n as u32);
                }
            }
        }
        self.give_worn(t.get.0, t.get.1, t.wear);
        self.sfx(Sfx::Hmm, Some(at));
        let points = self.rng.int(1, 3) as u32;
        self.add_xp(points);
        self.advance("what_a_deal");
        let me = crate::players::record_key(&self.player_name);
        self.note_regular(id, me);
        self.regular_advancements(id);
    }

    /// A joined player's trade: check the Hmmer's stock and their ledger.
    pub fn host_trade(&mut self, from: u32, mob: u32, index: u8) {
        let Some(me) = self.peers.get(&from).map(|p| p.target) else { return };
        let Some(m) = self.mobs.iter_mut().find(|m| m.id == mob && m.body.pos.distance(me) < 8.0) else { return };
        let Some(&t) = trades_of(m).and_then(|(_, list)| list.get(index as usize).copied()).as_ref() else { return };
        let t = if self.heroes.contains_key(&from) { crate::raids::hero_price(t) } else { t };
        let who = self.peers.get(&from).map(|p| crate::players::record_key(&p.name)).unwrap_or_default();
        let t = regular_price(t, self.regulars.get(&(mob, who.clone())).copied().unwrap_or(0));
        let Some(m) = self.mobs.iter_mut().find(|m| m.id == mob) else { return };
        if m.trades_used[(index as usize).min(7)] >= STOCK {
            self.system_message(Some(from), "Hmm. (Out of stock until tomorrow.)");
            return;
        }
        let affordable = self.peer_free(from) || self.peers.get(&from).is_some_and(|p| can_afford(|i| p.ledger.bag.count(i), &t));
        if !affordable {
            return;
        }
        if let Some(m) = self.mobs.iter_mut().find(|m| m.id == mob) {
            m.trades_used[(index as usize).min(7)] += 1;
        }
        for (i, n) in t.give {
            if i != AIR {
                self.peer_take(from, i, n as u32);
            }
        }
        self.give_peer_worn(from, t.get.0, t.get.1, t.wear);
        let points = self.rng.int(1, 3) as u32;
        self.give_peer_xp(from, points);
        let trades = self.note_regular(mob, who);
        self.net_send_to(from, Msg::Regular { mob, trades });
    }

    /// A joined player asks how many times they've traded with `mob`.
    pub fn host_regular_ask(&mut self, from: u32, mob: u32) {
        let Some(who) = self.peers.get(&from).map(|p| crate::players::record_key(&p.name)) else { return };
        let trades = self.regulars.get(&(mob, who)).copied().unwrap_or(0);
        self.net_send_to(from, Msg::Regular { mob, trades });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regulars_get_a_discount_that_is_saved() {
        let t = Trade { give: [(GOLD_INGOT, 10), (WHEAT, 1)], get: (BREAD, 1), wear: 0 };
        assert_eq!(regular_price(t, 0), t);
        assert_eq!(regular_price(t, 5).give, [(GOLD_INGOT, 9), (WHEAT, 1)]);
        assert_eq!(regular_price(t, 15).give[0].1, 8);
        assert_eq!(regular_price(t, 40).give[0].1, 7);
        let mut g = crate::game::tests::arena(64);
        g.alloc_mob(MobKind::Hmmer, macroquad::math::Vec3::new(1.5, 50.0, 1.5));
        let id = g.mobs[0].id;
        g.mobs[0].seed = 0; // a Farmer
        g.open_trade(id);
        let first = trades(0)[0];
        let price = first.give[0].1;
        for _ in 0..5 {
            for (i, n) in first.give {
                if i != AIR {
                    g.inv.add(i, n.saturating_mul(2));
                }
            }
            g.mobs[0].trades_used = [0; 8];
            g.make_trade(0);
        }
        assert_eq!(g.my_trades_with(id), 5);
        let (title, list) = g.trade_list().unwrap();
        assert!(title.contains("10% off"), "{title}");
        assert!(price <= 1 || list[0].give[0].1 < price);
        let back = decode_regulars(&encode_regulars(&g.regulars));
        assert_eq!(back, g.regulars);
    }

    #[test]
    fn jobs_and_trades() {
        for seed in 0..16u32 {
            let list = trades(seed);
            assert_eq!(list, trades(seed), "the same every time");
            assert!(list.len() >= 4 && list.len() <= 8);
            // Always something to sell them and something to buy with gold.
            assert!(list.iter().any(|t| t.get.0 == GOLD_INGOT));
            assert!(list.iter().any(|t| t.give[0].0 == GOLD_INGOT));
        }
        let librarian = (0..16).find(|&s| Job::of(s) == Job::Librarian).unwrap();
        let book = trades(librarian).into_iter().find(|t| t.get.0 == ENCHANTED_BOOK).unwrap();
        assert!(book.wear >> 16 != 0, "enchanted books come enchanted");
        let counts = |i: Id| if i == GOLD_INGOT { 5 } else if i == BOOK { 1 } else { 0 };
        assert!(can_afford(counts, &book));
        assert!(!can_afford(|i| if i == GOLD_INGOT { 5 } else { 0 }, &book));
    }
}
