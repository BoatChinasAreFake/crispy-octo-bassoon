//! Blocks, items, tools and recipes, held in a registry that mods extend.
//!
//! Every block and item id is an [`Id`] (two bytes): ids below `FIRST_ITEM` are
//! placeable blocks (base game `0..NUM_BLOCKS`, mods `NUM_BLOCKS..FIRST_ITEM`),
//! the rest are plain items (base game `FIRST_ITEM..FIRST_MOD_ITEM`, mods after
//! that). Saves and the network store names for mod things, so the numbers
//! themselves can move between versions.

use crate::texture::*;
use std::sync::atomic::{AtomicPtr, AtomicU32, Ordering};

/// A block or item id.
pub type Id = u16;

pub const AIR: Id = 0;
pub const GRASS: Id = 1;
pub const DIRT: Id = 2;
pub const STONE: Id = 3;
pub const COBBLE: Id = 4;
pub const SAND: Id = 5;
pub const GRAVEL: Id = 6;
pub const WATER: Id = 7;
pub const LOG: Id = 8;
pub const LEAVES: Id = 9;
pub const PLANKS: Id = 10;
pub const GLASS: Id = 11;
pub const BEDROCK: Id = 12;
pub const COAL_ORE: Id = 13;
pub const IRON_ORE: Id = 14;
pub const DIAMOND_ORE: Id = 15;
pub const SNOW_GRASS: Id = 16;
pub const BRICK: Id = 17;
pub const TNT: Id = 18;
pub const TABLE: Id = 19;
pub const GLOWROCK: Id = 20;
pub const TORCH: Id = 21;
pub const FLOWER: Id = 22;
pub const TALL_GRASS: Id = 23;
pub const GOLD_ORE: Id = 24;
pub const PUMPKIN: Id = 25;
pub const JACK: Id = 26;
pub const CACTUS: Id = 27;
pub const ICE: Id = 28;
pub const BOUNCY: Id = 29;
pub const BED: Id = 30;
pub const CAKE: Id = 31;
pub const SPONGE: Id = 32;
pub const WOOL: Id = 33;
pub const SANDSTONE: Id = 34;
pub const STONE_BRICKS: Id = 35;
pub const MOSSY_COBBLE: Id = 36;
pub const HAY: Id = 37;
pub const BOOKSHELF: Id = 38;
pub const LANTERN: Id = 39;
pub const MUSHROOM: Id = 40;
pub const SCARECROW: Id = 41;
pub const WEEDS: Id = 42;
pub const FARMLAND: Id = 43;
pub const FARMLAND_WET: Id = 44;
/// Crops: four growth stages each, in order (see farming.rs).
pub const WHEAT_0: Id = 45;
pub const CARROT_0: Id = 49;
pub const POTATO_0: Id = 53;
pub const CHEST: Id = 57;
pub const FURNACE: Id = 58;
/// A furnace while it's burning (it glows); the same container.
pub const FURNACE_LIT: Id = 59;
/// Slabs: `SLAB_FIRST + material * 2 + top` (see `MATERIALS`). The bottom one is the item.
pub const SLAB_FIRST: Id = 60;
/// Stairs: `STAIRS_FIRST + material * 4 + facing`. The north-facing one is the item.
pub const STAIRS_FIRST: Id = 66;
/// Door halves: `DOOR_FIRST + facing * 4 + open * 2 + top`. Placed with the `DOOR` item.
pub const DOOR_FIRST: Id = 78;
/// Anvils, from new to nearly broken (each use may knock it down a stage; see anvil.rs).
pub const ANVIL: Id = 94;
pub const ANVIL_CHIPPED: Id = 95;
pub const ANVIL_DAMAGED: Id = 96;
/// Cave decorations: a mushroom that glows, and pointy rocks (up from floors, down from ceilings).
pub const GLOWSHROOM: Id = 97;
pub const POINTY_ROCK: Id = 98;
pub const ENCHANTING_TABLE: Id = 99;
/// Flowing water: `WATER_FLOW + level - 1`, levels 1 (next to the source) to 7 (see liquids.rs).
pub const WATER_FLOW: Id = 100;
pub const LAVA: Id = 107;
/// Flowing lava: `LAVA_FLOW + level - 1`, levels 1 to 3.
pub const LAVA_FLOW: Id = 108;
pub const OBSIDIAN: Id = 111;
/// Zappy Dust (legally distinct redstone; see wiring.rs): the ore, the wire
/// (off, on), switches (off, on), the block that's always on, and a lamp.
pub const ZAP_ORE: Id = 112;
pub const WIRE: Id = 113;
pub const WIRE_ON: Id = 114;
pub const LEVER: Id = 115;
pub const LEVER_ON: Id = 116;
pub const BUTTON: Id = 117;
pub const BUTTON_ON: Id = 118;
pub const PLATE: Id = 119;
pub const PLATE_ON: Id = 120;
pub const ZAP_BLOCK: Id = 121;
pub const LAMP: Id = 122;
pub const LAMP_ON: Id = 123;
/// The Scorchlands (see scorch.rs): its rock, slow sand, gold ore, and portals
/// (spanning x or z).
pub const SCORCHROCK: Id = 124;
pub const EMBERSAND: Id = 125;
pub const SCORCH_GOLD_ORE: Id = 126;
pub const PORTAL_X: Id = 127;
pub const PORTAL_Z: Id = 128;
/// Rails (see vehicles.rs): `RAIL_FIRST + shape`, shapes north-south,
/// east-west, then corners NE, NW, SE, SW. The north-south one is the item.
pub const RAIL_FIRST: Id = 129;
/// Powered rails: `POWERED_RAIL + axis * 2 + on` (axis 0 north-south, 1 east-west).
pub const POWERED_RAIL: Id = 135;
/// Signs and item frames: `+ facing` (see decor.rs).
pub const SIGN_FIRST: Id = 139;
pub const FRAME_FIRST: Id = 143;
/// Number of base-game blocks; mod blocks start here.
/// Grows into a tree (see trees.rs).
pub const SAPLING: Id = 147;
/// Building bits (see carpentry.rs). Fences and panes: `FIRST + mask` of the
/// sides they join (1 east, 2 west, 4 south, 8 north).
pub const FENCE_FIRST: Id = 148;
pub const PANE_FIRST: Id = 164;
/// Gates: `GATE_FIRST + x_axis * 2 + open` (x_axis: it spans east-west).
pub const GATE_FIRST: Id = 180;
/// Ladders: `LADDER_FIRST + facing` (the wall they're on, like item frames).
pub const LADDER_FIRST: Id = 184;
/// Trapdoors: `TRAPDOOR_FIRST + facing * 2 + open` (the hinge side).
pub const TRAPDOOR_FIRST: Id = 188;
/// Coloured wool (`DYED_WOOL + colour - 1`; colour 0, white, is plain Wool) and
/// stained glass (`STAINED_GLASS + colour`). Colours: see carpentry::COLOURS.
pub const DYED_WOOL: Id = 196;
pub const STAINED_GLASS: Id = 203;
/// Burning (see fire.rs).
pub const FIRE: Id = 211;
/// Potions (see potions.rs): a Scorchlands ingredient, and the stand.
pub const EMBER_SHROOM: Id = 212;
pub const BREWING_STAND: Id = 213;
/// Contraptions (see contraptions.rs). Zappy Torches lit and out.
pub const ZTORCH_ON: Id = 214;
pub const ZTORCH_OFF: Id = 215;
/// Repeaters: `REPEATER_FIRST + facing * 2 + on` (facing: the way power goes).
pub const REPEATER_FIRST: Id = 216;
/// Pistons and sticky pistons: `+ facing * 2 + extended` (six facings, see contraptions::dir6).
pub const PISTON_FIRST: Id = 224;
pub const STICKY_FIRST: Id = 236;
/// Piston heads: `HEAD_FIRST + facing * 2 + sticky`.
pub const HEAD_FIRST: Id = 248;
/// Dispensers: `DISPENSER_FIRST + facing`.
pub const DISPENSER_FIRST: Id = 260;
/// Hoppers: `HOPPER_FIRST` spouts down, `+ 1 + facing` to a side (see hoppers.rs).
pub const HOPPER_FIRST: Id = 266;
pub const NUM_BLOCKS: Id = 271;
/// Half the id space for blocks, half for items.
pub const FIRST_ITEM: Id = 0x8000;

pub const STICK: Id = FIRST_ITEM;
pub const COAL: Id = FIRST_ITEM + 1;
pub const IRON: Id = FIRST_ITEM + 2;
pub const DIAMOND: Id = FIRST_ITEM + 3;
pub const GUNPOWDER: Id = FIRST_ITEM + 4;
pub const PORKCHOP: Id = FIRST_ITEM + 5;
pub const GOO: Id = FIRST_ITEM + 6;
pub const PICK_WOOD: Id = FIRST_ITEM + 10;
pub const PICK_STONE: Id = FIRST_ITEM + 11;
pub const PICK_IRON: Id = FIRST_ITEM + 12;
pub const PICK_DIAMOND: Id = FIRST_ITEM + 13;
pub const SWORD_WOOD: Id = FIRST_ITEM + 14;
pub const SWORD_STONE: Id = FIRST_ITEM + 15;
pub const SWORD_IRON: Id = FIRST_ITEM + 16;
pub const SWORD_DIAMOND: Id = FIRST_ITEM + 17;
pub const GOLD_INGOT: Id = FIRST_ITEM + 18;
pub const GOLDEN_CHOP: Id = FIRST_ITEM + 19;
pub const PEARL: Id = FIRST_ITEM + 20;
pub const MUTTON: Id = FIRST_ITEM + 21;
pub const FEATHER: Id = FIRST_ITEM + 22;
pub const CLUCKETS: Id = FIRST_ITEM + 23;
pub const MOO_STEAK: Id = FIRST_ITEM + 24;
pub const BONE: Id = FIRST_ITEM + 25;
pub const ARROW: Id = FIRST_ITEM + 26;
pub const STRING: Id = FIRST_ITEM + 27;
pub const BOW: Id = FIRST_ITEM + 28;
pub const HOE: Id = FIRST_ITEM + 29;
pub const WHEAT_SEEDS: Id = FIRST_ITEM + 30;
pub const WHEAT: Id = FIRST_ITEM + 31;
pub const CARROT: Id = FIRST_ITEM + 32;
pub const POTATO: Id = FIRST_ITEM + 33;
pub const BONE_DUST: Id = FIRST_ITEM + 34;
pub const COMPOST: Id = FIRST_ITEM + 35;
pub const WOOD_ASH: Id = FIRST_ITEM + 36;
pub const SOIL_PROBE: Id = FIRST_ITEM + 37;
pub const BREAD: Id = FIRST_ITEM + 38;
pub const ROD: Id = FIRST_ITEM + 39;
pub const COD: Id = FIRST_ITEM + 40;
pub const SALMON: Id = FIRST_ITEM + 41;
pub const PUFFER: Id = FIRST_ITEM + 42;
pub const TROPICAL: Id = FIRST_ITEM + 43;
pub const BIG_BOB: Id = FIRST_ITEM + 44;
pub const BOOT: Id = FIRST_ITEM + 45;
pub const BOTTLE: Id = FIRST_ITEM + 46;
pub const FISH_CHIPS: Id = FIRST_ITEM + 47;
pub const STEW: Id = FIRST_ITEM + 48;
pub const BAIT: Id = FIRST_ITEM + 49;
pub const COOKED_CHOP: Id = FIRST_ITEM + 50;
pub const COOKED_MUTTON: Id = FIRST_ITEM + 51;
pub const COOKED_CLUCKETS: Id = FIRST_ITEM + 52;
pub const STEAK: Id = FIRST_ITEM + 53;
pub const COOKED_COD: Id = FIRST_ITEM + 54;
pub const COOKED_SALMON: Id = FIRST_ITEM + 55;
pub const BAKED_POTATO: Id = FIRST_ITEM + 56;
pub const COOKED_PUFFER: Id = FIRST_ITEM + 57;
pub const COOKED_BOOT: Id = FIRST_ITEM + 58;
pub const DOOR: Id = FIRST_ITEM + 59;
/// Armour: `ARMOR_FIRST + tier * 4 + slot` (see `armor_of`).
pub const ARMOR_FIRST: Id = FIRST_ITEM + 60;
pub const BUCKET: Id = FIRST_ITEM + 76;
pub const WATER_BUCKET: Id = FIRST_ITEM + 77;
pub const LAVA_BUCKET: Id = FIRST_ITEM + 78;
pub const SHEARS: Id = FIRST_ITEM + 79;
pub const ZAP_DUST: Id = FIRST_ITEM + 80;
pub const BOOK: Id = FIRST_ITEM + 81;
/// Carries its enchantments in its wear, like a tool (see enchant.rs).
pub const ENCHANTED_BOOK: Id = FIRST_ITEM + 82;
pub const SHIELD: Id = FIRST_ITEM + 83;
/// Lights portals (and TNT).
pub const SPARKER: Id = FIRST_ITEM + 84;
pub const BOAT: Id = FIRST_ITEM + 85;
pub const MINECART: Id = FIRST_ITEM + 86;
pub const COMPASS: Id = FIRST_ITEM + 87;
pub const MAP: Id = FIRST_ITEM + 88;
pub const APPLE: Id = FIRST_ITEM + 89;
/// Dyes, one per colour (see carpentry::COLOURS).
pub const DYE_FIRST: Id = FIRST_ITEM + 90;
pub const GLASS_BOTTLE: Id = FIRST_ITEM + 98;
pub const WATER_BOTTLE: Id = FIRST_ITEM + 99;
/// Potions, then their splash versions, in `potions::ALL` order.
pub const POTION_FIRST: Id = FIRST_ITEM + 100;
pub const SPLASH_FIRST: Id = FIRST_ITEM + 105;
/// Dropped by Grumblers; makes a Brewing Stand.
pub const GRUMBLER_TUSK: Id = FIRST_ITEM + 110;
/// Goes on a tamed Galloper (see horses.rs).
pub const SADDLE: Id = FIRST_ITEM + 111;
/// Mod items start here.
pub const FIRST_MOD_ITEM: Id = FIRST_ITEM + 112;

/// Longest a liquid runs from its source: water 7 blocks, lava 3.
pub const WATER_REACH: u8 = 7;
pub const LAVA_REACH: u8 = 3;

pub fn is_water(id: Id) -> bool {
    id == WATER || (WATER_FLOW..WATER_FLOW + WATER_REACH as Id).contains(&id)
}
pub fn is_lava(id: Id) -> bool {
    id == LAVA || (LAVA_FLOW..LAVA_FLOW + LAVA_REACH as Id).contains(&id)
}
/// Part of a Zappy Dust contraption (wires, switches, lamps; see wiring.rs).
pub fn is_zappy(id: Id) -> bool {
    (WIRE..=LAMP_ON).contains(&id) || (RAIL_FIRST..POWERED_RAIL + 4).contains(&id) || crate::contraptions::is_contraption(id)
}
pub fn is_liquid(id: Id) -> bool {
    is_water(id) || is_lava(id)
}
/// How far a liquid block is from its source (0: it is the source).
pub fn liquid_level(id: Id) -> u8 {
    match id {
        WATER | LAVA => 0,
        _ if is_water(id) => (id - WATER_FLOW) as u8 + 1,
        _ if is_lava(id) => (id - LAVA_FLOW) as u8 + 1,
        _ => 0,
    }
}
/// Water or lava at `level` (0: a source).
pub fn liquid_at(lava: bool, level: u8) -> Id {
    match (lava, level) {
        (false, 0) => WATER,
        (true, 0) => LAVA,
        (false, l) => WATER_FLOW + l.min(WATER_REACH) as Id - 1,
        (true, l) => LAVA_FLOW + l.min(LAVA_REACH) as Id - 1,
    }
}

/// What slabs and stairs are made of: the full block, and its name.
pub const MATERIALS: [(Id, &str); 3] = [(PLANKS, "Planks"), (COBBLE, "Cobblestun"), (STONE_BRICKS, "Stone Brick")];

pub const fn slab(material: usize, top: bool) -> Id {
    SLAB_FIRST + material as Id * 2 + top as Id
}
pub const fn stairs(material: usize, facing: u8) -> Id {
    STAIRS_FIRST + material as Id * 4 + facing as Id
}
pub const fn door(facing: u8, open: bool, top: bool) -> Id {
    DOOR_FIRST + facing as Id * 4 + open as Id * 2 + top as Id
}

/// Armour slots, in order.
pub const HELMET: usize = 0;
pub const CHESTPLATE: usize = 1;
pub const LEGGINGS: usize = 2;
pub const BOOTS: usize = 3;
/// Armour points per tier (wool, iron, gold, dimond) and slot. 20 points is
/// Minecraft's full dimond set; each point takes 4% off most damage.
pub const ARMOR_POINTS: [[u8; 4]; 4] = [[1, 3, 2, 1], [2, 6, 5, 2], [2, 5, 3, 1], [3, 8, 6, 3]];

/// (slot, tier) of an armour item.
/// For mod armour, the tier is the one it looks like when worn.
pub fn armor_of(id: Id) -> Option<(usize, usize)> {
    if (ARMOR_FIRST..ARMOR_FIRST + 16).contains(&id) {
        return Some((((id - ARMOR_FIRST) % 4) as usize, ((id - ARMOR_FIRST) / 4) as usize));
    }
    item_def(id).and_then(|i| i.armor).map(|a| (a.slot as usize, a.looks_like as usize))
}

/// How many uses a tool, weapon or piece of armour survives (Minecraft's numbers;
/// wool armour takes leather's). None: it never wears out.
pub fn durability(id: Id) -> Option<u16> {
    const ARMOR: [[u16; 4]; 4] = [[55, 80, 75, 65], [165, 240, 225, 195], [77, 112, 105, 91], [363, 528, 495, 429]];
    if let Some((slot, tier)) = armor_of(id) {
        return Some(ARMOR[tier][slot]);
    }
    Some(match id {
        PICK_WOOD | SWORD_WOOD | HOE => 59,
        PICK_STONE | SWORD_STONE => 131,
        PICK_IRON | SWORD_IRON => 250,
        PICK_DIAMOND | SWORD_DIAMOND => 1561,
        BOW => 384,
        ROD => 64,
        SHEARS => 238,
        SHIELD => 336,
        SPARKER => 64,
        _ => return item_def(id).and_then(|i| i.durability),
    })
}

/// Swords (and mod weapons: things that wear out, hit hard and aren't pickaxes).
pub fn is_sword(id: Id) -> bool {
    matches!(id, SWORD_WOOD | SWORD_STONE | SWORD_IRON | SWORD_DIAMOND)
        || (id >= FIRST_MOD_ITEM && item_def(id).is_some_and(|i| i.durability.is_some() && i.pick_tier == 0 && i.armor.is_none() && i.damage > 1.0))
}

/// Wear from breaking a block with `held` (swords aren't meant for digging).
pub fn dig_wear(held: Id, broken: Id) -> u16 {
    if durability(held).is_none() || armor_of(held).is_some() || held == SHIELD || block(broken).hardness <= 0.0 {
        return 0;
    }
    if is_sword(held) { 2 } else { 1 }
}

/// Wear from hitting a mob with `held` (anything but a sword is a clumsy weapon).
pub fn hit_wear(held: Id) -> u16 {
    if durability(held).is_none() || armor_of(held).is_some() || matches!(held, BOW | ROD | SHEARS | SHIELD | SPARKER) {
        return 0;
    }
    if is_sword(held) { 1 } else { 2 }
}

pub fn armor_points(id: Id) -> u8 {
    if let Some(a) = item_def(id).and_then(|i| i.armor) {
        return a.points;
    }
    armor_of(id).map(|(slot, tier)| ARMOR_POINTS[tier][slot]).unwrap_or(0)
}

/// Facings: 0 north (-z), 1 east (+x), 2 south (+z), 3 west (-x).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shape {
    Full,
    Slab { top: bool },
    /// `facing` is the side the high step is on.
    Stairs { facing: u8 },
    /// Closed, the panel sits on the `facing` side of its cell.
    Door { facing: u8, open: bool, top: bool },
    Anvil,
    /// Three quarters of a block tall (the enchanting table).
    Table,
    /// A thin layer on the floor (Zappy Dust).
    Dust,
    /// A pressure plate, a little lower when stood on.
    Plate { down: bool },
    /// A button on the floor, pressed in or not.
    Button { down: bool },
    /// A portal's shimmering sheet, across x (or z).
    Portal { x_axis: bool },
    /// A signpost: its board faces north-south (or east-west).
    Sign { facing: u8 },
    /// A frame hung on one side of its cell.
    Frame { facing: u8 },
    /// A fence post with rails to the sides in `mask` (1 east, 2 west, 4 south, 8 north).
    Fence { mask: u8 },
    /// A glass pane, joined to the sides in `mask`.
    Pane { mask: u8 },
    /// A fence gate across x (or z), open or shut.
    Gate { x_axis: bool, open: bool },
    /// A ladder on the `facing` side of its cell.
    Ladder { facing: u8 },
    /// A trapdoor: shut, a thin floor; open, flipped up against its `facing` side.
    Trapdoor { facing: u8, open: bool },
    /// A brewing stand: a base and a rod.
    Brewer,
    /// A repeater: a thin plate (with a little post showing which way it points).
    Repeater { facing: u8 },
    /// A piston's body (shorter when its head is out), facing one of six ways.
    Piston { facing: u8, extended: bool },
    /// A piston's head: a plate at the front and a rod back to the body.
    PistonHead { facing: u8 },
    /// A hopper: a bowl, a funnel and a spout (0 down, else 1 + a side facing).
    Hopper { spout: u8 },
}

/// A box covering `a..b` of a cell measured along direction `facing` (see
/// contraptions::dir6), and all of it across.
fn along(facing: u8, a: f32, b: f32) -> Aabb {
    let (mut lo, mut hi) = ([0.0f32; 3], [1.0f32; 3]);
    let (axis, pos) = match facing % 6 {
        0 => (2, false),
        1 => (0, true),
        2 => (2, true),
        3 => (0, false),
        4 => (1, true),
        _ => (1, false),
    };
    if pos {
        lo[axis] = a;
        hi[axis] = b;
    } else {
        lo[axis] = 1.0 - b;
        hi[axis] = 1.0 - a;
    }
    (lo, hi)
}

/// The rod through the middle of a cell along `facing`, over `a..b`.
fn rod(facing: u8, a: f32, b: f32) -> Aabb {
    let (mut lo, mut hi) = along(facing, a, b);
    for k in 0..3 {
        if lo[k] == 0.0 && hi[k] == 1.0 {
            lo[k] = 0.375;
            hi[k] = 0.625;
        }
    }
    (lo, hi)
}

/// An axis-aligned box inside a block cell, in 0..1 coordinates.
pub type Aabb = ([f32; 3], [f32; 3]);

const DOOR_T: f32 = 3.0 / 16.0;

/// The thin panel along one side of a cell.
fn side_box(facing: u8) -> Aabb {
    match facing % 4 {
        0 => ([0.0, 0.0, 0.0], [1.0, 1.0, DOOR_T]),
        1 => ([1.0 - DOOR_T, 0.0, 0.0], [1.0, 1.0, 1.0]),
        2 => ([0.0, 0.0, 1.0 - DOOR_T], [1.0, 1.0, 1.0]),
        _ => ([0.0, 0.0, 0.0], [DOOR_T, 1.0, 1.0]),
    }
}

/// The half of a cell toward `facing`, between heights `y0` and `y1`.
fn half_box(facing: u8, y0: f32, y1: f32) -> Aabb {
    match facing % 4 {
        0 => ([0.0, y0, 0.0], [1.0, y1, 0.5]),
        1 => ([0.5, y0, 0.0], [1.0, y1, 1.0]),
        2 => ([0.0, y0, 0.5], [1.0, y1, 1.0]),
        _ => ([0.0, y0, 0.0], [0.5, y1, 1.0]),
    }
}

impl Shape {
    /// Up to three boxes (the count is the second value).
    pub fn boxes(self) -> ([Aabb; 3], usize) {
        let full = ([0.0; 3], [1.0; 3]);
        match self {
            Shape::Full => ([full; 3], 1),
            Shape::Slab { top: false } => ([([0.0; 3], [1.0, 0.5, 1.0]), full, full], 1),
            Shape::Slab { top: true } => ([([0.0, 0.5, 0.0], [1.0; 3]), full, full], 1),
            Shape::Stairs { facing } => ([([0.0; 3], [1.0, 0.5, 1.0]), half_box(facing, 0.5, 1.0), full], 2),
            Shape::Door { facing, open, .. } => ([side_box(if open { facing + 3 } else { facing }), full, full], 1),
            // A foot, a waist and a long top, lengthways along x.
            Shape::Anvil => ([([0.125, 0.0, 0.125], [0.875, 0.25, 0.875]), ([0.25, 0.25, 0.3125], [0.75, 0.625, 0.6875]), ([0.0, 0.625, 0.1875], [1.0, 1.0, 0.8125])], 3),
            Shape::Table => ([([0.0; 3], [1.0, 0.75, 1.0]), full, full], 1),
            Shape::Dust => ([([0.0; 3], [1.0, 1.0 / 16.0, 1.0]), full, full], 1),
            Shape::Plate { down } => ([([1.0 / 16.0, 0.0, 1.0 / 16.0], [15.0 / 16.0, if down { 1.0 / 32.0 } else { 1.0 / 16.0 }, 15.0 / 16.0]), full, full], 1),
            Shape::Button { down } => ([([5.0 / 16.0, 0.0, 6.0 / 16.0], [11.0 / 16.0, if down { 1.0 / 16.0 } else { 2.0 / 16.0 }, 10.0 / 16.0]), full, full], 1),
            Shape::Portal { x_axis: true } => ([([0.0, 0.0, 0.375], [1.0, 1.0, 0.625]), full, full], 1),
            Shape::Portal { x_axis: false } => ([([0.375, 0.0, 0.0], [0.625, 1.0, 1.0]), full, full], 1),
            Shape::Sign { facing } => {
                let post = ([0.44, 0.0, 0.44], [0.56, 0.55, 0.56]);
                let board = if facing % 2 == 0 { ([0.05, 0.55, 0.44], [0.95, 1.0, 0.56]) } else { ([0.44, 0.55, 0.05], [0.56, 1.0, 0.95]) };
                ([post, board, full], 2)
            }
            Shape::Fence { mask } => {
                let post = ([0.375, 0.0, 0.375], [0.625, 1.0, 0.625]);
                let (mut v, mut n) = ([post, post, post], 1);
                let (y0, y1) = (0.375, 0.9375);
                if mask & 3 != 0 {
                    v[n] = ([if mask & 2 != 0 { 0.0 } else { 0.5 }, y0, 0.4375], [if mask & 1 != 0 { 1.0 } else { 0.5 }, y1, 0.5625]);
                    n += 1;
                }
                if mask & 12 != 0 {
                    v[n] = ([0.4375, y0, if mask & 8 != 0 { 0.0 } else { 0.5 }], [0.5625, y1, if mask & 4 != 0 { 1.0 } else { 0.5 }]);
                    n += 1;
                }
                (v, n)
            }
            Shape::Pane { mask } => {
                let post = ([0.4375, 0.0, 0.4375], [0.5625, 1.0, 0.5625]);
                let (mut v, mut n) = ([post, post, post], 1);
                if mask & 3 != 0 {
                    v[n] = ([if mask & 2 != 0 { 0.0 } else { 0.5 }, 0.0, 0.4375], [if mask & 1 != 0 { 1.0 } else { 0.5 }, 1.0, 0.5625]);
                    n += 1;
                }
                if mask & 12 != 0 {
                    v[n] = ([0.4375, 0.0, if mask & 8 != 0 { 0.0 } else { 0.5 }], [0.5625, 1.0, if mask & 4 != 0 { 1.0 } else { 0.5 }]);
                    n += 1;
                }
                (v, n)
            }
            Shape::Gate { x_axis, open: false } => {
                let b = if x_axis { ([0.0, 0.375, 0.4375], [1.0, 1.0, 0.5625]) } else { ([0.4375, 0.375, 0.0], [0.5625, 1.0, 1.0]) };
                ([b, full, full], 1)
            }
            Shape::Gate { x_axis, open: true } => {
                // Two halves swung back against the posts' sides.
                let (a, b) = if x_axis {
                    (([0.0, 0.375, 0.4375], [0.125, 1.0, 1.0]), ([0.875, 0.375, 0.4375], [1.0, 1.0, 1.0]))
                } else {
                    (([0.4375, 0.375, 0.0], [1.0, 1.0, 0.125]), ([0.4375, 0.375, 0.875], [1.0, 1.0, 1.0]))
                };
                ([a, b, full], 2)
            }
            Shape::Ladder { facing } => {
                let t = 0.125;
                let b = match facing % 4 {
                    0 => ([0.0, 0.0, 0.0], [1.0, 1.0, t]),
                    1 => ([1.0 - t, 0.0, 0.0], [1.0, 1.0, 1.0]),
                    2 => ([0.0, 0.0, 1.0 - t], [1.0, 1.0, 1.0]),
                    _ => ([0.0, 0.0, 0.0], [t, 1.0, 1.0]),
                };
                ([b, full, full], 1)
            }
            Shape::Repeater { facing } => {
                let d = [[0.5, 0.2], [0.8, 0.5], [0.5, 0.8], [0.2, 0.5]][(facing % 4) as usize];
                let post = ([d[0] - 0.07, 0.125, d[1] - 0.07], [d[0] + 0.07, 0.3125, d[1] + 0.07]);
                ([([0.0; 3], [1.0, 0.125, 1.0]), post, full], 2)
            }
            Shape::Piston { extended: false, .. } => ([full, full, full], 1),
            Shape::Piston { facing, extended: true } => ([along(facing, 0.0, 0.75), full, full], 1),
            Shape::PistonHead { facing } => ([along(facing, 0.75, 1.0), rod(facing, -0.25, 0.75), full], 2),
            Shape::Hopper { spout } => {
                let bowl = ([0.0, 0.625, 0.0], [1.0, 1.0, 1.0]);
                let funnel = ([0.25, 0.25, 0.25], [0.75, 0.625, 0.75]);
                let tip = match spout {
                    0 => ([0.375, 0.0, 0.375], [0.625, 0.25, 0.625]),
                    k => {
                        let (lo, hi) = rod(k - 1, 0.0, 0.25);
                        ([lo[0], 0.25, lo[2]], [hi[0], 0.5, hi[2]])
                    }
                };
                ([bowl, funnel, tip], 3)
            }
            Shape::Brewer => ([([0.0625, 0.0, 0.0625], [0.9375, 0.125, 0.9375]), ([0.4375, 0.125, 0.4375], [0.5625, 0.875, 0.5625]), ([0.25, 0.5, 0.4375], [0.75, 0.625, 0.5625])], 3),
            Shape::Trapdoor { open: false, .. } => ([([0.0; 3], [1.0, 0.1875, 1.0]), full, full], 1),
            Shape::Trapdoor { facing, open: true } => ([side_box(facing), full, full], 1),
            Shape::Frame { facing } => {
                let t = 1.0 / 16.0;
                let b = match facing % 4 {
                    0 => ([0.125, 0.125, 0.0], [0.875, 0.875, t]),
                    1 => ([1.0 - t, 0.125, 0.125], [1.0, 0.875, 0.875]),
                    2 => ([0.125, 0.125, 1.0 - t], [0.875, 0.875, 1.0]),
                    _ => ([0.0, 0.125, 0.125], [t, 0.875, 0.875]),
                };
                ([b, full, full], 1)
            }
        }
    }
}

/// Collision and targeting boxes of a block (a full cube for most).
#[inline]
pub fn block_boxes(id: Id) -> ([Aabb; 3], usize) {
    block(id).shape.boxes()
}

pub fn is_door(id: Id) -> bool {
    (DOOR_FIRST..DOOR_FIRST + 16).contains(&id)
}

/// A door half's (facing, open, top).
pub fn door_state(id: Id) -> Option<(u8, bool, bool)> {
    is_door(id).then(|| {
        let k = id - DOOR_FIRST;
        ((k / 4) as u8, k & 2 != 0, k & 1 != 0)
    })
}

/// A slab's (bottom slab, which is the item; top). A slab's variants are
/// consecutive ids: bottom, then top.
pub fn slab_of(id: Id) -> Option<(Id, bool)> {
    match block(id).shape {
        Shape::Slab { top } => Some((block(id).family, top)),
        _ => None,
    }
}

/// A stair's (north-facing stairs, which is the item; facing). Its four
/// facings are consecutive ids.
pub fn stairs_of(id: Id) -> Option<(Id, u8)> {
    match block(id).shape {
        Shape::Stairs { facing } => Some((block(id).family, facing)),
        _ => None,
    }
}

/// The full block a slab or stair is made of (two slabs merge into it), if any.
pub fn made_of(id: Id) -> Id {
    block(id).full
}

/// The item a player spends to place block `id` (None: not something players
/// place directly; door tops come free with their bottom half).
pub fn placing_item(id: Id) -> Option<Id> {
    if id == WIRE {
        return Some(ZAP_DUST);
    }
    // Any rail is placed as a straight one, then bends to fit (see vehicles.rs).
    if id == RAIL_FIRST || id == POWERED_RAIL {
        return Some(id);
    }
    // Signs and frames come in four facings; the item is the first.
    if (SIGN_FIRST..SIGN_FIRST + 4).contains(&id) {
        return Some(SIGN_FIRST);
    }
    if (FRAME_FIRST..FRAME_FIRST + 4).contains(&id) {
        return Some(FRAME_FIRST);
    }
    if crate::hoppers::is_hopper(id) {
        return Some(HOPPER_FIRST);
    }
    if let Some(f) = crate::carpentry::family(id).or_else(|| crate::contraptions::family(id)) {
        return Some(f);
    }
    if let Some((family, _)) = slab_of(id) {
        return Some(family);
    }
    if let Some((family, _)) = stairs_of(id) {
        return Some(family);
    }
    if let Some((_, _, top)) = door_state(id) {
        return (!top).then_some(DOOR);
    }
    (id != AIR && valid_block(id) && block(id).creative).then_some(id)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Model {
    Empty,
    Cube,
    Cross,
    Liquid,
    /// Slabs, stairs and doors: the boxes of its `shape`.
    Shaped,
}

/// Something a mod can make happen when an item is used or a block is broken.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Heal(f32),
    Explode(f32),
    Launch(f32),
    Message(&'static str),
    Give(Id, u8),
    SetTime(f32),
    /// Mob kind index (`MobKind::ALL`): 0 oinker, 1 hisser, 2 groaner, 3 fluffer, 4 starer.
    Spawn(u8),
}

#[derive(Clone)]
pub struct BlockDef {
    /// Stable identifier, e.g. "stone" or "cheese:cheese_block" (used by saves).
    pub key: &'static str,
    pub name: &'static str,
    pub model: Model,
    /// Collides with players and mobs.
    pub solid: bool,
    /// Fully hides neighbouring faces and casts ambient occlusion.
    pub opaque: bool,
    /// Glass-like: light passes, and touching faces of the same block are hidden.
    pub see_through: bool,
    /// Top, side, bottom texture tiles.
    pub tex: [u16; 3],
    /// Seconds to break by hand; negative means unbreakable.
    pub hardness: f32,
    /// Needs a pickaxe of at least this tier to drop anything (0 = anything goes).
    pub pick_tier: u8,
    /// Whether pickaxes speed it up at all.
    pub pick_block: bool,
    pub drop: Id,
    /// Point-light radius in blocks (0 = dark).
    pub light: f32,
    /// Sound family: 0 stone, 1 wood, 2 grass, 3 sand, 4 glass.
    pub sound: u8,
    /// Landing on it bounces you back up by this fraction of your fall speed.
    pub bounce: f32,
    /// Walking speed multiplier when standing on it.
    pub speed: f32,
    pub on_break: Vec<Action>,
    /// Shown in the creative palette.
    pub creative: bool,
    /// Its boxes, for `Model::Shaped` blocks (everything else is a full cube).
    pub shape: Shape,
    /// Slabs and stairs: the variant that is the item (bottom slab, north-facing stairs).
    pub family: Id,
    /// Slabs and stairs: the full block they're made of (AIR: none).
    pub full: Id,
}

pub struct ItemDef {
    pub key: &'static str,
    pub name: &'static str,
    pub tile: u16,
    pub stack: u8,
    /// Acts as a pickaxe of this tier (0 = not a pickaxe).
    pub pick_tier: u8,
    pub damage: f32,
    pub food: Option<f32>,
    pub on_use: Vec<Action>,
    /// Whether using it (for `on_use`) uses one up.
    pub consume: bool,
    /// Exists (the base item id range has gaps).
    pub real: bool,
    /// Mod tools, weapons and armour: uses before it breaks.
    pub durability: Option<u16>,
    /// Mod armour.
    pub armor: Option<ModArmor>,
    /// Mod tools and armour: what repairs it at an anvil (AIR: nothing).
    pub repair: Id,
}

/// A mod item worn as armour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModArmor {
    /// 0 helmet .. 3 boots.
    pub slot: u8,
    pub points: u8,
    /// Which base tier it looks like when worn (0 wool .. 3 dimond).
    pub looks_like: u8,
}

#[derive(Clone)]
pub struct Recipe {
    pub inputs: Vec<(Id, u8)>,
    pub output: (Id, u8),
}

/// Mod world generation: ore veins underground.
#[derive(Clone, Debug)]
pub struct OreGen {
    pub block: Id,
    pub replace: Id,
    pub min_y: i32,
    pub max_y: i32,
    pub chance: f32,
}

/// Mod world generation: plants on the surface.
#[derive(Clone, Debug)]
pub struct PlantGen {
    pub block: Id,
    pub on: Id,
    pub chance: f32,
}

#[derive(Clone, Debug, Default)]
pub struct ModInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub enabled: bool,
    pub errors: Vec<String>,
    /// Counts of what it added: blocks, items, recipes.
    pub added: (usize, usize, usize),
    /// Number of `.rhai` script files.
    pub scripts: usize,
}

pub struct Registry {
    pub blocks: Vec<BlockDef>,
    /// Indexed by `id - FIRST_ITEM`.
    pub items: Vec<ItemDef>,
    pub recipes: Vec<Recipe>,
    pub ores: Vec<OreGen>,
    pub plants: Vec<PlantGen>,
    pub splashes: Vec<String>,
    pub mods: Vec<ModInfo>,
    /// Extra texture tiles painted over the atlas: (tile, 16x16 RGBA).
    pub textures: Vec<(u16, Vec<u8>)>,
    /// Mod furnace recipes (input, output) and fuels (item, seconds).
    pub smelting: Vec<(Id, Id)>,
    pub fuels: Vec<(Id, f32)>,
}

static REGISTRY: AtomicPtr<Registry> = AtomicPtr::new(std::ptr::null_mut());
static GENERATION: AtomicU32 = AtomicU32::new(0);

/// The active registry. The base game is installed on first use.
#[inline]
pub fn reg() -> &'static Registry {
    let p = REGISTRY.load(Ordering::Acquire);
    if !p.is_null() {
        // SAFETY: registries are leaked on install and never freed.
        return unsafe { &*p };
    }
    let fresh = Box::into_raw(Box::new(Registry::base()));
    match REGISTRY.compare_exchange(std::ptr::null_mut(), fresh, Ordering::AcqRel, Ordering::Acquire) {
        Ok(_) => unsafe { &*fresh },
        Err(existing) => unsafe { &*existing },
    }
}

/// Make `r` the active registry. The old one is intentionally leaked: other
/// threads may still hold `&'static` references into it, and it's small.
pub fn install(r: Registry) {
    REGISTRY.store(Box::into_raw(Box::new(r)), Ordering::Release);
    GENERATION.fetch_add(1, Ordering::AcqRel);
}

/// Bumped on every install, so the renderer knows to rebuild the atlas.
pub fn generation() -> u32 {
    GENERATION.load(Ordering::Acquire)
}

pub(crate) fn leak(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

use Model::*;

#[allow(clippy::too_many_arguments)]
pub(crate) fn def(key: &'static str, name: &'static str, model: Model, solid: bool, opaque: bool, tex: [u16; 3], hardness: f32, pick_tier: u8, pick_block: bool, drop: Id, light: f32, sound: u8) -> BlockDef {
    BlockDef {
        key,
        name,
        model,
        solid,
        opaque,
        see_through: false,
        tex,
        hardness,
        pick_tier,
        pick_block,
        drop,
        light,
        sound,
        bounce: 0.0,
        speed: 1.0,
        on_break: Vec::new(),
        creative: true,
        shape: Shape::Full,
        family: AIR,
        full: AIR,
    }
}

pub(crate) fn item(key: &'static str, name: &'static str, tile: u16) -> ItemDef {
    ItemDef { key, name, tile, stack: 64, pick_tier: 0, damage: 1.0, food: None, on_use: Vec::new(), consume: true, real: true, durability: None, armor: None, repair: AIR }
}

const S_STONE: u8 = 0;
const S_WOOD: u8 = 1;
const S_GRASS: u8 = 2;
const S_SAND: u8 = 3;
const S_GLASS: u8 = 4;

impl Registry {
    pub fn base() -> Registry {
        let mut blocks = vec![
            def("air", "Air", Empty, false, false, [0, 0, 0], 0.0, 0, false, AIR, 0.0, S_GRASS),
            def("grass", "Grass Block", Cube, true, true, [T_GRASS_TOP, T_GRASS_SIDE, T_DIRT], 0.6, 0, false, DIRT, 0.0, S_GRASS),
            def("dirt", "Dirt (Premium)", Cube, true, true, [T_DIRT; 3], 0.5, 0, false, DIRT, 0.0, S_GRASS),
            def("stone", "Stone", Cube, true, true, [T_STONE; 3], 1.5, 1, true, COBBLE, 0.0, S_STONE),
            def("cobblestone", "Cobblestun", Cube, true, true, [T_COBBLE; 3], 2.0, 1, true, COBBLE, 0.0, S_STONE),
            def("sand", "Sand", Cube, true, true, [T_SAND; 3], 0.5, 0, false, SAND, 0.0, S_SAND),
            def("gravel", "Gravel", Cube, true, true, [T_GRAVEL; 3], 0.6, 0, false, GRAVEL, 0.0, S_SAND),
            def("water", "Water", Liquid, false, false, [T_WATER; 3], -1.0, 0, false, AIR, 0.0, S_GRASS),
            def("log", "Tree Chunk", Cube, true, true, [T_LOG_TOP, T_LOG_SIDE, T_LOG_TOP], 2.0, 0, false, LOG, 0.0, S_WOOD),
            def("leaves", "Leaves", Cube, true, false, [T_LEAVES; 3], 0.2, 0, false, AIR, 0.0, S_GRASS),
            def("planks", "Planks", Cube, true, true, [T_PLANKS; 3], 2.0, 0, false, PLANKS, 0.0, S_WOOD),
            def("glass", "Glass", Cube, true, false, [T_GLASS; 3], 0.3, 0, false, AIR, 0.0, S_GLASS),
            def("bedrock", "Bedrock (Nope)", Cube, true, true, [T_BEDROCK; 3], -1.0, 0, false, AIR, 0.0, S_STONE),
            def("coal_ore", "Coal Ore", Cube, true, true, [T_COAL_ORE; 3], 3.0, 1, true, COAL, 0.0, S_STONE),
            def("iron_ore", "Iron Ore", Cube, true, true, [T_IRON_ORE; 3], 3.0, 2, true, IRON, 0.0, S_STONE),
            def("diamond_ore", "Dimond Ore", Cube, true, true, [T_DIAMOND_ORE; 3], 3.0, 3, true, DIAMOND, 0.0, S_STONE),
            def("snowy_grass", "Snowy Grass", Cube, true, true, [T_SNOW, T_SNOW_SIDE, T_DIRT], 0.6, 0, false, DIRT, 0.0, S_GRASS),
            def("bricks", "Bricks", Cube, true, true, [T_BRICK; 3], 2.0, 1, true, BRICK, 0.0, S_STONE),
            def("tnt", "TNT (Totally Not Trouble)", Cube, true, true, [T_TNT_TOP, T_TNT_SIDE, T_TNT_BOTTOM], 0.0, 0, false, TNT, 0.0, S_GRASS),
            def("crafting_table", "Decorative Crafting Table", Cube, true, true, [T_TABLE_TOP, T_TABLE_SIDE, T_PLANKS], 2.5, 0, false, TABLE, 0.0, S_WOOD),
            def("glowrock", "Glowrock", Cube, true, true, [T_GLOW; 3], 0.3, 0, false, GLOWROCK, 11.0, S_STONE),
            def("torch", "Torch", Cross, false, false, [T_TORCH; 3], 0.0, 0, false, TORCH, 8.0, S_WOOD),
            def("flower", "Poppy-ish", Cross, false, false, [T_FLOWER; 3], 0.0, 0, false, FLOWER, 0.0, S_GRASS),
            def("tall_grass", "Tall Grass", Cross, false, false, [T_TALLGRASS; 3], 0.0, 0, false, AIR, 0.0, S_GRASS),
            def("gold_ore", "Gold Ore (Shiny, Useless)", Cube, true, true, [T_GOLD_ORE; 3], 3.0, 3, true, GOLD_INGOT, 0.0, S_STONE),
            def("pumpkin", "Pumpkin (Fruit? Vegetable? Yes.)", Cube, true, true, [T_PUMPKIN_TOP, T_PUMPKIN_SIDE, T_PUMPKIN_TOP], 1.0, 0, false, PUMPKIN, 0.0, S_WOOD),
            def("jack_o_lantern", "Jack o'Lantern (Faces Everywhere)", Cube, true, true, [T_PUMPKIN_TOP, T_JACK_FACE, T_PUMPKIN_TOP], 1.0, 0, false, JACK, 12.0, S_WOOD),
            def("cactus", "Pokey Plant (Do Not Hug)", Cube, true, true, [T_CACTUS_TOP, T_CACTUS_SIDE, T_CACTUS_TOP], 0.4, 0, false, CACTUS, 0.0, S_GRASS),
            def("ice", "Ice (Nature's Floor Wax)", Cube, true, true, [T_ICE; 3], 0.5, 0, false, AIR, 0.0, S_GLASS),
            def("bouncy_goo", "Bouncy Goo Block", Cube, true, true, [T_BOUNCY; 3], 0.3, 0, false, BOUNCY, 0.0, S_GRASS),
            def("bed", "Bed (One Block, Budget Cuts)", Cube, true, true, [T_BED_TOP, T_BED_SIDE, T_PLANKS], 0.4, 0, false, BED, 0.0, S_WOOD),
            def("cake", "Cake (Not a Lie)", Cube, true, true, [T_CAKE_TOP, T_CAKE_SIDE, T_CAKE_SIDE], 0.5, 0, false, AIR, 0.0, S_GRASS),
            def("sponge", "Sponge (Very Thirsty)", Cube, true, true, [T_SPONGE; 3], 0.6, 0, false, SPONGE, 0.0, S_GRASS),
            def("wool", "Wool (Ethically Sheared)", Cube, true, true, [T_WOOL; 3], 0.8, 0, false, WOOL, 0.0, S_GRASS),
            def("sandstone", "Sandstone (Sand, But Committed)", Cube, true, true, [T_SANDSTONE_TOP, T_SANDSTONE, T_SANDSTONE_TOP], 1.2, 1, true, SANDSTONE, 0.0, S_STONE),
            def("stone_bricks", "Stone Bricks (Fancy Rocks)", Cube, true, true, [T_STONE_BRICKS; 3], 1.8, 1, true, STONE_BRICKS, 0.0, S_STONE),
            def("mossy_cobblestone", "Mossy Cobblestun (Vintage)", Cube, true, true, [T_MOSSY; 3], 2.0, 1, true, MOSSY_COBBLE, 0.0, S_STONE),
            def("hay_bale", "Hay Bale (Soft Landing)", Cube, true, true, [T_HAY_TOP, T_HAY_SIDE, T_HAY_TOP], 0.5, 0, false, HAY, 0.0, S_GRASS),
            def("bookshelf", "Bookshelf (Unread)", Cube, true, true, [T_PLANKS, T_BOOKSHELF, T_PLANKS], 1.5, 0, false, BOOKSHELF, 0.0, S_WOOD),
            def("lantern", "Lantern (Fancy Torch)", Cube, true, false, [T_LANTERN; 3], 0.8, 0, false, LANTERN, 14.0, S_GLASS),
            def("mushroom", "Mushroom (Probably Fine)", Cross, false, false, [T_MUSHROOM; 3], 0.0, 0, false, MUSHROOM, 0.0, S_GRASS),
            def("scarecrow", "Scarecrow (Unconvincing)", Cube, true, true, [T_PUMPKIN_TOP, T_SCARECROW, T_HAY_TOP], 0.8, 0, false, SCARECROW, 0.0, S_WOOD),
            def("weeds", "Weeds (Unwelcome)", Cross, false, false, [T_WEEDS; 3], 0.0, 0, false, AIR, 0.0, S_GRASS),
            def("farmland", "Farmland (Thirsty)", Cube, true, true, [T_FARMLAND, T_DIRT, T_DIRT], 0.5, 0, false, DIRT, 0.0, S_GRASS),
            def("farmland_wet", "Farmland (Hydrated)", Cube, true, true, [T_FARMLAND_WET, T_DIRT, T_DIRT], 0.5, 0, false, DIRT, 0.0, S_GRASS),
        ];
        // Crops: drops are decided by farming.rs when they're broken.
        let crops = [("wheat", "Wheat", T_CROP_WHEAT), ("carrots", "Carrots", T_CROP_CARROT), ("potatoes", "Potatoes", T_CROP_POTATO)];
        for (key, name, tile) in crops {
            for stage in 0..4u16 {
                let label = ["Sprouting", "Growing", "Nearly There", "Ready"][stage as usize];
                let mut b = def(leak(&format!("{key}_{stage}")), leak(&format!("{name} ({label})")), Cross, false, false, [tile + stage; 3], 0.0, 0, false, AIR, 0.0, S_GRASS);
                b.creative = stage == 3;
                blocks.push(b);
            }
        }
        blocks.push(def("chest", "Chest (Latches on Every Side)", Cube, true, true, [T_CHEST_TOP, T_CHEST_SIDE, T_CHEST_TOP], 2.5, 0, false, CHEST, 0.0, S_WOOD));
        blocks.push(def("furnace", "Furnace (Omnidirectional)", Cube, true, true, [T_FURNACE_TOP, T_FURNACE_SIDE, T_FURNACE_TOP], 3.5, 1, true, FURNACE, 0.0, S_STONE));
        let mut lit = def("furnace_lit", "Furnace (Toasty)", Cube, true, true, [T_FURNACE_TOP, T_FURNACE_LIT, T_FURNACE_TOP], 3.5, 1, true, FURNACE, 13.0, S_STONE);
        lit.creative = false;
        blocks.push(lit);
        // Slabs and stairs of each material: they look like it and dig like it.
        let props: Vec<([u16; 3], f32, u8, bool, u8)> = MATERIALS
            .iter()
            .map(|(full, _)| {
                let b = &blocks[*full as usize];
                (b.tex, b.hardness, b.pick_tier, b.pick_block, b.sound)
            })
            .collect();
        let variant = |m: usize, key: String, name: String, shape: Shape, creative: bool| {
            let (tex, hardness, tier, pick, sound) = props[m];
            let mut d = def(leak(&key), leak(&name), Shaped, true, false, tex, hardness, tier, pick, AIR, 0.0, sound);
            d.shape = shape;
            d.creative = creative;
            d
        };
        let base_key = ["planks", "cobblestone", "stone_brick"];
        for (m, (_, name)) in MATERIALS.iter().enumerate() {
            for top in [false, true] {
                let key = format!("{}_slab{}", base_key[m], if top { "_top" } else { "" });
                let label = format!("{name} Slab{}", if top { " (Upside Down)" } else { " (Half the Commitment)" });
                let mut d = variant(m, key, label, Shape::Slab { top }, !top);
                d.drop = slab(m, false);
                d.family = slab(m, false);
                d.full = MATERIALS[m].0;
                blocks.push(d);
            }
        }
        for (m, (_, name)) in MATERIALS.iter().enumerate() {
            for facing in 0..4u8 {
                let key = format!("{}_stairs{}", base_key[m], ["", "_east", "_south", "_west"][facing as usize]);
                let mut d = variant(m, key, format!("{name} Stairs (Up, Mostly)"), Shape::Stairs { facing }, facing == 0);
                d.drop = stairs(m, 0);
                d.family = stairs(m, 0);
                d.full = MATERIALS[m].0;
                blocks.push(d);
            }
        }
        for facing in 0..4u8 {
            for open in [false, true] {
                for top in [false, true] {
                    let key = format!("door_{}_{}_{}", ["north", "east", "south", "west"][facing as usize], if open { "open" } else { "closed" }, if top { "top" } else { "bottom" });
                    let tile = if top { T_DOOR_TOP } else { T_DOOR_BOTTOM };
                    let mut d = def(leak(&key), "Door (Opens Both Ways, Emotionally)", Shaped, true, false, [tile; 3], 1.5, 0, false, DOOR, 0.0, S_WOOD);
                    d.shape = Shape::Door { facing, open, top };
                    d.creative = false;
                    blocks.push(d);
                }
            }
        }
        let anvils = [("anvil", "Anvil (Drops Ominously)", T_ANVIL_TOP), ("anvil_chipped", "Chipped Anvil (Still Ominous)", T_ANVIL_TOP_CHIPPED), ("anvil_damaged", "Damaged Anvil (Living on Borrowed Time)", T_ANVIL_TOP_DAMAGED)];
        for (i, (key, name, top)) in anvils.into_iter().enumerate() {
            let id = ANVIL + i as Id;
            let mut d = def(key, name, Shaped, true, false, [top, T_ANVIL_SIDE, T_ANVIL_SIDE], 5.0, 1, true, id, 0.0, S_STONE);
            d.shape = Shape::Anvil;
            d.creative = i == 0;
            blocks.push(d);
        }
        blocks.push(def("glowshroom", "Glowshroom (Cave Nightlight)", Cross, false, false, [T_GLOWSHROOM; 3], 0.0, 0, false, GLOWSHROOM, 7.0, S_GRASS));
        blocks.push(def("pointy_rock", "Pointy Rock (Mind Your Head)", Cross, false, false, [T_POINTY_ROCK; 3], 0.6, 1, true, POINTY_ROCK, 0.0, S_STONE));
        let mut table = def("enchanting_table", "Enchanting Table (Bookshelf-Powered Guesswork)", Shaped, true, false, [T_ENCH_TOP, T_ENCH_SIDE, T_ENCH_BOTTOM], 5.0, 1, true, ENCHANTING_TABLE, 7.0, S_STONE);
        table.shape = Shape::Table;
        blocks.push(table);
        for level in 1..=WATER_REACH {
            let mut d = def(leak(&format!("water_flowing_{level}")), "Water (In a Hurry)", Liquid, false, false, [T_WATER; 3], -1.0, 0, false, AIR, 0.0, S_GRASS);
            d.creative = false;
            blocks.push(d);
        }
        blocks.push(def("lava", "Lava (Spicy Water)", Liquid, false, false, [T_LAVA; 3], -1.0, 0, false, AIR, 0.0, S_STONE));
        for level in 1..=LAVA_REACH {
            let mut d = def(leak(&format!("lava_flowing_{level}")), "Lava (On the Move)", Liquid, false, false, [T_LAVA; 3], -1.0, 0, false, AIR, 0.0, S_STONE);
            d.creative = false;
            blocks.push(d);
        }
        blocks.push(def("obsidian", "Obsidian (Very Committed)", Cube, true, true, [T_OBSIDIAN; 3], 50.0, 4, true, OBSIDIAN, 0.0, S_STONE));
        blocks.push(def("zap_ore", "Zappy Ore (Tingly)", Cube, true, true, [T_ZAP_ORE; 3], 3.0, 3, true, ZAP_DUST, 0.0, S_STONE));
        for (on, key, tile) in [(false, "zap_wire", T_WIRE), (true, "zap_wire_on", T_WIRE_ON)] {
            let mut d = def(key, "Zappy Dust (Laid Out)", Shaped, false, false, [tile, tile, tile], 0.0, 0, false, ZAP_DUST, 0.0, S_STONE);
            let _ = on;
            d.shape = Shape::Dust;
            d.creative = false;
            blocks.push(d);
        }
        for (on, key, tile) in [(false, "lever", T_LEVER), (true, "lever_on", T_LEVER_ON)] {
            let mut d = def(key, "Lever (Pull It)", Cross, false, false, [tile; 3], 0.3, 0, false, LEVER, 0.0, S_WOOD);
            d.creative = !on;
            blocks.push(d);
        }
        for down in [false, true] {
            let mut d = def(if down { "button_on" } else { "button" }, "Button (Press It)", Shaped, false, false, [T_STONE; 3], 0.3, 0, false, BUTTON, 0.0, S_STONE);
            d.shape = Shape::Button { down };
            d.creative = !down;
            blocks.push(d);
        }
        for down in [false, true] {
            let mut d = def(if down { "pressure_plate_on" } else { "pressure_plate" }, "Pressure Plate (Step On It)", Shaped, false, false, [T_STONE; 3], 0.4, 0, false, PLATE, 0.0, S_STONE);
            d.shape = Shape::Plate { down };
            d.creative = !down;
            blocks.push(d);
        }
        blocks.push(def("zap_block", "Block of Zappy Dust (Always On)", Cube, true, true, [T_ZAP_BLOCK; 3], 3.0, 1, true, ZAP_BLOCK, 0.0, S_STONE));
        blocks.push(def("zap_lamp", "Zappy Lamp (Off)", Cube, true, true, [T_LAMP; 3], 0.4, 0, false, LAMP, 0.0, S_GLASS));
        let mut lit = def("zap_lamp_on", "Zappy Lamp (On)", Cube, true, true, [T_LAMP_ON; 3], 0.4, 0, false, LAMP, 14.0, S_GLASS);
        lit.creative = false;
        blocks.push(lit);
        blocks.push(def("scorchrock", "Scorchrock (Grumbly)", Cube, true, true, [T_SCORCHROCK; 3], 0.4, 1, true, SCORCHROCK, 0.0, S_STONE));
        let mut sand = def("embersand", "Embersand (Clingy)", Cube, true, true, [T_EMBERSAND; 3], 0.5, 0, false, EMBERSAND, 0.0, S_SAND);
        sand.speed = 0.5;
        blocks.push(sand);
        blocks.push(def("scorch_gold_ore", "Scorched Gold Ore (Still Useless, Now Warm)", Cube, true, true, [T_SCORCH_GOLD; 3], 3.0, 1, true, GOLD_INGOT, 0.0, S_STONE));
        for x_axis in [true, false] {
            let mut d = def(if x_axis { "portal" } else { "portal_z" }, "Portal (Shimmery)", Shaped, false, false, [T_PORTAL; 3], -1.0, 0, false, AIR, 11.0, S_GLASS);
            d.shape = Shape::Portal { x_axis };
            d.creative = false;
            blocks.push(d);
        }
        for (k, key) in ["rail", "rail_ew", "rail_ne", "rail_nw", "rail_se", "rail_sw"].into_iter().enumerate() {
            let tile = T_RAIL + k as u16;
            let mut d = def(key, "Rail (Choo Choo)", Shaped, false, false, [tile; 3], 0.7, 0, false, RAIL_FIRST, 0.0, S_STONE);
            d.shape = Shape::Dust;
            d.creative = k == 0;
            blocks.push(d);
        }
        for (k, key) in ["powered_rail", "powered_rail_on", "powered_rail_ew", "powered_rail_ew_on"].into_iter().enumerate() {
            let tile = T_POWERED_RAIL + k as u16;
            let mut d = def(key, "Powered Rail (Zoom)", Shaped, false, false, [tile; 3], 0.7, 0, false, POWERED_RAIL, 0.0, S_STONE);
            d.shape = Shape::Dust;
            d.creative = k == 0;
            blocks.push(d);
        }
        for facing in 0..4u8 {
            let mut d = def(leak(&format!("sign{}", ["", "_east", "_south", "_west"][facing as usize])), "Sign (Words Go Here)", Shaped, false, false, [T_PLANKS; 3], 1.0, 0, false, SIGN_FIRST, 0.0, S_WOOD);
            d.shape = Shape::Sign { facing };
            d.creative = facing == 0;
            blocks.push(d);
        }
        for facing in 0..4u8 {
            let mut d = def(leak(&format!("item_frame{}", ["", "_east", "_south", "_west"][facing as usize])), "Item Frame (Look What I Have)", Shaped, false, false, [T_FRAME; 3], 0.4, 0, false, FRAME_FIRST, 0.0, S_WOOD);
            d.shape = Shape::Frame { facing };
            d.creative = facing == 0;
            blocks.push(d);
        }
        let mut sapling = def("sapling", "Sapling (Tree, Eventually)", Cross, false, false, [T_SAPLING; 3], 0.0, 0, false, SAPLING, 0.0, S_GRASS);
        sapling.creative = true;
        blocks.push(sapling);
        let fire_def;
        // Fences and panes: every combination of joined sides.
        for mask in 0..16u8 {
            let mut d = def(leak(&format!("fence{}", if mask == 0 { String::new() } else { format!("_{mask}") })), "Fence (Keeps Honest Animals In)", Shaped, true, false, [T_PLANKS; 3], 2.0, 0, false, FENCE_FIRST, 0.0, S_WOOD);
            d.shape = Shape::Fence { mask };
            d.creative = mask == 0;
            d.see_through = true;
            blocks.push(d);
        }
        for mask in 0..16u8 {
            let mut d = def(leak(&format!("glass_pane{}", if mask == 0 { String::new() } else { format!("_{mask}") })), "Glass Pane (Window, Budget)", Shaped, true, false, [T_GLASS; 3], 0.3, 0, false, AIR, 0.0, S_GLASS);
            d.shape = Shape::Pane { mask };
            d.creative = mask == 0;
            d.see_through = true;
            blocks.push(d);
        }
        for (x_axis, open) in [(false, false), (false, true), (true, false), (true, true)] {
            let key = format!("fence_gate{}{}", if x_axis { "_x" } else { "" }, if open { "_open" } else { "" });
            let mut d = def(leak(&key), "Fence Gate (Swings Both Ways)", Shaped, !open, false, [T_PLANKS; 3], 2.0, 0, false, GATE_FIRST, 0.0, S_WOOD);
            d.shape = Shape::Gate { x_axis, open };
            d.creative = !x_axis && !open;
            d.see_through = true;
            blocks.push(d);
        }
        for facing in 0..4u8 {
            let mut d = def(leak(&format!("ladder{}", ["", "_east", "_south", "_west"][facing as usize])), "Ladder (Up, Mostly)", Shaped, false, false, [T_LADDER; 3], 0.4, 0, false, LADDER_FIRST, 0.0, S_WOOD);
            d.shape = Shape::Ladder { facing };
            d.creative = facing == 0;
            d.see_through = true;
            blocks.push(d);
        }
        for facing in 0..4u8 {
            for open in [false, true] {
                let key = format!("trapdoor{}{}", ["", "_east", "_south", "_west"][facing as usize], if open { "_open" } else { "" });
                let mut d = def(leak(&key), "Trapdoor (Floor Door)", Shaped, true, false, [T_TRAPDOOR; 3], 3.0, 0, false, TRAPDOOR_FIRST, 0.0, S_WOOD);
                d.shape = Shape::Trapdoor { facing, open };
                d.creative = facing == 0 && !open;
                d.see_through = true;
                blocks.push(d);
            }
        }
        for c in 1..8u16 {
            let (key, name) = crate::carpentry::COLOURS[c as usize];
            blocks.push(def(leak(&format!("{key}_wool")), leak(&format!("{name} Wool")), Cube, true, true, [T_DYED_WOOL + c - 1; 3], 0.8, 0, false, DYED_WOOL + c - 1, 0.0, S_GRASS));
        }
        {
            let mut d = def("fire", "Fire (Hot)", Cross, false, false, [T_FIRE; 3], 0.0, 0, false, AIR, 9.0, S_GRASS);
            d.creative = false;
            fire_def = Some(d);
        }
        for c in 0..8u16 {
            let (key, name) = crate::carpentry::COLOURS[c as usize];
            let mut d = def(leak(&format!("{key}_stained_glass")), leak(&format!("{name} Stained Glass")), Cube, true, false, [T_STAINED_GLASS + c; 3], 0.3, 0, false, AIR, 0.0, S_GLASS);
            d.see_through = true;
            blocks.push(d);
        }
        blocks.extend(fire_def);
        blocks.push(def("ember_shroom", "Ember Shroom (Warm to the Touch)", Cross, false, false, [T_EMBER_SHROOM; 3], 0.0, 0, false, EMBER_SHROOM, 5.0, S_GRASS));
        let mut stand = def("brewing_stand", "Brewing Stand (Chemistry, Loosely)", Shaped, true, false, [T_BREWING_TOP, T_BREWING_SIDE, T_BREWING_TOP], 1.0, 0, false, BREWING_STAND, 2.0, S_STONE);
        stand.shape = Shape::Brewer;
        blocks.push(stand);
        // Contraptions (see contraptions.rs).
        for lit in [true, false] {
            let mut d = def(if lit { "zappy_torch" } else { "zappy_torch_off" }, "Zappy Torch (Contrarian)", Cross, false, false, [if lit { T_ZTORCH_ON } else { T_ZTORCH_OFF }; 3], 0.0, 0, false, ZTORCH_ON, if lit { 4.0 } else { 0.0 }, S_WOOD);
            d.creative = lit;
            blocks.push(d);
        }
        for facing in 0..4u8 {
            for on in [false, true] {
                let key = format!("repeater{}{}", ["", "_east", "_south", "_west"][facing as usize], if on { "_on" } else { "" });
                let mut d = def(leak(&key), "Repeater (Says It Again)", Shaped, true, false, [if on { T_REPEATER_ON } else { T_REPEATER }, T_STONE, T_STONE], 0.0, 0, false, REPEATER_FIRST, 0.0, S_STONE);
                d.shape = Shape::Repeater { facing };
                d.creative = facing == 0 && !on;
                d.see_through = true;
                blocks.push(d);
            }
        }
        for sticky in [false, true] {
            for facing in 0..6u8 {
                for extended in [false, true] {
                    let key = format!("{}piston_{}{}", if sticky { "sticky_" } else { "" }, ["north", "east", "south", "west", "up", "down"][facing as usize], if extended { "_out" } else { "" });
                    let name = if sticky { "Sticky Piston (Clingy)" } else { "Piston (Pushy)" };
                    let family = if sticky { STICKY_FIRST } else { PISTON_FIRST };
                    let mut d = def(leak(&key), name, Shaped, true, !extended, [T_PISTON_SIDE; 3], 1.5, 0, false, family, 0.0, S_STONE);
                    d.shape = Shape::Piston { facing, extended };
                    d.creative = facing == 0 && !extended;
                    d.see_through = extended;
                    blocks.push(d);
                }
            }
        }
        for facing in 0..6u8 {
            for sticky in [false, true] {
                let key = format!("piston_head_{}{}", ["north", "east", "south", "west", "up", "down"][facing as usize], if sticky { "_sticky" } else { "" });
                let mut d = def(leak(&key), "Piston Head", Shaped, true, false, [T_PISTON_SIDE; 3], 1.5, 0, false, AIR, 0.0, S_STONE);
                d.shape = Shape::PistonHead { facing };
                d.creative = false;
                d.see_through = true;
                blocks.push(d);
            }
        }
        let mut hoppers = Vec::new();
        for spout in 0..5u8 {
            let key = format!("hopper{}", ["", "_north", "_east", "_south", "_west"][spout as usize]);
            let mut d = def(leak(&key), "Hopper (Funnel With Ambition)", Shaped, true, false, [T_HOPPER_TOP, T_HOPPER_SIDE, T_HOPPER_SIDE], 3.0, 1, true, HOPPER_FIRST, 0.0, S_STONE);
            d.shape = Shape::Hopper { spout };
            d.creative = spout == 0;
            d.see_through = true;
            hoppers.push(d);
        }
        for facing in 0..6u8 {
            let key = format!("dispenser{}", ["", "_east", "_south", "_west", "_up", "_down"][facing as usize]);
            let mut d = def(leak(&key), "Dispenser (Spits Things)", Cube, true, true, [T_COBBLE; 3], 3.5, 1, true, DISPENSER_FIRST, 0.0, S_STONE);
            d.creative = facing == 0;
            blocks.push(d);
        }
        blocks.extend(hoppers);
        debug_assert_eq!(blocks.len(), NUM_BLOCKS as usize);
        debug_assert_eq!(blocks[POWERED_RAIL as usize].key, "powered_rail");
        debug_assert_eq!(blocks[SIGN_FIRST as usize].key, "sign");
        debug_assert_eq!(blocks[FRAME_FIRST as usize].key, "item_frame");
        blocks[GLASS as usize].see_through = true;
        blocks[ICE as usize].speed = 1.6;
        blocks[BOUNCY as usize].bounce = 0.85;
        blocks[LANTERN as usize].see_through = true;
        for id in [AIR, WATER, LAVA, BEDROCK] {
            blocks[id as usize].creative = false;
        }

        let gap = || ItemDef { real: false, ..item("", "???", T_WHITE) };
        let mut items = vec![
            item("stick", "Stick (Artisanal)", T_STICK),
            item("coal", "Coal", T_COAL),
            item("iron", "Iron Chunk", T_IRON),
            item("diamond", "Dimond", T_DIAMOND),
            item("gunpowder", "Hisspowder", T_GUNPOWDER),
            ItemDef { food: Some(3.0), ..item("porkchop", "Raw Oinkchop", T_PORK) },
            ItemDef { food: Some(4.0), ..item("goo", "Groaner Goo", T_GOO) },
            gap(),
            gap(),
            gap(),
        ];
        let tiers = [("wooden", "Wooden"), ("stone", "Stone"), ("iron", "Iron"), ("diamond", "Dimond")];
        for (i, (k, n)) in tiers.iter().enumerate() {
            items.push(ItemDef {
                stack: 1,
                pick_tier: i as u8 + 1,
                damage: 2.0 + i as f32,
                ..item(leak(&format!("{k}_pickaxe")), leak(&format!("{n} Pickaxe")), T_PICK0 + i as u16)
            });
        }
        for (i, (k, n)) in tiers.iter().enumerate() {
            items.push(ItemDef { stack: 1, damage: 4.0 + i as f32, ..item(leak(&format!("{k}_sword")), leak(&format!("{n} Sword")), T_SWORD0 + i as u16) });
        }
        items.extend([
            item("gold", "Gold Ingot (Too Soft for Anything)", T_GOLD),
            ItemDef { food: Some(8.0), stack: 16, ..item("golden_oinkchop", "Suspiciously Golden Oinkchop", T_GOLD_CHOP) },
            ItemDef { stack: 16, ..item("stare_pearl", "Stare Pearl (Throw to Teleport)", T_PEARL) },
            ItemDef { food: Some(2.0), ..item("mutton", "Raw Baa-con", T_MUTTON) },
            item("feather", "Feather (Ticklish)", T_FEATHER),
            ItemDef { food: Some(2.0), ..item("cluckets", "Raw Cluckets", T_CLUCKETS) },
            ItemDef { food: Some(3.0), ..item("moo_steak", "Raw Moo-steak", T_MOO_STEAK) },
            item("bone", "Bone (Previously Owned)", T_BONE_ITEM),
            item("pointy_stick", "Pointy Stick", T_ARROW),
            item("string", "String (Not Spaghetti)", T_STRING),
            ItemDef { stack: 1, ..item("bow", "Bow (Twangy)", T_BOW) },
            ItemDef { stack: 1, ..item("hoe", "Hoe (Dirt Scratcher)", T_HOE) },
            item("wheat_seeds", "Wheat Seeds (Tiny)", T_SEEDS),
            item("wheat", "Wheat (Bread Pending)", T_WHEAT_ITEM),
            ItemDef { food: Some(3.0), ..item("carrot", "Carrot (Crunchy)", T_CARROT_ITEM) },
            ItemDef { food: Some(1.0), ..item("potato", "Raw Spud (Regrettable)", T_POTATO_ITEM) },
            item("bone_dust", "Bone Dust (Phosphorus!)", T_BONE_DUST),
            item("compost", "Compost (Nitrogen!)", T_COMPOST),
            item("wood_ash", "Wood Ash (Potassium!)", T_WOOD_ASH),
            ItemDef { stack: 1, ..item("soil_probe", "Soil Probe (Science!)", T_SOIL_PROBE) },
            ItemDef { food: Some(5.0), ..item("bread", "Bread (Finally)", T_BREAD) },
            ItemDef { stack: 1, ..item("fishing_rod", "Fishing Stick (Advanced)", T_ROD) },
            ItemDef { food: Some(2.0), ..item("cod", "Raw Cod-ish", T_COD) },
            ItemDef { food: Some(2.0), ..item("salmon", "Raw Salmon-ish", T_SALMON) },
            ItemDef { food: Some(-4.0), ..item("pufferfish", "Pufferfish (Do Not Eat)", T_PUFFER) },
            ItemDef { food: Some(1.0), ..item("tropical_fish", "Tropical Fish (Suspiciously Colorful)", T_TROPICAL) },
            ItemDef { food: Some(20.0), stack: 1, ..item("big_bob", "Big Bob (Legendary Carp)", T_BIG_BOB) },
            ItemDef { stack: 1, ..item("soggy_boot", "Soggy Boot (Left Only)", T_BOOT) },
            item("message_bottle", "Message in a Bottle", T_BOTTLE),
            ItemDef { food: Some(10.0), ..item("fish_and_chips", "Fish n' Chips (Legally Distinct)", T_FISH_CHIPS) },
            ItemDef { food: Some(0.0), stack: 1, ..item("suspicious_stew", "Suspicious Stew", T_STEW) },
            item("worm", "Wiggly Worm (Bait)", T_WORM),
            ItemDef { food: Some(8.0), ..item("cooked_oinkchop", "Cooked Oinkchop", T_COOKED_CHOP) },
            ItemDef { food: Some(6.0), ..item("cooked_mutton", "Cooked Baa-con (Crispy)", T_COOKED_MUTTON) },
            ItemDef { food: Some(6.0), ..item("cooked_cluckets", "Cooked Cluckets", T_COOKED_CLUCKETS) },
            ItemDef { food: Some(8.0), ..item("steak", "Moo-steak (Well Done, Sorry)", T_STEAK) },
            ItemDef { food: Some(5.0), ..item("cooked_cod", "Cooked Cod-ish", T_COOKED_COD) },
            ItemDef { food: Some(6.0), ..item("cooked_salmon", "Cooked Salmon-ish", T_COOKED_SALMON) },
            ItemDef { food: Some(5.0), ..item("baked_potato", "Baked Spud (Redeemed)", T_BAKED_POTATO) },
            ItemDef { food: Some(-2.0), ..item("cooked_pufferfish", "Cooked Pufferfish (Still Do Not Eat)", T_COOKED_PUFFER) },
            ItemDef { food: Some(1.0), stack: 1, ..item("cooked_boot", "Cooked Boot (Chewy)", T_COOKED_BOOT) },
            ItemDef { stack: 16, ..item("door", "Door (Opens Both Ways, Emotionally)", T_DOOR_ITEM) },
        ]);
        let armor_names = [
            ["Woolly Hat (Hand-Knitted)", "Woolly Jumper (Itchy)", "Woolly Trousers (Very Itchy)", "Woolly Socks (For Sandals)"],
            ["Iron Helmet (Bucket With Ambition)", "Iron Chestplate (Clanky)", "Iron Leggings (Squeaky)", "Iron Boots (Loud Walking)"],
            ["Golden Helmet (Crown-Adjacent)", "Golden Chestplate (Soft, Shiny)", "Golden Leggings (Why)", "Golden Boots (Bling Toes)"],
            ["Dimond Helmet (Flex)", "Dimond Chestplate (Maximum Flex)", "Dimond Leggings (Rich Knees)", "Dimond Boots (Sparkly Stomping)"],
        ];
        for (t, tier) in ["wool", "iron", "golden", "diamond"].iter().enumerate() {
            for (slot, part) in ["helmet", "chestplate", "leggings", "boots"].iter().enumerate() {
                items.push(ItemDef { stack: 1, ..item(leak(&format!("{tier}_{part}")), armor_names[t][slot], T_ARMOR_ITEMS + (t * 4 + slot) as u16) });
            }
        }
        items.extend([
            ItemDef { stack: 16, ..item("bucket", "Bucket (Empty, Optimistic)", T_BUCKET) },
            ItemDef { stack: 1, ..item("water_bucket", "Bucket of Water (Sloshy)", T_WATER_BUCKET) },
            ItemDef { stack: 1, ..item("lava_bucket", "Bucket of Lava (Hold Level)", T_LAVA_BUCKET) },
            ItemDef { stack: 1, ..item("shears", "Shears (For Fluffers, Not Haircuts)", T_SHEARS) },
            item("zap_dust", "Zappy Dust (Do Not Lick)", T_ZAP_DUST),
            item("book", "Book (Mostly Wheat)", T_BOOK),
            ItemDef { stack: 1, ..item("enchanted_book", "Enchanted Book (Spoilers Inside)", T_ENCHANTED_BOOK) },
            ItemDef { stack: 1, ..item("shield", "Shield (Door You Can Carry)", T_SHIELD) },
            ItemDef { stack: 1, ..item("sparker", "Sparker (Hot Hands in a Can)", T_SPARKER) },
            ItemDef { stack: 1, ..item("boat", "Boat (Mostly Waterproof)", T_BOAT_ITEM) },
            ItemDef { stack: 1, ..item("minecart", "Minecart (Wheeled Bucket)", T_CART_ITEM) },
            item("compass", "Compass (Points Home, Mostly)", T_COMPASS),
            item("map", "Map (You Are Here)", T_MAP),
            ItemDef { food: Some(4.0), ..item("apple", "Apple (Keeps the Doctor Confused)", T_APPLE) },
        ]);
        for c in 0..8u16 {
            let (key, name) = crate::carpentry::COLOURS[c as usize];
            items.push(item(leak(&format!("{key}_dye")), leak(&format!("{name} Dye")), T_DYE_FIRST + c));
        }
        items.push(ItemDef { stack: 16, ..item("glass_bottle", "Glass Bottle (Empty, Hopeful)", T_GLASS_BOTTLE) });
        items.push(ItemDef { stack: 16, ..item("water_bottle", "Water Bottle (Just Water)", T_WATER_BOTTLE) });
        for splash in [false, true] {
            for (i, p) in crate::potions::ALL.iter().enumerate() {
                let key = format!("{}potion_of_{}", if splash { "splash_" } else { "" }, p.key());
                let name = format!("{}Potion of {}", if splash { "Splash " } else { "" }, p.name());
                let tile = if splash { T_SPLASH_FIRST } else { T_POTION_FIRST } + i as u16;
                items.push(ItemDef { stack: 1, ..item(leak(&key), leak(&name), tile) });
            }
        }
        items.push(item("grumbler_tusk", "Grumbler Tusk (Rude to Ask)", T_TUSK));
        items.push(ItemDef { stack: 1, ..item("saddle", "Saddle (Some Assembly Required)", T_SADDLE) });
        debug_assert_eq!(items.len(), (FIRST_MOD_ITEM - FIRST_ITEM) as usize);

        let r = |inputs: &[(Id, u8)], output: (Id, u8)| Recipe { inputs: inputs.to_vec(), output };
        let recipes = vec![
            r(&[(LOG, 1)], (PLANKS, 4)),
            r(&[(PLANKS, 2)], (STICK, 4)),
            r(&[(PLANKS, 4)], (TABLE, 1)),
            r(&[(STICK, 1), (COAL, 1)], (TORCH, 4)),
            r(&[(PLANKS, 3), (STICK, 2)], (PICK_WOOD, 1)),
            r(&[(COBBLE, 3), (STICK, 2)], (PICK_STONE, 1)),
            r(&[(IRON, 3), (STICK, 2)], (PICK_IRON, 1)),
            r(&[(DIAMOND, 3), (STICK, 2)], (PICK_DIAMOND, 1)),
            r(&[(PLANKS, 2), (STICK, 1)], (SWORD_WOOD, 1)),
            r(&[(COBBLE, 2), (STICK, 1)], (SWORD_STONE, 1)),
            r(&[(IRON, 2), (STICK, 1)], (SWORD_IRON, 1)),
            r(&[(DIAMOND, 2), (STICK, 1)], (SWORD_DIAMOND, 1)),
            r(&[(COBBLE, 1), (COAL, 1)], (STONE, 1)),
            r(&[(SAND, 4), (COAL, 1)], (GLASS, 4)),
            r(&[(DIRT, 4), (COAL, 1)], (BRICK, 4)),
            r(&[(GUNPOWDER, 5), (SAND, 4)], (TNT, 1)),
            r(&[(TORCH, 4), (GLASS, 1)], (GLOWROCK, 1)),
            r(&[(PUMPKIN, 1), (TORCH, 1)], (JACK, 1)),
            r(&[(GOO, 4)], (BOUNCY, 1)),
            r(&[(WOOL, 3), (PLANKS, 3)], (BED, 1)),
            r(&[(PORKCHOP, 1), (MUTTON, 1), (SAND, 2)], (CAKE, 1)),
            r(&[(WOOL, 2), (GOO, 2)], (SPONGE, 1)),
            r(&[(PORKCHOP, 1), (GOLD_INGOT, 4)], (GOLDEN_CHOP, 1)),
            r(&[(GOLD_INGOT, 3), (STICK, 2)], (PICK_WOOD, 1)),
            r(&[(STICK, 3), (STRING, 3)], (BOW, 1)),
            r(&[(STICK, 1), (FEATHER, 1), (COBBLE, 1)], (ARROW, 4)),
            r(&[(BONE, 1), (FEATHER, 1)], (ARROW, 2)),
            r(&[(STRING, 4)], (WOOL, 1)),
            r(&[(SAND, 4)], (SANDSTONE, 1)),
            r(&[(STONE, 4)], (STONE_BRICKS, 4)),
            r(&[(COBBLE, 1), (WEEDS, 1)], (MOSSY_COBBLE, 1)),
            r(&[(WHEAT, 9)], (HAY, 1)),
            r(&[(HAY, 1)], (WHEAT, 9)),
            r(&[(PLANKS, 6), (FEATHER, 3)], (BOOKSHELF, 1)),
            r(&[(IRON, 1), (TORCH, 1)], (LANTERN, 1)),
            r(&[(PUMPKIN, 1), (HAY, 1), (STICK, 2)], (SCARECROW, 1)),
            r(&[(PLANKS, 2), (STICK, 2)], (HOE, 1)),
            r(&[(BONE, 1)], (BONE_DUST, 3)),
            r(&[(GOO, 2), (DIRT, 1)], (COMPOST, 2)),
            r(&[(WHEAT_SEEDS, 3), (DIRT, 1)], (COMPOST, 1)),
            r(&[(LOG, 1), (COAL, 1)], (WOOD_ASH, 3)),
            r(&[(IRON, 1), (STICK, 1), (GLASS, 1)], (SOIL_PROBE, 1)),
            r(&[(WHEAT, 3)], (BREAD, 1)),
            r(&[(STICK, 3), (STRING, 2)], (ROD, 1)),
            r(&[(COD, 1), (POTATO, 1)], (FISH_CHIPS, 1)),
            r(&[(SALMON, 1), (POTATO, 1)], (FISH_CHIPS, 1)),
            r(&[(MUSHROOM, 2), (FLOWER, 1)], (STEW, 1)),
            r(&[(PLANKS, 8)], (CHEST, 1)),
            r(&[(COBBLE, 8)], (FURNACE, 1)),
            r(&[(PLANKS, 6)], (DOOR, 3)),
            // A real anvil takes 31 iron. This one is a bargain.
            r(&[(IRON, 10)], (ANVIL, 1)),
            r(&[(BOOKSHELF, 1), (DIAMOND, 2), (COBBLE, 4)], (ENCHANTING_TABLE, 1)),
            r(&[(IRON, 3)], (BUCKET, 1)),
            r(&[(IRON, 2)], (SHEARS, 1)),
            r(&[(ZAP_DUST, 9)], (ZAP_BLOCK, 1)),
            r(&[(ZAP_BLOCK, 1)], (ZAP_DUST, 9)),
            r(&[(STICK, 1), (COBBLE, 1)], (LEVER, 1)),
            r(&[(STONE, 1)], (BUTTON, 2)),
            r(&[(STONE, 2)], (PLATE, 1)),
            r(&[(GLOWROCK, 1), (ZAP_DUST, 4)], (LAMP, 1)),
            r(&[(WHEAT, 3), (STRING, 1)], (BOOK, 1)),
            r(&[(PLANKS, 6), (IRON, 1)], (SHIELD, 1)),
            r(&[(IRON, 1), (COAL, 1)], (SPARKER, 1)),
            r(&[(PLANKS, 5)], (BOAT, 1)),
            r(&[(IRON, 4), (ZAP_DUST, 1)], (COMPASS, 1)),
            r(&[(WHEAT, 8), (COMPASS, 1)], (MAP, 1)),
            r(&[(PLANKS, 6), (STICK, 1)], (SIGN_FIRST, 3)),
            r(&[(PLANKS, 4), (STICK, 2)], (FENCE_FIRST, 3)),
            r(&[(STICK, 4), (PLANKS, 2)], (GATE_FIRST, 1)),
            r(&[(STICK, 7)], (LADDER_FIRST, 3)),
            r(&[(PLANKS, 6)], (TRAPDOOR_FIRST, 2)),
            r(&[(GLASS, 6)], (PANE_FIRST, 16)),
            r(&[(GLASS, 3)], (GLASS_BOTTLE, 3)),
            r(&[(COBBLE, 3), (GRUMBLER_TUSK, 1)], (BREWING_STAND, 1)),
            r(&[(STICK, 1), (ZAP_DUST, 1)], (ZTORCH_ON, 1)),
            r(&[(STONE, 3), (ZTORCH_ON, 2), (ZAP_DUST, 1)], (REPEATER_FIRST, 1)),
            r(&[(PLANKS, 3), (COBBLE, 4), (IRON, 1), (ZAP_DUST, 1)], (PISTON_FIRST, 1)),
            r(&[(PISTON_FIRST, 1), (GOO, 1)], (STICKY_FIRST, 1)),
            r(&[(COBBLE, 7), (BOW, 1), (ZAP_DUST, 1)], (DISPENSER_FIRST, 1)),
            r(&[(IRON, 5), (CHEST, 1)], (HOPPER_FIRST, 1)),
            r(&[(WOOL, 3), (IRON, 1), (STRING, 2)], (SADDLE, 1)),
            r(&[(BONE_DUST, 1)], (DYE_FIRST, 2)),
            r(&[(COAL, 1)], (DYE_FIRST + 1, 2)),
            r(&[(FLOWER, 1)], (DYE_FIRST + 2, 2)),
            r(&[(PUMPKIN, 1)], (DYE_FIRST + 3, 3)),
            r(&[(GOLD_INGOT, 1)], (DYE_FIRST + 4, 4)),
            r(&[(CACTUS, 1)], (DYE_FIRST + 5, 2)),
            r(&[(TROPICAL, 1)], (DYE_FIRST + 6, 2)),
            r(&[(DYE_FIRST + 2, 1), (DYE_FIRST + 6, 1)], (DYE_FIRST + 7, 2)),
            r(&[(STICK, 8), (WOOL, 1)], (FRAME_FIRST, 1)),
            r(&[(IRON, 5)], (MINECART, 1)),
            r(&[(IRON, 6), (STICK, 1)], (RAIL_FIRST, 16)),
            r(&[(GOLD_INGOT, 6), (STICK, 1), (ZAP_DUST, 1)], (POWERED_RAIL, 6)),
        ];
        let mut recipes = recipes;
        // Dye wool one block at a time, glass eight at a time.
        for c in 0..8u16 {
            let wool = if c == 0 { WOOL } else { DYED_WOOL + c - 1 };
            if c > 0 {
                recipes.push(r(&[(WOOL, 1), (DYE_FIRST + c, 1)], (wool, 1)));
            }
            recipes.push(r(&[(GLASS, 8), (DYE_FIRST + c, 1)], (STAINED_GLASS + c, 8)));
        }
        for (m, (full, _)) in MATERIALS.iter().enumerate() {
            recipes.push(r(&[(*full, 3)], (slab(m, false), 6)));
            recipes.push(r(&[(*full, 6)], (stairs(m, 0), 4)));
        }
        // Minecraft's amounts: 5 for a helmet, 8 chestplate, 7 leggings, 4 boots.
        for (t, material) in [WOOL, IRON, GOLD_INGOT, DIAMOND].into_iter().enumerate() {
            for (slot, n) in [5, 8, 7, 4].into_iter().enumerate() {
                recipes.push(r(&[(material, n)], (ARMOR_FIRST + (t * 4 + slot) as Id, 1)));
            }
        }
        Registry { blocks, items, recipes, ores: Vec::new(), plants: Vec::new(), splashes: Vec::new(), mods: Vec::new(), textures: Vec::new(), smelting: Vec::new(), fuels: Vec::new() }
    }

    /// Look up a block or item id by key ("stone", "cheese:wheel", ...).
    pub fn lookup(&self, key: &str) -> Option<Id> {
        if let Some(i) = self.blocks.iter().position(|b| b.key == key) {
            return Some(i as Id);
        }
        self.items.iter().position(|it| it.real && it.key == key).map(|i| i as Id + FIRST_ITEM)
    }

    pub fn key_of(&self, id: Id) -> &'static str {
        if id < FIRST_ITEM {
            self.blocks.get(id as usize).map(|b| b.key).unwrap_or("air")
        } else {
            self.items.get((id - FIRST_ITEM) as usize).map(|i| i.key).unwrap_or("")
        }
    }
}

#[inline]
pub fn block(id: Id) -> &'static BlockDef {
    let r = reg();
    r.blocks.get(id as usize).unwrap_or(&r.blocks[0])
}

/// A placeable block id that exists in the current registry.
pub fn valid_block(id: Id) -> bool {
    (id as usize) < reg().blocks.len()
}

/// Any block or item id that exists in the current registry.
pub fn valid_item(id: Id) -> bool {
    if id < FIRST_ITEM {
        id > AIR && valid_block(id)
    } else {
        reg().items.get((id - FIRST_ITEM) as usize).map(|i| i.real).unwrap_or(false)
    }
}

fn item_def(id: Id) -> Option<&'static ItemDef> {
    if id < FIRST_ITEM {
        return None;
    }
    reg().items.get((id - FIRST_ITEM) as usize).filter(|i| i.real)
}

#[inline]
pub fn is_opaque(id: Id) -> bool {
    block(id).opaque
}
#[inline]
pub fn is_solid(id: Id) -> bool {
    block(id).solid
}
/// Stops sunlight: used by the column heightmap for sky lighting.
#[inline]
pub fn blocks_sky(id: Id) -> bool {
    let b = block(id);
    !matches!(b.model, Empty | Cross) && !b.see_through && !dapples_sky(id) && !is_door(id)
}
/// Foliage: lets dappled sunlight through instead of blocking it (see `world::exposure`).
#[inline]
pub fn dapples_sky(id: Id) -> bool {
    id == LEAVES
}
/// Can the player point at it (and break it)?
#[inline]
pub fn targetable(id: Id) -> bool {
    id != AIR && !is_liquid(id)
}
/// Placing into this cell simply replaces it.
#[inline]
pub fn replaceable(id: Id) -> bool {
    matches!(id, AIR | TALL_GRASS | WEEDS) || is_liquid(id)
}

pub fn is_block_item(id: Id) -> bool {
    id > AIR && id < FIRST_ITEM && valid_block(id)
}

pub fn item_name(id: Id) -> &'static str {
    if id < FIRST_ITEM {
        return block(id).name;
    }
    item_def(id).map(|i| i.name).unwrap_or("???")
}

/// Texture tile used for the flat inventory icon (blocks get an isometric cube instead).
pub fn item_tile(id: Id) -> u16 {
    if id < FIRST_ITEM {
        return block(id).tex[1];
    }
    item_def(id).map(|i| i.tile).unwrap_or(T_WHITE)
}

pub fn max_stack(id: Id) -> u8 {
    item_def(id).map(|i| i.stack.clamp(1, 64)).unwrap_or(64)
}

/// Pickaxe tier 1..=4, or 0 if not a pickaxe.
pub fn pick_tier(id: Id) -> u8 {
    item_def(id).map(|i| i.pick_tier.min(4)).unwrap_or(0)
}

pub fn attack_damage(id: Id) -> f32 {
    item_def(id).map(|i| i.damage).unwrap_or(1.0)
}

/// Hunger points restored when eaten, if edible.
pub fn food_value(id: Id) -> Option<f32> {
    item_def(id).and_then(|i| i.food)
}

/// How long a food keeps you full (its saturation per point, Minecraft style).
pub fn food_quality(id: Id) -> f32 {
    match id {
        PORKCHOP | MUTTON | CLUCKETS | MOO_STEAK | COD | SALMON | TROPICAL | POTATO => 0.3,
        GOO | COOKED_BOOT => 0.1,
        CARROT | BREAD | APPLE => 0.6,
        GOLDEN_CHOP | BIG_BOB => 1.2,
        _ => 0.8,
    }
}

/// Mod-defined behaviour when the item is used: (actions, consumes one).
pub fn use_actions(id: Id) -> Option<(&'static [Action], bool)> {
    item_def(id).filter(|i| !i.on_use.is_empty()).map(|i| (i.on_use.as_slice(), i.consume))
}

/// Seconds to break `id` while holding `held`, and whether it drops anything.
#[cfg(test)]
pub fn break_time(id: Id, held: Id) -> (f32, bool) {
    break_time_with(id, held, 0)
}

/// `break_time` for a pickaxe with Efficiency `efficiency` (Minecraft's bonus: level² + 1).
pub fn break_time_with(id: Id, held: Id, efficiency: u8) -> (f32, bool) {
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
    let bonus = if efficiency > 0 { (efficiency as f32).powi(2) + 1.0 } else { 0.0 };
    let speed = [1.0, 2.0, 4.0, 6.0, 8.0][tier as usize] + bonus;
    (b.hardness * 1.5 / speed, tier >= b.pick_tier)
}

/// Damage with Sharpness `sharpness`.
pub fn attack_damage_with(id: Id, sharpness: u8) -> f32 {
    attack_damage(id) + sharpness as f32 * 1.25
}

/// How many of an ore's drop come out with Fortune `fortune` (ores that drop
/// something other than themselves; everything else is always one).
pub fn fortune_count(id: Id, fortune: u8, roll: f32) -> u8 {
    // Zappy ore always gives a handful (and Fortune adds to it).
    if id == ZAP_ORE {
        return 4 + (roll * 2.0) as u8 + fortune;
    }
    let b = block(id);
    if fortune == 0 || !matches!(id, COAL_ORE | IRON_ORE | GOLD_ORE | DIAMOND_ORE) || b.drop == id {
        return 1;
    }
    1 + (roll * (fortune as f32 + 1.0)).floor().min(fortune as f32) as u8
}

pub fn recipes() -> &'static [Recipe] {
    &reg().recipes
}

/// Everything the creative palette offers.
pub fn creative_items() -> Vec<Id> {
    let r = reg();
    let mut v: Vec<Id> = (1..r.blocks.len() as Id).filter(|&b| r.blocks[b as usize].creative).collect();
    v.extend((0..r.items.len()).filter(|&i| r.items[i].real).map(|i| i as Id + FIRST_ITEM));
    v
}

/// Constructors the mod loader uses while building a new registry.
pub(crate) mod build {
    pub(crate) use super::{def, item, leak};
}

#[cfg(test)]
mod id_order_tests {
    use super::*;

    #[test]
    fn named_ids_match_their_registrations() {
        for (id, key) in [
            (OBSIDIAN, "obsidian"),
            (ZAP_ORE, "zap_ore"),
            (LAMP_ON, "zap_lamp_on"),
            (PORTAL_X, "portal"),
            (RAIL_FIRST, "rail"),
            (POWERED_RAIL, "powered_rail"),
            (SIGN_FIRST, "sign"),
            (FRAME_FIRST, "item_frame"),
        ] {
            assert_eq!(block(id).key, key, "id {id}");
        }
    }
}
