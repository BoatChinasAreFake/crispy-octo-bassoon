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

use crate::block::*;
use crate::noise::hash2;
use macroquad::math::{ivec3, IVec3};

/// How many plans there are.
pub const PLANS: u32 = 3;

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
pub fn house_blocks(origin: IVec3, facing: u8, seed: u32) -> Vec<(IVec3, Id)> {
    let plan = (hash2(seed, 3, 9) * PLANS as f32) as u32 % PLANS;
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
    // Clear the space, lay the footing and the floor.
    for x in -4..=4i32 {
        for z in -4..=4i32 {
            for y in 1..=wall_top + 6 {
                put(x, y, z, AIR);
            }
            if x.abs() <= 3 && z.abs() <= 3 {
                for y in -3..0 {
                    put(x, y, z, COBBLE);
                }
                put(x, 0, z, if x.abs() == 3 || z.abs() == 3 { COBBLE } else { PLANKS });
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
                    LOG
                } else if y == 1 {
                    COBBLE
                } else if two_storey && y == 4 {
                    // A log band where the upper floor sits.
                    LOG
                } else {
                    let along = if ex { z } else { x };
                    let window = (y == 2 || (two_storey && y == 5)) && along.abs() == 1 && !(z == 3 && y == 2);
                    if window { PANE_FIRST } else { PLANKS }
                };
                put(x, y, z, id);
            }
        }
    }
    // Under the eaves, the walls go up to meet the roof.
    for k in -3..=3 {
        let (x, z) = if plan == 1 { (3, k) } else { (k, 3) };
        put(x, wall_top + 1, z, PLANKS);
        put(-x, wall_top + 1, -z, PLANKS);
    }
    // The door, with a step out front.
    put(0, 1, 3, door(f(2), false, false));
    put(0, 2, 3, door(f(2), false, true));
    put(0, 0, 4, stairs(1, f(0)));
    // The roof.
    let eave = wall_top + 1;
    if plan == 1 {
        // Ridge running back from the door: slopes to the east and west, gables front and back.
        for x in -4..=4i32 {
            let rise = 4 - x.abs();
            for z in -4..=4i32 {
                let y = eave + rise;
                if x == 0 {
                    put(x, y, z, slab(0, false));
                } else {
                    put(x, y, z, stairs(0, f(if x > 0 { 3 } else { 1 })));
                }
                // The gable ends, filled in under the slope.
                if z.abs() == 3 {
                    for yy in eave..y {
                        if x.abs() <= 2 {
                            put(x, yy, z, if yy == eave + 1 && x == 0 { PANE_FIRST } else { PLANKS });
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
                if z == 0 {
                    put(x, y, z, slab(0, false));
                } else {
                    put(x, y, z, stairs(0, f(if z > 0 { 0 } else { 2 })));
                }
                if x.abs() == 3 {
                    for yy in eave..y {
                        if z.abs() <= 2 {
                            put(x, yy, z, if yy == eave + 1 && z == 0 { PANE_FIRST } else { PLANKS });
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
                    put(x, 4, z, PLANKS);
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
        put(0, wall_top + 4, 0, LANTERN_HANGING);
        if plan == 0 {
            put(2, 1, 1, BARREL);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

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
