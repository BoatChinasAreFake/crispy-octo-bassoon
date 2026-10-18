//! Block sets (v0.8): slabs, stairs and walls for the stones (and planks)
//! that didn't have them yet.
//!
//! Every set's blocks sit together from `SET_FIRST`, in the order of `SETS`:
//! its slabs (bottom, top), its stairs (north, east, south, west), then its
//! wall (`+ mask` of the sides joined, like a fence). Cobblestun and Stone
//! Bricks already had slabs and stairs, so they only get walls here; the
//! planks only get slabs and stairs.
//!
//! **Walls** join walls, fence gates and solid blocks next door, the same
//! way fences do (worked out where the world lives; joined players get the
//! edit), and stand a block and a half tall for anything trying to jump them.

use crate::block::*;

/// One block set: the full block, the key and name its pieces are named
/// after, and which pieces it gets.
pub struct Set {
    pub full: Id,
    pub key: &'static str,
    pub name: &'static str,
    pub steps: bool,
    pub wall: bool,
}

const fn s(full: Id, key: &'static str, name: &'static str) -> Set {
    Set { full, key, name, steps: true, wall: true }
}
const fn walls(full: Id, key: &'static str, name: &'static str) -> Set {
    Set { full, key, name, steps: false, wall: true }
}
const fn steps(full: Id, key: &'static str, name: &'static str) -> Set {
    Set { full, key, name, steps: true, wall: false }
}

pub const SETS: [Set; 30] = [
    walls(COBBLE, "cobblestone", "Cobblestun"),
    walls(STONE_BRICKS, "stone_brick", "Stone Brick"),
    s(STONE, "stone", "Stone"),
    s(MOSSY_COBBLE, "mossy_cobblestone", "Mossy Cobblestun"),
    s(BRICK, "brick", "Brick"),
    s(SANDSTONE, "sandstone", "Sandstone"),
    s(COBBLED_DEEPSLATE, "cobbled_deepslate", "Cobbled Deepslate"),
    s(POLISHED_DEEPSLATE, "polished_deepslate", "Polished Deepslate"),
    s(DEEPSLATE_BRICKS, "deepslate_brick", "Deepslate Brick"),
    s(DEEPSLATE_TILES, "deepslate_tile", "Deepslate Tile"),
    s(TUFF, "tuff", "Tuff"),
    s(POLISHED_TUFF, "polished_tuff", "Polished Tuff"),
    s(TUFF_BRICKS, "tuff_brick", "Tuff Brick"),
    s(MUD_BRICKS, "mud_brick", "Mud Brick"),
    s(SCORCHROCK, "scorchrock", "Scorchrock"),
    s(SCORCH_BRICKS, "scorch_brick", "Scorch Brick"),
    s(BASALT, "basalt", "Basalt"),
    s(BLACKSTONE, "blackstone", "Blackstone"),
    s(SMOOTH_BASALT, "smooth_basalt", "Smooth Basalt"),
    s(CALCITE, "calcite", "Calcite"),
    s(DRIPSTONE_BLOCK, "dripstone", "Dripstone"),
    s(PRISMARINE, "prismarine", "Prismarine"),
    s(PRISMARINE_BRICKS, "prismarine_brick", "Prismarine Brick"),
    s(DARK_PRISMARINE, "dark_prismarine", "Dark Prismarine"),
    s(RESIN_BRICKS, "resin_brick", "Resin Brick"),
    s(SPRING_ROCK, "spring_rock", "Spring Rock"),
    s(HOLLOW_STONE, "hollow_stone", "Hollow Stone"),
    s(MAUVE_BLOCK, "mauve", "Mauve"),
    steps(CHERRY_PLANKS, "cherry", "Cherry"),
    steps(MANGROVE_PLANKS, "mangrove", "Mangrove"),
];

/// How many blocks a set has.
pub const fn size(set: &Set) -> Id {
    (if set.steps { 6 } else { 0 }) + if set.wall { 16 } else { 0 }
}

/// Where set `i` starts.
pub const fn start(i: usize) -> Id {
    let mut at = SET_FIRST;
    let mut k = 0;
    while k < i {
        at += size(&SETS[k]);
        k += 1;
    }
    at
}

/// How many blocks all the sets take.
pub const COUNT: Id = start(SETS.len()) - SET_FIRST;

/// Set `i`'s bottom slab (the item), if it has one.
pub fn slab(i: usize) -> Option<Id> {
    SETS[i].steps.then(|| start(i))
}
/// Set `i`'s north-facing stairs (the item), if it has them.
pub fn stairs(i: usize) -> Option<Id> {
    SETS[i].steps.then(|| start(i) + 2)
}
/// Set `i`'s unjoined wall (the item), if it has one.
pub fn wall(i: usize) -> Option<Id> {
    SETS[i].wall.then(|| start(i) + if SETS[i].steps { 6 } else { 0 })
}

/// The set made of `full`, if there is one.
pub fn set_of(full: Id) -> Option<usize> {
    SETS.iter().position(|s| s.full == full)
}

/// The first block of a wall's family (its unjoined wall).
pub fn wall_base(id: Id) -> Option<Id> {
    match block(id).shape {
        Shape::Wall { mask } => Some(id - mask as Id),
        _ => None,
    }
}

pub fn is_wall(id: Id) -> bool {
    wall_base(id).is_some()
}

/// Every set's blocks, in id order (`blocks` holds the full blocks, to copy
/// their looks and digging from).
pub fn defs(blocks: &[BlockDef]) -> Vec<BlockDef> {
    let mut out = Vec::new();
    for (i, set) in SETS.iter().enumerate() {
        let b = &blocks[set.full as usize];
        let (k, n) = (set.key, set.name);
        let piece = |key: String, name: String, shape: Shape, creative: bool, item: Id| {
            let mut d = def(leak(&key), leak(&name), Model::Shaped, true, false, b.tex, b.hardness, b.pick_tier, b.pick_block, item, 0.0, b.sound);
            d.shape = shape;
            d.creative = creative;
            d
        };
        if let (Some(sl), Some(st)) = (slab(i), stairs(i)) {
            for top in [false, true] {
                let mut d = piece(format!("{k}_slab{}", if top { "_top" } else { "" }), format!("{n} Slab"), Shape::Slab { top }, !top, sl);
                d.family = sl;
                d.full = set.full;
                out.push(d);
            }
            for facing in 0..4u8 {
                let mut d = piece(format!("{k}_stairs{}", ["", "_east", "_south", "_west"][facing as usize]), format!("{n} Stairs"), Shape::Stairs { facing }, facing == 0, st);
                d.family = st;
                d.full = set.full;
                out.push(d);
            }
        }
        if let Some(w) = wall(i) {
            for mask in 0..16u8 {
                let mut d = piece(format!("{k}_wall{}", if mask == 0 { String::new() } else { format!("_{mask}") }), format!("{n} Wall"), Shape::Wall { mask }, mask == 0, w);
                d.see_through = true;
                out.push(d);
            }
        }
    }
    out
}

/// Every set's recipes (the stonecutter-free way: as at a crafting table).
pub fn recipes() -> Vec<Recipe> {
    let r = |inputs: &[(Id, u8)], output: (Id, u8)| Recipe { inputs: inputs.to_vec(), output };
    let mut out = Vec::new();
    for (i, set) in SETS.iter().enumerate() {
        if let (Some(sl), Some(st)) = (slab(i), stairs(i)) {
            out.push(r(&[(set.full, 3)], (sl, 6)));
            out.push(r(&[(set.full, 6)], (st, 4)));
        }
        if let Some(w) = wall(i) {
            out.push(r(&[(set.full, 6)], (w, 6)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use macroquad::math::{ivec3, IVec3};

    #[test]
    fn every_set_is_whole_and_in_order() {
        for (i, set) in SETS.iter().enumerate() {
            if let Some(sl) = slab(i) {
                assert_eq!(block(sl).key, format!("{}_slab", set.key));
                assert_eq!(slab_of(sl + 1), Some((sl, true)));
                assert_eq!(made_of(sl), set.full);
                assert_eq!(stairs_of(stairs(i).unwrap() + 3), Some((stairs(i).unwrap(), 3)));
                assert_eq!(placing_item(stairs(i).unwrap() + 2), Some(stairs(i).unwrap()));
            }
            if let Some(w) = wall(i) {
                assert_eq!(block(w).key, format!("{}_wall", set.key));
                assert_eq!(wall_base(w + 13), Some(w));
                assert_eq!(placing_item(w + 5), Some(w));
                assert_eq!(block(w + 5).drop, w);
            }
            // Pieces dig like what they're made of.
            let first = start(i);
            assert_eq!(block(first).pick_tier, block(set.full).pick_tier);
        }
        assert_eq!(start(SETS.len()), SET_FIRST + COUNT);
        // Every stone has a wall now; the woods have their steps.
        for full in [COBBLE, STONE_BRICKS, STONE, TUFF, DEEPSLATE_TILES, PRISMARINE, MAUVE_BLOCK, BLACKSTONE] {
            assert!(set_of(full).and_then(wall).is_some(), "{}", block(full).key);
        }
        assert!(set_of(CHERRY_PLANKS).and_then(slab).is_some());
    }

    #[test]
    fn walls_join_walls_fences_gates_and_solid_blocks() {
        let mut g = crate::game::tests::arena(808);
        let w = wall(set_of(TUFF).unwrap()).unwrap();
        let cobble = wall(set_of(COBBLE).unwrap()).unwrap();
        let a = ivec3(-3, 50, 3);
        g.world.set_v(a, w);
        g.world.set_v(a + IVec3::X, cobble);
        assert_eq!(g.world.get_v(a), w + 1, "joined east");
        assert_eq!(g.world.get_v(a + IVec3::X), cobble + 2, "different stones join too");
        g.world.set_v(a - IVec3::Z, STONE);
        assert_eq!(g.world.get_v(a), w + 1 + 8, "and to a solid block");
        g.world.set_v(a - IVec3::X, crate::carpentry::gate(false, false));
        assert_eq!(g.world.get_v(a), w + 1 + 2 + 8, "and to a gate");
        // A wall stands tall, like a fence.
        assert!(crate::carpentry::is_tall(w));
        // Two slabs of a set make the full block.
        let sl = slab(set_of(TUFF).unwrap()).unwrap();
        assert_eq!(made_of(sl), TUFF);
        // Five kinds of wall in your pockets.
        let idle = crate::game::Controls::default();
        for full in [COBBLE, STONE, TUFF, BRICK] {
            g.inv.add(wall(set_of(full).unwrap()).unwrap(), 1);
        }
        g.update(0.05, &idle);
        assert!(!g.advancements.has("wall_to_wall"));
        g.inv.add(wall(set_of(MAUVE_BLOCK).unwrap()).unwrap(), 1);
        g.update(0.05, &idle);
        assert!(g.advancements.has("wall_to_wall"));
    }

    #[test]
    fn a_wall_is_a_post_with_arms() {
        let (boxes, n) = Shape::Wall { mask: 0 }.boxes();
        assert_eq!(n, 1);
        assert!(boxes[0].1[1] == 1.0 && boxes[0].0[0] > 0.0);
        let (_, n) = Shape::Wall { mask: 1 | 8 }.boxes();
        assert_eq!(n, 3);
    }
}
