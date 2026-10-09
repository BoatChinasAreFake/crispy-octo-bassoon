//! Remembering joined players between visits: their inventory (with wear and
//! enchantments), armour, experience, health, hunger and where they were.
//!
//! Players are recognised by name (there are no accounts; a server password is
//! the way to keep strangers out). A joined player's game reports its layout
//! every few seconds; the host keeps that, and what its ledger says they own,
//! and saves both with the world when they leave or the world is saved. When
//! they come back the host puts it all back, and the ledger's counts win if
//! the two ever disagree (the usual inventory check sorts that out).

use crate::block::*;
use crate::dims::Dim;
use crate::game::Game;
use crate::inventory::Wear;
use crate::net::Msg;
use macroquad::math::Vec3;
use std::collections::BTreeMap;

/// Seconds between a joined player's reports.
pub const REPORT_SECS: f32 = 5.0;
/// Inventory, then armour.
pub const SLOTS: usize = 41;

/// What a joined player last said about themselves.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub slots: Vec<(Id, u8, Wear)>,
    pub health: f32,
    pub food: f32,
    pub saturation: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerRecord {
    pub pos: Vec3,
    pub xp: u32,
    /// What the host's ledger said they owned.
    pub bag: Vec<(Id, u32)>,
    pub report: Report,
    /// Enchanted things the ledger knew they had: (item, enchantments, how many).
    pub enchanted: Vec<(Id, u16, u32)>,
    /// How many times they've enchanted something (seeds the table's offers).
    pub enchant_count: u32,
    /// Their statistics (`stats::Stats::encode`) and game mode (`modes::GameMode` index).
    pub stats: Vec<u8>,
    pub mode: u8,
    /// Which dimension `pos` is in.
    pub dim: Dim,
}

/// The most statistics a joined player may send (they're short text).
pub const MAX_STATS: usize = 2048;

/// Players are remembered by name, whatever its capitals.
pub fn record_key(name: &str) -> String {
    name.to_ascii_lowercase()
}

/// A layout for `bag` when we never heard one: stacks in order, nothing worn.
fn layout_from(bag: &[(Id, u32)]) -> Vec<(Id, u8, Wear)> {
    let mut slots = Vec::new();
    for &(id, mut n) in bag {
        while n > 0 && slots.len() < 36 {
            let k = n.min(max_stack(id) as u32);
            slots.push((id, k as u8, 0));
            n -= k;
        }
    }
    slots.resize(SLOTS, (AIR, 0, 0));
    slots
}

/// Keep only real items and sane numbers (reports come from joined players).
fn clean(slots: Vec<(Id, u8, Wear)>) -> Vec<(Id, u8, Wear)> {
    let mut v: Vec<(Id, u8, Wear)> = slots
        .into_iter()
        .take(SLOTS)
        .map(|(id, n, w)| {
            if n == 0 || !valid_item(id) {
                return (AIR, 0, 0);
            }
            let w = crate::inventory::sanitize_wear(id, w);
            (id, n.min(max_stack(id)), w)
        })
        .collect();
    v.resize(SLOTS, (AIR, 0, 0));
    v
}

fn finite(v: f32, lo: f32, hi: f32, default: f32) -> f32 {
    if v.is_finite() { v.clamp(lo, hi) } else { default }
}

// ------------------------------------------------------------------ saving

pub fn encode(records: &BTreeMap<String, PlayerRecord>) -> Vec<u8> {
    let mut out = Vec::new();
    let mut put = |b: &[u8]| out.extend_from_slice(b);
    put(&(records.len() as u32).to_le_bytes());
    for (name, r) in records {
        put(&(name.len() as u8).to_le_bytes());
        put(name.as_bytes());
        for v in [r.pos.x, r.pos.y, r.pos.z, r.report.health, r.report.food, r.report.saturation] {
            put(&v.to_le_bytes());
        }
        put(&r.xp.to_le_bytes());
        put(&(r.bag.len() as u32).to_le_bytes());
        for &(id, n) in &r.bag {
            put(&id.to_le_bytes());
            put(&n.to_le_bytes());
        }
        for &(id, n, w) in &r.report.slots {
            put(&id.to_le_bytes());
            put(&[n]);
            put(&w.to_le_bytes());
        }
        put(&r.enchant_count.to_le_bytes());
        put(&(r.enchanted.len() as u32).to_le_bytes());
        for &(id, e, n) in &r.enchanted {
            put(&id.to_le_bytes());
            put(&e.to_le_bytes());
            put(&n.to_le_bytes());
        }
    }
    // A trailer (older saves simply end above): each player's statistics and mode.
    put(b"STAT");
    put(&(records.len() as u32).to_le_bytes());
    for (name, r) in records {
        put(&(name.len() as u8).to_le_bytes());
        put(name.as_bytes());
        put(&[r.mode]);
        let stats = &r.stats[..r.stats.len().min(MAX_STATS)];
        put(&(stats.len() as u32).to_le_bytes());
        put(stats);
    }
    // Another (older saves end above, from when the dimensions shared one map):
    // which dimension each player is in.
    put(b"DIMS");
    put(&(records.len() as u32).to_le_bytes());
    for (name, r) in records {
        put(&(name.len() as u8).to_le_bytes());
        put(name.as_bytes());
        put(&[r.dim.index()]);
    }
    out
}

/// Unpack `encode`'s output; stops at anything malformed.
pub fn decode(b: &[u8]) -> BTreeMap<String, PlayerRecord> {
    let mut map = BTreeMap::new();
    let mut i = 0usize;
    let mut take = |n: usize| -> Option<&[u8]> {
        let s = b.get(i..i + n)?;
        i += n;
        Some(s)
    };
    let u32_of = |s: &[u8]| u32::from_le_bytes([s[0], s[1], s[2], s[3]]);
    let f32_of = |s: &[u8]| f32::from_le_bytes([s[0], s[1], s[2], s[3]]);
    let Some(count) = take(4).map(u32_of) else { return map };
    for _ in 0..count.min(10_000) {
        let Some(len) = take(1).map(|s| s[0] as usize) else { break };
        let Some(name) = take(len).map(|s| String::from_utf8_lossy(s).into_owned()) else { break };
        let Some(f) = take(24).map(|s| (0..6).map(|k| f32_of(&s[k * 4..])).collect::<Vec<f32>>()) else { break };
        let Some(xp) = take(4).map(u32_of) else { break };
        let Some(nbag) = take(4).map(u32_of) else { break };
        let mut bag = Vec::new();
        for _ in 0..nbag.min(4096) {
            let Some(s) = take(6) else { break };
            bag.push((u16::from_le_bytes([s[0], s[1]]), u32_of(&s[2..])));
        }
        let mut slots = Vec::new();
        for _ in 0..SLOTS {
            let Some(s) = take(7) else { break };
            slots.push((u16::from_le_bytes([s[0], s[1]]), s[2], u32_of(&s[3..])));
        }
        if slots.len() < SLOTS || !f.iter().all(|v| v.is_finite()) {
            break;
        }
        let Some(enchant_count) = take(4).map(u32_of) else { break };
        let Some(nench) = take(4).map(u32_of) else { break };
        let mut enchanted = Vec::new();
        for _ in 0..nench.min(4096) {
            let Some(s) = take(8) else { break };
            enchanted.push((u16::from_le_bytes([s[0], s[1]]), u16::from_le_bytes([s[2], s[3]]), u32_of(&s[4..])));
        }
        let report = Report { slots, health: f[3], food: f[4], saturation: f[5] };
        map.insert(name, PlayerRecord { pos: Vec3::new(f[0], f[1], f[2]), xp: xp.min(1 << 24), bag, report, enchanted, enchant_count, stats: Vec::new(), mode: 0, dim: Dim::Over });
    }
    if take(4) == Some(b"STAT")
        && let Some(n) = take(4).map(u32_of)
    {
        for _ in 0..n.min(10_000) {
            let Some(len) = take(1).map(|s| s[0] as usize) else { break };
            let Some(name) = take(len).map(|s| String::from_utf8_lossy(s).into_owned()) else { break };
            let Some(mode) = take(1).map(|s| s[0]) else { break };
            let Some(slen) = take(4).map(u32_of) else { break };
            let Some(stats) = take((slen as usize).min(MAX_STATS)).map(|s| s.to_vec()) else { break };
            if let Some(r) = map.get_mut(&name) {
                r.stats = stats;
                r.mode = mode.min(2);
            }
        }
    }
    let mut split = false;
    if take(4) == Some(b"DIMS")
        && let Some(n) = take(4).map(u32_of)
    {
        split = true;
        for _ in 0..n.min(10_000) {
            let Some(len) = take(1).map(|s| s[0] as usize) else { break };
            let Some(name) = take(len).map(|s| String::from_utf8_lossy(s).into_owned()) else { break };
            let Some(d) = take(1).map(|s| s[0]) else { break };
            if let Some(r) = map.get_mut(&name) {
                r.dim = Dim::from_index(d).unwrap_or_default();
            }
        }
    }
    if !split {
        // From when the dimensions shared one map: work out where they are.
        for r in map.values_mut() {
            r.dim = Dim::of_old_x(r.pos.x as i32);
            r.pos.x -= r.dim.gen_x() as f32;
        }
    }
    map
}

impl Game {
    /// For scripts: a player's health (0–20), as the host last heard it, or -1 if unknown.
    /// Joined players report every few seconds, so theirs can lag slightly behind.
    pub fn script_health(&self, name: &str) -> f32 {
        if self.is_local_player(name) {
            return self.player.health;
        }
        self.peer_by_name(name).and_then(|id| self.peer_ref(id)).map(|p| p.report.as_ref().map(|r| r.health).unwrap_or(20.0)).unwrap_or(-1.0)
    }

    /// For scripts: a player's food level (0–20), or -1 if unknown.
    pub fn script_food(&self, name: &str) -> f32 {
        if self.is_local_player(name) {
            return self.player.hunger.food;
        }
        self.peer_by_name(name).and_then(|id| self.peer_ref(id)).map(|p| p.report.as_ref().map(|r| r.food).unwrap_or(20.0)).unwrap_or(-1.0)
    }

    /// For scripts: what a player holds. For joined players, only what the
    /// host knows they really have (bare hands otherwise). None: no such player.
    pub fn script_held(&self, name: &str) -> Option<Id> {
        if self.is_local_player(name) {
            return Some(self.inv.held());
        }
        self.peer_by_name(name).map(|id| self.verified_held(id))
    }

    /// For scripts: how many of an item a player has. For joined players this
    /// is the host's own tally (the ledger), so it can't be faked.
    pub fn script_count(&self, name: &str, item: Id) -> u32 {
        if self.is_local_player(name) {
            return self.inv.count(item);
        }
        self.peer_by_name(name).and_then(|id| self.peer_ref(id)).map(|p| p.ledger.bag.count(item)).unwrap_or(0)
    }

    /// For scripts: everything a player has, as (item, count), sorted by item.
    pub fn script_items(&self, name: &str) -> Vec<(Id, u32)> {
        let mut v: Vec<(Id, u32)> = if self.is_local_player(name) {
            self.inv.counts().into_iter().collect()
        } else {
            self.peer_by_name(name).and_then(|id| self.peer_ref(id)).map(|p| p.ledger.bag.items()).unwrap_or_default()
        };
        v.retain(|&(id, n)| id != AIR && n > 0);
        v.sort_unstable();
        v
    }

    /// Joined players: every few seconds, tell the host what we look like.
    pub fn report_tick(&mut self, dt: f32) {
        if !self.is_client() {
            return;
        }
        self.report_timer += dt;
        if self.report_timer < REPORT_SECS {
            return;
        }
        self.report_timer = 0.0;
        let m = self.report_msg();
        self.net_send_msg(m);
        // Our statistics, for the host to keep with the world.
        let data = self.stats.encode();
        self.net_send_msg(Msg::Stats { data });
    }

    pub fn report_msg(&self) -> Msg {
        let inv = &self.inv;
        let slots = inv.slots.iter().zip(inv.wear.iter()).chain(inv.armor.iter().zip(inv.armor_wear.iter())).chain(std::iter::once((&inv.offhand, &inv.offhand_wear))).map(|(s, w)| s.map(|(id, n)| (id, n, *w)).unwrap_or((AIR, 0, 0))).collect();
        let h = &self.player.hunger;
        Msg::PlayerData { slots, health: self.player.health, food: h.food, saturation: h.saturation }
    }

    /// The host: a joined player's report.
    pub fn host_report(&mut self, from: u32, slots: Vec<(Id, u8, Wear)>, health: f32, food: f32, saturation: f32) {
        if let Some(p) = self.peer_mut(from) {
            let report = Report { slots: clean(slots), health: finite(health, 0.0, 20.0, 20.0), food: finite(food, 0.0, 20.0, 20.0), saturation: finite(saturation, 0.0, 20.0, 5.0) };
            p.report = Some(report);
        }
    }

    /// The host: what to remember about a joined player.
    pub fn record_of(&self, id: u32) -> Option<(String, PlayerRecord)> {
        let p = self.peer_ref(id)?;
        let bag = p.ledger.bag.items();
        let mut enchanted: Vec<(Id, u16, u32)> = p.ledger.enchanted.iter().map(|(&(id, e), &n)| (id, e, n)).collect();
        enchanted.sort_unstable();
        let report = p.report.clone().unwrap_or_else(|| Report { slots: layout_from(&bag), health: 20.0, food: 20.0, saturation: 5.0 });
        Some((record_key(&p.name), PlayerRecord { pos: p.target, xp: p.ledger.xp, bag, report, enchanted, enchant_count: p.ledger.enchant_count, stats: p.stats.clone(), mode: p.mode.index(), dim: p.dim }))
    }

    /// The host: a joined player is leaving; remember them.
    pub fn remember_peer(&mut self, id: u32) {
        if let Some((key, record)) = self.record_of(id) {
            self.saved_players.insert(key, record);
        }
    }

    /// Everyone remembered, including whoever is here right now (for saving the world).
    pub fn all_player_records(&self) -> BTreeMap<String, PlayerRecord> {
        let mut all = self.saved_players.clone();
        all.extend(self.all_peers().filter_map(|(&id, _)| self.record_of(id)));
        all
    }

    /// The host: someone joined; if we know them, put everything back.
    pub fn welcome_back(&mut self, id: u32, name: &str) {
        let Some(r) = self.saved_players.remove(&record_key(name)) else { return };
        if let Some(p) = self.peer_mut(id) {
            p.ledger.bag = crate::ledger::Bag::from_items(&r.bag);
            p.ledger.xp = r.xp;
            p.ledger.enchanted = r.enchanted.iter().filter(|e| valid_item(e.0)).map(|&(id, e, n)| ((id, e), n)).collect();
            p.ledger.enchant_count = r.enchant_count;
            p.target = r.pos;
            p.pos = r.pos;
            p.stats = r.stats.clone();
            p.mode = crate::modes::GameMode::from_index(r.mode);
        }
        // Back to the dimension they left from (everyone's told).
        if r.dim != crate::dims::Dim::Over {
            self.move_peer(id, r.dim, r.pos);
        }
        self.net_send_to(id, Msg::Stats { data: r.stats.clone() });
        if r.mode != 0 {
            self.net_send_to(id, Msg::GameMode { mode: r.mode });
        }
        let rep = r.report;
        self.net_send_to(id, Msg::Restore { pos: r.pos, xp: r.xp, slots: rep.slots, health: rep.health, food: rep.food, saturation: rep.saturation });
        self.net_send_to(id, Msg::Enchanted { item: AIR, ench: 0, count: r.enchant_count });
        self.msg(format!("{name} is back where they left off."));
    }

    /// Joined players: the host remembered us.
    pub fn apply_restore(&mut self, pos: Vec3, xp: u32, slots: Vec<(Id, u8, Wear)>, health: f32, food: f32, saturation: f32) {
        let slots = clean(slots);
        self.inv.slots = [None; 36];
        self.inv.wear = [0; 36];
        self.inv.armor = [None; 4];
        self.inv.armor_wear = [0; 4];
        self.inv.offhand = None;
        self.inv.offhand_wear = 0;
        for (i, (id, n, w)) in slots.into_iter().enumerate() {
            if id == AIR {
                continue;
            }
            if i < 36 {
                self.inv.slots[i] = Some((id, n));
                self.inv.wear[i] = w;
            } else if i == 40 {
                self.inv.offhand = Some((id, n));
                self.inv.offhand_wear = w;
            } else if armor_of(id).map(|(s, _)| s) == Some(i - 36) {
                self.inv.armor[i - 36] = Some((id, 1));
                self.inv.armor_wear[i - 36] = w;
            }
        }
        self.xp = xp.min(1 << 24);
        if pos.is_finite() {
            self.player.body.pos = pos;
            self.player.body.vel = Vec3::ZERO;
            self.player.fall_start = pos.y;
        }
        // Back from the dead? Then back at full health.
        self.player.health = if health > 0.0 { finite(health, 1.0, 20.0, 20.0) } else { crate::player::MAX_HEALTH };
        self.player.hunger = crate::hunger::Hunger::new(finite(food, 0.0, 20.0, 20.0), finite(saturation, 0.0, 20.0, 5.0));
        self.inv_sync.restart(self.inv.counts());
        self.msg("Welcome back! Your things were right where you left them.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_round_trip() {
        let mut slots = vec![(AIR, 0, 0); SLOTS];
        slots[0] = (PICK_IRON, 1, 0x0003_0012);
        slots[37] = (ARMOR_FIRST + 5, 1, 9);
        let r = PlayerRecord { pos: Vec3::new(1.5, 70.0, -3.0), xp: 99, bag: vec![(PICK_IRON, 1), (ARMOR_FIRST + 5, 1)], report: Report { slots, health: 13.0, food: 7.0, saturation: 1.5 }, enchanted: vec![(PICK_IRON, 3, 1)], enchant_count: 4, stats: b"mined=12\n".to_vec(), mode: 2, dim: Dim::Over };
        let mut map = BTreeMap::new();
        map.insert("stove".to_string(), r.clone());
        map.insert("joiny".to_string(), PlayerRecord { bag: vec![], ..r.clone() });
        map.insert("hotty".to_string(), PlayerRecord { dim: Dim::Scorch, pos: Vec3::new(-5.0, 40.0, 2.0), ..r.clone() });
        assert_eq!(decode(&encode(&map)), map);
        // A save from before statistics were kept still loads.
        let old = encode(&map);
        let cut = old.windows(4).position(|w| w == b"STAT").unwrap();
        assert_eq!(decode(&old[..cut])["stove"].stats, Vec::<u8>::new());
        // One from when the dimensions shared a map is sorted out.
        let mut shared = map.clone();
        shared.get_mut("hotty").unwrap().pos.x += crate::scorch::SCORCH_ORIGIN as f32;
        let old = encode(&shared);
        let cut = old.windows(4).position(|w| w == b"DIMS").unwrap();
        let back = decode(&old[..cut]);
        assert_eq!(back["hotty"].dim, Dim::Scorch);
        assert_eq!(back["hotty"].pos, Vec3::new(-5.0, 40.0, 2.0));
        assert_eq!(back["stove"].dim, Dim::Over);
        assert!(decode(&[1, 0, 0]).is_empty());
        // Without a report, the bag is laid out in stacks.
        let l = layout_from(&[(DIRT, 130), (DIAMOND, 1)]);
        assert_eq!(&l[..4], &[(DIRT, 64, 0), (DIRT, 64, 0), (DIRT, 2, 0), (DIAMOND, 1, 0)]);
        assert_eq!(l.len(), SLOTS);
        // Nonsense in a report is cleaned up.
        let c = clean(vec![(DIRT, 200, 5), (60000, 3, 0), (PICK_IRON, 1, 60000)]);
        assert_eq!(&c[..3], &[(DIRT, 64, 0), (AIR, 0, 0), (PICK_IRON, 1, 249)]);
    }
}
