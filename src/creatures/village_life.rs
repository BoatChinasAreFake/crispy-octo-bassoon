//! **Villages that live** (v0.5).
//!
//! - **Beds.** Each grown-up Hmmer claims a bed near home: the nearest one
//!   nobody else has. Its home is its bed from then on.
//! - **Workstations.** Each job has one: a Composter (Farmers), a Lectern
//!   (Librarians), a Smithing Table (Smiths), a Barrel (Fishers), a Brewing
//!   Stand (Clerics), a Blast Furnace (Armourers), a Cartography Table
//!   (Cartographers) and a Smoker (Butchers). A Hmmer claims a free one of
//!   its own job's nearby. A Hmmer with no job (a grown-up baby, or one whose
//!   workstation was broken before anyone traded with it) takes the job of
//!   the first free workstation it finds.
//! - **The day.** Mornings at the workstation, afternoons pottering about
//!   home; at night everyone goes indoors to bed and sleeps until morning.
//! - **Babies.** While the village has a free bed, two grown-ups near each
//!   other in the daytime now and then have a baby, who grows up in a while
//!   and looks for a bed and a job of its own.
//!
//! Claims are by mob id, kept where the world lives and saved with it.
//! Joined players just see Hmmers walking about (and lying in bed).

use crate::block::*;
use crate::entity::{Mob, MobKind};
use crate::game::Game;
use crate::villagers::Job;
use macroquad::math::{IVec3, Vec3};
use std::collections::{HashMap, HashSet};

/// A Hmmer with no job (its colouring byte; jobs are 1 + `Job::index`).
pub const JOBLESS: u8 = 9;
/// How far from home a Hmmer looks for a bed or a workstation (across, and up and down).
pub const LOOK: i32 = 24;
const LOOK_Y: i32 = 8;
/// Seconds between looks round (each looks at a few Hmmers).
const LOOK_EVERY: f32 = 1.0;
const PER_LOOK: usize = 4;
/// Seconds a baby Hmmer takes to grow up.
pub const GROW_SECS: f32 = 600.0;
/// At most one baby per this many seconds in the whole world (they're a lot of work).
pub const BABY_EVERY: f32 = 90.0;
/// How close two grown-ups must be to have a baby.
const BABY_NEAR: f32 = 8.0;

/// A job's workstation (village houses get one each: see houses.rs).
pub fn station_of(job: Job) -> Id {
    match job {
        Job::Farmer => COMPOSTER,
        Job::Librarian => LECTERN,
        Job::Smith => SMITHING_TABLE,
        Job::Fisher => BARREL,
        Job::Cleric => BREWING_STAND,
        Job::Armourer => BLAST_FURNACE,
        Job::Cartographer => CARTOGRAPHY_TABLE,
        Job::Butcher => SMOKER,
    }
}

/// Whose workstation a block is, if anyone's.
pub fn station_job(id: Id) -> Option<Job> {
    if crate::homecraft::is_composter(id) {
        return Some(Job::Farmer);
    }
    if crate::books::is_lectern(id) {
        return Some(Job::Librarian);
    }
    match id {
        SMITHING_TABLE => Some(Job::Smith),
        BARREL => Some(Job::Fisher),
        BREWING_STAND => Some(Job::Cleric),
        BLAST_FURNACE | BLAST_FURNACE_LIT => Some(Job::Armourer),
        CARTOGRAPHY_TABLE => Some(Job::Cartographer),
        SMOKER | SMOKER_LIT => Some(Job::Butcher),
        _ => None,
    }
}

/// Has this Hmmer no job?
pub fn jobless(m: &Mob) -> bool {
    m.kind == MobKind::Hmmer && m.variant == JOBLESS
}

/// What part of the day it is, for a Hmmer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Part {
    Work,
    Free,
    Bed,
}

/// The part of the day at time `t` (0 sunrise, 0.25 noon, 0.5 sunset).
pub fn part_of_day(t: f32) -> Part {
    let t = t.rem_euclid(1.0);
    if !(0.03..0.47).contains(&t) {
        Part::Bed
    } else if t < 0.27 {
        Part::Work
    } else {
        Part::Free
    }
}

/// Who has claimed which bed and workstation (by mob id).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Life {
    pub beds: HashMap<u32, IVec3>,
    pub stations: HashMap<u32, IVec3>,
    /// (Not saved.) Seconds to the next look round, and which Hmmer is next.
    pub look: f32,
    pub next: usize,
    /// Seconds until a baby may come along.
    pub baby_wait: f32,
}

impl Life {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = String::new();
        let mut put = |kind: &str, map: &HashMap<u32, IVec3>| {
            let mut v: Vec<_> = map.iter().collect();
            v.sort_by_key(|e| *e.0);
            for (id, p) in v {
                out.push_str(&format!("{kind}\t{id}\t{}\t{}\t{}\n", p.x, p.y, p.z));
            }
        };
        put("bed", &self.beds);
        put("station", &self.stations);
        out.into_bytes()
    }

    pub fn decode(b: &[u8]) -> Life {
        let mut l = Life::default();
        for line in String::from_utf8_lossy(b).lines() {
            let f: Vec<&str> = line.split('\t').collect();
            let [kind, id, x, y, z] = f[..] else { continue };
            let (Ok(id), Ok(x), Ok(y), Ok(z)) = (id.parse::<u32>(), x.parse(), y.parse(), z.parse()) else { continue };
            let p = IVec3::new(x, y, z);
            match kind {
                "bed" => l.beds.insert(id, p),
                "station" => l.stations.insert(id, p),
                _ => None,
            };
        }
        l
    }
}

/// The nearest block round `at` that `want` likes and isn't in `taken`.
fn nearest(world: &crate::world::World, at: Vec3, want: impl Fn(Id) -> bool, taken: &HashSet<IVec3>) -> Option<IVec3> {
    let c = at.floor().as_ivec3();
    let mut best: Option<(i32, IVec3)> = None;
    for dy in -LOOK_Y..=LOOK_Y {
        for dz in -LOOK..=LOOK {
            for dx in -LOOK..=LOOK {
                let p = c + IVec3::new(dx, dy, dz);
                let d = dx * dx + dz * dz + dy * dy * 4;
                if best.is_some_and(|b| b.0 <= d) {
                    continue;
                }
                if want(world.get_v(p)) && !taken.contains(&p) {
                    best = Some((d, p));
                }
            }
        }
    }
    best.map(|b| b.1)
}

/// Somewhere to stand beside a block (on the ground next to it), else on top of it.
fn beside(world: &crate::world::World, p: IVec3) -> Vec3 {
    for d in [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z] {
        let q = p + d;
        if !is_solid(world.get_v(q)) && !is_solid(world.get_v(q + IVec3::Y)) && is_solid(world.get_v(q - IVec3::Y)) {
            return q.as_vec3() + Vec3::new(0.5, 0.0, 0.5);
        }
    }
    p.as_vec3() + Vec3::new(0.5, 1.0, 0.5)
}

impl Game {
    /// Where the world lives: Hmmers claim beds and workstations, keep their
    /// day, and now and then have babies.
    pub fn village_life_tick(&mut self, dt: f32) {
        if self.is_client() {
            return;
        }
        self.life.look -= dt;
        self.life.baby_wait = (self.life.baby_wait - dt).max(0.0);
        if self.life.look <= 0.0 {
            self.life.look = LOOK_EVERY;
            self.village_look();
        }
        self.village_routine();
    }

    /// The Hmmers who live here (grown-up, alive) and the babies.
    fn hmmer_ids(&self) -> Vec<u32> {
        self.mobs.iter().filter(|m| m.kind == MobKind::Hmmer && m.health > 0.0).map(|m| m.id).collect()
    }

    /// Forget claims on beds and workstations that are gone, and on Hmmers
    /// who are; then let a few Hmmers look round for what they're missing.
    fn village_look(&mut self) {
        let alive: HashSet<u32> = self.hmmer_ids().into_iter().collect();
        self.life.beds.retain(|id, _| alive.contains(id));
        self.life.stations.retain(|id, _| alive.contains(id));
        // A bed that's gone (or isn't loaded: keep the claim till it is).
        let world = &self.world;
        let loaded = |p: &IVec3| world.is_loaded(p.x, p.z);
        self.life.beds.retain(|_, p| !loaded(p) || crate::beds::is_bed(world.get_v(*p)));
        let mut lost = Vec::new();
        self.life.stations.retain(|id, p| {
            let keep = !loaded(p) || station_job(world.get_v(*p)).is_some();
            if !keep {
                lost.push(*id);
            }
            keep
        });
        // A Hmmer whose workstation was broken before anyone got to know it loses its job.
        for id in lost {
            let known = self.regulars.keys().any(|(mob, _)| *mob == id);
            if let Some(m) = self.mobs.iter_mut().find(|m| m.id == id)
                && !known
            {
                m.variant = JOBLESS;
            }
        }
        // A wrong-job workstation (someone swapped the block) is let go too.
        let mismatched: Vec<u32> = self
            .life
            .stations
            .iter()
            .filter(|(id, p)| self.world.is_loaded(p.x, p.z) && self.mobs.iter().find(|m| m.id == **id).is_some_and(|m| !jobless(m) && station_job(self.world.get_v(**p)) != Some(Job::of_mob(m))))
            .map(|(id, _)| *id)
            .collect();
        for id in mismatched {
            self.life.stations.remove(&id);
        }
        // A few Hmmers a time look round.
        let ids: Vec<u32> = alive.into_iter().collect::<std::collections::BTreeSet<_>>().into_iter().collect();
        if ids.is_empty() {
            return;
        }
        for k in 0..PER_LOOK.min(ids.len()) {
            let id = ids[(self.life.next + k) % ids.len()];
            self.hmmer_looks_round(id);
        }
        self.life.next = (self.life.next + PER_LOOK) % ids.len().max(1);
        self.maybe_baby();
    }

    /// Hmmer `id` looks for a bed and (if grown up) a workstation.
    pub fn hmmer_looks_round(&mut self, id: u32) {
        let Some(m) = self.mobs.iter().find(|m| m.id == id) else { return };
        let (at, grown, job) = (m.home.unwrap_or(m.body.pos), m.baby <= 0.0, if jobless(m) { None } else { Some(Job::of_mob(m)) });
        if !self.world.is_loaded(at.x.floor() as i32, at.z.floor() as i32) {
            return;
        }
        if !self.life.beds.contains_key(&id) {
            let taken: HashSet<IVec3> = self.life.beds.values().copied().collect();
            if let Some(bed) = nearest(&self.world, at, crate::beds::is_bed, &taken) {
                self.life.beds.insert(id, bed);
                if let Some(m) = self.mobs.iter_mut().find(|m| m.id == id) {
                    m.home = Some(bed.as_vec3() + Vec3::new(0.5, 1.0, 0.5));
                }
            }
        }
        if grown && !self.life.stations.contains_key(&id) {
            let taken: HashSet<IVec3> = self.life.stations.values().copied().collect();
            let want = |b: Id| match job {
                Some(j) => station_job(b) == Some(j),
                None => station_job(b).is_some(),
            };
            if let Some(p) = nearest(&self.world, at, want, &taken) {
                self.life.stations.insert(id, p);
                if job.is_none()
                    && let Some(j) = station_job(self.world.get_v(p))
                    && let Some(m) = self.mobs.iter_mut().find(|m| m.id == id)
                {
                    // A new job (and a fresh set of trades).
                    m.variant = 1 + j.index();
                    m.trades_used = [0; 8];
                    let pos = m.body.pos;
                    self.sfx(crate::sound::Sfx::Hmm, Some(pos));
                    self.smoke(pos + Vec3::Y * 2.1, 4, 0.2);
                }
            }
        }
    }

    /// Hmmers go where the time of day says.
    fn village_routine(&mut self) {
        let part = part_of_day(self.time);
        let beds = &self.life.beds;
        let stations = &self.life.stations;
        let world = &self.world;
        for m in self.mobs.iter_mut().filter(|m| m.kind == MobKind::Hmmer && m.health > 0.0) {
            if m.flee > 0.0 || m.rider != 0 || m.leash.is_some() {
                m.sitting = false;
                continue;
            }
            let bed = beds.get(&m.id).copied();
            match part {
                Part::Bed => {
                    let Some(bed) = bed else {
                        m.sitting = false;
                        continue;
                    };
                    let top = bed.as_vec3() + Vec3::new(0.5, 0.6, 0.5);
                    let flat = Vec3::new(top.x - m.body.pos.x, 0.0, top.z - m.body.pos.z).length();
                    if m.sitting || (flat < 1.3 && (m.body.pos.y - top.y).abs() < 1.6) {
                        // In bed: lie down and stay there (see `entity::Mob::draw`).
                        m.sitting = true;
                        m.body.pos = top;
                        m.body.vel = Vec3::ZERO;
                        m.goal = Some(top);
                    } else {
                        m.goal = Some(beside(world, bed));
                    }
                }
                Part::Work | Part::Free => {
                    m.sitting = false;
                    if part == Part::Work
                        && m.baby <= 0.0
                        && let Some(s) = stations.get(&m.id)
                    {
                        let spot = beside(world, *s);
                        if spot.distance(m.body.pos) > 3.0 {
                            m.goal = Some(spot);
                        }
                    }
                }
            }
        }
    }

    /// A baby, if the village has room and two grown-ups are about in the daytime.
    fn maybe_baby(&mut self) {
        if self.life.baby_wait > 0.0 || part_of_day(self.time) == Part::Bed {
            return;
        }
        let hmmers: Vec<(u32, Vec3)> = self.mobs.iter().filter(|m| m.kind == MobKind::Hmmer && m.health > 0.0 && m.baby <= 0.0 && !m.sitting).map(|m| (m.id, m.body.pos)).collect();
        for &(a, pa) in &hmmers {
            let Some(&bed) = self.life.beds.get(&a) else { continue };
            let Some(&(_, pb)) = hmmers.iter().find(|(b, p)| *b != a && p.distance(pa) < BABY_NEAR) else { continue };
            // Room for one more: a free bed near home, more than there are Hmmers without one.
            let taken: HashSet<IVec3> = self.life.beds.values().copied().collect();
            let Some(free) = nearest(&self.world, bed.as_vec3(), crate::beds::is_bed, &taken) else { continue };
            let homeless = self.mobs.iter().filter(|m| m.kind == MobKind::Hmmer && m.health > 0.0 && !self.life.beds.contains_key(&m.id) && m.body.pos.distance(free.as_vec3()) < LOOK as f32 * 1.5).count();
            if homeless > 0 {
                continue;
            }
            let at = (pa + pb) * 0.5;
            let id = self.alloc_mob(MobKind::Hmmer, at);
            let seed = self.rng.next_u32() & 0xFF_FFFF;
            if let Some(m) = self.mobs.iter_mut().find(|m| m.id == id) {
                m.set_baby(GROW_SECS);
                m.variant = JOBLESS;
                m.seed = seed;
                m.persistent = true;
                m.home = Some(free.as_vec3() + Vec3::new(0.5, 1.0, 0.5));
            }
            self.life.beds.insert(id, free);
            self.hearts(at + Vec3::Y * 2.0, 4);
            self.life.baby_wait = BABY_EVERY;
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A flat little village: Hmmers, beds and a workstation on a stone floor.
    fn village(seed: u32) -> Game {
        let mut g = crate::game::tests::arena(seed);
        let y = 50;
        for x in -20..=20 {
            for z in -20..=20 {
                g.world.set(x, y - 1, z, STONE);
                for yy in y..y + 4 {
                    g.world.set(x, yy, z, AIR);
                }
            }
        }
        g.time = 0.1;
        // (No night-time visitors to chase anyone out of bed.)
        g.rules.difficulty = crate::rules::Difficulty::Peaceful;
        g
    }

    fn hmmer(g: &mut Game, at: Vec3, variant: u8) -> u32 {
        let id = g.alloc_mob(MobKind::Hmmer, at);
        let m = g.mobs.iter_mut().find(|m| m.id == id).unwrap();
        m.variant = variant;
        m.home = Some(at);
        m.persistent = true;
        id
    }

    #[test]
    fn every_job_has_a_workstation_and_back() {
        for job in Job::ALL {
            assert_eq!(station_job(station_of(job)), Some(job));
        }
        assert_eq!(station_job(SMOKER_LIT), Some(Job::Butcher));
        assert_eq!(station_job(COMPOSTER + 7), Some(Job::Farmer));
        assert_eq!(station_job(STONE), None);
        assert_eq!(part_of_day(0.1), Part::Work);
        assert_eq!(part_of_day(0.35), Part::Free);
        assert_eq!(part_of_day(0.7), Part::Bed);
    }

    #[test]
    fn hmmers_claim_beds_and_workstations_and_jobless_ones_find_jobs() {
        let mut g = village(501);
        let a = hmmer(&mut g, Vec3::new(0.5, 50.0, 0.5), JOBLESS);
        let b = hmmer(&mut g, Vec3::new(3.5, 50.0, 0.5), 1 + Job::Librarian.index());
        let c = hmmer(&mut g, Vec3::new(-3.5, 50.0, 0.5), JOBLESS);
        for p in [IVec3::new(5, 50, 5), IVec3::new(-5, 50, 5)] {
            g.world.set_v(p, crate::beds::bed(0));
        }
        g.world.set_v(IVec3::new(2, 50, -4), SMOKER);
        g.world.set_v(IVec3::new(6, 50, -4), LECTERN);
        for _ in 0..4 {
            g.village_life_tick(LOOK_EVERY);
        }
        // Two beds, three Hmmers: two get one each, nobody shares.
        assert_eq!(g.life.beds.len(), 2);
        let beds: HashSet<_> = g.life.beds.values().collect();
        assert_eq!(beds.len(), 2);
        // The Librarian takes the Lectern; one jobless Hmmer becomes a Butcher at the Smoker.
        assert_eq!(g.life.stations.get(&b), Some(&IVec3::new(6, 50, -4)));
        let butchers: Vec<u32> = [a, c].into_iter().filter(|id| g.mobs.iter().any(|m| m.id == *id && Job::of_mob(m) == Job::Butcher && !jobless(m))).collect();
        assert_eq!(butchers.len(), 1, "one of them takes the Smoker");
        let other = if butchers[0] == a { c } else { a };
        assert!(jobless(g.mobs.iter().find(|m| m.id == other).unwrap()), "no workstation left for the other");
        // Break the Smoker: a Butcher nobody has traded with is out of a job.
        g.world.set_v(IVec3::new(2, 50, -4), AIR);
        g.village_life_tick(LOOK_EVERY);
        assert!(jobless(g.mobs.iter().find(|m| m.id == butchers[0]).unwrap()));
        assert!(!g.life.stations.contains_key(&butchers[0]));
        // Kept with the world.
        assert_eq!(Life::decode(&g.life.encode()).beds, g.life.beds);
        let back = Game::from_save(g.to_save());
        assert_eq!(back.life.beds, g.life.beds);
        assert_eq!(back.life.stations, g.life.stations);
    }

    #[test]
    fn at_night_they_go_to_bed_and_in_the_morning_to_work() {
        let mut g = village(502);
        let id = hmmer(&mut g, Vec3::new(0.5, 50.0, 0.5), 1 + Job::Fisher.index());
        let bed = IVec3::new(8, 50, 0);
        let barrel = IVec3::new(-8, 50, 0);
        g.world.set_v(bed, crate::beds::bed(1));
        g.world.set_v(barrel, BARREL);
        g.village_life_tick(LOOK_EVERY);
        assert_eq!(g.life.beds.get(&id), Some(&bed));
        assert_eq!(g.life.stations.get(&id), Some(&barrel));
        // Night: off to bed, and asleep once there.
        g.time = 0.7;
        let mut slept = false;
        for _ in 0..1200 {
            g.update_entities(0.05);
            slept |= g.mobs.iter().any(|m| m.id == id && m.sitting);
        }
        assert!(slept, "it got into bed: {:?}", g.mobs.iter().find(|m| m.id == id).map(|m| m.body.pos));
        // Morning: up, and over to the barrel.
        g.time = 0.05;
        for _ in 0..1200 {
            g.update_entities(0.05);
        }
        let m = g.mobs.iter().find(|m| m.id == id).unwrap();
        assert!(!m.sitting);
        assert!(m.body.pos.distance(barrel.as_vec3() + Vec3::new(0.5, 0.0, 0.5)) < 4.0, "at work: {:?}", m.body.pos);
    }

    #[test]
    fn babies_come_when_there_are_free_beds_and_not_otherwise() {
        let mut g = village(503);
        hmmer(&mut g, Vec3::new(0.5, 50.0, 0.5), 1);
        hmmer(&mut g, Vec3::new(2.5, 50.0, 0.5), 2);
        g.world.set_v(IVec3::new(5, 50, 5), crate::beds::bed(0));
        g.world.set_v(IVec3::new(-5, 50, 5), crate::beds::bed(0));
        for _ in 0..4 {
            g.village_life_tick(LOOK_EVERY);
        }
        let count = |g: &Game| g.mobs.iter().filter(|m| m.kind == MobKind::Hmmer).count();
        assert_eq!(count(&g), 2, "two beds, two Hmmers: no room for a baby");
        g.world.set_v(IVec3::new(0, 50, 8), crate::beds::bed(0));
        for _ in 0..4 {
            g.village_life_tick(LOOK_EVERY);
        }
        assert_eq!(count(&g), 3, "a third bed: a baby");
        let baby = g.mobs.iter().find(|m| m.baby > 0.0).expect("a baby");
        assert!(jobless(baby) && g.life.beds.contains_key(&baby.id));
        // And not another straight away (or with no bed for it).
        g.world.set_v(IVec3::new(0, 50, -8), crate::beds::bed(0));
        g.village_life_tick(LOOK_EVERY);
        assert_eq!(count(&g), 3);
    }
}
