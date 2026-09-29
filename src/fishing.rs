//! Fishing, which is harder than it looks.
//!
//! Cast a Fishing Stick and the bobber arcs out and floats. After a wait
//! (shorter at dawn and dusk, with bait, and in big deep water; much longer in
//! a puddle), the fish nibble a few times, which are *not* bites: reel in on
//! a nibble and you scare them off. Then a real bite: you have a moment to
//! reel in. Small fish come straight in; big ones start a tug-of-war where
//! holding right-click reels in but raises line tension, and too much tension
//! snaps the line. What you catch depends on where, when, how deep, your bait
//! and your Angler level (which rises with every catch). A Fishing Log keeps
//! count, and the biggest of each.
//!
//! The rod, bobber and tug-of-war run on the fisher's own machine; the catch
//! itself is rolled wherever the world lives, so joined players can't award
//! themselves a Big Bob.

use crate::block::*;
use crate::noise::Rng;
use crate::world::{Biome, World, SEA};
use macroquad::math::Vec3;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BobberState {
    Flying,
    /// In the water, waiting. `nibble` counts down to the next fake-out.
    Floating { wait: f32, nibble: f32 },
    /// A fish has it: reel in before `window` runs out.
    Biting { window: f32 },
    /// Landed on something that isn't water.
    Grounded,
}

/// A big fish on the line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fight {
    /// 0..1: how close it is to landed.
    pub progress: f32,
    /// 0..1: the line snaps at 1.
    pub tension: f32,
    surge: f32,
}

impl Fight {
    pub fn new(progress: f32, tension: f32) -> Fight {
        Fight { progress, tension, surge: 1.0 }
    }
}

pub struct Bobber {
    pub pos: Vec3,
    pub vel: Vec3,
    pub state: BobberState,
    pub fight: Option<Fight>,
    /// Seconds since a nibble (for the bob animation and "too early" messages).
    pub since_nibble: f32,
    pub bait: bool,
}

/// Something that happened this tick that the game should react to.
#[derive(Debug, PartialEq)]
pub enum FishEvent {
    Splashdown,
    Nibble,
    Bite,
    GotAway,
    Snapped,
    /// Reeled in successfully: roll a catch at this position.
    Landed(Vec3),
    TooFar,
}

impl Bobber {
    pub fn cast(from: Vec3, dir: Vec3, bait: bool) -> Bobber {
        Bobber { pos: from, vel: dir * 13.0 + Vec3::Y * 2.0, state: BobberState::Flying, fight: None, since_nibble: 9.0, bait }
    }

    /// Advance the bobber. `reeling` is whether the use button is held (for fights).
    #[allow(clippy::too_many_arguments)]
    pub fn update(&mut self, dt: f32, world: &World, angler: &FishLog, time: f32, raining: bool, rod_at: Vec3, reeling: bool, rng: &mut Rng) -> Vec<FishEvent> {
        let mut ev = Vec::new();
        self.since_nibble += dt;
        if self.pos.distance(rod_at) > 40.0 {
            ev.push(FishEvent::TooFar);
            return ev;
        }
        if let Some(f) = &mut self.fight {
            // Reeling pulls it in but strains the line; the fish surges now and then.
            let level = angler.level() as f32;
            if reeling {
                f.progress += (0.22 + level * 0.01) * dt;
                f.tension += (0.45 - level * 0.015) * dt;
            } else {
                f.progress = (f.progress - 0.06 * dt).max(0.0);
                f.tension = (f.tension - 0.7 * dt).max(0.0);
            }
            f.surge -= dt;
            if f.surge <= 0.0 {
                f.tension += rng.range(0.12, 0.3);
                f.surge = rng.range(0.6, 1.6);
            }
            if f.tension >= 1.0 {
                self.fight = None;
                ev.push(FishEvent::Snapped);
            } else if f.progress >= 1.0 {
                self.fight = None;
                ev.push(FishEvent::Landed(self.pos));
            }
            return ev;
        }
        match self.state {
            BobberState::Flying => {
                self.vel.y -= 20.0 * dt;
                let next = self.pos + self.vel * dt;
                let cell = world.get(next.x.floor() as i32, next.y.floor() as i32, next.z.floor() as i32);
                if cell == WATER {
                    self.pos = next;
                    self.vel = Vec3::ZERO;
                    self.state = BobberState::Floating { wait: bite_wait(world, self.pos, time, self.bait, angler, raining, rng), nibble: rng.range(1.0, 3.0) };
                    ev.push(FishEvent::Splashdown);
                } else if is_solid(cell) {
                    self.vel = Vec3::ZERO;
                    self.state = BobberState::Grounded;
                } else {
                    self.pos = next;
                }
            }
            BobberState::Floating { mut wait, mut nibble } => {
                // Ride the surface.
                let surface = water_surface(world, self.pos);
                self.pos.y += (surface - 0.1 - self.pos.y) * (dt * 6.0).min(1.0);
                wait -= dt;
                nibble -= dt;
                if nibble <= 0.0 && wait > 0.8 {
                    nibble = rng.range(1.0, 3.5);
                    self.since_nibble = 0.0;
                    ev.push(FishEvent::Nibble);
                }
                if wait <= 0.0 {
                    let window = 0.9 + angler.level() as f32 * 0.05;
                    self.state = BobberState::Biting { window };
                    ev.push(FishEvent::Bite);
                } else {
                    self.state = BobberState::Floating { wait, nibble };
                }
            }
            BobberState::Biting { window } => {
                let window = window - dt;
                if window <= 0.0 {
                    self.state = BobberState::Floating { wait: bite_wait(world, self.pos, time, self.bait, angler, raining, rng), nibble: rng.range(1.0, 3.0) };
                    ev.push(FishEvent::GotAway);
                } else {
                    self.state = BobberState::Biting { window };
                }
            }
            BobberState::Grounded => {}
        }
        ev
    }

    /// The player reeled in. Returns what happened (None: just reeled in an empty line).
    pub fn reel(&mut self, big_chance: f32, rng: &mut Rng) -> Option<FishEvent> {
        match self.state {
            BobberState::Biting { .. } => {
                if rng.chance(big_chance) {
                    self.fight = Some(Fight { progress: 0.15, tension: 0.2, surge: 0.8 });
                    None
                } else {
                    Some(FishEvent::Landed(self.pos))
                }
            }
            _ => None,
        }
    }

    /// Visual: the bobber dips during a bite and twitches on a nibble.
    pub fn draw_pos(&self, t: f32) -> Vec3 {
        let dip = match self.state {
            BobberState::Biting { .. } => -0.25,
            _ if self.fight.is_some() => -0.2 + (t * 20.0).sin() * 0.05,
            _ if self.since_nibble < 0.3 => -0.1,
            BobberState::Floating { .. } => (t * 2.5).sin() * 0.03,
            _ => 0.0,
        };
        self.pos + Vec3::Y * dip
    }
}

/// Top of the water column the bobber is in.
fn water_surface(world: &World, p: Vec3) -> f32 {
    let (x, z) = (p.x.floor() as i32, p.z.floor() as i32);
    let mut y = p.y.floor() as i32;
    while y < 127 && world.get(x, y + 1, z) == WATER {
        y += 1;
    }
    y as f32 + 1.0
}

/// How deep the water is below `p`, and how much open water surface is around it (0..25).
pub fn water_info(world: &World, p: Vec3) -> (i32, i32) {
    let (x, y, z) = (p.x.floor() as i32, water_surface(world, p) as i32 - 1, p.z.floor() as i32);
    let mut depth = 0;
    while depth < 12 && world.get(x, y - depth, z) == WATER {
        depth += 1;
    }
    let area = (-2..=2).flat_map(|dz| (-2..=2).map(move |dx| (dx, dz))).filter(|&(dx, dz)| world.get(x + dx, y, z + dz) == WATER).count() as i32;
    (depth, area)
}

/// Dawn and dusk are prime time; the middle of the night is slow.
pub fn time_factor(time: f32) -> f32 {
    let from_edge = |t: f32| (time - t).abs().min(1.0 - (time - t).abs());
    if from_edge(0.0) < 0.06 || from_edge(0.5) < 0.06 {
        0.6
    } else if (0.55..0.95).contains(&time) {
        1.3
    } else {
        1.0
    }
}

/// Seconds until a real bite.
fn bite_wait(world: &World, p: Vec3, time: f32, bait: bool, angler: &FishLog, raining: bool, rng: &mut Rng) -> f32 {
    let (depth, area) = water_info(world, p);
    let water = if area < 6 || depth < 2 { 3.0 } else if area >= 20 && depth >= 4 { 0.8 } else { 1.0 };
    let bait = if bait { 0.6 } else { 1.0 };
    let skill = 1.0 - angler.level() as f32 * 0.03;
    // Fish bite sooner in the rain (everyone knows this).
    let rain = if raining { 0.75 } else { 1.0 };
    rng.range(5.0, 14.0) * water * bait * skill * rain * time_factor(time)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Category {
    Fish,
    Junk,
    Treasure,
    Legendary,
}

pub struct Catch {
    pub item: Id,
    pub n: u8,
    /// Length in cm for fish.
    pub cm: Option<f32>,
    pub category: Category,
}

/// What comes up. Rolled where the world lives.
pub fn roll_catch(world: &World, p: Vec3, time: f32, level: u32, bait: bool, rng: &mut Rng) -> Catch {
    let (depth, area) = water_info(world, p);
    let puddle = area < 6 || depth < 2;
    let (_, biome) = world.generator.column(p.x.floor() as i32, p.z.floor() as i32);
    let ocean = biome == Biome::Ocean || p.y < SEA as f32 - 2.0;
    let edge = time_factor(time) < 1.0;
    let lvl = level as f32;
    let r = rng.f32();
    let legendary = if ocean && edge && depth >= 4 { 0.004 + lvl * 0.001 } else { 0.0 };
    let treasure = 0.04 + lvl * 0.006 + if bait { 0.01 } else { 0.0 };
    let junk = if puddle { 0.6 } else { 0.14 - lvl * 0.008 }.max(0.03);
    let category = if r < legendary {
        Category::Legendary
    } else if r < legendary + treasure && !puddle {
        Category::Treasure
    } else if r < legendary + treasure + junk {
        Category::Junk
    } else {
        Category::Fish
    };
    match category {
        Category::Legendary => Catch { item: BIG_BOB, n: 1, cm: Some(rng.range(120.0, 180.0)), category },
        Category::Treasure => {
            let item = [DIAMOND, GOLD_INGOT, PEARL, IRON][rng.int(0, 3) as usize];
            Catch { item, n: 1, cm: None, category }
        }
        Category::Junk => {
            let item = [BOOT, BOTTLE, STICK, STRING][rng.int(0, 3) as usize];
            Catch { item, n: 1, cm: None, category }
        }
        Category::Fish => {
            let warm = matches!(biome, Biome::Desert | Biome::Plains);
            let cold = biome == Biome::Snowy || world.get(p.x.floor() as i32, SEA, p.z.floor() as i32) == ICE;
            let f = rng.f32();
            let (item, min, max) = if f < 0.08 {
                (PUFFER, 15.0, 35.0)
            } else if warm && f < 0.3 {
                (TROPICAL, 8.0, 25.0)
            } else if (cold && f < 0.65) || f < 0.4 {
                (SALMON, 40.0, 90.0)
            } else {
                (COD, 30.0, 80.0)
            };
            // Deeper water and skill mean bigger fish.
            let size = rng.f32().powf(1.6 - (depth.min(8) as f32 * 0.05 + lvl * 0.03).min(0.8));
            Catch { item, n: 1, cm: Some(min + (max - min) * size), category }
        }
    }
}

/// Chance a bite turns into a tug-of-war.
pub fn big_chance(level: u32) -> f32 {
    0.3 + level as f32 * 0.01
}

/// Everything you've caught, per world.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FishLog {
    /// Item key -> (count, biggest in cm).
    pub species: BTreeMap<String, (u32, f32)>,
    pub xp: u32,
    pub snapped: u32,
}

impl FishLog {
    /// Angler level 0..=10.
    pub fn level(&self) -> u32 {
        ((self.xp as f32 / 10.0).sqrt() as u32).min(10)
    }

    /// Record a catch. Returns true if it's a new personal best.
    pub fn record(&mut self, item: Id, cm: Option<f32>, category: Category) -> bool {
        self.xp += match category {
            Category::Fish => 3,
            Category::Junk => 1,
            Category::Treasure => 5,
            Category::Legendary => 50,
        };
        let e = self.species.entry(reg().key_of(item).to_string()).or_insert((0, 0.0));
        e.0 += 1;
        match cm {
            Some(cm) if cm > e.1 => {
                let first = e.1 == 0.0;
                e.1 = cm;
                !first
            }
            _ => false,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.xp.to_le_bytes());
        out.extend_from_slice(&self.snapped.to_le_bytes());
        for (k, (n, cm)) in &self.species {
            out.push(k.len().min(255) as u8);
            out.extend_from_slice(&k.as_bytes()[..k.len().min(255)]);
            out.extend_from_slice(&n.to_le_bytes());
            out.extend_from_slice(&cm.to_le_bytes());
        }
        out
    }

    pub fn decode(b: &[u8]) -> FishLog {
        let mut log = FishLog::default();
        if b.len() < 8 {
            return log;
        }
        log.xp = u32::from_le_bytes(b[0..4].try_into().unwrap()).min(1_000_000);
        log.snapped = u32::from_le_bytes(b[4..8].try_into().unwrap());
        let mut i = 8;
        while i < b.len() {
            let len = b[i] as usize;
            if i + 1 + len + 8 > b.len() {
                break;
            }
            let key = String::from_utf8_lossy(&b[i + 1..i + 1 + len]).into_owned();
            let o = i + 1 + len;
            let n = u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
            let cm = f32::from_le_bytes(b[o + 4..o + 8].try_into().unwrap());
            log.species.insert(key, (n, if cm.is_finite() { cm.clamp(0.0, 1000.0) } else { 0.0 }));
            i = o + 8;
        }
        log
    }
}

/// A few hundred years of maritime correspondence.
pub const BOTTLE_MESSAGES: &[&str] = &[
    "\"Help, I'm stuck in a block game.\" - Stove",
    "\"If you're reading this, the crafting table was decorative all along.\"",
    "\"Day 47. The Clucksters have unionised.\"",
    "\"Dear finder: the Dimond is spelled wrong on purpose. Please stop emailing.\"",
    "\"I dropped my Big Bob. He was right there. He's gone.\"",
    "\"Rotate your crops. The soil is watching.\" - A Concerned Potato",
    "\"This bottle intentionally left blank.\"",
    "A treasure map! It leads to... the spot where you're standing. Congratulations.",
];

// ------------------------------------------------------------------ in the game

use crate::game::Game;
use crate::net::Msg;
use crate::sound::Sfx;

impl Game {
    /// Right-click with the Fishing Stick: cast, or reel in.
    pub fn use_rod(&mut self) {
        self.player.swing = 1.0;
        let Some(b) = &mut self.bobber else {
            let dir = self.player.look_dir();
            let bait = self.inv.count(BAIT) > 0;
            self.bobber = Some(Bobber::cast(self.player.eye() + dir * 0.6, dir, bait));
            self.sfx(Sfx::Twang, None);
            return;
        };
        if b.fight.is_some() {
            return; // reeling a big one is done by holding the button
        }
        let early = matches!(b.state, BobberState::Floating { .. }) && b.since_nibble < 0.6;
        match b.reel(big_chance(self.fish_log.level()), &mut self.rng) {
            Some(FishEvent::Landed(p)) => {
                self.bobber = None;
                self.land_catch(p);
            }
            None if b.fight.is_some() => {
                self.msg("It's a big one! Hold right-click to reel in, but ease off before the line snaps!");
            }
            _ => {
                self.bobber = None;
                self.msg(if early { "Too early! That was a nibble. The fish are laughing at you." } else { "You reel in... nothing. Very relaxing, though." });
            }
        }
    }

    /// Per frame, on the fisher's own machine.
    pub fn update_fishing(&mut self, dt: f32, reeling: bool) {
        if self.bobber.is_none() {
            return;
        }
        if self.inv.held() != ROD || self.dead.is_some() {
            self.bobber = None;
            return;
        }
        let rod_at = self.player.eye();
        let Some(b) = &mut self.bobber else { return };
        let raining = self.weather.kind.wet();
        let events = b.update(dt, &self.world, &self.fish_log, self.time, raining, rod_at, reeling, &mut self.rng);
        let at = b.pos;
        for e in events {
            match e {
                FishEvent::Splashdown => self.sfx(Sfx::Splash, Some(at)),
                FishEvent::Nibble => self.sfx(Sfx::Bloop, Some(at)),
                FishEvent::Bite => {
                    self.sfx(Sfx::Splash, Some(at));
                    self.msg("!!! Something's biting! Reel in!");
                }
                FishEvent::GotAway => self.msg("It got away. Fish are fast."),
                FishEvent::Snapped => {
                    self.bobber = None;
                    self.fish_log.snapped += 1;
                    self.sfx(Sfx::Twang, None);
                    self.msg("SNAP! The line broke. The fish is telling all its friends about you.");
                    self.advance("one_that_got_away");
                }
                FishEvent::Landed(p) => {
                    self.bobber = None;
                    self.land_catch(p);
                }
                FishEvent::TooFar => {
                    self.bobber = None;
                    self.msg("The line ran out. You reel it back in.");
                }
            }
        }
    }

    /// Reeled something in at `p`: the world's owner decides what it is.
    /// A catch leaves the water on an arc toward whoever caught it: an item
    /// on the ground like any other, picked up when it arrives.
    fn fling_catch(&mut self, from: Vec3, to: Vec3, item: Id, n: u8) {
        const FLIGHT: f32 = 0.8;
        let start = from + Vec3::Y * 0.6;
        let vel = (to - start) / FLIGHT + Vec3::Y * (0.5 * crate::entity::GRAVITY * FLIGHT);
        self.spawn_drop(start, item, n, 0, vel, 0.0);
    }

    fn land_catch(&mut self, p: Vec3) {
        if self.inv.held() == ROD {
            self.use_tool(1);
        }
        let bait = self.inv.count(BAIT) > 0;
        if bait && !self.creative {
            self.inv.remove(BAIT, 1);
        }
        self.sfx(Sfx::Splash, None);
        if self.is_client() {
            self.net_send_msg(Msg::Catch { pos: p, bait });
            return;
        }
        let level = self.fish_log.level();
        let c = roll_catch(&self.world, p, self.time, level, bait, &mut self.rng);
        let best = self.fish_log.record(c.item, c.cm, c.category);
        let line = catch_line(&c, best);
        self.msg(line);
        // The catch flies out of the water toward you, like it should.
        let me = self.player.body.pos + Vec3::Y * 0.9;
        self.fling_catch(p, me, c.item, c.n);
        let points = self.rng.int(1, 6) as u32;
        self.add_xp(points);
        self.catch_advancements(c.item, c.category);
        let new_level = self.fish_log.level();
        if new_level > level {
            self.msg(format!("Angler level {new_level}! The fish have started a group chat about you."));
        }
    }

    fn catch_advancements(&mut self, item: Id, category: Category) {
        self.advance("gone_fishin");
        match category {
            Category::Legendary => self.advance("big_bob"),
            Category::Treasure => self.advance("sunken_treasure"),
            _ if item == BOOT => self.advance("bootiful"),
            _ => {}
        }
    }

    /// A joined player landed a fish at `pos`: check it's plausible, then roll it here.
    pub fn host_catch(&mut self, from: u32, pos: Vec3, bait: bool) {
        let Some(peer) = self.peers.get(&from) else { return };
        let them = peer.target + Vec3::Y * 0.9;
        let near = peer.target.distance(pos) < 42.0;
        let wet = (0..2).any(|dy| self.world.get(pos.x.floor() as i32, pos.y.floor() as i32 - dy, pos.z.floor() as i32) == WATER);
        if !near || !wet || !pos.is_finite() || !self.peer_has(from, ROD) {
            return;
        }
        // Real fishing takes a few seconds per fish.
        if !self.peer_rate_ok(from, "catch", 2.5) {
            return;
        }
        // Bait only counts if they really had some (and then it's used up).
        let bait = bait && self.peer_take(from, BAIT, 1);
        let c = roll_catch(&self.world, pos, self.time, 0, bait, &mut self.rng);
        self.system_message(Some(from), &catch_line(&c, false));
        self.fling_catch(pos, them, c.item, c.n);
        self.host_wear(from, ROD, 1);
        let points = self.rng.int(1, 6) as u32;
        self.give_peer_xp(from, points);
    }
}

/// "You caught a 54cm Raw Salmon-ish!"
pub fn catch_line(c: &Catch, best: bool) -> String {
    let name = item_name(c.item);
    let mut s = match (c.category, c.cm) {
        (Category::Legendary, Some(cm)) => format!("LEGENDARY! You caught BIG BOB, all {cm:.0}cm of him. He looks disappointed in you."),
        (_, Some(cm)) => format!("You caught a {cm:.0}cm {name}!"),
        (Category::Treasure, None) => format!("Sunken treasure: {name}!"),
        _ => format!("You fished up a {name}. Congratulations?"),
    };
    if best {
        s += " New personal best!";
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_of_day_matters() {
        assert!(time_factor(0.01) < time_factor(0.25), "dawn beats noon");
        assert!(time_factor(0.5) < time_factor(0.25), "dusk beats noon");
        assert!(time_factor(0.75) > time_factor(0.25), "midnight is slow");
    }

    #[test]
    fn log_levels_and_saving() {
        let mut log = FishLog::default();
        assert_eq!(log.level(), 0);
        assert!(!log.record(COD, Some(40.0), Category::Fish), "the first is not a 'new best'");
        assert!(log.record(COD, Some(55.0), Category::Fish));
        assert!(!log.record(COD, Some(20.0), Category::Fish));
        log.record(BIG_BOB, Some(150.0), Category::Legendary);
        assert_eq!(log.species["cod"], (3, 55.0));
        assert!(log.level() >= 2);
        log.snapped = 4;
        assert_eq!(FishLog::decode(&log.encode()), log);
        assert_eq!(FishLog::decode(&[1, 2]), FishLog::default());
    }

    #[test]
    fn fights_can_be_won_or_lost() {
        let log = FishLog::default();
        let mut rng = Rng::new(3);
        // Hold the line the whole time: it snaps.
        let mut b = Bobber { pos: Vec3::ZERO, vel: Vec3::ZERO, state: BobberState::Grounded, fight: Some(Fight { progress: 0.1, tension: 0.2, surge: 1.0 }), since_nibble: 9.0, bait: false };
        let world = World::new(1);
        let mut out = Vec::new();
        for _ in 0..600 {
            out = b.update(0.02, &world, &log, 0.25, false, Vec3::ZERO, true, &mut rng);
            if !out.is_empty() {
                break;
            }
        }
        assert_eq!(out, vec![FishEvent::Snapped]);
        // Reel in bursts and let the tension drop: it lands.
        let mut b = Bobber { fight: Some(Fight { progress: 0.1, tension: 0.2, surge: 1.0 }), ..b };
        let mut t = 0.0;
        let mut out = Vec::new();
        for _ in 0..3000 {
            let tension = b.fight.map(|f| f.tension).unwrap_or(0.0);
            out = b.update(0.02, &world, &log, 0.25, false, Vec3::ZERO, tension < 0.5, &mut rng);
            t += 0.02;
            if !out.is_empty() {
                break;
            }
        }
        assert!(matches!(out.as_slice(), [FishEvent::Landed(_)]), "{out:?} after {t}s");
    }
}
