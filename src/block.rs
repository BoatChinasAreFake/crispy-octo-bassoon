//! Blocks, items, tools and recipes, held in a registry that mods extend.
//!
//! Every item id is a `u8`: ids below `FIRST_ITEM` are placeable blocks
//! (base game `0..NUM_BLOCKS`, mods `NUM_BLOCKS..FIRST_ITEM`), the rest are
//! plain items (base game `100..FIRST_MOD_ITEM`, mods after that).

use crate::texture::*;
use std::sync::atomic::{AtomicPtr, AtomicU32, Ordering};

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
/// Number of base-game blocks; mod blocks start here.
pub const NUM_BLOCKS: u8 = 24;
pub const FIRST_ITEM: u8 = 100;

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
/// Mod items start here.
pub const FIRST_MOD_ITEM: u8 = 118;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Model {
    Empty,
    Cube,
    Cross,
    Liquid,
}

/// Something a mod can make happen when an item is used or a block is broken.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Heal(f32),
    Explode(f32),
    Launch(f32),
    Message(&'static str),
    Give(u8, u8),
    SetTime(f32),
    /// Mob kind index: 0 oinker, 1 hisser, 2 groaner.
    Spawn(u8),
}

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
    pub drop: u8,
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
}

#[derive(Clone)]
pub struct Recipe {
    pub inputs: Vec<(u8, u8)>,
    pub output: (u8, u8),
}

/// Mod world generation: ore veins underground.
#[derive(Clone, Debug)]
pub struct OreGen {
    pub block: u8,
    pub replace: u8,
    pub min_y: i32,
    pub max_y: i32,
    pub chance: f32,
}

/// Mod world generation: plants on the surface.
#[derive(Clone, Debug)]
pub struct PlantGen {
    pub block: u8,
    pub on: u8,
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
pub(crate) fn def(key: &'static str, name: &'static str, model: Model, solid: bool, opaque: bool, tex: [u16; 3], hardness: f32, pick_tier: u8, pick_block: bool, drop: u8, light: f32, sound: u8) -> BlockDef {
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
    }
}

pub(crate) fn item(key: &'static str, name: &'static str, tile: u16) -> ItemDef {
    ItemDef { key, name, tile, stack: 64, pick_tier: 0, damage: 1.0, food: None, on_use: Vec::new(), consume: true, real: true }
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
        ];
        blocks[GLASS as usize].see_through = true;
        for id in [AIR, WATER, BEDROCK] {
            blocks[id as usize].creative = false;
        }

        let gap = || ItemDef { real: false, ..item("", "???", T_WHITE) };
        let mut items = vec![
            item("stick", "Stick (Artisanal)", T_STICK),
            item("coal", "Coal", T_COAL),
            item("iron", "Iron Chunk", T_IRON),
            item("diamond", "Dimond", T_DIAMOND),
            item("gunpowder", "Hisspowder", T_GUNPOWDER),
            ItemDef { food: Some(6.0), ..item("porkchop", "Raw Oinkchop", T_PORK) },
            ItemDef { food: Some(1.0), ..item("goo", "Groaner Goo", T_GOO) },
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
        debug_assert_eq!(items.len(), (FIRST_MOD_ITEM - FIRST_ITEM) as usize);

        let r = |inputs: &[(u8, u8)], output: (u8, u8)| Recipe { inputs: inputs.to_vec(), output };
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
        ];
        Registry { blocks, items, recipes, ores: Vec::new(), plants: Vec::new(), splashes: Vec::new(), mods: Vec::new(), textures: Vec::new() }
    }

    /// Look up a block or item id by key ("stone", "cheese:wheel", ...).
    pub fn lookup(&self, key: &str) -> Option<u8> {
        if let Some(i) = self.blocks.iter().position(|b| b.key == key) {
            return Some(i as u8);
        }
        self.items.iter().position(|it| it.real && it.key == key).map(|i| i as u8 + FIRST_ITEM)
    }

    pub fn key_of(&self, id: u8) -> &'static str {
        if id < FIRST_ITEM {
            self.blocks.get(id as usize).map(|b| b.key).unwrap_or("air")
        } else {
            self.items.get((id - FIRST_ITEM) as usize).map(|i| i.key).unwrap_or("")
        }
    }
}

#[inline]
pub fn block(id: u8) -> &'static BlockDef {
    let r = reg();
    r.blocks.get(id as usize).unwrap_or(&r.blocks[0])
}

/// A placeable block id that exists in the current registry.
pub fn valid_block(id: u8) -> bool {
    (id as usize) < reg().blocks.len()
}

/// Any block or item id that exists in the current registry.
pub fn valid_item(id: u8) -> bool {
    if id < FIRST_ITEM {
        id > AIR && valid_block(id)
    } else {
        reg().items.get((id - FIRST_ITEM) as usize).map(|i| i.real).unwrap_or(false)
    }
}

fn item_def(id: u8) -> Option<&'static ItemDef> {
    if id < FIRST_ITEM {
        return None;
    }
    reg().items.get((id - FIRST_ITEM) as usize).filter(|i| i.real)
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
    let b = block(id);
    !matches!(b.model, Empty | Cross) && !b.see_through
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
    id > AIR && id < FIRST_ITEM && valid_block(id)
}

pub fn item_name(id: u8) -> &'static str {
    if id < FIRST_ITEM {
        return block(id).name;
    }
    item_def(id).map(|i| i.name).unwrap_or("???")
}

/// Texture tile used for the flat inventory icon (blocks get an isometric cube instead).
pub fn item_tile(id: u8) -> u16 {
    if id < FIRST_ITEM {
        return block(id).tex[1];
    }
    item_def(id).map(|i| i.tile).unwrap_or(T_WHITE)
}

pub fn max_stack(id: u8) -> u8 {
    item_def(id).map(|i| i.stack.clamp(1, 64)).unwrap_or(64)
}

/// Pickaxe tier 1..=4, or 0 if not a pickaxe.
pub fn pick_tier(id: u8) -> u8 {
    item_def(id).map(|i| i.pick_tier.min(4)).unwrap_or(0)
}

pub fn attack_damage(id: u8) -> f32 {
    item_def(id).map(|i| i.damage).unwrap_or(1.0)
}

/// Health restored when eaten, if edible.
pub fn food_value(id: u8) -> Option<f32> {
    item_def(id).and_then(|i| i.food)
}

/// Mod-defined behaviour when the item is used: (actions, consumes one).
pub fn use_actions(id: u8) -> Option<(&'static [Action], bool)> {
    item_def(id).filter(|i| !i.on_use.is_empty()).map(|i| (i.on_use.as_slice(), i.consume))
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

pub fn recipes() -> &'static [Recipe] {
    &reg().recipes
}

/// Everything the creative palette offers.
pub fn creative_items() -> Vec<u8> {
    let r = reg();
    let mut v: Vec<u8> = (1..r.blocks.len() as u8).filter(|&b| r.blocks[b as usize].creative).collect();
    v.extend((0..r.items.len()).filter(|&i| r.items[i].real).map(|i| i as u8 + FIRST_ITEM));
    v
}

/// Constructors the mod loader uses while building a new registry.
pub(crate) mod build {
    pub(crate) use super::{def, item, leak};
}
