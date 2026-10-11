//! **Reputation** (v0.5): each village has its own opinion of each player,
//! from -100 (hated) to 100 (loved), kept where the world lives.
//!
//! - Up: trading with its Hmmers (+1 a trade), curing a Zombie Hmmer (+25),
//!   seeing off monsters near it at night or in a raid (+2 each), and saving
//!   it from a raid (+30).
//! - Down: hitting a Hmmer (-8) or its Clanker (-12), and worse for killing
//!   one (-30). Grudges fade a little each day; good opinions don't.
//! - It shows: a liked player pays less (up to a quarter off at 100) and a
//!   disliked one more (up to half as much again); at -60 the Hmmers won't
//!   trade at all and keep away. The Clanker turns on anyone the village
//!   dislikes (-30 or worse), and now and then brings a flower to someone it
//!   loves (50 or more).
//!
//! A village is known by its square (see raids.rs). A Hmmer living outside
//! any village counts its home patch (64 blocks across) as one.

use crate::block::*;
use crate::entity::MobKind;
use crate::game::Game;
use crate::villagers::Trade;
use macroquad::math::{IVec3, Vec3};
use std::collections::HashMap;

pub const TRADE: i32 = 1;
pub const CURE: i32 = 25;
pub const DEFEND: i32 = 2;
pub const RAID_WON: i32 = 30;
pub const HIT_HMMER: i32 = -8;
pub const HIT_CLANKER: i32 = -12;
pub const KILL: i32 = -30;
/// At or below: the Clanker is after you, and the Hmmers keep away.
pub const DISLIKED: i32 = -30;
/// At or below: the Hmmers won't trade.
pub const HATED: i32 = -60;
/// At or above: flowers from the Clanker.
pub const LOVED: i32 = 50;
/// How much a grudge fades each day.
const FORGIVE: i32 = 4;
/// Seconds a hit counts as "just now" (for a kill), and between looks round.
const RECENT: f32 = 4.0;
const LOOK_EVERY: f32 = 1.0;

/// What a village thinks of someone, in a word.
pub fn standing(rep: i32) -> &'static str {
    match rep {
        ..=-60 => "Hated",
        -59..=-30 => "Disliked",
        -29..=-10 => "Wary",
        -9..=9 => "Neutral",
        10..=49 => "Liked",
        _ => "Loved",
    }
}

/// The colour an opinion is shown in: grey when neutral, shading to green
/// as the village likes you more and to red as it likes you less.
pub fn standing_colour(rep: i32) -> [f32; 3] {
    const GREY: [f32; 3] = [0.72, 0.72, 0.72];
    const GREEN: [f32; 3] = [0.3, 0.95, 0.35];
    const RED: [f32; 3] = [1.0, 0.3, 0.28];
    let t = (rep.clamp(-100, 100) as f32 / 100.0).abs();
    let to = if rep >= 0 { GREEN } else { RED };
    [0, 1, 2].map(|i| GREY[i] + (to[i] - GREY[i]) * t)
}

/// A trade's price for someone the village thinks `rep` of.
pub fn rep_price(mut t: Trade, rep: i32) -> Trade {
    let rep = rep.clamp(-100, 100) as f32;
    let factor = if rep >= 0.0 { 1.0 - rep / 400.0 } else { 1.0 - rep / 200.0 };
    for g in t.give.iter_mut().filter(|g| g.0 != AIR) {
        let n = g.1 as f32 * factor;
        // (Discounts round down to at least one; markups round up.)
        g.1 = if factor < 1.0 { (n.floor() as u8).max(1) } else { n.ceil().min(64.0) as u8 };
    }
    t
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reputation {
    /// (village, player record key) -> opinion.
    pub map: HashMap<(IVec3, String), i32>,
    /// The day the grudges last faded, and the day each player last got a flower.
    pub day: u32,
    pub flowers: HashMap<(IVec3, String), u32>,
    /// (Not saved.) Which village each mob belongs to; who last hit each mob, and how long ago.
    pub mob_village: HashMap<u32, IVec3>,
    pub hits: HashMap<u32, (String, f32)>,
    pub look: f32,
}

impl Reputation {
    pub fn get(&self, village: IVec3, who: &str) -> i32 {
        self.map.get(&(village, crate::players::record_key(who))).copied().unwrap_or(0)
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = format!("day\t{}\n", self.day);
        let mut v: Vec<_> = self.map.iter().collect();
        v.sort_by(|a, b| (a.0 .0.to_array(), &a.0 .1).cmp(&(b.0 .0.to_array(), &b.0 .1)));
        for ((p, who), n) in v {
            out.push_str(&format!("rep\t{}\t{}\t{}\t{who}\t{n}\n", p.x, p.y, p.z));
        }
        let mut f: Vec<_> = self.flowers.iter().collect();
        f.sort_by(|a, b| (a.0 .0.to_array(), &a.0 .1).cmp(&(b.0 .0.to_array(), &b.0 .1)));
        for ((p, who), d) in f {
            out.push_str(&format!("flower\t{}\t{}\t{}\t{who}\t{d}\n", p.x, p.y, p.z));
        }
        out.into_bytes()
    }

    pub fn decode(b: &[u8]) -> Reputation {
        let mut r = Reputation::default();
        for line in String::from_utf8_lossy(b).lines() {
            let f: Vec<&str> = line.split('\t').collect();
            match f[..] {
                ["day", d] => r.day = d.parse().unwrap_or(0),
                [kind @ ("rep" | "flower"), x, y, z, who, n] => {
                    let (Ok(x), Ok(y), Ok(z)) = (x.parse(), y.parse(), z.parse()) else { continue };
                    let key = (IVec3::new(x, y, z), who.to_string());
                    if kind == "rep" {
                        if let Ok(n) = n.parse::<i32>() {
                            r.map.insert(key, n.clamp(-100, 100));
                        }
                    } else if let Ok(d) = n.parse::<u32>() {
                        r.flowers.insert(key, d);
                    }
                }
                _ => {}
            }
        }
        r
    }
}

impl Game {
    /// The village a place belongs to: its square, else (outside any
    /// village) the patch of land it's in.
    pub fn village_key(&self, at: Vec3) -> IVec3 {
        match self.village_at(at) {
            Some(c) => c.floor().as_ivec3(),
            None => IVec3::new((at.x / 64.0).floor() as i32 * 64, i32::MIN, (at.z / 64.0).floor() as i32 * 64),
        }
    }

    /// Which village a mob belongs to (where it lives, else where it is).
    pub fn mob_village(&mut self, mob: u32) -> Option<IVec3> {
        if let Some(v) = self.rep.mob_village.get(&mob) {
            return Some(*v);
        }
        let m = self.mobs.iter().find(|m| m.id == mob)?;
        let at = m.home.unwrap_or(m.body.pos);
        let v = self.village_key(at);
        self.rep.mob_village.insert(mob, v);
        Some(v)
    }

    /// What `mob`'s village thinks of `who` (a joined player's own game: what the host last said).
    pub fn village_rep_with(&mut self, mob: u32, who: &str) -> i32 {
        if self.is_client() {
            return self.rep_seen.get(&mob).copied().unwrap_or(0) as i32;
        }
        match self.mob_village(mob) {
            Some(v) => self.rep.get(v, who),
            None => 0,
        }
    }

    /// The same, without remembering anything (for drawing the trade screen).
    pub fn village_rep_of(&self, mob: u32, who: &str) -> i32 {
        if self.is_client() {
            return self.rep_seen.get(&mob).copied().unwrap_or(0) as i32;
        }
        let v = match self.rep.mob_village.get(&mob) {
            Some(v) => *v,
            None => match self.mobs.iter().find(|m| m.id == mob) {
                Some(m) => self.village_key(m.home.unwrap_or(m.body.pos)),
                None => return 0,
            },
        };
        self.rep.get(v, who)
    }

    /// Where the world lives: `village` thinks `delta` more of `who`.
    pub fn change_rep(&mut self, village: IVec3, who: &str, delta: i32) {
        if who.is_empty() || delta == 0 {
            return;
        }
        let key = (village, crate::players::record_key(who));
        let before = self.rep.map.get(&key).copied().unwrap_or(0);
        let after = (before + delta).clamp(-100, 100);
        self.rep.map.insert(key, after);
        let line = if before < LOVED && after >= LOVED {
            Some("The village thinks the world of you now. Its Hmmers give you their best prices.")
        } else if before > DISLIKED && after <= DISLIKED {
            Some("The village won't forget that. Its Clanker is watching you.")
        } else if before > HATED && after <= HATED {
            Some("The village has had enough of you. Its Hmmers won't trade with you.")
        } else {
            None
        };
        if let Some(line) = line {
            self.tell(who, line);
        }
        if after >= LOVED {
            self.advance_for(&crate::players::record_key(who), "pillar_of_the_community");
        }
        if after <= HATED {
            self.advance_for(&crate::players::record_key(who), "persona_non_grata");
        }
    }

    /// The name of player `id` (ours, or a joined player's).
    pub(crate) fn player_name_of(&self, id: u32) -> Option<String> {
        if id == self.my_id && !self.dedicated {
            Some(self.player_name.clone())
        } else {
            self.peers.get(&id).map(|p| p.name.clone())
        }
    }

    /// Where the world lives: player `who` (an id) just hit `mob`.
    pub fn note_player_hit(&mut self, mob: u32, who: u32) {
        if self.is_client() {
            return;
        }
        let Some(name) = self.player_name_of(who) else { return };
        let Some(kind) = self.mobs.iter().find(|m| m.id == mob).map(|m| m.kind) else { return };
        self.rep.hits.insert(mob, (name.clone(), RECENT));
        let delta = match kind {
            MobKind::Hmmer => HIT_HMMER,
            MobKind::Clanker => HIT_CLANKER,
            _ => return,
        };
        if let Some(v) = self.mob_village(mob) {
            self.change_rep(v, &name, delta);
            self.send_rep_news(who, mob);
        }
    }

    /// A joined player hears how they stand with `mob`'s village (and Hmmer).
    pub fn send_rep_news(&mut self, who: u32, mob: u32) {
        if who != self.my_id && self.peers.contains_key(&who) && self.mobs.iter().any(|m| m.id == mob && m.kind == MobKind::Hmmer) {
            let name = crate::players::record_key(&self.peer_name(who));
            let trades = self.regulars.get(&(mob, name)).copied().unwrap_or(0);
            self.send_regular_to(who, mob, trades);
        }
    }

    /// Where the world lives: a mob died. Killing a Hmmer or a Clanker is
    /// remembered; killing a monster that was bothering a village is too.
    pub fn rep_mob_died(&mut self, mob: u32, kind: MobKind, at: Vec3) {
        let Some((who, _)) = self.rep.hits.remove(&mob) else { return };
        self.bounty_kill(&who, kind);
        if matches!(kind, MobKind::Hmmer | MobKind::Clanker) {
            if let Some(v) = self.mob_village(mob) {
                self.change_rep(v, &who, KILL);
            }
        } else if kind.hostile() && (self.is_night() || self.raid.is_some()) && let Some(c) = self.village_at(at) {
            self.change_rep(c.floor().as_ivec3(), &who, DEFEND);
        }
        self.rep.mob_village.remove(&mob);
    }

    /// Where the world lives: everyone near a saved village is thanked.
    pub fn rep_raid_won(&mut self, centre: Vec3, who: &str) {
        self.change_rep(centre.floor().as_ivec3(), who, RAID_WON);
    }

    /// Where the world lives: grudges fade, Clankers keep an eye on who's
    /// about, and Hmmers keep away from people they don't like.
    pub fn reputation_tick(&mut self, dt: f32) {
        if self.is_client() {
            return;
        }
        for h in self.rep.hits.values_mut() {
            h.1 -= dt;
        }
        // (A dying mob's last hit is kept until it's gone: see `rep_mob_died`.)
        let alive: std::collections::HashSet<u32> = self.mobs.iter().filter(|m| m.health > 0.0).map(|m| m.id).collect();
        self.rep.hits.retain(|id, h| h.1 > 0.0 || !alive.contains(id));
        if self.day != self.rep.day {
            let days = self.day.saturating_sub(self.rep.day).min(30) as i32;
            self.rep.day = self.day;
            for n in self.rep.map.values_mut().filter(|n| **n < 0) {
                *n = (*n + FORGIVE * days).min(0);
            }
            self.rep.map.retain(|_, n| *n != 0);
        }
        self.rep.look -= dt;
        if self.rep.look > 0.0 || self.rep.map.is_empty() {
            return;
        }
        self.rep.look = LOOK_EVERY;
        let players: Vec<(u32, String, Vec3)> = self.player_spots_by_id().into_iter().filter_map(|(id, at)| self.player_name_of(id).map(|n| (id, n, at))).collect();
        let watchers: Vec<(u32, MobKind, Vec3)> = self.mobs.iter().filter(|m| matches!(m.kind, MobKind::Hmmer | MobKind::Clanker) && m.health > 0.0).map(|m| (m.id, m.kind, m.body.pos)).collect();
        for (id, kind, pos) in watchers {
            let Some((_, name, at)) = players.iter().filter(|p| p.2.distance(pos) < 16.0).min_by(|a, b| a.2.distance(pos).total_cmp(&b.2.distance(pos))).cloned() else { continue };
            let Some(village) = self.mob_village(id) else { continue };
            let rep = self.rep.get(village, &name);
            match kind {
                MobKind::Clanker if rep <= DISLIKED => {
                    if let Some(m) = self.mobs.iter_mut().find(|m| m.id == id) {
                        m.angry = true;
                    }
                }
                MobKind::Clanker if rep >= LOVED && at.distance(pos) < 6.0 => {
                    let key = (village, crate::players::record_key(&name));
                    if self.rep.flowers.get(&key) != Some(&self.day) && self.rng.chance(0.05) {
                        self.rep.flowers.insert(key, self.day);
                        self.pop_drop(at + Vec3::Y * 0.5, FLOWER, 1);
                        self.tell(&name, "The Clanker holds out a flower to you. Somehow it manages to look shy.");
                    }
                }
                MobKind::Hmmer if rep <= DISLIKED && at.distance(pos) < 5.0 => {
                    if let Some(m) = self.mobs.iter_mut().find(|m| m.id == id) {
                        m.flee = 2.0;
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::MobKind;

    #[test]
    fn prices_follow_the_village_and_words_fit() {
        let t = Trade { give: [(GOLD_INGOT, 8), (AIR, 0)], get: (BREAD, 1), wear: 0 };
        assert_eq!(rep_price(t, 0).give[0].1, 8);
        assert_eq!(rep_price(t, 100).give[0].1, 6, "a quarter off at best");
        assert_eq!(rep_price(t, -100).give[0].1, 12, "half again at worst");
        let one = Trade { give: [(GOLD_INGOT, 1), (AIR, 0)], get: (BREAD, 1), wear: 0 };
        assert_eq!(rep_price(one, 100).give[0].1, 1);
        assert_eq!(rep_price(one, -40).give[0].1, 2);
        assert_eq!(standing(0), "Neutral");
        assert_eq!(standing(60), "Loved");
        assert_eq!(standing(-70), "Hated");
        // Grey in the middle, all the way to green and red at the ends, and in between on the way.
        assert_eq!(standing_colour(0), [0.72, 0.72, 0.72]);
        assert_eq!(standing_colour(100), [0.3, 0.95, 0.35]);
        assert_eq!(standing_colour(-150), [1.0, 0.3, 0.28]);
        let half = standing_colour(50);
        assert!(half[1] > 0.72 && half[1] < 0.95 && half[0] < 0.72);
        let half = standing_colour(-50);
        assert!(half[0] > 0.72 && half[0] < 1.0 && half[1] < 0.72);
    }

    #[test]
    fn hitting_and_killing_are_remembered_trading_helps_and_grudges_fade() {
        let mut g = crate::game::tests::arena(601);
        g.player_name = "Ann".into();
        let hmmer = g.alloc_mob(MobKind::Hmmer, Vec3::new(3.5, 50.0, 0.5));
        let v = g.mob_village(hmmer).unwrap();
        g.note_player_hit(hmmer, g.my_id);
        assert_eq!(g.rep.get(v, "Ann"), HIT_HMMER);
        // A kill straight after a hit is a kill.
        g.rep_mob_died(hmmer, MobKind::Hmmer, Vec3::new(3.5, 50.0, 0.5));
        assert_eq!(g.rep.get(v, "ann"), HIT_HMMER + KILL);
        assert!(g.rep.get(v, "Ann") <= DISLIKED);
        // The Clanker is after us now, and Hmmers keep away.
        // (In the same patch of land as the Hmmer: there's no village here.)
        let clanker = g.alloc_mob(MobKind::Clanker, Vec3::new(5.5, 50.0, 4.5));
        let other = g.alloc_mob(MobKind::Hmmer, Vec3::new(1.5, 50.0, 1.5));
        g.reputation_tick(1.5);
        assert!(g.mobs.iter().any(|m| m.id == clanker && m.angry));
        assert!(g.mobs.iter().any(|m| m.id == other && m.flee > 0.0));
        // Prices are worse for us.
        let t = Trade { give: [(GOLD_INGOT, 8), (AIR, 0)], get: (BREAD, 1), wear: 0 };
        assert!(rep_price(t, g.village_rep_with(other, "Ann")).give[0].1 > 8);
        // A few days later it has mellowed.
        let before = g.rep.get(v, "Ann");
        g.day += 3;
        g.reputation_tick(0.1);
        assert_eq!(g.rep.get(v, "Ann"), before + FORGIVE * 3);
        // Saved with the world.
        let back = Game::from_save(g.to_save());
        assert_eq!(back.rep.map, g.rep.map);
        assert_eq!(Reputation::decode(&g.rep.encode()).map, g.rep.map);
    }

    #[test]
    fn a_loved_player_gets_flowers_and_a_hated_one_no_trades() {
        let mut g = crate::game::tests::arena(602);
        g.player_name = "Bo".into();
        let clanker = g.alloc_mob(MobKind::Clanker, Vec3::new(2.5, 50.0, 0.5));
        let v = g.mob_village(clanker).unwrap();
        g.change_rep(v, "Bo", 60);
        assert!(g.advancements.has("pillar_of_the_community"));
        let mut flowers = 0;
        for _ in 0..400 {
            g.reputation_tick(1.0);
            flowers = g.drops.iter().filter(|d| d.item == FLOWER).count();
            if flowers > 0 {
                break;
            }
        }
        assert_eq!(flowers, 1, "a flower, now and then");
        // Only one a day.
        for _ in 0..200 {
            g.reputation_tick(1.0);
        }
        assert_eq!(g.drops.iter().filter(|d| d.item == FLOWER).count(), 1);
        g.change_rep(v, "Bo", -200);
        assert!(g.advancements.has("persona_non_grata"));
        assert_eq!(g.rep.get(v, "Bo"), -100);
    }
}
