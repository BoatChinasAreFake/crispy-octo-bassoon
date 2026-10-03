//! Archaeology: slow, careful, and much more involved than hitting sand.
//!
//! **Dig sites** lie buried around the world: Sunken Sandstone ruins under
//! desert sand, Trailfolk ruins of terracotta and mud brick under forests,
//! the Drowned Port's walls on warm sea floors, and the Hushed Ones' city in
//! the Deep Dark. Each was built by a different **culture**, with its own
//! pottery, relics and stories. Their fill hides **Suspicious Sand** and
//! **Suspicious Gravel**: dig it and whatever was inside is lost.
//!
//! Hold right-click with a **Brush** instead. Brushing uncovers the find, but
//! it also builds up **pressure** on it, and now and then the grit shifts
//! ("the find shifts!") and the pressure jumps. Let go to let it settle; push
//! too hard and the find **cracks**. One crack and it's merely Fine, then
//! Worn, then Cracked; crack it four times and it shatters. A **Dimond Brush**
//! is gentler, and so is experience: every dig earns Archaeologist levels.
//!
//! How deep a find was buried matters: the **recent layer** (the top couple of
//! blocks) is mostly pottery shards and coins, the **old layer** gives up
//! relics, clay tablets and torn map fragments, and the **ancient layer** is
//! where the rarest relics are, often **encrusted** (clean them at a
//! **Restoration Bench**) and, at Trailfolk sites, Torchflower seeds that grow
//! a flower bees make Ancient honey from.
//!
//! - **Pottery shards** (twelve designs) go on **Decorated Pots**: a shard
//!   and three bricks.
//! - **Relics** (three per culture) keep their condition; the **Field
//!   Journal** records the best of each, and finishing a culture's
//!   collection (every shard and relic) is rewarded. All relics Pristine is
//!   the curator's dream.
//! - **Clay tablets** are someone's diary; right-click to read.
//! - **Map fragments** point to another site of their culture.
//!
//! The brushing happens on the archaeologist's own machine; the find is
//! rolled where the world lives, like a fish.

use crate::block::*;
use crate::inventory::Wear;
use crate::noise::Rng;
use crate::structures::Kind;
use crate::world::{Biome, Generator};
use macroquad::math::{IVec3, Vec3};
use std::collections::BTreeMap;

/// Pottery shards: key, name, and the little motif painted on them (7x7).
pub const SHARDS: [(&str, &str, [&str; 7]); 12] = [
    ("archer", "Archer", ["...#...", "..#.#..", ".#...#.", "#######", ".#...#.", "..#.#..", "...#..."]),
    ("prize", "Prize", ["...#...", "..###..", ".#####.", "#######", ".#####.", "..###..", "...#..."]),
    ("arms_up", "Arms Up", ["#..#..#", ".#.#.#.", "..###..", "...#...", "..#.#..", ".#...#.", "#.....#"]),
    ("skull", "Skull", [".#####.", "#######", "#..#..#", "#######", ".#####.", ".#.#.#.", "......."]),
    ("heartbreak", "Heartbreak", [".##.##.", "#######", "###.###", ".##.##.", "..#.#..", "...#...", "......."]),
    ("friend", "Friend", [".#####.", "#.....#", "#.#.#.#", "#.....#", "#.#.#.#", "#..#..#", ".#####."]),
    ("sheaf", "Sheaf", ["#.#.#.#", ".#####.", "..###..", "...#...", "..###..", ".#.#.#.", "#..#..#"]),
    ("hive", "Hive", ["..###..", ".#...#.", "#..#..#", "#.###.#", "#..#..#", ".#...#.", "..###.."]),
    ("fish", "Fish", ["......#", ".###.##", "#.####.", "#######", "#####..", ".###.##", "......#"]),
    ("explorer", "Explorer", ["#######", "#.....#", "#.#...#", "#..#..#", "#...#.#", "#.....#", "#######"]),
    ("danger", "Danger", ["#######", "#.#.#.#", "#######", "##...##", "##.#.##", "##.#.##", "#######"]),
    ("echo", "Echo", ["#######", "#.....#", "#.###.#", "#.#.#.#", "#.#...#", "#.#####", "#......"]),
];

/// Relics: key, name, culture.
pub const RELICS: [(&str, &str, Culture); 12] = [
    ("scarab_charm", "Scarab Charm (Lucky, Allegedly)", Culture::Sunken),
    ("gilded_mask", "Gilded Mask (Smug Expression)", Culture::Sunken),
    ("sun_disc", "Sun Disc (Warm to the Touch)", Culture::Sunken),
    ("bead_necklace", "Bead Necklace (Hand-Strung)", Culture::Trail),
    ("bone_flute", "Bone Flute (Plays One Note)", Culture::Trail),
    ("woven_charm", "Woven Charm (Smells of Pine)", Culture::Trail),
    ("ships_bell", "Ship's Bell (Still Rings)", Culture::Drowned),
    ("barnacle_compass", "Barnacled Compass (Points Down)", Culture::Drowned),
    ("pearl_brooch", "Pearl Brooch (Fancy, Damp)", Culture::Drowned),
    ("silent_bell", "Silent Bell (Rings Without a Sound)", Culture::Hushed),
    ("echo_idol", "Echo Idol (Whispers Back)", Culture::Hushed),
    ("hush_mask", "Hushed Mask (Do Not Wear)", Culture::Hushed),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum Culture {
    Sunken,
    Trail,
    Drowned,
    Hushed,
}

impl Culture {
    pub const ALL: [Culture; 4] = [Culture::Sunken, Culture::Trail, Culture::Drowned, Culture::Hushed];

    pub fn index(self) -> usize {
        Culture::ALL.iter().position(|c| *c == self).unwrap_or(0)
    }
    pub fn from_index(i: u8) -> Culture {
        Culture::ALL.get(i as usize).copied().unwrap_or(Culture::Sunken)
    }
    pub fn name(self) -> &'static str {
        match self {
            Culture::Sunken => "the Sunken Sandstone people",
            Culture::Trail => "the Trailfolk",
            Culture::Drowned => "the Drowned Port",
            Culture::Hushed => "the Hushed Ones",
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Culture::Sunken => "Sunken Sandstone",
            Culture::Trail => "Trailfolk",
            Culture::Drowned => "Drowned Port",
            Culture::Hushed => "Hushed Ones",
        }
    }
    /// The dig sites they left.
    pub fn site(self) -> Kind {
        match self {
            Culture::Sunken => Kind::DesertRuins,
            Culture::Trail => Kind::TrailRuins,
            Culture::Drowned => Kind::OceanRuins,
            Culture::Hushed => Kind::HushedCity,
        }
    }
    /// Indices into `SHARDS`.
    pub fn shards(self) -> &'static [usize] {
        match self {
            Culture::Sunken => &[0, 1, 2, 3],
            Culture::Trail => &[4, 5, 6, 7],
            Culture::Drowned => &[8, 9, 10],
            Culture::Hushed => &[11],
        }
    }
    /// Indices into `RELICS`.
    pub fn relics(self) -> [usize; 3] {
        let i = self.index() * 3;
        [i, i + 1, i + 2]
    }
    /// What their clay tablets say.
    pub fn lore(self) -> &'static [&'static str] {
        match self {
            Culture::Sunken => &[
                "\"Day 3 of building the temple. Sand in everything. EVERYTHING.\"",
                "\"The Sun Disc must face the morning. Gerald put it facing the wall. Again.\"",
                "\"Note to self: do not bury the treasure under the treasury. Too obvious.\"",
                "\"The scarabs are lucky. The scarab-keeper is not. Long story.\"",
                "\"Tax receipt: three goats, one golden mask (smug).\"",
            ],
            Culture::Trail => &[
                "\"Walked the trail again today. It's a loop. We've been doing this for years.\"",
                "\"The torchflowers are blooming. The bees went absolutely feral.\"",
                "\"Traded a necklace for a flute. The flute plays one note. Regrets.\"",
                "\"If you find this, the pottery was ours. The bad pottery was Kevin's.\"",
                "\"Planted the old seeds by the path. They glow at night. Nobody can sleep.\"",
            ],
            Culture::Drowned => &[
                "\"Harbour log: tide came in. Tide did not go back out. Investigating.\"",
                "\"The compass points down now. The navigator says this is fine.\"",
                "\"Rang the ship's bell. Something big rang back.\"",
                "\"Lost a pearl brooch overboard. Also the ship. Also the harbour.\"",
                "\"Fish census: many. Fish attitude: poor.\"",
            ],
            Culture::Hushed => &[
                "\"Quiet in the halls. QUIET. It listens.\"",
                "\"We built the city to keep it asleep. Tiptoe. Always tiptoe.\"",
                "\"The bell rings without a sound so it does not hear us pray.\"",
                "\"Do not wear the mask. Do not wear the mask. Do not wear the\"",
                "\"Someone dropped a pot. We are moving out. Goodbye.\"",
            ],
        }
    }
}

/// How well a find came out.
pub const CONDITIONS: [&str; 4] = ["Pristine", "Fine", "Worn", "Cracked"];

/// A relic, tablet, fragment or encrusted lump carries a label in its wear.
pub fn is_find(id: Id) -> bool {
    (RELIC_FIRST..RELIC_FIRST + 12).contains(&id) || matches!(id, ENCRUSTED_RELIC | MAP_FRAGMENT | CLAY_TABLET)
}

pub fn is_shard(id: Id) -> bool {
    (SHARD_FIRST..SHARD_FIRST + 12).contains(&id)
}

pub fn is_relic(id: Id) -> bool {
    (RELIC_FIRST..RELIC_FIRST + 12).contains(&id)
}

pub fn is_suspicious(id: Id) -> bool {
    matches!(id, SUSPICIOUS_SAND | SUSPICIOUS_GRAVEL)
}

pub fn is_pot(id: Id) -> bool {
    (POT_FIRST..POT_FIRST + 13).contains(&id)
}

/// A label: condition (0 pristine .. 3 cracked) and something extra (a
/// culture, or which tablet).
pub fn tag(condition: u8, extra: u8) -> Wear {
    (((condition.min(3) as u32) + 1) | ((extra as u32) << 4)) << 16
}

pub fn condition_of(w: Wear) -> Option<u8> {
    let c = (w >> 16) & 0xF;
    (c > 0).then(|| (c - 1) as u8)
}

pub fn extra_of(w: Wear) -> u8 {
    ((w >> 20) & 0xFF) as u8
}

/// Tooltip detail for a find (its condition, culture or story).
pub fn describe(id: Id, w: Wear) -> Option<String> {
    if is_relic(id) {
        let c = condition_of(w).unwrap_or(1);
        let culture = RELICS[(id - RELIC_FIRST) as usize].2;
        return Some(format!("{}, made by {}", CONDITIONS[c as usize], culture.name()));
    }
    match id {
        ENCRUSTED_RELIC | MAP_FRAGMENT => Some(format!("Left by {}", Culture::from_index(extra_of(w)).name())),
        CLAY_TABLET => Some("Right-click to read".into()),
        _ => None,
    }
}

/// How deep a find was buried: 0 recent, 1 old, 2 ancient.
pub fn layer(depth: i32) -> u8 {
    if depth <= 2 {
        0
    } else if depth <= 5 {
        1
    } else {
        2
    }
}

pub const LAYER_NAMES: [&str; 3] = ["recent", "old", "ancient"];

/// Whose ruins these are, from where they are.
pub fn culture_at(g: &Generator, p: IVec3) -> Culture {
    if p.y < crate::deepdark::DEEP_TOP && g.deep_dark(p.x, p.z) {
        return Culture::Hushed;
    }
    match g.column(p.x, p.z).1 {
        Biome::Desert | Biome::Badlands => Culture::Sunken,
        Biome::Ocean => Culture::Drowned,
        _ => Culture::Trail,
    }
}

/// How deep below the ground (or sea floor) `p` is.
pub fn depth_at(g: &Generator, p: IVec3) -> i32 {
    (g.column(p.x, p.z).0 - p.y).max(0)
}

/// What came out of the ground.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Find {
    pub item: Id,
    pub n: u8,
    pub wear: Wear,
}

/// Roll a find (where the world lives).
pub fn roll_find(culture: Culture, layer: u8, condition: u8, level: u32, rng: &mut Rng) -> Find {
    let shard = |rng: &mut Rng| {
        let list = culture.shards();
        Find { item: SHARD_FIRST + list[rng.int(0, list.len() as i32 - 1) as usize] as Id, n: 1, wear: 0 }
    };
    let relic = |rng: &mut Rng| {
        let r = culture.relics()[rng.int(0, 2) as usize];
        Find { item: RELIC_FIRST + r as Id, n: 1, wear: tag(condition, 0) }
    };
    let tablet = |rng: &mut Rng| Find { item: CLAY_TABLET, n: 1, wear: tag(0, (culture.index() * 16 + rng.int(0, culture.lore().len() as i32 - 1) as usize) as u8) };
    let bonus = level as f32 * 0.02;
    let r = rng.f32();
    match layer {
        0 => {
            if r < 0.45 {
                shard(rng)
            } else if r < 0.7 {
                Find { item: ANCIENT_COIN, n: rng.int(1, 3) as u8, wear: 0 }
            } else if r < 0.8 + bonus {
                tablet(rng)
            } else {
                let junk = [(STICK, 2), (COAL, 1), (BONE, 1), (WHEAT_SEEDS, 3), (GOLD_INGOT, 1), (BRICK, 2)];
                let (item, n) = junk[rng.int(0, junk.len() as i32 - 1) as usize];
                Find { item, n, wear: 0 }
            }
        }
        1 => {
            if r < 0.2 + bonus {
                relic(rng)
            } else if r < 0.55 {
                shard(rng)
            } else if r < 0.7 {
                Find { item: MAP_FRAGMENT, n: 1, wear: tag(0, culture.index() as u8) }
            } else if r < 0.8 {
                tablet(rng)
            } else if r < 0.9 {
                Find { item: ENCRUSTED_RELIC, n: 1, wear: tag(0, culture.index() as u8) }
            } else {
                Find { item: ANCIENT_COIN, n: rng.int(2, 5) as u8, wear: 0 }
            }
        }
        _ => {
            let seeds = if culture == Culture::Trail { 0.25 } else { 0.1 };
            if r < 0.3 + bonus {
                relic(rng)
            } else if r < 0.55 {
                Find { item: ENCRUSTED_RELIC, n: 1, wear: tag(0, culture.index() as u8) }
            } else if r < 0.55 + seeds {
                Find { item: TORCHFLOWER_SEEDS, n: rng.int(1, 2) as u8, wear: 0 }
            } else if culture == Culture::Trail && r < 0.55 + seeds + 0.08 {
                // Something big laid this, a long time ago (see sniffers.rs).
                Find { item: SNIFFER_EGG, n: 1, wear: 0 }
            } else if r < 0.85 {
                shard(rng)
            } else {
                Find { item: MAP_FRAGMENT, n: 1, wear: tag(0, culture.index() as u8) }
            }
        }
    }
}

// ------------------------------------------------------------------ brushing

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DigEvent {
    /// A brush stroke (for the sound and dust).
    Scrape,
    /// The grit shifted: pressure jumped.
    Grit,
    /// Too much pressure: the find cracked.
    Crack,
    /// Done: (cracks so far).
    Done(u8),
    /// Cracked once too often.
    Shattered,
}

/// A brushing in progress.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dig {
    pub pos: IVec3,
    /// 0..1: uncovered.
    pub progress: f32,
    /// 0..1: it cracks at 1.
    pub pressure: f32,
    pub cracks: u8,
    /// Seconds to the next shift of grit, and to the next scrape sound.
    grit: f32,
    scrape: f32,
    /// Seconds since right-click was let go (the dig is forgotten after a while).
    pub idle: f32,
}

impl Dig {
    pub fn new(pos: IVec3, rng: &mut Rng) -> Dig {
        Dig { pos, progress: 0.0, pressure: 0.0, cracks: 0, grit: rng.range(1.0, 3.0), scrape: 0.0, idle: 0.0 }
    }

    /// `gentle`: 1.0 for a Brush, less for a Dimond Brush. `level`: Archaeologist level.
    pub fn update(&mut self, dt: f32, brushing: bool, gentle: f32, level: u32, rng: &mut Rng) -> Vec<DigEvent> {
        let mut ev = Vec::new();
        if !brushing {
            self.idle += dt;
            self.pressure = (self.pressure - 0.9 * dt).max(0.0);
            return ev;
        }
        self.idle = 0.0;
        let skill = 1.0 - level.min(10) as f32 * 0.04;
        self.progress += (0.2 + if gentle < 1.0 { 0.04 } else { 0.0 }) * dt;
        self.pressure += 0.3 * gentle * skill * dt;
        self.scrape -= dt;
        if self.scrape <= 0.0 {
            self.scrape = 0.35;
            ev.push(DigEvent::Scrape);
        }
        self.grit -= dt;
        if self.grit <= 0.0 {
            self.grit = rng.range(1.2, 3.5);
            self.pressure += rng.range(0.15, 0.35) * gentle * skill;
            ev.push(DigEvent::Grit);
        }
        if self.pressure >= 1.0 {
            self.cracks += 1;
            self.pressure = 0.35;
            if self.cracks >= 4 {
                ev.push(DigEvent::Shattered);
                return ev;
            }
            ev.push(DigEvent::Crack);
        }
        if self.progress >= 1.0 {
            ev.push(DigEvent::Done(self.cracks));
        }
        ev
    }
}

// ------------------------------------------------------------------ the journal

/// Everything you've dug up, per world.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Journal {
    pub xp: u32,
    pub digs: u32,
    pub cracked: u32,
    pub shattered: u32,
    pub restored: u32,
    /// Item key -> (count, best condition: 0 pristine .. 3 cracked).
    pub finds: BTreeMap<String, (u32, u8)>,
    /// Cultures whose collections are complete (bits).
    pub complete: u8,
}

impl Journal {
    /// Archaeologist level 0..=10.
    pub fn level(&self) -> u32 {
        ((self.xp as f32 / 10.0).sqrt() as u32).min(10)
    }

    /// Note a find. Returns true if it's new to the journal.
    pub fn note(&mut self, item: Id, condition: u8) -> bool {
        let e = self.finds.entry(reg().key_of(item).to_string()).or_insert((0, 3));
        let new = e.0 == 0;
        e.0 += 1;
        e.1 = e.1.min(condition);
        new
    }

    pub fn has(&self, item: Id) -> Option<(u32, u8)> {
        self.finds.get(reg().key_of(item)).copied()
    }

    /// How much of a culture's collection has been found: (found, total).
    pub fn progress(&self, c: Culture) -> (usize, usize) {
        let items: Vec<Id> = c.shards().iter().map(|&i| SHARD_FIRST + i as Id).chain(c.relics().iter().map(|&i| RELIC_FIRST + i as Id)).collect();
        (items.iter().filter(|&&i| self.has(i).is_some()).count(), items.len())
    }

    /// Every relic of every culture, pristine.
    pub fn curator(&self) -> bool {
        (0..12).all(|i| self.has(RELIC_FIRST + i).is_some_and(|(_, c)| c == 0))
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut v = Vec::new();
        for n in [self.xp, self.digs, self.cracked, self.shattered, self.restored] {
            v.extend_from_slice(&n.to_le_bytes());
        }
        v.push(self.complete);
        for (k, (n, c)) in &self.finds {
            v.push(k.len().min(255) as u8);
            v.extend_from_slice(&k.as_bytes()[..k.len().min(255)]);
            v.extend_from_slice(&n.to_le_bytes());
            v.push(*c);
        }
        v
    }

    pub fn decode(b: &[u8]) -> Journal {
        let mut j = Journal::default();
        if b.len() < 21 {
            return j;
        }
        let u = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        j.xp = u(0).min(1_000_000);
        j.digs = u(4);
        j.cracked = u(8);
        j.shattered = u(12);
        j.restored = u(16);
        j.complete = b[20];
        let mut i = 21;
        while i < b.len() {
            let len = b[i] as usize;
            if i + 1 + len + 5 > b.len() {
                break;
            }
            let key = String::from_utf8_lossy(&b[i + 1..i + 1 + len]).into_owned();
            let o = i + 1 + len;
            j.finds.insert(key, (u(o), b[o + 4].min(3)));
            i = o + 5;
        }
        j
    }
}

// ------------------------------------------------------------------ in the game

use crate::game::Game;
use crate::net::Msg;
use crate::sound::{Mat, Sfx};

impl Game {
    /// Per frame, on the archaeologist's own machine: brushing with right-click held.
    pub fn update_brushing(&mut self, dt: f32, use_held: bool) {
        let held = self.inv.held();
        let brush = matches!(held, BRUSH | DIAMOND_BRUSH);
        let target = match &self.target {
            Some(crate::game::Target::Block(h)) if brush && is_suspicious(self.world.get_v(h.pos)) => Some(h.pos),
            _ => None,
        };
        let Some(pos) = target else {
            if let Some(d) = &mut self.dig {
                d.idle += dt;
                d.pressure = (d.pressure - 0.9 * dt).max(0.0);
                if d.idle > 8.0 || !is_suspicious(self.world.get_v(d.pos)) {
                    self.dig = None;
                }
            }
            return;
        };
        if self.dig.is_none_or(|d| d.pos != pos) {
            self.dig = Some(Dig::new(pos, &mut self.rng));
        }
        let gentle = if held == DIAMOND_BRUSH { 0.65 } else { 1.0 };
        let level = self.journal.level();
        let mut rng = std::mem::replace(&mut self.rng, Rng::new(0));
        let events = self.dig.as_mut().map(|d| d.update(dt, use_held, gentle, level, &mut rng)).unwrap_or_default();
        self.rng = rng;
        if use_held {
            self.player.swing = self.player.swing.max(0.6);
        }
        let at = pos.as_vec3() + Vec3::splat(0.5);
        for e in events {
            match e {
                DigEvent::Scrape => {
                    self.sfx(Sfx::Brush, Some(at));
                    self.block_particles(pos, 2);
                }
                DigEvent::Grit => self.msg("The find shifts! Ease off..."),
                DigEvent::Crack => {
                    self.sfx(Sfx::Break(Mat::Glass), Some(at));
                    self.journal.cracked += 1;
                    self.msg("Crack! You pressed too hard. Gently does it.");
                }
                DigEvent::Shattered => {
                    self.dig = None;
                    self.journal.shattered += 1;
                    self.sfx(Sfx::Break(Mat::Glass), Some(at));
                    self.msg("It shattered. Somewhere, an archaeologist weeps.");
                    self.excavated(pos, 4);
                    return;
                }
                DigEvent::Done(cracks) => {
                    self.dig = None;
                    self.excavated(pos, cracks);
                    return;
                }
            }
        }
    }

    /// The brushing is done (or the find shattered: `cracks` 4): the world's owner decides what it was.
    fn excavated(&mut self, pos: IVec3, cracks: u8) {
        if matches!(self.inv.held(), BRUSH | DIAMOND_BRUSH) {
            self.use_tool(1);
        }
        self.journal.digs += 1;
        self.advance("careful_hands");
        if self.is_client() {
            self.net_send_msg(Msg::Excavate { x: pos.x, y: pos.y, z: pos.z, cracks });
            return;
        }
        let to = self.player.body.pos + Vec3::Y * 0.9;
        let level = self.journal.level();
        if let Some(line) = self.unearth(pos, cracks, level, to) {
            self.msg(line);
        }
        self.journal.xp += if cracks == 0 { 4 } else { 2 };
        let lvl = self.journal.level();
        if lvl > level {
            self.msg(format!("Archaeologist level {lvl}! Your brush hand is steadier."));
        }
    }

    /// Where the world lives: turn the suspicious block plain and roll its find,
    /// flinging it to `to`. Returns what to tell them.
    pub fn unearth(&mut self, pos: IVec3, cracks: u8, level: u32, to: Vec3) -> Option<String> {
        let id = self.world.get_v(pos);
        if !is_suspicious(id) {
            return None;
        }
        self.world.set_v(pos, if id == SUSPICIOUS_SAND { SAND } else { GRAVEL });
        let at = pos.as_vec3() + Vec3::new(0.5, 0.8, 0.5);
        if cracks >= 4 {
            return None;
        }
        let g = self.world.generator.clone();
        let culture = culture_at(&g, pos);
        let layer = layer(depth_at(&g, pos));
        let find = roll_find(culture, layer, cracks, level, &mut self.rng);
        const FLIGHT: f32 = 0.6;
        let vel = (to - at) / FLIGHT + Vec3::Y * (0.5 * crate::entity::GRAVITY * FLIGHT);
        self.spawn_drop(at, find.item, find.n, find.wear, vel, 0.0);
        // A little extra for the experienced.
        if self.rng.chance(level as f32 * 0.02) {
            self.spawn_drop(at, ANCIENT_COIN, 1, 0, vel * 0.9, 0.1);
        }
        self.sfx(Sfx::Pop, Some(at));
        let cond = if is_relic(find.item) { format!("{} ", CONDITIONS[cracks.min(3) as usize].to_lowercase()) } else { String::new() };
        Some(format!("From the {} layer: a {cond}{}.", LAYER_NAMES[layer as usize], item_name(find.item).split(" (").next().unwrap_or("")))
    }

    /// A joined player finished brushing at `pos`.
    pub fn host_excavate(&mut self, from: u32, pos: IVec3, cracks: u8) {
        let Some(peer) = self.peers.get(&from) else { return };
        let to = peer.target + Vec3::Y * 0.9;
        let near = (peer.target + Vec3::Y * 1.6).distance(pos.as_vec3() + Vec3::splat(0.5)) <= crate::multiplayer::REACH + 0.5;
        let brush = if self.peer_has(from, DIAMOND_BRUSH) { DIAMOND_BRUSH } else { BRUSH };
        if !near || !self.peer_has(from, brush) || !self.peer_rate_ok(from, "excavate", 2.0) {
            return;
        }
        self.host_wear(from, brush, 1);
        if let Some(line) = self.unearth(pos, cracks.min(4), 0, to) {
            self.system_message(Some(from), &line);
        }
    }

    /// Something arrived in the inventory: if it's a find, the journal notes it.
    pub fn journal_pickup(&mut self, item: Id, wear: Wear) {
        if !is_shard(item) && !is_relic(item) {
            return;
        }
        let cond = condition_of(wear).unwrap_or(if is_relic(item) { 1 } else { 0 });
        if self.journal.note(item, cond) {
            self.msg(format!("New in your Field Journal: {}.", item_name(item).split(" (").next().unwrap_or("")));
        }
        if is_relic(item) && cond == 0 {
            self.advance("pristine");
        }
        for c in Culture::ALL {
            let bit = 1 << c.index();
            let (found, total) = self.journal.progress(c);
            if found == total && self.journal.complete & bit == 0 {
                self.journal.complete |= bit;
                self.msg(format!("Collection complete: {}! Museums would fight over this.", c.title()));
                self.advance("collection_complete");
                self.add_xp(100);
            }
        }
        if self.journal.curator() {
            self.advance("curator");
        }
    }

    /// Right-click with a tablet or map fragment, or something at a Restoration Bench.
    /// Returns whether it did anything.
    pub fn use_find(&mut self, held: Id) -> bool {
        let wear = self.inv.wear[self.inv.selected];
        match held {
            CLAY_TABLET => {
                let x = extra_of(wear) as usize;
                let culture = Culture::from_index((x / 16) as u8);
                let lore = culture.lore();
                self.msg(format!("The tablet, from {}, reads: {}", culture.name(), lore[(x % 16).min(lore.len() - 1)]));
                true
            }
            MAP_FRAGMENT => {
                let culture = Culture::from_index(extra_of(wear));
                let here = self.player.body.pos;
                let g = self.world.generator.clone();
                match g.nearest_site(culture.site(), here, 96) {
                    Some(p) => {
                        let d = p.as_vec3() - here;
                        let dist = Vec3::new(d.x, 0.0, d.z).length();
                        self.msg(format!("The fragment shows ruins of {} about {:.0} blocks {}.", culture.name(), (dist / 10.0).round() * 10.0, compass_word(d)));
                    }
                    None => self.msg("The fragment is too faded to read. Or the ruins are very far away."),
                }
                true
            }
            FIELD_JOURNAL => {
                self.open_journal = true;
                true
            }
            _ => false,
        }
    }

    /// An encrusted relic, cleaned at a bench (where the world lives).
    pub fn restore(&mut self, who: Option<u32>, bench: IVec3, wear: Wear) -> Option<String> {
        if self.world.get_v(bench) != RESTORATION_BENCH {
            return None;
        }
        let culture = Culture::from_index(extra_of(wear));
        let r = culture.relics()[self.rng.int(0, 2) as usize];
        // Careful cleaning: usually fine, sometimes pristine.
        let cond = if self.rng.chance(0.35) { 0 } else { 1 };
        let at = bench.as_vec3() + Vec3::new(0.5, 1.1, 0.5);
        let to = match who {
            Some(p) => self.peers.get(&p).map(|q| q.target + Vec3::Y * 0.9)?,
            None => self.player.body.pos + Vec3::Y * 0.9,
        };
        self.fling_worn(at, to, RELIC_FIRST + r as Id, tag(cond, 0));
        self.sfx(Sfx::Brush, Some(at));
        if who.is_none() {
            self.journal.restored += 1;
            self.journal.xp += 5;
            self.advance("restorer");
        }
        Some(format!("You clean off centuries of grime: a {} {}!", CONDITIONS[cond as usize].to_lowercase(), RELICS[r].1.split(" (").next().unwrap_or("")))
    }

    /// The local player cleans an encrusted relic at a bench.
    pub fn use_restoration(&mut self, bench: IVec3) {
        self.player.swing = 1.0;
        let wear = self.inv.wear[self.inv.selected];
        if self.is_client() {
            if !self.creative {
                self.inv.consume_held();
            }
            self.net_send_msg(Msg::Interact { x: bench.x, y: bench.y, z: bench.z, item: ENCRUSTED_RELIC });
            return;
        }
        if let Some(text) = self.restore(None, bench, wear) {
            if !self.creative {
                self.inv.consume_held();
            }
            self.msg(text);
        }
    }

    /// A joined player cleans an encrusted relic (whose culture the host can't see: any).
    pub fn host_restore(&mut self, from: u32, bench: IVec3, item: Id) {
        if item != ENCRUSTED_RELIC || !self.peer_take(from, ENCRUSTED_RELIC, 1) {
            return;
        }
        let w = tag(0, self.rng.int(0, 3) as u8);
        if let Some(text) = self.restore(Some(from), bench, w) {
            self.system_message(Some(from), &text);
        }
    }

    fn fling_worn(&mut self, from: Vec3, to: Vec3, item: Id, wear: Wear) {
        const FLIGHT: f32 = 0.6;
        let vel = (to - from) / FLIGHT + Vec3::Y * (0.5 * crate::entity::GRAVITY * FLIGHT);
        self.spawn_drop(from, item, 1, wear, vel, 0.0);
    }

    /// A pot broke: it gives back its shard (and a brick).
    pub fn pot_broken(&mut self, pos: IVec3, old: Id) {
        if !is_pot(old) || self.creative || self.is_client() {
            return;
        }
        let at = pos.as_vec3() + Vec3::splat(0.5);
        if old > POT_FIRST {
            self.pop_drop(at, SHARD_FIRST + (old - POT_FIRST - 1), 1);
        }
        let bricks = 1 + self.rng.int(0, 1) as u8;
        self.pop_drop(at, BRICK, bricks);
    }
}

/// "north-east" and so on (north is -z).
pub fn compass_word(d: Vec3) -> &'static str {
    let a = d.x.atan2(-d.z).to_degrees().rem_euclid(360.0);
    ["north", "north-east", "east", "south-east", "south", "south-west", "west", "north-west"][((a + 22.5) / 45.0) as usize % 8]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn careful_brushing_beats_scrubbing() {
        let mut rng = Rng::new(4);
        // Holding on the whole time: it cracks (probably more than once).
        let mut d = Dig::new(IVec3::ZERO, &mut rng);
        let mut cracks = 0;
        let mut result = None;
        for _ in 0..2000 {
            for e in d.update(0.02, true, 1.0, 0, &mut rng) {
                match e {
                    DigEvent::Crack => cracks += 1,
                    DigEvent::Done(c) => result = Some(c),
                    DigEvent::Shattered => result = Some(4),
                    _ => {}
                }
            }
            if result.is_some() {
                break;
            }
        }
        assert!(cracks >= 1, "scrubbing cracks it");
        // Brushing in bursts, easing off when the pressure is high: pristine.
        let mut d = Dig::new(IVec3::ZERO, &mut rng);
        let mut done = None;
        for _ in 0..5000 {
            let brushing = d.pressure < 0.45;
            for e in d.update(0.02, brushing, 0.65, 5, &mut rng) {
                if let DigEvent::Done(c) = e {
                    done = Some(c);
                }
            }
            if done.is_some() {
                break;
            }
        }
        assert_eq!(done, Some(0));
    }

    #[test]
    fn deeper_is_rarer() {
        let mut rng = Rng::new(7);
        let count = |layer: u8, rng: &mut Rng| (0..2000).filter(|_| is_relic(roll_find(Culture::Sunken, layer, 0, 0, rng).item)).count();
        let (recent, old, ancient) = (count(0, &mut rng), count(1, &mut rng), count(2, &mut rng));
        assert_eq!(recent, 0, "no relics near the surface");
        assert!(ancient > old && old > 200, "{old} {ancient}");
        // Finds belong to their culture.
        for _ in 0..200 {
            let f = roll_find(Culture::Drowned, 2, 1, 0, &mut rng);
            if is_relic(f.item) {
                assert_eq!(RELICS[(f.item - RELIC_FIRST) as usize].2, Culture::Drowned);
                assert_eq!(condition_of(f.wear), Some(1));
            }
            if is_shard(f.item) {
                assert!(Culture::Drowned.shards().contains(&((f.item - SHARD_FIRST) as usize)));
            }
        }
        assert_eq!(layer(1), 0);
        assert_eq!(layer(4), 1);
        assert_eq!(layer(9), 2);
    }

    #[test]
    fn the_journal_keeps_count() {
        let mut j = Journal::default();
        assert!(j.note(SHARD_FIRST, 0));
        assert!(!j.note(SHARD_FIRST, 2));
        assert!(j.note(RELIC_FIRST + 1, 2));
        j.note(RELIC_FIRST + 1, 0);
        assert_eq!(j.has(RELIC_FIRST + 1), Some((2, 0)), "the best condition is kept");
        for &i in Culture::Hushed.shards() {
            j.note(SHARD_FIRST + i as Id, 1);
        }
        for i in Culture::Hushed.relics() {
            j.note(RELIC_FIRST + i as Id, 1);
        }
        assert_eq!(j.progress(Culture::Hushed), (4, 4));
        assert_eq!(j.progress(Culture::Sunken).1, 7);
        j.xp = 90;
        j.digs = 5;
        assert_eq!(Journal::decode(&j.encode()), j);
        assert_eq!(j.level(), 3);
        assert_eq!(condition_of(tag(2, 9)), Some(2));
        assert_eq!(extra_of(tag(2, 9)), 9);
        assert_eq!(compass_word(Vec3::new(1.0, 0.0, -1.0)), "north-east");
    }
}
