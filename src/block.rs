//! Blocks, items, tools and recipes. Every item id is a `u8`:
//! ids below `FIRST_ITEM` are placeable blocks, the rest are plain items.

use crate::texture::*;

pub const AIR: u8 = 0;
pub const GRASS: u8 = 1;
pub const DIRT: u8 = 2;
pub const STONE: u8 = 3;
pub const COBBLE: u8 = 4;
pub const SAND: u8 = 5;
pub const GRAVEL: u8 = 6;
pub const WATER: u8 = 7;
pub const LOG: u8 = 8;
pub const LEAVES: u8 = 9;
pub const PLANKS: u8 = 10;
pub const GLASS: u8 = 11;
pub const BEDROCK: u8 = 12;
pub const COAL_ORE: u8 = 13;
pub const IRON_ORE: u8 = 14;
pub const DIAMOND_ORE: u8 = 15;
pub const SNOW_GRASS: u8 = 16;
pub const BRICK: u8 = 17;
pub const TNT: u8 = 18;
pub const TABLE: u8 = 19;
pub const GLOWROCK: u8 = 20;
pub const TORCH: u8 = 21;
pub const FLOWER: u8 = 22;
pub const TALL_GRASS: u8 = 23;
pub const NUM_BLOCKS: u8 = 24;

pub const STICK: u8 = 100;
pub const COAL: u8 = 101;
pub const IRON: u8 = 102;
pub const DIAMOND: u8 = 103;
pub const GUNPOWDER: u8 = 104;
pub const PORKCHOP: u8 = 105;
pub const GOO: u8 = 106;
pub const PICK_WOOD: u8 = 110;
pub const PICK_STONE: u8 = 111;
pub const PICK_IRON: u8 = 112;
pub const PICK_DIAMOND: u8 = 113;
pub const SWORD_WOOD: u8 = 114;
pub const SWORD_STONE: u8 = 115;
pub const SWORD_IRON: u8 = 116;
pub const SWORD_DIAMOND: u8 = 117;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Model {
    Empty,
    Cube,
    Cross,
    Liquid,
}

pub struct BlockDef {
    pub name: &'static str,
    pub model: Model,
    /// Collides with players and mobs.
    pub solid: bool,
    /// Fully hides neighbouring faces and casts ambient occlusion.
    pub opaque: bool,
    /// Top, side, bottom texture tiles.
    pub tex: [u16; 3],
    /// Seconds to break by hand; negative means unbreakable.
    pub hardness: f32,
    /// Needs a pickaxe of at least this tier to drop anything (0 = anything goes).
    pub pick_tier: u8,
    /// Whether pickaxes speed it up at all.
    pub pick_block: bool,
    pub drop: u8,
    /// Point-light radius in blocks (0 = dark).
    pub light: f32,
}

const fn def(
    name: &'static str,
    model: Model,
    solid: bool,
    opaque: bool,
    tex: [u16; 3],
    hardness: f32,
    pick_tier: u8,
    pick_block: bool,
    drop: u8,
    light: f32,
) -> BlockDef {
    BlockDef { name, model, solid, opaque, tex, hardness, pick_tier, pick_block, drop, light }
}

use Model::*;

pub static BLOCKS: [BlockDef; NUM_BLOCKS as usize] = [
    def("Air", Empty, false, false, [0, 0, 0], 0.0, 0, false, AIR, 0.0),
    def("Grass Block", Cube, true, true, [T_GRASS_TOP, T_GRASS_SIDE, T_DIRT], 0.6, 0, false, DIRT, 0.0),
    def("Dirt (Premium)", Cube, true, true, [T_DIRT; 3], 0.5, 0, false, DIRT, 0.0),
    def("Stone", Cube, true, true, [T_STONE; 3], 1.5, 1, true, COBBLE, 0.0),
    def("Cobblestun", Cube, true, true, [T_COBBLE; 3], 2.0, 1, true, COBBLE, 0.0),
    def("Sand", Cube, true, true, [T_SAND; 3], 0.5, 0, false, SAND, 0.0),
    def("Gravel", Cube, true, true, [T_GRAVEL; 3], 0.6, 0, false, GRAVEL, 0.0),
    def("Water", Liquid, false, false, [T_WATER; 3], -1.0, 0, false, AIR, 0.0),
    def("Tree Chunk", Cube, true, true, [T_LOG_TOP, T_LOG_SIDE, T_LOG_TOP], 2.0, 0, false, LOG, 0.0),
    def("Leaves", Cube, true, false, [T_LEAVES; 3], 0.2, 0, false, AIR, 0.0),
    def("Planks", Cube, true, true, [T_PLANKS; 3], 2.0, 0, false, PLANKS, 0.0),
    def("Glass", Cube, true, false, [T_GLASS; 3], 0.3, 0, false, AIR, 0.0),
    def("Bedrock (Nope)", Cube, true, true, [T_BEDROCK; 3], -1.0, 0, false, AIR, 0.0),
    def("Coal Ore", Cube, true, true, [T_COAL_ORE; 3], 3.0, 1, true, COAL, 0.0),
    def("Iron Ore", Cube, true, true, [T_IRON_ORE; 3], 3.0, 2, true, IRON, 0.0),
    def("Dimond Ore", Cube, true, true, [T_DIAMOND_ORE; 3], 3.0, 3, true, DIAMOND, 0.0),
    def("Snowy Grass", Cube, true, true, [T_SNOW, T_SNOW_SIDE, T_DIRT], 0.6, 0, false, DIRT, 0.0),
    def("Bricks", Cube, true, true, [T_BRICK; 3], 2.0, 1, true, BRICK, 0.0),
    def("TNT (Totally Not Trouble)", Cube, true, true, [T_TNT_TOP, T_TNT_SIDE, T_TNT_BOTTOM], 0.0, 0, false, TNT, 0.0),
    def("Decorative Crafting Table", Cube, true, true, [T_TABLE_TOP, T_TABLE_SIDE, T_PLANKS], 2.5, 0, false, TABLE, 0.0),
    def("Glowrock", Cube, true, true, [T_GLOW; 3], 0.3, 0, false, GLOWROCK, 11.0),
    def("Torch", Cross, false, false, [T_TORCH; 3], 0.0, 0, false, TORCH, 8.0),
    def("Poppy-ish", Cross, false, false, [T_FLOWER; 3], 0.0, 0, false, FLOWER, 0.0),
    def("Tall Grass", Cross, false, false, [T_TALLGRASS; 3], 0.0, 0, false, AIR, 0.0),
];

#[inline]
pub fn block(id: u8) -> &'static BlockDef {
    &BLOCKS[(id as usize).min(NUM_BLOCKS as usize - 1)]
}
#[inline]
pub fn is_opaque(id: u8) -> bool {
    block(id).opaque
}
#[inline]
pub fn is_solid(id: u8) -> bool {
    block(id).solid
}
/// Stops sunlight: used by the column heightmap for sky lighting.
#[inline]
pub fn blocks_sky(id: u8) -> bool {
    !matches!(id, AIR | GLASS | TORCH | FLOWER | TALL_GRASS)
}
/// Can the player point at it (and break it)?
#[inline]
pub fn targetable(id: u8) -> bool {
    id != AIR && id != WATER
}
/// Placing into this cell simply replaces it.
#[inline]
pub fn replaceable(id: u8) -> bool {
    matches!(id, AIR | WATER | TALL_GRASS)
}

pub fn is_block_item(id: u8) -> bool {
    id > AIR && id < NUM_BLOCKS
}

pub fn item_name(id: u8) -> &'static str {
    if id < NUM_BLOCKS {
        return block(id).name;
    }
    match id {
        STICK => "Stick (Artisanal)",
        COAL => "Coal",
        IRON => "Iron Chunk",
        DIAMOND => "Dimond",
        GUNPOWDER => "Hisspowder",
        PORKCHOP => "Raw Oinkchop",
        GOO => "Groaner Goo",
        PICK_WOOD => "Wooden Pickaxe",
        PICK_STONE => "Stone Pickaxe",
        PICK_IRON => "Iron Pickaxe",
        PICK_DIAMOND => "Dimond Pickaxe",
        SWORD_WOOD => "Wooden Sword",
        SWORD_STONE => "Stone Sword",
        SWORD_IRON => "Iron Sword",
        SWORD_DIAMOND => "Dimond Sword",
        _ => "???",
    }
}

/// Texture tile used for the flat inventory icon (blocks get an isometric cube instead).
pub fn item_tile(id: u8) -> u16 {
    if id < NUM_BLOCKS {
        return block(id).tex[1];
    }
    match id {
        STICK => T_STICK,
        COAL => T_COAL,
        IRON => T_IRON,
        DIAMOND => T_DIAMOND,
        GUNPOWDER => T_GUNPOWDER,
        PORKCHOP => T_PORK,
        GOO => T_GOO,
        PICK_WOOD..=PICK_DIAMOND => T_PICK0 + (id - PICK_WOOD) as u16,
        SWORD_WOOD..=SWORD_DIAMOND => T_SWORD0 + (id - SWORD_WOOD) as u16,
        _ => T_WHITE,
    }
}

pub fn max_stack(id: u8) -> u8 {
    if (PICK_WOOD..=SWORD_DIAMOND).contains(&id) { 1 } else { 64 }
}

/// Pickaxe tier 1..=4, or 0 if not a pickaxe.
pub fn pick_tier(id: u8) -> u8 {
    if (PICK_WOOD..=PICK_DIAMOND).contains(&id) { id - PICK_WOOD + 1 } else { 0 }
}

pub fn attack_damage(id: u8) -> f32 {
    match id {
        SWORD_WOOD..=SWORD_DIAMOND => 4.0 + (id - SWORD_WOOD) as f32,
        PICK_WOOD..=PICK_DIAMOND => 2.0 + (id - PICK_WOOD) as f32,
        _ => 1.0,
    }
}

/// Health restored when eaten, if edible.
pub fn food_value(id: u8) -> Option<f32> {
    match id {
        PORKCHOP => Some(6.0),
        GOO => Some(1.0),
        _ => None,
    }
}

/// Seconds to break `id` while holding `held`, and whether it drops anything.
pub fn break_time(id: u8, held: u8) -> (f32, bool) {
    let b = block(id);
    if b.hardness < 0.0 {
        return (f32::INFINITY, false);
    }
    if !b.pick_block {
        return (b.hardness, true);
    }
    let tier = pick_tier(held);
    if tier == 0 {
        return (b.hardness * 5.0, b.pick_tier == 0);
    }
    let speed = [1.0, 2.0, 4.0, 6.0, 8.0][tier as usize];
    (b.hardness * 1.5 / speed, tier >= b.pick_tier)
}

pub struct Recipe {
    pub inputs: &'static [(u8, u8)],
    pub output: (u8, u8),
}

pub static RECIPES: &[Recipe] = &[
    Recipe { inputs: &[(LOG, 1)], output: (PLANKS, 4) },
    Recipe { inputs: &[(PLANKS, 2)], output: (STICK, 4) },
    Recipe { inputs: &[(PLANKS, 4)], output: (TABLE, 1) },
    Recipe { inputs: &[(STICK, 1), (COAL, 1)], output: (TORCH, 4) },
    Recipe { inputs: &[(PLANKS, 3), (STICK, 2)], output: (PICK_WOOD, 1) },
    Recipe { inputs: &[(COBBLE, 3), (STICK, 2)], output: (PICK_STONE, 1) },
    Recipe { inputs: &[(IRON, 3), (STICK, 2)], output: (PICK_IRON, 1) },
    Recipe { inputs: &[(DIAMOND, 3), (STICK, 2)], output: (PICK_DIAMOND, 1) },
    Recipe { inputs: &[(PLANKS, 2), (STICK, 1)], output: (SWORD_WOOD, 1) },
    Recipe { inputs: &[(COBBLE, 2), (STICK, 1)], output: (SWORD_STONE, 1) },
    Recipe { inputs: &[(IRON, 2), (STICK, 1)], output: (SWORD_IRON, 1) },
    Recipe { inputs: &[(DIAMOND, 2), (STICK, 1)], output: (SWORD_DIAMOND, 1) },
    Recipe { inputs: &[(COBBLE, 1), (COAL, 1)], output: (STONE, 1) },
    Recipe { inputs: &[(SAND, 4), (COAL, 1)], output: (GLASS, 4) },
    Recipe { inputs: &[(DIRT, 4), (COAL, 1)], output: (BRICK, 4) },
    Recipe { inputs: &[(GUNPOWDER, 5), (SAND, 4)], output: (TNT, 1) },
    Recipe { inputs: &[(TORCH, 4), (GLASS, 1)], output: (GLOWROCK, 1) },
];

/// Everything the creative palette offers.
pub fn creative_items() -> Vec<u8> {
    let mut v: Vec<u8> = (1..NUM_BLOCKS).filter(|&b| b != WATER).collect();
    v.extend([STICK, COAL, IRON, DIAMOND, GUNPOWDER, PORKCHOP, GOO]);
    v.extend(PICK_WOOD..=SWORD_DIAMOND);
    v
}
