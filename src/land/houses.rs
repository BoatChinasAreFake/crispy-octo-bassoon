//! **Village houses**, in newer worlds: proper little houses with pitched
//! roofs instead of plank boxes with a lid.
//!
//! There are three plans, all on a 7×7 footprint (the roofs hang a block
//! over): a cottage with its ridge along the front, a gabled house with its
//! ridge running back from the door (so the roof shelters the doorstep), and
//! a two-storey house with a ladder up to a bedroom. Each has log corners
//! on a cobblestone footing, glass-pane windows, and a bed, a chest, a
//! crafting table, a furnace and a light: somebody lives here.
//!
//! Plans are drawn with the door on the south side and turned to face the
//! path, the same way as the old huts (see `structures.rs`).
//!
//! **Village styles** (v0.5, in new worlds): each biome builds its own way.
//! Plains houses are as they were; desert houses are Mud Brick and
//! sandstone with flat roofs; savanna houses are acacia under Thatch; taiga
//! houses are dark oak on mossy footings; snowy houses are birch under Frost
//! Shingles. And every house has a workstation for whoever lives there (see
//! village_life.rs).

use crate::block::*;
use crate::noise::hash2;
use crate::villagers::Job;
use crate::woods::{id as wood, part};
use macroquad::math::{ivec3, IVec3};

/// How many plans there are.
pub const PLANS: u32 = 3;

/// How a village builds.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Style {
    Plains,
    Desert,
    Savanna,
    Taiga,
    Snowy,
}

impl Style {
    #[cfg(test)]
    pub const ALL: [Style; 5] = [Style::Plains, Style::Desert, Style::Savanna, Style::Taiga, Style::Snowy];

    /// A village in this biome builds like this.
    pub fn of(biome: crate::world::Biome) -> Style {
        use crate::world::Biome::*;
        match biome {
            Desert | Badlands => Style::Desert,
            Savanna => Style::Savanna,
            Taiga => Style::Taiga,
            Snowy | IceSpikes => Style::Snowy,
            _ => Style::Plains,
        }
    }

    pub fn palette(self) -> Palette {
        let stairs_of = |w: usize| wood(w, part::STAIRS);
        match self {
            Style::Plains => Palette { wall: PLANKS, corner: LOG, footing: COBBLE, floor: PLANKS, roof: Roof::Stairs(stairs(0, 0), slab(0, false)), gable: PLANKS, door: DOOR_FIRST, path: GRAVEL, square: COBBLE },
            Style::Desert => Palette { wall: MUD_BRICKS, corner: SANDSTONE, footing: SANDSTONE, floor: SANDSTONE, roof: Roof::Flat(SANDSTONE), gable: MUD_BRICKS, door: DOOR_FIRST, path: SANDSTONE, square: SANDSTONE },
            Style::Savanna => Palette { wall: wood(0, part::PLANKS), corner: ACACIA_LOG, footing: TERRACOTTA, floor: wood(0, part::PLANKS), roof: Roof::Blocks(THATCH), gable: wood(0, part::PLANKS), door: wood(0, part::DOOR), path: DIRT, square: TERRACOTTA },
            Style::Taiga => Palette { wall: wood(2, part::PLANKS), corner: SPRUCE_LOG, footing: MOSSY_COBBLE, floor: wood(2, part::PLANKS), roof: Roof::Stairs(stairs_of(2), wood(2, part::SLAB)), gable: wood(2, part::PLANKS), door: wood(2, part::DOOR), path: GRAVEL, square: MOSSY_COBBLE },
            Style::Snowy => Palette { wall: wood(1, part::PLANKS), corner: SPRUCE_LOG, footing: COBBLE, floor: wood(1, part::PLANKS), roof: Roof::Blocks(FROST_SHINGLES), gable: wood(1, part::PLANKS), door: wood(1, part::DOOR), path: GRAVEL, square: COBBLE },
        }
    }
}

/// What a roof is made of.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Roof {
    /// Stairs (the north-facing one; its four facings follow) and a slab along the ridge.
    Stairs(Id, Id),
    /// Full blocks, stepped.
    Blocks(Id),
    /// A flat roof with a low wall round it.
    Flat(Id),
}

/// What a style builds with.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Palette {
    pub wall: Id,
    pub corner: Id,
    pub footing: Id,
    pub floor: Id,
    pub roof: Roof,
    pub gable: Id,
    /// The first block of its door's family (see `door_of`).
    pub door: Id,
    /// The village's paths and square.
    pub path: Id,
    pub square: Id,
}

/// The job whose workstation a house of this seed has.
pub fn house_job(seed: u32) -> Job {
    Job::ALL[(hash2(seed, 11, 5) * 8.0) as usize % 8]
}

/// Where a house's workstation stands (before turning): against the east wall, mid-way.
pub const STATION: (i32, i32, i32) = (2, 1, 0);

/// Turn a local (x, z) (door to the south) `turns` quarter turns.
fn turn(x: i32, z: i32, turns: u8) -> (i32, i32) {
    let (mut x, mut z) = (x, z);
    for _ in 0..turns {
        (x, z) = (-z, x);
    }
    (x, z)
}

/// A house at `origin` (its ground floor) with its door facing `facing`
/// (0 north .. 3 west). The plan is picked by `seed`.
#[cfg(test)]
pub fn house_blocks(origin: IVec3, facing: u8, seed: u32) -> Vec<(IVec3, Id)> {
    styled_house(origin, facing, seed, Style::Plains, false)
}

/// A house in `style`, with a workstation if `station` (new worlds).
pub fn styled_house(origin: IVec3, facing: u8, seed: u32, style: Style, station: bool) -> Vec<(IVec3, Id)> {
    let pal = style.palette();
    let plan = (hash2(seed, 3, 9) * PLANS as f32) as u32 % PLANS;
    // Desert houses keep to one storey under their flat roofs.
    let plan = if matches!(pal.roof, Roof::Flat(_)) && plan == 2 { 0 } else { plan };
    let turns = (facing + 2) % 4;
    let mut out = Vec::new();
    // Facings (stairs, doors, beds, ladders) turn with the house.
    let f = |d: u8| (d + turns) % 4;
    let mut put = |x: i32, y: i32, z: i32, id: Id| {
        let (tx, tz) = turn(x, z, turns);
        out.push((origin + ivec3(tx, y, tz), id));
    };
    let two_storey = plan == 2;
    let wall_top = if two_storey { 6 } else { 3 };
    // Clear the space (in newer villages, high enough that no tree crown sits
    // on the roof), lay the footing and the floor.
    let clear = if station { wall_top + 16 } else { wall_top + 6 };
    for x in -4..=4i32 {
        for z in -4..=4i32 {
            for y in 1..=clear {
                put(x, y, z, AIR);
            }
            if x.abs() <= 3 && z.abs() <= 3 {
                for y in -3..0 {
                    put(x, y, z, pal.footing);
                }
                put(x, 0, z, if x.abs() == 3 || z.abs() == 3 { pal.footing } else { pal.floor });
            }
        }
    }
    // Walls: logs at the corners, a cobblestone course along the bottom, planks above, windows.
    for x in -3..=3i32 {
        for z in -3..=3i32 {
            let (ex, ez) = (x.abs() == 3, z.abs() == 3);
            if !ex && !ez {
                continue;
            }
            for y in 1..=wall_top {
                let id = if ex && ez {
                    pal.corner
                } else if y == 1 {
                    pal.footing
                } else if two_storey && y == 4 {
                    // A log band where the upper floor sits.
                    pal.corner
                } else {
                    let along = if ex { z } else { x };
                    let window = (y == 2 || (two_storey && y == 5)) && along.abs() == 1 && !(z == 3 && y == 2);
                    if window { PANE_FIRST } else { pal.wall }
                };
                put(x, y, z, id);
            }
        }
    }
    // The door, with a step out front.
    put(0, 1, 3, door_of(pal.door, f(2), false, false));
    put(0, 2, 3, door_of(pal.door, f(2), false, true));
    put(0, 0, 4, stairs(1, f(0)));
    // The roof.
    let eave = wall_top + 1;
    // A slope's piece facing `d` (or the ridge, for None).
    let piece = |d: Option<u8>| match (pal.roof, d) {
        (Roof::Stairs(_, ridge), None) => ridge,
        (Roof::Stairs(north, _), Some(d)) => north + f(d) as Id,
        (Roof::Blocks(b), _) | (Roof::Flat(b), _) => b,
    };
    if let Roof::Flat(top) = pal.roof {
        // Flat: a roof over the walls, a low wall round its edge.
        for x in -3..=3i32 {
            for z in -3..=3i32 {
                put(x, eave, z, top);
                if x.abs() == 3 || z.abs() == 3 {
                    put(x, eave + 1, z, pal.wall);
                }
            }
        }
    } else {
        // Under the eaves, the walls go up to meet the roof.
        for k in -3..=3 {
            let (x, z) = if plan == 1 { (3, k) } else { (k, 3) };
            put(x, wall_top + 1, z, pal.gable);
            put(-x, wall_top + 1, -z, pal.gable);
        }
    }
    if matches!(pal.roof, Roof::Flat(_)) {
    } else if plan == 1 {
        // Ridge running back from the door: slopes to the east and west, gables front and back.
        for x in -4..=4i32 {
            let rise = 4 - x.abs();
            for z in -4..=4i32 {
                let y = eave + rise;
                put(x, y, z, if x == 0 { piece(None) } else { piece(Some(if x > 0 { 3 } else { 1 })) });
                // The gable ends, filled in under the slope.
                if z.abs() == 3 {
                    for yy in eave..y {
                        if x.abs() <= 2 {
                            put(x, yy, z, if yy == eave + 1 && x == 0 { PANE_FIRST } else { pal.gable });
                        }
                    }
                }
            }
        }
    } else {
        // Ridge along the front: slopes to the south and north, gables on the sides.
        for z in -4..=4i32 {
            let rise = 4 - z.abs();
            for x in -4..=4i32 {
                let y = eave + rise;
                put(x, y, z, if z == 0 { piece(None) } else { piece(Some(if z > 0 { 0 } else { 2 })) });
                if x.abs() == 3 {
                    for yy in eave..y {
                        if z.abs() <= 2 {
                            put(x, yy, z, if yy == eave + 1 && z == 0 { PANE_FIRST } else { pal.gable });
                        }
                    }
                }
            }
        }
    }
    // Inside.
    put(2, 1, -2, CHEST);
    put(-2, 1, -2, TABLE);
    put(-2, 1, -1, FURNACE);
    if two_storey {
        // The upper floor, a ladder up to it, and the bedroom.
        for x in -2..=2i32 {
            for z in -2..=2i32 {
                if (x, z) != (2, 2) {
                    put(x, 4, z, pal.floor);
                }
            }
        }
        for y in 1..=4 {
            put(2, y, 2, LADDER_FIRST + f(1) as Id);
        }
        put(-1, 5, -2, crate::beds::bed(f(0)));
        put(1, 5, -2, BOOKSHELF);
        put(0, 3, 0, LANTERN_HANGING);
        put(0, wall_top + 4, 0, LANTERN_HANGING);
    } else {
        put(-2, 1, 2, crate::beds::bed(f(0)));
        // (Under a flat roof, the lantern hangs from it.)
        put(0, if matches!(pal.roof, Roof::Flat(_)) { wall_top } else { wall_top + 4 }, 0, LANTERN_HANGING);
        // (In newer villages the workstation stands here instead, so a Barrel isn't mistaken for one.)
        if plan == 0 && !station {
            put(2, 1, 1, BARREL);
        }
    }
    // Whoever lives here works here (see village_life.rs).
    if station {
        let (x, y, z) = STATION;
        put(x, y, z, crate::village_life::station_of(house_job(seed)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_style_builds_its_own_way_with_a_workstation() {
        for style in Style::ALL {
            let pal = style.palette();
            for seed in 0..20u32 {
                let o = ivec3(0, 70, 0);
                let mut at = std::collections::HashMap::new();
                for (p, id) in styled_house(o, (seed % 4) as u8, seed, style, true) {
                    at.insert(p, id);
                }
                let has = |id: Id| at.values().any(|v| *v == id);
                assert!(has(pal.wall) && has(pal.corner), "{style:?}");
                assert!(at.values().any(|v| door_base(*v) == Some(pal.door)), "{style:?} has its own door");
                match pal.roof {
                    Roof::Flat(top) | Roof::Blocks(top) => assert!(has(top), "{style:?}"),
                    Roof::Stairs(north, _) => assert!(at.values().any(|v| (north..north + 4).contains(v))),
                }
                // One workstation, for the job the house's seed says.
                let stations: Vec<_> = at.values().filter(|v| crate::village_life::station_job(**v).is_some()).collect();
                assert_eq!(stations.len(), 1, "{style:?} seed {seed}");
                assert_eq!(crate::village_life::station_job(*stations[0]), Some(house_job(seed)));
            }
        }
        // Plains without a workstation is just the house it always was.
        assert_eq!(styled_house(ivec3(5, 60, 5), 1, 7, Style::Plains, false), house_blocks(ivec3(5, 60, 5), 1, 7));
        assert_eq!(Style::of(crate::world::Biome::Desert), Style::Desert);
        assert_eq!(Style::of(crate::world::Biome::Forest), Style::Plains);
    }

    #[test]
    fn houses_have_a_door_a_bed_a_chest_and_a_roof() {
        let mut plans = std::collections::HashSet::new();
        for seed in 0..60u32 {
            plans.insert((hash2(seed, 3, 9) * PLANS as f32) as u32 % PLANS);
            for facing in 0..4u8 {
                let o = ivec3(100, 70, -40);
                let b = house_blocks(o, facing, seed);
                // What ends up where (later blocks win).
                let mut at = std::collections::HashMap::new();
                for (p, id) in &b {
                    at.insert(*p, *id);
                }
                let count = |want: &dyn Fn(Id) -> bool| at.values().filter(|id| want(**id)).count();
                assert_eq!(count(&|id| id == CHEST), 1, "one chest (one Hmmer) per house");
                assert_eq!(count(&crate::beds::is_bed), 1);
                assert_eq!(count(&|id| (DOOR_FIRST..DOOR_FIRST + 16).contains(&id)), 2);
                // The door faces the way it was asked to, and is on that side.
                let (dp, did) = at.iter().find(|(_, id)| (DOOR_FIRST..DOOR_FIRST + 16).contains(*id)).unwrap();
                assert_eq!((did - DOOR_FIRST) / 4, facing as Id);
                let d = *dp - o;
                let out = [ivec3(0, 0, -1), ivec3(1, 0, 0), ivec3(0, 0, 1), ivec3(-1, 0, 0)][facing as usize];
                assert_eq!(d.x * out.x + d.z * out.z, 3, "the door is in the front wall");
                // Covered over: everything inside the walls has roof above it.
                for x in -2..=2 {
                    for z in -2..=2 {
                        let roofed = (o.y + 4..o.y + 16).any(|y| at.get(&ivec3(o.x + x, y, o.z + z)).is_some_and(|id| *id != AIR && *id != LANTERN_HANGING));
                        assert!(roofed, "roof over ({x}, {z})");
                    }
                }
                // The chest's Hmmer has somewhere to stand (see `hut_middle`).
                let (cp, _) = at.iter().find(|(_, id)| **id == CHEST).unwrap();
                let free = [ivec3(-2, 0, 2), ivec3(2, 0, 2), ivec3(-2, 0, -2), ivec3(2, 0, -2)].iter().any(|d| at.get(&(*cp + *d)) == Some(&AIR) && at.get(&(*cp + *d + IVec3::Y)) == Some(&AIR));
                assert!(free);
            }
        }
        assert_eq!(plans.len(), PLANS as usize, "every plan turns up");
    }
}
