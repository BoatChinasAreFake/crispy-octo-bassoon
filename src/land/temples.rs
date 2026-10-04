//! Temples, mineshafts and igloos: four more things the generator builds.
//!
//! - **Desert pyramids**: a stepped sandstone pyramid with a hall inside.
//!   Under the blue tile in the middle of the floor, a shaft drops into a
//!   hidden room with four chests round a pressure plate. The plate is wired
//!   to the TNT under it. Don't land on it.
//! - **Jungle temples**: mossy cobblestone, two floors and a basement. The
//!   basement corridor has a tripwire across it, and dispensers in its walls
//!   full of arrows; its chests are at the end (see tripwire.rs).
//! - **Abandoned mineshafts**: plank-floored tunnels deep underground, held
//!   up by fence-and-plank supports, running out from a dirt room. Rails
//!   (some missing), cobwebs in the corners, now and then a Webber cage in a
//!   nest of webs, and **Minecarts with Chests** of loot parked on the rails.
//! - **Igloos**: a snow dome on the snowy plains with a bed, a furnace and a
//!   crafting table. Some have a trapdoor in the floor and a ladder down to a
//!   basement: a brewing stand, a chest with a Golden Chop in it, and two
//!   cells, one holding a Hmmer and the other a Zombie Hmmer. (A Golden Chop
//!   cures a Zombie Hmmer; see villagers.rs.)
//!
//! **Cobwebs** slow anything caught in them to a crawl (a sword or shears
//! cut them quickly, and they drop string).

use crate::block::*;
use crate::noise::{hash2, hash3};
use crate::structures::Site;
use crate::world::World;
use macroquad::math::{ivec3, IVec3, Vec3};

/// Mineshafts' tunnels run this far out from the middle room.
pub const SHAFT_REACH: i32 = 22;

/// Is a body in this box caught in a cobweb?
pub fn in_cobweb(world: &World, min: Vec3, max: Vec3) -> bool {
    let (x0, x1) = (min.x.floor() as i32, (max.x - 1e-3).floor() as i32);
    let (y0, y1) = (min.y.floor() as i32, (max.y - 1e-3).floor() as i32);
    let (z0, z1) = (min.z.floor() as i32, (max.z - 1e-3).floor() as i32);
    (y0..=y1).any(|y| (z0..=z1).any(|z| (x0..=x1).any(|x| world.get(x, y, z) == COBWEB)))
}

/// A desert pyramid (see the top of this file), its entrance on the south side.
pub fn pyramid_blocks(site: &Site) -> Vec<(IVec3, Id)> {
    let o = site.origin;
    let s = site.seed;
    let mut out = Vec::new();
    let mut put = |x: i32, y: i32, z: i32, id: Id| out.push((o + ivec3(x, y, z), id));
    // A footing, and clear sky.
    for x in -8..=8 {
        for z in -8..=8 {
            for y in -4..0 {
                put(x, y, z, SANDSTONE);
            }
            for y in 1..=12 {
                put(x, y, z, AIR);
            }
        }
    }
    // The steps: each layer one smaller than the one under it.
    for k in 0..=7i32 {
        let r = 7 - k;
        for x in -r..=r {
            for z in -r..=r {
                let edge = x.abs() == r || z.abs() == r;
                // Orange bands round the outside.
                let id = if edge && k % 3 == 2 { TERRACOTTA + 1 } else { SANDSTONE };
                put(x, k, z, id);
            }
        }
    }
    put(0, 8, 0, CHISELED_SANDSTONE);
    // The hall inside, narrowing as it rises.
    for y in 1..=4 {
        let r = 6 - y;
        for x in -r..=r {
            for z in -r..=r {
                put(x, y, z, AIR);
            }
        }
    }
    // Pillars of chiseled sandstone in the hall's corners.
    for (x, z) in [(-4, -4), (4, -4), (-4, 4), (4, 4)] {
        put(x, 1, z, CHISELED_SANDSTONE);
    }
    // The entrance: a passage in from the south.
    for z in 4..=7 {
        for x in -1..=1 {
            for y in 1..=2 {
                put(x, y, z, AIR);
            }
        }
    }
    // A patterned floor: orange rings round a blue tile, which hides the shaft.
    for x in -5..=5i32 {
        for z in -5..=5i32 {
            let ring = x.abs().max(z.abs());
            let id = match ring {
                0 => GLAZED_FIRST + 6,
                1 | 3 => TERRACOTTA + 1,
                _ => SANDSTONE,
            };
            put(x, 0, z, id);
        }
    }
    // The hidden room: chests round a pressure plate, TNT under it.
    for x in -3..=3i32 {
        for z in -3..=3i32 {
            for y in -13..=-9 {
                let wall = x.abs() == 3 || z.abs() == 3 || y == -13 || y == -9 || y == -12;
                put(x, y, z, if wall { SANDSTONE } else { AIR });
            }
        }
    }
    for y in -9..=-1 {
        put(0, y, 0, AIR);
    }
    for x in -1..=1 {
        for z in -1..=1 {
            put(x, -13, z, TNT);
        }
    }
    put(0, -12, 0, TNT);
    put(0, -11, 0, PLATE);
    for (x, z) in [(2, 0), (-2, 0), (0, 2), (0, -2)] {
        put(x, -11, z, CHEST);
    }
    // A few bones and broken pots on the floor, and the odd chiseled stone.
    for (x, z) in [(-2, -2), (2, 2)] {
        if hash2(s, x, z) < 0.6 {
            put(x, -11, z, CHISELED_SANDSTONE);
        }
    }
    out
}

/// A jungle temple (see the top of this file), its door on the south side.
pub fn jungle_temple_blocks(site: &Site) -> Vec<(IVec3, Id)> {
    let o = site.origin;
    let s = site.seed;
    let mut out = Vec::new();
    let mut put = |x: i32, y: i32, z: i32, id: Id| out.push((o + ivec3(x, y, z), id));
    let stone = |x: i32, y: i32, z: i32| if hash3(s, x, y, z) < 0.45 { MOSSY_COBBLE } else { COBBLE };
    // A footing, and the jungle cleared away round it (and above: jungle trees are tall).
    for x in -8..=8i32 {
        for z in -9..=9i32 {
            if x.abs() <= 7 && z.abs() <= 8 {
                for y in -6..0 {
                    put(x, y, z, stone(x, y, z));
                }
            }
            for y in 1..=26 {
                put(x, y, z, AIR);
            }
        }
    }
    // Two floors and a roof inside thick walls.
    for x in -5..=5i32 {
        for z in -6..=6i32 {
            for y in 0..=8 {
                let shell = x.abs() == 5 || z.abs() == 6 || y == 0 || y == 4 || y == 8;
                put(x, y, z, if shell { stone(x, y, z) } else { AIR });
            }
        }
    }
    // A stepped roof.
    for (y, rx, rz) in [(9, 3, 4), (10, 1, 2)] {
        for x in -rx..=rx {
            for z in -rz..=rz {
                put(x, y, z, stone(x, y, z));
            }
        }
    }
    // The door, and windows upstairs.
    for x in -1..=1 {
        for y in 1..=3 {
            put(x, y, 6, AIR);
        }
    }
    for z in [-3, 0, 3] {
        put(5, 6, z, AIR);
        put(-5, 6, z, AIR);
    }
    // Up: a ladder in the north-east corner.
    for y in 1..=4 {
        put(4, y, -5, LADDER_FIRST + 1);
    }
    // The basement: two rooms either side of a wall, a corridor on the west.
    for x in -4..=4i32 {
        for z in -5..=3i32 {
            for y in -3..=-1 {
                let inner = x == -1 && z <= 1;
                put(x, y, z, if inner { stone(x, y, z) } else { AIR });
            }
        }
    }
    // Down: a ladder in the south-west corner, to the basement.
    for y in -3..=0 {
        put(-4, y, 3, LADDER_FIRST + 3);
    }
    // The trap: a tripwire across the corridor, hooks on both walls, and a
    // dispenser of arrows in each wall beside the hooks, aimed across it.
    put(-4, -3, -1, crate::tripwire::hook(3, false));
    put(-3, -3, -1, crate::tripwire::tripwire(1, false));
    put(-2, -3, -1, crate::tripwire::hook(1, false));
    put(-5, -3, -1, DISPENSER_FIRST + 1);
    put(-1, -3, -1, DISPENSER_FIRST + 3);
    // What it guards.
    put(-3, -3, -5, CHEST);
    put(3, -3, -5, CHEST);
    // Vines on the outside walls.
    for z in -6..=6 {
        for y in 1..=7 {
            if hash3(s ^ 7, 6, y, z) < 0.25 {
                put(6, y, z, JUNGLE_LEAVES);
            }
        }
    }
    out
}

/// An abandoned mineshaft: a room in the middle and tunnels out four ways,
/// each turning off sideways at its far end.
pub fn mineshaft_blocks(site: &Site) -> Vec<(IVec3, Id)> {
    let o = site.origin;
    let s = site.seed;
    let mut out = Vec::new();
    let mut put = |p: IVec3, id: Id| out.push((o + p, id));
    // The room: dirt floor, open above.
    for x in -3..=3 {
        for z in -3..=3 {
            put(ivec3(x, 0, z), DIRT);
            for y in 1..=4 {
                put(ivec3(x, y, z), AIR);
            }
        }
    }
    // A tunnel from `start` along `dir` for `len` blocks.
    let tunnel = |out: &mut Vec<(IVec3, Id)>, start: IVec3, dir: IVec3, len: i32, salt: u32| {
        let side = ivec3(dir.z, 0, -dir.x);
        let rail = if dir.x == 0 { RAIL_FIRST } else { RAIL_FIRST + 1 };
        for k in 0..len {
            let c = start + dir * k;
            for w in -1..=1 {
                let p = c + side * w;
                out.push((o + p, PLANKS));
                for y in 1..=3 {
                    out.push((o + p + IVec3::Y * y, AIR));
                }
            }
            let r = hash3(s ^ salt, c.x, k, c.z);
            // Supports every fourth block: fence posts and a plank beam.
            if k % 4 == 2 {
                for w in [-1, 1] {
                    out.push((o + c + side * w + IVec3::Y, FENCE_FIRST));
                    out.push((o + c + side * w + IVec3::Y * 2, FENCE_FIRST));
                }
                for w in -1..=1 {
                    out.push((o + c + side * w + IVec3::Y * 3, PLANKS));
                }
            } else if r < 0.12 {
                // Cobwebs in the top corners.
                out.push((o + c + side * if r < 0.06 { 1 } else { -1 } + IVec3::Y * 3, COBWEB));
            }
            // Rails down the middle, a few gone.
            if hash3(s ^ salt ^ 1, c.x, k, c.z) < 0.75 {
                out.push((o + c + IVec3::Y, rail));
            }
        }
    };
    let dirs = [IVec3::NEG_Z, IVec3::X, IVec3::Z, IVec3::NEG_X];
    let mut carts = 0;
    for (i, &dir) in dirs.iter().enumerate() {
        if i >= 2 && hash2(s, i as i32, 7) < 0.3 {
            continue;
        }
        let len = SHAFT_REACH - 4 - (hash2(s ^ 2, i as i32, 0) * 6.0) as i32;
        let start = dir * 4;
        tunnel(&mut out, start, dir, len, 0x10 + i as u32);
        // At the end, it turns off one way or the other.
        let end = start + dir * (len - 1);
        let turn = if hash2(s ^ 3, i as i32, 0) < 0.5 { ivec3(dir.z, 0, -dir.x) } else { ivec3(-dir.z, 0, dir.x) };
        tunnel(&mut out, end + turn * 2, turn, 8 + (hash2(s ^ 4, i as i32, 0) * 4.0) as i32, 0x20 + i as u32);
        // A cart of loot parked on the rails (it moves in when the chunk first loads; see world.rs).
        if hash2(s ^ 5, i as i32, 0) < 0.6 && carts < 3 {
            out.push((o + start + dir * (len / 2 + 1) + IVec3::Y, CHEST));
            carts += 1;
        }
        // Now and then, a Webber nest: a cage in a tangle of webs.
        if hash2(s ^ 6, i as i32, 0) < 0.25 {
            let nest = start + dir * (len / 3);
            out.push((o + nest + IVec3::Y, SPAWNER));
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                for y in 1..=2 {
                    if hash3(s ^ 8, dx, y, dz) < 0.7 {
                        out.push((o + nest + ivec3(dx, y, dz), COBWEB));
                    }
                }
            }
        }
    }
    out
}

/// An igloo (see the top of this file), its tunnel on the south side.
pub fn igloo_blocks(site: &Site) -> Vec<(IVec3, Id)> {
    let o = site.origin;
    let s = site.seed;
    let mut out = Vec::new();
    let mut put = |x: i32, y: i32, z: i32, id: Id| out.push((o + ivec3(x, y, z), id));
    for x in -5..=5 {
        for z in -5..=6 {
            put(x, 0, z, SNOW_BLOCK);
            for y in 1..=6 {
                put(x, y, z, AIR);
            }
        }
    }
    // The dome.
    for x in -4..=4i32 {
        for z in -4..=4i32 {
            for y in 1..=5i32 {
                let d = (x * x + z * z) as f32 / 16.0 + (y as f32 / 4.6).powi(2);
                if d <= 1.0 {
                    put(x, y, z, if d > 0.62 { SNOW_BLOCK } else { AIR });
                }
            }
        }
    }
    // The tunnel in.
    for z in 3..=5 {
        for x in -1..=1i32 {
            for y in 1..=2 {
                put(x, y, z, if x == 0 { AIR } else { SNOW_BLOCK });
            }
            put(x, 3, z, SNOW_BLOCK);
        }
    }
    put(0, 1, 5, AIR);
    put(0, 2, 5, AIR);
    // Furniture.
    put(-2, 1, -1, BED);
    put(2, 1, -1, FURNACE);
    put(2, 1, 1, TABLE);
    put(-2, 1, 1, TORCH);
    let basement = hash2(s, 1, 1) < 0.6;
    if !basement {
        return out;
    }
    // A trapdoor in the floor, a ladder down.
    put(0, 0, -2, crate::carpentry::trapdoor(0, false));
    for y in -8..=-1 {
        put(0, y, -2, LADDER_FIRST);
        for (dx, dz) in [(1, 0), (-1, 0), (0, -1), (0, 1)] {
            put(dx, y, -2 + dz, STONE_BRICKS);
        }
    }
    // The basement: stone bricks, two cells at the north end behind panes.
    for x in -4..=4i32 {
        for z in -7..=2i32 {
            for y in -10..=-6 {
                let wall = x.abs() == 4 || z == -7 || z == 2 || y == -10 || y == -6;
                put(x, y, z, if wall { STONE_BRICKS } else { AIR });
            }
        }
    }
    for y in -9..=-6 {
        put(0, y, -2, LADDER_FIRST);
    }
    for x in -3..=3i32 {
        for y in -9..=-8 {
            // Panes across the front of both cells, a wall between them.
            if x.abs() >= 2 {
                put(x, y, -5, PANE_FIRST);
            } else {
                put(x, y, -5, STONE_BRICKS);
                put(x, y, -6, STONE_BRICKS);
            }
        }
    }
    put(0, -9, 1, BREWING_STAND);
    put(-3, -9, 1, CHEST);
    put(3, -9, 1, LANTERN);
    put(0, -9, -1, COBWEB);
    out
}

/// Where an igloo basement's two residents stand (from its chest): the
/// Hmmer's cell, the Zombie Hmmer's.
pub fn igloo_cells(chest: IVec3) -> [Vec3; 2] {
    let o = (chest - ivec3(-3, -9, 1)).as_vec3();
    [o + Vec3::new(-2.5, -9.0, -5.5), o + Vec3::new(3.5, -9.0, -5.5)]
}

impl crate::game::Game {
    /// Mineshaft carts and igloo prisoners whose chunks just loaded move in.
    pub fn move_in_temples(&mut self) {
        for (at, cargo) in std::mem::take(&mut self.world.new_carts) {
            let id = self.spawn_vehicle(crate::vehicles::CHEST_CART_KIND, at, 0.0);
            if let Some(v) = self.vehicles.iter_mut().find(|v| v.id == id) {
                v.contents = Some(cargo);
            }
        }
        for (at, kind) in std::mem::take(&mut self.world.new_residents) {
            let id = self.alloc_mob(kind, at);
            if let Some(m) = self.mobs.iter_mut().find(|m| m.id == id) {
                m.persistent = true;
                m.home = Some(at);
                if kind == crate::entity::MobKind::Hmmer {
                    m.variant = 1 + (id % 8) as u8;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structures::Kind;

    /// Where a mineshaft's loot carts are: its chests (which become rails and a
    /// cart full of loot when the chunk first loads).
    fn is_cart_spot(site: &Site, p: IVec3) -> bool {
        site.kind == Kind::Mineshaft && p.y == site.origin.y + 1
    }

    fn site(kind: Kind) -> Site {
        Site { kind, origin: ivec3(0, 60, 0), facing: 0, seed: 12345 }
    }

    #[test]
    fn the_pyramid_hides_a_trap() {
        let b = pyramid_blocks(&site(Kind::DesertPyramid));
        let at = |p: IVec3| b.iter().rev().find(|(q, _)| *q == p).map(|x| x.1);
        assert_eq!(b.iter().filter(|x| x.1 == CHEST).count(), 4);
        assert_eq!(at(ivec3(0, 49, 0)), Some(PLATE), "a plate in the hidden room");
        assert_eq!(at(ivec3(0, 48, 0)), Some(TNT), "TNT right under it");
        assert_eq!(at(ivec3(0, 60, 0)), Some(GLAZED_FIRST + 6), "the blue tile over the shaft");
        assert_eq!(at(ivec3(0, 55, 0)), Some(AIR), "a shaft down");
    }

    #[test]
    fn the_jungle_temple_is_trapped() {
        let b = jungle_temple_blocks(&site(Kind::JungleTemple));
        assert_eq!(b.iter().filter(|x| crate::tripwire::is_tripwire(x.1)).count(), 1);
        assert_eq!(b.iter().filter(|x| crate::tripwire::is_hook(x.1)).count(), 2);
        assert_eq!(b.iter().filter(|x| crate::contraptions::is_dispenser(x.1)).count(), 2);
        assert_eq!(b.iter().filter(|x| x.1 == CHEST).count(), 2);
    }

    #[test]
    fn mineshafts_have_rails_webs_and_carts() {
        let mut seen = std::collections::HashSet::new();
        let mut carts = 0;
        for seed in 0..20 {
            let s = Site { seed, ..site(Kind::Mineshaft) };
            let b = mineshaft_blocks(&s);
            seen.extend(b.iter().map(|x| x.1));
            carts += b.iter().filter(|x| x.1 == CHEST && is_cart_spot(&s, x.0)).count();
            assert!(b.iter().all(|(p, _)| (p.x - s.origin.x).abs() <= SHAFT_REACH + 12 && (p.z - s.origin.z).abs() <= SHAFT_REACH + 12));
        }
        for id in [RAIL_FIRST, RAIL_FIRST + 1, COBWEB, FENCE_FIRST, PLANKS, SPAWNER] {
            assert!(seen.contains(&id), "{}", block(id).key);
        }
        assert!(carts >= 10, "carts {carts}");
    }

    #[test]
    fn igloos_sometimes_have_a_basement() {
        let with: Vec<bool> = (0..20).map(|seed| igloo_blocks(&Site { seed, ..site(Kind::Igloo) }).iter().any(|x| x.1 == BREWING_STAND)).collect();
        assert!(with.iter().any(|&b| b) && with.iter().any(|&b| !b));
    }

    #[test]
    fn mineshaft_carts_and_igloo_prisoners_move_in() {
        let mut g = crate::game::Game::new(424242, true, false);
        let worldgen = &g.world.generator;
        // One with a cart in it (most have).
        let has_cart = |s: &Site| mineshaft_blocks(s).iter().any(|x| x.1 == CHEST);
        let shaft = (0..60)
            .flat_map(|r: i32| (-r..=r).flat_map(move |dz| (-r..=r).map(move |dx| (dx, dz))))
            .find_map(|(dx, dz)| worldgen.site(dx, dz).filter(|s| s.kind == Kind::Mineshaft && has_cart(s)))
            .expect("a mineshaft");
        let (cx, cz) = (shaft.origin.x.div_euclid(16), shaft.origin.z.div_euclid(16));
        for dz in -2..=2 {
            for dx in -2..=2 {
                g.world.load_now(cx + dx, cz + dz);
            }
        }
        g.move_in_temples();
        let carts: Vec<_> = g.vehicles.iter().filter(|v| v.kind == crate::vehicles::CHEST_CART_KIND).collect();
        assert!(!carts.is_empty(), "loot carts parked in the mineshaft");
        assert!(carts.iter().all(|v| v.contents.as_ref().is_some_and(|c| c.slots.iter().any(|s| s.is_some()))), "with loot in");
        let cell = carts[0].pos.floor().as_ivec3();
        assert!(crate::vehicles::is_rail(g.world.get_v(cell)), "on a rail");
        // Loading the chunks again doesn't park more.
        let n = g.vehicles.len();
        for dz in -2..=2 {
            for dx in -2..=2 {
                g.world.chunks.remove(&(cx + dx, cz + dz));
                g.world.load_now(cx + dx, cz + dz);
            }
        }
        g.move_in_temples();
        assert_eq!(g.vehicles.len(), n);

        // An igloo with a basement: its prisoners move in when its chest is first filled.
        let worldgen = &g.world.generator;
        let has_basement = |s: &Site| igloo_blocks(s).iter().any(|x| x.1 == BREWING_STAND);
        let igloo = (0..120)
            .flat_map(|r: i32| (-r..=r).flat_map(move |dz| (-r..=r).map(move |dx| (dx, dz))))
            .find_map(|(dx, dz)| worldgen.site(dx, dz).filter(|s| s.kind == Kind::Igloo && has_basement(s)))
            .expect("an igloo");
        let (cx, cz) = (igloo.origin.x.div_euclid(16), igloo.origin.z.div_euclid(16));
        for dz in -1..=1 {
            for dx in -1..=1 {
                g.world.load_now(cx + dx, cz + dz);
            }
        }
        g.move_in_temples();
        let near = |k: crate::entity::MobKind| g.mobs.iter().any(|m| m.kind == k && m.body.pos.distance(igloo.origin.as_vec3()) < 12.0);
        assert!(near(crate::entity::MobKind::Hmmer) && near(crate::entity::MobKind::ZombieHmmer));
    }

    #[test]
    fn cobwebs_slow_you_down() {
        let mut g = crate::game::tests::arena(92);
        let p = ivec3(3, 50, 3);
        g.world.set_v(p, COBWEB);
        let min = p.as_vec3() + Vec3::new(0.2, 0.0, 0.2);
        assert!(in_cobweb(&g.world, min, min + Vec3::new(0.6, 1.8, 0.6)));
        assert!(!in_cobweb(&g.world, min + Vec3::X * 3.0, min + Vec3::new(3.6, 1.8, 0.6)));
    }
}
