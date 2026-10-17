//! Gateways, and the Wyrm's return.
//!
//! - Each time the Hollow Wyrm is beaten a **Gateway** opens on the main
//!   island (up to eight, round its edge): a two-block-tall slice of starry
//!   void in an obsidian frame. Walk in and you come out on an outer island
//!   out that way, by a Gateway of its own that takes you back.
//! - Once the portal home is open, four **Wyrm Crystals** (four Star
//!   Fragments, four glass and a Staring Eye) set round it, one on each
//!   side, call the Wyrm back: the crystals on the pillars grow again, the
//!   portal home closes, and it's round two.
//!
//! All of it happens where the world lives; joined players walk into a
//! Gateway as into a portal (`Msg::UsePortal`).

use crate::block::*;
use crate::dims::Dim;
use crate::game::Game;
use crate::hollow::{arrival, exit_open, outer_island, pillars, GEN_ORIGIN, ISLAND, ORIGIN};
use crate::sound::Sfx;
use crate::world::{World, CW};
use macroquad::math::{ivec3, IVec3, Vec3};

/// How many Gateways the main island can have, and how far out they stand.
pub const GATEWAYS: usize = 8;
const RING: f32 = 46.0;
/// How far out a Gateway sends you.
const REACH: f32 = 320.0;

/// Which way Gateway `k` faces out from the middle.
fn dir(k: usize) -> Vec3 {
    let a = k as f32 * std::f32::consts::TAU / GATEWAYS as f32 + 0.35;
    Vec3::new(a.cos(), 0.0, a.sin())
}

/// The column Gateway `k` stands in on the main island.
pub fn ring_column(k: usize) -> (i32, i32) {
    let d = dir(k) * RING;
    (ORIGIN.x + d.x.round() as i32, ORIGIN.z + d.z.round() as i32)
}

/// The lower Gateway block in column (x, z), if one's there.
fn gateway_in(world: &World, x: i32, z: i32) -> Option<IVec3> {
    (ORIGIN.y - 12..ORIGIN.y + 24).map(|y| ivec3(x, y, z)).find(|&p| world.get_v(p) == GATEWAY && world.get_v(p - IVec3::Y) != GATEWAY)
}

/// How many of the main island's Gateways are open.
pub fn gateways_open(world: &World) -> usize {
    (0..GATEWAYS).filter(|&k| {
        let (x, z) = ring_column(k);
        gateway_in(world, x, z).is_some()
    }).count()
}

/// A Gateway standing on the ground at `foot` (the block it stands on).
pub fn gateway_blocks(foot: IVec3) -> Vec<(IVec3, Id)> {
    let mut v = vec![(foot, OBSIDIAN), (foot + IVec3::Y, GATEWAY), (foot + IVec3::Y * 2, GATEWAY), (foot + IVec3::Y * 3, OBSIDIAN)];
    for (dx, dz) in [(-1, -1), (1, -1), (-1, 1), (1, 1)] {
        for dy in 0..=3 {
            v.push((foot + ivec3(dx, dy, dz), OBSIDIAN));
        }
    }
    v
}

/// The highest solid block in column (x, z) between `lo` and `hi`.
fn ground(world: &World, x: i32, z: i32, lo: i32, hi: i32) -> Option<i32> {
    (lo..hi).rev().find(|&y| is_solid(world.get(x, y, z)) && world.get(x, y, z) != GATEWAY)
}

impl Game {
    /// The Wyrm was beaten: open the next Gateway (if there's room for one).
    pub fn open_gateway(&mut self) -> bool {
        let Some(k) = (0..GATEWAYS).find(|&k| {
            let (x, z) = ring_column(k);
            gateway_in(&self.world, x, z).is_none()
        }) else {
            return false;
        };
        let (x, z) = ring_column(k);
        let top = ground(&self.world, x, z, ORIGIN.y - 20, ORIGIN.y + 10).unwrap_or(ORIGIN.y);
        for (p, id) in gateway_blocks(ivec3(x, top, z)) {
            self.world.set_v(p, id);
        }
        self.msg("A Gateway opens at the edge of the island.");
        true
    }

    /// Where a Gateway at `at` takes you, in the Hollow (where the world lives).
    pub fn gateway_destination(&mut self, at: IVec3) -> Vec3 {
        let flat = Vec3::new((at.x - ORIGIN.x) as f32, 0.0, (at.z - ORIGIN.z) as f32);
        if flat.length() > ISLAND + 24.0 {
            // Out on an island: back to the main one.
            return arrival();
        }
        let way = flat.normalize_or(Vec3::X);
        // The outer island nearest that far out, that way (in generator coordinates).
        let seed = self.world.seed();
        let aim = GEN_ORIGIN.as_vec3() + way * REACH;
        let cell = |v: f32, o: i32| (v as i32 - o).div_euclid(80);
        let (gx, gz) = (cell(aim.x, GEN_ORIGIN.x), cell(aim.z, GEN_ORIGIN.z));
        let island = (gz - 3..=gz + 3)
            .flat_map(|z| (gx - 3..=gx + 3).map(move |x| (x, z)))
            .filter_map(|(x, z)| outer_island(seed, x, z))
            .min_by_key(|(c, _, _)| (c.as_vec3() - aim).length() as i32);
        let Some((c, r, _)) = island else { return arrival() };
        let back = ivec3(Dim::Hollow.gen_x(), 0, 0);
        // Somewhere off its middle (clear of a spire or a tower), toward home.
        let home = -way;
        let stand = c + ivec3((home.x * (r as f32 * 0.5)) as i32, 0, (home.z * (r as f32 * 0.5)) as i32);
        let near = c + ivec3((home.x * (r as f32 * 0.5 + 3.0)) as i32, 0, (home.z * (r as f32 * 0.5 + 3.0)) as i32);
        let (stand, near) = (stand - back, near - back);
        let (cx, cz) = (stand.x.div_euclid(CW), stand.z.div_euclid(CW));
        for dz in -2..=2 {
            for dx in -2..=2 {
                self.world.load_now(cx + dx, cz + dz);
            }
        }
        // A Gateway home on the island, if there isn't one yet.
        if gateway_in(&self.world, near.x, near.z).is_none()
            && let Some(top) = ground(&self.world, near.x, near.z, c.y - 12, c.y + 12)
        {
            for (p, id) in gateway_blocks(ivec3(near.x, top, near.z)) {
                self.world.set_v(p, id);
            }
        }
        let top = ground(&self.world, stand.x, stand.z, c.y - 12, c.y + 40).unwrap_or(c.y);
        Vec3::new(stand.x as f32 + 0.5, top as f32 + 1.0, stand.z as f32 + 0.5)
    }

    /// A Wyrm Crystal went down at `p`: four round the portal home call the Wyrm back.
    pub fn try_resummon_wyrm(&mut self, p: IVec3, who: &str) -> bool {
        if !self.world.is_hollow() || !exit_open(&self.world) {
            return false;
        }
        let spots = summon_spots();
        if !spots.contains(&p) || !spots.iter().all(|&s| self.world.get_v(s) == WYRM_CRYSTAL) {
            return false;
        }
        for s in spots {
            self.world.set_v(s, AIR);
        }
        // The pillars' crystals grow back, and the way home shuts.
        for (x, z, h) in pillars(self.world.seed()) {
            self.world.set_v(ivec3(x - GEN_ORIGIN.x, h, z), WYRM_CRYSTAL);
        }
        for z in -1..=1 {
            for x in -1..=1 {
                self.world.set_v(ORIGIN + ivec3(x, 1, z), AIR);
            }
        }
        self.sfx(Sfx::Roar, Some(ORIGIN.as_vec3()));
        let t = "The crystals hum. Something big is coming back.";
        self.msg(t);
        self.system_message(None, t);
        self.advance_for(who, "round_two");
        true
    }
}

/// Where the four crystals go: one on each side of the portal home.
pub fn summon_spots() -> [IVec3; 4] {
    [ORIGIN + ivec3(2, 1, 0), ORIGIN + ivec3(-2, 1, 0), ORIGIN + ivec3(0, 1, 2), ORIGIN + ivec3(0, 1, -2)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::GenOptions;

    fn in_hollow(seed: u32) -> Game {
        let mut g = Game::new_with(seed, false, false, GenOptions::DEFAULT);
        g.ready = true;
        let (dim, to) = g.hollow_destination(IVec3::ZERO);
        g.move_local_player(dim, to);
        for dz in -5..=5 {
            for dx in -5..=5 {
                g.world.load_now(dx, dz);
            }
        }
        g
    }

    #[test]
    fn beating_the_wyrm_opens_gateways_that_go_out_and_back() {
        let mut g = in_hollow(31);
        assert_eq!(gateways_open(&g.world), 0);
        g.wyrm_defeated(ORIGIN.as_vec3());
        assert_eq!(gateways_open(&g.world), 1);
        assert_eq!(g.world.get_v(ORIGIN + ivec3(0, 2, 3)), WYRM_EGG, "the egg, the first time");
        g.world.set_v(ORIGIN + ivec3(0, 2, 3), AIR);
        g.wyrm_defeated(ORIGIN.as_vec3());
        assert_eq!(gateways_open(&g.world), 2);
        assert_ne!(g.world.get_v(ORIGIN + ivec3(0, 2, 3)), WYRM_EGG, "only the once");
        // Through the first: out to an island, by a Gateway home.
        let (x, z) = ring_column(0);
        let at = gateway_in(&g.world, x, z).unwrap();
        let out = g.gateway_destination(at);
        assert!(Vec3::new(out.x, 0.0, out.z).length() > ISLAND + 40.0, "{out}");
        let feet = out.floor().as_ivec3();
        assert!(is_solid(g.world.get_v(feet - IVec3::Y)) && !is_solid(g.world.get_v(feet)) && !is_solid(g.world.get_v(feet + IVec3::Y)));
        let home = (-8..=8).flat_map(|dz| (-8..=8).map(move |dx| (dx, dz))).find_map(|(dx, dz)| gateway_in(&g.world, feet.x + dx, feet.z + dz));
        let home = home.expect("a Gateway back");
        assert_eq!(g.gateway_destination(home), arrival());
    }

    #[test]
    fn four_crystals_call_the_wyrm_back() {
        let mut g = in_hollow(32);
        g.wyrm_defeated(ORIGIN.as_vec3());
        assert!(exit_open(&g.world));
        let spots = summon_spots();
        for (i, &s) in spots.iter().enumerate() {
            g.world.set_v(s, WYRM_CRYSTAL);
            let done = g.try_resummon_wyrm(s, "");
            assert_eq!(done, i == 3);
        }
        assert!(!exit_open(&g.world));
        assert!(spots.iter().all(|&s| g.world.get_v(s) == AIR));
        let (x, z, h) = pillars(g.world.seed())[0];
        assert_eq!(g.world.get_v(ivec3(x - GEN_ORIGIN.x, h, z)), WYRM_CRYSTAL);
        // And it turns up.
        g.mobs.clear();
        g.hollow_tick(0.1);
        assert!(g.mobs.iter().any(|m| m.kind == crate::entity::MobKind::Wyrm));
    }
}
