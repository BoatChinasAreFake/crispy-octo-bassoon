//! Beekeeping, which like fishing and farming has rather more to it than
//! "put box near flowers".
//!
//! Wild **Bee Nests** hang in trees in flowery places. Every nest and every
//! crafted **Beehive** with bees in it keeps its own colony record (where the
//! world lives), and each second a colony's foraging is the product of:
//!
//! - **population**: up to twelve bees, and more bees bring in more;
//! - **health**: a colony crawling with mites limps along;
//! - **daylight**: bees sleep at night and dawdle at dawn and dusk;
//! - **weather**: nobody flies in the rain (and thunder makes them cross);
//! - **flowers** within seven blocks: the more the better, up to a point,
//!   and **variety** is a bonus: bees like a mixed diet;
//! - **climate**: jungle bees thrive, desert and snowy bees struggle;
//! - **privacy**: unlike crops, bees work harder when nobody's watching;
//! - **the queen**: Busy queens forage more, Gentle ones are calmer, Hardy
//!   ones shrug off mites.
//!
//! What the bees bring back becomes honey of the flowers they visited:
//! Wildflower (poppies), Sunny (dandelions), Blue (cornflowers), Lavender, or,
//! from Torchflowers grown from seeds dug up at old ruins, Ancient honey. Each
//! bottle tastes different and does something different. Honey also becomes
//! wax (**honeycomb**), feeds new bees, and keeps the colony alive through the
//! night; a colony that runs out starves. **Mites** creep in over time
//! (faster when crowded or damp) and spread between hives that are close
//! together; dust a hive with Wood Ash to knock them back. A colony that's
//! full and well fed **swarms**: half the bees leave with the old queen, and
//! they'll move into an empty hive within sixteen blocks if there is one, or
//! fly off forever if not.
//!
//! Bees don't like being robbed. Use a **Bee Smoker** first and they stay
//! calm for a while; otherwise their temper rises, and a cross colony sends
//! out bees that sting (and then, as bees do, expire). Breaking a calm nest
//! catches its **queen** in a jar; put her in an empty hive to start your own
//! colony. A **Hive Tool** tells you everything, in one long sentence (more
//! precisely the better a beekeeper you are). Every harvest earns Beekeeper
//! experience, and the Beekeeping Log keeps count.
//!
//! Bees also **pollinate**: crops within ten blocks of a working hive grow a
//! quarter faster (see farming.rs).

use crate::block::*;
use crate::noise::Rng;
use crate::world::Biome;
use macroquad::math::{ivec3, IVec3, Vec3};
use std::collections::HashMap;

pub const MAX_BEES: u8 = 12;
/// How far bees look for flowers.
pub const FORAGE_RADIUS: i32 = 7;
/// Crops this close to a working hive get pollinated.
pub const POLLINATE_RADIUS: f32 = 10.0;
/// How far a swarm looks for a new home.
pub const SWARM_RADIUS: i32 = 16;
/// Mites hop between hives this close together.
pub const MITE_RADIUS: f32 = 8.0;
pub const HONEY_PER_BOTTLE: f32 = 20.0;
pub const COMB_PER_HARVEST: f32 = 30.0;
/// Honey a second at perfect conditions (so a full hive takes about seven minutes).
const HONEY_RATE: f32 = 0.25;
/// Seconds of smoke from one puff of the smoker.
pub const SMOKE_SECS: f32 = 45.0;
/// Temper at which the colony starts sending out stingers.
pub const STING_TEMPER: f32 = 50.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Flavour {
    Wildflower,
    Sunny,
    Blue,
    Lavender,
    Ancient,
}

impl Flavour {
    pub const ALL: [Flavour; 5] = [Flavour::Wildflower, Flavour::Sunny, Flavour::Blue, Flavour::Lavender, Flavour::Ancient];

    pub fn index(self) -> usize {
        Flavour::ALL.iter().position(|f| *f == self).unwrap_or(0)
    }
    pub fn key(self) -> &'static str {
        match self {
            Flavour::Wildflower => "wildflower",
            Flavour::Sunny => "sunny",
            Flavour::Blue => "blue",
            Flavour::Lavender => "lavender",
            Flavour::Ancient => "ancient",
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Flavour::Wildflower => "Wildflower",
            Flavour::Sunny => "Sunny",
            Flavour::Blue => "Blue",
            Flavour::Lavender => "Lavender",
            Flavour::Ancient => "Ancient",
        }
    }
    pub fn colour(self) -> [u8; 3] {
        match self {
            Flavour::Wildflower => [235, 150, 35],
            Flavour::Sunny => [250, 210, 50],
            Flavour::Blue => [90, 140, 230],
            Flavour::Lavender => [180, 130, 220],
            Flavour::Ancient => [250, 110, 40],
        }
    }
    /// The flower it comes from.
    pub fn flower(self) -> Id {
        match self {
            Flavour::Wildflower => FLOWER,
            Flavour::Sunny => DANDELION,
            Flavour::Blue => CORNFLOWER,
            Flavour::Lavender => LAVENDER,
            Flavour::Ancient => TORCHFLOWER,
        }
    }
    pub fn of_flower(id: Id) -> Option<Flavour> {
        Flavour::ALL.into_iter().find(|f| f.flower() == id)
    }
    pub fn item(self) -> Id {
        HONEY_FIRST + self.index() as Id
    }
    pub fn of_item(id: Id) -> Option<Flavour> {
        (HONEY_FIRST..HONEY_FIRST + 5).contains(&id).then(|| Flavour::ALL[(id - HONEY_FIRST) as usize])
    }
    /// What it's like to eat.
    pub fn taste(self) -> &'static str {
        match self {
            Flavour::Wildflower => "Wildflower honey: sweet, floral, and a little healing.",
            Flavour::Sunny => "Sunny honey: thick and golden. You won't be hungry for ages.",
            Flavour::Blue => "Blue honey: tastes like a cornflower looks. You feel zippy.",
            Flavour::Lavender => "Lavender honey: so calming you feel lighter on your feet.",
            Flavour::Ancient => "Ancient honey: tastes of old stone and starlight. Your eyes tingle.",
        }
    }
}

/// A queen's temperament, carried in her jar and passed to her colony.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Queen {
    Plain,
    /// Calmer: temper rises half as fast.
    Gentle,
    /// Forages a fifth more.
    Busy,
    /// Mites grow half as fast.
    Hardy,
}

impl Queen {
    pub const ALL: [Queen; 4] = [Queen::Plain, Queen::Gentle, Queen::Busy, Queen::Hardy];

    pub fn name(self) -> &'static str {
        match self {
            Queen::Plain => "Plain",
            Queen::Gentle => "Gentle",
            Queen::Busy => "Busy",
            Queen::Hardy => "Hardy",
        }
    }
    pub fn index(self) -> u8 {
        Queen::ALL.iter().position(|q| *q == self).unwrap_or(0) as u8
    }
    pub fn from_index(i: u8) -> Queen {
        Queen::ALL.get(i as usize).copied().unwrap_or(Queen::Plain)
    }
    pub fn random(rng: &mut Rng) -> Queen {
        Queen::ALL[rng.int(0, 3) as usize]
    }
    /// In a Queen Bee item's wear (the high half, like other labelled items).
    pub fn wear(self) -> crate::inventory::Wear {
        ((self.index() as u32) + 1) << 16
    }
    pub fn of_wear(w: crate::inventory::Wear) -> Queen {
        Queen::from_index(((w >> 16) as u8).saturating_sub(1))
    }
}

/// Is it a nest or hive (any state)?
pub fn is_hive(id: Id) -> bool {
    matches!(id, BEE_NEST | BEE_NEST_HONEY | BEEHIVE | BEEHIVE_BUSY | BEEHIVE_HONEY)
}

pub fn is_nest(id: Id) -> bool {
    matches!(id, BEE_NEST | BEE_NEST_HONEY)
}

/// Any flower bees visit.
pub fn is_flower(id: Id) -> bool {
    Flavour::of_flower(id).is_some()
}

/// One colony: everything a nest or hive remembers.
#[derive(Clone, Debug, PartialEq)]
pub struct Colony {
    pub bees: u8,
    /// Stored honey, 0..100, split by flavour in `nectar`.
    pub honey: f32,
    pub nectar: [f32; 5],
    /// Wax built, 0..100 (harvested as honeycomb).
    pub comb: f32,
    /// 0..100.
    pub health: f32,
    /// 0..100: mite load.
    pub mites: f32,
    /// 0..100: how cross they are.
    pub temper: f32,
    /// Seconds of smoke left.
    pub calm: f32,
    /// Progress toward the next bee (0..1).
    pub brood: f32,
    /// Seconds spent full and well stocked (they swarm at `SWARM_AFTER`).
    pub crowded: f32,
    /// Seconds without honey.
    pub starving: f32,
    pub queen: Queen,
    pub wild: bool,
    /// Flowers of each kind nearby (surveyed now and then), and seconds to the next survey.
    pub flowers: [u16; 5],
    pub survey: f32,
}

const SWARM_AFTER: f32 = 120.0;

impl Default for Colony {
    fn default() -> Colony {
        Colony { bees: 0, honey: 0.0, nectar: [0.0; 5], comb: 0.0, health: 100.0, mites: 0.0, temper: 0.0, calm: 0.0, brood: 0.0, crowded: 0.0, starving: 0.0, queen: Queen::Plain, wild: false, flowers: [0; 5], survey: 0.0 }
    }
}

/// What the colony's surroundings are like right now.
#[derive(Clone, Copy, Debug)]
pub struct Conditions {
    /// 0..1.
    pub daylight: f32,
    pub raining: bool,
    pub thunder: bool,
    pub biome: Biome,
    /// A player within a few blocks.
    pub watched: bool,
}

/// The factors behind a colony's foraging (all multiply).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Forage {
    pub population: f32,
    pub health: f32,
    pub daylight: f32,
    pub weather: f32,
    pub flowers: f32,
    pub variety: f32,
    pub climate: f32,
    pub privacy: f32,
    pub queen: f32,
}

impl Forage {
    pub fn total(&self) -> f32 {
        self.population * self.health * self.daylight * self.weather * self.flowers * self.variety * self.climate * self.privacy * self.queen
    }
}

/// Things that happened to a colony this tick.
#[derive(Debug, PartialEq)]
pub enum HiveEvent {
    /// Half the colony left: (bees, queen).
    Swarm(u8, Queen),
    /// A bee starved.
    Starved,
    NewBee,
}

pub fn climate(b: Biome) -> f32 {
    match b {
        Biome::Jungle => 1.15,
        Biome::Plains | Biome::Forest => 1.0,
        // Bees adore cherry blossom.
        Biome::Cherry => 1.2,
        Biome::Mangrove => 0.95,
        // Nothing much flowers in the pale forest.
        Biome::PaleGarden => 0.5,
        Biome::Swamp => 0.9,
        Biome::Taiga => 0.7,
        Biome::Desert | Biome::Badlands => 0.6,
        Biome::Snowy => 0.45,
        Biome::Ocean => 0.8,
    }
}

/// Bonus for a mixed diet: one kind of flower is fine, five is a feast.
pub fn variety_bonus(kinds: usize) -> f32 {
    [0.0, 1.0, 1.15, 1.3, 1.45, 1.6][kinds.min(5)]
}

impl Colony {
    /// A wild colony, found in a nest.
    pub fn wild(rng: &mut Rng) -> Colony {
        let honey = rng.range(20.0, 50.0);
        let mut c = Colony { bees: rng.int(5, 9) as u8, honey, comb: rng.range(20.0, 60.0), queen: Queen::random(rng), wild: true, mites: rng.range(0.0, 15.0), ..Colony::default() };
        c.nectar[0] = honey;
        c
    }

    /// A new colony started by a queen in an empty hive.
    pub fn founded(queen: Queen) -> Colony {
        let mut c = Colony { bees: 3, honey: 10.0, queen, ..Colony::default() };
        c.nectar[0] = 10.0;
        c
    }

    pub fn kinds(&self) -> usize {
        self.flowers.iter().filter(|&&n| n > 0).count()
    }

    pub fn forage(&self, c: &Conditions) -> Forage {
        let day = if c.daylight > 0.75 {
            1.0
        } else if c.daylight > 0.4 {
            0.5
        } else {
            0.0
        };
        let flowers: u32 = self.flowers.iter().map(|&n| n as u32).sum();
        Forage {
            population: self.bees as f32 / MAX_BEES as f32,
            health: (self.health / 100.0).clamp(0.0, 1.0),
            daylight: day,
            weather: if c.raining || c.thunder { 0.0 } else { 1.0 },
            flowers: (flowers as f32 / 12.0).min(1.0),
            variety: variety_bonus(self.kinds()).max(if flowers == 0 { 0.0 } else { 1.0 }),
            climate: climate(c.biome),
            privacy: if c.watched { 0.9 } else { 1.0 },
            queen: if self.queen == Queen::Busy { 1.2 } else { 1.0 },
        }
    }

    /// A second (or `dt`) of colony life.
    pub fn tick(&mut self, c: &Conditions, dt: f32, rng: &mut Rng) -> Vec<HiveEvent> {
        let mut ev = Vec::new();
        self.calm = (self.calm - dt).max(0.0);
        if self.bees == 0 {
            self.temper = 0.0;
            return ev;
        }
        let f = self.forage(c);
        // Foraging: honey of the flowers they found, in proportion.
        let gain = f.total() * HONEY_RATE * dt;
        let total: f32 = self.flowers.iter().map(|&n| n as f32).sum();
        if gain > 0.0 && total > 0.0 && self.honey < 100.0 {
            let room = 100.0 - self.honey;
            let gain = gain.min(room);
            for (i, &n) in self.flowers.iter().enumerate() {
                self.nectar[i] += gain * n as f32 / total;
            }
            self.honey += gain;
        }
        // Everyone eats, day and night.
        let eat = 0.004 * self.bees as f32 * dt;
        if self.honey >= eat {
            self.take_nectar(eat);
            self.starving = 0.0;
        } else {
            self.take_nectar(self.honey);
            self.starving += dt;
            if self.starving > 60.0 && !(self.wild && self.bees <= 2) {
                self.starving = 0.0;
                self.bees -= 1;
                ev.push(HiveEvent::Starved);
            }
        }
        // Spare honey becomes wax.
        if self.honey > 30.0 && self.comb < 100.0 {
            let wax = (0.003 * self.bees as f32 * dt).min(100.0 - self.comb);
            self.comb += wax;
            self.take_nectar(wax * 0.5);
        }
        // New bees, in good times.
        if self.honey > 20.0 && self.health > 50.0 && f.daylight > 0.0 && self.bees < MAX_BEES {
            self.brood += dt / 90.0;
            if self.brood >= 1.0 {
                self.brood = 0.0;
                self.bees += 1;
                ev.push(HiveEvent::NewBee);
            }
        }
        // Mites: worse when crowded or damp, and they wear the colony down.
        let hardy = if self.queen == Queen::Hardy { 0.5 } else { 1.0 };
        let damp = if c.raining { 1.5 } else { 1.0 };
        self.mites = (self.mites + 0.015 * (1.0 + self.bees as f32 / MAX_BEES as f32) * damp * hardy * dt).min(100.0);
        if self.mites > 40.0 {
            self.health = (self.health - (self.mites - 40.0) * 0.003 * dt).max(0.0);
        } else if self.mites < 20.0 && self.honey > 10.0 {
            self.health = (self.health + 0.05 * dt).min(100.0);
        }
        if self.health <= 0.0 && self.bees > 0 {
            // Too sick: a bee dies now and then until it recovers.
            self.health = 10.0;
            self.bees -= 1;
            ev.push(HiveEvent::Starved);
        }
        // Temper: storms, mites and hunger make them cross; smoke and time calm them.
        let gentle = if self.queen == Queen::Gentle { 0.5 } else { 1.0 };
        let mut rise = 0.0;
        if c.thunder {
            rise += 0.3;
        }
        if self.mites > 60.0 {
            rise += 0.1;
        }
        if self.starving > 0.0 {
            rise += 0.1;
        }
        self.temper = (self.temper + rise * gentle * dt).min(100.0);
        self.temper = (self.temper - if self.calm > 0.0 { 5.0 } else { 0.4 } * dt).max(0.0);
        // Swarming: full and well stocked for a while.
        if self.bees >= MAX_BEES && self.honey > 60.0 {
            self.crowded += dt;
            if self.crowded >= SWARM_AFTER && f.daylight > 0.0 && f.weather > 0.0 {
                self.crowded = 0.0;
                let leaving = self.bees / 2;
                self.bees -= leaving;
                let old = self.queen;
                // A daughter takes over (and might be different).
                self.queen = if rng.chance(0.6) { old } else { Queen::random(rng) };
                // The swarm takes a good share of the honey with it.
                self.take_nectar(self.honey * 0.3);
                ev.push(HiveEvent::Swarm(leaving, old));
            }
        } else {
            self.crowded = (self.crowded - dt).max(0.0);
        }
        ev
    }

    /// Use up `amount` of honey, evenly across the flavours.
    fn take_nectar(&mut self, amount: f32) {
        if self.honey <= 0.0 {
            return;
        }
        let k = (1.0 - amount / self.honey).clamp(0.0, 1.0);
        for n in self.nectar.iter_mut() {
            *n *= k;
        }
        self.honey = (self.honey - amount).max(0.0);
    }

    /// Fill a bottle: the flavour is picked by how much of each is stored.
    pub fn take_honey(&mut self, rng: &mut Rng) -> Option<Flavour> {
        if self.honey < HONEY_PER_BOTTLE {
            return None;
        }
        let total: f32 = self.nectar.iter().sum();
        let mut roll = rng.f32() * total.max(1e-3);
        let mut pick = Flavour::Wildflower;
        for (i, &n) in self.nectar.iter().enumerate() {
            if roll < n {
                pick = Flavour::ALL[i];
                break;
            }
            roll -= n;
        }
        let i = pick.index();
        let from_pick = self.nectar[i].min(HONEY_PER_BOTTLE);
        self.nectar[i] -= from_pick;
        self.honey -= from_pick;
        if from_pick < HONEY_PER_BOTTLE {
            self.take_nectar(HONEY_PER_BOTTLE - from_pick);
        }
        Some(pick)
    }

    pub fn take_comb(&mut self) -> bool {
        if self.comb < COMB_PER_HARVEST {
            return false;
        }
        self.comb -= COMB_PER_HARVEST;
        true
    }

    /// Robbed without smoke: they're not pleased.
    pub fn provoke(&mut self, amount: f32) {
        if self.calm <= 0.0 {
            let gentle = if self.queen == Queen::Gentle { 0.5 } else { 1.0 };
            self.temper = (self.temper + amount * gentle).min(100.0);
        }
    }

    pub fn smoke(&mut self) {
        self.calm = SMOKE_SECS;
    }

    /// Wood Ash: knocks the mites back.
    pub fn dust(&mut self) {
        self.mites = (self.mites - 40.0).max(0.0);
    }

    /// Which block the nest or hive should look like.
    pub fn block(&self, nest: bool) -> Id {
        let full = self.honey >= HONEY_PER_BOTTLE;
        match (nest, self.bees > 0, full) {
            (true, _, true) => BEE_NEST_HONEY,
            (true, _, false) => BEE_NEST,
            (false, false, _) => BEEHIVE,
            (false, true, true) => BEEHIVE_HONEY,
            (false, true, false) => BEEHIVE_BUSY,
        }
    }

    /// The Hive Tool's one very long sentence. Better beekeepers read more into it.
    pub fn report(&self, c: &Conditions, level: u32) -> String {
        if self.bees == 0 {
            return "This hive is empty: put a Queen Bee in it, or leave it near a full colony and hope it swarms your way.".into();
        }
        let f = self.forage(c);
        fn vague(v: f32, words: [&'static str; 4]) -> &'static str {
            if v < 15.0 {
                words[0]
            } else if v < 40.0 {
                words[1]
            } else if v < 70.0 {
                words[2]
            } else {
                words[3]
            }
        }
        let exact = level >= 3;
        let mites = if exact { format!("mites at {:.0}%", self.mites) } else { vague(self.mites, ["hardly any mites", "a few mites", "plenty of mites", "an alarming number of mites"]).to_string() };
        let temper = if self.calm > 0.0 {
            "calm under the smoke".to_string()
        } else if exact {
            format!("temper {:.0}%", self.temper)
        } else {
            vague(self.temper, ["perfectly content", "a bit grumpy", "properly cross", "furious"]).to_string()
        };
        let flowers: Vec<String> = Flavour::ALL.iter().zip(self.flowers).filter(|(_, n)| *n > 0).map(|(fl, n)| format!("{n} {}", item_name(fl.flower()).split(" (").next().unwrap_or(""))).collect();
        let flowers = if flowers.is_empty() { "no flowers in reach (they need some within seven blocks)".to_string() } else { format!("{} nearby", flowers.join(", ")) };
        let mood = if f.daylight == 0.0 {
            "asleep for the night"
        } else if f.weather == 0.0 {
            "sheltering from the rain"
        } else if f.total() > 0.6 {
            "working flat out"
        } else if f.total() > 0.25 {
            "pottering about"
        } else {
            "barely bothering"
        };
        let mut s = format!(
            "A {} queen and {} bee{}, {mood}, with {:.0}% honey and {:.0}% comb, {mites}, health {:.0}%, {temper}, {flowers}",
            self.queen.name().to_lowercase(),
            self.bees,
            if self.bees == 1 { "" } else { "s" },
            self.honey,
            self.comb,
            self.health
        );
        if level >= 5 {
            s += &format!(", foraging at x{:.2} (variety x{:.2}, climate x{:.2})", f.total(), f.variety, f.climate);
        }
        if self.bees >= MAX_BEES && self.honey > 60.0 {
            s += ", and they look ready to swarm: have an empty hive nearby";
        }
        s.push('.');
        s
    }
}

/// Everything you've done as a beekeeper, per world.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BeeLog {
    pub xp: u32,
    /// Bottles of each flavour.
    pub bottles: [u32; 5],
    pub comb: u32,
    pub stings: u32,
    pub swarms_caught: u32,
    pub swarms_lost: u32,
    pub queens: u32,
    pub colonies_started: u32,
}

impl BeeLog {
    /// Beekeeper level 0..=10.
    pub fn level(&self) -> u32 {
        ((self.xp as f32 / 10.0).sqrt() as u32).min(10)
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut v = Vec::new();
        for n in [self.xp, self.comb, self.stings, self.swarms_caught, self.swarms_lost, self.queens, self.colonies_started].iter().chain(self.bottles.iter()) {
            v.extend_from_slice(&n.to_le_bytes());
        }
        v
    }

    pub fn decode(b: &[u8]) -> BeeLog {
        let n: Vec<u32> = b.chunks_exact(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
        let g = |i: usize| n.get(i).copied().unwrap_or(0);
        BeeLog { xp: g(0).min(1_000_000), comb: g(1), stings: g(2), swarms_caught: g(3), swarms_lost: g(4), queens: g(5), colonies_started: g(6), bottles: [g(7), g(8), g(9), g(10), g(11)] }
    }
}

pub fn encode(hives: &HashMap<IVec3, Colony>) -> Vec<u8> {
    let mut v = Vec::new();
    for (p, c) in hives {
        for n in [p.x, p.y, p.z] {
            v.extend_from_slice(&n.to_le_bytes());
        }
        v.push(c.bees);
        v.push(c.queen.index());
        v.push(c.wild as u8);
        for f in [c.honey, c.comb, c.health, c.mites, c.temper, c.brood, c.crowded].iter().chain(c.nectar.iter()) {
            v.extend_from_slice(&f.to_le_bytes());
        }
    }
    v
}

pub fn decode(b: &[u8]) -> HashMap<IVec3, Colony> {
    const SIZE: usize = 12 + 3 + 12 * 4;
    let mut out = HashMap::new();
    for rec in b.chunks_exact(SIZE) {
        let i = |o: usize| i32::from_le_bytes(rec[o..o + 4].try_into().unwrap());
        let f = |k: usize| {
            let o = 15 + k * 4;
            let v = f32::from_le_bytes(rec[o..o + 4].try_into().unwrap());
            if v.is_finite() { v.clamp(0.0, 100.0) } else { 0.0 }
        };
        let c = Colony {
            bees: rec[12].min(MAX_BEES),
            queen: Queen::from_index(rec[13]),
            wild: rec[14] != 0,
            honey: f(0),
            comb: f(1),
            health: f(2),
            mites: f(3),
            temper: f(4),
            brood: f(5).min(1.0),
            crowded: f(6),
            nectar: [f(7), f(8), f(9), f(10), f(11)],
            ..Colony::default()
        };
        out.insert(ivec3(i(0), i(4), i(8)), c);
    }
    out
}

/// A bee buzzing about, purely for show (each machine makes its own).
#[derive(Clone, Debug)]
pub struct Buzz {
    pub pos: Vec3,
    pub home: IVec3,
    pub target: Vec3,
    /// Seconds before picking somewhere else to go.
    pub t: f32,
    pub yaw: f32,
    pub phase: f32,
}

// ------------------------------------------------------------------ in the game

use crate::entity::MobKind;
use crate::game::Game;
use crate::sound::{Mat, Sfx};

impl Game {
    /// The conditions a hive at `p` is living in.
    fn hive_conditions(&self, p: IVec3, players: &[Vec3]) -> Conditions {
        let (_, biome) = self.world.generator.column(p.x, p.z);
        let w = self.weather.kind;
        Conditions {
            daylight: if self.is_night() { 0.0 } else { self.daylight() },
            raining: w.wet() && !crate::scorch::in_scorch(p.x as f32),
            thunder: w == crate::weather::Weather::Thunder,
            biome,
            watched: players.iter().any(|q| q.distance(p.as_vec3()) < 6.0),
        }
    }

    /// Count the flowers around a hive.
    fn survey_flowers(&self, p: IVec3) -> [u16; 5] {
        let mut n = [0u16; 5];
        let r = FORAGE_RADIUS;
        for dy in -3..=3 {
            for dz in -r..=r {
                for dx in -r..=r {
                    if let Some(f) = Flavour::of_flower(self.world.get_v(p + ivec3(dx, dy, dz))) {
                        n[f.index()] += 1;
                    }
                }
            }
        }
        n
    }

    /// The hive at `p`, made up on the spot if it's a nest nobody's looked at yet.
    pub fn colony_at(&mut self, p: IVec3) -> Option<&mut Colony> {
        let id = self.world.get_v(p);
        if !is_hive(id) {
            return None;
        }
        if !self.hives.contains_key(&p) {
            let c = if is_nest(id) { Colony::wild(&mut self.rng) } else { Colony::default() };
            self.hives.insert(p, c);
        }
        self.hives.get_mut(&p)
    }

    /// Once a second, where the world lives: every loaded colony forages,
    /// eats, grows, gets mites, swarms and maybe stings.
    pub fn hive_tick(&mut self, dt: f32) {
        if self.is_client() || self.menu {
            return;
        }
        self.hive_timer += dt;
        if self.hive_timer < 1.0 {
            return;
        }
        let step = std::mem::take(&mut self.hive_timer);
        let players: Vec<Vec3> = self.player_targets().into_iter().map(|t| t.1).collect();
        // Hives whose block is gone lose their record.
        self.hives.retain(|p, _| !self.world.is_loaded(p.x, p.z) || is_hive(self.world.get_v(*p)));
        let keys: Vec<IVec3> = self.hives.keys().copied().filter(|p| self.world.is_loaded(p.x, p.z)).collect();
        // Mites spread from badly infested hives to close neighbours.
        let infested: Vec<IVec3> = keys.iter().copied().filter(|p| self.hives[p].mites > 60.0).collect();
        let mut swarms = Vec::new();
        let mut stingers = Vec::new();
        let mut edits = Vec::new();
        for &p in &keys {
            let mut cond = self.hive_conditions(p, &players);
            let due = self.hives[&p].survey <= 0.0;
            if due {
                let fl = self.survey_flowers(p);
                let c = self.hives.get_mut(&p).unwrap();
                c.flowers = fl;
                c.survey = 10.0;
            }
            let near_infested = infested.iter().any(|q| *q != p && q.as_vec3().distance(p.as_vec3()) <= MITE_RADIUS);
            let id = self.world.get_v(p);
            let Some(c) = self.hives.get_mut(&p) else { continue };
            c.survey -= step;
            if near_infested {
                c.mites = (c.mites + 0.02 * step).min(100.0);
            }
            cond.watched &= c.bees > 0;
            for e in c.tick(&cond, step, &mut self.rng) {
                if let HiveEvent::Swarm(n, q) = e {
                    swarms.push((p, n, q));
                }
            }
            // Cross bees go for whoever's close (and not smoking them).
            if c.temper >= STING_TEMPER && c.calm <= 0.0 && c.bees > 0 && players.iter().any(|q| q.distance(p.as_vec3() + Vec3::splat(0.5)) < 6.0) {
                let n = ((c.temper - STING_TEMPER) / 15.0).ceil().clamp(1.0, 3.0) as u8;
                c.temper -= 15.0 * n as f32;
                stingers.push((p, n.min(c.bees)));
            }
            let want = c.block(is_nest(id));
            if want != id {
                edits.push((p, want));
            }
        }
        for (p, id) in edits {
            self.world.set_v(p, id);
        }
        for (p, n) in stingers {
            self.release_stingers(p, n);
        }
        for (from, n, queen) in swarms {
            self.swarm(from, n, queen, &players);
        }
    }

    /// Bees out for revenge.
    fn release_stingers(&mut self, p: IVec3, n: u8) {
        let live = self.mobs.iter().filter(|m| m.kind == MobKind::Bee && m.home == Some(p.as_vec3())).count();
        for _ in 0..(n as usize).saturating_sub(live) {
            let at = p.as_vec3() + Vec3::new(0.5, 1.2, 0.5);
            self.alloc_mob(MobKind::Bee, at);
            if let Some(m) = self.mobs.last_mut() {
                m.home = Some(p.as_vec3());
                m.angry = true;
            }
        }
        self.sfx(Sfx::Buzz, Some(p.as_vec3() + Vec3::splat(0.5)));
    }

    /// A swarm leaves `from` looking for a home.
    fn swarm(&mut self, from: IVec3, bees: u8, queen: Queen, players: &[Vec3]) {
        let r = SWARM_RADIUS;
        let mut best: Option<(i32, IVec3)> = None;
        for dy in -6..=6 {
            for dz in -r..=r {
                for dx in -r..=r {
                    let q = from + ivec3(dx, dy, dz);
                    if q == from || self.world.get_v(q) != BEEHIVE {
                        continue;
                    }
                    if self.hives.get(&q).is_some_and(|c| c.bees > 0) {
                        continue;
                    }
                    let d = dx * dx + dy * dy + dz * dz;
                    if best.is_none_or(|(b, _)| d < b) {
                        best = Some((d, q));
                    }
                }
            }
        }
        let watched = players.iter().any(|q| q.distance(from.as_vec3()) < 32.0);
        match best {
            Some((_, to)) => {
                let mut c = Colony::founded(queen);
                c.bees = bees.max(2);
                self.hives.insert(to, c);
                self.world.set_v(to, BEEHIVE_BUSY);
                if watched {
                    self.bee_log.swarms_caught += 1;
                    self.bee_xp(10);
                    self.msg("A swarm moved into your empty hive! Free bees. The best kind.");
                    self.advance("swarm_catcher");
                }
            }
            None if watched => {
                self.bee_log.swarms_lost += 1;
                self.msg("Your bees swarmed and flew off. An empty hive nearby would have caught them.");
            }
            None => {}
        }
        self.sfx(Sfx::Buzz, Some(from.as_vec3() + Vec3::splat(0.5)));
    }

    fn bee_xp(&mut self, n: u32) {
        let before = self.bee_log.level();
        self.bee_log.xp += n;
        let after = self.bee_log.level();
        if after > before {
            self.msg(format!("Beekeeper level {after}! The bees have started calling you 'the big one'."));
        }
    }

    /// Something used on a nest or hive at `pos`, by the local player (`who`
    /// None) or a joined player (where the world lives). Returns what to say,
    /// or None if it didn't apply. Items made fly out of the hive to them.
    pub fn hive_use(&mut self, who: Option<u32>, pos: IVec3, item: Id) -> Option<String> {
        let id = self.world.get_v(pos);
        if !is_hive(id) {
            return None;
        }
        let to = match who {
            Some(p) => self.peers.get(&p).map(|q| q.target + Vec3::Y * 0.9).unwrap_or(pos.as_vec3()),
            None => self.player.body.pos + Vec3::Y * 0.9,
        };
        let level = if who.is_none() { self.bee_log.level() } else { 0 };
        let players: Vec<Vec3> = self.player_targets().into_iter().map(|t| t.1).collect();
        let cond = self.hive_conditions(pos, &players);
        let survey = self.survey_flowers(pos);
        let mut rng = std::mem::replace(&mut self.rng, Rng::new(0));
        let c = self.colony_at(pos)?;
        c.flowers = survey;
        let out = match item {
            HIVE_TOOL => Some(Ok(c.report(&cond, level))),
            BEE_SMOKER => {
                c.smoke();
                Some(Ok("You puff smoke into the hive. The bees settle down for a nice sit.".to_string()))
            }
            WOOD_ASH => {
                c.dust();
                Some(Ok("You dust the hive with wood ash. The mites are having a terrible day.".to_string()))
            }
            GLASS_BOTTLE => match c.take_honey(&mut rng) {
                Some(f) => {
                    // Less likely to get stung the better you are at this.
                    c.provoke(60.0 - level as f32 * 4.0);
                    Some(Err((f.item(), 1, format!("You bottle some {} honey.", f.name()))))
                }
                None => Some(Ok("Not enough honey yet. The bees are working on it (they say).".to_string())),
            },
            SHEARS => {
                if c.take_comb() {
                    c.provoke(50.0 - level as f32 * 4.0);
                    let n = 3 + (level >= 5 && rng.chance(0.3)) as u8;
                    Some(Err((HONEYCOMB, n, "You cut out some honeycomb.".to_string())))
                } else {
                    Some(Ok("The comb isn't ready to harvest yet.".to_string()))
                }
            }
            _ => None,
        };
        let extra_bottle = matches!(out, Some(Err((it, _, _))) if Flavour::of_item(it).is_some()) && level >= 5 && rng.chance(0.15);
        self.rng = rng;
        let out = out?;
        let nest = is_nest(id);
        if let Some(c) = self.hives.get(&pos) {
            let want = c.block(nest);
            if want != id {
                self.world.set_v(pos, want);
            }
        }
        let at = pos.as_vec3() + Vec3::new(0.5, 0.6, 0.5);
        match out {
            Ok(text) => {
                if item == BEE_SMOKER {
                    self.smoke(at, 10, 0.6);
                }
                Some(text)
            }
            Err((made, n, text)) => {
                self.sfx(if item == SHEARS { Sfx::Snip } else { Sfx::Place(Mat::Glass) }, Some(at));
                self.fling_to(at, to, made, n);
                if extra_bottle {
                    self.fling_to(at, to, made, 1);
                }
                if who.is_none() {
                    if let Some(f) = Flavour::of_item(made) {
                        self.bee_log.bottles[f.index()] += 1 + extra_bottle as u32;
                        if f == Flavour::Ancient {
                            self.advance("ancient_honey");
                        }
                    } else {
                        self.bee_log.comb += n as u32;
                    }
                    self.bee_xp(2);
                    self.advance("sweet_success");
                }
                Some(text)
            }
        }
    }

    /// The local player used something on a nest or hive.
    pub fn use_on_hive(&mut self, pos: IVec3, held: Id) {
        self.player.swing = 1.0;
        let id = self.world.get_v(pos);
        // What gets used up (bottles only when there's honey to fill them).
        let consumes = match held {
            GLASS_BOTTLE => matches!(id, BEE_NEST_HONEY | BEEHIVE_HONEY),
            WOOD_ASH => true,
            QUEEN_BEE => id == BEEHIVE,
            _ => false,
        };
        let wears = matches!(held, SHEARS | BEE_SMOKER);
        if held == BEE_SMOKER {
            self.smoke(pos.as_vec3() + Vec3::new(0.5, 0.8, 0.5), 10, 0.6);
        }
        if self.is_client() {
            if consumes && !self.creative {
                self.inv.consume_held();
            }
            if wears {
                self.use_tool(1);
            }
            // The colony lives on the host: it does the rest and replies.
            self.net_send_msg(crate::net::Msg::Interact { x: pos.x, y: pos.y, z: pos.z, item: held });
            return;
        }
        if held == QUEEN_BEE {
            let wear = self.inv.wear[self.inv.selected];
            if self.place_queen(None, pos, wear) {
                if !self.creative {
                    self.inv.consume_held();
                }
                self.msg("The queen moves in. Long live the queen.");
            } else {
                self.msg("Queens need an empty Beehive (a wild nest won't do).");
            }
            return;
        }
        if let Some(text) = self.hive_use(None, pos, held) {
            if consumes && !self.creative {
                self.inv.consume_held();
            }
            if wears {
                self.use_tool(1);
            }
            self.msg(text);
        }
    }

    /// A joined player used something on a nest or hive (where the world lives).
    pub fn host_hive_use(&mut self, from: u32, pos: IVec3, item: Id) {
        let id = self.world.get_v(pos);
        match item {
            QUEEN_BEE => {
                // Their queen's temperament isn't known here: a fresh one.
                let q = Queen::random(&mut self.rng).wear();
                if self.place_queen(Some(from), pos, q) {
                    self.peer_take(from, QUEEN_BEE, 1);
                }
                return;
            }
            WOOD_ASH => {
                if !self.peer_take(from, WOOD_ASH, 1) {
                    return;
                }
            }
            GLASS_BOTTLE if !matches!(id, BEE_NEST_HONEY | BEEHIVE_HONEY) => {}
            GLASS_BOTTLE => {
                if !self.peer_take(from, GLASS_BOTTLE, 1) {
                    return;
                }
            }
            SHEARS | BEE_SMOKER => self.host_wear(from, item, 1),
            HIVE_TOOL => {}
            _ => return,
        }
        if let Some(text) = self.hive_use(Some(from), pos, item) {
            self.system_message(Some(from), &text);
        }
    }

    /// A queen goes into an empty hive.
    pub fn place_queen(&mut self, who: Option<u32>, pos: IVec3, wear: crate::inventory::Wear) -> bool {
        if self.world.get_v(pos) != BEEHIVE || self.hives.get(&pos).is_some_and(|c| c.bees > 0) {
            return false;
        }
        let queen = Queen::of_wear(wear);
        self.hives.insert(pos, Colony::founded(queen));
        self.world.set_v(pos, BEEHIVE_BUSY);
        self.sfx(Sfx::Buzz, Some(pos.as_vec3() + Vec3::splat(0.5)));
        if who.is_none() {
            self.bee_log.colonies_started += 1;
            self.bee_xp(5);
            self.advance("new_colony");
        }
        true
    }

    /// A nest or hive broke (where the world lives): calm colonies give up
    /// their queen; cross ones come after you.
    pub fn hive_broken(&mut self, pos: IVec3, old: Id) {
        let Some(c) = self.colony_at_was(pos, old) else { return };
        let at = pos.as_vec3() + Vec3::splat(0.5);
        let combs = (c.comb / COMB_PER_HARVEST).floor() as u8 + if is_nest(old) { 1 } else { 0 };
        if combs > 0 {
            self.pop_drop(at, HONEYCOMB, combs);
        }
        if c.bees > 0 {
            if c.calm > 0.0 {
                self.pop_drop_worn(at, QUEEN_BEE, 1, c.queen.wear());
                if is_nest(old) {
                    self.bee_log.queens += 1;
                }
            } else {
                // Everyone out, and they're furious.
                self.release_stingers(pos, c.bees.min(4));
            }
        }
    }

    /// The colony a now-broken block had (made up if nobody ever looked).
    fn colony_at_was(&mut self, pos: IVec3, old: Id) -> Option<Colony> {
        if !is_hive(old) {
            return None;
        }
        Some(self.hives.remove(&pos).unwrap_or_else(|| if is_nest(old) { Colony::wild(&mut self.rng) } else { Colony::default() }))
    }

    /// A catch flying out of something toward someone (fishing's arc).
    pub fn fling_to(&mut self, from: Vec3, to: Vec3, item: Id, n: u8) {
        const FLIGHT: f32 = 0.6;
        let vel = (to - from) / FLIGHT + Vec3::Y * (0.5 * crate::entity::GRAVITY * FLIGHT);
        self.spawn_drop(from, item, n, 0, vel, 0.0);
    }

    /// Is there a working colony near a crop at `p` (for pollination)?
    pub fn pollinated(&self, p: IVec3) -> bool {
        if self.weather.kind.wet() || self.is_night() {
            return false;
        }
        self.hives.iter().any(|(q, c)| c.bees > 0 && c.honey < 100.0 && q.as_vec3().distance(p.as_vec3()) <= POLLINATE_RADIUS)
    }

    /// Eat a bottle of honey: food, a flavour, and the bottle back.
    pub fn eat_honey(&mut self, held: Id) -> bool {
        let Some(f) = Flavour::of_item(held) else { return false };
        if self.player.hunger.full() && !self.creative && f != Flavour::Wildflower {
            return false;
        }
        self.player.hunger.eat(6.0, if f == Flavour::Sunny { 1.2 } else { 0.6 });
        self.stats.eaten += 1;
        match f {
            Flavour::Wildflower => self.player.health = (self.player.health + 4.0).min(crate::player::MAX_HEALTH),
            Flavour::Sunny => {}
            Flavour::Blue => self.timed_effect(crate::potions::Potion::Speed, 45.0),
            Flavour::Lavender => self.timed_effect(crate::potions::Potion::Leaping, 60.0),
            Flavour::Ancient => self.timed_effect(crate::potions::Potion::NightVision, 120.0),
        }
        if !self.creative {
            // Joined players' hosts hand the bottle back themselves (see `host_consume`).
            self.use_up_held();
            self.inv.add(GLASS_BOTTLE, 1);
        }
        self.sfx(Sfx::Eat, None);
        self.msg(f.taste());
        true
    }

    /// Bees buzzing between hives and flowers near the camera (for show).
    pub fn buzz_tick(&mut self, dt: f32) {
        if self.menu || self.dedicated {
            return;
        }
        let rain = self.weather.kind.wet();
        let day = !self.is_night() && self.daylight() > 0.4;
        self.buzz_scan -= dt;
        if self.buzz_scan <= 0.0 {
            self.buzz_scan = 2.0;
            let c = self.player.body.pos.floor().as_ivec3();
            let mut homes = Vec::new();
            let r = 16;
            for dy in -8..=8 {
                for dz in -r..=r {
                    for dx in -r..=r {
                        let p = c + ivec3(dx, dy, dz);
                        if matches!(self.world.get_v(p), BEE_NEST | BEE_NEST_HONEY | BEEHIVE_BUSY | BEEHIVE_HONEY) {
                            homes.push(p);
                        }
                    }
                }
            }
            self.buzz.retain(|b| homes.contains(&b.home));
            if day && !rain {
                for h in homes {
                    let have = self.buzz.iter().filter(|b| b.home == h).count();
                    for i in have..3 {
                        let pos = h.as_vec3() + Vec3::new(0.5, 0.5, 0.5);
                        self.buzz.push(Buzz { pos, home: h, target: pos, t: i as f32 * 0.7, yaw: 0.0, phase: self.rng.range(0.0, 6.0) });
                    }
                }
            }
        }
        // At night or in the rain they head home and disappear inside.
        let out = day && !rain;
        let mut flowers_cache: HashMap<IVec3, Vec<Vec3>> = HashMap::new();
        let mut buzz = std::mem::take(&mut self.buzz);
        for b in buzz.iter_mut() {
            b.t -= dt;
            b.phase += dt * 9.0;
            let home = b.home.as_vec3() + Vec3::new(0.5, 0.5, 0.5);
            if b.t <= 0.0 {
                b.t = self.rng.range(2.0, 5.0);
                let near = flowers_cache.entry(b.home).or_insert_with(|| {
                    let mut v = Vec::new();
                    for dy in -2..=2 {
                        for dz in -6..=6 {
                            for dx in -6..=6 {
                                let q = b.home + ivec3(dx, dy, dz);
                                if is_flower(self.world.get_v(q)) {
                                    v.push(q.as_vec3() + Vec3::new(0.5, 0.6, 0.5));
                                }
                            }
                        }
                    }
                    v
                });
                b.target = if out && !near.is_empty() && self.rng.chance(0.7) {
                    near[self.rng.int(0, near.len() as i32 - 1) as usize]
                } else {
                    home + Vec3::new(self.rng.range(-2.5, 2.5), self.rng.range(-0.5, 1.5), self.rng.range(-2.5, 2.5))
                };
            }
            let goal = if out { b.target } else { home };
            let d = goal - b.pos;
            let step = d.normalize_or_zero() * 2.2 * dt;
            b.pos += if step.length() > d.length() { d } else { step };
            b.pos.y += (b.phase * 0.7).sin() * 0.3 * dt;
            if d.length() > 0.05 {
                b.yaw = d.x.atan2(-d.z);
            }
        }
        buzz.retain(|b| out || b.pos.distance(b.home.as_vec3() + Vec3::splat(0.5)) > 0.3);
        self.buzz = buzz;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sunny() -> Conditions {
        Conditions { daylight: 1.0, raining: false, thunder: false, biome: Biome::Plains, watched: false }
    }

    #[test]
    fn foraging_depends_on_everything() {
        let mut c = Colony::founded(Queen::Plain);
        c.bees = 12;
        c.flowers = [12, 0, 0, 0, 0];
        let base = c.forage(&sunny()).total();
        assert!((base - 1.0).abs() < 1e-4, "{base}");
        let night = Conditions { daylight: 0.1, ..sunny() };
        assert_eq!(c.forage(&night).total(), 0.0, "bees sleep");
        let rain = Conditions { raining: true, ..sunny() };
        assert_eq!(c.forage(&rain).total(), 0.0, "and stay in when it's wet");
        let watched = Conditions { watched: true, ..sunny() };
        assert!(c.forage(&watched).total() < base, "bees like privacy");
        let snowy = Conditions { biome: Biome::Snowy, ..sunny() };
        assert!(c.forage(&snowy).total() < base * 0.5);
        c.flowers = [4, 4, 4, 0, 0];
        assert!(c.forage(&sunny()).total() > base, "variety beats a monoculture");
        c.flowers = [0; 5];
        assert_eq!(c.forage(&sunny()).total(), 0.0, "no flowers, no honey");
        c.flowers = [12, 0, 0, 0, 0];
        c.queen = Queen::Busy;
        assert!(c.forage(&sunny()).total() > base);
        c.health = 50.0;
        c.queen = Queen::Plain;
        assert!((c.forage(&sunny()).total() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn honey_flavours_follow_the_flowers() {
        let mut rng = Rng::new(5);
        let mut c = Colony::founded(Queen::Plain);
        c.bees = 12;
        c.honey = 0.0;
        c.nectar = [0.0; 5];
        c.flowers = [0, 0, 0, 12, 0];
        for _ in 0..200 {
            c.tick(&sunny(), 1.0, &mut rng);
        }
        assert!(c.honey >= HONEY_PER_BOTTLE, "{}", c.honey);
        assert_eq!(c.take_honey(&mut rng), Some(Flavour::Lavender));
        let mut empty = Colony::founded(Queen::Plain);
        empty.honey = 5.0;
        assert_eq!(empty.take_honey(&mut rng), None);
    }

    #[test]
    fn hungry_colonies_starve_and_crowded_ones_swarm() {
        let mut rng = Rng::new(1);
        let mut c = Colony::founded(Queen::Plain);
        c.bees = 8;
        c.honey = 0.0;
        c.nectar = [0.0; 5];
        let night = Conditions { daylight: 0.0, ..sunny() };
        let mut starved = 0;
        for _ in 0..200 {
            starved += c.tick(&night, 1.0, &mut rng).iter().filter(|e| **e == HiveEvent::Starved).count();
        }
        assert!(starved >= 2 && c.bees < 8);
        let mut full = Colony::founded(Queen::Gentle);
        full.bees = MAX_BEES;
        full.honey = 90.0;
        full.nectar = [90.0, 0.0, 0.0, 0.0, 0.0];
        full.flowers = [10, 0, 0, 0, 0];
        let mut swarmed = None;
        for _ in 0..200 {
            for e in full.tick(&sunny(), 1.0, &mut rng) {
                if let HiveEvent::Swarm(n, q) = e {
                    swarmed = Some((n, q));
                }
            }
        }
        assert_eq!(swarmed, Some((MAX_BEES / 2, Queen::Gentle)));
        assert!(full.bees < MAX_BEES);
    }

    #[test]
    fn mites_smoke_and_temper() {
        let mut rng = Rng::new(2);
        let mut c = Colony::founded(Queen::Plain);
        c.mites = 90.0;
        c.honey = 50.0;
        let h = c.health;
        for _ in 0..30 {
            c.tick(&sunny(), 1.0, &mut rng);
        }
        assert!(c.health < h, "mites wear the colony down");
        c.dust();
        assert!(c.mites < 60.0);
        c.provoke(80.0);
        assert!(c.temper >= 70.0);
        c.smoke();
        let before = c.temper;
        c.tick(&sunny(), 5.0, &mut rng);
        assert!(c.temper < before - 20.0, "smoke calms them fast");
        let calmed = c.temper;
        c.provoke(80.0);
        assert_eq!(c.temper, calmed, "and they don't mind being robbed while smoked");
    }

    #[test]
    fn records_save_and_load() {
        let mut rng = Rng::new(9);
        let mut m = HashMap::new();
        let mut c = Colony::wild(&mut rng);
        c.nectar = [1.0, 2.0, 3.0, 4.0, 5.0];
        m.insert(ivec3(1, -2, 3), c.clone());
        let back = decode(&encode(&m));
        let b = &back[&ivec3(1, -2, 3)];
        assert_eq!((b.bees, b.queen, b.wild, b.nectar), (c.bees, c.queen, c.wild, c.nectar));
        let log = BeeLog { xp: 40, bottles: [1, 2, 3, 4, 5], comb: 6, stings: 7, swarms_caught: 8, swarms_lost: 9, queens: 10, colonies_started: 11 };
        assert_eq!(BeeLog::decode(&log.encode()), log);
        assert_eq!(log.level(), 2);
        assert_eq!(Queen::of_wear(Queen::Hardy.wear()), Queen::Hardy);
        assert_eq!(Flavour::of_item(Flavour::Blue.item()), Some(Flavour::Blue));
    }
}
