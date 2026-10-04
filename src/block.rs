//! Blocks, items, tools and recipes, held in a registry that mods extend.
//!
//! Every block and item id is an [`Id`] (two bytes): ids below `FIRST_ITEM` are
//! placeable blocks (base game `0..NUM_BLOCKS`, mods `NUM_BLOCKS..FIRST_ITEM`),
//! the rest are plain items (base game `FIRST_ITEM..FIRST_MOD_ITEM`, mods after
//! that). Saves and the network store names for mod things, so the numbers
//! themselves can move between versions.

use crate::potions::Potion;
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
/// The Hollow (see hollow.rs).
pub const HOLLOW_STONE: Id = 271;
pub const HOLLOW_PORTAL: Id = 272;
pub const EYE_FRAME: Id = 273;
pub const EYE_FRAME_FULL: Id = 274;
pub const WYRM_CRYSTAL: Id = 275;
pub const WYRM_EGG: Id = 276;
/// Biome blocks (see world.rs): two more kinds of tree, swamp, badlands and jungle bits.
pub const SPRUCE_LOG: Id = 277;
pub const SPRUCE_LEAVES: Id = 278;
pub const JUNGLE_LOG: Id = 279;
pub const JUNGLE_LEAVES: Id = 280;
pub const MUD: Id = 281;
pub const LILY_PAD: Id = 282;
pub const RED_SAND: Id = 283;
/// Terracotta: plain, then orange, red and yellow bands.
pub const TERRACOTTA: Id = 284;
pub const DEAD_BUSH: Id = 288;
pub const MELON: Id = 289;
/// Detector rails: `DETECTOR_RAIL + axis * 2 + on`, like powered rails (see vehicles.rs).
pub const DETECTOR_RAIL: Id = 290;
/// Comparators: `COMPARATOR_FIRST + facing * 4 + more * 2 + on` (see contraptions.rs).
pub const COMPARATOR_FIRST: Id = 294;
/// Beacons: `BEACON_FIRST + ` the effect it gives (`potions::ALL` order; see beacon.rs).
pub const BEACON_FIRST: Id = 310;
/// A chest that keeps what's inside when you break it (see boxes.rs).
pub const HOLLOW_BOX: Id = 315;
pub const COPPER_ORE: Id = 316;
/// Copper blocks, weathering over time: new, exposed, weathered, oxidized (see copper.rs).
pub const COPPER_FIRST: Id = 317;
/// The same, waxed with Goo so they stay as they are.
pub const WAXED_COPPER_FIRST: Id = 321;
pub const BAMBOO: Id = 325;
pub const BAMBOO_BLOCK: Id = 326;
pub const BAMBOO_PLANKS: Id = 327;
pub const BAMBOO_MOSAIC: Id = 328;
/// Bamboo slabs (bottom, top) and stairs (four facings).
pub const BAMBOO_SLAB: Id = 329;
pub const BAMBOO_STAIRS: Id = 331;
/// Coral blocks: tube, brain, bubble, fire; and what they become out of water.
pub const CORAL_FIRST: Id = 335;
pub const DEAD_CORAL: Id = 339;
/// Beekeeping (see bees.rs): wild nests, crafted hives (empty, busy, full of honey).
pub const BEE_NEST: Id = 340;
pub const BEE_NEST_HONEY: Id = 341;
pub const BEEHIVE: Id = 342;
pub const BEEHIVE_BUSY: Id = 343;
pub const BEEHIVE_HONEY: Id = 344;
pub const HONEY_BLOCK: Id = 345;
/// More flowers, for the bees (each makes its own honey).
pub const DANDELION: Id = 346;
pub const CORNFLOWER: Id = 347;
pub const LAVENDER: Id = 348;
/// An ancient flower grown from seeds dug up by archaeologists.
pub const TORCHFLOWER_SPROUT: Id = 349;
pub const TORCHFLOWER: Id = 350;
/// What a Ribbit leaves after eating a small Bloop: colour by climate (see critters.rs).
pub const FROGLIGHT_FIRST: Id = 351;
/// The Deep Dark (see deepdark.rs).
pub const DEEPSLATE: Id = 354;
pub const COBBLED_DEEPSLATE: Id = 355;
pub const DEEPSLATE_BRICKS: Id = 356;
pub const DEEPSLATE_TILES: Id = 357;
pub const REINFORCED_DEEPSLATE: Id = 358;
pub const SCULK: Id = 359;
pub const SCULK_SENSOR: Id = 360;
pub const SCULK_SENSOR_ACTIVE: Id = 361;
pub const SCULK_SHRIEKER: Id = 362;
pub const SCULK_CATALYST: Id = 363;
pub const SOUL_LANTERN: Id = 364;
/// Archaeology (see archaeology.rs).
pub const SUSPICIOUS_SAND: Id = 365;
pub const SUSPICIOUS_GRAVEL: Id = 366;
pub const RESTORATION_BENCH: Id = 367;
/// Decorated pots: plain, then one per shard design.
pub const POT_FIRST: Id = 368;
/// Grinding and smithing (see smithing.rs).
pub const GRINDSTONE: Id = 381;
pub const SMITHING_TABLE: Id = 382;
/// Deep in the Scorchlands: where Scorchite comes from.
pub const OLD_DEBRIS: Id = 383;
/// Note blocks: `NOTE_BLOCK + pitch` (25 pitches; see music.rs).
pub const NOTE_BLOCK: Id = 384;
/// An empty jukebox, then one per disc playing (`JUKEBOX_DISC_FIRST + disc`).
pub const JUKEBOX: Id = 409;
pub const JUKEBOX_DISC_FIRST: Id = 410;
/// The Scorchlands' fortresses and Snout camps (see fortress.rs).
pub const SCORCH_BRICKS: Id = 418;
pub const SIZZLER_CAGE: Id = 419;
pub const GOLD_BLOCK: Id = 420;
pub const GILDED_SCORCHROCK: Id = 421;
/// Every village square has one: ring it when the raiders come (see raids.rs).
pub const BELL: Id = 422;
/// A dungeon's monster cage (see fortress.rs, where the Sizzler Cages live too).
pub const SPAWNER: Id = 423;
/// Cherry Groves and Mangrove Swamps (see world.rs).
pub const CHERRY_LOG: Id = 424;
pub const CHERRY_LEAVES: Id = 425;
pub const PINK_PETALS: Id = 426;
pub const CHERRY_PLANKS: Id = 427;
pub const MANGROVE_LOG: Id = 428;
pub const MANGROVE_LEAVES: Id = 429;
pub const MANGROVE_ROOTS: Id = 430;
pub const MANGROVE_PLANKS: Id = 431;
/// Observers: `OBSERVER_FIRST + facing * 2 + on` (facing: the way it looks; see contraptions.rs).
pub const OBSERVER_FIRST: Id = 432;
/// Crafters: `CRAFTER_FIRST + facing` (facing: where its results come out).
pub const CRAFTER_FIRST: Id = 444;
/// A copper lamp that flips on or off each time power arrives.
pub const COPPER_BULB: Id = 450;
pub const COPPER_BULB_ON: Id = 451;
/// Trial Chambers (see trial.rs).
pub const TUFF_BRICKS: Id = 452;
pub const CHISELED_TUFF: Id = 453;
pub const COPPER_GRATE: Id = 454;
pub const TRIAL_SPAWNER: Id = 455;
/// A trial spawner that's been beaten and is cooling down.
pub const TRIAL_SPAWNER_SPENT: Id = 456;
pub const VAULT: Id = 457;
pub const VAULT_OPEN: Id = 458;
/// Point a compass at it (see navigation.rs).
pub const LODESTONE: Id = 459;
/// Ominous Trials (see trial.rs): a spawner woken by someone with Bad Omen, its vaults, and their prize.
pub const TRIAL_SPAWNER_OMINOUS: Id = 460;
pub const VAULT_OMINOUS: Id = 461;
pub const HEAVY_CORE: Id = 462;
/// The Pale Garden (see world.rs and creaking.rs).
pub const PALE_OAK_LOG: Id = 463;
pub const PALE_OAK_LEAVES: Id = 464;
pub const PALE_OAK_PLANKS: Id = 465;
pub const PALE_MOSS: Id = 466;
pub const PALE_HANGING_MOSS: Id = 467;
/// In a pale oak's trunk: wakes at night and calls up a Creaking.
pub const CREAKING_HEART: Id = 468;
pub const CREAKING_HEART_AWAKE: Id = 469;
/// Sniffers (see sniffers.rs): an egg that hatches, and a Pitcher plant growing up.
pub const SNIFFER_EGG: Id = 470;
pub const PITCHER_CROP: Id = 471;
pub const PITCHER_PLANT: Id = 472;
/// Read books on it (see books.rs); with a book on it.
pub const LECTERN: Id = 473;
pub const LECTERN_BOOK: Id = 474;
/// A banner on a pole (drawn live, see banners.rs), and the Loom that patterns them.
pub const BANNER: Id = 475;
pub const LOOM: Id = 476;
/// Opens to your own storage, wherever it is (see stash.rs).
pub const PERSONAL_CHEST: Id = 477;
/// A chest Copper Golems sort from (see golems.rs).
pub const COPPER_CHEST: Id = 478;
/// Pale Garden flowers: shut by day, open (and glowing a little) at night.
pub const EYEBLOSSOM: Id = 479;
pub const EYEBLOSSOM_OPEN: Id = 480;
/// Resin, from Creaking Hearts (see creaking.rs).
pub const RESIN_BLOCK: Id = 481;
pub const RESIN_BRICKS: Id = 482;
/// Fireflies come out of it at night (see ambient in render.rs).
pub const FIREFLY_BUSH: Id = 483;
/// Thin ground covers.
pub const LEAF_LITTER: Id = 484;
pub const WILDFLOWERS: Id = 485;
/// Put it next to water and it slowly wakes into a baby Floaty (see floaty.rs).
pub const DRIED_FLOATY: Id = 486;
/// Mangrove roots standing in water (drawn with the water in them).
pub const MANGROVE_ROOTS_WET: Id = 487;
/// Cooks food dropped on it, slowly, and sends up smoke (see home.rs).
pub const CAMPFIRE: Id = 488;
/// Furnaces that work twice as fast: one for food, one for everything else.
pub const SMOKER: Id = 489;
pub const SMOKER_LIT: Id = 490;
pub const BLAST_FURNACE: Id = 491;
pub const BLAST_FURNACE_LIT: Id = 492;
/// A chest you can put things on top of.
pub const BARREL: Id = 493;
/// Laid on beaches by turtles (see turtles in animals.rs).
pub const TURTLE_EGG: Id = 494;
/// A painting on one side of its cell (four facings; the picture depends on where it hangs).
pub const PAINTING_FIRST: Id = 495;
/// Shows off a set of armour (four facings; it holds the armour like a chest).
pub const ARMOUR_STAND_FIRST: Id = 499;
/// Building blocks (see masonry.rs): concrete and its powder (which sets in
/// water), and glazed terracotta, each in the eight colours (`+ colour`).
pub const CONCRETE_FIRST: Id = 503;
pub const CONCRETE_POWDER_FIRST: Id = 511;
pub const GLAZED_FIRST: Id = 519;
/// A candle, and one burning.
pub const CANDLE: Id = 527;
pub const CANDLE_LIT: Id = 528;
pub const CHAIN: Id = 529;
/// Climbable from inside, like a ladder you can stand in.
pub const SCAFFOLDING: Id = 530;
/// A lantern hung from the block above.
pub const LANTERN_HANGING: Id = 531;
/// Torches on a wall, leaning out on the side they face (0 north .. 3 west, as frames).
pub const WALL_TORCH_FIRST: Id = 532;
pub const NUM_BLOCKS: Id = 536;

pub fn is_wall_torch(id: Id) -> bool {
    (WALL_TORCH_FIRST..WALL_TORCH_FIRST + 4).contains(&id)
}
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
/// Points the way to a Crypt, and opens its portal (see hollow.rs).
pub const STARING_EYE: Id = FIRST_ITEM + 112;
/// Names a mob (see nametags.rs).
pub const NAME_TAG: Id = FIRST_ITEM + 113;
/// From melons (jungles).
pub const MELON_SLICE: Id = FIRST_ITEM + 114;
/// Carts with a chest or a hopper in them (see vehicles.rs).
pub const CHEST_MINECART: Id = FIRST_ITEM + 115;
pub const HOPPER_MINECART: Id = FIRST_ITEM + 116;
/// Mod items start here.
/// Worn in the chest slot: glide from heights (see glider.rs).
pub const GLIDER: Id = FIRST_ITEM + 117;
/// Speeds up a glide (or just goes bang).
pub const ROCKET: Id = FIRST_ITEM + 118;
/// A Soggy Groaner's weapon: hits hard, and can be thrown.
pub const SPEAR: Id = FIRST_ITEM + 119;
pub const COPPER_INGOT: Id = FIRST_ITEM + 120;
/// Axes and shovels: `+ tier` (wood, stone, copper, iron, dimond; see tools.rs).
pub const AXE_FIRST: Id = FIRST_ITEM + 121;
pub const SHOVEL_FIRST: Id = FIRST_ITEM + 126;
pub const PICK_COPPER: Id = FIRST_ITEM + 131;
pub const SWORD_COPPER: Id = FIRST_ITEM + 132;
/// Copper armour: `+ slot` (helmet, chestplate, leggings, boots).
pub const COPPER_ARMOR_FIRST: Id = FIRST_ITEM + 133;
/// Honey bottles, one per flavour (see `bees::Flavour`).
pub const HONEY_FIRST: Id = FIRST_ITEM + 137;
pub const HONEYCOMB: Id = FIRST_ITEM + 142;
pub const BEE_SMOKER: Id = FIRST_ITEM + 143;
pub const HIVE_TOOL: Id = FIRST_ITEM + 144;
/// A queen in a jar (her temperament rides in the wear, like a box's number).
pub const QUEEN_BEE: Id = FIRST_ITEM + 145;
/// Dug up at a dig site (see archaeology.rs); grows a Torchflower.
pub const TORCHFLOWER_SEEDS: Id = FIRST_ITEM + 146;
/// A Rollo's shed shell plate, and armour for a tame Woofer made of them.
pub const SCUTE: Id = FIRST_ITEM + 147;
pub const WOLF_ARMOR: Id = FIRST_ITEM + 148;
/// Archaeology (see archaeology.rs).
pub const BRUSH: Id = FIRST_ITEM + 149;
pub const DIAMOND_BRUSH: Id = FIRST_ITEM + 150;
pub const FIELD_JOURNAL: Id = FIRST_ITEM + 151;
/// Twelve pottery shards, then twelve relics.
pub const SHARD_FIRST: Id = FIRST_ITEM + 152;
pub const RELIC_FIRST: Id = FIRST_ITEM + 164;
pub const ENCRUSTED_RELIC: Id = FIRST_ITEM + 176;
pub const MAP_FRAGMENT: Id = FIRST_ITEM + 177;
pub const ANCIENT_COIN: Id = FIRST_ITEM + 178;
pub const CLAY_TABLET: Id = FIRST_ITEM + 179;
/// The Deep Dark's treasures (see deepdark.rs).
pub const ECHO_SHARD: Id = FIRST_ITEM + 180;
pub const RECOVERY_COMPASS: Id = FIRST_ITEM + 181;
/// The top tier (see smithing.rs).
pub const SCORCHITE_SCRAP: Id = FIRST_ITEM + 182;
pub const SCORCHITE_INGOT: Id = FIRST_ITEM + 183;
pub const UPGRADE_TEMPLATE: Id = FIRST_ITEM + 184;
pub const PICK_SCORCHITE: Id = FIRST_ITEM + 185;
pub const SWORD_SCORCHITE: Id = FIRST_ITEM + 186;
pub const AXE_SCORCHITE: Id = FIRST_ITEM + 187;
pub const SHOVEL_SCORCHITE: Id = FIRST_ITEM + 188;
pub const SCORCHITE_ARMOR_FIRST: Id = FIRST_ITEM + 189;
/// Music discs (see songs.rs for what's on them).
pub const DISC_FIRST: Id = FIRST_ITEM + 193;
/// The Scorchlands' spoils (see fortress.rs).
pub const SIZZLE_ROD: Id = FIRST_ITEM + 201;
pub const SIZZLE_POWDER: Id = FIRST_ITEM + 202;
pub const WEEPER_TEAR: Id = FIRST_ITEM + 203;
pub const SHROOM_STICK: Id = FIRST_ITEM + 204;
/// Potions past the first five: Strength, Regeneration (and their splash versions).
pub const POTION_EXTRA_FIRST: Id = FIRST_ITEM + 205;
pub const SPLASH_EXTRA_FIRST: Id = FIRST_ITEM + 207;
/// Raids (see raids.rs).
pub const CROSSBOW: Id = FIRST_ITEM + 209;
pub const TOTEM: Id = FIRST_ITEM + 210;
pub const OMINOUS_BANNER: Id = FIRST_ITEM + 211;
/// Opens a Vault (see trial.rs).
pub const TRIAL_KEY: Id = FIRST_ITEM + 212;
/// Thrown: a burst of wind that knocks things back (see trial.rs).
pub const WIND_CHARGE: Id = FIRST_ITEM + 213;
pub const BREEZE_ROD: Id = FIRST_ITEM + 214;
/// Blow it (see critters.rs).
pub const GOAT_HORN: Id = FIRST_ITEM + 215;
/// Armour trim templates: `TRIM_FIRST + pattern` (see trims.rs).
pub const TRIM_FIRST: Id = FIRST_ITEM + 216;
pub const TRIMS: usize = 4;
/// Look through it to zoom in.
pub const SPYGLASS: Id = FIRST_ITEM + 220;
/// Holds a mix of small stacks in one slot (see bundle.rs).
pub const BUNDLE: Id = FIRST_ITEM + 221;
/// Opens an Ominous Vault (see trial.rs).
pub const OMINOUS_TRIAL_KEY: Id = FIRST_ITEM + 222;
/// Hits harder the further you fell first.
pub const MACE: Id = FIRST_ITEM + 223;
/// Drink it for a Bad Omen.
pub const OMINOUS_BOTTLE: Id = FIRST_ITEM + 224;
/// A seed a Sniffer dug up: plant it for a Pitcher plant.
pub const PITCHER_POD: Id = FIRST_ITEM + 225;
/// Write in it; sign it and it's a Written Book (see books.rs).
pub const BOOK_AND_QUILL: Id = FIRST_ITEM + 226;
pub const WRITTEN_BOOK: Id = FIRST_ITEM + 227;
/// Knocked out of a Creaking Heart when its Creaking is hit; bake it into bricks.
pub const RESIN_CLUMP: Id = FIRST_ITEM + 228;
pub const RESIN_BRICK: Id = FIRST_ITEM + 229;
/// Put it on a grown Floaty to ride it (see floaty.rs).
pub const HARNESS: Id = FIRST_ITEM + 230;
/// Craftable spears, wood to dimond (the Soggy Spear sits between stone and iron).
pub const SPEAR_FIRST: Id = FIRST_ITEM + 231;
pub const SPEARS: usize = 4;
/// From shipwrecks and Cartographers: marks where treasure is buried (see treasure.rs).
pub const TREASURE_MAP: Id = FIRST_ITEM + 235;
/// Dropped by turtles as they grow up; five make a Turtle Shell.
pub const TURTLE_SCUTE: Id = FIRST_ITEM + 236;
/// A helmet: you see much further underwater.
pub const TURTLE_SHELL: Id = FIRST_ITEM + 237;
pub const FIRST_MOD_ITEM: Id = FIRST_ITEM + 238;

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
    (WIRE..=LAMP_ON).contains(&id) || matches!(id, COPPER_BULB | COPPER_BULB_ON) || (RAIL_FIRST..POWERED_RAIL + 4).contains(&id) || (DETECTOR_RAIL..DETECTOR_RAIL + 4).contains(&id) || crate::contraptions::is_contraption(id)
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
/// (Row 4 is the Glider's: none; row 5 copper, between gold and iron.)
pub const ARMOR_POINTS: [[u8; 4]; 7] = [[1, 3, 2, 1], [2, 6, 5, 2], [2, 5, 3, 1], [3, 8, 6, 3], [0, 0, 0, 0], [2, 5, 4, 2], [3, 8, 6, 3]];

/// The "tier" a worn Glider counts as: no protection, drawn as wings.
pub const GLIDER_TIER: usize = 4;
/// Copper armour's tier (after the Glider's, so older tiers keep their numbers).
pub const COPPER_TIER: usize = 5;
/// Scorchite armour's row (see smithing.rs).
pub const SCORCHITE_TIER: usize = 6;
/// The Turtle Shell's look.
pub const TURTLE_TIER: usize = 7;

/// (slot, tier) of an armour item.
/// For mod armour, the tier is the one it looks like when worn.
pub fn armor_of(id: Id) -> Option<(usize, usize)> {
    if id == GLIDER {
        return Some((CHESTPLATE, GLIDER_TIER));
    }
    if (COPPER_ARMOR_FIRST..COPPER_ARMOR_FIRST + 4).contains(&id) {
        return Some(((id - COPPER_ARMOR_FIRST) as usize, COPPER_TIER));
    }
    if (SCORCHITE_ARMOR_FIRST..SCORCHITE_ARMOR_FIRST + 4).contains(&id) {
        return Some(((id - SCORCHITE_ARMOR_FIRST) as usize, SCORCHITE_TIER));
    }
    if (ARMOR_FIRST..ARMOR_FIRST + 16).contains(&id) {
        return Some((((id - ARMOR_FIRST) % 4) as usize, ((id - ARMOR_FIRST) / 4) as usize));
    }
    item_def(id).and_then(|i| i.armor).map(|a| (a.slot as usize, a.looks_like as usize))
}

/// How many uses a tool, weapon or piece of armour survives (Minecraft's numbers;
/// wool armour takes leather's). None: it never wears out.
pub fn durability(id: Id) -> Option<u16> {
    const ARMOR: [[u16; 4]; 7] = [[55, 80, 75, 65], [165, 240, 225, 195], [77, 112, 105, 91], [363, 528, 495, 429], [0; 4], [121, 176, 165, 143], [407, 592, 555, 481]];
    match id {
        GLIDER => return Some(432),
        SPEAR => return Some(250),
        SPEAR_FIRST => return Some(60),
        x if x == SPEAR_FIRST + 1 => return Some(132),
        x if x == SPEAR_FIRST + 2 => return Some(250),
        x if x == SPEAR_FIRST + 3 => return Some(1561),
        BRUSH => return Some(64),
        DIAMOND_BRUSH => return Some(256),
        BEE_SMOKER => return Some(64),
        _ => {}
    }
    if let Some(n) = crate::tools::tool_uses(id) {
        return Some(n);
    }
    if let Some((slot, tier)) = armor_of(id)
        && let Some(row) = ARMOR.get(tier)
    {
        return Some(row[slot]);
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
    matches!(id, SWORD_WOOD | SWORD_STONE | SWORD_IRON | SWORD_DIAMOND | SWORD_COPPER | SWORD_SCORCHITE)
        || (id >= FIRST_MOD_ITEM && item_def(id).is_some_and(|i| i.durability.is_some() && i.pick_tier == 0 && i.armor.is_none() && i.damage > 1.0))
}

/// Wear from breaking a block with `held` (swords aren't meant for digging).
pub fn dig_wear(held: Id, broken: Id) -> u16 {
    if durability(held).is_none() || armor_of(held).is_some() || matches!(held, SHIELD | BRUSH | DIAMOND_BRUSH | BEE_SMOKER) || block(broken).hardness <= 0.0 {
        return 0;
    }
    if is_sword(held) { 2 } else { 1 }
}

/// Wear from hitting a mob with `held` (anything but a sword is a clumsy weapon).
pub fn hit_wear(held: Id) -> u16 {
    if durability(held).is_none() || armor_of(held).is_some() || matches!(held, BOW | ROD | SHEARS | SHIELD | SPARKER | BRUSH | DIAMOND_BRUSH | BEE_SMOKER) {
        return 0;
    }
    if is_sword(held) { 1 } else { 2 }
}

pub fn armor_points(id: Id) -> u8 {
    if let Some(a) = item_def(id).and_then(|i| i.armor) {
        return a.points;
    }
    armor_of(id).and_then(|(slot, tier)| ARMOR_POINTS.get(tier).map(|t| t[slot])).unwrap_or(0)
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
    /// A thin sheet three quarters of the way up (the Hollow's portal).
    Sheet,
    /// A bell hanging from a beam.
    Bell,
    /// Two logs crossed on the ground.
    Campfire,
    /// A painting flat on one side of its cell.
    Painting { facing: u8 },
    /// An armour stand: a base and a post, its shoulders across `facing`.
    Stand { facing: u8 },
    /// A small candle standing on the floor.
    Candle,
    /// A thin chain running up and down the middle of the cell.
    Chain,
    /// Scaffolding: four posts and a platform on top.
    Scaffold,
    /// A lantern hanging from the block above.
    Hanging,
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
            Shape::Sheet => ([([0.0, 0.7, 0.0], [1.0, 0.75, 1.0]), full, full], 1),
            Shape::Bell => ([([0.0, 0.8125, 0.4375], [1.0, 0.9375, 0.5625]), ([0.3125, 0.375, 0.3125], [0.6875, 0.8125, 0.6875]), ([0.25, 0.25, 0.25], [0.75, 0.375, 0.75])], 3),
            Shape::Brewer => ([([0.0625, 0.0, 0.0625], [0.9375, 0.125, 0.9375]), ([0.4375, 0.125, 0.4375], [0.5625, 0.875, 0.5625]), ([0.25, 0.5, 0.4375], [0.75, 0.625, 0.5625])], 3),
            Shape::Trapdoor { open: false, .. } => ([([0.0; 3], [1.0, 0.1875, 1.0]), full, full], 1),
            Shape::Trapdoor { facing, open: true } => ([side_box(facing), full, full], 1),
            Shape::Candle => ([([0.4375, 0.0, 0.4375], [0.5625, 0.4375, 0.5625]), full, full], 1),
            Shape::Chain => ([([0.4375, 0.0, 0.4375], [0.5625, 1.0, 0.5625]), full, full], 1),
            Shape::Scaffold => ([([0.0, 0.875, 0.0], [1.0, 1.0, 1.0]), ([0.0, 0.0, 0.0], [0.125, 0.875, 0.125]), ([0.875, 0.0, 0.875], [1.0, 0.875, 1.0])], 3),
            Shape::Hanging => ([([0.3125, 0.0625, 0.3125], [0.6875, 0.5625, 0.6875]), ([0.4375, 0.5625, 0.4375], [0.5625, 1.0, 0.5625]), full], 2),
            Shape::Campfire => ([([0.0, 0.0, 0.3125], [1.0, 0.25, 0.6875]), ([0.3125, 0.25, 0.0], [0.6875, 0.4375, 1.0]), full], 2),
            Shape::Painting { facing } => {
                let t = 1.0 / 16.0;
                let b = match facing % 4 {
                    0 => ([0.0, 0.0, 0.0], [1.0, 1.0, t]),
                    1 => ([1.0 - t, 0.0, 0.0], [1.0, 1.0, 1.0]),
                    2 => ([0.0, 0.0, 1.0 - t], [1.0, 1.0, 1.0]),
                    _ => ([0.0, 0.0, 0.0], [t, 1.0, 1.0]),
                };
                ([b, full, full], 1)
            }
            Shape::Stand { facing } => {
                let base = ([0.125, 0.0, 0.125], [0.875, 0.0625, 0.875]);
                let post = ([0.4375, 0.0625, 0.4375], [0.5625, 1.0, 0.5625]);
                let bar = if facing % 2 == 0 { ([0.125, 0.75, 0.4375], [0.875, 0.8125, 0.5625]) } else { ([0.4375, 0.75, 0.125], [0.5625, 0.8125, 0.875]) };
                ([base, post, bar], 3)
            }
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
    if id == RAIL_FIRST || id == POWERED_RAIL || id == DETECTOR_RAIL {
        return Some(id);
    }
    // Signs and frames come in four facings; the item is the first.
    if (SIGN_FIRST..SIGN_FIRST + 4).contains(&id) {
        return Some(SIGN_FIRST);
    }
    if (FRAME_FIRST..FRAME_FIRST + 4).contains(&id) {
        return Some(FRAME_FIRST);
    }
    if crate::home::is_painting(id) {
        return Some(PAINTING_FIRST);
    }
    if crate::home::is_stand(id) {
        return Some(ARMOUR_STAND_FIRST);
    }
    if id == SMOKER_LIT || id == BLAST_FURNACE_LIT {
        return Some(id - 1);
    }
    if id == CANDLE_LIT {
        return Some(CANDLE);
    }
    if id == LANTERN_HANGING {
        return Some(LANTERN);
    }
    if is_wall_torch(id) {
        return Some(TORCH);
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

/// A bounded shape for a modded mob's projectile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectileModel {
    Arrow,
    Billboard,
    Cube,
}

impl ProjectileModel {
    pub fn to_wire(self) -> u8 {
        match self {
            ProjectileModel::Arrow => 0,
            ProjectileModel::Billboard => 1,
            ProjectileModel::Cube => 2,
        }
    }

    pub fn from_wire(value: u8) -> ProjectileModel {
        match value {
            1 => ProjectileModel::Billboard,
            2 => ProjectileModel::Cube,
            _ => ProjectileModel::Arrow,
        }
    }
}

/// The transient, client-visible appearance of a modded mob's projectile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectileAppearance {
    pub model: ProjectileModel,
    pub tile: Option<u16>,
    pub scale: f32,
}

impl Default for ProjectileAppearance {
    fn default() -> Self {
        Self { model: ProjectileModel::Arrow, tile: None, scale: 1.0 }
    }
}

impl ProjectileAppearance {
    /// Normalize untrusted wire values before they can reach the renderer.
    pub fn from_wire(model: u8, tile: u16, scale: f32) -> Self {
        let max_tile = TILES_PER_ROW * TILES_PER_ROW;
        Self {
            model: ProjectileModel::from_wire(model),
            tile: (tile != u16::MAX && tile < max_tile).then_some(tile),
            scale: if scale.is_finite() { scale.clamp(0.25, 4.0) } else { 1.0 },
        }
    }

    pub fn normalized(self) -> Self {
        Self::from_wire(self.model.to_wire(), self.tile.unwrap_or(u16::MAX), self.scale)
    }
}

/// A bounded timed effect carried by a host-owned modded projectile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectileEffect {
    pub kind: Potion,
    pub duration: f32,
    pub amplifier: u8,
}

/// A new mob type defined by a mod (a `[mob <name>]` section in `mod.txt`).
///
/// A modded mob borrows one of a handful of base-game body shapes (`template`)
/// and paints it with the mod's own texture tile. It has its own size, health,
/// walking speed and (optionally) a single drop. A `hostile` mob with
/// `attack_damage > 0` will pursue and melee-attack players exactly like a base
/// hostile mob (host-authoritative and deterministic); a `hostile` mob with
/// `attack_damage == 0` only counts toward the night-time monster cap and
/// otherwise behaves like a passive wanderer (the backward-compatible default,
/// so existing mod.txt files are unchanged). A `hostile` mob with
/// `ranged_damage > 0` or a configured projectile effect additionally fires
/// projectiles at players, exactly like a base ranged attacker
/// (host-authoritative and deterministic). Their bounded arrow, billboard or
/// cube appearance, safe timed hit effect, and one-to-five-shot deterministic
/// fan are data-defined; projectile state and appearance are synchronized to
/// joined clients. Zero damage with no effect (the default) keeps the ranged
/// attack off, so existing mod.txt files are unchanged. Behaviours that need
/// code (bosses, taming, raids, trading, flying, homing/AoE projectiles,
/// bespoke geometry/UI) are deliberately out of scope so no modded mob can
/// wedge the AI or the renderer. See MODDING.md.
#[derive(Clone, Debug, PartialEq)]
pub struct ModMob {
    /// Stable identifier, e.g. "cheese:mouse" (used by saves and `from_name`).
    pub key: String,
    /// Display name shown in-game.
    pub name: String,
    /// Texture tile painted over every face of the body.
    pub tile: u16,
    /// Half-width and height of the body box, in blocks.
    pub half_width: f32,
    pub height: f32,
    pub max_health: f32,
    /// Walking speed multiplier over the base wander speed.
    pub speed: f32,
    /// Counts toward the night-time monster cap. When combined with
    /// `attack_damage > 0` the mob also pursues and melee-attacks players.
    pub hostile: bool,
    /// Melee damage dealt to a player per hit. `0.0` (the default) means the
    /// mob never attacks, even when `hostile` is true (backward compatible).
    /// Clamped to `0.0..=50.0` at parse time.
    pub attack_damage: f32,
    /// Horizontal distance within which the mob can land a melee hit.
    /// Clamped to `0.5..=4.0` at parse time (default `1.3`).
    pub attack_reach: f32,
    /// How far the mob will notice and pursue a player.
    /// Clamped to `1.0..=48.0` at parse time (default `16.0`).
    pub aggro_range: f32,
    /// Seconds between hits. The non-zero minimum prevents a zero-cooldown DPS
    /// exploit. Clamped to `0.25..=10.0` at parse time (default `1.0`).
    pub attack_cooldown: f32,
    /// Ranged (arrow) damage dealt to a player per shot. `0.0` (the default)
    /// means the mob only fires when `projectile_effect` is configured.
    /// Clamped to `0.0..=30.0` at parse time.
    pub ranged_damage: f32,
    /// Maximum horizontal distance at which the mob opens fire, mirroring
    /// `aggro_range`'s bounds. Clamped to `1.0..=48.0` at parse time
    /// (default `16.0`).
    pub ranged_range: f32,
    /// Launch speed of the fired projectile, in blocks per second (base
    /// `Arrow::SPEED` is `24.0`). Clamped to `8.0..=48.0` at parse time
    /// (default `24.0`).
    pub projectile_speed: f32,
    /// Client-visible projectile shape, optional texture and scale. The classic
    /// base arrow is the default, so existing mods keep their exact appearance.
    pub projectile_appearance: ProjectileAppearance,
    /// Optional bounded timed effect applied on a confirmed player hit.
    pub projectile_effect: Option<ProjectileEffect>,
    /// Projectiles fired in one volley. Clamped to `1..=5` at parse time
    /// (default `1`).
    pub projectile_count: u8,
    /// Total horizontal fan spread in degrees. Clamped to `0.0..=45.0` at
    /// parse time (default `0.0`).
    pub projectile_spread: f32,
    /// How fast its projectiles turn toward the nearest player, in degrees per
    /// second. `0.0` (the default) is an ordinary lobbed shot; above zero the
    /// projectile flies straight (no drop) and steers. Clamped to `0.0..=180.0`.
    pub projectile_homing: f32,
    /// Blast radius in blocks when a projectile lands: everyone inside takes
    /// its damage (less toward the edge) and its effect. Never breaks blocks.
    /// `0.0` (the default) means a plain single-target hit. Clamped to `0.0..=4.0`.
    pub projectile_blast: f32,
    /// Seconds between shots. The `0.5` floor prevents a zero-cooldown
    /// projectile-spam exploit. Clamped to `0.5..=10.0` at parse time
    /// (default `2.0`).
    pub ranged_cooldown: f32,
    /// One item it may drop on death: (item id, up to this many).
    pub drop: Option<(Id, u8)>,
    /// Which base-game body shape to render with.
    pub template: MobTemplate,
    /// Flies instead of walking (no gravity), holding `fly_height` above the
    /// ground and dropping to a player's height to fight.
    pub flying: bool,
    /// Blocks above the ground a flier cruises at. Clamped to `1.0..=16.0`.
    pub fly_height: f32,
    /// A flier's top speed, in blocks per second. Clamped to `0.5..=10.0`.
    pub fly_speed: f32,
    /// A flier that lands now and then to rest (and when told to sit).
    pub perches: bool,
    /// Right-click it with this to (maybe) tame it. None: can't be tamed.
    pub tame_item: Option<Id>,
    /// Chance each `tame_item` works. Clamped to `0.01..=1.0`.
    pub tame_chance: f32,
    /// Feed it this to breed it (and it follows anyone holding it). None: doesn't breed.
    pub breed_item: Option<Id>,
    /// A body built from the mod's own boxes (`part = ...` lines), used
    /// instead of `template` when there are any. At most `MAX_MOD_PARTS`.
    pub parts: Vec<ModPart>,
    /// What it trades, Hmmer style (`trade = ...` lines; at most 8).
    pub trades: Vec<crate::villagers::Trade>,
    /// A boss: a health bar across the top of the screen, never tamed or
    /// despawned, and an announcement when it's beaten.
    pub boss: bool,
    /// Health fraction (0-1) at which a boss enrages; 0 never does.
    pub enrage_at: f32,
    /// Enraged, it moves this many times faster (1-3)...
    pub enrage_speed: f32,
    /// ...and waits this fraction of its usual cooldowns between attacks (0.25-1).
    pub enrage_cooldown: f32,
    /// What an enraged boss calls for help: (mob kind index, how many, 1-4).
    pub summon: Option<(u8, u8)>,
    /// Seconds between calls for help (5-120).
    pub summon_every: f32,
    /// How much it shrugs off knockback (0 = none, 1 = immovable).
    pub knockback_resist: f32,
    /// Experience it drops (0-1000). None: the usual amount.
    pub xp: Option<u32>,
}

/// Most boxes a modded mob's body may have (keeps drawing cheap).
pub const MAX_MOD_PARTS: usize = 16;

/// How one box of a custom modded mob moves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PartAnim {
    /// Doesn't move.
    Still,
    /// Swings forward and back as it walks (legs, arms). Negative amounts swing the other way.
    Walk,
    /// Sweeps side to side as it walks (a tail, spider legs).
    Sway,
    /// Beats up and down as it moves (big wings).
    Wing,
    /// A bird's wing: flaps while flying or falling. Use -1 / 1 for left / right.
    Flap,
    /// Held at a fixed lean of `amount` degrees.
    Tilt,
    /// Bobs up and down as it walks (a head, a jelly).
    Bob,
    /// Spins round as it moves (a propeller, a wheel on its side).
    Spin,
}

impl PartAnim {
    pub fn from_name(s: &str) -> Option<PartAnim> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "still" | "none" | "fixed" => PartAnim::Still,
            "walk" | "swing" | "leg" => PartAnim::Walk,
            "sway" | "tail" => PartAnim::Sway,
            "wing" => PartAnim::Wing,
            "flap" => PartAnim::Flap,
            "tilt" => PartAnim::Tilt,
            "bob" => PartAnim::Bob,
            "spin" => PartAnim::Spin,
            _ => return None,
        })
    }
}

/// One box of a custom modded mob, in blocks relative to its feet (x right,
/// y up, -z forward), turning about `pivot`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModPart {
    pub min: [f32; 3],
    pub size: [f32; 3],
    pub pivot: [f32; 3],
    pub anim: PartAnim,
    /// How much it moves (-3..=3; degrees for `Tilt`, -180..=180).
    pub amount: f32,
    /// +x, -x, +y, -y, +z, -z
    pub tiles: [u16; 6],
}

/// The curated base-game body shapes a modded mob may borrow for v1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MobTemplate {
    /// Four-legged, pig-ish (the default).
    Quadruped,
    /// Two-legged, humanoid.
    Biped,
    /// A squishy cube, slime-ish.
    Blob,
    /// A little two-legged bird.
    Bird,
}

impl MobTemplate {
    /// Accepts the parody shape name or a plain description.
    pub fn from_name(s: &str) -> Option<MobTemplate> {
        match s.trim().to_ascii_lowercase().as_str() {
            "quadruped" | "oinker" | "pig" | "cow" | "mooer" | "animal" => Some(MobTemplate::Quadruped),
            "biped" | "humanoid" | "groaner" | "zombie" | "villager" => Some(MobTemplate::Biped),
            "blob" | "bloop" | "slime" | "cube" => Some(MobTemplate::Blob),
            "bird" | "cluckster" | "chicken" => Some(MobTemplate::Bird),
            _ => None,
        }
    }
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
    /// Mod-defined mob types, in deterministic id order (see entity.rs).
    pub mobs: Vec<ModMob>,
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
            def("crafting_table", "Crafting Table (No Longer Decorative)", Cube, true, true, [T_TABLE_TOP, T_TABLE_SIDE, T_PLANKS], 2.5, 0, false, TABLE, 0.0, S_WOOD),
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
        blocks.push(def("hollow_stone", "Hollow Stone (Pale, Suspicious)", Cube, true, true, [T_HOLLOW_STONE; 3], 3.0, 1, true, HOLLOW_STONE, 0.0, S_STONE));
        let mut sheet = def("hollow_portal", "Hollow Portal (Starry)", Shaped, false, false, [T_HOLLOW_PORTAL; 3], -1.0, 0, false, AIR, 11.0, S_GLASS);
        sheet.shape = Shape::Sheet;
        sheet.creative = false;
        blocks.push(sheet);
        for (key, name, top) in [("eye_frame", "Eye Frame (Empty)", T_EYE_FRAME_TOP), ("eye_frame_full", "Eye Frame (Staring Back)", T_EYE_FRAME_FULL)] {
            let mut d = def(key, name, Cube, true, true, [top, T_EYE_FRAME_SIDE, T_HOLLOW_STONE], -1.0, 0, false, AIR, 0.0, S_STONE);
            d.creative = key == "eye_frame";
            blocks.push(d);
        }
        blocks.push(def("wyrm_crystal", "Wyrm Crystal (Do Not Touch)", Cube, true, false, [T_WYRM_CRYSTAL; 3], 0.3, 0, false, AIR, 12.0, S_GLASS));
        blocks.push(def("wyrm_egg", "Wyrm Egg (Trophy, Allegedly)", Cube, true, true, [T_WYRM_EGG; 3], 3.0, 0, false, WYRM_EGG, 2.0, S_STONE));
        // Biome blocks.
        blocks.push(def("spruce_log", "Spruce Log (Pointy Tree Chunk)", Cube, true, true, [T_SPRUCE_LOG_TOP, T_SPRUCE_LOG_SIDE, T_SPRUCE_LOG_TOP], 2.0, 0, false, SPRUCE_LOG, 0.0, S_WOOD));
        blocks.push(def("spruce_leaves", "Spruce Needles", Cube, true, false, [T_SPRUCE_LEAVES; 3], 0.2, 0, false, AIR, 0.0, S_GRASS));
        blocks.push(def("jungle_log", "Jungle Log (Tall Tree Chunk)", Cube, true, true, [T_JUNGLE_LOG_TOP, T_JUNGLE_LOG_SIDE, T_JUNGLE_LOG_TOP], 2.0, 0, false, JUNGLE_LOG, 0.0, S_WOOD));
        blocks.push(def("jungle_leaves", "Jungle Leaves (Extra Leafy)", Cube, true, false, [T_JUNGLE_LEAVES; 3], 0.2, 0, false, AIR, 0.0, S_GRASS));
        blocks.push(def("mud", "Mud (Squelchy)", Cube, true, true, [T_MUD; 3], 0.5, 0, false, MUD, 0.0, S_GRASS));
        let mut lily = def("lily_pad", "Lily Pad (Frog Not Included)", Shaped, false, false, [T_LILY_PAD; 3], 0.0, 0, false, LILY_PAD, 0.0, S_GRASS);
        lily.shape = Shape::Dust;
        // Stood on (see entity::collides), not swum into.
        lily.solid = true;
        blocks.push(lily);
        blocks.push(def("red_sand", "Red Sand (Sunburnt)", Cube, true, true, [T_RED_SAND; 3], 0.5, 0, false, RED_SAND, 0.0, S_SAND));
        for (i, (key, name)) in [("terracotta", "Terracotta (Fancy Mud)"), ("orange_terracotta", "Orange Terracotta"), ("red_terracotta", "Red Terracotta"), ("yellow_terracotta", "Yellow Terracotta")].into_iter().enumerate() {
            let id = TERRACOTTA + i as Id;
            blocks.push(def(key, name, Cube, true, true, [T_TERRACOTTA + i as u16; 3], 1.2, 1, true, id, 0.0, S_STONE));
        }
        blocks.push(def("dead_bush", "Dead Bush (It's Fine)", Cross, false, false, [T_DEAD_BUSH; 3], 0.0, 0, false, STICK, 0.0, S_GRASS));
        blocks.push(def("melon", "Melon (Heavy Snack)", Cube, true, true, [T_MELON_TOP, T_MELON_SIDE, T_MELON_TOP], 1.0, 0, false, AIR, 0.0, S_WOOD));
        for (k, key) in ["detector_rail", "detector_rail_on", "detector_rail_ew", "detector_rail_ew_on"].into_iter().enumerate() {
            let mut d = def(key, "Detector Rail (Tattletale)", Shaped, false, false, [T_DETECTOR_RAIL + k as u16; 3], 0.7, 0, false, DETECTOR_RAIL, 0.0, S_STONE);
            d.shape = Shape::Dust;
            d.creative = k == 0;
            blocks.push(d);
        }
        for facing in 0..4u8 {
            for more in [false, true] {
                for on in [false, true] {
                    let key = format!("comparator{}{}{}", ["", "_east", "_south", "_west"][facing as usize], if more { "_half" } else { "" }, if on { "_on" } else { "" });
                    let tile = T_COMPARATOR + more as u16 * 2 + on as u16;
                    let mut d = def(leak(&key), "Comparator (Counts Your Stuff)", Shaped, true, false, [tile, T_STONE, T_STONE], 0.0, 0, false, COMPARATOR_FIRST, 0.0, S_STONE);
                    d.shape = Shape::Repeater { facing };
                    d.creative = facing == 0 && !more && !on;
                    d.see_through = true;
                    blocks.push(d);
                }
            }
        }
        for (k, p) in crate::potions::ALL.iter().enumerate() {
            let mut d = def(leak(&format!("beacon{}", if k == 0 { String::new() } else { format!("_{}", p.key()) })), "Beacon (Wyrm-Powered Lighthouse)", Cube, true, false, [T_BEACON; 3], 3.0, 0, false, BEACON_FIRST, 15.0, S_GLASS);
            d.see_through = true;
            d.creative = k == 0;
            blocks.push(d);
        }
        blocks.push(def("hollow_box", "Hollow Box (Bigger on the Inside)", Cube, true, true, [T_HOLLOW_BOX_TOP, T_HOLLOW_BOX_SIDE, T_HOLLOW_BOX_TOP], 2.0, 0, true, AIR, 0.0, S_STONE));
        blocks.push(def("copper_ore", "Copper Ore (Future Statue)", Cube, true, true, [T_COPPER_ORE; 3], 3.0, 1, true, COPPER_INGOT, 0.0, S_STONE));
        let copper_names = ["Copper Block (Shiny, For Now)", "Exposed Copper (Ageing Gracefully)", "Weathered Copper (Mostly Green)", "Oxidized Copper (Statue Chic)"];
        let copper_keys = ["copper_block", "exposed_copper", "weathered_copper", "oxidized_copper"];
        for waxed in [false, true] {
            for (i, (key, name)) in copper_keys.iter().zip(copper_names).enumerate() {
                let id = if waxed { WAXED_COPPER_FIRST } else { COPPER_FIRST } + i as Id;
                let key = if waxed { leak(&format!("waxed_{key}")) } else { key };
                let name = if waxed { leak(&format!("Waxed {}", name.split(" (").next().unwrap_or(name))) } else { name };
                blocks.push(def(key, name, Cube, true, true, [T_COPPER + i as u16; 3], 3.0, 1, true, id, 0.0, S_STONE));
            }
        }
        let mut bamboo = def("bamboo", "Bamboo (Grows While You Watch)", Cross, false, false, [T_BAMBOO; 3], 0.2, 0, false, BAMBOO, 0.0, S_WOOD);
        bamboo.speed = 1.0;
        blocks.push(bamboo);
        blocks.push(def("bamboo_block", "Block of Bamboo (Bundled)", Cube, true, true, [T_BAMBOO_BLOCK_TOP, T_BAMBOO_BLOCK_SIDE, T_BAMBOO_BLOCK_TOP], 2.0, 0, false, BAMBOO_BLOCK, 0.0, S_WOOD));
        blocks.push(def("bamboo_planks", "Bamboo Planks (Stripey)", Cube, true, true, [T_BAMBOO_PLANKS; 3], 2.0, 0, false, BAMBOO_PLANKS, 0.0, S_WOOD));
        blocks.push(def("bamboo_mosaic", "Bamboo Mosaic (Fancy Stripes)", Cube, true, true, [T_BAMBOO_MOSAIC; 3], 2.0, 0, false, BAMBOO_MOSAIC, 0.0, S_WOOD));
        for top in [false, true] {
            let mut d = def(if top { "bamboo_slab_top" } else { "bamboo_slab" }, if top { "Bamboo Slab (Upside Down)" } else { "Bamboo Slab (Half the Commitment)" }, Shaped, true, false, [T_BAMBOO_PLANKS; 3], 2.0, 0, false, BAMBOO_SLAB, 0.0, S_WOOD);
            d.shape = Shape::Slab { top };
            d.creative = !top;
            d.family = BAMBOO_SLAB;
            d.full = BAMBOO_PLANKS;
            blocks.push(d);
        }
        for facing in 0..4u8 {
            let key = leak(&format!("bamboo_stairs{}", ["", "_east", "_south", "_west"][facing as usize]));
            let mut d = def(key, "Bamboo Stairs (Up, Mostly)", Shaped, true, false, [T_BAMBOO_PLANKS; 3], 2.0, 0, false, BAMBOO_STAIRS, 0.0, S_WOOD);
            d.shape = Shape::Stairs { facing };
            d.creative = facing == 0;
            d.family = BAMBOO_STAIRS;
            d.full = BAMBOO_PLANKS;
            blocks.push(d);
        }
        let corals = [("tube_coral_block", "Tube Coral Block (Blue, Busy)"), ("brain_coral_block", "Brain Coral Block (Thinks Pink)"), ("bubble_coral_block", "Bubble Coral Block (Pops Purple)"), ("fire_coral_block", "Fire Coral Block (Not Actually Hot)")];
        for (i, (key, name)) in corals.into_iter().enumerate() {
            blocks.push(def(key, name, Cube, true, true, [T_CORAL + i as u16; 3], 1.5, 1, true, CORAL_FIRST + i as Id, 0.0, S_STONE));
        }
        blocks.push(def("dead_coral_block", "Dead Coral Block (Needed Water)", Cube, true, true, [T_DEAD_CORAL; 3], 1.5, 1, true, DEAD_CORAL, 0.0, S_STONE));
        // Bees. Nests and hives drop what the colony decides (see bees.rs).
        for (key, name, side, creative) in [("bee_nest", "Bee Nest (Occupied, Buzzing)", T_NEST_SIDE, true), ("bee_nest_honey", "Bee Nest (Dripping Honey)", T_NEST_HONEY_SIDE, false)] {
            let mut d = def(key, name, Cube, true, true, [T_NEST_TOP, side, T_NEST_TOP], 0.3, 0, false, AIR, 0.0, S_WOOD);
            d.creative = creative;
            blocks.push(d);
        }
        for (key, name, side, creative) in [
            ("beehive", "Beehive (Vacancies Available)", T_HIVE_SIDE, true),
            ("beehive_busy", "Beehive (Occupied)", T_HIVE_BUSY_SIDE, false),
            ("beehive_honey", "Beehive (Full of Honey)", T_HIVE_HONEY_SIDE, false),
        ] {
            let mut d = def(key, name, Cube, true, true, [T_HIVE_TOP, side, T_HIVE_TOP], 0.6, 0, false, BEEHIVE, 0.0, S_WOOD);
            d.creative = creative;
            blocks.push(d);
        }
        let mut honey = def("honey_block", "Honey Block (Sticky Situation)", Cube, true, false, [T_HONEY_BLOCK_TOP, T_HONEY_BLOCK_SIDE, T_HONEY_BLOCK_TOP], 0.2, 0, false, HONEY_BLOCK, 0.0, S_GRASS);
        honey.speed = 0.4;
        honey.see_through = true;
        blocks.push(honey);
        blocks.push(def("dandelion", "Dandelion (Sunny Disposition)", Cross, false, false, [T_DANDELION; 3], 0.0, 0, false, DANDELION, 0.0, S_GRASS));
        blocks.push(def("cornflower", "Cornflower (Suspiciously Blue)", Cross, false, false, [T_CORNFLOWER; 3], 0.0, 0, false, CORNFLOWER, 0.0, S_GRASS));
        blocks.push(def("lavender", "Lavender (Smells Calming)", Cross, false, false, [T_LAVENDER; 3], 0.0, 0, false, LAVENDER, 0.0, S_GRASS));
        let mut sprout = def("torchflower_sprout", "Torchflower Sprout (Very Old Seed)", Cross, false, false, [T_TORCH_SPROUT; 3], 0.0, 0, false, TORCHFLOWER_SEEDS, 0.0, S_GRASS);
        sprout.creative = false;
        blocks.push(sprout);
        blocks.push(def("torchflower", "Torchflower (Ancient, Glowing)", Cross, false, false, [T_TORCHFLOWER; 3], 0.0, 0, false, TORCHFLOWER, 9.0, S_GRASS));
        for (i, (key, name)) in [("ochre_froglight", "Ochre Froglight (Warm Glow)"), ("verdant_froglight", "Verdant Froglight (Cool Glow)"), ("pearlescent_froglight", "Pearlescent Froglight (Fancy Glow)")].into_iter().enumerate() {
            blocks.push(def(key, name, Cube, true, true, [T_FROGLIGHT + i as u16; 3], 0.3, 0, false, FROGLIGHT_FIRST + i as Id, 15.0, S_GRASS));
        }
        // The Deep Dark.
        let mut deepslate = def("deepslate", "Deepslate (Stone, But Moody)", Cube, true, true, [T_DEEPSLATE_TOP, T_DEEPSLATE, T_DEEPSLATE_TOP], 3.0, 1, true, COBBLED_DEEPSLATE, 0.0, S_STONE);
        deepslate.creative = true;
        blocks.push(deepslate);
        blocks.push(def("cobbled_deepslate", "Cobbled Deepslate (Moody Rubble)", Cube, true, true, [T_COBBLED_DEEPSLATE; 3], 3.5, 1, true, COBBLED_DEEPSLATE, 0.0, S_STONE));
        blocks.push(def("deepslate_bricks", "Deepslate Bricks (Very Serious)", Cube, true, true, [T_DEEPSLATE_BRICKS; 3], 3.5, 1, true, DEEPSLATE_BRICKS, 0.0, S_STONE));
        blocks.push(def("deepslate_tiles", "Deepslate Tiles (Tidy Gloom)", Cube, true, true, [T_DEEPSLATE_TILES; 3], 3.5, 1, true, DEEPSLATE_TILES, 0.0, S_STONE));
        let mut reinforced = def("reinforced_deepslate", "Reinforced Deepslate (Absolutely Not)", Cube, true, true, [T_REINFORCED_TOP, T_REINFORCED_SIDE, T_REINFORCED_TOP], -1.0, 0, false, AIR, 0.0, S_STONE);
        reinforced.creative = true;
        blocks.push(reinforced);
        blocks.push(def("sculk", "Sculk (Squishy, Listening)", Cube, true, true, [T_SCULK; 3], 0.6, 0, false, AIR, 0.0, S_GRASS));
        blocks.push(def("sculk_sensor", "Sculk Sensor (Heard That)", Cube, true, true, [T_SENSOR_TOP, T_SENSOR_SIDE, T_SCULK], 1.5, 0, false, SCULK_SENSOR, 1.0, S_GRASS));
        let mut active = def("sculk_sensor_active", "Sculk Sensor (Definitely Heard That)", Cube, true, true, [T_SENSOR_ACTIVE_TOP, T_SENSOR_SIDE, T_SCULK], 1.5, 0, false, SCULK_SENSOR, 8.0, S_GRASS);
        active.creative = false;
        blocks.push(active);
        blocks.push(def("sculk_shrieker", "Sculk Shrieker (Do Not Wake)", Cube, true, true, [T_SHRIEKER_TOP, T_SHRIEKER_SIDE, T_SCULK], 3.0, 0, false, SCULK_SHRIEKER, 0.0, S_GRASS));
        blocks.push(def("sculk_catalyst", "Sculk Catalyst (Feeds on Endings)", Cube, true, true, [T_CATALYST_TOP, T_CATALYST_SIDE, T_SCULK], 3.0, 0, false, SCULK_CATALYST, 6.0, S_GRASS));
        let mut soul = def("soul_lantern", "Soul Lantern (Spooky Blue)", Cube, true, false, [T_SOUL_LANTERN; 3], 0.8, 0, false, SOUL_LANTERN, 10.0, S_GLASS);
        soul.see_through = true;
        blocks.push(soul);
        // Archaeology.
        blocks.push(def("suspicious_sand", "Suspicious Sand (Brush, Don't Dig)", Cube, true, true, [T_SUS_SAND; 3], 0.5, 0, false, SAND, 0.0, S_SAND));
        blocks.push(def("suspicious_gravel", "Suspicious Gravel (Brush, Don't Dig)", Cube, true, true, [T_SUS_GRAVEL; 3], 0.6, 0, false, GRAVEL, 0.0, S_SAND));
        blocks.push(def("restoration_bench", "Restoration Bench (Patience Required)", Cube, true, true, [T_BENCH_TOP, T_BENCH_SIDE, T_PLANKS], 2.5, 0, false, RESTORATION_BENCH, 0.0, S_WOOD));
        for i in 0..13usize {
            let key = if i == 0 { "decorated_pot".to_string() } else { format!("decorated_pot_{}", crate::archaeology::SHARDS[i - 1].0) };
            let name = if i == 0 { "Decorated Pot (Plain, Tasteful)".to_string() } else { format!("Decorated Pot ({})", crate::archaeology::SHARDS[i - 1].1) };
            let mut d = def(leak(&key), leak(&name), Shaped, true, false, [T_POT_TOP, T_POT_SIDE_FIRST + i as u16, T_POT_TOP], 0.1, 0, false, AIR, 0.0, S_STONE);
            d.shape = Shape::Table;
            blocks.push(d);
        }
        let mut grind = def("grindstone", "Grindstone (Unenchanting Since Forever)", Shaped, true, false, [T_GRINDSTONE_TOP, T_GRINDSTONE_SIDE, T_PLANKS], 2.0, 1, true, GRINDSTONE, 0.0, S_STONE);
        grind.shape = Shape::Table;
        blocks.push(grind);
        blocks.push(def("smithing_table", "Smithing Table (Hammer Time)", Cube, true, true, [T_SMITHING_TOP, T_SMITHING_SIDE, T_PLANKS], 2.5, 0, false, SMITHING_TABLE, 0.0, S_WOOD));
        blocks.push(def("old_debris", "Old Debris (Ancient, Stubborn)", Cube, true, true, [T_OLD_DEBRIS_TOP, T_OLD_DEBRIS_SIDE, T_OLD_DEBRIS_TOP], 15.0, 3, true, OLD_DEBRIS, 0.0, S_STONE));
        // Music (see music.rs): a note block per pitch, a jukebox per disc.
        for p in 0..crate::songs::PITCHES as u16 {
            let key = if p == 0 { "note_block".to_string() } else { format!("note_block_{p}") };
            let name = if p == 0 { "Note Block (Plinky)".to_string() } else { format!("Note Block (Plinky, Note {p})") };
            let mut d = def(leak(&key), leak(&name), Cube, true, true, [T_NOTE_BLOCK; 3], 0.8, 0, false, NOTE_BLOCK, 0.0, S_WOOD);
            d.creative = p == 0;
            blocks.push(d);
        }
        blocks.push(def("jukebox", "Jukebox (Plays Your Jams)", Cube, true, true, [T_JUKEBOX_TOP, T_JUKEBOX_SIDE, T_JUKEBOX_SIDE], 2.0, 0, false, JUKEBOX, 0.0, S_WOOD));
        for (key, title) in crate::songs::DISCS {
            let mut d = def(leak(&format!("jukebox_{}", &key[11..])), leak(&format!("Jukebox (Playing {title})")), Cube, true, true, [T_JUKEBOX_TOP, T_JUKEBOX_SIDE, T_JUKEBOX_SIDE], 2.0, 0, false, JUKEBOX, 0.0, S_WOOD);
            d.creative = false;
            blocks.push(d);
        }
        // The Scorchlands' fortresses and camps.
        blocks.push(def("scorch_bricks", "Scorch Bricks (Grim Masonry)", Cube, true, true, [T_SCORCH_BRICKS; 3], 2.0, 1, true, SCORCH_BRICKS, 0.0, S_STONE));
        let mut cage = def("sizzler_cage", "Sizzler Cage (Spawns Trouble)", Cube, true, false, [T_CAGE; 3], 5.0, 1, true, AIR, 3.0, S_STONE);
        cage.see_through = true;
        blocks.push(cage);
        blocks.push(def("gold_block", "Gold Block (Shiny, Still Useless)", Cube, true, true, [T_GOLD_BLOCK; 3], 3.0, 2, true, GOLD_BLOCK, 0.0, S_STONE));
        blocks.push(def("gilded_scorchrock", "Gilded Scorchrock (Snout Chic)", Cube, true, true, [T_GILDED; 3], 1.5, 1, true, GILDED_SCORCHROCK, 0.0, S_STONE));
        let mut bell = def("bell", "Bell (Ding Dong)", Shaped, true, false, [T_BELL; 3], 5.0, 1, true, BELL, 0.0, S_STONE);
        bell.shape = Shape::Bell;
        blocks.push(bell);
        let mut spawner = def("monster_cage", "Monster Cage (Still Occupied)", Cube, true, false, [T_SPAWNER; 3], 5.0, 1, true, AIR, 2.0, S_STONE);
        spawner.see_through = true;
        blocks.push(spawner);
        // Cherry Groves and Mangrove Swamps.
        blocks.push(def("cherry_log", "Cherry Log (Pink Inside)", Cube, true, true, [T_CHERRY_LOG_TOP, T_CHERRY_LOG_SIDE, T_CHERRY_LOG_TOP], 2.0, 0, false, CHERRY_LOG, 0.0, S_WOOD));
        blocks.push(def("cherry_leaves", "Cherry Blossom (Extremely Pink)", Cube, true, false, [T_CHERRY_LEAVES; 3], 0.2, 0, false, AIR, 0.0, S_GRASS));
        let mut petals = def("pink_petals", "Pink Petals (Ground Confetti)", Shaped, false, false, [T_PINK_PETALS; 3], 0.0, 0, false, PINK_PETALS, 0.0, S_GRASS);
        petals.shape = Shape::Dust;
        blocks.push(petals);
        blocks.push(def("cherry_planks", "Cherry Planks (Blush)", Cube, true, true, [T_CHERRY_PLANKS; 3], 2.0, 0, false, CHERRY_PLANKS, 0.0, S_WOOD));
        blocks.push(def("mangrove_log", "Mangrove Log (Damp Tree Chunk)", Cube, true, true, [T_MANGROVE_LOG_TOP, T_MANGROVE_LOG_SIDE, T_MANGROVE_LOG_TOP], 2.0, 0, false, MANGROVE_LOG, 0.0, S_WOOD));
        blocks.push(def("mangrove_leaves", "Mangrove Leaves (Salty)", Cube, true, false, [T_MANGROVE_LEAVES; 3], 0.2, 0, false, AIR, 0.0, S_GRASS));
        let mut roots = def("mangrove_roots", "Mangrove Roots (Tangled)", Cube, true, false, [T_MANGROVE_ROOTS; 3], 0.7, 0, false, MANGROVE_ROOTS, 0.0, S_WOOD);
        roots.see_through = true;
        blocks.push(roots);
        blocks.push(def("mangrove_planks", "Mangrove Planks (Reddish)", Cube, true, true, [T_MANGROVE_PLANKS; 3], 2.0, 0, false, MANGROVE_PLANKS, 0.0, S_WOOD));
        // Automation (see contraptions.rs).
        for facing in 0..6u8 {
            for on in [false, true] {
                let key = format!("observer{}{}", ["", "_east", "_south", "_west", "_up", "_down"][facing as usize], if on { "_on" } else { "" });
                let mut d = def(leak(&key), "Observer (Nosy)", Cube, true, true, [T_OBSERVER_SIDE; 3], 3.0, 1, true, OBSERVER_FIRST, 0.0, S_STONE);
                d.creative = facing == 0 && !on;
                blocks.push(d);
            }
        }
        for facing in 0..6u8 {
            let key = format!("crafter{}", ["", "_east", "_south", "_west", "_up", "_down"][facing as usize]);
            let mut d = def(leak(&key), "Crafter (Crafts Unsupervised)", Cube, true, true, [T_CRAFTER_TOP, T_CRAFTER_SIDE, T_CRAFTER_SIDE], 3.5, 1, true, CRAFTER_FIRST, 0.0, S_STONE);
            d.creative = facing == 0;
            blocks.push(d);
        }
        blocks.push(def("copper_bulb", "Copper Bulb (Off, For Now)", Cube, true, true, [T_COPPER_BULB; 3], 3.0, 1, true, COPPER_BULB, 0.0, S_STONE));
        let mut lit = def("copper_bulb_on", "Copper Bulb (On, For Now)", Cube, true, true, [T_COPPER_BULB_ON; 3], 3.0, 1, true, COPPER_BULB, 15.0, S_STONE);
        lit.creative = false;
        blocks.push(lit);
        // Trial Chambers.
        blocks.push(def("tuff_bricks", "Tuff Bricks (Grey, Proud)", Cube, true, true, [T_TUFF_BRICKS; 3], 1.5, 1, true, TUFF_BRICKS, 0.0, S_STONE));
        blocks.push(def("chiseled_tuff", "Chiseled Tuff (Fancy Grey)", Cube, true, true, [T_CHISELED_TUFF_TOP, T_CHISELED_TUFF, T_CHISELED_TUFF_TOP], 1.5, 1, true, CHISELED_TUFF, 0.0, S_STONE));
        let mut grate = def("copper_grate", "Copper Grate (Holey)", Cube, true, false, [T_COPPER_GRATE; 3], 3.0, 1, true, COPPER_GRATE, 0.0, S_STONE);
        grate.see_through = true;
        blocks.push(grate);
        for (key, name, tile, light) in [("trial_spawner", "Trial Spawner (Testing You)", T_TRIAL_SPAWNER, 4.0), ("trial_spawner_spent", "Trial Spawner (Resting)", T_TRIAL_SPAWNER_SPENT, 0.0)] {
            let mut d = def(key, name, Cube, true, false, [T_TRIAL_SPAWNER_TOP, tile, T_TRIAL_SPAWNER_TOP], 50.0, 2, true, AIR, light, S_STONE);
            d.see_through = true;
            d.creative = key == "trial_spawner";
            blocks.push(d);
        }
        for (key, name, front) in [("vault", "Vault (Locked)", T_VAULT_FRONT), ("vault_open", "Vault (Emptied)", T_VAULT_OPEN)] {
            let mut d = def(key, name, Cube, true, true, [T_VAULT_TOP, front, T_VAULT_TOP], 50.0, 2, true, AIR, if key == "vault" { 6.0 } else { 0.0 }, S_STONE);
            d.creative = key == "vault";
            blocks.push(d);
        }
        blocks.push(def("lodestone", "Lodestone (Magnetic Personality)", Cube, true, true, [T_LODESTONE_TOP, T_LODESTONE_SIDE, T_LODESTONE_TOP], 3.5, 1, true, LODESTONE, 0.0, S_STONE));
        let mut ominous = def("ominous_trial_spawner", "Trial Spawner (Ominous)", Cube, true, false, [T_TRIAL_SPAWNER_TOP, T_OMINOUS_SPAWNER, T_TRIAL_SPAWNER_TOP], 50.0, 2, true, AIR, 6.0, S_STONE);
        ominous.see_through = true;
        ominous.creative = false;
        blocks.push(ominous);
        let mut ov = def("ominous_vault", "Vault (Ominous, Locked)", Cube, true, true, [T_VAULT_TOP, T_VAULT_OMINOUS, T_VAULT_TOP], 50.0, 2, true, AIR, 6.0, S_STONE);
        ov.creative = false;
        blocks.push(ov);
        blocks.push(def("heavy_core", "Heavy Core (Surprisingly Dense)", Cube, true, true, [T_HEAVY_CORE; 3], 10.0, 1, true, HEAVY_CORE, 0.0, S_STONE));
        // The Pale Garden.
        blocks.push(def("pale_oak_log", "Pale Oak Log (Washed Out)", Cube, true, true, [T_PALE_LOG_TOP, T_PALE_LOG_SIDE, T_PALE_LOG_TOP], 2.0, 0, false, PALE_OAK_LOG, 0.0, S_WOOD));
        blocks.push(def("pale_oak_leaves", "Pale Oak Leaves (Ghostly)", Cube, true, false, [T_PALE_LEAVES; 3], 0.2, 0, false, AIR, 0.0, S_GRASS));
        blocks.push(def("pale_oak_planks", "Pale Oak Planks (Off-White)", Cube, true, true, [T_PALE_PLANKS; 3], 2.0, 0, false, PALE_OAK_PLANKS, 0.0, S_WOOD));
        blocks.push(def("pale_moss", "Pale Moss (Grey Carpet)", Cube, true, true, [T_PALE_MOSS, T_PALE_MOSS, T_DIRT], 0.1, 0, false, PALE_MOSS, 0.0, S_GRASS));
        blocks.push(def("pale_hanging_moss", "Pale Hanging Moss (Dangly)", Cross, false, false, [T_PALE_HANGING_MOSS; 3], 0.0, 0, false, PALE_HANGING_MOSS, 0.0, S_GRASS));
        blocks.push(def("creaking_heart", "Creaking Heart (Asleep)", Cube, true, true, [T_PALE_LOG_TOP, T_CREAKING_HEART, T_PALE_LOG_TOP], 10.0, 0, false, CREAKING_HEART, 0.0, S_WOOD));
        let mut awake = def("creaking_heart_awake", "Creaking Heart (Awake)", Cube, true, true, [T_PALE_LOG_TOP, T_CREAKING_HEART_ON, T_PALE_LOG_TOP], 10.0, 0, false, CREAKING_HEART, 6.0, S_WOOD);
        awake.creative = false;
        blocks.push(awake);
        // Sniffers.
        let mut egg = def("sniffer_egg", "Sniffer Egg (Ancient, Still Warm)", Shaped, true, false, [T_SNIFFER_EGG; 3], 0.5, 0, false, SNIFFER_EGG, 0.0, S_WOOD);
        egg.see_through = true;
        egg.shape = Shape::Table;
        blocks.push(egg);
        let mut crop = def("pitcher_crop", "Pitcher Sprout (Taking Its Time)", Cross, false, false, [T_PITCHER_CROP; 3], 0.0, 0, false, PITCHER_POD, 0.0, S_GRASS);
        crop.creative = false;
        blocks.push(crop);
        blocks.push(def("pitcher_plant", "Pitcher Plant (Ancient, Teal)", Cross, false, false, [T_PITCHER_PLANT; 3], 0.0, 0, false, PITCHER_PLANT, 0.0, S_GRASS));
        let mut lectern = def("lectern", "Lectern (Reading Stand)", Shaped, true, false, [T_LECTERN_TOP, T_LECTERN_SIDE, T_PLANKS], 2.5, 0, false, LECTERN, 0.0, S_WOOD);
        lectern.shape = Shape::Table;
        blocks.push(lectern);
        let mut with_book = def("lectern_book", "Lectern (With a Book)", Shaped, true, false, [T_LECTERN_BOOK_TOP, T_LECTERN_SIDE, T_PLANKS], 2.5, 0, false, LECTERN, 0.0, S_WOOD);
        with_book.shape = Shape::Table;
        with_book.creative = false;
        blocks.push(with_book);
        blocks.push(def("banner", "Banner (Flag-Adjacent)", Empty, false, false, [T_BANNER_ICON; 3], 1.0, 0, false, AIR, 0.0, S_GRASS));
        blocks.push(def("loom", "Loom (Pattern Machine)", Cube, true, true, [T_LOOM_TOP, T_LOOM_SIDE, T_PLANKS], 2.5, 0, false, LOOM, 0.0, S_WOOD));
        blocks.push(def("personal_chest", "Personal Chest (Yours Alone)", Cube, true, true, [T_PERSONAL_CHEST_TOP, T_PERSONAL_CHEST_SIDE, T_PERSONAL_CHEST_TOP], 22.0, 1, true, PERSONAL_CHEST, 7.0, S_STONE));
        blocks.push(def("copper_chest", "Copper Chest (Sorted, Hopefully)", Cube, true, true, [T_COPPER_CHEST_TOP, T_COPPER_CHEST_SIDE, T_COPPER_CHEST_TOP], 3.0, 0, false, COPPER_CHEST, 0.0, S_STONE));
        blocks.push(def("eyeblossom", "Eyeblossom (Pretending to Sleep)", Cross, false, false, [T_EYEBLOSSOM; 3], 0.0, 0, false, EYEBLOSSOM, 0.0, S_GRASS));
        let mut open = def("eyeblossom_open", "Eyeblossom (Wide Awake)", Cross, false, false, [T_EYEBLOSSOM_OPEN; 3], 0.0, 0, false, EYEBLOSSOM, 3.0, S_GRASS);
        open.creative = false;
        blocks.push(open);
        blocks.push(def("resin_block", "Block of Resin (Sticky)", Cube, true, true, [T_RESIN_BLOCK; 3], 0.5, 0, false, RESIN_BLOCK, 0.0, S_GRASS));
        blocks.push(def("resin_bricks", "Resin Bricks (Amber Masonry)", Cube, true, true, [T_RESIN_BRICKS; 3], 1.5, 1, true, RESIN_BRICKS, 0.0, S_STONE));
        blocks.push(def("firefly_bush", "Firefly Bush (Twinkly After Dark)", Cross, false, false, [T_FIREFLY_BUSH; 3], 0.0, 0, false, FIREFLY_BUSH, 2.0, S_GRASS));
        let mut litter = def("leaf_litter", "Leaf Litter (Crunchy)", Shaped, false, false, [T_LEAF_LITTER; 3], 0.0, 0, false, LEAF_LITTER, 0.0, S_GRASS);
        litter.shape = Shape::Dust;
        blocks.push(litter);
        let mut wild = def("wildflowers", "Wildflowers (Yellow Confetti)", Shaped, false, false, [T_WILDFLOWERS; 3], 0.0, 0, false, WILDFLOWERS, 0.0, S_GRASS);
        wild.shape = Shape::Dust;
        blocks.push(wild);
        let mut dried = def("dried_floaty", "Dried Floaty (Just Add Water)", Shaped, true, false, [T_DRIED_FLOATY; 3], 0.5, 0, false, DRIED_FLOATY, 0.0, S_GRASS);
        dried.see_through = true;
        dried.shape = Shape::Table;
        blocks.push(dried);
        let mut wet = def("mangrove_roots_wet", "Mangrove Roots (Waterlogged)", Cube, true, false, [T_MANGROVE_ROOTS; 3], 0.7, 0, false, MANGROVE_ROOTS, 0.0, S_WOOD);
        wet.see_through = true;
        wet.creative = false;
        blocks.push(wet);
        let mut fire = def("campfire", "Campfire (Marshmallows Not Included)", Shaped, false, false, [T_CAMPFIRE_TOP, T_CAMPFIRE_SIDE, T_LOG_TOP], 2.0, 0, false, CAMPFIRE, 15.0, S_WOOD);
        fire.shape = Shape::Campfire;
        blocks.push(fire);
        for (key, name, tex, tile_lit, lit_name) in [
            ("smoker", "Smoker (Food, Fast)", [T_SMOKER_TOP, T_SMOKER_SIDE, T_SMOKER_TOP], T_SMOKER_LIT, "Smoker (Smoking)"),
            ("blast_furnace", "Blast Furnace (Everything but Food, Fast)", [T_BLAST_TOP, T_BLAST_SIDE, T_BLAST_TOP], T_BLAST_LIT, "Blast Furnace (Blasting)"),
        ] {
            let id = blocks.len() as Id;
            blocks.push(def(key, name, Cube, true, true, tex, 3.5, 1, true, id, 0.0, S_STONE));
            let mut lit = def(leak(&format!("{key}_lit")), lit_name, Cube, true, true, [tex[0], tile_lit, tex[2]], 3.5, 1, true, id, 13.0, S_STONE);
            lit.creative = false;
            blocks.push(lit);
        }
        blocks.push(def("barrel", "Barrel (A Chest, but Round)", Cube, true, true, [T_BARREL_TOP, T_BARREL_SIDE, T_BARREL_TOP], 2.5, 0, false, BARREL, 0.0, S_WOOD));
        let mut egg = def("turtle_egg", "Turtle Eggs (Do Not Step On)", Shaped, false, false, [T_TURTLE_EGG; 3], 0.5, 0, false, AIR, 0.0, S_SAND);
        egg.shape = Shape::Button { down: false };
        blocks.push(egg);
        for facing in 0..4u8 {
            let mut d = def(leak(&format!("painting{}", ["", "_east", "_south", "_west"][facing as usize])), "Painting (Original Art, Probably)", Shaped, false, false, [T_PAINTING_FIRST; 3], 0.4, 0, false, PAINTING_FIRST, 0.0, S_WOOD);
            d.shape = Shape::Painting { facing };
            d.creative = facing == 0;
            blocks.push(d);
        }
        for facing in 0..4u8 {
            let mut d = def(leak(&format!("armour_stand{}", ["", "_east", "_south", "_west"][facing as usize])), "Armour Stand (Dressed to Impress)", Shaped, false, false, [T_PLANKS; 3], 1.0, 0, false, ARMOUR_STAND_FIRST, 0.0, S_WOOD);
            d.shape = Shape::Stand { facing };
            d.creative = facing == 0;
            blocks.push(d);
        }
        for c in 0..8u16 {
            let (key, name) = crate::carpentry::COLOURS[c as usize];
            let id = blocks.len() as Id;
            blocks.push(def(leak(&format!("{key}_concrete")), leak(&format!("{name} Concrete")), Cube, true, true, [T_CONCRETE + c; 3], 1.8, 1, true, id, 0.0, S_STONE));
        }
        for c in 0..8u16 {
            let (key, name) = crate::carpentry::COLOURS[c as usize];
            let id = blocks.len() as Id;
            blocks.push(def(leak(&format!("{key}_concrete_powder")), leak(&format!("{name} Concrete Powder (Just Add Water)")), Cube, true, true, [T_CONCRETE_POWDER + c; 3], 0.5, 0, false, id, 0.0, S_SAND));
        }
        for c in 0..8u16 {
            let (key, name) = crate::carpentry::COLOURS[c as usize];
            let id = blocks.len() as Id;
            blocks.push(def(leak(&format!("{key}_glazed_terracotta")), leak(&format!("{name} Glazed Terracotta")), Cube, true, true, [T_GLAZED + c; 3], 1.4, 1, true, id, 0.0, S_STONE));
        }
        let mut candle = def("candle", "Candle (Romantic, Probably)", Shaped, false, false, [T_CANDLE; 3], 0.1, 0, false, CANDLE, 0.0, S_GRASS);
        candle.shape = Shape::Candle;
        blocks.push(candle);
        let mut lit = def("candle_lit", "Candle (Lit)", Shaped, false, false, [T_CANDLE_LIT; 3], 0.1, 0, false, CANDLE, 6.0, S_GRASS);
        lit.shape = Shape::Candle;
        lit.creative = false;
        blocks.push(lit);
        let mut chain = def("chain", "Chain (Strong Links)", Shaped, false, false, [T_CHAIN; 3], 2.5, 1, true, CHAIN, 0.0, S_STONE);
        chain.shape = Shape::Chain;
        blocks.push(chain);
        let mut scaffold = def("scaffolding", "Scaffolding (Climb Inside It)", Shaped, false, false, [T_SCAFFOLD_TOP, T_SCAFFOLD_SIDE, T_SCAFFOLD_TOP], 0.1, 0, false, SCAFFOLDING, 0.0, S_WOOD);
        scaffold.shape = Shape::Scaffold;
        blocks.push(scaffold);
        let mut hanging = def("lantern_hanging", "Lantern (Hanging)", Shaped, false, false, [T_LANTERN; 3], 0.8, 0, false, LANTERN, 14.0, S_GLASS);
        hanging.shape = Shape::Hanging;
        hanging.creative = false;
        blocks.push(hanging);
        for key in ["wall_torch_north", "wall_torch_east", "wall_torch_south", "wall_torch_west"] {
            let mut t = def(key, "Torch", Cross, false, false, [T_TORCH; 3], 0.0, 0, false, TORCH, 8.0, S_WOOD);
            t.creative = false;
            blocks.push(t);
        }
        debug_assert_eq!(blocks.len(), NUM_BLOCKS as usize);
        debug_assert_eq!(blocks[NOTE_BLOCK as usize].key, "note_block");
        debug_assert_eq!(blocks[JUKEBOX as usize].key, "jukebox");
        debug_assert_eq!(blocks[BELL as usize].key, "bell");
        debug_assert_eq!(blocks[HOLLOW_BOX as usize].key, "hollow_box");
        debug_assert_eq!(blocks[FROGLIGHT_FIRST as usize].key, "ochre_froglight");
        debug_assert_eq!(blocks[DEEPSLATE as usize].key, "deepslate");
        debug_assert_eq!(blocks[SUSPICIOUS_SAND as usize].key, "suspicious_sand");
        debug_assert_eq!(blocks[GRINDSTONE as usize].key, "grindstone");
        debug_assert_eq!(blocks[WAXED_COPPER_FIRST as usize].key, "waxed_copper_block");
        debug_assert_eq!(blocks[BAMBOO_STAIRS as usize].key, "bamboo_stairs");
        debug_assert_eq!(blocks[DEAD_CORAL as usize].key, "dead_coral_block");
        debug_assert_eq!(blocks[BEE_NEST as usize].key, "bee_nest");
        debug_assert_eq!(blocks[TORCHFLOWER as usize].key, "torchflower");
        debug_assert_eq!(blocks[POWERED_RAIL as usize].key, "powered_rail");
        debug_assert_eq!(blocks[SPRUCE_LOG as usize].key, "spruce_log");
        debug_assert_eq!(blocks[MELON as usize].key, "melon");
        debug_assert_eq!(blocks[DETECTOR_RAIL as usize].key, "detector_rail");
        debug_assert_eq!(blocks[COMPARATOR_FIRST as usize].key, "comparator");
        debug_assert_eq!(blocks[BEACON_FIRST as usize].key, "beacon");
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
        items.push(item("staring_eye", "Staring Eye (It Knows)", T_STARING_EYE));
        items.push(item("name_tag", "Name Tag (Hello, My Name Is)", T_NAME_TAG));
        items.push(ItemDef { food: Some(2.0), ..item("melon_slice", "Melon Slice (Mostly Water)", T_MELON_SLICE) });
        items.push(ItemDef { stack: 1, ..item("chest_minecart", "Minecart with Chest (Freight)", T_CHEST_CART_ITEM) });
        items.push(ItemDef { stack: 1, ..item("hopper_minecart", "Minecart with Hopper (Vacuum)", T_HOPPER_CART_ITEM) });
        items.push(ItemDef { stack: 1, ..item("glider", "Glider (Wings, Basically)", T_GLIDER) });
        items.push(item("rocket", "Boom Rocket (Glide Faster)", T_ROCKET));
        items.push(ItemDef { stack: 1, damage: 9.0, ..item("spear", "Soggy Spear (Pointy, Throwable)", T_SPEAR) });
        items.push(item("copper_ingot", "Copper Ingot (Penny-Adjacent)", T_COPPER_INGOT));
        for (t, (k, n)) in crate::tools::TIER_NAMES.iter().enumerate() {
            let damage = [3.5, 4.5, 5.0, 5.5, 6.5][t];
            items.push(ItemDef { stack: 1, damage, ..item(leak(&format!("{k}_axe")), leak(&format!("{n} Axe (Chop Chop)")), T_AXE0 + t as u16) });
        }
        for (t, (k, n)) in crate::tools::TIER_NAMES.iter().enumerate() {
            let damage = [2.5, 3.5, 4.0, 4.5, 5.5][t];
            items.push(ItemDef { stack: 1, damage, ..item(leak(&format!("{k}_shovel")), leak(&format!("{n} Shovel (Dig It)")), T_SHOVEL0 + t as u16) });
        }
        items.push(ItemDef { stack: 1, pick_tier: 2, damage: 3.5, ..item("copper_pickaxe", "Copper Pickaxe (Between Stone and Iron)", T_PICK_COPPER) });
        items.push(ItemDef { stack: 1, damage: 5.5, ..item("copper_sword", "Copper Sword (Tarnishes Beautifully)", T_SWORD_COPPER) });
        for (slot, name) in ["Copper Helmet (Penny Hat)", "Copper Chestplate (Very Conductive)", "Copper Leggings (Clanky)", "Copper Boots (Squeaky)"].into_iter().enumerate() {
            let key = ["copper_helmet", "copper_chestplate", "copper_leggings", "copper_boots"][slot];
            items.push(ItemDef { stack: 1, ..item(key, name, T_COPPER_ARMOR_ITEMS + slot as u16) });
        }
        for (i, f) in crate::bees::Flavour::ALL.iter().enumerate() {
            let key = leak(&format!("{}_honey_bottle", f.key()));
            let name = leak(&format!("{} Honey (Bottled)", f.name()));
            items.push(ItemDef { stack: 16, food: Some(6.0), ..item(key, name, T_HONEY_FIRST + i as u16) });
        }
        items.push(item("honeycomb", "Honeycomb (Hexagonal, Efficient)", T_HONEYCOMB));
        items.push(ItemDef { stack: 1, ..item("bee_smoker", "Bee Smoker (Calm Down, Bees)", T_SMOKER) });
        items.push(ItemDef { stack: 1, ..item("hive_tool", "Hive Tool (Flat Crowbar of Knowledge)", T_HIVE_TOOL) });
        items.push(ItemDef { stack: 1, ..item("queen_bee", "Queen Bee (In a Jar, Unimpressed)", T_QUEEN) });
        items.push(item("torchflower_seeds", "Torchflower Seeds (Older Than Dirt)", T_TORCH_SEEDS));
        items.push(item("scute", "Rollo Scute (Shed With Dignity)", T_SCUTE));
        items.push(ItemDef { stack: 1, ..item("wolf_armor", "Woofer Armour (Very Good Boy Protection)", T_WOLF_ARMOR_ITEM) });
        items.push(ItemDef { stack: 1, ..item("brush", "Brush (For Dusting History)", T_BRUSH) });
        items.push(ItemDef { stack: 1, ..item("diamond_brush", "Dimond Brush (Very Gentle, Very Expensive)", T_DIAMOND_BRUSH) });
        items.push(ItemDef { stack: 1, ..item("field_journal", "Field Journal (Mostly Sketches)", T_JOURNAL) });
        for (i, (key, name, _)) in crate::archaeology::SHARDS.iter().enumerate() {
            items.push(item(leak(&format!("{key}_pottery_shard")), leak(&format!("Pottery Shard ({name})")), T_SHARD_FIRST + i as u16));
        }
        for (i, (key, name, _)) in crate::archaeology::RELICS.iter().enumerate() {
            items.push(ItemDef { stack: 1, ..item(key, name, T_RELIC_FIRST + i as u16) });
        }
        items.push(ItemDef { stack: 1, ..item("encrusted_relic", "Encrusted Relic (Something's In There)", T_ENCRUSTED) });
        items.push(ItemDef { stack: 1, ..item("map_fragment", "Map Fragment (Torn, Promising)", T_MAP_FRAGMENT) });
        items.push(item("ancient_coin", "Ancient Coin (Legal Tender, Once)", T_COIN));
        items.push(ItemDef { stack: 1, ..item("clay_tablet", "Clay Tablet (Someone's Diary)", T_TABLET) });
        items.push(item("echo_shard", "Echo Shard (Hums Faintly)", T_ECHO_SHARD));
        items.push(item("recovery_compass", "Recovery Compass (Points to Your Last Mistake)", T_RECOVERY_COMPASS));
        items.push(item("scorchite_scrap", "Scorchite Scrap (Hot Off the Debris)", T_SCRAP));
        items.push(item("scorchite_ingot", "Scorchite Ingot (Heavier Than It Looks)", T_SCORCHITE_INGOT));
        items.push(item("upgrade_template", "Scorchite Upgrade Template (Some Assembly Required)", T_TEMPLATE));
        items.push(ItemDef { stack: 1, pick_tier: 4, damage: 6.0, ..item("scorchite_pickaxe", "Scorchite Pickaxe (Overkill)", T_SCORCHITE_TOOLS) });
        items.push(ItemDef { stack: 1, damage: 8.0, ..item("scorchite_sword", "Scorchite Sword (Unreasonably Sharp)", T_SCORCHITE_TOOLS + 1) });
        items.push(ItemDef { stack: 1, damage: 7.5, ..item("scorchite_axe", "Scorchite Axe (Trees Fear It)", T_SCORCHITE_TOOLS + 2) });
        items.push(ItemDef { stack: 1, damage: 6.5, ..item("scorchite_shovel", "Scorchite Shovel (Digs Holes in Holes)", T_SCORCHITE_TOOLS + 3) });
        for (slot, name) in ["Scorchite Helmet (Hot Headed)", "Scorchite Chestplate (Very Chesty)", "Scorchite Leggings (Legendary Legs)", "Scorchite Boots (Lava Optional)"].into_iter().enumerate() {
            let key = ["scorchite_helmet", "scorchite_chestplate", "scorchite_leggings", "scorchite_boots"][slot];
            items.push(ItemDef { stack: 1, ..item(key, name, T_SCORCHITE_ARMOR_ITEMS + slot as u16) });
        }
        for (i, (key, title)) in crate::songs::DISCS.into_iter().enumerate() {
            items.push(ItemDef { stack: 1, ..item(key, leak(&format!("Music Disc ({title})")), T_DISC_FIRST + i as u16) });
        }
        items.push(item("sizzle_rod", "Sizzle Rod (Still Warm)", T_SIZZLE_ROD));
        items.push(item("sizzle_powder", "Sizzle Powder (Do Not Sniff)", T_SIZZLE_POWDER));
        items.push(item("weeper_tear", "Weeper Tear (It Was Sad)", T_WEEPER_TEAR));
        items.push(ItemDef { stack: 1, ..item("shroom_on_a_stick", "Ember Shroom on a Stick (Steering Snack)", T_SHROOM_STICK) });
        for (i, (key, name)) in [("potion_strength", "Potion of Strength (Flex)"), ("potion_regeneration", "Potion of Regeneration (Get Well Soon)")].into_iter().enumerate() {
            items.push(ItemDef { stack: 1, ..item(key, name, T_POTION_EXTRA + i as u16) });
        }
        for (i, (key, name)) in [("splash_potion_strength", "Splash Potion of Strength (Flex for Everyone)"), ("splash_potion_regeneration", "Splash Potion of Regeneration (Group Hug)")].into_iter().enumerate() {
            items.push(ItemDef { stack: 1, ..item(key, name, T_SPLASH_EXTRA + i as u16) });
        }
        items.push(ItemDef { stack: 1, damage: 3.0, ..item("crossbow", "Crossbow (Pre-Loaded Opinions)", T_CROSSBOW) });
        items.push(ItemDef { stack: 1, ..item("totem_of_undying", "Totem of Not Dying (Once)", T_TOTEM) });
        items.push(item("ominous_banner", "Ominous Banner (Looks Important)", T_BANNER));
        items.push(item("trial_key", "Trial Key (Opens Exactly One Vault)", T_TRIAL_KEY));
        items.push(ItemDef { stack: 16, ..item("wind_charge", "Wind Charge (Bottled Gust)", T_WIND_CHARGE) });
        items.push(item("breeze_rod", "Breeze Rod (Whooshy Stick)", T_BREEZE_ROD));
        items.push(ItemDef { stack: 1, consume: false, ..item("goat_horn", "Goat Horn (Very Loud)", T_GOAT_HORN) });
        for (key, name) in [("coast_trim", "Coast Trim Template"), ("wild_trim", "Wild Trim Template"), ("ward_trim", "Ward Trim Template"), ("spire_trim", "Spire Trim Template")] {
            items.push(item(key, name, T_TRIM_TEMPLATE));
        }
        items.push(ItemDef { stack: 1, consume: false, ..item("spyglass", "Spyglass (Pirate Approved)", T_SPYGLASS) });
        items.push(ItemDef { stack: 1, consume: false, ..item("bundle", "Bundle (Bag of Bits)", T_BUNDLE) });
        items.push(item("ominous_trial_key", "Ominous Trial Key (Opens an Ominous Vault)", T_OMINOUS_KEY));
        items.push(ItemDef { stack: 1, damage: 6.0, durability: Some(500), ..item("mace", "Mace (Gravity Assisted)", T_MACE) });
        items.push(ItemDef { stack: 16, ..item("ominous_bottle", "Ominous Bottle (Tastes Foreboding)", T_OMINOUS_BOTTLE) });
        items.push(item("pitcher_pod", "Pitcher Pod (Very Old Seed)", T_PITCHER_POD));
        items.push(ItemDef { stack: 1, consume: false, ..item("writable_book", "Book and Quill (Blank, Full of Promise)", T_BOOK_QUILL) });
        items.push(ItemDef { stack: 1, consume: false, ..item("written_book", "Written Book (Signed and Everything)", T_WRITTEN_BOOK) });
        items.push(item("resin_clump", "Resin Clump (Tree Gum, Basically)", T_RESIN_CLUMP));
        items.push(item("resin_brick", "Resin Brick (Amber, Tiny)", T_RESIN_BRICK));
        items.push(ItemDef { stack: 1, ..item("harness", "Harness (Floaty-Sized)", T_HARNESS) });
        for (i, (key, name, damage)) in [
            ("wooden_spear", "Wooden Spear (Pointy Stick, Promoted)", 4.0),
            ("stone_spear", "Stone Spear (Flint-ish)", 5.0),
            ("iron_spear", "Iron Spear (Jousting Optional)", 6.5),
            ("dimond_spear", "Dimond Spear (Very Pointy)", 8.0),
        ]
        .into_iter()
        .enumerate()
        {
            items.push(ItemDef { stack: 1, damage, ..item(key, name, T_SPEAR_FIRST + i as u16) });
        }
        items.push(ItemDef { stack: 1, consume: false, ..item("treasure_map", "Treasure Map (X Marks the Spot)", T_TREASURE_MAP) });
        items.push(item("turtle_scute", "Turtle Scute (Shell Shard)", T_TURTLE_SCUTE));
        items.push(ItemDef { stack: 1, durability: Some(275), armor: Some(ModArmor { slot: 0, points: 2, looks_like: TURTLE_TIER as u8 }), repair: TURTLE_SCUTE, ..item("turtle_shell", "Turtle Shell (Clear Sight Underwater)", T_TURTLE_SHELL) });
        debug_assert_eq!(items.len(), (FIRST_MOD_ITEM - FIRST_ITEM) as usize);

        let r = |inputs: &[(Id, u8)], output: (Id, u8)| Recipe { inputs: inputs.to_vec(), output };
        let recipes = vec![
            r(&[(LOG, 1)], (PLANKS, 4)),
            r(&[(SPRUCE_LOG, 1)], (PLANKS, 4)),
            r(&[(JUNGLE_LOG, 1)], (PLANKS, 4)),
            r(&[(CHERRY_LOG, 1)], (CHERRY_PLANKS, 4)),
            r(&[(PALE_OAK_LOG, 1)], (PALE_OAK_PLANKS, 4)),
            r(&[(PALE_OAK_PLANKS, 1)], (PLANKS, 1)),
            r(&[(MANGROVE_LOG, 1)], (MANGROVE_PLANKS, 4)),
            // The new woods do anything plain planks do, once you've made them plain.
            r(&[(CHERRY_PLANKS, 1)], (PLANKS, 1)),
            r(&[(MANGROVE_PLANKS, 1)], (PLANKS, 1)),
            r(&[(MELON_SLICE, 9)], (MELON, 1)),
            r(&[(MINECART, 1), (CHEST, 1)], (CHEST_MINECART, 1)),
            r(&[(MINECART, 1), (HOPPER_FIRST, 1)], (HOPPER_MINECART, 1)),
            r(&[(IRON, 6), (PLATE, 1), (ZAP_DUST, 1)], (DETECTOR_RAIL, 6)),
            r(&[(STONE, 3), (ZTORCH_ON, 3), (GOLD_INGOT, 1)], (COMPARATOR_FIRST, 1)),
            r(&[(WYRM_EGG, 1), (GLASS, 5), (OBSIDIAN, 3)], (BEACON_FIRST, 1)),
            r(&[(MUD, 4), (WHEAT, 1)], (BRICK, 4)),
            r(&[(RED_SAND, 4)], (SANDSTONE, 1)),
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
            r(&[(COBBLE, 6), (ZAP_DUST, 2), (GLASS, 1)], (OBSERVER_FIRST, 1)),
            r(&[(IRON, 5), (TABLE, 1), (ZAP_DUST, 2), (DISPENSER_FIRST, 1)], (CRAFTER_FIRST, 1)),
            r(&[(COPPER_INGOT, 4), (ZAP_DUST, 1), (TORCH, 1)], (COPPER_BULB, 1)),
            r(&[(COPPER_INGOT, 4)], (COPPER_GRATE, 4)),
            r(&[(STONE_BRICKS, 4), (COBBLE, 1)], (TUFF_BRICKS, 4)),
            r(&[(TUFF_BRICKS, 2)], (CHISELED_TUFF, 1)),
            r(&[(BREEZE_ROD, 1)], (WIND_CHARGE, 4)),
            r(&[(STONE_BRICKS, 8), (IRON, 1)], (LODESTONE, 1)),
            r(&[(GLASS, 1), (COPPER_INGOT, 2)], (SPYGLASS, 1)),
            r(&[(HEAVY_CORE, 1), (BREEZE_ROD, 1)], (MACE, 1)),
            r(&[(BOOK, 1), (FEATHER, 1), (COAL, 1)], (BOOK_AND_QUILL, 1)),
            r(&[(PLANKS, 4), (BOOKSHELF, 1)], (LECTERN, 1)),
            r(&[(WOOL, 6), (STICK, 1)], (BANNER, 1)),
            r(&[(PLANKS, 2), (STRING, 2)], (LOOM, 1)),
            r(&[(OBSIDIAN, 8), (STARING_EYE, 1)], (PERSONAL_CHEST, 1)),
            r(&[(CHEST, 1), (COPPER_INGOT, 4)], (COPPER_CHEST, 1)),
            r(&[(RESIN_CLUMP, 9)], (RESIN_BLOCK, 1)),
            r(&[(RESIN_BLOCK, 1)], (RESIN_CLUMP, 9)),
            r(&[(RESIN_BRICK, 4)], (RESIN_BRICKS, 1)),
            r(&[(WOOL, 3), (STRING, 2), (GLASS, 2)], (HARNESS, 1)),
            r(&[(STICK, 2), (PLANKS, 1)], (SPEAR_FIRST, 1)),
            r(&[(STICK, 2), (COBBLE, 1)], (SPEAR_FIRST + 1, 1)),
            r(&[(STICK, 2), (IRON, 1)], (SPEAR_FIRST + 2, 1)),
            r(&[(STICK, 2), (DIAMOND, 1)], (SPEAR_FIRST + 3, 1)),
            r(&[(STICK, 3), (COAL, 1), (LOG, 3)], (CAMPFIRE, 1)),
            r(&[(FURNACE, 1), (LOG, 4)], (SMOKER, 1)),
            r(&[(FURNACE, 1), (IRON, 5), (STONE, 3)], (BLAST_FURNACE, 1)),
            r(&[(PLANKS, 7)], (BARREL, 1)),
            r(&[(STICK, 8), (WOOL, 2)], (PAINTING_FIRST, 1)),
            r(&[(STICK, 6), (STONE, 1)], (ARMOUR_STAND_FIRST, 1)),
            r(&[(TURTLE_SCUTE, 5)], (TURTLE_SHELL, 1)),
            r(&[(STRING, 1), (HONEYCOMB, 1)], (CANDLE, 2)),
            r(&[(IRON, 3)], (CHAIN, 2)),
            r(&[(BAMBOO, 6), (STRING, 1)], (SCAFFOLDING, 6)),
            r(&[(STRING, 2), (WOOL, 1)], (BUNDLE, 1)),
            r(&[(IRON, 5), (CHEST, 1)], (HOPPER_FIRST, 1)),
            r(&[(WOOL, 3), (IRON, 1), (STRING, 2)], (SADDLE, 1)),
            r(&[(PEARL, 1), (EMBER_SHROOM, 1)], (STARING_EYE, 1)),
            r(&[(STRING, 1), (BOOK, 1)], (NAME_TAG, 2)),
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
            recipes.push(r(&[(SAND, 4), (GRAVEL, 4), (DYE_FIRST + c, 1)], (CONCRETE_POWDER_FIRST + c, 8)));
        }
        for (m, (full, _)) in MATERIALS.iter().enumerate() {
            recipes.push(r(&[(*full, 3)], (slab(m, false), 6)));
            recipes.push(r(&[(*full, 6)], (stairs(m, 0), 4)));
        }
        // Axes and shovels of every tier, and copper everything.
        for (t, material) in [PLANKS, COBBLE, COPPER_INGOT, IRON, DIAMOND].into_iter().enumerate() {
            recipes.push(r(&[(material, 3), (STICK, 2)], (AXE_FIRST + t as Id, 1)));
            recipes.push(r(&[(material, 1), (STICK, 2)], (SHOVEL_FIRST + t as Id, 1)));
        }
        recipes.push(r(&[(COPPER_INGOT, 3), (STICK, 2)], (PICK_COPPER, 1)));
        recipes.push(r(&[(COPPER_INGOT, 2), (STICK, 1)], (SWORD_COPPER, 1)));
        for (slot, n) in [5, 8, 7, 4].into_iter().enumerate() {
            recipes.push(r(&[(COPPER_INGOT, n)], (COPPER_ARMOR_FIRST + slot as Id, 1)));
        }
        // Gliding, boxes, copper and bamboo.
        recipes.push(r(&[(GUNPOWDER, 1), (STRING, 1)], (ROCKET, 3)));
        recipes.push(r(&[(CHEST, 1), (HOLLOW_STONE, 4)], (HOLLOW_BOX, 1)));
        recipes.push(r(&[(COPPER_INGOT, 9)], (COPPER_FIRST, 1)));
        recipes.push(r(&[(COPPER_FIRST, 1)], (COPPER_INGOT, 9)));
        for i in 0..4 {
            recipes.push(r(&[(COPPER_FIRST + i, 1), (GOO, 1)], (WAXED_COPPER_FIRST + i, 1)));
        }
        recipes.push(r(&[(BAMBOO, 9)], (BAMBOO_BLOCK, 1)));
        recipes.push(r(&[(BAMBOO_BLOCK, 1)], (BAMBOO_PLANKS, 2)));
        recipes.push(r(&[(BAMBOO, 2)], (STICK, 1)));
        recipes.push(r(&[(BAMBOO_PLANKS, 2)], (BAMBOO_MOSAIC, 1)));
        recipes.push(r(&[(BAMBOO_PLANKS, 3)], (BAMBOO_SLAB, 6)));
        recipes.push(r(&[(BAMBOO_PLANKS, 6)], (BAMBOO_STAIRS, 4)));
        recipes.push(r(&[(BAMBOO_PLANKS, 8)], (CHEST, 1)));
        // Beekeeping.
        recipes.push(r(&[(PLANKS, 6), (HONEYCOMB, 3)], (BEEHIVE, 1)));
        recipes.push(r(&[(PLANKS, 9)], (BEEHIVE, 1)));
        recipes.push(r(&[(IRON, 2), (STICK, 1), (WOOL, 1)], (BEE_SMOKER, 1)));
        recipes.push(r(&[(IRON, 1), (STICK, 1)], (HIVE_TOOL, 1)));
        for f in 0..5 {
            recipes.push(r(&[(HONEY_FIRST + f, 4)], (HONEY_BLOCK, 1)));
        }
        for i in 0..4 {
            recipes.push(r(&[(COPPER_FIRST + i, 1), (HONEYCOMB, 1)], (WAXED_COPPER_FIRST + i, 1)));
        }
        recipes.push(r(&[(DANDELION, 1)], (DYE_FIRST + 4, 2)));
        recipes.push(r(&[(CORNFLOWER, 1)], (DYE_FIRST + 6, 2)));
        recipes.push(r(&[(LAVENDER, 1)], (DYE_FIRST + 7, 2)));
        recipes.push(r(&[(SCUTE, 6)], (WOLF_ARMOR, 1)));
        // Archaeology, the Deep Dark, grinding and smithing.
        recipes.push(r(&[(COPPER_INGOT, 1), (FEATHER, 1), (STICK, 1)], (BRUSH, 1)));
        recipes.push(r(&[(DIAMOND, 1), (FEATHER, 2), (STICK, 1)], (DIAMOND_BRUSH, 1)));
        recipes.push(r(&[(BOOK, 1), (COAL, 1)], (FIELD_JOURNAL, 1)));
        recipes.push(r(&[(PLANKS, 4), (BRUSH, 1), (STONE_BRICKS, 2)], (RESTORATION_BENCH, 1)));
        recipes.push(r(&[(BRICK, 4)], (POT_FIRST, 1)));
        for i in 0..12 {
            recipes.push(r(&[(SHARD_FIRST + i, 1), (BRICK, 3)], (POT_FIRST + 1 + i, 1)));
        }
        recipes.push(r(&[(COBBLED_DEEPSLATE, 4)], (DEEPSLATE_BRICKS, 4)));
        recipes.push(r(&[(DEEPSLATE_BRICKS, 4)], (DEEPSLATE_TILES, 4)));
        recipes.push(r(&[(IRON, 1), (TORCH, 1), (ECHO_SHARD, 1)], (SOUL_LANTERN, 1)));
        recipes.push(r(&[(ECHO_SHARD, 8), (COMPASS, 1)], (RECOVERY_COMPASS, 1)));
        recipes.push(r(&[(STICK, 2), (STONE, 1), (PLANKS, 2)], (GRINDSTONE, 1)));
        recipes.push(r(&[(IRON, 2), (PLANKS, 4)], (SMITHING_TABLE, 1)));
        // Music, the Scorchlands, raids.
        recipes.push(r(&[(PLANKS, 8), (ZAP_DUST, 1)], (NOTE_BLOCK, 1)));
        recipes.push(r(&[(PLANKS, 8), (DIAMOND, 1)], (JUKEBOX, 1)));
        recipes.push(r(&[(SIZZLE_ROD, 1)], (SIZZLE_POWDER, 2)));
        recipes.push(r(&[(PEARL, 1), (SIZZLE_POWDER, 1)], (STARING_EYE, 2)));
        recipes.push(r(&[(ROD, 1), (EMBER_SHROOM, 1)], (SHROOM_STICK, 1)));
        recipes.push(r(&[(GOLD_INGOT, 9)], (GOLD_BLOCK, 1)));
        recipes.push(r(&[(GOLD_BLOCK, 1)], (GOLD_INGOT, 9)));
        recipes.push(r(&[(SCORCHROCK, 4)], (SCORCH_BRICKS, 2)));
        recipes.push(r(&[(GOLD_BLOCK, 1), (STICK, 2)], (BELL, 1)));
        recipes.push(r(&[(STICK, 3), (STRING, 2), (IRON, 1)], (CROSSBOW, 1)));
        recipes.push(r(&[(SCORCHITE_SCRAP, 4), (GOLD_INGOT, 4)], (SCORCHITE_INGOT, 1)));
        recipes.push(r(&[(DIAMOND, 7), (UPGRADE_TEMPLATE, 1), (COBBLED_DEEPSLATE, 1)], (UPGRADE_TEMPLATE, 2)));
        for t in TRIM_FIRST..TRIM_FIRST + TRIMS as Id {
            recipes.push(r(&[(DIAMOND, 7), (t, 1), (TUFF_BRICKS, 1)], (t, 2)));
        }

        // Minecraft's amounts: 5 for a helmet, 8 chestplate, 7 leggings, 4 boots.
        for (t, material) in [WOOL, IRON, GOLD_INGOT, DIAMOND].into_iter().enumerate() {
            for (slot, n) in [5, 8, 7, 4].into_iter().enumerate() {
                recipes.push(r(&[(material, n)], (ARMOR_FIRST + (t * 4 + slot) as Id, 1)));
            }
        }
        Registry { blocks, items, recipes, ores: Vec::new(), plants: Vec::new(), splashes: Vec::new(), mods: Vec::new(), textures: Vec::new(), smelting: Vec::new(), fuels: Vec::new(), mobs: Vec::new() }
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
    is_leaves(id)
}
/// Any kind of tree trunk.
#[inline]
pub fn is_log(id: Id) -> bool {
    matches!(id, LOG | SPRUCE_LOG | JUNGLE_LOG | CHERRY_LOG | MANGROVE_LOG | PALE_OAK_LOG)
}
/// Any kind of leaves.
#[inline]
pub fn is_leaves(id: Id) -> bool {
    matches!(id, LEAVES | SPRUCE_LEAVES | JUNGLE_LEAVES | CHERRY_LEAVES | MANGROVE_LEAVES | PALE_OAK_LEAVES)
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

/// Any spear: the Soggy Spear and the craftable ones. All of them throw, and
/// hit harder from a moving mount (see combat.rs).
pub fn is_spear(id: Id) -> bool {
    id == SPEAR || (SPEAR_FIRST..SPEAR_FIRST + SPEARS as Id).contains(&id)
}

pub fn max_stack(id: Id) -> u8 {
    // Each Hollow Box carries its own contents.
    // (So does each Banner its design.)
    if id == HOLLOW_BOX || id == BANNER {
        return 1;
    }
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
    let bonus = if efficiency > 0 { (efficiency as f32).powi(2) + 1.0 } else { 0.0 };
    if !b.pick_block {
        // Axes for wood, shovels for earth (see tools.rs).
        return match crate::tools::best_tool(id).and_then(|kind| crate::tools::speed(held, kind)) {
            Some(speed) => (b.hardness * 1.5 / (speed + bonus), true),
            None => (b.hardness, true),
        };
    }
    let tier = pick_tier(held);
    let Some(speed) = crate::tools::speed(held, crate::tools::Tool::Pick) else {
        return (b.hardness * 5.0, b.pick_tier == 0);
    };
    (b.hardness * 1.5 / (speed + bonus), tier >= b.pick_tier)
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
        // This test reads the installed registry's exact shape, so it must not
        // run while a gear/furnace test has a mod registry installed.
        let _guard = crate::mods::registry_test_lock();
        for (id, key) in [
            (OBSIDIAN, "obsidian"),
            (ZAP_ORE, "zap_ore"),
            (LAMP_ON, "zap_lamp_on"),
            (PORTAL_X, "portal"),
            (RAIL_FIRST, "rail"),
            (POWERED_RAIL, "powered_rail"),
            (SIGN_FIRST, "sign"),
            (FRAME_FIRST, "item_frame"),
            (SAPLING, "sapling"),
            (FIRE, "fire"),
            (BREWING_STAND, "brewing_stand"),
            (HOPPER_FIRST, "hopper"),
            (WYRM_EGG, "wyrm_egg"),
            (SPRUCE_LOG, "spruce_log"),
            (SPRUCE_LEAVES, "spruce_leaves"),
            (JUNGLE_LOG, "jungle_log"),
            (JUNGLE_LEAVES, "jungle_leaves"),
            (MUD, "mud"),
            (LILY_PAD, "lily_pad"),
            (RED_SAND, "red_sand"),
            (TERRACOTTA, "terracotta"),
            (DEAD_BUSH, "dead_bush"),
            (MELON, "melon"),
            (DETECTOR_RAIL, "detector_rail"),
            (COMPARATOR_FIRST, "comparator"),
            (BEACON_FIRST, "beacon"),
        ] {
            assert_eq!(block(id).key, key, "id {id}");
        }
        assert_eq!(reg().blocks.len(), NUM_BLOCKS as usize);
        for (id, key) in [(MELON_SLICE, "melon_slice"), (CHEST_MINECART, "chest_minecart"), (HOPPER_MINECART, "hopper_minecart"), (NAME_TAG, "name_tag")] {
            assert_eq!(reg().key_of(id), key, "item {id}");
        }
        // The newest blocks and items, where their ids say.
        for (id, key) in [
            (OBSERVER_FIRST, "observer"),
            (COPPER_BULB, "copper_bulb"),
            (TUFF_BRICKS, "tuff_bricks"),
            (TRIAL_SPAWNER, "trial_spawner"),
            (VAULT, "vault"),
            (LODESTONE, "lodestone"),
            (TRIAL_SPAWNER_OMINOUS, "ominous_trial_spawner"),
            (VAULT_OMINOUS, "ominous_vault"),
            (HEAVY_CORE, "heavy_core"),
            (PALE_OAK_LOG, "pale_oak_log"),
            (PALE_HANGING_MOSS, "pale_hanging_moss"),
            (CREAKING_HEART, "creaking_heart"),
            (CREAKING_HEART_AWAKE, "creaking_heart_awake"),
            (SNIFFER_EGG, "sniffer_egg"),
            (PITCHER_CROP, "pitcher_crop"),
            (PITCHER_PLANT, "pitcher_plant"),
            (LECTERN, "lectern"),
            (LECTERN_BOOK, "lectern_book"),
            (BANNER, "banner"),
            (LOOM, "loom"),
            (PERSONAL_CHEST, "personal_chest"),
            (COPPER_CHEST, "copper_chest"),
            (EYEBLOSSOM_OPEN, "eyeblossom_open"),
            (RESIN_BRICKS, "resin_bricks"),
            (FIREFLY_BUSH, "firefly_bush"),
            (WILDFLOWERS, "wildflowers"),
            (DRIED_FLOATY, "dried_floaty"),
            (MANGROVE_ROOTS_WET, "mangrove_roots_wet"),
            (CAMPFIRE, "campfire"),
            (SMOKER_LIT, "smoker_lit"),
            (BLAST_FURNACE, "blast_furnace"),
            (BARREL, "barrel"),
            (TURTLE_EGG, "turtle_egg"),
            (PAINTING_FIRST + 3, "painting_west"),
            (ARMOUR_STAND_FIRST, "armour_stand"),
            (CONCRETE_FIRST, "white_concrete"),
            (CONCRETE_POWDER_FIRST + 7, "purple_concrete_powder"),
            (GLAZED_FIRST + 2, "red_glazed_terracotta"),
            (CANDLE_LIT, "candle_lit"),
            (SCAFFOLDING, "scaffolding"),
            (LANTERN_HANGING, "lantern_hanging"),
            (WALL_TORCH_FIRST + 3, "wall_torch_west"),
        ] {
            assert_eq!(block(id).key, key, "id {id}");
        }
        for (id, key) in [
            (TRIAL_KEY, "trial_key"),
            (WIND_CHARGE, "wind_charge"),
            (TRIM_FIRST, "coast_trim"),
            (SPYGLASS, "spyglass"),
            (BUNDLE, "bundle"),
            (OMINOUS_TRIAL_KEY, "ominous_trial_key"),
            (MACE, "mace"),
            (OMINOUS_BOTTLE, "ominous_bottle"),
            (PITCHER_POD, "pitcher_pod"),
            (BOOK_AND_QUILL, "writable_book"),
            (WRITTEN_BOOK, "written_book"),
            (RESIN_CLUMP, "resin_clump"),
            (HARNESS, "harness"),
            (SPEAR_FIRST, "wooden_spear"),
            (SPEAR_FIRST + 3, "dimond_spear"),
            (TREASURE_MAP, "treasure_map"),
            (TURTLE_SHELL, "turtle_shell"),
        ] {
            assert_eq!(reg().key_of(id), key, "item {id}");
        }
    }
}
