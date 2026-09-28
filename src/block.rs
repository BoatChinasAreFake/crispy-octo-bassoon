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
/// Number of base-game blocks; mod blocks start here.
pub const NUM_BLOCKS: Id = 34;
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
/// Mod items start here.
pub const FIRST_MOD_ITEM: Id = FIRST_ITEM + 22;

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
    Give(Id, u8),
    SetTime(f32),
    /// Mob kind index (`MobKind::ALL`): 0 oinker, 1 hisser, 2 groaner, 3 fluffer, 4 starer.
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
            def("gold_ore", "Gold Ore (Shiny, Useless)", Cube, true, true, [T_GOLD_ORE; 3], 3.0, 3, true, GOLD_INGOT, 0.0, S_STONE),
            def("pumpkin", "Pumpkin (Fruit? Vegetable? Yes.)", Cube, true, true, [T_PUMPKIN_TOP, T_PUMPKIN_SIDE, T_PUMPKIN_TOP], 1.0, 0, false, PUMPKIN, 0.0, S_WOOD),
            def("jack_o_lantern", "Jack o'Lantern (Faces Everywhere)", Cube, true, true, [T_PUMPKIN_TOP, T_JACK_FACE, T_PUMPKIN_TOP], 1.0, 0, false, JACK, 12.0, S_WOOD),
            def("cactus", "Pokey Plant (Do Not Hug)", Cube, true, true, [T_CACTUS_TOP, T_CACTUS_SIDE, T_CACTUS_TOP], 0.4, 0, false, CACTUS, 0.0, S_GRASS),
            def("ice", "Ice (Nature's Floor Wax)", Cube, true, true, [T_ICE; 3], 0.5, 0, false, AIR, 0.0, S_GLASS),
            def("bouncy_goo", "Bouncy Goo Block", Cube, true, true, [T_BOUNCY; 3], 0.3, 0, false, BOUNCY, 0.0, S_GRASS),
            def("bed", "Bed (One Block, Budget Cuts)", Cube, true, true, [T_BED_TOP, T_BED_SIDE, T_PLANKS], 0.4, 0, false, BED, 0.0, S_WOOD),
            def("cake", "Cake (Not a Lie)", Cube, true, true, [T_CAKE_TOP, T_CAKE_SIDE, T_CAKE_SIDE], 0.5, 0, false, CAKE, 0.0, S_GRASS),
            def("sponge", "Sponge (Very Thirsty)", Cube, true, true, [T_SPONGE; 3], 0.6, 0, false, SPONGE, 0.0, S_GRASS),
            def("wool", "Wool (Ethically Sheared)", Cube, true, true, [T_WOOL; 3], 0.8, 0, false, WOOL, 0.0, S_GRASS),
        ];
        blocks[GLASS as usize].see_through = true;
        blocks[ICE as usize].speed = 1.6;
        blocks[BOUNCY as usize].bounce = 0.85;
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
        items.extend([
            item("gold", "Gold Ingot (Too Soft for Anything)", T_GOLD),
            ItemDef { food: Some(20.0), stack: 16, ..item("golden_oinkchop", "Suspiciously Golden Oinkchop", T_GOLD_CHOP) },
            ItemDef { stack: 16, ..item("stare_pearl", "Stare Pearl (Throw to Teleport)", T_PEARL) },
            ItemDef { food: Some(5.0), ..item("mutton", "Raw Baa-con", T_MUTTON) },
        ]);
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
        ];
        Registry { blocks, items, recipes, ores: Vec::new(), plants: Vec::new(), splashes: Vec::new(), mods: Vec::new(), textures: Vec::new() }
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
    !matches!(b.model, Empty | Cross) && !b.see_through
}
/// Can the player point at it (and break it)?
#[inline]
pub fn targetable(id: Id) -> bool {
    id != AIR && id != WATER
}
/// Placing into this cell simply replaces it.
#[inline]
pub fn replaceable(id: Id) -> bool {
    matches!(id, AIR | WATER | TALL_GRASS)
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

/// Health restored when eaten, if edible.
pub fn food_value(id: Id) -> Option<f32> {
    item_def(id).and_then(|i| i.food)
}

/// Mod-defined behaviour when the item is used: (actions, consumes one).
pub fn use_actions(id: Id) -> Option<(&'static [Action], bool)> {
    item_def(id).filter(|i| !i.on_use.is_empty()).map(|i| (i.on_use.as_slice(), i.consume))
}

/// Seconds to break `id` while holding `held`, and whether it drops anything.
pub fn break_time(id: Id, held: Id) -> (f32, bool) {
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
