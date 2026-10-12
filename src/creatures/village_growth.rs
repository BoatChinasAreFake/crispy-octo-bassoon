//! **Growing villages** (v0.5): a village with people about keeps itself up.
//!
//! - **The Notice Board.** The first time anyone comes by, the village puts
//!   up its Notice Board on the square (see bounties.rs).
//! - **New houses.** Once a day, a village with every bed taken starts on a
//!   new house: first on the spots its plan left empty, then further out at
//!   the ends of its paths. It goes up a few blocks at a time over a couple
//!   of minutes, in the village's own style and with a workstation, so a
//!   baby has somewhere to sleep and a grown-up somewhere to work (see
//!   village_life.rs). A spot with anyone's building on it is left alone.
//! - **New Clankers.** A village builds itself a Clanker for every six
//!   Hmmers it has (at most one a day): a puff of smoke on the square and
//!   there it is.
//!
//! All of this happens where the world lives, in villages near a player;
//! joined players see the blocks go up like any others.

use crate::block::*;
use crate::entity::MobKind;
use crate::game::Game;
use crate::structures::{Kind, Site};
use macroquad::math::{IVec3, Vec3};
use std::collections::{HashMap, HashSet};

/// Hmmers a village needs for each Clanker.
pub const HMMERS_PER_CLANKER: usize = 6;
/// Seconds between looks round.
const LOOK_EVERY: f32 = 2.0;
/// Blocks a house gets each look.
pub const BLOCKS_PER_LOOK: usize = 6;
/// Fewest Hmmers that will start on a house.
const BUILDERS: usize = 2;
/// How near (blocks) a player must be to see it happen (for the advancements).
const WATCH: f32 = 48.0;

/// A house going up: which spot, and how far through its blocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Build {
    pub spot: u8,
    pub step: u32,
}

/// What a village has done about growing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Village {
    /// The day it last thought about building (one house and one Clanker a day, at most).
    pub day: u32,
    pub clanker_day: u32,
    /// The next spot to try for a house, and houses built.
    pub next: u8,
    pub houses: u8,
    /// Whether its Notice Board has gone up.
    pub board: bool,
    pub build: Option<Build>,
}

/// Every village's growth, by its square (as reputation.rs knows it).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Growth {
    pub villages: HashMap<IVec3, Village>,
    /// (Not saved.) Seconds to the next look round.
    pub look: f32,
}

impl Growth {
    pub fn encode(&self) -> Vec<u8> {
        let mut v: Vec<_> = self.villages.iter().collect();
        v.sort_by_key(|(k, _)| k.to_array());
        let mut out = String::new();
        for (k, g) in v {
            let (spot, step) = g.build.map_or((-1, 0), |b| (b.spot as i32, b.step));
            out.push_str(&format!("village\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{spot}\t{step}\n", k.x, k.y, k.z, g.day, g.clanker_day, g.next, g.houses, g.board as u8));
        }
        out.into_bytes()
    }

    pub fn decode(b: &[u8]) -> Growth {
        let mut out = Growth::default();
        for line in String::from_utf8_lossy(b).lines() {
            let f: Vec<&str> = line.split('\t').collect();
            let ["village", x, y, z, day, cday, next, houses, board, spot, step] = f[..] else { continue };
            let n = |s: &str| s.parse::<i64>().ok();
            let (Some(x), Some(y), Some(z), Some(day), Some(cday), Some(next), Some(houses), Some(board), Some(spot), Some(step)) = (n(x), n(y), n(z), n(day), n(cday), n(next), n(houses), n(board), n(spot), n(step)) else { continue };
            let build = (spot >= 0).then_some(Build { spot: spot as u8, step: step as u32 });
            let v = Village { day: day as u32, clanker_day: cday as u32, next: next as u8, houses: houses as u8, board: board != 0, build };
            out.villages.insert(IVec3::new(x as i32, y as i32, z as i32), v);
        }
        out
    }
}

/// Could a house go up over this? (The lie of the land, trees and plants,
/// yes; anyone's building, no.)
fn clear_enough(id: Id) -> bool {
    let b = block(id);
    // Plants and things lying on the ground (flowers, leaf litter), but not torches.
    let plant = (b.model == Model::Cross || !b.solid && b.sound == S_GRASS) && b.light == 0.0;
    replaceable(id) || is_leaves(id) || is_log(id) || crate::bees::is_flower(id) || plant || matches!(id, GRASS | DIRT | STONE | SAND | GRAVEL | SNOW_GRASS | SNOW_BLOCK | BEE_NEST | BEE_NEST_HONEY)
}

/// The blocks a new house is built from, in the order they go up: the
/// clearing first, then from the footing up.
pub fn house_plan(at: IVec3, facing: u8, seed: u32, style: crate::houses::Style) -> Vec<(IVec3, Id)> {
    let mut blocks = crate::houses::styled_house(at, facing, seed, style, true);
    // (Later blocks win: keep only the last word on each spot.)
    let mut last = HashMap::new();
    for (i, (p, _)) in blocks.iter().enumerate() {
        last.insert(*p, i);
    }
    let mut keep: Vec<(usize, (IVec3, Id))> = blocks.drain(..).enumerate().filter(|(i, (p, _))| last[p] == *i).collect();
    keep.sort_by_key(|(i, (p, id))| (*id != AIR, p.y, *i));
    keep.into_iter().map(|(_, b)| b).collect()
}

impl Game {
    /// The village whose square is at `square` (a village key), from the generator.
    pub fn village_site(&self, square: IVec3) -> Option<Site> {
        let off = self.world.dim().gen_x();
        let origin = square - IVec3::Y + IVec3::new(off, 0, 0);
        let site = self.world.generator.site(origin.x.div_euclid(crate::world::CW), origin.z.div_euclid(crate::world::CW))?;
        (site.kind == Kind::Village && site.origin == origin).then(|| Site { origin: site.origin - IVec3::new(off, 0, 0), ..site })
    }

    /// Where a village can grow (in this dimension's own coordinates).
    pub(crate) fn growth_spots(&self, site: &Site) -> Vec<(IVec3, u8, u32)> {
        let off = IVec3::new(self.world.dim().gen_x(), 0, 0);
        let gen_site = Site { origin: site.origin + off, ..*site };
        self.world.generator.village_growth_spots(&gen_site).into_iter().map(|(p, f, s)| (p - off, f, s)).collect()
    }

    /// How a village builds.
    pub(crate) fn growth_style(&self, site: &Site) -> crate::houses::Style {
        let off = IVec3::new(self.world.dim().gen_x(), 0, 0);
        self.world.generator.village_style(&Site { origin: site.origin + off, ..*site })
    }

    /// Where the world lives: villages near players put up their boards, build houses and Clankers.
    pub fn village_growth_tick(&mut self, dt: f32) {
        if self.is_client() {
            return;
        }
        self.growth.look -= dt;
        if self.growth.look > 0.0 {
            return;
        }
        self.growth.look = LOOK_EVERY;
        let mut squares: Vec<IVec3> = Vec::new();
        for (_, at) in self.player_spots_by_id() {
            if let Some(c) = self.village_at(at) {
                let key = c.floor().as_ivec3();
                if !squares.contains(&key) {
                    squares.push(key);
                }
            }
        }
        for key in squares {
            self.grow_village(key);
        }
    }

    /// One look at the village whose square is `key`.
    pub fn grow_village(&mut self, key: IVec3) {
        let Some(site) = self.village_site(key) else { return };
        if !self.world.is_loaded(key.x, key.z) {
            return;
        }
        let mut v = self.growth.villages.get(&key).copied().unwrap_or_default();
        if !v.board {
            let at = crate::structures::village_board(site.origin);
            if replaceable(self.world.get_v(at)) {
                self.world.set_v(at, NOTICE_BOARD);
            }
            v.board = true;
        }
        if let Some(b) = v.build {
            v.build = self.build_house(&site, b);
            if v.build.is_none() {
                v.houses = v.houses.saturating_add(1);
                self.house_done(key);
            }
        }
        let daytime = crate::village_life::part_of_day(self.time) != crate::village_life::Part::Bed;
        if daytime && v.day != self.day + 1 {
            v.day = self.day + 1;
            if v.build.is_none() && self.village_full(key) {
                v.build = self.start_house(&site, &mut v);
            }
        }
        if daytime && v.clanker_day != self.day + 1 && self.wants_clanker(key) {
            v.clanker_day = self.day + 1;
            self.build_clanker(key);
        }
        self.growth.villages.insert(key, v);
    }

    /// The village's grown-up Hmmers and its Clankers.
    fn village_folk(&self, key: IVec3) -> (usize, usize) {
        let centre = key.as_vec3() + Vec3::new(0.5, 0.0, 0.5);
        let near = |p: Vec3| Vec3::new(p.x - centre.x, 0.0, p.z - centre.z).length() < crate::raids::VILLAGE_RANGE;
        let alive = self.mobs.iter().filter(|m| m.health > 0.0 && near(m.home.unwrap_or(m.body.pos)));
        let (mut hmmers, mut clankers) = (0, 0);
        for m in alive {
            match m.kind {
                MobKind::Hmmer if m.baby <= 0.0 => hmmers += 1,
                MobKind::Clanker => clankers += 1,
                _ => {}
            }
        }
        (hmmers, clankers)
    }

    /// Has every bed in the village been claimed (and are there Hmmers to build)?
    pub fn village_full(&self, key: IVec3) -> bool {
        let (hmmers, _) = self.village_folk(key);
        let taken: HashSet<IVec3> = self.life.beds.values().copied().collect();
        hmmers >= BUILDERS && crate::village_life::nearest(&self.world, key.as_vec3(), crate::beds::is_bed, &taken).is_none()
    }

    /// Does the village have Hmmers enough for another Clanker?
    pub fn wants_clanker(&self, key: IVec3) -> bool {
        let (hmmers, clankers) = self.village_folk(key);
        hmmers / HMMERS_PER_CLANKER > clankers
    }

    /// The next spot with room for a house, if any is left.
    fn start_house(&mut self, site: &Site, v: &mut Village) -> Option<Build> {
        let spots = self.growth_spots(site);
        while (v.next as usize) < spots.len() {
            let i = v.next as usize;
            v.next += 1;
            let (at, _, _) = spots[i];
            if self.spot_is_clear(at) {
                return Some(Build { spot: i as u8, step: 0 });
            }
        }
        None
    }

    /// Is there nothing anyone's built where a house would go (and is it loaded)?
    fn spot_is_clear(&self, at: IVec3) -> bool {
        for x in -4..=4 {
            for z in -4..=4 {
                if !self.world.is_loaded(at.x + x, at.z + z) {
                    return false;
                }
                for y in 1..=8 {
                    if !clear_enough(self.world.get_v(at + IVec3::new(x, y, z))) {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// A few more blocks on the house going up; `None` once it's finished.
    fn build_house(&mut self, site: &Site, b: Build) -> Option<Build> {
        let spots = self.growth_spots(site);
        let &(at, facing, seed) = spots.get(b.spot as usize)?;
        let plan = house_plan(at, facing, seed, self.growth_style(site));
        let mut step = b.step as usize;
        let mut placed = 0;
        while step < plan.len() && placed < BLOCKS_PER_LOOK {
            let (p, id) = plan[step];
            if !self.world.is_loaded(p.x, p.z) {
                // Wait for it to load.
                return Some(Build { spot: b.spot, step: step as u32 });
            }
            step += 1;
            let was = self.world.get_v(p);
            // (Clearing the sky over it takes trees, never anyone's building.)
            if was == id || id == AIR && !clear_enough(was) {
                continue;
            }
            self.world.set_v(p, id);
            if id != AIR {
                placed += 1;
            }
        }
        let last = plan.get(step.saturating_sub(1)).map(|b| b.0.as_vec3() + Vec3::splat(0.5));
        if placed > 0 && let Some(last) = last {
            self.sfx(crate::sound::Sfx::Place(crate::sound::Mat::Wood), Some(last));
            if self.rng.chance(0.3) {
                self.smoke(last, 2, 0.3);
            }
        }
        (step < plan.len()).then_some(Build { spot: b.spot, step: step as u32 })
    }

    /// A house is finished: everyone about hears of it.
    fn house_done(&mut self, key: IVec3) {
        let place = self.place_of(key.as_vec3());
        self.report(crate::paper::Event::House, &place, "", 1);
        for who in self.players_near(key.as_vec3()) {
            self.tell(&who, "The village has finished a new house. (Somebody's moving in.)");
            self.advance_for(&who, "new_neighbours");
        }
    }

    /// The village builds itself a Clanker on the square.
    pub fn build_clanker(&mut self, key: IVec3) {
        let at = key.as_vec3() + Vec3::new(-1.5, 0.0, -1.5);
        let place = self.place_of(key.as_vec3());
        self.report(crate::paper::Event::Clanker, &place, "", 1);
        self.world.new_clankers.push(at);
        self.house_clankers();
        self.smoke(at + Vec3::Y, 12, 0.6);
        self.sfx(crate::sound::Sfx::Thud, Some(at));
        for who in self.players_near(key.as_vec3()) {
            self.tell(&who, "The village has built itself a new Clanker. (Some assembly was required.)");
            self.advance_for(&who, "some_assembly_required");
        }
    }

    /// Players (by record key) near a village square.
    fn players_near(&self, at: Vec3) -> Vec<String> {
        let near = self.player_spots_by_id().into_iter().filter(|(_, p)| p.distance(at) < WATCH);
        near.filter_map(|(id, _)| self.player_name_of(id)).map(|n| crate::players::record_key(&n)).collect()
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// A world with a real village (as the generator placed it), loaded, with us on its square.
    /// Returns the game and the village's key (its square, as `village_key` gives it).
    pub fn village_with_square(seed: u32) -> (Game, IVec3) {
        let mut g = Game::new(seed, false, false);
        g.world = crate::world::World::new(seed);
        g.ready = true;
        g.spawn_timer = 1e9;
        g.rules.difficulty = crate::rules::Difficulty::Peaceful;
        let o = g.world.nearest_site(Kind::Village, Vec3::ZERO, 40).expect("a village");
        let (cx, cz) = (o.x.div_euclid(crate::world::CW), o.z.div_euclid(crate::world::CW));
        for dz in -2..=2 {
            for dx in -2..=2 {
                g.world.load_now(cx + dx, cz + dz);
            }
        }
        g.player.body.pos = o.as_vec3() + Vec3::new(-1.5, 1.0, 0.5);
        let key = g.village_key(g.player.body.pos);
        assert_eq!(key, o + IVec3::Y);
        // The village's own Clanker moves in.
        g.house_clankers();
        (g, key)
    }

    fn hmmer(g: &mut Game, at: Vec3) -> u32 {
        let id = g.alloc_mob(MobKind::Hmmer, at);
        if let Some(m) = g.mobs.iter_mut().find(|m| m.id == id) {
            m.home = Some(at);
            m.persistent = true;
        }
        id
    }

    #[test]
    fn growth_is_saved() {
        let mut gr = Growth::default();
        gr.villages.insert(IVec3::new(5, 70, -9), Village { day: 3, clanker_day: 2, next: 1, houses: 1, board: true, build: Some(Build { spot: 1, step: 40 }) });
        gr.villages.insert(IVec3::new(-50, 64, 9), Village { board: true, ..Default::default() });
        assert_eq!(Growth::decode(&gr.encode()).villages, gr.villages);
    }

    #[test]
    fn a_village_puts_up_its_board_builds_a_house_when_full_and_a_clanker_when_big() {
        let (mut g, key) = village_with_square(21);
        g.time = 0.1;
        let site = g.village_site(key).expect("the village's site");
        g.grow_village(key);
        assert_eq!(g.world.get_v(crate::structures::village_board(site.origin)), NOTICE_BOARD, "the board goes up");
        // Two Hmmers and no free beds: a house goes up.
        let taken: Vec<IVec3> = {
            let mut v = Vec::new();
            let mut taken = HashSet::new();
            while let Some(b) = crate::village_life::nearest(&g.world, key.as_vec3(), crate::beds::is_bed, &taken) {
                taken.insert(b);
                v.push(b);
            }
            v
        };
        for (i, b) in taken.iter().enumerate() {
            g.life.beds.insert(10_000 + i as u32, *b);
        }
        assert!(!g.village_full(key), "nobody to build it yet");
        let a = hmmer(&mut g, key.as_vec3() + Vec3::new(1.5, 0.0, 4.5));
        let b = hmmer(&mut g, key.as_vec3() + Vec3::new(-1.5, 0.0, 4.5));
        assert!(g.village_full(key));
        g.day = 1;
        g.grow_village(key);
        let build = g.growth.villages[&key].build.expect("a house is started");
        let (at, facing, seed) = g.growth_spots(&site)[build.spot as usize];
        // Only one a day.
        g.grow_village(key);
        let mut looks = 2;
        while g.growth.villages[&key].build.is_some() {
            g.grow_village(key);
            looks += 1;
            assert!(looks < 2000, "it finishes");
        }
        assert_eq!(g.growth.villages[&key].houses, 1);
        assert!(g.advancements.earned.iter().any(|a| a == "new_neighbours"));
        assert!(looks > 30, "it takes a while ({looks} looks)");
        for (p, id) in house_plan(at, facing, seed, g.growth_style(&site)) {
            // (Glass panes join up with their neighbours as they go in.)
            let got = g.world.get_v(p);
            assert!(got == id || crate::carpentry::is_pane(got) && crate::carpentry::is_pane(id), "{p} is built: {} not {}", block(got).key, block(id).key);
        }
        // It has a bed, and a workstation.
        let plan = house_plan(at, facing, seed, g.growth_style(&site));
        assert!(plan.iter().any(|(_, id)| crate::beds::is_bed(*id)));
        assert!(plan.iter().any(|(_, id)| crate::village_life::station_job(*id).is_some()));
        // A Clanker for every six Hmmers (the one it started with counts).
        let clankers = |g: &Game| g.mobs.iter().filter(|m| m.kind == MobKind::Clanker && m.health > 0.0).count();
        assert!(!g.wants_clanker(key));
        let need = HMMERS_PER_CLANKER * (clankers(&g) + 1);
        for i in 2..need {
            hmmer(&mut g, key.as_vec3() + Vec3::new(i as f32 * 0.5 - 4.0, 0.0, -5.5));
            assert_eq!(g.wants_clanker(key), i + 1 == need);
        }
        let before = clankers(&g);
        g.day = 2;
        g.grow_village(key);
        assert_eq!(clankers(&g), before + 1);
        assert!(g.advancements.earned.iter().any(|a| a == "some_assembly_required"));
        assert!(!g.wants_clanker(key), "one each is enough");
        let _ = (a, b);
    }

    #[test]
    fn houses_go_up_over_the_wilds_but_not_over_anyones_building() {
        for id in [AIR, TALL_GRASS, LEAF_LITTER, WILDFLOWERS, LOG, GRASS, DIRT, SAND, FLOWER, DANDELION] {
            assert!(clear_enough(id), "{} is in the way", block(id).key);
        }
        for id in [PLANKS, COBBLE, BRICK, TORCH, CHEST, crate::beds::bed(0), GLASS, RAIL_FIRST] {
            assert!(!clear_enough(id), "{} would be built over", block(id).key);
        }
    }

    #[test]
    fn a_village_leaves_someone_elses_building_alone() {
        let (mut g, key) = village_with_square(21);
        let site = g.village_site(key).unwrap();
        let spots = g.growth_spots(&site);
        // Somebody's built a wall on every spot.
        for (at, _, _) in &spots {
            g.world.set_v(*at + IVec3::new(1, 2, 1), BRICK);
        }
        let mut v = Village::default();
        assert_eq!(g.start_house(&site, &mut v), None);
        assert_eq!(v.next as usize, spots.len(), "it gave up on all of them");
    }
}
