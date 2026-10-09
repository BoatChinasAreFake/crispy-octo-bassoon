//! Snout Bastions: big gilded ruins out over the Scorchlands' lava sea.
//!
//! A square keep of Blackstone Bricks (cracked here and there, gilded now
//! and then, crumbling along the top) on pillars down into the lava, with a
//! gate on the south side, ramparts round the inside, a staircase up, and:
//!
//! - **The treasure room** in the middle: a gilded floor, a core of Magma
//!   Blocks crowned with gold, and the treasure chest (Scorchite, diamonds,
//!   an Upgrade Template, often an enchanted something).
//! - **Tusker pens** in the two back corners, floored with Crimson Nylium,
//!   with Tuskers in them (see beasts.rs).
//! - Chests in the courtyard and up on the ramparts, and gold lying about.
//! - **Residents**: Snouts in the courtyard, and **Snout Brutes** at the gate
//!   and in the treasure room. Brutes are always cross and can't be bought
//!   off; Snouts are fine until you open their chests or take their gold.
//!
//! Residents move in the first time someone comes near in a session (where
//! the world lives), so a bastion is never empty and never endlessly full.

use crate::block::*;
use crate::entity::MobKind;
use crate::game::Game;
use crate::noise::{hash2, hash3};
use crate::scorch::LAVA_SEA;
use crate::structures::{Kind, Site};
use macroquad::math::{ivec3, IVec3, Vec3};

/// The courtyard floor's height (blocks stand on the layer under it).
pub const BASTION_Y: i32 = LAVA_SEA + 6;
/// Half the width of the keep, and the height of its walls.
pub const R: i32 = 16;
const H: i32 = 13;
/// How far off people move in when someone comes near.
const NEAR: f32 = 48.0;

/// Where the treasure chest is.
pub fn treasure_chest(site: &Site) -> IVec3 {
    site.origin + ivec3(0, 0, -1)
}

/// The treasure room's middle (for the advancement, and its Brute).
pub fn treasure_room(site: &Site) -> IVec3 {
    site.origin + ivec3(0, 0, -4)
}

/// Every block of a bastion (the gate faces south).
pub fn bastion_blocks(site: &Site) -> Vec<(IVec3, Id)> {
    let o = site.origin;
    let s = site.seed;
    let mut out = Vec::new();
    let mut put = |x: i32, y: i32, z: i32, id: Id| out.push((o + ivec3(x, y, z), id));
    // Old masonry: mostly bricks, some cracked, a little gilded or plain.
    let brick = |x: i32, y: i32, z: i32| -> Id {
        let h = hash3(s, x, y, z);
        if h < 0.04 {
            GILDED_BLACKSTONE
        } else if h < 0.22 {
            CRACKED_BLACKSTONE_BRICKS
        } else if h < 0.3 {
            BLACKSTONE
        } else {
            BLACKSTONE_BRICKS
        }
    };
    for x in -R..=R {
        for z in -R..=R {
            let edge = x.abs() == R || z.abs() == R;
            put(x, -1, z, brick(x, -1, z));
            // Pillars down into the lava.
            if x % 4 == 0 && z % 4 == 0 && (edge || (x.abs() % 8 == 0 && z.abs() % 8 == 0)) {
                for y in (LAVA_SEA - 3 - BASTION_Y)..-1 {
                    put(x, y, z, BLACKSTONE);
                }
            }
            // The wall's top crumbles away unevenly.
            let top = H - (hash2(s ^ 7, x, z) * 4.0) as i32;
            for y in 0..=H {
                let gate = z == R && x.abs() <= 2 && y <= 5;
                let lamp = edge && y == 4 && (x + z).rem_euclid(8) == 0;
                let id = if !edge || gate || y > top {
                    AIR
                } else if lamp {
                    GLOWROCK
                } else {
                    brick(x, y, z)
                };
                put(x, y, z, id);
            }
            // Ramparts: a walkway round the inside, two wide (over the gate too).
            if !edge && (x.abs() >= R - 2 || z.abs() >= R - 2) {
                put(x, 6, z, brick(x, 6, z));
            }
        }
    }
    // A way out through the rock beyond the gate.
    for z in R + 1..R + 9 {
        for x in -2..=2 {
            put(x, -1, z, brick(x, -1, z));
            for y in 0..5 {
                put(x, y, z, AIR);
            }
        }
    }
    // Steps up to the ramparts, along the west wall.
    for k in 0..6 {
        for y in 0..=k {
            for x in [-(R - 3), -(R - 4)] {
                put(x, y, 6 - k, brick(x, y, 6 - k));
            }
        }
    }
    // The treasure room.
    for x in -5..=5i32 {
        for z in -9..=1i32 {
            put(x, -1, z, if (x + z).rem_euclid(2) == 0 { GILDED_BLACKSTONE } else { BLACKSTONE_BRICKS });
            for y in 0..=8 {
                let wall = x.abs() == 5 || z == -9 || z == 1 || y == 8;
                let door = z == 1 && x.abs() <= 1 && y <= 3;
                let id = if !wall || door {
                    AIR
                } else if y == 5 && (x.abs() == 5 || z == -9) && (x + z).rem_euclid(3) == 0 {
                    SHROOMLIGHT
                } else {
                    brick(x, y + 40, z)
                };
                put(x, y, z, id);
            }
        }
    }
    // Its core: magma under a crown of gold, and gold in the corners.
    for x in -1..=1 {
        for z in -5..=-3 {
            put(x, 0, z, MAGMA_BLOCK);
            put(x, 1, z, MAGMA_BLOCK);
            put(x, 2, z, GOLD_BLOCK);
        }
    }
    for (x, z) in [(-4, -8), (4, -8), (-4, 0), (4, 0)] {
        put(x, 0, z, GOLD_BLOCK);
    }
    let t = treasure_chest(site) - o;
    put(t.x, t.y, t.z, CHEST);
    // Tusker pens in the back corners.
    for side in [-1, 1] {
        for a in 8..=14 {
            for z in -14..=-8 {
                let x = a * side;
                let fence = a == 8 || a == 14 || z == -14 || z == -8;
                put(x, -1, z, CRIMSON_NYLIUM);
                put(x, 0, z, if fence { BLACKSTONE_BRICKS } else if hash3(s ^ 3, x, 0, z) < 0.15 { CRIMSON_ROOTS } else { AIR });
                put(x, 1, z, if fence && (a + z) % 2 == 0 { BLACKSTONE_BRICKS } else { AIR });
            }
        }
    }
    // Chests in the courtyard and on the ramparts, and gold lying about.
    put(12, 0, 10, CHEST);
    put(-12, 0, 10, CHEST);
    put(0, 7, -(R - 1), CHEST);
    for (x, z) in [(9, 3), (-10, 2), (7, 12), (-6, 13)] {
        if hash3(s ^ 5, x, 0, z) < 0.7 {
            put(x, 0, z, GOLD_BLOCK);
        }
    }
    out
}

/// Who lives in a bastion, and where (relative to its origin).
pub fn residents(site: &Site) -> Vec<(MobKind, IVec3)> {
    let mut v = vec![
        (MobKind::Snout, ivec3(6, 0, 6)),
        (MobKind::Snout, ivec3(-6, 0, 7)),
        (MobKind::Snout, ivec3(10, 0, 1)),
        (MobKind::Snout, ivec3(-10, 7, -(R - 1))),
        (MobKind::SnoutBrute, ivec3(3, 0, R - 2)),
        (MobKind::SnoutBrute, ivec3(-3, 0, R - 2)),
        (MobKind::SnoutBrute, treasure_room(site) - site.origin + ivec3(3, 0, -2)),
    ];
    for side in [-1, 1] {
        v.push((MobKind::Tusker, ivec3(11 * side, 0, -11)));
        if site.seed.is_multiple_of(2) {
            v.push((MobKind::Tusker, ivec3(10 * side, 0, -10)));
        }
    }
    v
}

impl Game {
    /// Move the residents into any bastion someone's come near (where the world lives).
    pub fn bastions_tick(&mut self, dt: f32) {
        if self.is_client() || !self.rules.difficulty.monsters() {
            return;
        }
        self.bastion_timer -= dt;
        if self.bastion_timer > 0.0 {
            return;
        }
        self.bastion_timer = 2.0;
        for (_, at, _) in self.player_spots() {
            if !self.world.is_scorch() {
                continue;
            }
            let Some(o) = self.world.nearest_site(Kind::Bastion, at, 3) else { continue };
            if self.bastions_peopled.contains(&o) || o.as_vec3().distance(at) > NEAR || !self.world.is_loaded(o.x, o.z) {
                continue;
            }
            self.bastions_peopled.insert(o);
            let (cx, cz) = (o.x.div_euclid(crate::world::CW), o.z.div_euclid(crate::world::CW));
            let Some(site) = (-2..=2).flat_map(|dz| (-2..=2).map(move |dx| (dx, dz))).filter_map(|(dx, dz)| self.world.site(cx + dx, cz + dz)).find(|s| s.kind == Kind::Bastion && s.origin == o) else { continue };
            for (kind, p) in residents(&site) {
                let pos = (o + p).as_vec3() + Vec3::new(0.5, 0.0, 0.5);
                if is_solid(self.world.get_v(o + p)) {
                    continue;
                }
                let id = self.alloc_mob(kind, pos);
                if let Some(m) = self.mobs.iter_mut().find(|m| m.id == id) {
                    m.home = Some(pos);
                }
            }
        }
    }

    /// Opening a chest anywhere near Snouts makes them cross (they're touchy about their things).
    pub fn snouts_see_chest_opened(&mut self, pos: IVec3) {
        if self.is_client() || self.world.get_v(pos) != CHEST {
            return;
        }
        let at = pos.as_vec3();
        for m in self.mobs.iter_mut().filter(|m| m.kind == MobKind::Snout && m.body.pos.distance(at) < 16.0) {
            m.angry = true;
            m.seed = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scorch::SCORCH_X;

    fn site() -> Site {
        Site { kind: Kind::Bastion, origin: ivec3(SCORCH_X + 600, BASTION_Y, 40), facing: 0, seed: 12 }
    }

    #[test]
    fn a_bastion_has_a_treasure_room_pens_and_chests() {
        let s = site();
        let b = bastion_blocks(&s);
        let at = |p: IVec3| b.iter().rev().find(|x| x.0 == p).map(|x| x.1);
        assert_eq!(at(treasure_chest(&s)), Some(CHEST));
        assert!(b.iter().filter(|x| x.1 == CHEST).count() >= 4);
        assert!(b.iter().filter(|x| x.1 == MAGMA_BLOCK).count() >= 9);
        assert!(b.iter().filter(|x| x.1 == CRIMSON_NYLIUM).count() >= 60, "two pens");
        assert!(b.iter().any(|x| x.1 == GILDED_BLACKSTONE));
        // The gate's open, and the way in from it.
        assert_eq!(at(s.origin + ivec3(0, 1, R)), Some(AIR));
        // Everyone has somewhere to stand.
        for (_, p) in residents(&s) {
            assert!(at(s.origin + p).is_some_and(|id| !is_solid(id)), "{p} is clear");
            assert!(at(s.origin + p - IVec3::Y).is_some_and(is_solid), "{p} has a floor");
        }
    }

    #[test]
    fn bastions_turn_up_in_the_scorchlands_and_not_on_top_of_each_other() {
        let g = crate::world::Generator::with_dim(31, crate::world::GenOptions::LEGACY, crate::dims::Dim::Scorch);
        let mut found = Vec::new();
        for cz in -150..150 {
            for cx in 0..150 {
                let cx = cx - 75;
                if let Some(s) = g.site(cx, cz).filter(|s| s.kind == Kind::Bastion) {
                    found.push(s.origin);
                }
            }
        }
        assert!(found.len() >= 3, "{} bastions", found.len());
        for (i, a) in found.iter().enumerate() {
            for b in &found[i + 1..] {
                assert!(a.as_vec3().distance(b.as_vec3()) > 40.0, "{a} and {b}");
            }
        }
    }

    #[test]
    fn residents_take_their_places_and_mind_their_chests() {
        let mut g = crate::game::tests::arena(191);
        let s = site();
        let b = bastion_blocks(&s);
        let (cx, cz) = (s.origin.x.div_euclid(16), s.origin.z.div_euclid(16));
        for dz in -2..=2 {
            for dx in -2..=2 {
                g.world.load_now(cx + dx, cz + dz);
            }
        }
        for (p, id) in b {
            g.world.set_v(p, id);
        }
        // (The arena's generator doesn't know about this site, so place the residents by hand.)
        for (kind, p) in residents(&s) {
            g.alloc_mob(kind, (s.origin + p).as_vec3() + Vec3::new(0.5, 0.0, 0.5));
        }
        assert!(g.mobs.iter().filter(|m| m.kind == MobKind::SnoutBrute).count() >= 3);
        assert!(g.mobs.iter().filter(|m| m.kind == MobKind::Tusker).count() >= 2);
        // Opening the treasure chest riles the Snouts.
        g.snouts_see_chest_opened(treasure_chest(&s));
        assert!(g.mobs.iter().filter(|m| m.kind == MobKind::Snout && m.body.pos.distance(treasure_chest(&s).as_vec3()) < 16.0).all(|m| m.angry));
    }
}
