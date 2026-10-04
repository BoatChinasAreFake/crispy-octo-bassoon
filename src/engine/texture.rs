//! Procedurally painted 16x16 texture atlas. Zero image files ship with the game:
//! every pixel below is computed from noise, hashes or tiny ASCII sprites.

use crate::noise::{Perlin, Rng};

/// 64x64 tiles: the base game keeps the first 256, mods get the rest.
pub const ATLAS: usize = 1024;
pub const TILE: usize = 16;
pub const TILES_PER_ROW: u16 = (ATLAS / TILE) as u16;

pub const T_GRASS_TOP: u16 = 0;
pub const T_GRASS_SIDE: u16 = 1;
pub const T_DIRT: u16 = 2;
pub const T_STONE: u16 = 3;
pub const T_COBBLE: u16 = 4;
pub const T_SAND: u16 = 5;
pub const T_GRAVEL: u16 = 6;
pub const T_WATER: u16 = 7;
pub const T_LOG_SIDE: u16 = 8;
pub const T_LOG_TOP: u16 = 9;
pub const T_LEAVES: u16 = 10;
pub const T_PLANKS: u16 = 11;
pub const T_GLASS: u16 = 12;
pub const T_BEDROCK: u16 = 13;
pub const T_COAL_ORE: u16 = 14;
pub const T_IRON_ORE: u16 = 15;
pub const T_DIAMOND_ORE: u16 = 16;
pub const T_SNOW: u16 = 17;
pub const T_SNOW_SIDE: u16 = 18;
pub const T_BRICK: u16 = 19;
pub const T_TNT_SIDE: u16 = 20;
pub const T_TNT_TOP: u16 = 21;
pub const T_TNT_BOTTOM: u16 = 22;
pub const T_TABLE_TOP: u16 = 23;
pub const T_TABLE_SIDE: u16 = 24;
pub const T_GLOW: u16 = 25;
pub const T_TORCH: u16 = 26;
pub const T_FLOWER: u16 = 27;
pub const T_TALLGRASS: u16 = 28;
pub const T_CRACK0: u16 = 32; // .. 36
pub const T_STICK: u16 = 48;
pub const T_COAL: u16 = 49;
pub const T_IRON: u16 = 50;
pub const T_DIAMOND: u16 = 51;
pub const T_GUNPOWDER: u16 = 52;
pub const T_PORK: u16 = 53;
pub const T_GOO: u16 = 54;
pub const T_PICK0: u16 = 56; // .. 59
pub const T_SWORD0: u16 = 60; // .. 63
pub const T_PIG_SKIN: u16 = 64;
pub const T_PIG_FACE: u16 = 65;
pub const T_HISSER_SKIN: u16 = 66;
pub const T_HISSER_FACE: u16 = 67;
pub const T_GROAN_FACE: u16 = 68;
pub const T_GROAN_SKIN: u16 = 69;
pub const T_GROAN_SHIRT: u16 = 70;
pub const T_GROAN_PANTS: u16 = 71;
pub const T_WHITE: u16 = 72;
pub const T_SUN: u16 = 73;
pub const T_MOON: u16 = 74;
pub const T_HEART: u16 = 75;
pub const T_HEART_EMPTY: u16 = 76;
pub const T_CLOUD: u16 = 77;
pub const T_SKIN: u16 = 78;
pub const T_STOVE_FACE: u16 = 79;
pub const T_STOVE_SHIRT: u16 = 80;
pub const T_STOVE_PANTS: u16 = 81;
pub const T_HALF_HEART: u16 = 82;
pub const T_GOLD_ORE: u16 = 29;
pub const T_PUMPKIN_TOP: u16 = 30;
pub const T_PUMPKIN_SIDE: u16 = 31;
pub const T_JACK_FACE: u16 = 37;
pub const T_CACTUS_TOP: u16 = 38;
pub const T_CACTUS_SIDE: u16 = 39;
pub const T_ICE: u16 = 40;
pub const T_BOUNCY: u16 = 41;
pub const T_BED_TOP: u16 = 42;
pub const T_BED_SIDE: u16 = 43;
pub const T_CAKE_TOP: u16 = 44;
pub const T_CAKE_SIDE: u16 = 45;
pub const T_SPONGE: u16 = 46;
pub const T_WOOL: u16 = 47;
pub const T_GOLD: u16 = 55;
pub const T_GOLD_CHOP: u16 = 83;
pub const T_PEARL: u16 = 84;
pub const T_MUTTON: u16 = 85;
pub const T_FLUFF_FACE: u16 = 86;
pub const T_FLUFF_SKIN: u16 = 87;
pub const T_STARER_SKIN: u16 = 88;
pub const T_STARER_FACE: u16 = 89;
pub const T_TROPHY: u16 = 90;
pub const T_CLUCK_BODY: u16 = 96;
pub const T_CLUCK_FACE: u16 = 97;
pub const T_CLUCK_LEG: u16 = 98;
pub const T_MOO_SKIN: u16 = 99;
pub const T_MOO_FACE: u16 = 100;
pub const T_BONE: u16 = 101;
pub const T_RATTLER_FACE: u16 = 102;
pub const T_WEB_SKIN: u16 = 103;
pub const T_WEB_FACE: u16 = 104;
pub const T_BLOOP: u16 = 105;
pub const T_BLOOP_FACE: u16 = 106;
pub const T_FEATHER: u16 = 107;
pub const T_CLUCKETS: u16 = 108;
pub const T_MOO_STEAK: u16 = 109;
pub const T_BONE_ITEM: u16 = 110;
pub const T_ARROW: u16 = 111;
pub const T_STRING: u16 = 112;
pub const T_BOW: u16 = 113;
pub const T_SANDSTONE: u16 = 114;
pub const T_SANDSTONE_TOP: u16 = 115;
pub const T_STONE_BRICKS: u16 = 116;
pub const T_MOSSY: u16 = 117;
pub const T_HAY_TOP: u16 = 118;
pub const T_HAY_SIDE: u16 = 119;
pub const T_BOOKSHELF: u16 = 120;
pub const T_LANTERN: u16 = 121;
pub const T_MUSHROOM: u16 = 122;
pub const T_SCARECROW: u16 = 123;
pub const T_WEEDS: u16 = 124;
pub const T_FARMLAND: u16 = 125;
pub const T_FARMLAND_WET: u16 = 126;
pub const T_CROP_WHEAT: u16 = 127;
pub const T_CROP_CARROT: u16 = 131;
pub const T_CROP_POTATO: u16 = 135;
pub const T_HOE: u16 = 139;
pub const T_SEEDS: u16 = 140;
pub const T_WHEAT_ITEM: u16 = 141;
pub const T_CARROT_ITEM: u16 = 142;
pub const T_POTATO_ITEM: u16 = 143;
pub const T_BONE_DUST: u16 = 144;
pub const T_COMPOST: u16 = 145;
pub const T_WOOD_ASH: u16 = 146;
pub const T_SOIL_PROBE: u16 = 147;
pub const T_BREAD: u16 = 148;
pub const T_ROD: u16 = 149;
pub const T_COD: u16 = 150;
pub const T_SALMON: u16 = 151;
pub const T_PUFFER: u16 = 152;
pub const T_TROPICAL: u16 = 153;
pub const T_BIG_BOB: u16 = 154;
pub const T_BOOT: u16 = 155;
pub const T_BOTTLE: u16 = 156;
pub const T_FISH_CHIPS: u16 = 157;
pub const T_STEW: u16 = 158;
pub const T_WORM: u16 = 159;
pub const T_BOBBER: u16 = 160;
pub const T_CHEST_TOP: u16 = 161;
pub const T_CHEST_SIDE: u16 = 162;
pub const T_FURNACE_TOP: u16 = 163;
pub const T_FURNACE_SIDE: u16 = 164;
pub const T_FURNACE_LIT: u16 = 165;
pub const T_COOKED_CHOP: u16 = 166;
pub const T_COOKED_MUTTON: u16 = 167;
pub const T_COOKED_CLUCKETS: u16 = 168;
pub const T_STEAK: u16 = 169;
pub const T_COOKED_COD: u16 = 170;
pub const T_COOKED_SALMON: u16 = 171;
pub const T_BAKED_POTATO: u16 = 172;
pub const T_COOKED_PUFFER: u16 = 173;
pub const T_COOKED_BOOT: u16 = 174;
/// UI: the furnace's flame and progress arrow.
pub const T_FLAME: u16 = 175;
pub const T_ARROW_UI: u16 = 176;
/// Doors: the two halves, and the item.
pub const T_DOOR_BOTTOM: u16 = 177;
pub const T_DOOR_TOP: u16 = 178;
pub const T_DOOR_ITEM: u16 = 179;
/// Armour item sprites: `T_ARMOR_ITEMS + tier * 4 + slot` (tiers wool, iron,
/// gold, dimond; slots helmet, chestplate, leggings, boots).
pub const T_ARMOR_ITEMS: u16 = 180;
/// What worn armour looks like on a player, per tier.
pub const T_ARMOR_WORN: u16 = 196;
/// HUD: one armour point pair.
pub const T_ARMOR_ICON: u16 = 200;
/// HUD: two hunger points.
pub const T_HUNGER_ICON: u16 = 201;
pub const T_XP_ORB: u16 = 202;
pub const T_ANVIL_SIDE: u16 = 203;
pub const T_ANVIL_TOP: u16 = 204;
pub const T_ANVIL_TOP_CHIPPED: u16 = 205;
pub const T_ANVIL_TOP_DAMAGED: u16 = 206;
pub const T_GLOWSHROOM: u16 = 207;
pub const T_POINTY_ROCK: u16 = 208;
pub const T_ENCH_TOP: u16 = 209;
pub const T_ENCH_SIDE: u16 = 210;
pub const T_ENCH_BOTTOM: u16 = 211;
pub const T_LAVA: u16 = 212;
pub const T_OBSIDIAN: u16 = 213;
pub const T_BUCKET: u16 = 214;
pub const T_WATER_BUCKET: u16 = 215;
pub const T_LAVA_BUCKET: u16 = 216;
pub const T_WOOF_SKIN: u16 = 217;
pub const T_WOOF_FACE: u16 = 218;
pub const T_COLLAR: u16 = 219;
pub const T_SHEARS: u16 = 220;
pub const T_ZAP_ORE: u16 = 221;
pub const T_WIRE: u16 = 222;
pub const T_WIRE_ON: u16 = 223;
pub const T_LEVER: u16 = 224;
pub const T_LEVER_ON: u16 = 225;
pub const T_ZAP_BLOCK: u16 = 226;
pub const T_LAMP: u16 = 227;
pub const T_LAMP_ON: u16 = 228;
pub const T_ZAP_DUST: u16 = 229;
pub const T_BOOK: u16 = 230;
pub const T_ENCHANTED_BOOK: u16 = 231;
pub const T_HMM_FACE: u16 = 232;
pub const T_HMM_ROBE: u16 = 233;
pub const T_SHIELD: u16 = 234;
pub const T_SCORCHROCK: u16 = 235;
pub const T_EMBERSAND: u16 = 236;
pub const T_SCORCH_GOLD: u16 = 237;
pub const T_PORTAL: u16 = 238;
pub const T_SPARKER: u16 = 239;
pub const T_GRUMBLE_SKIN: u16 = 240;
pub const T_GRUMBLE_FACE: u16 = 241;
/// Six rail shapes in a row (see `RAIL_FIRST`), then four powered rails.
pub const T_RAIL: u16 = 242;
pub const T_POWERED_RAIL: u16 = 248;
pub const T_CART: u16 = 252;
pub const T_BOAT_ITEM: u16 = 253;
pub const T_CART_ITEM: u16 = 254;
pub const T_FRAME: u16 = 255;
pub const T_COMPASS: u16 = 256;
pub const T_MAP: u16 = 257;
pub const T_SAPLING: u16 = 258;
pub const T_APPLE: u16 = 259;
pub const T_LADDER: u16 = 260;
pub const T_TRAPDOOR: u16 = 261;
/// Seven dyed wools (colours 1..8), eight stained glasses and eight dyes.
pub const T_DYED_WOOL: u16 = 262;
pub const T_STAINED_GLASS: u16 = 269;
pub const T_DYE_FIRST: u16 = 277;
pub const T_FIRE: u16 = 285;
pub const T_GLASS_BOTTLE: u16 = 286;
pub const T_WATER_BOTTLE: u16 = 287;
pub const T_POTION_FIRST: u16 = 288;
pub const T_SPLASH_FIRST: u16 = 293;
pub const T_TUSK: u16 = 298;
pub const T_EMBER_SHROOM: u16 = 299;
pub const T_BREWING_TOP: u16 = 300;
pub const T_BREWING_SIDE: u16 = 301;
pub const T_ZTORCH_ON: u16 = 302;
pub const T_ZTORCH_OFF: u16 = 303;
pub const T_REPEATER: u16 = 304;
pub const T_REPEATER_ON: u16 = 305;
pub const T_PISTON_FACE: u16 = 306;
pub const T_STICKY_FACE: u16 = 307;
pub const T_PISTON_SIDE: u16 = 308;
pub const T_PISTON_BACK: u16 = 309;
pub const T_DISPENSER_FACE: u16 = 310;
pub const T_HOPPER_TOP: u16 = 311;
pub const T_HOPPER_SIDE: u16 = 312;
pub const T_GALLOPER: u16 = 313;
pub const T_GALLOP_FACE: u16 = 314;
pub const T_GALLOP_MANE: u16 = 315;
pub const T_SADDLE_LEATHER: u16 = 316;
pub const T_SADDLE: u16 = 317;
pub const T_HOLLOW_STONE: u16 = 318;
pub const T_HOLLOW_PORTAL: u16 = 319;
pub const T_EYE_FRAME_TOP: u16 = 320;
pub const T_EYE_FRAME_FULL: u16 = 321;
pub const T_EYE_FRAME_SIDE: u16 = 322;
pub const T_WYRM_CRYSTAL: u16 = 323;
pub const T_WYRM_EGG: u16 = 324;
pub const T_STARING_EYE: u16 = 325;
pub const T_WYRM_SKIN: u16 = 326;
pub const T_WYRM_WING: u16 = 327;
pub const T_WYRM_FACE: u16 = 328;
/// Player skins: four tiles each (tone, face, shirt, trousers; see nametags.rs).
pub const T_SKIN_FIRST: u16 = 329;
pub const T_NAME_TAG: u16 = 353;
// Biomes (see world.rs).
pub const T_SPRUCE_LOG_SIDE: u16 = 354;
pub const T_SPRUCE_LOG_TOP: u16 = 355;
pub const T_SPRUCE_LEAVES: u16 = 356;
pub const T_JUNGLE_LOG_SIDE: u16 = 357;
pub const T_JUNGLE_LOG_TOP: u16 = 358;
pub const T_JUNGLE_LEAVES: u16 = 359;
pub const T_MUD: u16 = 360;
pub const T_LILY_PAD: u16 = 361;
pub const T_RED_SAND: u16 = 362;
/// Four terracottas: plain, orange, red, yellow.
pub const T_TERRACOTTA: u16 = 363;
pub const T_DEAD_BUSH: u16 = 367;
pub const T_MELON_SIDE: u16 = 368;
pub const T_MELON_TOP: u16 = 369;
pub const T_MELON_SLICE: u16 = 370;
pub const T_SQUAWK: u16 = 371;
pub const T_SQUAWK_FACE: u16 = 372;
pub const T_SQUAWK_WING: u16 = 373;
pub const T_CLANK: u16 = 374;
pub const T_CLANK_FACE: u16 = 375;
/// Detector rails: north-south off/on, east-west off/on.
pub const T_DETECTOR_RAIL: u16 = 376;
pub const T_CHEST_CART_ITEM: u16 = 380;
pub const T_HOPPER_CART_ITEM: u16 = 381;
/// Comparators: any/off, any/on, half/off, half/on.
pub const T_COMPARATOR: u16 = 382;
pub const T_BEACON: u16 = 386;
pub const T_BEACON_BEAM: u16 = 387;
/// Empty slots for the hunger and armour bars.
pub const T_HUNGER_EMPTY: u16 = 388;
pub const T_ARMOR_EMPTY: u16 = 389;
pub const T_WOOF_NOSE: u16 = 390;
pub const T_GALLOP_EYE: u16 = 391;
pub const T_HOLLOW_BOX_TOP: u16 = 392;
pub const T_HOLLOW_BOX_SIDE: u16 = 393;
pub const T_COPPER_ORE: u16 = 394;
/// Copper blocks, from new to fully oxidized.
pub const T_COPPER: u16 = 395;
pub const T_BAMBOO: u16 = 399;
pub const T_BAMBOO_BLOCK_TOP: u16 = 400;
pub const T_BAMBOO_BLOCK_SIDE: u16 = 401;
pub const T_BAMBOO_PLANKS: u16 = 402;
pub const T_BAMBOO_MOSAIC: u16 = 403;
/// Coral blocks: tube (blue), brain (pink), bubble (purple), fire (red); then dead.
pub const T_CORAL: u16 = 404;
pub const T_DEAD_CORAL: u16 = 408;
pub const T_GLIDER: u16 = 409;
pub const T_ROCKET: u16 = 410;
pub const T_SPEAR: u16 = 411;
pub const T_COPPER_INGOT: u16 = 412;
/// A worn Glider's membrane (on the player's back).
pub const T_GLIDER_WING: u16 = 413;
pub const T_FISHY: u16 = 414;
pub const T_FISHY_FACE: u16 = 415;
pub const T_SOGGY_SKIN: u16 = 416;
pub const T_SOGGY_FACE: u16 = 417;
pub const T_SOGGY_SHIRT: u16 = 418;
pub const T_SOGGY_PANTS: u16 = 419;
pub const T_FISHY_FIN: u16 = 420;
/// Axes and shovels by tier (wood, stone, copper, iron, dimond).
pub const T_AXE0: u16 = 421;
pub const T_SHOVEL0: u16 = 426;
pub const T_PICK_COPPER: u16 = 431;
pub const T_SWORD_COPPER: u16 = 432;
/// Copper armour items (helmet .. boots), and how it looks when worn.
pub const T_COPPER_ARMOR_ITEMS: u16 = 433;
pub const T_COPPER_ARMOR_WORN: u16 = 437;
// Beekeeping.
pub const T_NEST_TOP: u16 = 438;
pub const T_NEST_SIDE: u16 = 439;
pub const T_NEST_HONEY_SIDE: u16 = 440;
pub const T_HIVE_TOP: u16 = 441;
pub const T_HIVE_SIDE: u16 = 442;
pub const T_HIVE_BUSY_SIDE: u16 = 443;
pub const T_HIVE_HONEY_SIDE: u16 = 444;
pub const T_HONEY_BLOCK_TOP: u16 = 445;
pub const T_HONEY_BLOCK_SIDE: u16 = 446;
pub const T_DANDELION: u16 = 447;
pub const T_CORNFLOWER: u16 = 448;
pub const T_LAVENDER: u16 = 449;
pub const T_TORCH_SPROUT: u16 = 450;
pub const T_TORCHFLOWER: u16 = 451;
/// Five honey bottles in a row.
pub const T_HONEY_FIRST: u16 = 452;
pub const T_HONEYCOMB: u16 = 457;
pub const T_SMOKER: u16 = 458;
pub const T_HIVE_TOOL: u16 = 459;
pub const T_QUEEN: u16 = 460;
pub const T_BEE: u16 = 461;
pub const T_BEE_FACE: u16 = 462;
pub const T_BEE_WING: u16 = 463;
pub const T_TORCH_SEEDS: u16 = 464;
// New animals (see critters.rs) and the Hush (deepdark.rs).
pub const T_FOX: u16 = 465;
pub const T_FOX_FACE: u16 = 466;
pub const T_FOX_TAIL: u16 = 467;
pub const T_FOX_DARK: u16 = 468;
pub const T_FROG: u16 = 469;
pub const T_FROG_FACE: u16 = 470;
pub const T_FROG_EYE: u16 = 471;
pub const T_SHELL: u16 = 472;
pub const T_ROLLO_SKIN: u16 = 473;
pub const T_ROLLO_FACE: u16 = 474;
pub const T_HUSH: u16 = 475;
pub const T_HUSH_FACE: u16 = 476;
pub const T_HUSH_GLOW: u16 = 477;
/// Three froglights in a row.
pub const T_FROGLIGHT: u16 = 478;
pub const T_SCUTE: u16 = 481;
pub const T_WOLF_ARMOR_ITEM: u16 = 482;
pub const T_WOLF_ARMOR_WORN: u16 = 483;
// The Deep Dark (see deepdark.rs).
pub const T_DEEPSLATE: u16 = 484;
pub const T_DEEPSLATE_TOP: u16 = 485;
pub const T_COBBLED_DEEPSLATE: u16 = 486;
pub const T_DEEPSLATE_BRICKS: u16 = 487;
pub const T_DEEPSLATE_TILES: u16 = 488;
pub const T_REINFORCED_SIDE: u16 = 489;
pub const T_REINFORCED_TOP: u16 = 490;
pub const T_SCULK: u16 = 491;
pub const T_SENSOR_TOP: u16 = 492;
pub const T_SENSOR_SIDE: u16 = 493;
pub const T_SENSOR_ACTIVE_TOP: u16 = 494;
pub const T_SHRIEKER_TOP: u16 = 495;
pub const T_SHRIEKER_SIDE: u16 = 496;
pub const T_CATALYST_TOP: u16 = 497;
pub const T_CATALYST_SIDE: u16 = 498;
pub const T_SOUL_LANTERN: u16 = 499;
// Archaeology (see archaeology.rs).
pub const T_SUS_SAND: u16 = 500;
pub const T_SUS_GRAVEL: u16 = 501;
pub const T_BENCH_TOP: u16 = 502;
pub const T_BENCH_SIDE: u16 = 503;
pub const T_POT_TOP: u16 = 504;
/// Thirteen pot sides: plain, then one per shard.
pub const T_POT_SIDE_FIRST: u16 = 505;
// Grinding and smithing (see smithing.rs).
pub const T_GRINDSTONE_SIDE: u16 = 518;
pub const T_GRINDSTONE_TOP: u16 = 519;
pub const T_SMITHING_TOP: u16 = 520;
pub const T_SMITHING_SIDE: u16 = 521;
pub const T_OLD_DEBRIS_SIDE: u16 = 522;
pub const T_OLD_DEBRIS_TOP: u16 = 523;
pub const T_BRUSH: u16 = 524;
pub const T_DIAMOND_BRUSH: u16 = 525;
pub const T_JOURNAL: u16 = 526;
/// Twelve shards, then twelve relics.
pub const T_SHARD_FIRST: u16 = 527;
pub const T_RELIC_FIRST: u16 = 539;
pub const T_ENCRUSTED: u16 = 551;
pub const T_MAP_FRAGMENT: u16 = 552;
pub const T_COIN: u16 = 553;
pub const T_TABLET: u16 = 554;
pub const T_ECHO_SHARD: u16 = 555;
pub const T_RECOVERY_COMPASS: u16 = 556;
pub const T_SCRAP: u16 = 557;
pub const T_SCORCHITE_INGOT: u16 = 558;
pub const T_TEMPLATE: u16 = 559;
/// Pickaxe, sword, axe, shovel.
pub const T_SCORCHITE_TOOLS: u16 = 560;
pub const T_SCORCHITE_ARMOR_ITEMS: u16 = 564;
pub const T_SCORCHITE_ARMOR_WORN: u16 = 568;
// Music (see music.rs).
pub const T_NOTE_BLOCK: u16 = 572;
pub const T_JUKEBOX_TOP: u16 = 573;
pub const T_JUKEBOX_SIDE: u16 = 574;
pub const T_DISC_FIRST: u16 = 575;
pub const T_NOTE_PARTICLE: u16 = 583;
// The Scorchlands (see fortress.rs).
pub const T_SCORCH_BRICKS: u16 = 584;
pub const T_CAGE: u16 = 585;
pub const T_GOLD_BLOCK: u16 = 586;
pub const T_GILDED: u16 = 587;
pub const T_BELL: u16 = 588;
pub const T_SIZZLE_ROD: u16 = 589;
pub const T_SIZZLE_POWDER: u16 = 590;
pub const T_WEEPER_TEAR: u16 = 591;
pub const T_SHROOM_STICK: u16 = 592;
pub const T_POTION_EXTRA: u16 = 593;
pub const T_SPLASH_EXTRA: u16 = 595;
// Raids (see raids.rs).
pub const T_CROSSBOW: u16 = 597;
pub const T_CROSSBOW_LOADED: u16 = 598;
pub const T_TOTEM: u16 = 599;
pub const T_BANNER: u16 = 600;
pub const T_FIREBALL: u16 = 601;
// The new mobs' skins.
pub const T_SIZZLER: u16 = 602;
pub const T_SIZZLER_FACE: u16 = 603;
pub const T_SIZZLER_ROD: u16 = 604;
pub const T_WEEPER: u16 = 605;
pub const T_WEEPER_FACE: u16 = 606;
pub const T_WEEPER_ANGRY: u16 = 607;
pub const T_STRUTTER: u16 = 608;
pub const T_STRUTTER_FACE: u16 = 609;
pub const T_STRUTTER_COLD: u16 = 610;
pub const T_SNOUT: u16 = 611;
pub const T_SNOUT_FACE: u16 = 612;
pub const T_SNOUT_TUNIC: u16 = 613;
pub const T_ILLAGER: u16 = 614;
pub const T_ILLAGER_FACE: u16 = 615;
pub const T_PILFERER_COAT: u16 = 616;
pub const T_HACKLER_COAT: u16 = 617;
pub const T_INVOICER_ROBE: u16 = 618;
pub const T_FEE: u16 = 619;
pub const T_RAMPAGER: u16 = 620;
pub const T_RAMPAGER_FACE: u16 = 621;
pub const T_BANNER_WORN: u16 = 622;
pub const T_SPAWNER: u16 = 623;
pub const T_CHERRY_LOG_SIDE: u16 = 624;
pub const T_CHERRY_LOG_TOP: u16 = 625;
pub const T_CHERRY_LEAVES: u16 = 626;
pub const T_PINK_PETALS: u16 = 627;
pub const T_CHERRY_PLANKS: u16 = 628;
pub const T_MANGROVE_LOG_SIDE: u16 = 629;
pub const T_MANGROVE_LOG_TOP: u16 = 630;
pub const T_MANGROVE_LEAVES: u16 = 631;
pub const T_MANGROVE_ROOTS: u16 = 632;
pub const T_MANGROVE_PLANKS: u16 = 633;
pub const T_OBSERVER_FACE: u16 = 634;
pub const T_OBSERVER_SIDE: u16 = 635;
pub const T_OBSERVER_BACK: u16 = 636;
pub const T_OBSERVER_BACK_ON: u16 = 637;
pub const T_CRAFTER_TOP: u16 = 638;
pub const T_CRAFTER_SIDE: u16 = 639;
pub const T_CRAFTER_FACE: u16 = 640;
pub const T_COPPER_BULB: u16 = 641;
pub const T_COPPER_BULB_ON: u16 = 642;
pub const T_TUFF_BRICKS: u16 = 643;
pub const T_CHISELED_TUFF: u16 = 644;
pub const T_CHISELED_TUFF_TOP: u16 = 645;
pub const T_COPPER_GRATE: u16 = 646;
pub const T_TRIAL_SPAWNER: u16 = 647;
pub const T_TRIAL_SPAWNER_SPENT: u16 = 648;
pub const T_TRIAL_SPAWNER_TOP: u16 = 649;
pub const T_VAULT_FRONT: u16 = 650;
pub const T_VAULT_OPEN: u16 = 651;
pub const T_VAULT_TOP: u16 = 652;
pub const T_LODESTONE_TOP: u16 = 653;
pub const T_LODESTONE_SIDE: u16 = 654;
pub const T_TRIAL_KEY: u16 = 655;
pub const T_WIND_CHARGE: u16 = 656;
pub const T_BREEZE_ROD: u16 = 657;
pub const T_GOAT_HORN: u16 = 658;
pub const T_TRIM_TEMPLATE: u16 = 659;
pub const T_SPYGLASS: u16 = 660;
pub const T_BUNDLE: u16 = 661;
pub const T_BREEZE: u16 = 662;
pub const T_BREEZE_FACE: u16 = 663;
pub const T_GOAT: u16 = 664;
pub const T_GOAT_FACE: u16 = 665;
pub const T_AXOLOTL: u16 = 666;
pub const T_AXOLOTL_FACE: u16 = 667;
pub const T_AXOLOTL_GILL: u16 = 668;
pub const T_CAMEL: u16 = 669;
pub const T_CAMEL_FACE: u16 = 670;
pub const T_CAMEL_HUMP: u16 = 671;
/// Firework sparks, one per colour (see fireworks.rs).
pub const T_SPARK_FIRST: u16 = 672;
/// Armour trim colours, one per material (see trims.rs).
pub const T_TRIM_FIRST: u16 = 680;
/// Ominous Trials and the Mace.
pub const T_OMINOUS_SPAWNER: u16 = 686;
pub const T_VAULT_OMINOUS: u16 = 687;
pub const T_HEAVY_CORE: u16 = 688;
pub const T_OMINOUS_KEY: u16 = 689;
pub const T_MACE: u16 = 690;
pub const T_OMINOUS_BOTTLE: u16 = 691;
/// The Pale Garden and the Creaking.
pub const T_PALE_LOG_SIDE: u16 = 692;
pub const T_PALE_LOG_TOP: u16 = 693;
pub const T_PALE_LEAVES: u16 = 694;
pub const T_PALE_PLANKS: u16 = 695;
pub const T_PALE_MOSS: u16 = 696;
pub const T_PALE_HANGING_MOSS: u16 = 697;
pub const T_CREAKING_HEART: u16 = 698;
pub const T_CREAKING_HEART_ON: u16 = 699;
pub const T_CREAKING: u16 = 700;
pub const T_CREAKING_FACE: u16 = 701;
pub const T_CREAKING_FACE_ON: u16 = 702;
/// Sniffers and their finds.
pub const T_SNIFFER: u16 = 703;
pub const T_SNIFFER_FACE: u16 = 704;
pub const T_SNIFFER_EGG: u16 = 705;
pub const T_PITCHER_POD: u16 = 706;
pub const T_PITCHER_CROP: u16 = 707;
pub const T_PITCHER_PLANT: u16 = 708;
/// Books and lecterns.
pub const T_BOOK_QUILL: u16 = 709;
pub const T_WRITTEN_BOOK: u16 = 710;
pub const T_LECTERN_TOP: u16 = 711;
pub const T_LECTERN_SIDE: u16 = 712;
pub const T_LECTERN_BOOK_TOP: u16 = 713;
/// Looms and banners.
pub const T_LOOM_TOP: u16 = 714;
pub const T_LOOM_SIDE: u16 = 715;
pub const T_BANNER_ICON: u16 = 716;
/// Personal Chests.
pub const T_PERSONAL_CHEST_TOP: u16 = 717;
pub const T_PERSONAL_CHEST_SIDE: u16 = 718;
/// v0.1.18: copper chests and golems, the Pale Garden's flowers and resin,
/// ground covers, Floaties, spears and Rotsteeds.
pub const T_COPPER_CHEST_TOP: u16 = 719;
pub const T_COPPER_CHEST_SIDE: u16 = 720;
pub const T_COPPER_GOLEM: u16 = 721;
pub const T_COPPER_GOLEM_FACE: u16 = 722;
pub const T_EYEBLOSSOM: u16 = 723;
pub const T_EYEBLOSSOM_OPEN: u16 = 724;
pub const T_RESIN_BLOCK: u16 = 725;
pub const T_RESIN_BRICKS: u16 = 726;
pub const T_RESIN_CLUMP: u16 = 727;
pub const T_RESIN_BRICK: u16 = 728;
pub const T_FIREFLY_BUSH: u16 = 729;
pub const T_LEAF_LITTER: u16 = 730;
pub const T_WILDFLOWERS: u16 = 731;
pub const T_DRIED_FLOATY: u16 = 732;
pub const T_FLOATY: u16 = 733;
pub const T_FLOATY_FACE: u16 = 734;
pub const T_HARNESS: u16 = 735;
/// Four in a row: wood, stone, iron, dimond.
pub const T_SPEAR_FIRST: u16 = 736;
pub const T_ROTSTEED: u16 = 740;
pub const T_ROTSTEED_FACE: u16 = 741;
pub const T_HARNESS_WORN: u16 = 742;
/// The moon in each of its eight phases, full first (see skies.rs).
pub const T_MOON_PHASES: u16 = 743;
/// v0.1.20: home blocks, treasure, new animals and Hmmers.
pub const T_CAMPFIRE_TOP: u16 = 751;
pub const T_CAMPFIRE_SIDE: u16 = 752;
pub const T_SMOKER_TOP: u16 = 753;
pub const T_SMOKER_SIDE: u16 = 754;
pub const T_SMOKER_LIT: u16 = 755;
pub const T_BLAST_TOP: u16 = 756;
pub const T_BLAST_SIDE: u16 = 757;
pub const T_BLAST_LIT: u16 = 758;
pub const T_BARREL_TOP: u16 = 759;
pub const T_BARREL_SIDE: u16 = 760;
pub const T_TURTLE_EGG: u16 = 761;
/// Eight pictures in a row (see home.rs).
pub const T_PAINTING_FIRST: u16 = 762;
pub const PAINTINGS: u16 = 8;
pub const T_TREASURE_MAP: u16 = 770;
pub const T_TURTLE_SCUTE: u16 = 771;
pub const T_TURTLE_SHELL: u16 = 772;
pub const T_TURTLE_WORN: u16 = 773;
pub const T_TURTLE: u16 = 774;
pub const T_TURTLE_FACE: u16 = 775;
pub const T_TURTLE_SHELL_TOP: u16 = 776;
pub const T_DOLPHIN: u16 = 777;
pub const T_DOLPHIN_FACE: u16 = 778;
pub const T_PANDA: u16 = 779;
pub const T_PANDA_BLACK: u16 = 780;
pub const T_PANDA_FACE: u16 = 781;
pub const T_POLAR: u16 = 782;
pub const T_POLAR_FACE: u16 = 783;
pub const T_LLAMA: u16 = 784;
pub const T_LLAMA_FACE: u16 = 785;
pub const T_ZHMM_FACE: u16 = 786;
pub const T_ZHMM_ROBE: u16 = 787;
pub const T_WANDERER_ROBE: u16 = 788;
/// v0.1.21: building blocks (eight colours each, in carpentry::COLOURS order).
pub const T_CONCRETE: u16 = 789;
pub const T_CONCRETE_POWDER: u16 = 797;
pub const T_GLAZED: u16 = 805;
pub const T_CANDLE: u16 = 813;
pub const T_CANDLE_LIT: u16 = 814;
pub const T_CHAIN: u16 = 815;
pub const T_SCAFFOLD_TOP: u16 = 816;
pub const T_SCAFFOLD_SIDE: u16 = 817;
/// Chest tiers (see chests.rs).
pub const T_IRON_CHEST_TOP: u16 = 818;
pub const T_IRON_CHEST_SIDE: u16 = 819;
pub const T_GOLD_CHEST_TOP: u16 = 820;
pub const T_GOLD_CHEST_SIDE: u16 = 821;
pub const T_DIAMOND_CHEST_TOP: u16 = 822;
pub const T_DIAMOND_CHEST_SIDE: u16 = 823;
/// A bed's top turned to face east, south and west (the pillow at the head).
pub const T_BED_TOP_E: u16 = 824;
pub const T_BED_TOP_S: u16 = 825;
pub const T_BED_TOP_W: u16 = 826;
/// Backpacks.
pub const T_BACKPACK: u16 = 827;
pub const T_BIG_BACKPACK: u16 = 828;
pub const T_HUGE_BACKPACK: u16 = 829;
// Cave biomes.
pub const T_DRIPSTONE: u16 = 830;
pub const T_MOSS: u16 = 831;
pub const T_CAVE_VINES: u16 = 832;
pub const T_CAVE_VINES_LIT: u16 = 833;
pub const T_AZALEA: u16 = 834;
pub const T_CALCITE: u16 = 835;
pub const T_SMOOTH_BASALT: u16 = 836;
pub const T_AMETHYST: u16 = 837;
pub const T_BUDDING_AMETHYST: u16 = 838;
pub const T_AMETHYST_BUD_SMALL: u16 = 839;
pub const T_AMETHYST_BUD_LARGE: u16 = 840;
pub const T_AMETHYST_CLUSTER: u16 = 841;
pub const T_TINTED_GLASS: u16 = 842;
pub const T_GLOW_BERRIES: u16 = 843;
pub const T_AMETHYST_SHARD: u16 = 844;
// Temples, mineshafts and igloos.
pub const T_COBWEB: u16 = 845;
pub const T_TRIPWIRE: u16 = 846;
pub const T_TRIPWIRE_EW: u16 = 847;
pub const T_TRIPWIRE_HOOK: u16 = 848;
pub const T_TRIPWIRE_HOOK_ON: u16 = 849;
pub const T_CHISELED_SANDSTONE: u16 = 850;
// The Ocean Monument.
pub const T_PRISMARINE: u16 = 851;
pub const T_PRISMARINE_BRICKS: u16 = 852;
pub const T_DARK_PRISMARINE: u16 = 853;
pub const T_SEA_LANTERN: u16 = 854;
pub const T_CONDUIT: u16 = 855;
pub const T_PRISMARINE_SHARD: u16 = 856;
pub const T_PRISMARINE_CRYSTALS: u16 = 857;
pub const T_NAUTILUS_SHELL: u16 = 858;
pub const T_HEART_OF_THE_SEA: u16 = 859;
pub const T_GUARDIAN: u16 = 860;
pub const T_GUARDIAN_EYE: u16 = 861;
pub const T_GUARDIAN_SPIKE: u16 = 862;
pub const T_ELDER_GUARDIAN: u16 = 863;
pub const T_ELDER_EYE: u16 = 864;
pub const T_GUARDIAN_LASER: u16 = 865;
// Night threats and small creatures.
pub const T_GLOW_INK_SAC: u16 = 866;
pub const T_WITCH_ROBE: u16 = 867;
pub const T_WITCH_FACE: u16 = 868;
pub const T_WITCH_HAT: u16 = 869;
pub const T_DESERT_SKIN: u16 = 870;
pub const T_DESERT_FACE: u16 = 871;
pub const T_DESERT_CLOTH: u16 = 872;
pub const T_STRAY_BONE: u16 = 873;
pub const T_STRAY_FACE: u16 = 874;
pub const T_STRAY_CLOTH: u16 = 875;
pub const T_GLOW_SQUID: u16 = 876;
pub const T_GLOW_SQUID_FACE: u16 = 877;
pub const T_BAT: u16 = 878;
pub const T_BAT_WING: u16 = 879;
pub const T_ALLAY: u16 = 880;
pub const T_ALLAY_FACE: u16 = 881;
pub const T_ALLAY_WING: u16 = 882;
/// v0.2 part 2: the Wilter and what makes it, its star's beacons, rope and the Support Gauge.
pub const T_SORROW_SAND: u16 = 883;
pub const T_CHARRED_SKULL: u16 = 884;
pub const T_CHARRED_SKULL_FACE: u16 = 885;
pub const T_STARRED_BEACON: u16 = 886;
pub const T_ROPE: u16 = 887;
pub const T_WILTER_STAR: u16 = 888;
pub const T_SUPPORT_GAUGE: u16 = 889;
pub const T_CHARRED_BONE: u16 = 890;
pub const T_CHARRED_FACE: u16 = 891;
pub const T_WILTER: u16 = 892;
pub const T_WILTER_FACE: u16 = 893;
// Crop tiles are four in a row: T_CROP_* + stage.

/// Mod textures are allocated from here to the end of the atlas (the base game
/// keeps the first 1024 tiles; mods look textures up by name, so this can move).
pub const FIRST_MOD_TILE: u16 = 1024;

/// Names mods can use to refer to built-in textures.
pub const BASE_TEXTURES: &[(&str, u16)] = &[
    ("grass_top", T_GRASS_TOP),
    ("grass_side", T_GRASS_SIDE),
    ("dirt", T_DIRT),
    ("stone", T_STONE),
    ("cobblestone", T_COBBLE),
    ("sand", T_SAND),
    ("gravel", T_GRAVEL),
    ("water", T_WATER),
    ("log_side", T_LOG_SIDE),
    ("log_top", T_LOG_TOP),
    ("leaves", T_LEAVES),
    ("planks", T_PLANKS),
    ("glass", T_GLASS),
    ("bedrock", T_BEDROCK),
    ("coal_ore", T_COAL_ORE),
    ("iron_ore", T_IRON_ORE),
    ("diamond_ore", T_DIAMOND_ORE),
    ("snow", T_SNOW),
    ("snow_side", T_SNOW_SIDE),
    ("bricks", T_BRICK),
    ("tnt_side", T_TNT_SIDE),
    ("tnt_top", T_TNT_TOP),
    ("tnt_bottom", T_TNT_BOTTOM),
    ("crafting_table_top", T_TABLE_TOP),
    ("crafting_table_side", T_TABLE_SIDE),
    ("glowrock", T_GLOW),
    ("torch", T_TORCH),
    ("flower", T_FLOWER),
    ("tall_grass", T_TALLGRASS),
    ("stick", T_STICK),
    ("coal", T_COAL),
    ("iron", T_IRON),
    ("diamond", T_DIAMOND),
    ("gunpowder", T_GUNPOWDER),
    ("porkchop", T_PORK),
    ("goo", T_GOO),
    ("wooden_pickaxe", T_PICK0),
    ("stone_pickaxe", T_PICK0 + 1),
    ("iron_pickaxe", T_PICK0 + 2),
    ("diamond_pickaxe", T_PICK0 + 3),
    ("wooden_sword", T_SWORD0),
    ("stone_sword", T_SWORD0 + 1),
    ("iron_sword", T_SWORD0 + 2),
    ("diamond_sword", T_SWORD0 + 3),
    ("white", T_WHITE),
    ("gold_ore", T_GOLD_ORE),
    ("pumpkin_top", T_PUMPKIN_TOP),
    ("pumpkin_side", T_PUMPKIN_SIDE),
    ("jack_o_lantern", T_JACK_FACE),
    ("cactus_top", T_CACTUS_TOP),
    ("cactus_side", T_CACTUS_SIDE),
    ("ice", T_ICE),
    ("bouncy_goo", T_BOUNCY),
    ("bed_top", T_BED_TOP),
    ("bed_side", T_BED_SIDE),
    ("cake_top", T_CAKE_TOP),
    ("cake_side", T_CAKE_SIDE),
    ("sponge", T_SPONGE),
    ("wool", T_WOOL),
    ("gold", T_GOLD),
    ("golden_oinkchop", T_GOLD_CHOP),
    ("stare_pearl", T_PEARL),
    ("mutton", T_MUTTON),
    ("feather", T_FEATHER),
    ("cluckets", T_CLUCKETS),
    ("moo_steak", T_MOO_STEAK),
    ("bone", T_BONE_ITEM),
    ("pointy_stick", T_ARROW),
    ("string", T_STRING),
    ("bow", T_BOW),
    ("chest_top", T_CHEST_TOP),
    ("chest_side", T_CHEST_SIDE),
    ("furnace_top", T_FURNACE_TOP),
    ("furnace_side", T_FURNACE_SIDE),
    ("furnace_lit", T_FURNACE_LIT),
    ("door_bottom", T_DOOR_BOTTOM),
    ("door_top", T_DOOR_TOP),
    ("anvil_side", T_ANVIL_SIDE),
    ("anvil_top", T_ANVIL_TOP),
    ("glowshroom", T_GLOWSHROOM),
    ("pointy_rock", T_POINTY_ROCK),
    ("lava", T_LAVA),
    ("obsidian", T_OBSIDIAN),
];

pub fn base_texture(name: &str) -> Option<u16> {
    BASE_TEXTURES.iter().find(|(n, _)| *n == name).map(|(_, t)| *t)
}

/// Copy one tile's 16x16 RGBA out of an atlas.
pub fn tile_pixels(atlas: &[u8], tile: u16) -> Vec<u8> {
    let (tx, ty) = ((tile % TILES_PER_ROW) as usize * TILE, (tile / TILES_PER_ROW) as usize * TILE);
    let mut out = Vec::with_capacity(TILE * TILE * 4);
    for y in 0..TILE {
        let i = ((ty + y) * ATLAS + tx) * 4;
        out.extend_from_slice(&atlas[i..i + TILE * 4]);
    }
    out
}

/// Paint mod textures from the active registry over a base atlas.
pub fn apply_mod_textures(atlas: &mut [u8]) {
    for (tile, px) in &crate::block::reg().textures {
        let (tx, ty) = ((tile % TILES_PER_ROW) as usize * TILE, (tile / TILES_PER_ROW) as usize * TILE);
        for y in 0..TILE {
            let i = ((ty + y) * ATLAS + tx) * 4;
            atlas[i..i + TILE * 4].copy_from_slice(&px[y * TILE * 4..(y + 1) * TILE * 4]);
        }
    }
}

/// Top-left UV of a tile plus its size, in 0..1 atlas space.
pub fn tile_uv(tile: u16) -> (f32, f32, f32) {
    let s = 1.0 / TILES_PER_ROW as f32;
    ((tile % TILES_PER_ROW) as f32 * s, (tile / TILES_PER_ROW) as f32 * s, s)
}

type Rgba = [u8; 4];

fn rgb(r: u8, g: u8, b: u8) -> Rgba {
    [r, g, b, 255]
}

fn shade(c: Rgba, f: f32) -> Rgba {
    let m = |v: u8| ((v as f32 * f).round().clamp(0.0, 255.0)) as u8;
    [m(c[0]), m(c[1]), m(c[2]), c[3]]
}

struct Atlas {
    px: Vec<u8>,
    rng: Rng,
    perlin: Perlin,
}

impl Atlas {
    fn set(&mut self, tile: u16, x: usize, y: usize, c: Rgba) {
        let tx = (tile % TILES_PER_ROW) as usize * TILE + x;
        let ty = (tile / TILES_PER_ROW) as usize * TILE + y;
        let i = (ty * ATLAS + tx) * 4;
        self.px[i..i + 4].copy_from_slice(&c);
    }
    fn get(&self, tile: u16, x: usize, y: usize) -> Rgba {
        let tx = (tile % TILES_PER_ROW) as usize * TILE + x;
        let ty = (tile / TILES_PER_ROW) as usize * TILE + y;
        let i = (ty * ATLAS + tx) * 4;
        [self.px[i], self.px[i + 1], self.px[i + 2], self.px[i + 3]]
    }
    fn each(&mut self, tile: u16, mut f: impl FnMut(usize, usize, &mut Rng, &Perlin) -> Rgba) {
        for y in 0..TILE {
            for x in 0..TILE {
                let c = f(x, y, &mut self.rng, &self.perlin);
                self.set(tile, x, y, c);
            }
        }
    }
    fn speckle(&mut self, tile: u16, base: Rgba, var: f32) {
        self.each(tile, |_, _, r, _| shade(base, 1.0 + r.range(-var, var)));
    }
    fn copy(&mut self, from: u16, to: u16) {
        for y in 0..TILE {
            for x in 0..TILE {
                let c = self.get(from, x, y);
                self.set(to, x, y, c);
            }
        }
    }
    /// Paint an ASCII sprite; '.' leaves the pixel untouched.
    fn sprite(&mut self, tile: u16, rows: &[&str], pal: &[(char, Rgba)]) {
        for (y, row) in rows.iter().enumerate() {
            for (x, ch) in row.chars().enumerate() {
                if let Some((_, c)) = pal.iter().find(|(k, _)| *k == ch) {
                    let jitter = 1.0 + (self.rng.f32() - 0.5) * 0.08;
                    self.set(tile, x, y, shade(*c, jitter));
                }
            }
        }
    }
    /// Voronoi-ish cells: returns (cell id, distance to edge) for tileable patterns.
    fn cells(points: &[(f32, f32)], x: usize, y: usize) -> (usize, f32) {
        let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
        let (mut d1, mut d2, mut id) = (f32::MAX, f32::MAX, 0);
        for (i, &(cx, cy)) in points.iter().enumerate() {
            for ox in [-16.0, 0.0, 16.0] {
                for oy in [-16.0, 0.0, 16.0] {
                    let d = ((px - cx - ox).powi(2) + (py - cy - oy).powi(2)).sqrt();
                    if d < d1 {
                        d2 = d1;
                        d1 = d;
                        id = i;
                    } else if d < d2 {
                        d2 = d;
                    }
                }
            }
        }
        (id, d2 - d1)
    }
    fn random_points(&mut self, n: usize) -> Vec<(f32, f32)> {
        (0..n).map(|_| (self.rng.range(0.0, 16.0), self.rng.range(0.0, 16.0))).collect()
    }
    fn ore(&mut self, tile: u16, color: Rgba, dark: Rgba) {
        self.copy(T_STONE, tile);
        for _ in 0..5 {
            let cx = self.rng.int(2, 13);
            let cy = self.rng.int(2, 13);
            for _ in 0..4 {
                let x = (cx + self.rng.int(-1, 1)).clamp(0, 15) as usize;
                let y = (cy + self.rng.int(-1, 1)).clamp(0, 15) as usize;
                let c = if self.rng.chance(0.3) { dark } else { color };
                self.set(tile, x, y, c);
                if x < 15 {
                    self.set(tile, x + 1, y, shade(color, 0.85));
                }
            }
        }
    }
}

const TNT_FONT: [&str; 5] = ["###.#..#.###", ".#..##.#..#.", ".#..#.##..#.", ".#..#..#..#.", ".#..#..#..#."];

const STICK: [&str; 16] = [
    "................",
    "................",
    "............##..",
    "...........#o#..",
    "..........#o#...",
    ".........#o#....",
    "........#o#.....",
    ".......#o#......",
    "......#o#.......",
    ".....#o#........",
    "....#o#.........",
    "...#o#..........",
    "..#o#...........",
    "..##............",
    "................",
    "................",
];

const PICK: [&str; 16] = [
    "................",
    "....hhhhhhh.....",
    "...hHHHHHHHh....",
    "..hHhhhhhhHHh...",
    "..hh.....#oHHh..",
    "........#o#hHh..",
    ".......#o#..hHh.",
    "......#o#....hh.",
    ".....#o#.....h..",
    "....#o#.........",
    "...#o#..........",
    "..#o#...........",
    ".#o#............",
    ".##.............",
    "................",
    "................",
];

const SWORD: [&str; 16] = [
    "................",
    ".............hh.",
    "............hHh.",
    "...........hHh..",
    "..........hHh...",
    ".........hHh....",
    "........hHh.....",
    "...gg..hHh......",
    "....gghHh.......",
    ".....gHh........",
    ".....#gg........",
    "....#o.gg.......",
    "...#o#..........",
    "..#o#...........",
    "..##............",
    "................",
];

const COAL_LUMP: [&str; 16] = [
    "................",
    "................",
    "................",
    "......####......",
    "....##kkgk##....",
    "...#kkkkkkgk#...",
    "...#kgkkkkkk#...",
    "..#kkkkkkgkkk#..",
    "..#kkkgkkkkkk#..",
    "..#kkkkkkkkgk#..",
    "...#kkkkgkkk#...",
    "...#kgkkkkkk#...",
    "....##kkkk##....",
    "......####......",
    "................",
    "................",
];

const INGOT: [&str; 16] = [
    "................",
    "................",
    "................",
    "................",
    "................",
    ".......######...",
    ".....##wwwwww#..",
    "...##wwwwwwll#..",
    ".##wwwwwwwlll#..",
    "#llwwwwwwllll#..",
    "#lllllllllll#...",
    "#dllllllllll#...",
    "#ddddlllll##....",
    ".####dddd##.....",
    ".....####.......",
    "................",
];

const GEM: [&str; 16] = [
    "................",
    "................",
    "................",
    "....########....",
    "...#wwccccll#...",
    "..#wwccccclll#..",
    ".#wccccccclllD#.",
    ".##############.",
    "..#ccccclllDD#..",
    "...#ccccllDD#...",
    "....#ccclDD#....",
    ".....#cclD#.....",
    "......#cD#......",
    ".......##.......",
    "................",
    "................",
];

const POWDER: [&str; 16] = [
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    ".......g........",
    "......gdg.......",
    ".....gdgdg..g...",
    "....gdgdgdg.....",
    "...gdgdgkgdg....",
    "..gdgkgdgdgdg...",
    ".gdgdgdgdgkgdg..",
    "..gggggggggggg..",
    "................",
    "................",
];

const PORK: [&str; 16] = [
    "................",
    "................",
    "................",
    ".....######.....",
    "...##pppppp##...",
    "..#ppPPPPPPpp#..",
    ".#pPPPPwwPPPPp#.",
    ".#pPPPwwwwPPPp#.",
    ".#pPPPPwwPPPPp#.",
    ".#ppPPPPPPPPpp#.",
    "..#ppPPPPPPpp#..",
    "...##pppppp##...",
    ".....######bb...",
    "...........bwb..",
    "............bb..",
    "................",
];

const GOO_BLOB: [&str; 16] = [
    "................",
    "................",
    "................",
    "................",
    "......####......",
    "....##gggg##....",
    "...#gwggggGg#...",
    "..#gwgggggGGg#..",
    "..#ggggggGGGg#..",
    "..#gggggGGGGg#..",
    "...#ggGGGGGg#...",
    "....##GGGG##....",
    "......####......",
    "..........G.....",
    "................",
    "................",
];

const HEART: [&str; 16] = [
    "................",
    "................",
    "................",
    "..####...####...",
    ".#rrrr#.#rrrr#..",
    "#rwwrrr#rrrrrr#.",
    "#rwrrrrrrrrrrr#.",
    "#rrrrrrrrrrrrd#.",
    ".#rrrrrrrrrrd#..",
    "..#rrrrrrrrd#...",
    "...#rrrrrrd#....",
    "....#rrrrd#.....",
    ".....#rrd#......",
    "......#d#.......",
    ".......#........",
    "................",
];

const HISSER_FACE: [&str; 16] = [
    "................",
    "................",
    "................",
    "................",
    "..####....####..",
    "..####....####..",
    "..####....####..",
    "..####....####..",
    "......####......",
    "......####......",
    "....########....",
    "....########....",
    "....##....##....",
    "....##....##....",
    "................",
    "................",
];

const PEARL: [&str; 16] = [
    "................",
    "................",
    "................",
    "......####......",
    "....##tttt##....",
    "...#ttwwtttt#...",
    "...#twwttttt#...",
    "..#tttttPPttt#..",
    "..#ttttPppPtt#..",
    "..#ttttPppPtt#..",
    "...#tttPPttt#...",
    "...#tttttttd#...",
    "....##tttdd#....",
    "......####......",
    "................",
    "................",
];

const TROPHY: [&str; 16] = [
    "................",
    "................",
    "..############..",
    ".#y#yyyywyyy#y#.",
    ".#y#yyyywyyy#y#.",
    ".#y#yyyyyyyy#y#.",
    "..##yyyyyyyy##..",
    "....#yyyyyy#....",
    ".....#yyyy#.....",
    "......#dd#......",
    "......#yy#......",
    ".....#yyyy#.....",
    "....########....",
    "....#dddddd#....",
    "....########....",
    "................",
];

const JACK_FACE: [&str; 16] = [
    "................",
    "................",
    "................",
    "...ff......ff...",
    "..ffff....ffff..",
    "..ffff....ffff..",
    "................",
    ".......ff.......",
    "......ffff......",
    "................",
    "..f..........f..",
    "..ff.ff..ff.ff..",
    "...ffffffffff...",
    "....ff.ff.ff....",
    "................",
    "................",
];

const FEATHER: [&str; 16] = [
    "................",
    "............ww..",
    "...........wWw..",
    "..........wWww..",
    ".........wWwwg..",
    "........wWwwg...",
    ".......wWwwg....",
    "......wWwwg.....",
    ".....wWwwg......",
    "....wWwwg.......",
    "....wWwg........",
    "...wwgg.........",
    "..#g............",
    ".#..............",
    "#...............",
    "................",
];

const DRUMSTICK: [&str; 16] = [
    "................",
    "................",
    ".....######.....",
    "....#pppPPp#....",
    "...#pPPPPPPp#...",
    "...#pPPwPPPp#...",
    "...#pPPPPPPp#...",
    "....#pPPPPp#....",
    ".....#pppp#.....",
    "......#pp#......",
    ".......#bb#.....",
    "........#bb#....",
    ".........bbbb...",
    "........bb..bb..",
    ".........b...b..",
    "................",
];

const BONE: [&str; 16] = [
    "................",
    "..##............",
    ".#ww#...........",
    ".#wwwd..........",
    "..#wwwd.........",
    "...dwwwd........",
    "....dwwwd.......",
    ".....dwwwd......",
    "......dwwwd.....",
    ".......dwwwd....",
    "........dwwwd...",
    ".........dwww#..",
    "..........wwww#.",
    "...........#ww#.",
    "............##..",
    "................",
];

const POINTY_STICK: [&str; 16] = [
    "................",
    "............sss.",
    "............sSs.",
    "...........#sss.",
    "..........#o#...",
    ".........#o#....",
    "........#o#.....",
    ".......#o#......",
    "......#o#.......",
    ".....#o#........",
    "..ff#o#.........",
    ".fffo#..........",
    "..fff...........",
    ".f.ff...........",
    "................",
    "................",
];

const STRING: [&str; 16] = [
    "................",
    "................",
    "..........ww....",
    "........ww..w...",
    ".......w.....w..",
    "......w.......w.",
    ".....w..........",
    "....w...........",
    "....w...........",
    ".....w..........",
    "......ww........",
    "........w.......",
    ".........w......",
    "..........w.....",
    "...........w....",
    "................",
];

const BOW: [&str; 16] = [
    "................",
    "..........###s..",
    "........##oo#s..",
    ".......#oo##.s..",
    "......#o#....s..",
    ".....#o#.....s..",
    ".....#o#.....s..",
    "....#o#......s..",
    "....#o#......s..",
    ".....#o#.....s..",
    ".....#o#.....s..",
    "......#o#....s..",
    ".......#oo##.s..",
    "........##oo#s..",
    "..........###s..",
    "................",
];

const HOE: [&str; 16] = [
    "................",
    "....hhhhhh......",
    "...hHHHHHHh.....",
    "..hHhhhh#oh.....",
    "........#o#.....",
    ".......#o#......",
    "......#o#.......",
    ".....#o#........",
    "....#o#.........",
    "...#o#..........",
    "..#o#...........",
    ".#o#............",
    ".##.............",
    "................",
    "................",
    "................",
];

const LOAF: [&str; 16] = [
    "................",
    "................",
    "................",
    "................",
    ".....######.....",
    "...##bbbbbb##...",
    "..#bbBbbBbbbb#..",
    ".#bbbbBbbbBbbb#.",
    ".#bBbbbbBbbbbb#.",
    ".#bbbbbbbbbbBb#.",
    ".#dbbbbbbbbbbd#.",
    "..#dddddddddd#..",
    "...##########...",
    "................",
    "................",
    "................",
];

const FISH: [&str; 16] = [
    "................",
    "................",
    "................",
    "................",
    "......####......",
    "....##ffffF#..##",
    "...#fwffffFF##F#",
    "..#fkwfffffFFFF#",
    "..#ffffffffFFF#.",
    "...#bbbbbbFF##F#",
    "....##bbbbF#..##",
    "......####......",
    "................",
    "................",
    "................",
    "................",
];

const HELMET: [&str; 16] = [
    "................",
    "................",
    "................",
    "....########....",
    "...#hhbbbbbb#...",
    "..#hbbbbbbbbd#..",
    "..#hbbbbbbbbd#..",
    "..#bbbbbbbbbd#..",
    "..#bbd####dbd#..",
    "..#bd#....#dd#..",
    "..#bd#....#dd#..",
    "..###......###..",
    "................",
    "................",
    "................",
    "................",
];

const CHESTPLATE: [&str; 16] = [
    "................",
    "..####....####..",
    ".#hbb#....#bbd#.",
    ".#hbbb####bbbd#.",
    ".#hbbbbbbbbbbd#.",
    ".###bbbbbbbbd###",
    "...#hbbbbbbbd#..",
    "...#hbbbbbbbd#..",
    "...#hbbbbbbbd#..",
    "...#hbbbbbbbd#..",
    "...#hbbbbbbbd#..",
    "...#bbbbbbbbd#..",
    "...#dddddddddd#.",
    "...###########..",
    "................",
    "................",
];

const LEGGINGS: [&str; 16] = [
    "................",
    "...##########...",
    "...#hbbbbbbd#...",
    "...#hbbbbbbd#...",
    "...#hbb##bbd#...",
    "...#hbb##bbd#...",
    "...#hbb##bbd#...",
    "...#hbd##hbd#...",
    "...#hbd##hbd#...",
    "...#hbd##hbd#...",
    "...#hbd##hbd#...",
    "...#hbd##hbd#...",
    "...#ddd##ddd#...",
    "...#####.####...",
    "................",
    "................",
];

const BOOTS: [&str; 16] = [
    "................",
    "................",
    "................",
    "................",
    "................",
    "..####....####..",
    "..#hb#....#bd#..",
    "..#hb#....#bd#..",
    "..#hb#....#bd#..",
    "..#hb#....#bd#..",
    ".#hbb#....#bbd#.",
    ".#hbb#....#bbd#.",
    ".#ddd#....#ddd#.",
    ".#####....#####.",
    "................",
    "................",
];

const DOOR_ITEM: [&str; 16] = [
    "....########....",
    "....#bbbbbb#....",
    "....#b#bb#b#....",
    "....#b#bb#b#....",
    "....#bbbbbb#....",
    "....#bdbbdb#....",
    "....#bbbbbb#....",
    "....#bbbbbw#....",
    "....#bbbbbw#....",
    "....#bdbbdb#....",
    "....#bbbbbb#....",
    "....#bdbbdb#....",
    "....#bbbbbb#....",
    "....#bdbbdb#....",
    "....#bbbbbb#....",
    "....########....",
];

const BOOT: [&str; 16] = [
    "................",
    "................",
    "....#######.....",
    "....#bbbbb#.....",
    "....#bdbdb#.....",
    "....#bbbbb#.....",
    "....#bdbdb#.....",
    "....#bbbbb#.....",
    "....#bbbbb#.....",
    "....#bbbbb####..",
    "....#bbbbbbbbb#.",
    "....#bbbbbbbbb#.",
    "....#ddddddddd#.",
    "....###########.",
    "......w....w....",
    "................",
];

const BOTTLE: [&str; 16] = [
    "................",
    "......##........",
    "......cc........",
    ".....#gg#.......",
    ".....#gg#.......",
    "....#gggg#......",
    "...#gggggg#.....",
    "...#gwppgg#.....",
    "...#gwpppg#.....",
    "...#ggpppg#.....",
    "...#ggpppg#.....",
    "...#gggggg#.....",
    "...#gggggg#.....",
    "....######......",
    "................",
    "................",
];

const BOWL: [&str; 16] = [
    "................",
    "................",
    "................",
    "................",
    "................",
    "....r..m.r......",
    "..############..",
    "..#ssmssrsssm#..",
    "..#wwwwwwwwww#..",
    "...#wwwwwwww#...",
    "....#wwwwww#....",
    ".....######.....",
    "................",
    "................",
    "................",
    "................",
];

const ROD: [&str; 16] = [
    "................",
    "............##s.",
    "...........#o#s.",
    "..........#o#.s.",
    ".........#o#..s.",
    "........#o#...s.",
    ".......#o#....s.",
    "......#o#.....s.",
    ".....#o#......s.",
    "....#o#.......s.",
    "...#o#........s.",
    "..#o#.........k.",
    ".#o#..........k.",
    ".##.............",
    "................",
    "................",
];

const PROBE: [&str; 16] = [
    "................",
    "...........###..",
    "..........#ggg#.",
    "..........#gwg#.",
    "..........#ggg#.",
    "...........#i#..",
    "..........#i#...",
    ".........#i#....",
    "........#o#.....",
    ".......#o#......",
    "......#o#.......",
    ".....#o#........",
    "....#o#.........",
    "...#i#..........",
    "...##...........",
    "................",
];

const CARROT: [&str; 16] = [
    "................",
    "...........g.g..",
    "..........ggg...",
    "..........gg.g..",
    ".........##g....",
    "........#oo#....",
    ".......#oOo#....",
    "......#oOoo#....",
    ".....#ooOo#.....",
    "....#oOoo#......",
    "...#ooo#........",
    "...#oo#.........",
    "..#o#...........",
    "..##............",
    "................",
    "................",
];

const SPUD: [&str; 16] = [
    "................",
    "................",
    "................",
    "................",
    ".....######.....",
    "...##bbbbbb##...",
    "..#bbdbbbbbbb#..",
    "..#bbbbbbbdbb#..",
    ".#bbbbbbbbbbbb#.",
    ".#bbbdbbbbbbbb#.",
    "..#bbbbbbbbdb#..",
    "...##bbbbbb##...",
    ".....######.....",
    "................",
    "................",
    "................",
];

const SHEAF: [&str; 16] = [
    "................",
    ".....y..y..y....",
    "....yYy.yY.yY...",
    ".....yYyYyYy....",
    "......yYyYy.....",
    ".......yYy......",
    "........g.......",
    "......bbbbb.....",
    "........g.......",
    ".......g.g......",
    "......g...g.....",
    ".....g.....g....",
    "....g.......g...",
    "................",
    "................",
    "................",
];

const WORM: [&str; 16] = [
    "................",
    "................",
    "................",
    "................",
    "................",
    "..........pp....",
    ".........p..p...",
    "..pp....p....p..",
    ".p..p..p.....k..",
    "p....pp.........",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
];

const COMPASS_SPRITE: [&str; 16] = [
    "................",
    ".....######.....",
    "...##iiiiii##...",
    "..#iwwwwwwwwi#..",
    "..#iwwwrwwwwi#..",
    ".#iwwwwrwwwwwi#.",
    ".#iwwwwrrwwwwi#.",
    ".#iwwwwkkwwwwi#.",
    ".#iwwwwkkwwwwi#.",
    ".#iwwwwwkwwwwi#.",
    "..#iwwwwkwwwi#..",
    "..#iwwwwwwwwi#..",
    "...##iiiiii##...",
    ".....######.....",
    "................",
    "................",
];

const MAP_SPRITE: [&str; 16] = [
    "................",
    "................",
    "..############..",
    "..#pppppppggp#..",
    "..#ppgggpppgp#..",
    "..#pggggppppp#..",
    "..#ppgppbbbpp#..",
    "..#pppbbbbbpp#..",
    "..#ppbbbbrppp#..",
    "..#pppbbppppp#..",
    "..#ppppppgggp#..",
    "..#pggppggggp#..",
    "..#pppppppppp#..",
    "..############..",
    "................",
    "................",
];

const BOAT_SPRITE: [&str; 16] = [
    "................",
    "................",
    "................",
    "................",
    "................",
    "#..............#",
    "##............##",
    "#w#..........#w#",
    "#ww##########ww#",
    ".#wwwwwwwwwwww#.",
    ".#dddddddddddd#.",
    "..#dddddddddd#..",
    "...##########...",
    "................",
    "................",
    "................",
];

const CART_SPRITE: [&str; 16] = [
    "................",
    "................",
    "................",
    "................",
    "..############..",
    "..#hbbbbbbbbh#..",
    "..#bbbbbbbbbb#..",
    "..#bbbbbbbbbb#..",
    "..#bbbbbbbbbb#..",
    "..#hbbbbbbbbh#..",
    "..############..",
    "...##......##...",
    "..#kk#....#kk#..",
    "...##......##...",
    "................",
    "................",
];

const SPARKER_SPRITE: [&str; 16] = [
    "................",
    "..........y.....",
    ".........yfy....",
    "..........f.....",
    "........#.......",
    ".......#i#......",
    "......#ii#......",
    ".....#ii#.......",
    "....#cc#........",
    "...#ccc#........",
    "..#ccc#.........",
    "..#cc#..........",
    "...##...........",
    "................",
    "................",
    "................",
];

const SHIELD_SPRITE: [&str; 16] = [
    "................",
    "...##########...",
    "..#iwwwwwwwwi#..",
    "..#wwwdwwdwww#..",
    "..#wwwdwwdwww#..",
    "..#iiiiiiiiii#..",
    "..#wwwdwwdwww#..",
    "..#wwwdwwdwww#..",
    "..#wwwdwwdwww#..",
    "...#wwdwwdww#...",
    "...#wwdwwdww#...",
    "....#wwwwww#....",
    ".....#iwwi#.....",
    "......####......",
    "................",
    "................",
];

const BOOK_SPRITE: [&str; 16] = [
    "................",
    "................",
    "..###########...",
    "..#ccccccccp#...",
    "..#ccccccccp#...",
    "..#ccgggcccp#...",
    "..#cccgccccp#...",
    "..#ccccccccp#...",
    "..#cccccgccp#...",
    "..#ccccgggcp#...",
    "..#ccccccccp#...",
    "..#ddddddddp#...",
    "..#dddddddpp#...",
    "..###########...",
    "................",
    "................",
];

const DUST_SPRITE: [&str; 16] = [
    "................",
    "................",
    "................",
    "........#.......",
    ".......#r#......",
    "....#.#rhr#.....",
    "...#r##rrr#.#...",
    "..#rhr#rr#.#r#..",
    "..#rrr#r#.#rhr#.",
    "...#r#.#..#rrr#.",
    "....#..#r##r#...",
    "......#rhr##....",
    ".....#rrrrr#....",
    "......#####.....",
    "................",
    "................",
];

const SHEARS_SPRITE: [&str; 16] = [
    "................",
    "...........##...",
    "..........#hb#..",
    ".........#hb#...",
    "..##....#hb#....",
    ".#hb#..#hb#.....",
    "..#hb##hb#......",
    "...#hbhb#.......",
    "....#bb#........",
    "...#r##r#.......",
    "..#r#..#r#......",
    ".#r#....#r#.....",
    ".#r#....#r#.....",
    "..##.....##.....",
    "................",
    "................",
];

const BUCKET_SPRITE: [&str; 16] = [
    "................",
    "................",
    "....########....",
    "...#........#...",
    "..#..........#..",
    "..##########.#..",
    "..#ffffffffff#..",
    "..#bhbbbbbbdb#..",
    "...#hbbbbbbd#...",
    "...#hbbbbbbd#...",
    "...#bbbbbbdd#...",
    "....#bbbbbd#....",
    "....#bbbbdd#....",
    "....########....",
    "................",
    "................",
];

pub fn build_atlas(seed: u64) -> Vec<u8> {
    let mut a = Atlas { px: vec![0u8; ATLAS * ATLAS * 4], rng: Rng::new(seed), perlin: Perlin::new(seed) };

    // Grass & dirt
    a.each(T_GRASS_TOP, |x, y, r, p| {
        let n = p.noise2(x as f32 * 0.4, y as f32 * 0.4) * 0.15;
        shade(rgb(94, 160, 58), 1.0 + n + r.range(-0.12, 0.12))
    });
    a.speckle(T_DIRT, rgb(134, 96, 67), 0.14);
    for i in 0..10 {
        let (x, y) = (a.rng.int(0, 15) as usize, a.rng.int(0, 15) as usize);
        let c = if i % 2 == 0 { rgb(110, 78, 54) } else { rgb(160, 120, 88) };
        a.set(T_DIRT, x, y, c);
    }
    a.copy(T_DIRT, T_GRASS_SIDE);
    a.copy(T_DIRT, T_SNOW_SIDE);
    for x in 0..TILE {
        let depth = 3 + a.rng.int(0, 2) as usize;
        for y in 0..depth {
            let g = shade(rgb(94, 160, 58), 1.0 + a.rng.range(-0.12, 0.12));
            a.set(T_GRASS_SIDE, x, y, g);
            let s = shade(rgb(240, 248, 255), 1.0 + a.rng.range(-0.05, 0.03));
            a.set(T_SNOW_SIDE, x, y, s);
        }
    }
    a.each(T_SNOW, |_, _, r, _| shade(rgb(240, 248, 255), 1.0 + r.range(-0.06, 0.03)));

    // Stone family
    a.each(T_STONE, |x, y, r, p| {
        let n = p.noise2(x as f32 * 0.35 + 50.0, y as f32 * 0.6) * 0.18;
        shade(rgb(125, 125, 125), 1.0 + n + r.range(-0.07, 0.07))
    });
    let pts = a.random_points(9);
    let shades: Vec<f32> = (0..9).map(|_| a.rng.range(0.8, 1.1)).collect();
    a.each(T_COBBLE, |x, y, r, _| {
        let (id, edge) = Atlas::cells(&pts, x, y);
        if edge < 1.2 { shade(rgb(70, 70, 70), r.range(0.9, 1.1)) } else { shade(rgb(130, 130, 130), shades[id] + r.range(-0.06, 0.06)) }
    });
    a.each(T_BEDROCK, |_, _, r, _| {
        let v = r.f32();
        if v < 0.4 { rgb(30, 30, 30) } else if v < 0.8 { rgb(75, 75, 75) } else { rgb(120, 120, 120) }
    });
    a.ore(T_COAL_ORE, rgb(35, 35, 35), rgb(15, 15, 15));
    a.ore(T_IRON_ORE, rgb(216, 175, 147), rgb(175, 128, 95));
    a.ore(T_DIAMOND_ORE, rgb(93, 236, 245), rgb(40, 180, 190));

    // Sand & gravel
    a.speckle(T_SAND, rgb(219, 207, 163), 0.07);
    let gpts = a.random_points(14);
    let gsh: Vec<f32> = (0..14).map(|_| a.rng.range(0.7, 1.2)).collect();
    a.each(T_GRAVEL, |x, y, r, _| {
        let (id, edge) = Atlas::cells(&gpts, x, y);
        let base = if id % 3 == 0 { rgb(140, 125, 120) } else { rgb(128, 128, 128) };
        shade(base, gsh[id] * if edge < 0.8 { 0.7 } else { 1.0 } + r.range(-0.05, 0.05))
    });

    // Water (translucent)
    a.each(T_WATER, |x, y, r, p| {
        let w = (p.noise2(x as f32 * 0.25, y as f32 * 0.5 + 3.0) * 6.0).sin() * 0.1;
        let mut c = shade(rgb(50, 95, 220), 1.0 + w + r.range(-0.03, 0.03));
        c[3] = 170;
        c
    });

    // Wood
    a.each(T_LOG_SIDE, |x, _y, r, _| {
        let stripe = if x % 4 == 0 { 0.75 } else if x % 4 == 2 { 0.9 } else { 1.0 };
        shade(rgb(104, 83, 50), stripe * r.range(0.9, 1.1))
    });
    a.each(T_LOG_TOP, |x, y, r, _| {
        let (dx, dy) = (x as f32 - 7.5, y as f32 - 7.5);
        let d = dx.abs().max(dy.abs());
        if d > 6.5 {
            shade(rgb(104, 83, 50), r.range(0.85, 1.05))
        } else {
            let ring = if (d as i32) % 2 == 0 { 1.0 } else { 0.85 };
            shade(rgb(176, 143, 90), ring * r.range(0.95, 1.05))
        }
    });
    a.each(T_PLANKS, |x, y, r, _| {
        let board = y / 4;
        let seam = y % 4 == 3 || (x + board * 5) % 16 == 0;
        if seam { rgb(110, 85, 50) } else { shade(rgb(170, 135, 82), r.range(0.9, 1.08) * if (x + y * 3) % 7 == 0 { 0.9 } else { 1.0 }) }
    });
    a.copy(T_PLANKS, T_TABLE_SIDE);
    a.copy(T_PLANKS, T_TABLE_TOP);
    for i in 0..16 {
        a.set(T_TABLE_TOP, i, 0, rgb(90, 65, 38));
        a.set(T_TABLE_TOP, i, 15, rgb(90, 65, 38));
        a.set(T_TABLE_TOP, 0, i, rgb(90, 65, 38));
        a.set(T_TABLE_TOP, 15, i, rgb(90, 65, 38));
        if (3..13).contains(&i) {
            a.set(T_TABLE_TOP, i, 7, rgb(90, 65, 38));
            a.set(T_TABLE_TOP, 7, i, rgb(90, 65, 38));
        }
    }
    for y in 3..9 {
        a.set(T_TABLE_SIDE, 3, y, rgb(160, 160, 160)); // a saw blade, allegedly
        a.set(T_TABLE_SIDE, 11, y, rgb(90, 60, 30));
        a.set(T_TABLE_SIDE, 12, y + 1, rgb(90, 60, 30));
    }
    for x in 2..6 {
        a.set(T_TABLE_SIDE, x, 2, rgb(200, 200, 200));
    }

    a.each(T_LEAVES, |_, _, r, _| {
        // Holes keep the leaf colour (alpha 0) so mipmaps don't blend in black fringes.
        if r.chance(0.22) { [58, 130, 40, 0] } else { shade(rgb(58, 130, 40), r.range(0.7, 1.15)) }
    });
    a.each(T_GLASS, |x, y, r, _| {
        let edge = x == 0 || y == 0 || x == 15 || y == 15;
        let glint = (x + y == 9 || x + y == 11) && (3..9).contains(&x);
        if edge { shade(rgb(200, 225, 235), r.range(0.9, 1.0)) } else if glint { [255, 255, 255, 200] } else { [0, 0, 0, 0] }
    });

    // Bricks
    a.each(T_BRICK, |x, y, r, _| {
        let row = y / 4;
        let off = if row % 2 == 0 { 0 } else { 4 };
        if y % 4 == 3 || (x + off) % 8 == 7 { shade(rgb(190, 180, 170), r.range(0.9, 1.05)) } else { shade(rgb(150, 70, 55), r.range(0.85, 1.1)) }
    });

    // TNT
    a.each(T_TNT_SIDE, |_, y, r, _| {
        if (5..11).contains(&y) { shade(rgb(235, 235, 235), r.range(0.95, 1.02)) } else { shade(rgb(200, 50, 30), r.range(0.85, 1.1)) }
    });
    for (row, line) in TNT_FONT.iter().enumerate() {
        for (col, ch) in line.chars().enumerate() {
            if ch == '#' {
                a.set(T_TNT_SIDE, col + 2, row + 5, rgb(30, 30, 30));
            }
        }
    }
    a.each(T_TNT_BOTTOM, |_, _, r, _| shade(rgb(200, 50, 30), r.range(0.8, 1.05)));
    a.copy(T_TNT_BOTTOM, T_TNT_TOP);
    for y in 5..11 {
        for x in 5..11 {
            a.set(T_TNT_TOP, x, y, rgb(90, 90, 90));
        }
    }
    a.set(T_TNT_TOP, 7, 7, rgb(40, 40, 40));
    a.set(T_TNT_TOP, 8, 8, rgb(40, 40, 40));

    // Glowrock
    let lpts = a.random_points(8);
    a.each(T_GLOW, |x, y, r, _| {
        let (id, edge) = Atlas::cells(&lpts, x, y);
        if edge < 1.0 { rgb(140, 100, 40) } else { shade(if id % 2 == 0 { rgb(255, 230, 140) } else { rgb(250, 200, 90) }, r.range(0.9, 1.1)) }
    });

    // Cross-model plants and torch (transparent background)
    a.each(T_TORCH, |x, y, r, _| {
        if (7..9).contains(&x) && y >= 6 {
            shade(rgb(120, 90, 50), r.range(0.8, 1.1))
        } else if (7..9).contains(&x) && (3..6).contains(&y) {
            if y == 3 { rgb(255, 250, 200) } else { rgb(255, 200, 60) }
        } else {
            [0, 0, 0, 0]
        }
    });
    a.each(T_FLOWER, |x, y, r, _| {
        if x == 7 && y >= 7 {
            rgb(50, 120, 30)
        } else if (x == 5 || x == 6) && y == 10 {
            rgb(60, 140, 35)
        } else if (5..10).contains(&x) && (2..7).contains(&y) && !((x == 5 || x == 9) && (y == 2 || y == 6)) {
            if x == 7 && y == 4 { rgb(40, 30, 10) } else { shade(rgb(220, 30, 30), r.range(0.85, 1.1)) }
        } else {
            [0, 0, 0, 0]
        }
    });
    for x in 0..16 {
        for y in 0..16 {
            a.set(T_TALLGRASS, x, y, [0, 0, 0, 0]);
        }
    }
    for blade in 0..7 {
        let bx = 1 + blade * 2 + a.rng.int(0, 1) as usize;
        let top = a.rng.int(3, 9) as usize;
        for y in top..16 {
            let lean = ((16 - y) as f32 * a.rng.range(-0.15, 0.15)) as i32;
            let x = (bx as i32 + lean).clamp(0, 15) as usize;
            let c = shade(rgb(94, 160, 58), a.rng.range(0.7, 1.1));
            a.set(T_TALLGRASS, x, y, c);
        }
    }

    // Breaking cracks, 5 stages, each builds on the last
    let mut cracks: Vec<(usize, usize)> = Vec::new();
    let mut walkers: Vec<(i32, i32)> = vec![(8, 8)];
    for stage in 0..5u16 {
        for _ in 0..14 {
            let wi = a.rng.int(0, walkers.len() as i32 - 1) as usize;
            let (mut x, mut y) = walkers[wi];
            x = (x + a.rng.int(-1, 1)).clamp(0, 15);
            y = (y + a.rng.int(-1, 1)).clamp(0, 15);
            walkers[wi] = (x, y);
            if a.rng.chance(0.2) {
                walkers.push((x, y));
            }
            cracks.push((x as usize, y as usize));
        }
        let t = T_CRACK0 + stage;
        for x in 0..16 {
            for y in 0..16 {
                a.set(t, x, y, [0, 0, 0, 0]);
            }
        }
        for &(x, y) in &cracks {
            a.set(t, x, y, [20, 20, 20, 210]);
        }
    }

    // Items
    let wood = (rgb(137, 103, 39), rgb(104, 78, 30));
    a.sprite(T_STICK, &STICK, &[('#', rgb(73, 54, 21)), ('o', rgb(137, 103, 39))]);
    let tiers = [wood, (rgb(150, 150, 150), rgb(90, 90, 90)), (rgb(235, 235, 235), rgb(160, 160, 160)), (rgb(90, 240, 225), rgb(30, 160, 150))];
    for (i, (light, dark)) in tiers.iter().enumerate() {
        let pal = [('#', rgb(73, 54, 21)), ('o', rgb(137, 103, 39)), ('H', *light), ('h', *dark), ('g', rgb(73, 54, 21))];
        a.sprite(T_PICK0 + i as u16, &PICK, &pal);
        a.sprite(T_SWORD0 + i as u16, &SWORD, &pal);
    }
    a.sprite(T_COAL, &COAL_LUMP, &[('#', rgb(20, 20, 20)), ('k', rgb(45, 45, 45)), ('g', rgb(90, 90, 90))]);
    a.sprite(T_IRON, &INGOT, &[('#', rgb(60, 60, 60)), ('w', rgb(250, 250, 250)), ('l', rgb(210, 210, 210)), ('d', rgb(150, 150, 150))]);
    a.sprite(T_DIAMOND, &GEM, &[('#', rgb(15, 70, 70)), ('w', rgb(230, 255, 255)), ('c', rgb(90, 240, 225)), ('l', rgb(50, 200, 190)), ('D', rgb(25, 140, 135))]);
    a.sprite(T_GUNPOWDER, &POWDER, &[('g', rgb(110, 110, 110)), ('d', rgb(70, 70, 70)), ('k', rgb(40, 40, 40))]);
    a.sprite(T_PORK, &PORK, &[('#', rgb(90, 30, 30)), ('p', rgb(230, 110, 110)), ('P', rgb(245, 150, 150)), ('w', rgb(255, 225, 225)), ('b', rgb(230, 225, 200))]);
    a.sprite(T_GOO, &GOO_BLOB, &[('#', rgb(30, 70, 20)), ('g', rgb(110, 190, 60)), ('G', rgb(70, 140, 40)), ('w', rgb(210, 255, 180))]);
    a.sprite(T_HEART, &HEART, &[('#', rgb(30, 0, 0)), ('r', rgb(220, 20, 30)), ('w', rgb(255, 180, 180)), ('d', rgb(150, 10, 20))]);
    a.sprite(T_HEART_EMPTY, &HEART, &[('#', rgb(30, 0, 0)), ('r', rgb(60, 25, 25)), ('w', rgb(60, 25, 25)), ('d', rgb(60, 25, 25))]);
    a.copy(T_HEART_EMPTY, T_HALF_HEART);
    let half: Vec<&str> = HEART.iter().map(|r| &r[..8]).collect();
    a.sprite(T_HALF_HEART, &half, &[('#', rgb(30, 0, 0)), ('r', rgb(220, 20, 30)), ('w', rgb(255, 180, 180)), ('d', rgb(150, 10, 20))]);

    // Mobs
    a.speckle(T_PIG_SKIN, rgb(240, 160, 160), 0.06);
    a.copy(T_PIG_SKIN, T_PIG_FACE);
    for (x, y, c) in [(3, 5, rgb(255, 255, 255)), (4, 5, rgb(20, 20, 20)), (11, 5, rgb(20, 20, 20)), (12, 5, rgb(255, 255, 255))] {
        a.set(T_PIG_FACE, x, y, c);
        a.set(T_PIG_FACE, x, y + 1, c);
    }
    for y in 8..12 {
        for x in 5..11 {
            let nostril = y > 8 && y < 11 && (x == 6 || x == 9);
            a.set(T_PIG_FACE, x, y, if nostril { rgb(120, 60, 60) } else { rgb(250, 185, 185) });
        }
    }
    a.each(T_HISSER_SKIN, |_, _, r, _| {
        let v = r.f32();
        if v < 0.15 { rgb(170, 230, 160) } else if v < 0.5 { rgb(70, 170, 60) } else { rgb(90, 200, 75) }
    });
    a.copy(T_HISSER_SKIN, T_HISSER_FACE);
    a.sprite(T_HISSER_FACE, &HISSER_FACE, &[('#', rgb(20, 30, 20))]);
    a.speckle(T_GROAN_SKIN, rgb(80, 140, 90), 0.08);
    a.copy(T_GROAN_SKIN, T_GROAN_FACE);
    for x in [3usize, 4, 11, 12] {
        a.set(T_GROAN_FACE, x, 7, rgb(20, 20, 20));
        a.set(T_GROAN_FACE, x, 8, rgb(20, 20, 20));
    }
    for x in 5..11 {
        a.set(T_GROAN_FACE, x, 12, rgb(40, 60, 40));
    }
    a.speckle(T_GROAN_SHIRT, rgb(40, 140, 160), 0.1);
    a.speckle(T_GROAN_PANTS, rgb(70, 60, 150), 0.1);
    a.speckle(T_SKIN, rgb(200, 150, 110), 0.05);
    a.copy(T_SKIN, T_STOVE_FACE);
    for y in 0..4 {
        for x in 0..16 {
            a.set(T_STOVE_FACE, x, y, rgb(60, 40, 20));
        }
    }
    for (x, c) in [(3usize, rgb(255, 255, 255)), (4, rgb(60, 60, 170)), (11, rgb(60, 60, 170)), (12, rgb(255, 255, 255))] {
        a.set(T_STOVE_FACE, x, 8, c);
    }
    for x in 6..10 {
        a.set(T_STOVE_FACE, x, 12, rgb(120, 70, 50));
    }
    a.speckle(T_STOVE_SHIRT, rgb(60, 170, 170), 0.08);
    a.speckle(T_STOVE_PANTS, rgb(60, 60, 150), 0.08);
    // Player skins (see nametags.rs): the same face, different people.
    for (k, (_, tone, hair, shirt, pants)) in crate::nametags::SKINS.iter().enumerate() {
        let [t_tone, t_face, t_shirt, t_pants] = crate::nametags::skin_tiles(k as u16);
        let c = |v: [u8; 3]| rgb(v[0], v[1], v[2]);
        a.speckle(t_tone, c(*tone), 0.05);
        a.copy(t_tone, t_face);
        for y in 0..4 {
            for x in 0..16 {
                a.set(t_face, x, y, c(*hair));
            }
        }
        for (x, e) in [(3usize, rgb(255, 255, 255)), (4, rgb(60, 60, 170)), (11, rgb(60, 60, 170)), (12, rgb(255, 255, 255))] {
            a.set(t_face, x, 8, e);
        }
        for x in 6..10 {
            a.set(t_face, x, 12, rgb(120, 70, 50));
        }
        a.speckle(t_shirt, c(*shirt), 0.08);
        a.speckle(t_pants, c(*pants), 0.08);
    }
    a.each(T_NAME_TAG, |x, y, r, _| {
        let tag = (3..14).contains(&x) && (5..12).contains(&y) && !(x == 3 && (y == 5 || y == 11));
        let hole = x == 5 && y == 8;
        let string = (x == 1 || x == 2) && (y == 8 || y == 7);
        if string { rgb(230, 230, 230) } else if hole { [0, 0, 0, 0] } else if tag { shade(rgb(215, 190, 140), r.range(0.9, 1.05)) } else { [0, 0, 0, 0] }
    });

    // ---- The "More Parody" update
    a.ore(T_GOLD_ORE, rgb(252, 220, 70), rgb(200, 150, 30));

    a.each(T_PUMPKIN_SIDE, |x, _, r, _| {
        let groove = x % 4 == 0;
        shade(rgb(224, 128, 28), r.range(0.9, 1.05) * if groove { 0.78 } else { 1.0 })
    });
    a.each(T_PUMPKIN_TOP, |x, y, r, _| {
        let (dx, dy) = (x as f32 - 7.5, y as f32 - 7.5);
        if dx.abs() < 1.5 && dy.abs() < 1.5 {
            shade(rgb(90, 70, 30), r.range(0.9, 1.1)) // the stalk
        } else {
            shade(rgb(214, 120, 26), r.range(0.88, 1.05) * (1.0 - (dx * dx + dy * dy).sqrt() * 0.015))
        }
    });
    a.copy(T_PUMPKIN_SIDE, T_JACK_FACE);
    a.sprite(T_JACK_FACE, &JACK_FACE, &[('f', rgb(255, 230, 90))]);

    a.each(T_CACTUS_SIDE, |x, y, r, _| {
        let spine = (x == 2 || x == 8 || x == 13) && y % 4 == 1;
        if spine {
            rgb(230, 230, 190)
        } else {
            let rib = if x % 5 == 0 { 0.8 } else { 1.0 };
            shade(rgb(60, 140, 50), rib * r.range(0.9, 1.1))
        }
    });
    a.each(T_CACTUS_TOP, |x, y, r, _| {
        let edge = x == 0 || y == 0 || x == 15 || y == 15;
        shade(if edge { rgb(45, 110, 40) } else { rgb(90, 170, 70) }, r.range(0.9, 1.08))
    });

    a.each(T_ICE, |x, y, r, _| {
        let streak = (x + 16 - y) % 11 == 0 || (x + 16 - y) % 11 == 1;
        shade(if streak { rgb(215, 235, 255) } else { rgb(150, 190, 245) }, r.range(0.95, 1.04))
    });

    a.each(T_BOUNCY, |x, y, r, _| {
        let edge = x == 0 || y == 0 || x == 15 || y == 15;
        let inner = (4..12).contains(&x) && (4..12).contains(&y);
        let c = if edge { rgb(70, 150, 45) } else if inner { rgb(100, 200, 70) } else { rgb(130, 220, 95) };
        shade(c, r.range(0.94, 1.06))
    });
    a.set(T_BOUNCY, 5, 5, rgb(220, 255, 200));
    a.set(T_BOUNCY, 6, 5, rgb(220, 255, 200));

    a.each(T_BED_TOP, |x, y, r, _| {
        if y < 5 {
            if x == 0 || x == 15 || y == 0 { rgb(210, 210, 210) } else { shade(rgb(245, 245, 245), r.range(0.95, 1.02)) } // pillow
        } else {
            shade(rgb(180, 30, 35), r.range(0.88, 1.05) * if (x + y) % 6 == 0 { 0.85 } else { 1.0 })
        }
    });
    // Turned copies for beds facing the other ways (the top's +y is north).
    for (t, turn) in [(T_BED_TOP_E, 1), (T_BED_TOP_S, 2), (T_BED_TOP_W, 3)] {
        for y in 0..16usize {
            for x in 0..16usize {
                // The pixel this one comes from in the north-facing top.
                let (sx, sy) = match turn {
                    1 => (y, 15 - x),
                    2 => (15 - x, 15 - y),
                    _ => (15 - y, x),
                };
                let c = a.get(T_BED_TOP, sx, sy);
                a.set(t, x, y, c);
            }
        }
    }
    // The side: the mattress's part (rows 7 to 13) is a white sheet under a
    // red blanket; the rest is the wooden frame.
    a.copy(T_PLANKS, T_BED_SIDE);
    for y in 7..13 {
        for x in 0..16 {
            let c = if y == 12 { shade(rgb(240, 240, 240), a.rng.range(0.94, 1.02)) } else { shade(rgb(180, 30, 35), a.rng.range(0.88, 1.05) * if y == 7 { 0.85 } else { 1.0 }) };
            a.set(T_BED_SIDE, x, y, c);
        }
    }

    a.each(T_CAKE_TOP, |x, y, r, _| {
        let cherry = (x == 7 || x == 8) && (y == 7 || y == 8);
        let sprinkle = (x * 7 + y * 13) % 17 == 0;
        if cherry {
            rgb(220, 20, 40)
        } else if sprinkle {
            [rgb(255, 90, 150), rgb(90, 200, 255), rgb(255, 230, 80)][(x + y) % 3]
        } else {
            shade(rgb(250, 245, 240), r.range(0.95, 1.02))
        }
    });
    a.each(T_CAKE_SIDE, |x, y, r, _| {
        let drip = y < 3 || (y < 5 && (x * 5) % 7 < 3);
        if drip {
            shade(rgb(250, 245, 240), r.range(0.95, 1.02))
        } else if y == 9 {
            rgb(200, 40, 60) // jam layer
        } else {
            shade(rgb(190, 120, 70), r.range(0.9, 1.06))
        }
    });

    a.each(T_SPONGE, |x, y, r, p| {
        let hole = p.noise2(x as f32 * 0.7 + 20.0, y as f32 * 0.7) > 0.3;
        shade(rgb(215, 200, 70), r.range(0.92, 1.05) * if hole { 0.7 } else { 1.0 })
    });

    a.each(T_WOOL, |x, y, r, p| {
        let curl = (p.noise2(x as f32 * 0.9, y as f32 * 0.9 + 40.0) * 3.0).sin() * 0.06;
        shade(rgb(236, 236, 230), 1.0 + curl + r.range(-0.05, 0.03))
    });

    a.sprite(T_GOLD, &INGOT, &[('#', rgb(110, 80, 10)), ('w', rgb(255, 250, 180)), ('l', rgb(250, 215, 60)), ('d', rgb(200, 150, 30))]);
    a.sprite(T_GOLD_CHOP, &PORK, &[('#', rgb(110, 80, 10)), ('p', rgb(240, 190, 40)), ('P', rgb(255, 225, 90)), ('w', rgb(255, 255, 210)), ('b', rgb(255, 245, 190))]);
    a.sprite(T_MUTTON, &PORK, &[('#', rgb(80, 20, 20)), ('p', rgb(200, 60, 60)), ('P', rgb(225, 90, 85)), ('w', rgb(250, 235, 220)), ('b', rgb(230, 225, 200))]);
    a.sprite(T_PEARL, &PEARL, &[('#', rgb(10, 40, 40)), ('t', rgb(30, 110, 100)), ('w', rgb(170, 240, 225)), ('P', rgb(20, 70, 60)), ('p', rgb(5, 25, 25)), ('d', rgb(20, 75, 70))]);
    a.sprite(T_TROPHY, &TROPHY, &[('#', rgb(90, 60, 10)), ('y', rgb(250, 205, 50)), ('w', rgb(255, 250, 200)), ('d', rgb(120, 80, 40))]);

    // Fluffer (legally distinct sheep) and Starer (legally distinct tall stranger)
    a.speckle(T_FLUFF_SKIN, rgb(225, 200, 170), 0.06);
    a.copy(T_FLUFF_SKIN, T_FLUFF_FACE);
    for (x, y, c) in [(3, 6, rgb(255, 255, 255)), (4, 6, rgb(30, 30, 30)), (11, 6, rgb(30, 30, 30)), (12, 6, rgb(255, 255, 255))] {
        a.set(T_FLUFF_FACE, x, y, c);
    }
    for x in 6..10 {
        a.set(T_FLUFF_FACE, x, 11, rgb(200, 150, 150));
    }
    for x in 0..16 {
        for y in 0..3 {
            let c = shade(rgb(236, 236, 230), a.rng.range(0.92, 1.02));
            a.set(T_FLUFF_FACE, x, y, c);
        }
    }
    a.speckle(T_STARER_SKIN, rgb(22, 18, 28), 0.15);
    a.copy(T_STARER_SKIN, T_STARER_FACE);
    for x in [2usize, 3, 4, 11, 12, 13] {
        let c = if x == 3 || x == 12 { rgb(250, 200, 255) } else { rgb(200, 80, 240) };
        a.set(T_STARER_FACE, x, 8, c);
    }

    // ---- The "More Mobs" update
    a.each(T_CLUCK_BODY, |x, y, r, _| shade(rgb(245, 245, 240), r.range(0.9, 1.03) * if (x + 2 * y) % 5 == 0 { 0.93 } else { 1.0 }));
    a.copy(T_CLUCK_BODY, T_CLUCK_FACE);
    for (x, y, c) in [(3, 4, rgb(20, 20, 20)), (12, 4, rgb(20, 20, 20))] {
        a.set(T_CLUCK_FACE, x, y, c);
        a.set(T_CLUCK_FACE, x, y + 1, c);
    }
    for y in 6..10 {
        for x in 5..11 {
            a.set(T_CLUCK_FACE, x, y, if y < 8 { rgb(240, 170, 40) } else { rgb(220, 140, 30) }); // beak
        }
    }
    for y in 10..14 {
        for x in 6..10 {
            a.set(T_CLUCK_FACE, x, y, rgb(210, 30, 30)); // wattle
        }
    }
    a.speckle(T_CLUCK_LEG, rgb(235, 180, 50), 0.08);

    let cow_pts = a.random_points(7);
    a.each(T_MOO_SKIN, |x, y, r, _| {
        let (id, _) = Atlas::cells(&cow_pts, x, y);
        shade(if id % 3 == 0 { rgb(35, 30, 30) } else { rgb(240, 240, 235) }, r.range(0.92, 1.04))
    });
    a.copy(T_MOO_SKIN, T_MOO_FACE);
    // Eyes, each on a patch of white so they don't vanish into a black spot.
    for x0 in [1usize, 10] {
        for y in 4..8 {
            for x in x0..x0 + 5 {
                a.set(T_MOO_FACE, x, y, rgb(240, 240, 235));
            }
        }
    }
    for x in [3usize, 4, 11, 12] {
        a.set(T_MOO_FACE, x, 5, rgb(20, 20, 20));
        a.set(T_MOO_FACE, x, 6, rgb(20, 20, 20));
    }
    for y in 9..15 {
        for x in 3..13 {
            let nostril = y == 11 && (x == 5 || x == 10);
            a.set(T_MOO_FACE, x, y, if nostril { rgb(90, 50, 50) } else { rgb(230, 170, 160) }); // muzzle
        }
    }

    a.each(T_BONE, |x, y, r, _| shade(rgb(215, 215, 205), r.range(0.9, 1.05) * if (x * 3 + y) % 7 == 0 { 0.85 } else { 1.0 }));
    a.copy(T_BONE, T_RATTLER_FACE);
    for (x0, y0) in [(2usize, 5usize), (10, 5)] {
        for y in y0..y0 + 3 {
            for x in x0..x0 + 4 {
                a.set(T_RATTLER_FACE, x, y, rgb(25, 25, 25)); // eye sockets
            }
        }
    }
    a.set(T_RATTLER_FACE, 7, 9, rgb(40, 40, 40));
    a.set(T_RATTLER_FACE, 8, 9, rgb(40, 40, 40));
    for x in (3..13).step_by(2) {
        a.set(T_RATTLER_FACE, x, 12, rgb(40, 40, 40)); // teeth gaps
    }

    a.each(T_WEB_SKIN, |_, _, r, _| {
        let v = r.f32();
        if v < 0.15 { rgb(70, 55, 45) } else if v < 0.6 { rgb(45, 35, 30) } else { rgb(30, 25, 22) }
    });
    a.copy(T_WEB_SKIN, T_WEB_FACE);
    for (x, y) in [(3usize, 6usize), (4, 6), (11, 6), (12, 6), (5, 4), (10, 4), (6, 8), (9, 8)] {
        a.set(T_WEB_FACE, x, y, rgb(220, 20, 20)); // so many eyes
    }

    a.each(T_BLOOP, |x, y, r, _| {
        let edge = x == 0 || y == 0 || x == 15 || y == 15;
        shade(if edge { rgb(60, 150, 60) } else { rgb(110, 200, 100) }, r.range(0.94, 1.05))
    });
    a.copy(T_BLOOP, T_BLOOP_FACE);
    for (x, y) in [(3usize, 5usize), (4, 5), (3, 6), (4, 6), (11, 5), (12, 5), (11, 6), (12, 6), (7, 10), (8, 10)] {
        a.set(T_BLOOP_FACE, x, y, rgb(25, 60, 25));
    }

    a.sprite(T_FEATHER, &FEATHER, &[('#', rgb(90, 90, 90)), ('w', rgb(245, 245, 245)), ('W', rgb(255, 255, 255)), ('g', rgb(200, 200, 200))]);
    a.sprite(T_CLUCKETS, &DRUMSTICK, &[('#', rgb(110, 60, 40)), ('p', rgb(240, 170, 150)), ('P', rgb(250, 195, 175)), ('w', rgb(255, 235, 225)), ('b', rgb(235, 230, 210))]);
    a.sprite(T_MOO_STEAK, &PORK, &[('#', rgb(70, 15, 15)), ('p', rgb(180, 40, 40)), ('P', rgb(210, 60, 55)), ('w', rgb(250, 230, 220)), ('b', rgb(230, 225, 200))]);
    a.sprite(T_BONE_ITEM, &BONE, &[('#', rgb(120, 120, 110)), ('w', rgb(240, 240, 230)), ('d', rgb(190, 190, 180))]);
    a.sprite(T_ARROW, &POINTY_STICK, &[('#', rgb(73, 54, 21)), ('o', rgb(137, 103, 39)), ('s', rgb(150, 150, 150)), ('S', rgb(220, 220, 220)), ('f', rgb(245, 245, 245))]);
    a.sprite(T_STRING, &STRING, &[('w', rgb(235, 235, 235))]);
    a.sprite(T_BOW, &BOW, &[('#', rgb(73, 54, 21)), ('o', rgb(137, 103, 39)), ('s', rgb(230, 230, 230))]);

    // ---- The "Farming, Fishing and Fancy Blocks" update
    a.each(T_SANDSTONE, |_, y, r, _| {
        let band = if y % 5 == 4 { 0.86 } else { 1.0 };
        shade(rgb(222, 205, 150), band * r.range(0.95, 1.04))
    });
    a.speckle(T_SANDSTONE_TOP, rgb(225, 210, 155), 0.05);
    a.each(T_STONE_BRICKS, |x, y, r, _| {
        let off = if (y / 8) % 2 == 0 { 0 } else { 8 };
        let mortar = y % 8 == 7 || (x + off) % 16 == 15;
        if mortar { shade(rgb(90, 90, 90), r.range(0.9, 1.05)) } else { shade(rgb(135, 135, 135), r.range(0.92, 1.05)) }
    });
    a.copy(T_COBBLE, T_MOSSY);
    for _ in 0..70 {
        let (x, y) = (a.rng.int(0, 15) as usize, a.rng.int(0, 15) as usize);
        let c = shade(rgb(80, 130, 50), a.rng.range(0.8, 1.15));
        a.set(T_MOSSY, x, y, c);
    }
    a.each(T_HAY_SIDE, |x, y, r, _| {
        let tie = (3..5).contains(&y) || (11..13).contains(&y);
        if tie { shade(rgb(130, 70, 30), r.range(0.9, 1.05)) } else { shade(rgb(215, 185, 70), r.range(0.85, 1.08) * if x % 3 == 0 { 0.9 } else { 1.0 }) }
    });
    a.each(T_HAY_TOP, |x, y, r, _| {
        let d = ((x as f32 - 7.5).powi(2) + (y as f32 - 7.5).powi(2)).sqrt();
        shade(rgb(205, 175, 65), r.range(0.85, 1.05) * if (d as i32) % 3 == 0 { 0.88 } else { 1.0 })
    });
    a.copy(T_PLANKS, T_BOOKSHELF);
    let spines = [rgb(150, 40, 40), rgb(40, 80, 140), rgb(50, 110, 50), rgb(170, 140, 60), rgb(100, 50, 110)];
    for shelf in [1usize, 9] {
        let mut x = 1;
        while x < 15 {
            let w = 1 + a.rng.int(0, 1) as usize;
            let c = spines[a.rng.int(0, 4) as usize];
            let top = shelf + a.rng.int(0, 1) as usize;
            for bx in x..(x + w).min(15) {
                for y in top..shelf + 6 {
                    let cc = shade(c, a.rng.range(0.85, 1.05));
                    a.set(T_BOOKSHELF, bx, y, cc);
                }
            }
            x += w + 1;
        }
    }
    a.each(T_LANTERN, |x, y, r, _| {
        let frame = x <= 1 || x >= 14 || y <= 1 || y >= 14 || x == 7 || x == 8;
        if frame { shade(rgb(55, 55, 65), r.range(0.9, 1.1)) } else { shade(rgb(255, 205, 110), r.range(0.9, 1.05)) }
    });
    a.each(T_MUSHROOM, |x, y, _, _| {
        let cap = (3..9).contains(&y) && (3..13).contains(&x) && !(y == 3 && (x == 3 || x == 12));
        let stalk = (9..16).contains(&y) && (6..10).contains(&x);
        if cap { if (x + y) % 5 == 0 { rgb(250, 240, 230) } else { rgb(200, 40, 35) } } else if stalk { rgb(235, 225, 205) } else { [0, 0, 0, 0] }
    });
    a.copy(T_PUMPKIN_SIDE, T_SCARECROW);
    for (x, y) in [(4usize, 5usize), (5, 5), (10, 5), (11, 5), (7, 8), (8, 8)] {
        a.set(T_SCARECROW, x, y, rgb(30, 20, 10));
    }
    for x in 4..12 {
        a.set(T_SCARECROW, x, 11, rgb(30, 20, 10)); // a flat, unconvincing smile
    }
    for x in 0..16 {
        for y in 0..16 {
            a.set(T_WEEDS, x, y, [0, 0, 0, 0]);
        }
    }
    for blade in 0..9 {
        let bx = 1 + blade + a.rng.int(0, 5) as usize;
        let top = a.rng.int(5, 11) as usize;
        for y in top..16 {
            let lean = ((16 - y) as f32 * a.rng.range(-0.3, 0.3)) as i32;
            let x = (bx as i32 + lean).clamp(0, 15) as usize;
            let c = shade(rgb(110, 125, 40), a.rng.range(0.7, 1.1));
            a.set(T_WEEDS, x, y, c);
        }
    }
    for (tile, base) in [(T_FARMLAND, rgb(120, 85, 55)), (T_FARMLAND_WET, rgb(75, 50, 32))] {
        a.each(tile, |_, y, r, _| {
            let furrow = y % 4 == 0;
            shade(base, r.range(0.85, 1.1) * if furrow { 0.72 } else { 1.0 })
        });
    }

    // Crops: four stages each, drawn as little plants on a clear background.
    for t in T_CROP_WHEAT..T_CROP_POTATO + 4 {
        for x in 0..16 {
            for y in 0..16 {
                a.set(t, x, y, [0, 0, 0, 0]);
            }
        }
    }
    for stage in 0..4u16 {
        let height = 4 + stage as usize * 3; // pixels tall
        // Wheat: thin stalks, going golden with heads at the end.
        for i in 0..6usize {
            let x = 1 + i * 3 - (i % 2);
            let top = 16 - height - a.rng.int(0, 1) as usize;
            for y in top..16 {
                let c = if stage == 3 { shade(rgb(200, 170, 60), a.rng.range(0.85, 1.05)) } else { shade(rgb(80, 150, 50), a.rng.range(0.8, 1.05)) };
                a.set(T_CROP_WHEAT + stage, x, y, c);
            }
            if stage >= 2 {
                for y in top..top + 3 {
                    let c = if stage == 3 { rgb(230, 200, 90) } else { rgb(150, 170, 70) };
                    a.set(T_CROP_WHEAT + stage, x + 1, y, c);
                }
            }
        }
        // Carrots and potatoes: leafy tufts; carrots show orange shoulders when ready.
        for (tile, leaf) in [(T_CROP_CARROT, rgb(70, 160, 50)), (T_CROP_POTATO, rgb(60, 130, 45))] {
            for i in 0..4usize {
                let cx = 2 + i * 4;
                let top = 16 - height;
                for y in top..16 {
                    let spread = (y - top) / 3;
                    for dx in [0i32, -(spread as i32).min(1), (spread as i32).min(1)] {
                        let x = (cx as i32 + dx).clamp(0, 15) as usize;
                        let c = shade(leaf, a.rng.range(0.75, 1.1));
                        a.set(tile + stage, x, y, c);
                    }
                }
                if stage == 3 {
                    let c = if tile == T_CROP_CARROT { rgb(240, 130, 30) } else { rgb(190, 150, 90) };
                    for x in cx.saturating_sub(1)..=(cx + 1).min(15) {
                        a.set(tile + stage, x, 15, c);
                        a.set(tile + stage, x, 14, shade(c, 0.9));
                    }
                }
            }
        }
    }

    let wood = [('#', rgb(73, 54, 21)), ('o', rgb(137, 103, 39)), ('H', rgb(150, 150, 150)), ('h', rgb(90, 90, 90))];
    a.sprite(T_HOE, &HOE, &wood);
    a.sprite(T_SEEDS, &POWDER, &[('g', rgb(120, 170, 60)), ('d', rgb(90, 130, 40)), ('k', rgb(60, 90, 30))]);
    a.sprite(T_WHEAT_ITEM, &SHEAF, &[('y', rgb(220, 190, 80)), ('Y', rgb(240, 215, 110)), ('g', rgb(190, 160, 60)), ('b', rgb(130, 90, 40))]);
    a.sprite(T_CARROT_ITEM, &CARROT, &[('#', rgb(140, 60, 10)), ('o', rgb(240, 130, 30)), ('O', rgb(255, 170, 70)), ('g', rgb(70, 160, 50))]);
    a.sprite(T_POTATO_ITEM, &SPUD, &[('#', rgb(90, 60, 30)), ('b', rgb(190, 150, 90)), ('d', rgb(140, 105, 60))]);
    a.sprite(T_BONE_DUST, &POWDER, &[('g', rgb(240, 240, 230)), ('d', rgb(210, 210, 200)), ('k', rgb(180, 180, 170))]);
    a.sprite(T_COMPOST, &POWDER, &[('g', rgb(100, 70, 40)), ('d', rgb(70, 50, 30)), ('k', rgb(60, 100, 40))]);
    a.sprite(T_WOOD_ASH, &POWDER, &[('g', rgb(160, 160, 160)), ('d', rgb(120, 120, 120)), ('k', rgb(80, 80, 80))]);
    a.sprite(T_SOIL_PROBE, &PROBE, &[('#', rgb(50, 50, 60)), ('g', rgb(150, 220, 240)), ('w', rgb(240, 255, 255)), ('i', rgb(200, 200, 200)), ('o', rgb(137, 103, 39))]);
    a.sprite(T_BREAD, &LOAF, &[('#', rgb(100, 60, 20)), ('b', rgb(200, 140, 60)), ('B', rgb(230, 180, 100)), ('d', rgb(160, 100, 40))]);
    a.sprite(T_ROD, &ROD, &[('#', rgb(73, 54, 21)), ('o', rgb(137, 103, 39)), ('s', rgb(230, 230, 230)), ('k', rgb(150, 150, 150))]);
    let fish = |body: Rgba, fin: Rgba, belly: Rgba| [('#', shade(body, 0.45)), ('f', body), ('F', fin), ('b', belly), ('w', rgb(255, 255, 255)), ('k', rgb(10, 10, 10))];
    a.sprite(T_COD, &FISH, &fish(rgb(190, 170, 130), rgb(160, 140, 100), rgb(230, 220, 200)));
    a.sprite(T_SALMON, &FISH, &fish(rgb(200, 80, 70), rgb(150, 60, 60), rgb(240, 170, 150)));
    a.sprite(T_PUFFER, &FISH, &fish(rgb(240, 210, 60), rgb(200, 170, 40), rgb(250, 240, 180)));
    for (x, y) in [(6usize, 3usize), (9, 3), (5, 12), (10, 12), (3, 6), (3, 9)] {
        a.set(T_PUFFER, x, y, rgb(90, 70, 20)); // spikes
    }
    a.sprite(T_TROPICAL, &FISH, &fish(rgb(255, 140, 40), rgb(60, 120, 230), rgb(255, 255, 255)));
    a.sprite(T_BIG_BOB, &FISH, &fish(rgb(240, 190, 40), rgb(255, 230, 120), rgb(255, 245, 200)));
    a.sprite(T_BOOT, &BOOT, &[('#', rgb(40, 25, 15)), ('b', rgb(100, 65, 35)), ('d', rgb(70, 45, 25)), ('w', rgb(90, 140, 230))]);
    a.sprite(T_BOTTLE, &BOTTLE, &[('#', rgb(40, 80, 60)), ('g', rgb(120, 190, 150)), ('w', rgb(220, 255, 240)), ('p', rgb(240, 230, 200)), ('c', rgb(150, 110, 60))]);
    a.sprite(T_FISH_CHIPS, &FISH, &fish(rgb(210, 160, 70), rgb(190, 140, 60), rgb(230, 190, 110)));
    for (x, y) in [(2usize, 13usize), (4, 12), (6, 13), (8, 12), (10, 13), (12, 12), (3, 14), (7, 14), (11, 14)] {
        a.set(T_FISH_CHIPS, x, y, rgb(250, 220, 90)); // the chips
        a.set(T_FISH_CHIPS, x, y - 1, rgb(240, 200, 70));
    }
    a.sprite(T_STEW, &BOWL, &[('#', rgb(80, 55, 25)), ('w', rgb(140, 100, 50)), ('s', rgb(120, 80, 60)), ('m', rgb(200, 40, 35)), ('r', rgb(220, 30, 30))]);
    a.sprite(T_WORM, &WORM, &[('p', rgb(230, 130, 140)), ('k', rgb(40, 20, 20))]);
    a.each(T_BOBBER, |_, y, _, _| if y < 8 { rgb(220, 30, 30) } else { rgb(245, 245, 245) });

    // ---- Chests, furnaces and cooking
    a.each(T_CHEST_SIDE, |x, y, r, _| {
        let band = y == 0 || y == 15 || y == 5 || y == 6 || x == 0 || x == 15;
        let latch = (6..10).contains(&x) && (4..9).contains(&y);
        if latch {
            if (7..9).contains(&x) && y == 6 { rgb(40, 40, 40) } else { shade(rgb(200, 200, 205), r.range(0.9, 1.05)) }
        } else if band {
            shade(rgb(90, 60, 30), r.range(0.9, 1.05))
        } else {
            shade(rgb(165, 115, 60), r.range(0.88, 1.06) * if y % 4 == 0 { 0.9 } else { 1.0 })
        }
    });
    a.each(T_CHEST_TOP, |x, y, r, _| {
        let edge = x == 0 || y == 0 || x == 15 || y == 15;
        if edge { shade(rgb(90, 60, 30), r.range(0.9, 1.05)) } else { shade(rgb(165, 115, 60), r.range(0.88, 1.06) * if x % 5 == 0 { 0.9 } else { 1.0 }) }
    });
    // Bigger chests: the same chest in iron, gold and diamond, banded darker.
    for (top, side, body, band, latch) in [
        (T_IRON_CHEST_TOP, T_IRON_CHEST_SIDE, rgb(200, 200, 205), rgb(110, 110, 118), rgb(70, 70, 75)),
        (T_GOLD_CHEST_TOP, T_GOLD_CHEST_SIDE, rgb(240, 200, 70), rgb(170, 120, 30), rgb(120, 80, 20)),
        (T_DIAMOND_CHEST_TOP, T_DIAMOND_CHEST_SIDE, rgb(120, 230, 225), rgb(50, 150, 150), rgb(30, 90, 95)),
    ] {
        a.each(side, move |x, y, r, _| {
            let edge = y == 0 || y == 15 || y == 5 || y == 6 || x == 0 || x == 15;
            let lock = (6..10).contains(&x) && (4..9).contains(&y);
            if lock {
                if (7..9).contains(&x) && y == 6 { rgb(30, 30, 30) } else { shade(latch, r.range(0.9, 1.05)) }
            } else if edge {
                shade(band, r.range(0.9, 1.05))
            } else {
                shade(body, r.range(0.88, 1.06) * if (x + y * 3) % 7 == 0 { 1.12 } else { 1.0 })
            }
        });
        a.each(top, move |x, y, r, _| {
            let edge = x == 0 || y == 0 || x == 15 || y == 15;
            if edge { shade(band, r.range(0.9, 1.05)) } else { shade(body, r.range(0.88, 1.06) * if (x * 5 + y) % 9 == 0 { 1.12 } else { 1.0 }) }
        });
    }
    a.copy(T_STONE_BRICKS, T_FURNACE_TOP);
    a.copy(T_COBBLE, T_FURNACE_SIDE);
    a.copy(T_COBBLE, T_FURNACE_LIT);
    for y in 8..14 {
        for x in 3..13 {
            let edge = x == 3 || x == 12 || y == 8 || y == 13;
            a.set(T_FURNACE_SIDE, x, y, if edge { rgb(60, 60, 60) } else { rgb(25, 25, 25) });
            let fire = if edge { rgb(60, 60, 60) } else if (x + y) % 3 == 0 { rgb(255, 230, 120) } else if y > 10 { rgb(240, 120, 20) } else { rgb(250, 170, 40) };
            a.set(T_FURNACE_LIT, x, y, fire);
        }
    }
    // ---- Doors and armour
    for (tile, top) in [(T_DOOR_BOTTOM, false), (T_DOOR_TOP, true)] {
        a.each(tile, |x, y, r, _| {
            let frame = x <= 1 || x >= 14 || (!top && y >= 14) || (top && y <= 1);
            let window = top && (4..12).contains(&x) && (4..12).contains(&y) && x != 7 && x != 8 && y != 7 && y != 8;
            let panel = !top && (4..12).contains(&x) && ((2..7).contains(&y) || (9..13).contains(&y));
            let handle = !top && x == 12 && (1..3).contains(&y);
            if handle {
                rgb(60, 60, 60)
            } else if window {
                // Clear: you can see through the window.
                [0, 0, 0, 0]
            } else if frame {
                shade(rgb(110, 80, 45), r.range(0.9, 1.05))
            } else if panel {
                shade(rgb(150, 112, 65), r.range(0.9, 1.05))
            } else {
                shade(rgb(170, 130, 78), r.range(0.9, 1.06) * if x % 5 == 0 { 0.9 } else { 1.0 })
            }
        });
    }
    a.sprite(T_DOOR_ITEM, &DOOR_ITEM, &[('#', rgb(80, 55, 30)), ('b', rgb(170, 130, 78)), ('d', rgb(130, 95, 55)), ('w', rgb(60, 60, 60))]);
    let tiers = [rgb(225, 225, 225), rgb(200, 200, 205), rgb(245, 205, 60), rgb(100, 230, 225)];
    for (t, base) in tiers.iter().enumerate() {
        let pal = [('#', shade(*base, 0.3)), ('b', *base), ('d', shade(*base, 0.72)), ('h', shade(*base, 1.2))];
        for (slot, rows) in [&HELMET, &CHESTPLATE, &LEGGINGS, &BOOTS].into_iter().enumerate() {
            a.sprite(T_ARMOR_ITEMS + t as u16 * 4 + slot as u16, rows, &pal);
        }
        let (base, wool) = (*base, t == 0);
        a.each(T_ARMOR_WORN + t as u16, |x, y, r, _| {
            let rim = x == 0 || y == 0 || x == 15 || y == 15;
            let knit = wool && (x + y) % 4 == 0;
            shade(base, r.range(0.9, 1.05) * if rim { 0.7 } else if knit { 0.85 } else { 1.0 })
        });
    }
    // ---- Experience and anvils
    a.each(T_XP_ORB, |x, y, _, _| {
        let d = ((x as f32 - 7.5).powi(2) + (y as f32 - 7.5).powi(2)).sqrt();
        if d > 7.0 {
            [0, 0, 0, 0]
        } else if d > 5.5 {
            rgb(60, 140, 20)
        } else if d < 2.5 {
            rgb(250, 255, 170)
        } else {
            rgb(150, 230, 50)
        }
    });
    a.each(T_ANVIL_SIDE, |x, y, r, _| {
        let edge = y == 0 || y == 15;
        shade(rgb(68, 68, 72), r.range(0.85, 1.1) * if edge { 0.75 } else if (x + y * 3) % 11 == 0 { 1.15 } else { 1.0 })
    });
    for (i, tile) in [T_ANVIL_TOP, T_ANVIL_TOP_CHIPPED, T_ANVIL_TOP_DAMAGED].into_iter().enumerate() {
        a.each(tile, |x, y, r, _| {
            let edge = x == 0 || y == 0 || x == 15 || y == 15;
            // Cracks get worse with each stage.
            let crack = (i >= 1 && (x as i32 - y as i32 - 2).abs() <= 0 && (3..12).contains(&x)) || (i >= 2 && (x + y == 17 && (4..13).contains(&x) || (y == 9 && (2..8).contains(&x))));
            if crack {
                rgb(25, 25, 28)
            } else {
                shade(rgb(88, 88, 94), r.range(0.9, 1.08) * if edge { 0.75 } else { 1.0 })
            }
        });
    }
    // ---- Caves and enchanting
    a.each(T_GLOWSHROOM, |x, y, _, _| {
        let cap = (4..9).contains(&y) && (2..14).contains(&x) && (y != 4 || (4..=11).contains(&x));
        let stalk = (9..16).contains(&y) && (7..9).contains(&x);
        if cap {
            if (x * 3 + y) % 4 == 0 { rgb(220, 255, 250) } else { rgb(70, 220, 200) }
        } else if stalk {
            rgb(150, 210, 190)
        } else {
            [0, 0, 0, 0]
        }
    });
    a.each(T_POINTY_ROCK, |x, y, r, _| {
        // A stone cone, point at the top (the mesher flips it for ones hanging
        // from ceilings). Lit from the left, with ridges and a paler, worn tip.
        let half = ((y as f32 + 1.0) * 0.46).min(7.0);
        let dx = x as f32 - 7.5;
        if dx.abs() > half {
            return [0, 0, 0, 0];
        }
        let round = 1.1 - 0.35 * (dx / half.max(1.0) + 1.0) / 2.0;
        let ridge = if y % 4 == 0 { 0.85 } else { 1.0 };
        let tip = if y < 4 { 1.12 } else { 1.0 };
        shade(rgb(118, 116, 112), round * ridge * tip * r.range(0.9, 1.06))
    });
    a.each(T_ENCH_TOP, |x, y, r, _| {
        let edge = x == 0 || y == 0 || x == 15 || y == 15;
        let book = (4..12).contains(&x) && (5..11).contains(&y);
        if book {
            if x == 7 || x == 8 { rgb(120, 30, 30) } else { rgb(235, 225, 200) }
        } else if edge {
            rgb(20, 15, 30)
        } else {
            shade(rgb(170, 30, 40), r.range(0.9, 1.05))
        }
    });
    a.each(T_ENCH_SIDE, |x, y, r, _| {
        if y < 4 {
            shade(rgb(170, 30, 40), r.range(0.9, 1.05))
        } else if (x + y) % 7 == 0 {
            rgb(90, 220, 230)
        } else {
            shade(rgb(30, 20, 45), r.range(0.8, 1.1))
        }
    });
    a.each(T_ENCH_BOTTOM, |_, _, r, _| shade(rgb(30, 20, 45), r.range(0.8, 1.1)));
    // ---- Liquids
    a.each(T_LAVA, |x, y, r, p| {
        let n = p.noise2(x as f32 * 0.3 + 20.0, y as f32 * 0.3) + r.range(-0.08, 0.08);
        if n > 0.35 {
            rgb(255, 230, 120)
        } else if n > 0.0 {
            rgb(250, 150, 30)
        } else {
            shade(rgb(215, 80, 15), 1.0 + n * 0.4)
        }
    });
    a.each(T_OBSIDIAN, |x, y, r, p| {
        let n = p.noise2(x as f32 * 0.5 + 90.0, y as f32 * 0.5);
        let glint = (x * 7 + y * 3) % 23 == 0;
        if glint { rgb(120, 90, 170) } else { shade(rgb(28, 20, 40), 1.0 + n * 0.5 + r.range(-0.1, 0.1)) }
    });
    // ---- Zappy Dust
    a.ore(T_ZAP_ORE, rgb(230, 40, 40), rgb(150, 10, 10));
    for (tile, lit) in [(T_WIRE, false), (T_WIRE_ON, true)] {
        a.each(tile, |x, y, r, _| {
            // A cross of dust with a blob in the middle.
            let (dx, dy) = ((x as i32 - 7).abs(), (y as i32 - 7).abs());
            let on = dx <= 1 || dy <= 1 || (dx <= 3 && dy <= 3);
            if !on {
                [0, 0, 0, 0]
            } else if lit {
                shade(rgb(255, 40, 30), r.range(0.85, 1.15))
            } else {
                shade(rgb(110, 10, 10), r.range(0.85, 1.15))
            }
        });
    }
    for (tile, on) in [(T_LEVER, false), (T_LEVER_ON, true)] {
        a.each(tile, |x, y, r, _| {
            let base = y >= 12 && (4..12).contains(&x);
            // The handle leans left when off, right when on.
            let t = (12 - y as i32).max(0) as f32 / 10.0;
            let hx = 7.5 + if on { t * 4.0 } else { -t * 4.0 };
            let handle = (2..12).contains(&y) && (x as f32 - hx).abs() < 1.0;
            let knob = y < 4 && (x as f32 - hx).abs() < 1.5;
            if base {
                shade(rgb(120, 120, 120), r.range(0.85, 1.1))
            } else if knob && on {
                rgb(255, 50, 40)
            } else if knob || handle {
                shade(rgb(140, 105, 60), r.range(0.9, 1.1))
            } else {
                [0, 0, 0, 0]
            }
        });
    }
    a.each(T_ZAP_BLOCK, |x, y, r, _| {
        let sparkle = (x * 5 + y * 3) % 13 == 0;
        if sparkle { rgb(255, 180, 170) } else { shade(rgb(200, 25, 20), r.range(0.8, 1.1)) }
    });
    for (tile, on) in [(T_LAMP, false), (T_LAMP_ON, true)] {
        a.each(tile, |x, y, r, _| {
            let frame = x == 0 || y == 0 || x == 15 || y == 15 || x == 7 || y == 7;
            if frame {
                rgb(70, 45, 30)
            } else if on {
                shade(rgb(255, 225, 150), r.range(0.9, 1.08))
            } else {
                shade(rgb(120, 80, 55), r.range(0.85, 1.1))
            }
        });
    }
    for (tile, cover) in [(T_BOOK, rgb(120, 70, 40)), (T_ENCHANTED_BOOK, rgb(110, 40, 150))] {
        a.sprite(tile, &BOOK_SPRITE, &[('#', rgb(30, 15, 10)), ('c', cover), ('d', shade(cover, 0.7)), ('p', rgb(240, 232, 210)), ('g', rgb(250, 210, 60))]);
    }
    a.sprite(T_ZAP_DUST, &DUST_SPRITE, &[('#', rgb(90, 5, 5)), ('r', rgb(220, 30, 25)), ('h', rgb(255, 120, 100))]);
    a.sprite(T_SHIELD, &SHIELD_SPRITE, &[('#', rgb(40, 30, 20)), ('w', rgb(150, 110, 65)), ('d', rgb(115, 80, 45)), ('i', rgb(190, 190, 198))]);
    // ---- The Scorchlands
    a.each(T_SCORCHROCK, |x, y, r, p| {
        let n = p.noise2(x as f32 * 0.45 + 70.0, y as f32 * 0.45);
        shade(rgb(125, 45, 42), 1.0 + n * 0.3 + r.range(-0.12, 0.12))
    });
    a.each(T_EMBERSAND, |x, y, r, p| {
        let face = p.noise2(x as f32 * 0.6 + 11.0, y as f32 * 0.6) > 0.35;
        if face { rgb(60, 40, 30) } else { shade(rgb(95, 70, 55), r.range(0.8, 1.1)) }
    });
    a.copy(T_SCORCHROCK, T_SCORCH_GOLD);
    for _ in 0..9 {
        let (x, y) = (a.rng.int(1, 14) as usize, a.rng.int(1, 14) as usize);
        a.set(T_SCORCH_GOLD, x, y, rgb(250, 210, 60));
        a.set(T_SCORCH_GOLD, x + 1, y, rgb(200, 160, 40));
    }
    a.each(T_PORTAL, |x, y, r, p| {
        let swirl = (p.noise2(x as f32 * 0.35, y as f32 * 0.35 + 40.0) * 7.0).sin();
        if swirl > 0.75 {
            [0, 0, 0, 0]
        } else {
            shade(rgb(140, 60, 220), 0.8 + swirl.abs() * 0.4 + r.range(-0.05, 0.05))
        }
    });
    a.sprite(T_SPARKER, &SPARKER_SPRITE, &[('#', rgb(40, 40, 45)), ('i', rgb(190, 190, 198)), ('c', rgb(50, 50, 55)), ('f', rgb(255, 180, 40)), ('y', rgb(255, 240, 150))]);
    a.speckle(T_GRUMBLE_SKIN, rgb(210, 140, 130), 0.1);
    for _ in 0..30 {
        let (x, y) = (a.rng.int(0, 15) as usize, a.rng.int(0, 15) as usize);
        a.set(T_GRUMBLE_SKIN, x, y, rgb(110, 150, 90));
    }
    a.copy(T_GRUMBLE_SKIN, T_GRUMBLE_FACE);
    for (x, y, c) in [(3, 5, rgb(250, 250, 250)), (4, 5, rgb(200, 30, 30)), (11, 5, rgb(200, 30, 30)), (12, 5, rgb(250, 250, 250))] {
        a.set(T_GRUMBLE_FACE, x, y, c);
    }
    for y in 8..12 {
        for x in 5..11 {
            a.set(T_GRUMBLE_FACE, x, y, if y > 8 && (x == 6 || x == 9) { rgb(90, 40, 40) } else { rgb(230, 160, 150) });
        }
    }
    // ---- Rails and vehicles
    for k in 0..10u16 {
        let tile = T_RAIL + k;
        let powered = k >= 6;
        let (shape, on) = if powered { ((k - 6) / 2, (k - 6) % 2 == 1) } else { (k, false) };
        let rail = if powered { rgb(235, 190, 60) } else { rgb(170, 170, 178) };
        a.each(tile, |x, y, r, _| {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            // Distance across the track (0 at one rail's middle) and along it.
            let (across, along) = match shape {
                0 => (fx, fy),
                1 => (fy, fx),
                _ => {
                    // Corners: arcs round the corner between the two ends.
                    let (cx, cy) = match shape {
                        2 => (16.0, 0.0),
                        3 => (0.0, 0.0),
                        4 => (16.0, 16.0),
                        _ => (0.0, 16.0),
                    };
                    let d = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt();
                    let ang = (fy - cy).atan2(fx - cx);
                    (d, ang * 16.0 / std::f32::consts::FRAC_PI_2)
                }
            };
            let on_rail = (3.0..5.0).contains(&across) || (11.0..13.0).contains(&across);
            let sleeper = (2.0..14.0).contains(&across) && along.rem_euclid(4.0) < 2.0;
            let dust = powered && (7.0..9.0).contains(&across);
            if on_rail {
                shade(rail, r.range(0.9, 1.1))
            } else if dust {
                if on { rgb(255, 50, 40) } else { rgb(110, 15, 15) }
            } else if sleeper {
                shade(rgb(115, 85, 50), r.range(0.85, 1.05))
            } else {
                [0, 0, 0, 0]
            }
        });
    }
    // Detector rails: rails with a pressure plate in the middle, red when a cart's on it.
    for k in 0..4u16 {
        let (ew, on) = (k >= 2, k % 2 == 1);
        a.each(T_DETECTOR_RAIL + k, |x, y, r, _| {
            let (across, along) = if ew { (y as f32 + 0.5, x as f32 + 0.5) } else { (x as f32 + 0.5, y as f32 + 0.5) };
            if (3.0..5.0).contains(&across) || (11.0..13.0).contains(&across) {
                shade(rgb(170, 170, 178), r.range(0.9, 1.1))
            } else if (5.5..10.5).contains(&across) && (5.0..11.0).contains(&along) {
                if on { rgb(230, 50, 40) } else { shade(rgb(120, 115, 110), r.range(0.9, 1.05)) }
            } else if (2.0..14.0).contains(&across) && along.rem_euclid(4.0) < 2.0 {
                shade(rgb(115, 85, 50), r.range(0.85, 1.05))
            } else {
                [0, 0, 0, 0]
            }
        });
    }
    a.each(T_CART, |x, y, r, _| {
        let rivet = (x % 5 == 2) && (y % 5 == 2);
        let rim = x == 0 || y == 0 || x == 15 || y == 15;
        if rivet { rgb(210, 210, 215) } else { shade(rgb(95, 95, 102), r.range(0.85, 1.1) * if rim { 0.75 } else { 1.0 }) }
    });
    a.each(T_FRAME, |x, y, r, _| {
        let rim = x < 2 || y < 2 || x > 13 || y > 13;
        if rim { shade(rgb(140, 100, 60), r.range(0.85, 1.1)) } else { shade(rgb(150, 115, 80), r.range(0.9, 1.05) * 0.8) }
    });
    a.sprite(T_COMPASS, &COMPASS_SPRITE, &[('#', rgb(40, 40, 45)), ('i', rgb(190, 190, 198)), ('w', rgb(235, 235, 225)), ('r', rgb(220, 30, 30)), ('k', rgb(60, 60, 70))]);
    a.each(T_SAPLING, |x, y, r, _| {
        // A thin stem with a few leafy tufts.
        let stem = x == 7 && y >= 8;
        let d = |cx: i32, cy: i32, rad: i32| (x as i32 - cx).pow(2) + (y as i32 - cy).pow(2) <= rad * rad;
        if stem {
            shade(rgb(100, 70, 40), r.range(0.85, 1.1))
        } else if d(7, 5, 3) || d(4, 8, 2) || d(10, 8, 2) {
            shade(rgb(60, 140, 45), r.range(0.75, 1.15))
        } else {
            [0, 0, 0, 0]
        }
    });
    // Biomes.
    let bark = |a: &mut Atlas, side: u16, top: u16, bark: [u8; 3], heart: [u8; 3]| {
        a.each(side, |x, _y, r, _| {
            let stripe = if x % 3 == 0 { 0.72 } else if x % 5 == 2 { 0.88 } else { 1.0 };
            shade(rgb(bark[0], bark[1], bark[2]), stripe * r.range(0.88, 1.1))
        });
        a.each(top, |x, y, r, _| {
            let (dx, dy) = (x as f32 - 7.5, y as f32 - 7.5);
            let d = dx.abs().max(dy.abs());
            if d > 6.5 {
                shade(rgb(bark[0], bark[1], bark[2]), r.range(0.85, 1.05))
            } else {
                let ring = if (d as i32) % 2 == 0 { 1.0 } else { 0.85 };
                shade(rgb(heart[0], heart[1], heart[2]), ring * r.range(0.95, 1.05))
            }
        });
    };
    bark(&mut a, T_SPRUCE_LOG_SIDE, T_SPRUCE_LOG_TOP, [70, 50, 32], [140, 105, 65]);
    bark(&mut a, T_JUNGLE_LOG_SIDE, T_JUNGLE_LOG_TOP, [110, 90, 45], [175, 130, 80]);
    bark(&mut a, T_CHERRY_LOG_SIDE, T_CHERRY_LOG_TOP, [60, 30, 38], [215, 150, 150]);
    // Observers: a stone box with a face (two eyes) and a back that lights up.
    let stone_box = |x: usize, y: usize, r: &mut Rng| {
        let rim = x == 0 || y == 0 || x == 15 || y == 15;
        shade(if rim { rgb(70, 70, 74) } else { rgb(120, 120, 125) }, r.range(0.9, 1.08))
    };
    a.each(T_OBSERVER_SIDE, |x, y, r, _| if (3..13).contains(&x) && y % 4 == 1 { shade(rgb(85, 85, 90), r.range(0.9, 1.05)) } else { stone_box(x, y, r) });
    a.each(T_OBSERVER_FACE, |x, y, r, _| {
        let eye = (4..7).contains(&x) && (5..9).contains(&y) || (9..12).contains(&x) && (5..9).contains(&y);
        if eye { rgb(20, 20, 24) } else if (3..13).contains(&x) && (11..13).contains(&y) { rgb(45, 45, 50) } else { stone_box(x, y, r) }
    });
    for (t, dot) in [(T_OBSERVER_BACK, rgb(70, 20, 20)), (T_OBSERVER_BACK_ON, rgb(255, 60, 40))] {
        a.each(t, |x, y, r, _| if (6..10).contains(&x) && (6..10).contains(&y) { dot } else { stone_box(x, y, r) });
    }
    // Crafters: an iron box with a crafting grid on top and a mouth in front.
    let iron_box = |x: usize, y: usize, r: &mut Rng| {
        let rim = x == 0 || y == 0 || x == 15 || y == 15;
        shade(if rim { rgb(90, 90, 96) } else { rgb(160, 160, 166) }, r.range(0.92, 1.06))
    };
    a.each(T_CRAFTER_TOP, |x, y, r, _| if (x % 5 == 0 || y % 5 == 0) && (1..15).contains(&x) && (1..15).contains(&y) { rgb(60, 45, 30) } else { iron_box(x, y, r) });
    a.each(T_CRAFTER_SIDE, |x, y, r, _| if y < 3 { shade(rgb(150, 115, 70), r.range(0.9, 1.05)) } else { iron_box(x, y, r) });
    a.each(T_CRAFTER_FACE, |x, y, r, _| if (5..11).contains(&x) && (6..11).contains(&y) { rgb(25, 22, 20) } else if y < 3 { shade(rgb(150, 115, 70), r.range(0.9, 1.05)) } else { iron_box(x, y, r) });
    // Copper Bulbs: copper casing round a glass lamp, dark or glowing.
    for (t, inner) in [(T_COPPER_BULB, rgb(90, 60, 40)), (T_COPPER_BULB_ON, rgb(255, 225, 150))] {
        a.each(t, |x, y, r, _| {
            let frame = x < 3 || y < 3 || x > 12 || y > 12 || x == 7 || x == 8;
            if frame { shade(rgb(195, 110, 75), r.range(0.85, 1.1)) } else { shade(inner, r.range(0.9, 1.08)) }
        });
    }
    bark(&mut a, T_MANGROVE_LOG_SIDE, T_MANGROVE_LOG_TOP, [90, 55, 40], [150, 60, 50]);
    bark(&mut a, T_PALE_LOG_SIDE, T_PALE_LOG_TOP, [175, 170, 165], [225, 215, 210]);
    a.each(T_PALE_LEAVES, |_, _, r, _| if r.chance(0.15) { [180, 185, 175, 0] } else { shade(rgb(165, 172, 162), r.range(0.78, 1.12)) });
    a.each(T_PALE_PLANKS, |x, y, r, _| {
        let board = y / 4;
        let seam = y % 4 == 3 || (x + board * 5) % 16 == 0;
        if seam { rgb(170, 160, 155) } else { shade(rgb(230, 222, 215), r.range(0.92, 1.06)) }
    });
    a.each(T_PALE_MOSS, |_, _, r, _| shade(rgb(150, 158, 145), r.range(0.8, 1.12)));
    a.each(T_PALE_HANGING_MOSS, |x, y, r, _| {
        // Strands hanging down, longer in the middle.
        let len = 6 + (x * 7) % 5 + if (5..11).contains(&x) { 4 } else { 0 };
        if x % 3 != 1 && y < len { shade(rgb(150, 158, 145), r.range(0.8, 1.1)) } else { [0, 0, 0, 0] }
    });
    for (t, eye) in [(T_CREAKING_HEART, rgb(70, 60, 55)), (T_CREAKING_HEART_ON, rgb(255, 150, 40))] {
        a.each(t, |x, y, r, _| {
            let d = ((x as f32 - 7.5).powi(2) + (y as f32 - 7.5).powi(2)).sqrt();
            if d < 3.0 { shade(eye, r.range(0.85, 1.1)) } else if d < 4.0 { rgb(60, 50, 45) } else { shade(rgb(175, 170, 165), r.range(0.85, 1.05)) }
        });
    }
    a.sprite(T_BOOK_QUILL, &ORB_SPRITE, &[('#', rgb(60, 35, 20)), ('h', rgb(240, 235, 220)), ('b', rgb(150, 90, 50)), ('d', rgb(110, 65, 35))]);
    a.sprite(T_WRITTEN_BOOK, &ORB_SPRITE, &[('#', rgb(50, 25, 15)), ('h', rgb(230, 200, 90)), ('b', rgb(130, 55, 35)), ('d', rgb(95, 35, 25))]);
    let plank = |x: usize, y: usize, r: &mut Rng| {
        let seam = y % 4 == 3 || (x + (y / 4) * 5).is_multiple_of(16);
        if seam { rgb(110, 80, 45) } else { shade(rgb(170, 130, 80), r.range(0.9, 1.07)) }
    };
    a.each(T_LECTERN_SIDE, |x, y, r, _| if (5..11).contains(&x) && y > 3 { shade(rgb(130, 95, 55), r.range(0.9, 1.05)) } else { plank(x, y, r) });
    a.each(T_LECTERN_TOP, |x, y, r, _| plank(x, y, r));
    a.each(T_LECTERN_BOOK_TOP, |x, y, r, _| {
        let page = (2..14).contains(&x) && (3..13).contains(&y);
        if page && x != 7 && x != 8 { if y % 2 == 0 && (3..12).contains(&x) && x != 6 && x != 9 { rgb(150, 145, 130) } else { rgb(240, 235, 220) } } else if page { rgb(120, 60, 35) } else { plank(x, y, r) }
    });
    a.each(T_LOOM_TOP, |x, y, r, _| if (2..14).contains(&x) && y % 3 == 1 { rgb(235, 230, 215) } else { plank(x, y, r) });
    a.each(T_LOOM_SIDE, |x, y, r, _| if (3..13).contains(&x) && (2..8).contains(&y) && x % 2 == 0 { rgb(235, 230, 215) } else { plank(x, y, r) });
    a.each(T_BANNER_ICON, |x, y, r, _| {
        let pole = x == 7 || x == 8;
        let bar = y == 1 && (3..13).contains(&x);
        let cloth = (4..12).contains(&x) && (2..13).contains(&y);
        if bar || (pole && !cloth) { rgb(150, 110, 60) } else if cloth { shade(rgb(235, 235, 230), r.range(0.9, 1.05)) } else { [0, 0, 0, 0] }
    });
    a.each(T_PERSONAL_CHEST_TOP, |x, y, r, _| {
        let rim = x < 2 || y < 2 || x > 13 || y > 13;
        shade(if rim { rgb(30, 25, 45) } else { rgb(45, 60, 55) }, r.range(0.85, 1.1))
    });
    a.each(T_PERSONAL_CHEST_SIDE, |x, y, r, _| {
        let rim = !(2..=13).contains(&x) || y > 13 || y == 6;
        let eye = (6..10).contains(&x) && (4..9).contains(&y);
        if eye { shade(rgb(80, 220, 170), r.range(0.85, 1.1)) } else { shade(if rim { rgb(30, 25, 45) } else { rgb(45, 60, 55) }, r.range(0.85, 1.1)) }
    });
    a.each(T_SNIFFER, |x, y, r, _| {
        let tuft = (x * 5 + y * 3) % 9 == 0;
        shade(if tuft { rgb(90, 160, 80) } else { rgb(170, 70, 55) }, r.range(0.82, 1.1))
    });
    a.each(T_SNIFFER_FACE, |x, y, r, _| {
        let eye = (y == 5 || y == 6) && (x == 3 || x == 12);
        if eye { rgb(30, 25, 20) } else { shade(rgb(200, 150, 70), r.range(0.85, 1.08)) }
    });
    a.each(T_SNIFFER_EGG, |x, y, r, _| {
        let spot = (x * 7 + y * 5) % 11 < 2;
        shade(if spot { rgb(60, 110, 70) } else { rgb(170, 70, 55) }, r.range(0.85, 1.1))
    });
    a.each(T_COPPER_CHEST_TOP, |x, y, r, _| {
        let rim = x < 2 || y < 2 || x > 13 || y > 13;
        shade(if rim { rgb(120, 60, 35) } else { rgb(200, 110, 70) }, r.range(0.85, 1.1))
    });
    a.each(T_COPPER_CHEST_SIDE, |x, y, r, _| {
        let rim = !(2..=13).contains(&x) || y > 13 || y == 6;
        let latch = (7..9).contains(&x) && (5..9).contains(&y);
        let teal = (x * 3 + y * 5) % 13 == 0;
        if latch { rgb(70, 160, 140) } else { shade(if rim { rgb(120, 60, 35) } else if teal { rgb(90, 160, 130) } else { rgb(200, 110, 70) }, r.range(0.85, 1.1)) }
    });
    a.each(T_COPPER_GOLEM, |x, y, r, _| {
        let rivet = (x % 5 == 2) && (y % 5 == 2);
        let teal = (x * 7 + y * 3) % 17 == 0;
        shade(if rivet { rgb(240, 160, 110) } else if teal { rgb(80, 160, 130) } else { rgb(195, 105, 65) }, r.range(0.85, 1.1))
    });
    a.each(T_COPPER_GOLEM_FACE, |x, y, r, _| {
        let eye = (5..7).contains(&y) && (x == 4 || x == 5 || x == 10 || x == 11);
        let nose = (7..9).contains(&x) && (6..11).contains(&y);
        if eye { rgb(30, 20, 15) } else if nose { shade(rgb(230, 140, 90), r.range(0.9, 1.05)) } else { shade(rgb(195, 105, 65), r.range(0.85, 1.1)) }
    });
    a.each(T_EYEBLOSSOM, |x, y, r, _| {
        let stem = (7..9).contains(&x) && y > 7;
        let bud = (5..11).contains(&x) && (3..8).contains(&y);
        if bud { shade(rgb(140, 140, 145), r.range(0.85, 1.1)) } else if stem { rgb(90, 100, 90) } else { [0, 0, 0, 0] }
    });
    a.each(T_EYEBLOSSOM_OPEN, |x, y, r, _| {
        let stem = (7..9).contains(&x) && y > 7;
        let (dx, dy) = (x as f32 - 7.5, y as f32 - 5.0);
        let petal = dx * dx + dy * dy < 18.0;
        let eye = dx * dx + dy * dy < 3.0;
        if eye { rgb(255, 150, 40) } else if petal { shade(rgb(230, 230, 235), r.range(0.9, 1.05)) } else if stem { rgb(90, 100, 90) } else { [0, 0, 0, 0] }
    });
    a.each(T_RESIN_BLOCK, |x, y, r, _| {
        let blob = (x * 5 + y * 9) % 7 == 0;
        shade(if blob { rgb(250, 160, 50) } else { rgb(225, 120, 30) }, r.range(0.85, 1.1))
    });
    a.each(T_RESIN_BRICKS, |x, y, r, _| {
        let mortar = y % 4 == 3 || (x + (y / 4) * 4) % 8 == 0;
        shade(if mortar { rgb(150, 70, 20) } else { rgb(220, 115, 35) }, r.range(0.85, 1.1))
    });
    a.sprite(T_RESIN_CLUMP, &ORB_SPRITE, &[('#', rgb(110, 50, 10)), ('h', rgb(255, 190, 90)), ('b', rgb(230, 125, 35)), ('d', rgb(170, 80, 20))]);
    a.sprite(T_RESIN_BRICK, &INGOT_SPRITE, &[('#', rgb(110, 50, 10)), ('b', rgb(225, 120, 35)), ('h', rgb(255, 180, 80)), ('d', rgb(160, 75, 20))]);
    a.each(T_FIREFLY_BUSH, |x, y, r, _| {
        let twig = (x * 3 + y * 7) % 5 == 0 && y > 3;
        let glow = (x * 11 + y * 5) % 23 == 0 && y < 10;
        if glow { rgb(250, 240, 120) } else if twig { shade(rgb(70, 95, 45), r.range(0.8, 1.1)) } else { [0, 0, 0, 0] }
    });
    a.each(T_LEAF_LITTER, |x, y, r, _| {
        let leaf = (x * 7 + y * 3) % 5 < 2;
        let c = match (x * 3 + y) % 3 { 0 => rgb(160, 95, 40), 1 => rgb(185, 130, 55), _ => rgb(130, 75, 35) };
        if leaf { shade(c, r.range(0.85, 1.1)) } else { [0, 0, 0, 0] }
    });
    a.each(T_WILDFLOWERS, |x, y, r, _| {
        let head = (x * 5 + y * 7) % 11 == 0;
        let leaf = (x * 3 + y * 5) % 7 == 0;
        if head { shade(rgb(250, 215, 60), r.range(0.9, 1.05)) } else if leaf { rgb(80, 140, 60) } else { [0, 0, 0, 0] }
    });
    a.each(T_DRIED_FLOATY, |x, y, r, _| {
        let crack = (x + y * 3) % 7 == 0;
        shade(if crack { rgb(110, 95, 85) } else { rgb(160, 145, 130) }, r.range(0.85, 1.08))
    });
    a.each(T_FLOATY, |_, _, r, _| shade(rgb(245, 245, 250), r.range(0.92, 1.03)));
    a.each(T_FLOATY_FACE, |x, y, r, _| {
        let eye = (5..8).contains(&y) && (x == 4 || x == 5 || x == 10 || x == 11);
        let smile = y == 10 && (5..11).contains(&x) || y == 9 && (x == 4 || x == 11);
        let cheek = y == 8 && (x == 2 || x == 13);
        if eye || smile { rgb(40, 40, 50) } else if cheek { rgb(250, 160, 170) } else { shade(rgb(245, 245, 250), r.range(0.92, 1.03)) }
    });
    a.each(T_HARNESS, |x, y, r, _| {
        let strap = (x + y) % 6 == 0 || (x + 15 - y) % 6 == 0;
        let goggle = (y == 4 || y == 5) && ((3..6).contains(&x) || (10..13).contains(&x));
        if goggle { rgb(160, 210, 240) } else if strap && (2..14).contains(&x) && (2..14).contains(&y) { shade(rgb(120, 70, 35), r.range(0.85, 1.1)) } else { [0, 0, 0, 0] }
    });
    a.each(T_HARNESS_WORN, |x, y, r, _| {
        let strap = y % 6 < 2 || x % 6 < 1;
        shade(if strap { rgb(120, 70, 35) } else { rgb(200, 60, 60) }, r.range(0.85, 1.1))
    });
    for (i, (shaft, head, dark)) in [
        (rgb(140, 100, 55), rgb(175, 130, 75), rgb(95, 65, 35)),
        (rgb(140, 100, 55), rgb(140, 140, 140), rgb(80, 80, 80)),
        (rgb(140, 100, 55), rgb(225, 225, 230), rgb(120, 120, 130)),
        (rgb(140, 100, 55), rgb(110, 235, 225), rgb(40, 130, 125)),
    ]
    .into_iter()
    .enumerate()
    {
        a.sprite(T_SPEAR_FIRST + i as u16, &SPEAR_SPRITE, &[('#', dark), ('h', head), ('t', shaft)]);
    }
    a.each(T_ROTSTEED, |x, y, r, _| {
        let bone = (x * 5 + y * 3) % 13 == 0;
        shade(if bone { rgb(200, 205, 180) } else { rgb(85, 120, 75) }, r.range(0.82, 1.08))
    });
    a.each(T_ROTSTEED_FACE, |x, y, r, _| {
        let eye = (4..6).contains(&y) && (x == 3 || x == 12);
        let teeth = y == 12 && x % 2 == 0;
        if eye { rgb(200, 40, 30) } else if teeth { rgb(220, 220, 200) } else { shade(rgb(85, 120, 75), r.range(0.82, 1.08)) }
    });
    a.sprite(T_PITCHER_POD, &ORB_SPRITE, &[('#', rgb(40, 60, 50)), ('h', rgb(130, 210, 190)), ('b', rgb(70, 150, 130)), ('d', rgb(50, 110, 95))]);
    a.each(T_PITCHER_CROP, |x, y, r, _| {
        let stem = (7..9).contains(&x) && y > 6;
        let leaf = y > 9 && (x as i32 - 7).abs() < (y as i32 - 8);
        if stem || leaf { shade(rgb(70, 140, 110), r.range(0.85, 1.1)) } else { [0, 0, 0, 0] }
    });
    a.each(T_PITCHER_PLANT, |x, y, r, _| {
        let cup = (4..12).contains(&x) && (2..9).contains(&y) && !((6..10).contains(&x) && y < 4);
        let stem = (7..9).contains(&x) && y >= 9;
        if cup { shade(if y < 4 { rgb(170, 110, 220) } else { rgb(80, 175, 160) }, r.range(0.85, 1.1)) } else if stem { rgb(60, 120, 90) } else { [0, 0, 0, 0] }
    });
    a.each(T_CREAKING, |x, y, r, _| {
        let crack = (x * 3 + y) % 7 == 0;
        shade(if crack { rgb(55, 45, 40) } else { rgb(120, 105, 95) }, r.range(0.8, 1.1))
    });
    for (t, eye) in [(T_CREAKING_FACE, rgb(60, 50, 45)), (T_CREAKING_FACE_ON, rgb(255, 150, 40))] {
        a.each(t, |x, y, r, _| {
            let e = ((3..6).contains(&x) || (10..13).contains(&x)) && (5..8).contains(&y);
            if e { eye } else { shade(rgb(120, 105, 95), r.range(0.82, 1.08)) }
        });
    }
    a.each(T_CHERRY_LEAVES, |x, y, r, _| {
        if r.chance(0.13) { return [240, 170, 200, 0]; }
        let blossom = (x * 7 + y * 3) % 5 == 0;
        shade(if blossom { rgb(255, 215, 230) } else { rgb(240, 160, 195) }, r.range(0.82, 1.1))
    });
    a.each(T_PINK_PETALS, |x, y, r, _| {
        // Little scattered petals on the ground; the gaps show the grass.
        let petal = (x * 5 + y * 11) % 7 < 2 && r.chance(0.8);
        if petal { shade(rgb(245, 170, 205), r.range(0.85, 1.1)) } else { [0, 0, 0, 0] }
    });
    a.each(T_MANGROVE_LEAVES, |_, _, r, _| if r.chance(0.14) { [60, 140, 40, 0] } else { shade(rgb(70, 145, 45), r.range(0.72, 1.15)) });
    a.each(T_MANGROVE_ROOTS, |x, y, r, _| {
        // A tangle: diagonal roots with gaps between.
        let root = (x + y) % 5 < 2 || (x + 16 - y) % 6 < 2;
        if root { shade(rgb(95, 75, 55), r.range(0.8, 1.1)) } else { [0, 0, 0, 0] }
    });
    for (t, light, dark) in [(T_CHERRY_PLANKS, rgb(225, 175, 170), rgb(150, 95, 100)), (T_MANGROVE_PLANKS, rgb(150, 70, 60), rgb(95, 40, 35))] {
        a.each(t, |x, y, r, _| {
            let board = y / 4;
            let seam = y % 4 == 3 || (x + board * 5) % 16 == 0;
            if seam { dark } else { shade(light, r.range(0.9, 1.08)) }
        });
    }
    a.each(T_SPRUCE_LEAVES, |x, y, r, _| {
        // Needles: darker, bluer, in little diagonal strokes.
        if r.chance(0.2) { return [40, 80, 55, 0]; }
        shade(rgb(40, 85, 55), r.range(0.7, 1.1) * if (x + y) % 3 == 0 { 0.8 } else { 1.0 })
    });
    a.each(T_JUNGLE_LEAVES, |_, _, r, _| if r.chance(0.15) { [50, 150, 30, 0] } else { shade(rgb(50, 155, 30), r.range(0.7, 1.2)) });
    a.each(T_MUD, |x, y, r, p| {
        let wet = p.noise2(x as f32 / 5.0, y as f32 / 5.0) > 0.2;
        shade(if wet { rgb(60, 50, 45) } else { rgb(80, 65, 55) }, r.range(0.85, 1.1))
    });
    a.each(T_LILY_PAD, |x, y, r, _| {
        let (dx, dy) = (x as f32 - 7.5, y as f32 - 7.5);
        let notch = dx > 0.0 && dy.abs() < dx * 0.35;
        if dx * dx + dy * dy < 52.0 && !notch { shade(rgb(50, 120, 40), r.range(0.8, 1.1)) } else { [0, 0, 0, 0] }
    });
    a.each(T_RED_SAND, |_, _, r, _| shade(rgb(190, 100, 45), r.range(0.85, 1.1)));
    for (i, c) in [[150, 95, 70], [165, 85, 40], [140, 60, 45], [185, 135, 55]].iter().enumerate() {
        let c = *c;
        a.each(T_TERRACOTTA + i as u16, move |_, _, r, _| shade(rgb(c[0], c[1], c[2]), r.range(0.9, 1.06)));
    }
    a.each(T_DEAD_BUSH, |x, y, _, _| {
        // A few bare twigs from one root.
        let (fx, fy) = (x as f32 - 7.5, 15.0 - y as f32);
        let twig = |slope: f32, from: f32| fy > from && (fx - (fy - from) * slope).abs() < 0.7;
        if (fx.abs() < 0.7 && fy < 9.0) || twig(0.8, 3.0) || twig(-0.7, 4.0) || twig(0.35, 6.0) || twig(-1.2, 7.0) { rgb(125, 85, 45) } else { [0, 0, 0, 0] }
    });
    a.each(T_MELON_SIDE, |x, _, r, _| shade(if x % 5 < 2 { rgb(60, 120, 30) } else { rgb(100, 160, 40) }, r.range(0.85, 1.1)));
    a.each(T_MELON_TOP, |x, y, r, _| {
        let (dx, dy) = (x as f32 - 7.5, y as f32 - 7.5);
        shade(if ((dx * dx + dy * dy).sqrt() as i32) % 3 == 0 { rgb(70, 125, 30) } else { rgb(105, 160, 45) }, r.range(0.85, 1.1))
    });
    a.each(T_MELON_SLICE, |x, y, _, _| {
        // A wedge: green rind along the bottom, red flesh, black seeds.
        let (dx, dy) = (x as f32 - 7.5, y as f32 - 3.0);
        let d = (dx * dx + dy * dy).sqrt();
        if y < 3 || d > 10.5 {
            [0, 0, 0, 0]
        } else if d > 9.0 {
            rgb(70, 140, 40)
        } else if d > 8.0 {
            rgb(230, 235, 190)
        } else if (x * 7 + y * 3) % 11 == 0 && d < 7.0 {
            rgb(25, 20, 20)
        } else {
            rgb(225, 55, 60)
        }
    });
    a.each(T_SQUAWK, |x, y, r, _| shade(if (x + y) % 7 == 0 { rgb(240, 200, 40) } else { rgb(215, 35, 35) }, r.range(0.85, 1.1)));
    a.each(T_SQUAWK_WING, |_, y, r, _| shade(if y < 6 { rgb(40, 90, 210) } else if y < 11 { rgb(245, 200, 40) } else { rgb(215, 35, 35) }, r.range(0.85, 1.1)));
    a.each(T_SQUAWK_FACE, |x, y, r, _| {
        if (4..6).contains(&y) && (x == 3 || x == 12) {
            rgb(15, 15, 15)
        } else if y >= 7 && (6..10).contains(&x) {
            rgb(50, 45, 40) // a big curved beak
        } else if y < 4 {
            rgb(240, 200, 40)
        } else {
            shade(rgb(215, 35, 35), r.range(0.9, 1.05))
        }
    });
    a.each(T_CLANK, |x, y, r, p| {
        // Riveted iron, with vines creeping up it.
        let vine = p.noise2(x as f32 / 3.0, y as f32 / 6.0) > 0.35;
        if vine {
            shade(rgb(60, 120, 40), r.range(0.8, 1.1))
        } else if (x % 8 == 1 || x % 8 == 6) && y % 5 == 2 {
            rgb(120, 115, 110)
        } else {
            shade(rgb(205, 200, 190), r.range(0.85, 1.05))
        }
    });
    a.each(T_CLANK_FACE, |x, y, r, _| {
        if (5..7).contains(&y) && (x == 4 || x == 11) {
            rgb(150, 30, 20) // glowing, slightly worried eyes
        } else if (3..5).contains(&y) && (3..13).contains(&x) {
            rgb(150, 145, 135) // a heavy brow
        } else {
            shade(rgb(200, 195, 185), r.range(0.88, 1.04))
        }
    });
    a.each(T_APPLE, |x, y, r, _| {
        let (dx, dy) = (x as f32 - 7.5, y as f32 - 8.5);
        if x == 8 && (2..5).contains(&y) {
            rgb(90, 60, 30)
        } else if (9..12).contains(&x) && y == 3 {
            rgb(70, 150, 50)
        } else if dx * dx * 1.1 + dy * dy < 26.0 {
            let hi = dx < -1.5 && dy < -1.5;
            if hi { rgb(255, 140, 130) } else { shade(rgb(210, 35, 35), r.range(0.85, 1.05)) }
        } else {
            [0, 0, 0, 0]
        }
    });
    a.each(T_FIRE, |x, y, r, p| {
        // Flickering tongues: taller in the middle, yellow at the heart.
        let fx = x as f32 / 15.0;
        let wave = p.noise2(fx * 5.0, 0.3) * 0.25;
        let height = 0.9 - (fx - 0.5).abs() * 0.9 + wave;
        let h = 1.0 - y as f32 / 15.0;
        if h > height || r.chance(0.08) {
            return [0, 0, 0, 0];
        }
        let core = h < height * 0.55 && (fx - 0.5).abs() < 0.3;
        if core { rgb(255, 225, 90) } else { shade(rgb(240, 110, 30), r.range(0.8, 1.1)) }
    });
    // Bottles: glass, filled with water or a potion's colour (splash ones have a band).
    let bottle = |a: &mut Atlas, tile: u16, fill: Option<[u8; 3]>, band: bool| {
        a.each(tile, |x, y, r, _| {
            let (x, y) = (x as i32, y as i32);
            let neck = (6..10).contains(&x) && (2..6).contains(&y);
            let body = (x - 8).pow(2) * 3 / 2 + (y - 10).pow(2) < 26 && y >= 5;
            let cork = (6..10).contains(&x) && y == 1;
            if cork {
                return rgb(150, 110, 70);
            }
            if !(neck || body) {
                return [0, 0, 0, 0];
            }
            let rim = ((x - 8).pow(2) * 3 / 2 + (y - 10).pow(2) >= 16) && body || (neck && (x == 6 || x == 9));
            if rim {
                return [210, 230, 240, 255];
            }
            match fill {
                Some(c) if y >= 8 => {
                    if band && y == 11 {
                        return rgb(80, 80, 85);
                    }
                    let k = if x < 7 && y < 11 { 1.3 } else { r.range(0.85, 1.0) };
                    [(c[0] as f32 * k).min(255.0) as u8, (c[1] as f32 * k).min(255.0) as u8, (c[2] as f32 * k).min(255.0) as u8, 255]
                }
                _ => [220, 235, 245, 90],
            }
        });
    };
    bottle(&mut a, T_GLASS_BOTTLE, None, false);
    bottle(&mut a, T_WATER_BOTTLE, Some([60, 100, 220]), false);
    for (i, p) in crate::potions::ALL.iter().enumerate() {
        bottle(&mut a, T_POTION_FIRST + i as u16, Some(p.colour()), false);
        bottle(&mut a, T_SPLASH_FIRST + i as u16, Some(p.colour()), true);
    }
    for (i, p) in crate::potions::BREWABLE[5..].iter().enumerate() {
        bottle(&mut a, T_POTION_EXTRA + i as u16, Some(p.colour()), false);
        bottle(&mut a, T_SPLASH_EXTRA + i as u16, Some(p.colour()), true);
    }
    a.each(T_TUSK, |x, y, r, _| {
        let (fx, fy) = (x as f32, y as f32);
        let curve = (fy - 3.0 - (fx - 3.0).powi(2) * 0.12).abs();
        if (3..14).contains(&x) && curve < 2.2 - fx * 0.12 { shade(rgb(235, 225, 195), r.range(0.85, 1.05)) } else { [0, 0, 0, 0] }
    });
    a.each(T_EMBER_SHROOM, |x, y, r, _| {
        let cap = (4..12).contains(&x) && (4..8).contains(&y) && !((x == 4 || x == 11) && y == 4);
        let stem = (7..9).contains(&x) && y >= 8;
        if cap {
            if (x + y) % 3 == 0 { rgb(255, 210, 90) } else { shade(rgb(230, 90, 30), r.range(0.85, 1.1)) }
        } else if stem {
            shade(rgb(200, 150, 110), r.range(0.9, 1.05))
        } else {
            [0, 0, 0, 0]
        }
    });
    a.each(T_BREWING_TOP, |x, y, r, _| {
        let edge = x == 0 || y == 0 || x == 15 || y == 15;
        shade(rgb(110, 105, 100), r.range(0.8, 1.05) * if edge { 0.7 } else { 1.0 })
    });
    a.each(T_BREWING_SIDE, |x, _, r, _| shade(rgb(200, 170, 60), r.range(0.8, 1.1) * if x % 4 == 0 { 0.8 } else { 1.0 }));
    // Contraptions.
    for (tile, lit) in [(T_ZTORCH_ON, true), (T_ZTORCH_OFF, false)] {
        a.each(tile, |x, y, r, _| {
            if (7..9).contains(&x) && y >= 6 {
                shade(rgb(120, 90, 50), r.range(0.8, 1.1))
            } else if (7..9).contains(&x) && (3..6).contains(&y) {
                if lit { if y == 3 { rgb(255, 200, 190) } else { rgb(240, 40, 30) } } else { rgb(90, 30, 25) }
            } else {
                [0, 0, 0, 0]
            }
        });
    }
    for (tile, lit) in [(T_REPEATER, false), (T_REPEATER_ON, true)] {
        a.each(tile, |x, y, r, _| {
            let arrow = x == 7 || x == 8 || (y < 6 && (x as i32 - 7).abs() + y as i32 <= 6 && (x as i32 - 7).abs() + y as i32 >= 4);
            let edge = x == 0 || y == 0 || x == 15 || y == 15;
            if arrow { if lit { rgb(250, 60, 40) } else { rgb(110, 30, 25) } } else { shade(rgb(160, 160, 160), r.range(0.85, 1.05) * if edge { 0.8 } else { 1.0 }) }
        });
    }
    // Comparators: two torches at the back and one at the front; the front one
    // lights in "half full" mode. Red when putting out power.
    for k in 0..4u16 {
        let (more, on) = (k >= 2, k % 2 == 1);
        a.each(T_COMPARATOR + k, |x, y, r, _| {
            let dot = |cx: i32, cy: i32| (x as i32 - cx).abs() <= 1 && (y as i32 - cy).abs() <= 1;
            let edge = x == 0 || y == 0 || x == 15 || y == 15;
            if dot(4, 11) || dot(11, 11) {
                if on { rgb(250, 60, 40) } else { rgb(110, 30, 25) }
            } else if dot(7, 3) {
                if more { rgb(250, 200, 60) } else { rgb(90, 80, 60) }
            } else {
                shade(rgb(160, 160, 160), r.range(0.85, 1.05) * if edge { 0.8 } else { 1.0 })
            }
        });
    }
    a.each(T_BEACON, |x, y, r, _| {
        // Glass round a glowing, pale core.
        let edge = x == 0 || y == 0 || x == 15 || y == 15;
        let core = (4..12).contains(&x) && (4..12).contains(&y);
        if edge {
            shade(rgb(200, 235, 240), r.range(0.9, 1.0))
        } else if core {
            shade(rgb(170, 250, 245), r.range(0.9, 1.1))
        } else {
            [180, 230, 240, 110]
        }
    });
    a.each(T_BEACON_BEAM, |_, y, r, _| {
        let a = 150 + (r.range(0.0, 60.0) as u8) - if y % 4 == 0 { 30 } else { 0 };
        [200, 250, 255, a]
    });
    a.each(T_PISTON_SIDE, |x, y, r, _| {
        if y < 4 { shade(rgb(160, 125, 80), r.range(0.85, 1.1)) } else { shade(rgb(115, 115, 115), r.range(0.8, 1.05) * if x % 5 == 0 { 0.85 } else { 1.0 }) }
    });
    a.each(T_PISTON_FACE, |x, y, r, _| {
        let edge = x == 0 || y == 0 || x == 15 || y == 15;
        shade(rgb(165, 130, 85), r.range(0.85, 1.1) * if edge { 0.75 } else { 1.0 })
    });
    a.each(T_STICKY_FACE, |x, y, r, _| {
        let edge = x < 2 || y < 2 || x > 13 || y > 13;
        if edge { shade(rgb(165, 130, 85), r.range(0.85, 1.1)) } else { shade(rgb(110, 190, 90), r.range(0.8, 1.1)) }
    });
    a.each(T_PISTON_BACK, |x, y, r, _| {
        let hole = (6..10).contains(&x) && (6..10).contains(&y);
        if hole { rgb(60, 60, 60) } else { shade(rgb(115, 115, 115), r.range(0.8, 1.05)) }
    });
    a.each(T_HOPPER_TOP, |x, y, r, _| {
        let rim = x < 2 || y < 2 || x > 13 || y > 13;
        if rim { shade(rgb(70, 70, 75), r.range(0.85, 1.1)) } else { shade(rgb(35, 35, 38), r.range(0.8, 1.1)) }
    });
    a.each(T_HOPPER_SIDE, |x, y, r, _| shade(rgb(75, 75, 80), r.range(0.8, 1.1) * if y % 5 == 0 || x % 8 == 0 { 0.8 } else { 1.0 }));
    // The Hollow.
    a.each(T_HOLLOW_STONE, |_, _, r, _| shade(rgb(222, 222, 170), r.range(0.85, 1.05)));
    a.each(T_HOLLOW_PORTAL, |_, _, r, _| if r.chance(0.06) { rgb(220, 230, 255) } else { shade(rgb(15, 25, 40), r.range(0.6, 1.4)) });
    let eye_frame = |a: &mut Atlas, tile: u16, eye: bool| {
        a.each(tile, |x, y, r, _| {
            let (dx, dy) = (x as i32 - 8, y as i32 - 8);
            let hole = dx * dx + dy * dy < 16;
            if hole && eye {
                if dx * dx + dy * dy < 4 { rgb(10, 20, 15) } else { rgb(40, 170, 120) }
            } else if hole {
                rgb(30, 60, 50)
            } else {
                shade(rgb(60, 110, 90), r.range(0.8, 1.1))
            }
        });
    };
    eye_frame(&mut a, T_EYE_FRAME_TOP, false);
    eye_frame(&mut a, T_EYE_FRAME_FULL, true);
    a.each(T_EYE_FRAME_SIDE, |_, y, r, _| if y < 4 { shade(rgb(60, 110, 90), r.range(0.8, 1.1)) } else { shade(rgb(222, 222, 170), r.range(0.85, 1.05)) });
    a.each(T_WYRM_CRYSTAL, |x, y, r, _| {
        let edge = x == 0 || y == 0 || x == 15 || y == 15 || x == y || x + y == 15;
        if edge { rgb(255, 230, 250) } else { shade(rgb(230, 120, 220), r.range(0.8, 1.2)) }
    });
    a.each(T_WYRM_EGG, |_, _, r, _| if r.chance(0.08) { rgb(170, 60, 200) } else { shade(rgb(25, 10, 35), r.range(0.8, 1.2)) });
    a.each(T_STARING_EYE, |x, y, _, _| {
        let (dx, dy) = (x as i32 - 8, y as i32 - 8);
        let d = dx * dx + dy * dy;
        if d < 6 { rgb(10, 20, 15) } else if d < 30 { rgb(40, 170, 120) } else if d < 40 { rgb(20, 90, 70) } else { [0, 0, 0, 0] }
    });
    a.each(T_WYRM_SKIN, |x, y, r, _| shade(rgb(30, 25, 40), r.range(0.7, 1.2) * if (x + y) % 6 == 0 { 0.7 } else { 1.0 }));
    a.each(T_WYRM_WING, |x, _, r, _| shade(rgb(55, 40, 70), r.range(0.8, 1.1) * if x % 4 == 0 { 0.6 } else { 1.0 }));
    a.copy(T_WYRM_SKIN, T_WYRM_FACE);
    for (x, y) in [(3, 6), (4, 6), (11, 6), (12, 6)] {
        a.set(T_WYRM_FACE, x, y, rgb(200, 60, 230));
    }
    a.copy(T_COBBLE, T_DISPENSER_FACE);
    for y in 5..11 {
        for x in 4..12 {
            let edge = y == 5 || y == 10 || x == 4 || x == 11;
            a.set(T_DISPENSER_FACE, x, y, if edge { rgb(70, 70, 70) } else { rgb(25, 25, 25) });
        }
    }
    a.each(T_LADDER, |x, y, r, _| {
        let rail = !(2..=13).contains(&x);
        let rung = y % 4 == 1 && (2..14).contains(&x);
        if rail || rung { shade(rgb(125, 90, 50), r.range(0.8, 1.1)) } else { [0, 0, 0, 0] }
    });
    a.each(T_TRAPDOOR, |x, y, r, _| {
        let frame = x < 2 || y < 2 || x > 13 || y > 13 || x == 7 || x == 8;
        let hole = !frame && (y % 5 == 3) && (3..13).contains(&x);
        if hole { [0, 0, 0, 0] } else { shade(rgb(150, 110, 65), r.range(0.8, 1.1) * if frame { 0.85 } else { 1.0 }) }
    });
    for c in 0..8u16 {
        let tint = crate::carpentry::colour_rgb(c as usize);
        if c > 0 {
            let to = T_DYED_WOOL + c - 1;
            a.copy(T_WOOL, to);
            for y in 0..16 {
                for x in 0..16 {
                    let p = a.get(to, x, y);
                    let k = (p[0] as f32 + p[1] as f32 + p[2] as f32) / (3.0 * 235.0);
                    a.set(to, x, y, rgb((tint[0] as f32 * k).min(255.0) as u8, (tint[1] as f32 * k).min(255.0) as u8, (tint[2] as f32 * k).min(255.0) as u8));
                }
            }
        }
        let to = T_STAINED_GLASS + c;
        a.each(to, |x, y, r, _| {
            let edge = x == 0 || y == 0 || x == 15 || y == 15;
            let glint = (x + y == 9 || x + y == 11) && (3..9).contains(&x);
            let pane = (x + y) % 3 == 0;
            let m = |k: f32| [(tint[0] as f32 * k) as u8, (tint[1] as f32 * k) as u8, (tint[2] as f32 * k) as u8, 255];
            if edge { m(r.range(0.6, 0.75)) } else if glint { [255, 255, 255, 200] } else if pane { m(r.range(0.9, 1.0)) } else { [0, 0, 0, 0] }
        });
        a.each(T_DYE_FIRST + c, |x, y, r, _| {
            let (dx, dy) = (x as f32 - 7.5, y as f32 - 9.0);
            let blob = dx * dx + dy * dy * 1.4 < 22.0 && y > 4;
            let nozzle = (6..10).contains(&x) && (3..6).contains(&y);
            if nozzle {
                rgb(200, 200, 205)
            } else if blob {
                let k = if dx < -1.0 && dy < -1.0 { 1.25 } else { r.range(0.85, 1.05) };
                [(tint[0] as f32 * k).min(255.0) as u8, (tint[1] as f32 * k).min(255.0) as u8, (tint[2] as f32 * k).min(255.0) as u8, 255]
            } else {
                [0, 0, 0, 0]
            }
        });
    }
    a.sprite(T_MAP, &MAP_SPRITE, &[('#', rgb(90, 70, 40)), ('p', rgb(225, 210, 170)), ('g', rgb(110, 160, 80)), ('b', rgb(80, 120, 210)), ('r', rgb(200, 40, 40))]);
    a.sprite(T_BOAT_ITEM, &BOAT_SPRITE, &[('#', rgb(60, 40, 20)), ('w', rgb(170, 130, 78)), ('d', rgb(130, 95, 55))]);
    a.sprite(T_CART_ITEM, &CART_SPRITE, &[('#', rgb(30, 30, 35)), ('b', rgb(120, 120, 128)), ('h', rgb(180, 180, 188)), ('k', rgb(50, 50, 55))]);
    // Loaded carts: the cart with a chest lid, or a hopper's funnel, poking out of the top.
    a.copy(T_CART_ITEM, T_CHEST_CART_ITEM);
    a.copy(T_CART_ITEM, T_HOPPER_CART_ITEM);
    for x in 3..13 {
        for y in 2..7 {
            let chest = if y == 4 && (7..9).contains(&x) { rgb(200, 190, 90) } else if y == 2 || x == 3 || x == 12 { rgb(110, 75, 40) } else { rgb(160, 115, 60) };
            a.set(T_CHEST_CART_ITEM, x, y, chest);
            let funnel = y == 2 || ((y as i32 - 2) <= (x as i32 - 3).min(12 - x as i32));
            if funnel {
                a.set(T_HOPPER_CART_ITEM, x, y, if y == 2 { rgb(60, 60, 65) } else { rgb(95, 95, 102) });
            }
        }
    }
    // ---- Hmmers
    a.copy(T_SKIN, T_HMM_FACE);
    for (x, y, c) in [(3, 6, rgb(255, 255, 255)), (4, 6, rgb(40, 110, 40)), (11, 6, rgb(40, 110, 40)), (12, 6, rgb(255, 255, 255))] {
        a.set(T_HMM_FACE, x, y, c);
    }
    for x in 2..14 {
        a.set(T_HMM_FACE, x, 4, rgb(60, 40, 25));
    }
    a.each(T_HMM_ROBE, |x, _, r, _| shade(rgb(115, 75, 45), r.range(0.85, 1.08) * if x % 7 == 0 { 0.85 } else { 1.0 }));
    // ---- Animals
    a.speckle(T_GALLOPER, rgb(140, 95, 55), 0.08);
    a.copy(T_GALLOPER, T_GALLOP_FACE);
    // The front of the muzzle: two nostrils. The eyes are on the head's sides.
    for (x, y) in [(4, 7), (5, 7), (10, 7), (11, 7), (4, 8), (11, 8)] {
        a.set(T_GALLOP_FACE, x, y, rgb(50, 32, 20));
    }
    a.copy(T_GALLOPER, T_GALLOP_EYE);
    for (x, y, c) in [(7, 5, rgb(20, 15, 10)), (8, 5, rgb(20, 15, 10)), (7, 6, rgb(20, 15, 10)), (8, 6, rgb(240, 235, 225))] {
        a.set(T_GALLOP_EYE, x, y, c);
    }
    a.speckle(T_GALLOP_MANE, rgb(45, 30, 20), 0.1);
    a.speckle(T_SADDLE_LEATHER, rgb(120, 60, 30), 0.06);
    a.each(T_SADDLE, |x, y, r, _| {
        let (dx, dy) = (x as f32 - 7.5, y as f32 - 8.0);
        let seat = dx * dx * 0.5 + dy * dy * 1.6 < 22.0 && y > 3;
        let strap = (x == 4 || x == 11) && (8..14).contains(&y);
        if strap { rgb(70, 40, 20) } else if seat { shade(rgb(130, 65, 30), r.range(0.85, 1.1) * if dy < -1.0 { 1.15 } else { 1.0 }) } else { [0, 0, 0, 0] }
    });
    a.speckle(T_WOOF_SKIN, rgb(200, 196, 190), 0.07);
    a.copy(T_WOOF_SKIN, T_WOOF_FACE);
    for (x, y, c) in [(3, 5, rgb(20, 20, 20)), (4, 5, rgb(255, 255, 255)), (11, 5, rgb(255, 255, 255)), (12, 5, rgb(20, 20, 20))] {
        a.set(T_WOOF_FACE, x, y, c);
    }
    // The snout in front carries the nose.
    a.copy(T_WOOF_SKIN, T_WOOF_NOSE);
    for y in 3..13 {
        for x in 3..13 {
            a.set(T_WOOF_NOSE, x, y, if y < 7 && (4..12).contains(&x) { rgb(40, 30, 30) } else { rgb(170, 165, 160) });
        }
    }
    a.each(T_COLLAR, |x, _, r, _| if x % 5 == 2 { rgb(240, 210, 60) } else { shade(rgb(200, 30, 35), r.range(0.9, 1.05)) });
    a.sprite(T_SHEARS, &SHEARS_SPRITE, &[('#', rgb(40, 40, 45)), ('b', rgb(200, 200, 208)), ('h', rgb(240, 240, 245)), ('r', rgb(190, 40, 40))]);
    for (tile, fill) in [(T_BUCKET, None), (T_WATER_BUCKET, Some(rgb(50, 95, 220))), (T_LAVA_BUCKET, Some(rgb(245, 120, 25)))] {
        let pal = [('#', rgb(50, 50, 55)), ('b', rgb(190, 190, 198)), ('d', rgb(130, 130, 138)), ('h', rgb(235, 235, 240)), ('f', fill.unwrap_or(rgb(70, 70, 76)))];
        a.sprite(tile, &BUCKET_SPRITE, &pal);
    }
    a.sprite(T_HUNGER_ICON, &DRUMSTICK, &[('#', rgb(60, 30, 10)), ('p', rgb(170, 95, 40)), ('P', rgb(205, 130, 60)), ('w', rgb(235, 190, 120)), ('b', rgb(235, 230, 210))]);
    a.sprite(T_ARMOR_ICON, &CHESTPLATE, &[('#', rgb(30, 30, 30)), ('b', rgb(210, 210, 215)), ('d', rgb(150, 150, 155)), ('h', rgb(245, 245, 250))]);
    // Empty ones keep the outline over a flat dark fill, like an empty heart.
    let hollow = rgb(52, 40, 34);
    a.sprite(T_HUNGER_EMPTY, &DRUMSTICK, &[('#', rgb(30, 18, 8)), ('p', hollow), ('P', hollow), ('w', hollow), ('b', rgb(70, 62, 56))]);
    let hollow = rgb(48, 48, 52);
    a.sprite(T_ARMOR_EMPTY, &CHESTPLATE, &[('#', rgb(20, 20, 20)), ('b', hollow), ('d', hollow), ('h', hollow)]);

    let cooked = |raw: Rgba| shade([raw[0] / 2 + 70, raw[1] / 2 + 40, raw[2] / 3 + 20, 255], 1.0);
    a.sprite(T_COOKED_CHOP, &PORK, &[('#', rgb(60, 30, 15)), ('p', cooked(rgb(230, 110, 110))), ('P', cooked(rgb(245, 150, 150))), ('w', rgb(230, 200, 160)), ('b', rgb(230, 225, 200))]);
    a.sprite(T_COOKED_MUTTON, &PORK, &[('#', rgb(50, 25, 10)), ('p', cooked(rgb(200, 60, 60))), ('P', cooked(rgb(225, 90, 85))), ('w', rgb(220, 190, 150)), ('b', rgb(230, 225, 200))]);
    a.sprite(T_COOKED_CLUCKETS, &DRUMSTICK, &[('#', rgb(90, 50, 20)), ('p', rgb(200, 130, 60)), ('P', rgb(225, 160, 80)), ('w', rgb(245, 200, 130)), ('b', rgb(235, 230, 210))]);
    a.sprite(T_STEAK, &PORK, &[('#', rgb(40, 15, 5)), ('p', rgb(110, 60, 30)), ('P', rgb(140, 80, 40)), ('w', rgb(200, 160, 120)), ('b', rgb(230, 225, 200))]);
    let fish = |body: Rgba, fin: Rgba, belly: Rgba| [('#', shade(body, 0.45)), ('f', body), ('F', fin), ('b', belly), ('w', rgb(255, 255, 255)), ('k', rgb(10, 10, 10))];
    a.sprite(T_COOKED_COD, &FISH, &fish(rgb(210, 170, 100), rgb(170, 130, 70), rgb(240, 210, 160)));
    a.sprite(T_COOKED_SALMON, &FISH, &fish(rgb(220, 120, 70), rgb(170, 80, 50), rgb(245, 180, 130)));
    a.sprite(T_COOKED_PUFFER, &FISH, &fish(rgb(200, 150, 50), rgb(160, 110, 30), rgb(230, 200, 120)));
    a.sprite(T_BAKED_POTATO, &SPUD, &[('#', rgb(70, 40, 15)), ('b', rgb(215, 160, 80)), ('d', rgb(170, 110, 50))]);
    a.sprite(T_COOKED_BOOT, &BOOT, &[('#', rgb(20, 10, 5)), ('b', rgb(70, 40, 20)), ('d', rgb(45, 25, 10)), ('w', rgb(120, 120, 120))]);
    a.each(T_FLAME, |x, y, _, _| {
        let cx = (x as f32 - 7.5).abs();
        let top = 3.0 + cx * 1.6;
        if (y as f32) < top || cx > 6.0 { [0, 0, 0, 0] } else if cx < 2.5 && y > 9 { rgb(255, 240, 150) } else { rgb(250, 140, 30) }
    });
    a.each(T_ARROW_UI, |x, y, _, _| {
        let shaft = (6..10).contains(&y) && x < 10;
        let head = x >= 10 && (y as i32 - 7).unsigned_abs() as usize + x <= 17;
        if shaft || head { rgb(255, 255, 255) } else { [0, 0, 0, 0] }
    });

    paint_new_things(&mut a);
    paint_bees(&mut a);
    paint_critters(&mut a);
    paint_ancient(&mut a);
    paint_music(&mut a);
    paint_scorch_and_raids(&mut a);
    paint_trials_and_friends(&mut a);

    // Sky & misc
    a.each(T_WHITE, |_, _, _, _| rgb(255, 255, 255));
    a.each(T_CLOUD, |_, _, _, _| [255, 255, 255, 255]);
    a.each(T_SUN, |x, y, _, _| {
        let d = ((x as f32 - 7.5).abs()).max((y as f32 - 7.5).abs());
        if d < 5.0 { rgb(255, 255, 210) } else if d < 7.0 { [255, 240, 150, 150] } else { [255, 230, 120, 50] }
    });
    a.each(T_MOON, |x, y, r, _| {
        let d = ((x as f32 - 7.5).abs()).max((y as f32 - 7.5).abs());
        if d < 5.0 { shade(rgb(225, 225, 235), r.range(0.8, 1.0)) } else { [0, 0, 0, 0] }
    });
    for p in 0..crate::skies::PHASES {
        let f = crate::skies::brightness(p);
        a.each(T_MOON_PHASES + p as u16, |x, y, r, _| {
            let d = ((x as f32 - 7.5).abs()).max((y as f32 - 7.5).abs());
            if d >= 5.0 {
                return [0, 0, 0, 0];
            }
            // Waning, the dark creeps in from the right; waxing, it leaves from the left.
            let u = (x as f32 - 7.5) / 5.0;
            let lit = if p == 0 { true } else if p < crate::skies::PHASES / 2 { u < 2.0 * f - 1.0 } else { u > 1.0 - 2.0 * f };
            if lit { shade(rgb(225, 225, 235), r.range(0.8, 1.0)) } else { shade(rgb(45, 45, 60), r.range(0.8, 1.0)) }
        });
    }
    home_tiles(&mut a);

    a.px
}

/// v0.1.20: campfires, smokers, blast furnaces, barrels, turtle eggs, paintings and treasure.
fn home_tiles(a: &mut Atlas) {
    a.each(T_CAMPFIRE_SIDE, |x, y, r, _| {
        let bark = (x * 3 + y) % 5 == 0;
        let ember = y > 11 && (x * 7 + y * 3) % 6 == 0;
        if ember { rgb(255, 140, 30) } else { shade(if bark { rgb(70, 50, 30) } else { rgb(105, 75, 45) }, r.range(0.8, 1.05)) }
    });
    a.each(T_CAMPFIRE_TOP, |x, y, r, _| {
        let ash = (x * 5 + y * 7) % 4 == 0;
        let ember = (x * 11 + y * 13) % 9 == 0;
        if ember { rgb(255, 120, 20) } else { shade(if ash { rgb(60, 55, 55) } else { rgb(95, 70, 45) }, r.range(0.8, 1.05)) }
    });
    // Smoker: a furnace in a log jacket; blast furnace: iron-banded stone.
    a.copy(T_FURNACE_SIDE, T_SMOKER_SIDE);
    a.copy(T_FURNACE_SIDE, T_BLAST_SIDE);
    a.copy(T_FURNACE_LIT, T_SMOKER_LIT);
    a.copy(T_FURNACE_LIT, T_BLAST_LIT);
    for t in [T_SMOKER_SIDE, T_SMOKER_LIT] {
        for y in 0..16 {
            for x in 0..16 {
                if !(3..13).contains(&x) || !(4..14).contains(&y) {
                    let c = if (x + y * 3) % 7 == 0 { rgb(70, 50, 30) } else { rgb(110, 80, 50) };
                    a.set(t, x, y, shade(c, 0.9 + ((x * 7 + y) % 5) as f32 * 0.04));
                }
            }
        }
    }
    for t in [T_BLAST_SIDE, T_BLAST_LIT] {
        for y in 0..16 {
            for x in 0..16 {
                if !(3..=13).contains(&y) || !(2..=13).contains(&x) {
                    let c = if y % 4 == 0 { rgb(70, 72, 78) } else { rgb(130, 132, 138) };
                    a.set(t, x, y, shade(c, 0.9 + ((x * 3 + y) % 5) as f32 * 0.04));
                }
            }
        }
    }
    a.each(T_SMOKER_TOP, |x, y, r, _| {
        let vent = (5..11).contains(&x) && (5..11).contains(&y) && (x + y) % 2 == 0;
        if vent { rgb(30, 30, 30) } else { shade(rgb(100, 72, 45), r.range(0.82, 1.05)) }
    });
    a.each(T_BLAST_TOP, |x, y, r, _| {
        let vent = (4..12).contains(&x) && (4..12).contains(&y) && x % 2 == 0;
        if vent { rgb(35, 35, 40) } else { shade(rgb(125, 127, 133), r.range(0.82, 1.05)) }
    });
    a.each(T_BARREL_SIDE, |x, y, r, _| {
        let hoop = y == 2 || y == 13;
        let seam = x % 4 == 0;
        if hoop { shade(rgb(80, 80, 85), r.range(0.9, 1.05)) } else { shade(if seam { rgb(95, 65, 35) } else { rgb(140, 100, 55) }, r.range(0.85, 1.05)) }
    });
    a.each(T_BARREL_TOP, |x, y, r, _| {
        let rim = x == 0 || y == 0 || x == 15 || y == 15;
        let hole = (6..10).contains(&x) && (6..10).contains(&y);
        if rim { rgb(80, 80, 85) } else if hole { rgb(55, 35, 20) } else { shade(if x % 4 == 0 { rgb(110, 75, 40) } else { rgb(150, 110, 60) }, r.range(0.85, 1.05)) }
    });
    a.each(T_TURTLE_EGG, |x, y, r, _| {
        let spot = (x * 5 + y * 3) % 7 == 0;
        shade(if spot { rgb(120, 170, 110) } else { rgb(235, 230, 205) }, r.range(0.9, 1.05))
    });
    // Eight pictures, each in a wooden frame.
    for i in 0..PAINTINGS {
        a.each(T_PAINTING_FIRST + i, |x, y, r, _| {
            if x == 0 || y == 0 || x == 15 || y == 15 {
                return shade(rgb(120, 85, 50), r.range(0.85, 1.05));
            }
            let (fx, fy) = (x as f32 / 15.0, y as f32 / 15.0);
            let c = match i {
                // Sunset over the sea.
                0 => {
                    let sun = (fx - 0.5).powi(2) + (fy - 0.55).powi(2) < 0.04;
                    if fy > 0.6 { rgb(40, 80, 150) } else if sun { rgb(255, 210, 80) } else { rgb(250, (120.0 + 80.0 * fy) as u8, 80) }
                }
                // Mountains with snow.
                1 => {
                    let peak = 0.25 + (fx * 6.0).sin().abs() * 0.35;
                    if fy < peak { rgb(150, 200, 240) } else if fy < peak + 0.12 { rgb(240, 240, 245) } else { rgb(90, 110, 90) }
                }
                // A Hisser, looking smug.
                2 => {
                    let face = [(4, 5), (5, 5), (10, 5), (11, 5), (4, 6), (5, 6), (10, 6), (11, 6), (7, 8), (8, 8), (6, 9), (9, 9), (6, 10), (9, 10), (7, 9), (8, 9)];
                    if face.contains(&(x, y)) { rgb(20, 30, 20) } else { shade(rgb(80, 170, 70), r.range(0.85, 1.1)) }
                }
                // A boat at night.
                3 => {
                    let boat = (11..13).contains(&y) && (4..12).contains(&x) || y == 13 && (5..11).contains(&x);
                    let star = (x * 7 + y * 11) % 17 == 0 && y < 8;
                    if boat { rgb(120, 80, 40) } else if star { rgb(250, 250, 200) } else if fy > 0.85 { rgb(30, 60, 110) } else { rgb(20, 25, 60) }
                }
                // Flowers in a vase.
                4 => {
                    let vase = (6..10).contains(&x) && y > 9;
                    let bloom = [(5, 4), (8, 3), (10, 5), (7, 5), (6, 6), (9, 6)].iter().any(|&(bx, by)| (x as i32 - bx).abs() + (y as i32 - by).abs() < 2);
                    if vase { rgb(70, 110, 180) } else if bloom { [rgb(230, 60, 80), rgb(250, 200, 60)][(x + y) % 2] } else if (7..9).contains(&x) && y > 5 { rgb(60, 130, 60) } else { rgb(235, 225, 200) }
                }
                // Stripes, very modern.
                5 => [rgb(220, 70, 60), rgb(240, 200, 70), rgb(60, 120, 200), rgb(240, 240, 235)][((x + y) / 3) % 4],
                // A pig in a field.
                6 => {
                    let pig = (5..11).contains(&x) && (7..11).contains(&y) || (10..13).contains(&x) && (6..9).contains(&y);
                    let legs = y == 11 && (x == 6 || x == 9);
                    if pig { rgb(240, 170, 170) } else if legs { rgb(200, 130, 130) } else if fy > 0.7 { rgb(100, 170, 70) } else { rgb(160, 210, 245) }
                }
                // The moon over the hills.
                _ => {
                    let moon = (fx - 0.7).powi(2) + (fy - 0.3).powi(2) < 0.02;
                    let hill = fy > 0.65 + (fx * 4.0).sin() * 0.08;
                    if moon { rgb(240, 240, 220) } else if hill { rgb(40, 70, 50) } else { rgb(30, 35, 80) }
                }
            };
            shade(c, r.range(0.93, 1.04))
        });
    }
    a.sprite(T_TREASURE_MAP, &MAP_SPRITE, &[('#', rgb(90, 70, 40)), ('p', rgb(215, 190, 140)), ('g', rgb(190, 160, 110)), ('b', rgb(80, 120, 210)), ('r', rgb(220, 30, 30))]);
    // Backpacks: the same bag in leather, iron-trimmed and gold-trimmed.
    for (t, trim) in [(T_BACKPACK, rgb(110, 70, 40)), (T_BIG_BACKPACK, rgb(190, 190, 200)), (T_HUGE_BACKPACK, rgb(240, 200, 60))] {
        a.sprite(t, &BACKPACK_SPRITE, &[('#', rgb(60, 35, 20)), ('b', rgb(150, 95, 55)), ('d', rgb(115, 70, 40)), ('t', trim), ('k', rgb(30, 20, 15))]);
    }
    a.sprite(T_TURTLE_SCUTE, &SCUTE_SPRITE, &[('#', rgb(40, 90, 40)), ('b', rgb(80, 160, 70)), ('h', rgb(130, 200, 110)), ('d', rgb(55, 120, 50))]);
    a.each(T_TURTLE_SHELL, |x, y, r, _| {
        let dome = ((x as f32 - 7.5) / 7.0).powi(2) + ((y as f32 - 9.0) / 6.0).powi(2) < 1.0 && y < 13;
        let plate = (x + y * 2) % 5 == 0;
        if dome { shade(if plate { rgb(60, 120, 50) } else { rgb(90, 170, 70) }, r.range(0.88, 1.05)) } else { [0, 0, 0, 0] }
    });
    a.each(T_TURTLE_WORN, |x, y, r, _| shade(if (x + y * 2) % 5 == 0 { rgb(60, 120, 50) } else { rgb(90, 170, 70) }, r.range(0.88, 1.05)));
    mob_tiles(a);
    masonry_tiles(a);
    cave_tiles(a);
    temple_tiles(a);
    monument_tiles(a);
    night_tiles(a);
    wilter_tiles(a);
}

/// v0.2's night threats and small creatures.
fn night_tiles(a: &mut Atlas) {
    a.each(T_GLOW_INK_SAC, |x, y, r, _| {
        let (dx, dy) = (x as f32 - 7.5, y as f32 - 8.5);
        let sac = dx * dx / 25.0 + dy * dy / 36.0 < 1.0;
        let tie = (6..10).contains(&x) && (1..4).contains(&y);
        if sac { shade(if (x * 3 + y) % 5 == 0 { rgb(220, 255, 250) } else { rgb(60, 200, 190) }, r.range(0.9, 1.06)) } else if tie { rgb(30, 90, 90) } else { [0, 0, 0, 0] }
    });
    // A Witch: a purple robe, a green-tinged face with a big nose and a wart, a black hat.
    a.each(T_WITCH_ROBE, |x, y, r, _| shade(if y % 5 == 0 { rgb(70, 40, 80) } else if x % 7 == 3 { rgb(60, 110, 50) } else { rgb(95, 55, 105) }, r.range(0.88, 1.06)));
    a.each(T_WITCH_FACE, |x, y, r, _| {
        let eye = y == 6 && (x == 4 || x == 11);
        let nose = (7..9).contains(&x) && (7..12).contains(&y);
        let wart = x == 9 && y == 10;
        if eye { rgb(150, 230, 90) } else if wart { rgb(70, 110, 40) } else if nose { rgb(170, 160, 120) } else { shade(rgb(185, 175, 140), r.range(0.92, 1.05)) }
    });
    a.each(T_WITCH_HAT, |x, y, r, _| shade(if y == 12 { rgb(110, 60, 120) } else { rgb(35, 30, 40) }, if (x + y) % 6 == 0 { 0.85 } else { 1.0 } * r.range(0.92, 1.06)));
    // A desert Groaner: dried-out, sandy, in rags.
    a.each(T_DESERT_SKIN, |_, _, r, _| shade(rgb(165, 140, 95), r.range(0.85, 1.08)));
    a.each(T_DESERT_FACE, |x, y, r, _| {
        let eye = (5..7).contains(&y) && (x == 4 || x == 5 || x == 10 || x == 11);
        let mouth = y == 11 && (5..11).contains(&x);
        if eye { rgb(40, 30, 20) } else if mouth { rgb(90, 70, 45) } else { shade(rgb(165, 140, 95), r.range(0.88, 1.06)) }
    });
    a.each(T_DESERT_CLOTH, |x, y, r, _| shade(if (x + y * 3) % 7 == 0 { rgb(120, 95, 60) } else { rgb(190, 170, 120) }, r.range(0.86, 1.06)));
    // A snowy Rattler: frosty bones, ragged grey wrappings.
    a.each(T_STRAY_BONE, |x, y, r, _| shade(if (x + y) % 5 == 0 { rgb(200, 225, 230) } else { rgb(165, 190, 195) }, r.range(0.9, 1.06)));
    a.each(T_STRAY_FACE, |x, y, r, _| {
        let eye = (5..8).contains(&y) && ((3..6).contains(&x) || (10..13).contains(&x));
        let teeth = y == 11 && x % 2 == 0 && (4..12).contains(&x);
        if eye { rgb(30, 40, 50) } else if teeth { rgb(120, 140, 145) } else { shade(rgb(175, 200, 205), r.range(0.9, 1.05)) }
    });
    a.each(T_STRAY_CLOTH, |x, y, r, _| shade(if (x * 2 + y) % 6 < 2 { rgb(80, 95, 100) } else { rgb(120, 135, 140) }, r.range(0.86, 1.06)));
    // A Glow Squid: teal with glowing speckles.
    a.each(T_GLOW_SQUID, |x, y, r, _| if (x * 7 + y * 5) % 11 == 0 { rgb(200, 255, 245) } else { shade(rgb(40, 140, 140), r.range(0.85, 1.1)) });
    a.each(T_GLOW_SQUID_FACE, |x, y, r, _| {
        let eye = (5..9).contains(&y) && ((2..5).contains(&x) || (11..14).contains(&x));
        if eye { if y == 6 { rgb(250, 255, 250) } else { rgb(20, 60, 60) } } else { shade(rgb(40, 140, 140), r.range(0.85, 1.1)) }
    });
    // A Bat: brown fur, leathery wings.
    a.each(T_BAT, |x, y, r, _| if (y == 4 || y == 5) && (x == 5 || x == 10) { rgb(20, 15, 15) } else { shade(rgb(80, 60, 45), r.range(0.85, 1.1)) });
    a.each(T_BAT_WING, |x, _, r, _| shade(if x % 4 == 0 { rgb(40, 30, 25) } else { rgb(65, 50, 40) }, r.range(0.9, 1.05)));
    // An Allay: a little blue spirit with big eyes.
    a.each(T_ALLAY, |x, y, r, _| shade(if (x + y) % 6 == 0 { rgb(170, 230, 255) } else { rgb(90, 190, 245) }, r.range(0.92, 1.06)));
    a.each(T_ALLAY_FACE, |x, y, r, _| {
        let eye = (6..10).contains(&y) && ((3..6).contains(&x) || (10..13).contains(&x));
        if eye { if y == 6 { rgb(255, 255, 255) } else { rgb(20, 40, 90) } } else { shade(rgb(110, 205, 250), r.range(0.92, 1.06)) }
    });
    a.each(T_ALLAY_WING, |x, y, _, _| if (x + y) % 3 == 0 { [235, 250, 255, 220] } else { [190, 230, 255, 170] });
}

/// v0.2 part 2: the Wilter, its makings and its star; rope and the Support Gauge.
fn wilter_tiles(a: &mut Atlas) {
    // Sorrow Sand: dark brown sand with faint faces in it.
    a.each(T_SORROW_SAND, |x, y, r, _| {
        let face = (x % 8 == 2 || x % 8 == 5) && y % 8 == 3 || (y % 8 == 6 && (2..6).contains(&(x % 8)));
        shade(if face { rgb(60, 45, 35) } else { rgb(95, 75, 60) }, r.range(0.85, 1.08))
    });
    // A Charred Skull: sooty bone, and a grim face on its sides.
    a.each(T_CHARRED_SKULL, |x, y, r, _| shade(if (x * 3 + y) % 7 == 0 { rgb(35, 35, 38) } else { rgb(55, 55, 58) }, r.range(0.88, 1.06)));
    a.each(T_CHARRED_SKULL_FACE, |x, y, r, _| {
        let eye = (5..8).contains(&y) && ((3..6).contains(&x) || (10..13).contains(&x));
        let teeth = y == 11 && (4..12).contains(&x) && x % 2 == 0;
        if eye || teeth { rgb(15, 12, 12) } else { shade(rgb(55, 55, 58), r.range(0.88, 1.06)) }
    });
    // A Starred Beacon: the beacon's glass, with a white star in it.
    a.each(T_STARRED_BEACON, |x, y, r, _| {
        let (dx, dy) = ((x as i32 - 8).abs(), (y as i32 - 8).abs());
        let star = dx + dy < 4 || (dx < 1 && dy < 6) || (dy < 1 && dx < 6);
        let edge = x == 0 || y == 0 || x == 15 || y == 15;
        if star { rgb(255, 250, 220) } else if edge { rgb(200, 240, 245) } else { shade(rgb(90, 200, 210), r.range(0.9, 1.05)) }
    });
    // Rope: twisted fibres, see-through either side.
    a.each(T_ROPE, |x, y, r, _| if (6..10).contains(&x) { shade(if (x + y) % 3 == 0 { rgb(120, 90, 55) } else { rgb(170, 135, 85) }, r.range(0.9, 1.05)) } else { [0, 0, 0, 0] });
    // The Wilter Star: a pale four-pointed star.
    a.each(T_WILTER_STAR, |x, y, _, _| {
        let (dx, dy) = ((x as f32 - 7.5).abs(), (y as f32 - 7.5).abs());
        let star = dx * dy < 3.0 && dx + dy < 9.0;
        if star { if dx + dy < 3.0 { rgb(255, 255, 255) } else { rgb(225, 225, 245) } } else { [0, 0, 0, 0] }
    });
    // The Support Gauge: a copper dial with a needle.
    a.each(T_SUPPORT_GAUGE, |x, y, _, _| {
        let (dx, dy) = (x as f32 - 7.5, y as f32 - 8.5);
        let d = (dx * dx + dy * dy).sqrt();
        let needle = dy < 0.0 && (dx + dy * 0.5).abs() < 0.8 && d < 5.5;
        if d > 6.5 { [0, 0, 0, 0] } else if d > 5.5 { rgb(190, 110, 70) } else if needle { rgb(200, 40, 30) } else if dy > 2.0 { rgb(60, 60, 60) } else { rgb(235, 225, 200) }
    });
    // A Charred Rattler: sooty black bones.
    a.each(T_CHARRED_BONE, |x, y, r, _| shade(if (x * 3 + y) % 7 == 0 { rgb(30, 30, 32) } else { rgb(50, 50, 54) }, r.range(0.88, 1.06)));
    a.each(T_CHARRED_FACE, |x, y, r, _| {
        let eye = (5..8).contains(&y) && ((3..6).contains(&x) || (10..13).contains(&x));
        if eye { rgb(10, 8, 8) } else { shade(rgb(50, 50, 54), r.range(0.88, 1.06)) }
    });
    // The Wilter: ashen bones, three grim faces.
    a.each(T_WILTER, |x, y, r, _| shade(if (x + y * 2) % 5 == 0 { rgb(25, 25, 28) } else { rgb(42, 42, 46) }, r.range(0.85, 1.06)));
    a.each(T_WILTER_FACE, |x, y, r, _| {
        let eye = (5..8).contains(&y) && ((3..6).contains(&x) || (10..13).contains(&x));
        let mouth = (10..12).contains(&y) && (4..12).contains(&x);
        if eye { rgb(230, 220, 255) } else if mouth { rgb(12, 10, 12) } else { shade(rgb(42, 42, 46), r.range(0.85, 1.06)) }
    });
}

/// v0.2's Ocean Monument: prismarine, its lights and treasures, and Guardians.
fn monument_tiles(a: &mut Atlas) {
    // Prismarine: mottled teal-green that shifts between blue and green.
    a.each(T_PRISMARINE, |x, y, r, p| {
        let n = p.noise2(x as f32 / 5.0 + 40.0, y as f32 / 5.0);
        let c = if n > 0.1 { rgb(90, 160, 150) } else if n < -0.15 { rgb(70, 130, 150) } else { rgb(100, 170, 140) };
        shade(c, r.range(0.88, 1.08))
    });
    a.each(T_PRISMARINE_BRICKS, |x, y, r, _| {
        let row = y / 4;
        let mortar = y % 4 == 3 || (x + row * 4) % 8 == 0;
        if mortar { rgb(60, 110, 100) } else { shade(rgb(100, 175, 160), r.range(0.9, 1.06)) }
    });
    a.each(T_DARK_PRISMARINE, |x, y, r, _| {
        let edge = x == 0 || y == 0 || x == 15 || y == 15 || ((x == 4 || x == 11) && (4..12).contains(&y)) || ((y == 4 || y == 11) && (4..12).contains(&x));
        if edge { rgb(30, 60, 50) } else { shade(rgb(50, 95, 80), r.range(0.9, 1.06)) }
    });
    // A sea lantern: pale glowing panes round a bright core.
    a.each(T_SEA_LANTERN, |x, y, r, _| {
        let (dx, dy) = ((x as i32 - 7).abs().min((x as i32 - 8).abs()), (y as i32 - 7).abs().min((y as i32 - 8).abs()));
        let core = dx < 3 && dy < 3;
        let frame = x == 0 || y == 0 || x == 15 || y == 15;
        if frame { rgb(170, 200, 190) } else if core { rgb(250, 255, 250) } else { shade(rgb(205, 230, 225), r.range(0.92, 1.05)) }
    });
    // The conduit: a wooden-ish cage round a blue eye.
    a.each(T_CONDUIT, |x, y, r, _| {
        let (dx, dy) = (x as f32 - 7.5, y as f32 - 7.5);
        let d = (dx * dx + dy * dy).sqrt();
        if d < 3.0 { rgb(70, 160, 230) } else if d < 4.0 { rgb(20, 40, 70) } else if (x + y) % 4 < 2 { shade(rgb(160, 120, 70), r.range(0.9, 1.05)) } else { shade(rgb(120, 90, 55), r.range(0.9, 1.05)) }
    });
    a.each(T_PRISMARINE_SHARD, |x, y, r, _| {
        let d = (x as i32 - y as i32).abs();
        if d <= 2 && (3..14).contains(&x) { shade(rgb(110, 180, 165), r.range(0.85, 1.08)) } else { [0, 0, 0, 0] }
    });
    a.each(T_PRISMARINE_CRYSTALS, |x, y, r, _| {
        let blob = [(5, 6), (10, 5), (8, 10), (4, 11), (11, 11)].iter().any(|&(cx, cy)| (x as i32 - cx).abs() + (y as i32 - cy).abs() < 3);
        if blob { shade(if (x + y) % 3 == 0 { rgb(240, 255, 240) } else { rgb(170, 230, 210) }, r.range(0.9, 1.05)) } else { [0, 0, 0, 0] }
    });
    a.each(T_NAUTILUS_SHELL, |x, y, r, _| {
        let (dx, dy) = (x as f32 - 8.0, y as f32 - 8.5);
        let d = (dx * dx + dy * dy).sqrt();
        let a = dy.atan2(dx);
        let stripe = ((d * 1.2 - a * 1.5).rem_euclid(3.0)) < 1.0;
        if d < 6.5 { shade(if stripe { rgb(170, 90, 60) } else { rgb(240, 225, 205) }, r.range(0.9, 1.05)) } else { [0, 0, 0, 0] }
    });
    a.each(T_HEART_OF_THE_SEA, |x, y, r, _| {
        let (dx, dy) = (x as f32 - 7.5, y as f32 - 7.5);
        let d = dx.abs() + dy.abs();
        if d < 4.0 { rgb(120, 220, 255) } else if d < 6.5 { shade(rgb(40, 90, 170), r.range(0.9, 1.06)) } else if d < 7.5 { rgb(20, 40, 90) } else { [0, 0, 0, 0] }
    });
    // Guardians: teal-orange scales, a single big eye, and spikes.
    for (t, base, belly) in [(T_GUARDIAN, rgb(90, 150, 140), rgb(210, 130, 80)), (T_ELDER_GUARDIAN, rgb(200, 195, 175), rgb(160, 140, 170))] {
        a.each(t, move |x, y, r, _| {
            let scale = (x / 3 + y / 3) % 2 == 0;
            shade(if y > 11 { belly } else { base }, if scale { 1.0 } else { 0.86 } * r.range(0.92, 1.06))
        });
    }
    for (t, base, iris) in [(T_GUARDIAN_EYE, rgb(90, 150, 140), rgb(230, 120, 40)), (T_ELDER_EYE, rgb(200, 195, 175), rgb(150, 60, 160))] {
        a.each(t, move |x, y, r, _| {
            let (dx, dy) = (x as f32 - 7.5, y as f32 - 7.5);
            let d = (dx * dx + dy * dy).sqrt();
            if d < 2.0 { rgb(20, 20, 20) } else if d < 4.0 { iris } else if d < 5.5 { rgb(240, 240, 230) } else { shade(base, r.range(0.9, 1.05)) }
        });
    }
    a.each(T_GUARDIAN_SPIKE, |_, y, r, _| shade(if y < 6 { rgb(240, 225, 200) } else { rgb(200, 120, 70) }, r.range(0.9, 1.05)));
    a.each(T_GUARDIAN_LASER, |_, y, _, _| if (5..11).contains(&y) { [255, 210, 120, 255] } else { [200, 80, 40, 200] });
}

/// v0.2's temples, mineshafts and igloos.
fn temple_tiles(a: &mut Atlas) {
    // A web: spokes from the middle and rings round it.
    a.each(T_COBWEB, |x, y, _, _| {
        let (dx, dy) = (x as f32 - 7.5, y as f32 - 7.5);
        let d = (dx * dx + dy * dy).sqrt();
        let spoke = dx.abs() < 0.6 || dy.abs() < 0.6 || (dx.abs() - dy.abs()).abs() < 0.7;
        let ring = (d % 3.2) < 0.6 && d > 1.5;
        if (spoke || ring) && d < 8.0 { [235, 235, 240, 220] } else { [0, 0, 0, 0] }
    });
    // Tripwire: a thin taut string along the block (north-south, then east-west).
    a.each(T_TRIPWIRE, |x, _, _, _| if x == 7 { [220, 220, 215, 255] } else { [0, 0, 0, 0] });
    a.each(T_TRIPWIRE_EW, |_, y, _, _| if y == 7 { [220, 220, 215, 255] } else { [0, 0, 0, 0] });
    // A hook: a wooden plank on the wall with a metal ring (red when tripped).
    for (t, on) in [(T_TRIPWIRE_HOOK, false), (T_TRIPWIRE_HOOK_ON, true)] {
        a.each(t, move |x, y, r, _| {
            let plank = (6..10).contains(&x) && (2..14).contains(&y);
            let (dx, dy) = (x as f32 - 7.5, y as f32 - 9.0);
            let ring = ((dx * dx + dy * dy).sqrt() - 2.5).abs() < 0.8;
            if ring {
                if on { rgb(220, 60, 50) } else { rgb(170, 170, 175) }
            } else if plank {
                shade(rgb(150, 110, 65), r.range(0.88, 1.05))
            } else {
                [0, 0, 0, 0]
            }
        });
    }
    // Chiseled sandstone: a carved face in a frame.
    a.each(T_CHISELED_SANDSTONE, |x, y, r, _| {
        let frame = x == 0 || x == 15 || y == 1 || y == 14;
        let eye = (y == 6 || y == 7) && (x == 5 || x == 10);
        let mouth = y == 10 && (5..11).contains(&x);
        let base = shade(rgb(220, 205, 150), r.range(0.92, 1.04));
        if frame || eye || mouth { shade(base, 0.75) } else { base }
    });
}

/// v0.2's cave biomes: dripstone, lush caves and amethyst geodes.
fn cave_tiles(a: &mut Atlas) {
    // Dripstone: tan stone in wavy vertical streaks, where water ran down it.
    a.each(T_DRIPSTONE, |x, y, r, p| {
        let streak = (p.noise2(x as f32 / 3.0, y as f32 / 14.0) * 3.0 + x as f32 * 1.3).sin();
        let line = if streak > 0.8 { 0.86 } else if streak < -0.7 { 1.08 } else { 1.0 };
        let blotch = 1.0 + p.noise2(x as f32 / 4.0 + 30.0, y as f32 / 4.0) * 0.12;
        shade(rgb(150, 116, 92), line * blotch * r.range(0.9, 1.08))
    });
    a.each(T_MOSS, |x, y, r, p| {
        let clump = p.noise2(x as f32 / 4.0 + 7.0, y as f32 / 4.0);
        shade(rgb(90, 125, 45), (1.0 + clump * 0.25) * r.range(0.82, 1.12))
    });
    // Cave vines: a stem hanging down with leaves either side; the lit ones carry berries.
    for (t, berries) in [(T_CAVE_VINES, false), (T_CAVE_VINES_LIT, true)] {
        a.each(t, move |x, y, r, _| {
            let stem = (7..9).contains(&x);
            let leaf = (y % 5 == 1 && (4..7).contains(&x)) || (y % 5 == 3 && (9..12).contains(&x));
            let berry = berries && ((y % 6 == 4 && (4..7).contains(&x) && y > 3) || (y % 6 == 1 && (9..12).contains(&x) && y > 6));
            if berry {
                if (x + y) % 3 == 0 { rgb(255, 245, 170) } else { rgb(255, 175, 40) }
            } else if stem || leaf {
                shade(rgb(80, 120, 40), r.range(0.85, 1.12))
            } else {
                [0, 0, 0, 0]
            }
        });
    }
    // Azalea: a round green bush on a little trunk, with pink flowers.
    a.each(T_AZALEA, |x, y, r, _| {
        let (dx, dy) = (x as f32 - 7.5, y as f32 - 7.0);
        let bush = dx * dx / 42.0 + dy * dy / 30.0 < 1.0 && y < 12;
        let trunk = (7..9).contains(&x) && y >= 11;
        if bush {
            if (x * 5 + y * 3) % 11 == 0 { rgb(230, 120, 200) } else { shade(rgb(95, 135, 50), r.range(0.8, 1.12)) }
        } else if trunk {
            rgb(110, 85, 60)
        } else {
            [0, 0, 0, 0]
        }
    });
    a.each(T_CALCITE, |_, _, r, _| shade(rgb(222, 222, 215), r.range(0.92, 1.05)));
    a.each(T_SMOOTH_BASALT, |x, y, r, p| shade(rgb(72, 72, 78), (1.0 + p.noise2(x as f32 / 5.0, y as f32 / 5.0) * 0.12) * r.range(0.94, 1.04)));
    // Amethyst: purple facets.
    let facet = |x: usize, y: usize, r: &mut Rng| {
        // Crystal faces: diagonal bands crossing, lighter where they meet.
        let a = (x + y) / 3 % 3;
        let b = (x + 16 - y) / 4 % 3;
        shade(rgb(140, 95, 200), [0.78, 0.95, 1.12][a] * [0.92, 1.0, 1.1][b] * r.range(0.95, 1.04))
    };
    a.each(T_AMETHYST, |x, y, r, _| facet(x, y, r));
    a.each(T_BUDDING_AMETHYST, move |x, y, r, _| {
        // Budding: darker, with little bright crosses where buds start.
        let spot = (x % 6 == 2 && (1..4).contains(&(y % 6))) || (y % 6 == 2 && (1..4).contains(&(x % 6)));
        if spot { rgb(230, 190, 255) } else { shade(facet(x, y, r), 0.82) }
    });
    // Buds and the cluster: crystals pointing up (flipped when they hang).
    for (t, tall, wide) in [(T_AMETHYST_BUD_SMALL, 5, 2), (T_AMETHYST_BUD_LARGE, 9, 3), (T_AMETHYST_CLUSTER, 13, 4)] {
        a.each(t, move |x, y, r, _| {
            let h = 15 - y as i32;
            let crystals = [(7i32, tall), (7 - wide, tall * 2 / 3), (8 + wide, tall * 3 / 4)];
            for (cx, ch) in crystals {
                let half = 1 + (ch - h).max(0) / 4;
                if h < ch && (x as i32 - cx).abs() <= half.min(2) {
                    let tip = h > ch - 3;
                    return shade(if tip { rgb(235, 200, 255) } else { rgb(165, 110, 225) }, r.range(0.9, 1.08));
                }
            }
            [0, 0, 0, 0]
        });
    }
    // Tinted glass: dark smoky panes with a pale rim.
    a.each(T_TINTED_GLASS, |x, y, _, _| {
        let edge = x == 0 || y == 0 || x == 15 || y == 15;
        if edge { [70, 55, 85, 255] } else if (x + y) % 7 == 0 { [90, 80, 105, 200] } else { [40, 32, 50, 190] }
    });
    a.each(T_GLOW_BERRIES, |x, y, _, _| {
        let berry = |cx: f32, cy: f32| (x as f32 - cx).powi(2) + (y as f32 - cy).powi(2) < 9.0;
        let stem = x == 8 && y < 5;
        if berry(5.5, 9.5) || berry(10.5, 10.5) || berry(8.0, 6.5) {
            if (x + y) % 4 == 0 { rgb(255, 250, 190) } else { rgb(255, 170, 40) }
        } else if stem {
            rgb(80, 120, 40)
        } else {
            [0, 0, 0, 0]
        }
    });
    a.each(T_AMETHYST_SHARD, |x, y, r, _| {
        // A long crystal, corner to corner.
        let d = (x as i32 - (15 - y as i32)).abs();
        if d <= 2 && (2..14).contains(&y) {
            shade(if d == 0 { rgb(235, 200, 255) } else { rgb(160, 105, 220) }, r.range(0.9, 1.06))
        } else {
            [0, 0, 0, 0]
        }
    });
}

/// v0.1.21's building blocks.
fn masonry_tiles(a: &mut Atlas) {
    for c in 0..8u16 {
        let [r0, g0, b0] = crate::carpentry::colour_rgb(c as usize);
        let base = rgb(r0, g0, b0);
        a.each(T_CONCRETE + c, |_, _, r, _| shade(base, r.range(0.95, 1.02)));
        a.each(T_CONCRETE_POWDER + c, |_, _, r, _| shade(base, r.range(0.78, 1.12)));
        // Glazed terracotta: a swirl in the colour round a pale middle.
        let pale = shade(base, 1.35);
        let dark = shade(base, 0.6);
        a.each(T_GLAZED + c, move |x, y, r, _| {
            let (dx, dy) = (x as f32 - 7.5, y as f32 - 7.5);
            let ring = ((dx * dx + dy * dy).sqrt() + dx.atan2(dy) * 2.0) as i32 % 4;
            let c = match ring {
                0 => dark,
                1 | 2 => base,
                _ => pale,
            };
            shade(c, r.range(0.93, 1.04))
        });
    }
    a.each(T_CANDLE, |x, y, r, _| {
        let wax = (6..10).contains(&x) && y >= 6;
        let wick = x == 8 && (4..6).contains(&y);
        if wax { shade(rgb(235, 225, 190), r.range(0.9, 1.03)) } else if wick { rgb(40, 35, 30) } else { [0, 0, 0, 0] }
    });
    a.each(T_CANDLE_LIT, |x, y, r, _| {
        let wax = (6..10).contains(&x) && y >= 6;
        let flame = (7..10).contains(&x) && (1..6).contains(&y);
        if flame { if y < 3 { rgb(255, 240, 150) } else { rgb(255, 170, 50) } } else if wax { shade(rgb(235, 225, 190), r.range(0.9, 1.03)) } else { [0, 0, 0, 0] }
    });
    a.each(T_CHAIN, |x, y, r, _| {
        let link = (6..10).contains(&x) && (y % 6 != 0 || x == 6 || x == 9) && !((7..9).contains(&x) && y % 6 > 1 && y % 6 < 5);
        if link { shade(rgb(70, 75, 85), r.range(0.85, 1.1)) } else { [0, 0, 0, 0] }
    });
    a.each(T_SCAFFOLD_TOP, |x, y, r, _| {
        let rim = x < 2 || y < 2 || x > 13 || y > 13;
        let slat = x % 4 == 0;
        if rim { shade(rgb(180, 150, 70), r.range(0.9, 1.05)) } else if slat { shade(rgb(150, 120, 55), r.range(0.9, 1.05)) } else { shade(rgb(205, 175, 90), r.range(0.9, 1.05)) }
    });
    a.each(T_SCAFFOLD_SIDE, |x, y, r, _| {
        let frame = !(2..=13).contains(&x) || y < 2;
        let brace = (x as i32 - y as i32).abs() < 2;
        if frame || brace { shade(rgb(185, 155, 75), r.range(0.9, 1.05)) } else { [0, 0, 0, 0] }
    });
}

/// v0.1.20's creatures: turtles, dolphins, pandas, polar bears, llamas, and two new Hmmers.
fn mob_tiles(a: &mut Atlas) {
    let eyes = |x: usize, y: usize| (5..7).contains(&y) && (x == 4 || x == 11);
    a.each(T_TURTLE, |x, y, r, _| shade(if (x * 3 + y) % 7 == 0 { rgb(150, 170, 120) } else { rgb(120, 160, 95) }, r.range(0.88, 1.05)));
    a.each(T_TURTLE_FACE, |x, y, r, _| if eyes(x, y) { rgb(20, 20, 20) } else { shade(rgb(120, 160, 95), r.range(0.88, 1.05)) });
    a.each(T_TURTLE_SHELL_TOP, |x, y, r, _| {
        let seam = x % 5 == 0 || y % 5 == 0;
        shade(if seam { rgb(55, 85, 40) } else { rgb(70, 120, 55) }, r.range(0.85, 1.05))
    });
    a.each(T_DOLPHIN, |_, y, r, _| shade(if y > 10 { rgb(200, 205, 215) } else { rgb(110, 125, 145) }, r.range(0.92, 1.04)));
    a.each(T_DOLPHIN_FACE, |x, y, r, _| {
        let smile = y == 11 && (4..12).contains(&x);
        if eyes(x, y) { rgb(15, 15, 25) } else if smile { rgb(70, 80, 95) } else { shade(if y > 10 { rgb(200, 205, 215) } else { rgb(110, 125, 145) }, r.range(0.92, 1.04)) }
    });
    a.each(T_PANDA, |_, _, r, _| shade(rgb(235, 235, 230), r.range(0.92, 1.04)));
    a.each(T_PANDA_BLACK, |_, _, r, _| shade(rgb(40, 40, 42), r.range(0.9, 1.06)));
    a.each(T_PANDA_FACE, |x, y, r, _| {
        let patch = (3..7).contains(&y) && ((2..7).contains(&x) || (9..14).contains(&x));
        let nose = (9..11).contains(&y) && (7..9).contains(&x);
        if eyes(x, y) { rgb(240, 240, 240) } else if patch || nose { rgb(35, 35, 38) } else { shade(rgb(235, 235, 230), r.range(0.92, 1.04)) }
    });
    a.each(T_POLAR, |_, _, r, _| shade(rgb(240, 238, 228), r.range(0.9, 1.04)));
    a.each(T_POLAR_FACE, |x, y, r, _| {
        let nose = (8..11).contains(&y) && (6..10).contains(&x);
        if eyes(x, y) || nose { rgb(25, 25, 25) } else { shade(rgb(240, 238, 228), r.range(0.9, 1.04)) }
    });
    a.each(T_LLAMA, |x, y, r, _| shade(if (x * 5 + y * 3) % 9 == 0 { rgb(200, 180, 140) } else { rgb(225, 210, 175) }, r.range(0.88, 1.05)));
    a.each(T_LLAMA_FACE, |x, y, r, _| {
        let mouth = y == 12 && (6..10).contains(&x);
        if eyes(x, y) || mouth { rgb(40, 30, 25) } else { shade(rgb(225, 210, 175), r.range(0.88, 1.05)) }
    });
    a.each(T_ZHMM_ROBE, |x, y, r, _| shade(if (x + y) % 6 == 0 { rgb(70, 90, 60) } else { rgb(95, 75, 60) }, r.range(0.8, 1.05)));
    a.each(T_ZHMM_FACE, |x, y, r, _| {
        let nose = (6..11).contains(&y) && (7..9).contains(&x);
        if eyes(x, y) { rgb(160, 30, 20) } else if nose { rgb(80, 120, 70) } else { shade(rgb(100, 150, 90), r.range(0.85, 1.05)) }
    });
    a.each(T_WANDERER_ROBE, |x, y, r, _| shade(if y % 5 == 0 || x % 7 == 0 { rgb(220, 190, 60) } else { rgb(50, 80, 160) }, r.range(0.88, 1.05)));
}

/// Mipmap levels 1.. for the atlas, built tile by tile (down to one texel a
/// tile) so no tile ever picks up its neighbours. Colour is averaged from the
/// visible texels only, so see-through pixels never darken edges. Cut-out tiles
/// (grass, flowers, saplings: every texel fully clear or fully solid) stay
/// clear-or-solid at every level and keep the share of solid texels they
/// start with, so far-off plants neither swell into dark blobs nor vanish.
pub fn mip_levels(atlas: &[u8]) -> Vec<Vec<u8>> {
    let tiles = ATLAS / TILE;
    let mut cutout = vec![false; tiles * tiles];
    let mut coverage = vec![1.0f32; tiles * tiles];
    for ty in 0..tiles {
        for tx in 0..tiles {
            let (mut binary, mut solid) = (true, 0usize);
            for y in 0..TILE {
                for x in 0..TILE {
                    let a = atlas[((ty * TILE + y) * ATLAS + tx * TILE + x) * 4 + 3];
                    binary &= a == 0 || a == 255;
                    solid += (a == 255) as usize;
                }
            }
            cutout[ty * tiles + tx] = binary && solid < TILE * TILE;
            coverage[ty * tiles + tx] = solid as f32 / (TILE * TILE) as f32;
        }
    }
    let mut levels: Vec<Vec<u8>> = Vec::new();
    let mut size = ATLAS;
    let mut tile = TILE;
    while tile > 1 {
        let prev: &[u8] = levels.last().map(|v| v.as_slice()).unwrap_or(atlas);
        let (half, th) = (size / 2, tile / 2);
        let mut out = vec![0u8; half * half * 4];
        let mut alpha = vec![0f32; half * half];
        for y in 0..half {
            for x in 0..half {
                let (mut rgb, mut wsum, mut asum, mut plain) = ([0f32; 3], 0f32, 0f32, [0f32; 3]);
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let i = ((y * 2 + dy) * size + x * 2 + dx) * 4;
                    let a = prev[i + 3] as f32 / 255.0;
                    for k in 0..3 {
                        rgb[k] += prev[i + k] as f32 * a;
                        plain[k] += prev[i + k] as f32 / 4.0;
                    }
                    wsum += a;
                    asum += a / 4.0;
                }
                let o = (y * half + x) * 4;
                for k in 0..3 {
                    out[o + k] = if wsum > 0.0 { (rgb[k] / wsum).round() as u8 } else { plain[k].round() as u8 };
                }
                alpha[y * half + x] = asum;
                out[o + 3] = (asum * 255.0).round() as u8;
            }
        }
        for ty in 0..tiles {
            for tx in 0..tiles {
                if !cutout[ty * tiles + tx] {
                    continue;
                }
                // Keep the most-covered texels solid, as many as the full-size tile had.
                let mut cells: Vec<(f32, usize)> = (0..th).flat_map(|y| (0..th).map(move |x| (ty * th + y) * half + tx * th + x)).map(|i| (alpha[i], i)).collect();
                cells.sort_by(|a, b| b.0.total_cmp(&a.0));
                let keep = ((coverage[ty * tiles + tx] * cells.len() as f32).round() as usize).max(1);
                for (n, (a, i)) in cells.into_iter().enumerate() {
                    out[i * 4 + 3] = if n < keep && a > 0.0 { 255 } else { 0 };
                }
            }
        }
        levels.push(out);
        size = half;
        tile = th;
    }
    levels
}

/// The game's icon: a grass block seen from above a corner, `size` pixels
/// square (RGBA), drawn from the atlas and smoothed at the edges.
pub fn icon_rgba(atlas: &[u8], size: usize) -> Vec<u8> {
    let texel = |tile: u16, u: f32, v: f32| -> [f32; 4] {
        let (tx, ty) = ((tile % TILES_PER_ROW) as usize * TILE, (tile / TILES_PER_ROW) as usize * TILE);
        let (x, y) = (((u * TILE as f32) as usize).min(TILE - 1), ((v * TILE as f32) as usize).min(TILE - 1));
        let i = ((ty + y) * ATLAS + tx + x) * 4;
        [atlas[i] as f32, atlas[i + 1] as f32, atlas[i + 2] as f32, atlas[i + 3] as f32]
    };
    // Corners of the outline (in 0..1 across the icon), with the middle where the three faces meet.
    let (top, ul, ur, mid, ll, bottom) = ((0.5, 0.02), (0.06, 0.26), (0.94, 0.26), (0.5, 0.5), (0.06, 0.76), (0.5, 0.98));
    // Where (px, py) falls on the face spanned from `o` by `a` and `b`, as 0..1 coordinates.
    let on = |p: (f32, f32), o: (f32, f32), a: (f32, f32), b: (f32, f32)| -> Option<(f32, f32)> {
        let (dx, dy) = (p.0 - o.0, p.1 - o.1);
        let det = a.0 * b.1 - a.1 * b.0;
        let s = (dx * b.1 - dy * b.0) / det;
        let t = (a.0 * dy - a.1 * dx) / det;
        ((0.0..1.0).contains(&s) && (0.0..1.0).contains(&t)).then_some((s, t))
    };
    let sub = |a: (f32, f32), b: (f32, f32)| (a.0 - b.0, a.1 - b.1);
    let sample = |p: (f32, f32)| -> [f32; 4] {
        let shade = |c: [f32; 4], k: f32| [c[0] * k, c[1] * k, c[2] * k, c[3]];
        if let Some((s, t)) = on(p, ul, sub(top, ul), sub(mid, ul)) {
            return texel(T_GRASS_TOP, s, t);
        }
        if let Some((s, t)) = on(p, ul, sub(mid, ul), sub(ll, ul)) {
            return shade(texel(T_GRASS_SIDE, s, t), 0.85);
        }
        if let Some((s, t)) = on(p, mid, sub(ur, mid), sub(bottom, mid)) {
            return shade(texel(T_GRASS_SIDE, s, t), 0.65);
        }
        [0.0; 4]
    };
    let mut out = vec![0u8; size * size * 4];
    const N: usize = 4;
    for y in 0..size {
        for x in 0..size {
            let mut acc = [0f32; 4];
            for sy in 0..N {
                for sx in 0..N {
                    let p = ((x as f32 + (sx as f32 + 0.5) / N as f32) / size as f32, (y as f32 + (sy as f32 + 0.5) / N as f32) / size as f32);
                    let c = sample(p);
                    let a = c[3] / 255.0;
                    for k in 0..3 {
                        acc[k] += c[k] * a;
                    }
                    acc[3] += c[3];
                }
            }
            let n = (N * N) as f32;
            let a = acc[3] / n;
            let i = (y * size + x) * 4;
            for k in 0..3 {
                out[i + k] = if a > 0.0 { (acc[k] / n * 255.0 / a).round().clamp(0.0, 255.0) as u8 } else { 0 };
            }
            out[i + 3] = a.round() as u8;
        }
    }
    out
}

/// A Windows .ico holding the icon at the usual sizes (PNG-compressed entries
/// aren't needed: plain 32-bit bitmaps work everywhere).
pub fn icon_ico(atlas: &[u8]) -> Vec<u8> {
    let sizes = [16usize, 32, 48, 64, 128, 256];
    let images: Vec<Vec<u8>> = sizes
        .iter()
        .map(|&n| {
            let px = icon_rgba(atlas, n);
            let mut b = Vec::new();
            // BITMAPINFOHEADER: height counts the colour rows and the (empty) mask rows.
            for v in [40u32, n as u32, (n * 2) as u32] {
                b.extend(v.to_le_bytes());
            }
            b.extend(1u16.to_le_bytes());
            b.extend(32u16.to_le_bytes());
            for _ in 0..6 {
                b.extend(0u32.to_le_bytes());
            }
            // Rows bottom-up, BGRA.
            for y in (0..n).rev() {
                for x in 0..n {
                    let i = (y * n + x) * 4;
                    b.extend([px[i + 2], px[i + 1], px[i], px[i + 3]]);
                }
            }
            // The AND mask: all zero (alpha does the work), rows padded to 4 bytes.
            b.extend(vec![0u8; n.div_ceil(32) * 4 * n]);
            b
        })
        .collect();
    let mut ico = Vec::new();
    ico.extend(0u16.to_le_bytes());
    ico.extend(1u16.to_le_bytes());
    ico.extend((sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len();
    for (n, img) in sizes.iter().zip(&images) {
        let dim = if *n >= 256 { 0 } else { *n as u8 };
        ico.extend([dim, dim, 0, 0]);
        ico.extend(1u16.to_le_bytes());
        ico.extend(32u16.to_le_bytes());
        ico.extend((img.len() as u32).to_le_bytes());
        ico.extend((offset as u32).to_le_bytes());
        offset += img.len();
    }
    for img in images {
        ico.extend(img);
    }
    ico
}

/// Each atlas texel's alpha, kept on the CPU for shapes built from sprites
/// (held items get one pixel of thickness along their outline).
static ALPHA: std::sync::RwLock<Vec<u8>> = std::sync::RwLock::new(Vec::new());

/// Remember the atlas's alpha (whenever it's uploaded).
pub fn remember_alpha(atlas: &[u8]) {
    if let Ok(mut a) = ALPHA.write() {
        *a = atlas.chunks_exact(4).map(|p| p[3]).collect();
    }
}

/// Whether texel (x, y) of a tile is solid (everything is, before an atlas is known).
pub fn solid(tile: u16, x: usize, y: usize) -> bool {
    let Ok(a) = ALPHA.read() else { return true };
    if a.is_empty() {
        return true;
    }
    let (tx, ty) = ((tile % TILES_PER_ROW) as usize * TILE, (tile / TILES_PER_ROW) as usize * TILE);
    a.get((ty + y) * ATLAS + tx + x).is_none_or(|&v| v >= 128)
}

/// Each tile's cell in the UI atlas: the tile plus a one-pixel border that
/// repeats its edge. Menus and the HUD draw from this copy, so a GPU that
/// samples a hair outside a tile (multisampling does, at the edges of shapes)
/// still gets that tile's own colour, not a line of its neighbour.
pub const UI_CELL: usize = TILE + 2;
pub const UI_ATLAS: usize = TILES_PER_ROW as usize * UI_CELL;

/// The atlas re-laid-out for the UI (see `UI_CELL`).
pub fn ui_atlas(atlas: &[u8]) -> Vec<u8> {
    let tiles = TILES_PER_ROW as usize;
    let mut out = vec![0u8; UI_ATLAS * UI_ATLAS * 4];
    for ty in 0..tiles {
        for tx in 0..tiles {
            for y in 0..UI_CELL {
                for x in 0..UI_CELL {
                    // Border pixels copy the nearest edge pixel of the tile.
                    let sx = (x as i32 - 1).clamp(0, TILE as i32 - 1) as usize;
                    let sy = (y as i32 - 1).clamp(0, TILE as i32 - 1) as usize;
                    let s = ((ty * TILE + sy) * ATLAS + tx * TILE + sx) * 4;
                    let d = ((ty * UI_CELL + y) * UI_ATLAS + tx * UI_CELL + x) * 4;
                    out[d..d + 4].copy_from_slice(&atlas[s..s + 4]);
                }
            }
        }
    }
    out
}

/// Where a tile sits in the UI atlas, as (u, v, size) in 0..1 coordinates.
pub fn ui_tile_uv(tile: u16) -> (f32, f32, f32) {
    let (tx, ty) = ((tile % TILES_PER_ROW) as usize, (tile / TILES_PER_ROW) as usize);
    let n = UI_ATLAS as f32;
    ((tx * UI_CELL + 1) as f32 / n, (ty * UI_CELL + 1) as f32 / n, TILE as f32 / n)
}

const GLIDER_SPRITE: [&str; 16] = [
    "................",
    "................",
    "......####......",
    ".....#bBBb#.....",
    "....#bBBBBb#....",
    "...#bBBmmBBb#...",
    "..#bBBm##mBBb#..",
    "..#bBm#..#mBb#..",
    ".#bBm#....#mBb#.",
    ".#bBm#....#mBb#.",
    "#bBm#......#mBb#",
    "#bBm#......#mBb#",
    "#bm#........#mb#",
    "#m#..........#m#",
    "##............##",
    "................",
];

const ROCKET_SPRITE: [&str; 16] = [
    "................",
    "..........#.....",
    ".........#w#....",
    "........#rrw#...",
    ".......#rrrr#...",
    "......#rwrr#....",
    ".....#rrrr#.....",
    "....#rrwr#......",
    "...#rrrr#.......",
    "....#rr#........",
    "...#o##.........",
    "..#o#...........",
    ".#o#............",
    ".y#.............",
    "y.y.............",
    "................",
];

const SPEAR_SPRITE: [&str; 16] = [
    "...........###..",
    "..........#hhh#.",
    "...........#hh#.",
    "..........#h#h#.",
    ".........#t#.#..",
    "........#t#.....",
    ".......#t#......",
    "......#t#.......",
    ".....#t#........",
    "....#t#.........",
    "...#t#..........",
    "..#t#...........",
    ".#t#............",
    ".##.............",
    "................",
    "................",
];

const INGOT_SPRITE: [&str; 16] = [
    "................",
    "................",
    "................",
    "................",
    "......######....",
    ".....#hhhhhh#...",
    "....#hbbbbbb#...",
    "...#hbbbbbb#d...",
    "..#hbbbbbb#dd...",
    "..############..",
    "..#bbbbbbbbb#d..",
    "..#dddddddddd#..",
    "...##########...",
    "................",
    "................",
    "................",
];

const AXE: [&str; 16] = [
    "................",
    "......hhh.......",
    ".....hHHHh##....",
    "....hHHHHh#o#...",
    "....hHHHHho#....",
    ".....hHHho#.....",
    "......hh#o#.....",
    ".......#o#......",
    "......#o#.......",
    ".....#o#........",
    "....#o#.........",
    "...#o#..........",
    "..#o#...........",
    ".#o#............",
    ".##.............",
    "................",
];

const SHOVEL: [&str; 16] = [
    "................",
    "..........hhh...",
    ".........hHHHh..",
    "........hHHHHh..",
    "........hHHHh...",
    ".........hHh....",
    "........#o#.....",
    ".......#o#......",
    "......#o#.......",
    ".....#o#........",
    "....#o#.........",
    "...#o#..........",
    "..#o#...........",
    "..##............",
    "................",
    "................",
];

/// Everything added for gliders, boxes, copper, bamboo and the sea.
fn paint_new_things(a: &mut Atlas) {
    // Axes and shovels in five tiers, copper tools and armour.
    let wood = (rgb(170, 135, 82), rgb(110, 85, 50));
    let copper = (rgb(230, 140, 90), rgb(160, 80, 45));
    let tiers = [wood, (rgb(150, 150, 150), rgb(90, 90, 90)), copper, (rgb(235, 235, 235), rgb(160, 160, 160)), (rgb(90, 240, 225), rgb(30, 160, 150))];
    for (i, (light, dark)) in tiers.iter().enumerate() {
        let pal = [('#', rgb(73, 54, 21)), ('o', rgb(137, 103, 39)), ('H', *light), ('h', *dark), ('g', rgb(73, 54, 21))];
        a.sprite(T_AXE0 + i as u16, &AXE, &pal);
        a.sprite(T_SHOVEL0 + i as u16, &SHOVEL, &pal);
        if i == 2 {
            a.sprite(T_PICK_COPPER, &PICK, &pal);
            a.sprite(T_SWORD_COPPER, &SWORD, &pal);
        }
    }
    let base = rgb(215, 125, 80);
    let pal = [('#', shade(base, 0.3)), ('b', base), ('d', shade(base, 0.72)), ('h', shade(base, 1.2))];
    for (slot, rows) in [&HELMET, &CHESTPLATE, &LEGGINGS, &BOOTS].into_iter().enumerate() {
        a.sprite(T_COPPER_ARMOR_ITEMS + slot as u16, rows, &pal);
    }
    a.each(T_COPPER_ARMOR_WORN, |x, y, r, _| {
        let rim = x == 0 || y == 0 || x == 15 || y == 15;
        let green = (x * 7 + y * 3) % 13 == 0;
        shade(if green { rgb(110, 160, 120) } else { base }, r.range(0.9, 1.05) * if rim { 0.7 } else { 1.0 })
    });
    // The Hollow Box: pale purple shell with a darker seam around its lid.
    a.each(T_HOLLOW_BOX_SIDE, |x, y, r, _| {
        let seam = y == 5 || y == 6;
        let rim = x == 0 || x == 15 || y == 0 || y == 15;
        let base = if seam { rgb(70, 45, 90) } else if rim { rgb(120, 90, 145) } else { rgb(165, 125, 190) };
        shade(base, r.range(0.92, 1.06))
    });
    a.each(T_HOLLOW_BOX_TOP, |x, y, r, _| {
        let rim = x == 0 || x == 15 || y == 0 || y == 15;
        let d = (x as f32 - 7.5).abs().max((y as f32 - 7.5).abs());
        let base = if rim { rgb(120, 90, 145) } else if d < 2.5 { rgb(215, 190, 235) } else { rgb(165, 125, 190) };
        shade(base, r.range(0.92, 1.06))
    });
    a.ore(T_COPPER_ORE, rgb(215, 125, 80), rgb(80, 150, 120));
    // Copper: warm orange, dulling to brown, then streaked and finally all verdigris.
    let stages = [[200, 110, 75], [165, 120, 95], [110, 150, 115], [85, 170, 140]];
    for (i, c) in stages.into_iter().enumerate() {
        let tile = T_COPPER + i as u16;
        a.each(tile, move |x, y, r, p| {
            let plate = x % 8 == 0 || y % 8 == 0;
            let n = p.noise3(x as f32 * 0.35, y as f32 * 0.35, i as f32 * 3.0);
            let mut col = rgb(c[0], c[1], c[2]);
            if (i == 1 || i == 2) && n > 0.15 {
                col = if i == 1 { rgb(110, 150, 115) } else { rgb(85, 170, 140) };
            }
            shade(col, r.range(0.9, 1.07) * if plate { 0.8 } else { 1.0 })
        });
    }
    a.sprite(T_COPPER_INGOT, &INGOT_SPRITE, &[('#', rgb(90, 45, 25)), ('b', rgb(210, 115, 70)), ('h', rgb(245, 170, 120)), ('d', rgb(150, 75, 45))]);
    // Bamboo: green stalks with knots.
    a.each(T_BAMBOO, |x, y, r, _| {
        let stalk = (6..10).contains(&x);
        if !stalk {
            // A leaf or two off the side.
            return if (y == 3 && (10..14).contains(&x)) || (y == 9 && (2..6).contains(&x)) { shade(rgb(90, 160, 50), r.range(0.9, 1.05)) } else { [0, 0, 0, 0] };
        }
        let knot = y % 8 == 7;
        shade(if knot { rgb(95, 140, 40) } else if x == 6 { rgb(150, 200, 80) } else { rgb(120, 180, 60) }, r.range(0.93, 1.05))
    });
    a.each(T_BAMBOO_BLOCK_SIDE, |x, y, r, _| {
        let knot = y % 8 == 7;
        let edge = x % 4 == 0;
        shade(if knot { rgb(105, 140, 45) } else if edge { rgb(110, 155, 50) } else { rgb(140, 185, 70) }, r.range(0.92, 1.05))
    });
    a.each(T_BAMBOO_BLOCK_TOP, |x, y, r, _| {
        let ring = ((x as f32 - 7.5).hypot(y as f32 - 7.5) as i32) % 3 == 0;
        shade(if ring { rgb(150, 175, 90) } else { rgb(195, 205, 130) }, r.range(0.93, 1.05))
    });
    a.each(T_BAMBOO_PLANKS, |x, y, r, _| {
        let seam = x % 4 == 3 || (y + (x / 4) * 5) % 16 == 0;
        shade(if seam { rgb(160, 140, 55) } else { rgb(215, 195, 90) }, r.range(0.92, 1.06))
    });
    a.each(T_BAMBOO_MOSAIC, |x, y, r, _| {
        let (bx, by) = (x / 8, y / 8);
        let across = (bx + by) % 2 == 0;
        let seam = if across { y % 8 == 0 || y % 2 == 1 && x % 8 == 0 } else { x % 8 == 0 || x % 2 == 1 && y % 8 == 0 };
        let stripe = if across { y % 2 == 0 } else { x % 2 == 0 };
        shade(if seam { rgb(150, 130, 50) } else if stripe { rgb(220, 200, 95) } else { rgb(195, 175, 80) }, r.range(0.94, 1.05))
    });
    // Coral: bumpy, bright, cells with darker rims.
    let corals = [[60, 90, 220], [230, 110, 160], [170, 60, 190], [220, 60, 50], [150, 145, 140]];
    for (i, c) in corals.into_iter().enumerate() {
        let pts = a.random_points(9);
        let tile = if i < 4 { T_CORAL + i as u16 } else { T_DEAD_CORAL };
        a.each(tile, move |x, y, r, _| {
            let (_, edge) = Atlas::cells(&pts, x, y);
            let k = if edge < 1.0 { 0.7 } else if edge > 3.0 { 1.12 } else { 1.0 };
            shade(rgb(c[0], c[1], c[2]), k * r.range(0.9, 1.06))
        });
    }
    a.sprite(T_GLIDER, &GLIDER_SPRITE, &[('#', rgb(40, 30, 55)), ('b', rgb(120, 95, 150)), ('B', rgb(160, 130, 195)), ('m', rgb(210, 190, 230))]);
    a.each(T_GLIDER_WING, |x, y, r, _| {
        let rib = x % 5 == 0 || y == 0;
        shade(if rib { rgb(90, 70, 115) } else { rgb(165, 135, 200) }, r.range(0.92, 1.05))
    });
    a.sprite(T_ROCKET, &ROCKET_SPRITE, &[('#', rgb(50, 20, 20)), ('r', rgb(210, 50, 45)), ('w', rgb(240, 235, 230)), ('o', rgb(150, 110, 60)), ('y', rgb(250, 210, 60))]);
    a.sprite(T_SPEAR, &SPEAR_SPRITE, &[('#', rgb(25, 45, 50)), ('h', rgb(90, 190, 180)), ('t', rgb(60, 130, 125))]);
    // Fishies: a speckled scaly body, a face with a big eye, and see-through fins.
    a.each(T_FISHY, |x, y, r, _| shade(if (x + y * 2) % 5 == 0 { rgb(200, 140, 70) } else { rgb(230, 170, 90) }, r.range(0.9, 1.06)));
    a.copy(T_FISHY, T_FISHY_FACE);
    for (x, y, c) in [(3, 5, rgb(255, 255, 255)), (4, 5, rgb(20, 20, 20)), (3, 6, rgb(20, 20, 20)), (4, 6, rgb(20, 20, 20)), (11, 5, rgb(20, 20, 20)), (12, 5, rgb(255, 255, 255)), (11, 6, rgb(20, 20, 20)), (12, 6, rgb(20, 20, 20))] {
        a.set(T_FISHY_FACE, x, y, c);
    }
    for x in 6..10 {
        a.set(T_FISHY_FACE, x, 11, rgb(150, 90, 50));
    }
    a.each(T_FISHY_FIN, |x, _, r, _| shade(if x % 3 == 0 { rgb(200, 110, 60) } else { rgb(240, 150, 80) }, r.range(0.9, 1.05)));
    // The Soggy Groaner: a drowned Groaner, teal and waterlogged.
    a.each(T_SOGGY_SKIN, |_, _, r, p| {
        let _ = p;
        shade(rgb(80, 150, 140), r.range(0.85, 1.08))
    });
    a.copy(T_SOGGY_SKIN, T_SOGGY_FACE);
    for (x, y, c) in [(3, 6, rgb(120, 230, 255)), (4, 6, rgb(120, 230, 255)), (11, 6, rgb(120, 230, 255)), (12, 6, rgb(120, 230, 255))] {
        a.set(T_SOGGY_FACE, x, y, c);
    }
    for x in 5..11 {
        a.set(T_SOGGY_FACE, x, 11, rgb(30, 60, 55));
    }
    a.each(T_SOGGY_SHIRT, |x, y, r, _| shade(if (x * 7 + y * 3) % 11 == 0 { rgb(60, 110, 60) } else { rgb(90, 120, 150) }, r.range(0.85, 1.05)));
    a.each(T_SOGGY_PANTS, |x, y, r, _| shade(if (x + y) % 9 == 0 { rgb(60, 100, 60) } else { rgb(70, 80, 120) }, r.range(0.85, 1.05)));
}

const BOTTLE_SPRITE: [&str; 16] = [
    "................",
    "......####......",
    "......#cc#......",
    ".......##.......",
    "......#..#......",
    ".....#hhhh#.....",
    "....#hhhhhh#....",
    "....#hbbbbb#....",
    "....#bbbbbb#....",
    "....#bbbbbd#....",
    "....#bbbbdd#....",
    "....#bbbddd#....",
    ".....#dddd#.....",
    "......####......",
    "................",
    "................",
];

const SMOKER_SPRITE: [&str; 16] = [
    "........ss.s....",
    ".......s..s.....",
    "........ss......",
    "......#nn#......",
    ".....#nnnn#.....",
    ".....#iiii#.....",
    "....#iiiiii#....",
    "....#ihiiii#....",
    "....#ihiiii#....",
    "....#iiiiii#..##",
    "....#iiiiii#.#ww",
    "....#iiiiii##www",
    "....#iiiiii#wwww",
    ".....######.#ww.",
    "..............#.",
    "................",
];

const HIVE_TOOL_SPRITE: [&str; 16] = [
    "................",
    "............##..",
    "...........#hh#.",
    "..........#hhh#.",
    ".........#hhh#..",
    "........#hhh#...",
    ".......#hhh#....",
    "......#hhh#.....",
    ".....#hhh#......",
    "....#hhh#.......",
    "...#rrr#........",
    "..#rrr#.........",
    ".#rrr#..........",
    ".#rr#...........",
    "..##............",
    "................",
];

const QUEEN_SPRITE: [&str; 16] = [
    "......####......",
    ".....#cccc#.....",
    "......####......",
    ".....#....#.....",
    "....#......#....",
    "....#.k.k..#....",
    "....#.kkk..#....",
    "....#wyyyw.#....",
    "....#.kkk..#....",
    "....#.yyy..#....",
    "....#.kkk..#....",
    "....#..y...#....",
    "....#......#....",
    ".....######.....",
    "................",
    "................",
];

const SEEDS_SPRITE: [&str; 16] = [
    "................",
    "................",
    "................",
    "......o.........",
    "....o....o......",
    "........o.......",
    "...o..o.....o...",
    ".......o..o.....",
    "....o......o....",
    "......o.o.......",
    "..o.........o...",
    ".....o...o......",
    "........o.......",
    "................",
    "................",
    "................",
];

/// Nests, hives, honey, flowers and the bees themselves.
fn paint_bees(a: &mut Atlas) {
    // A wild nest: a papery, stripy cylinder in tree colours.
    a.each(T_NEST_TOP, |x, y, r, _| {
        let d = ((x as f32 - 7.5).powi(2) + (y as f32 - 7.5).powi(2)).sqrt();
        let ring = (d as usize).is_multiple_of(3);
        shade(if ring { rgb(170, 125, 60) } else { rgb(205, 165, 85) }, r.range(0.9, 1.05))
    });
    for (tile, drip) in [(T_NEST_SIDE, false), (T_NEST_HONEY_SIDE, true)] {
        a.each(tile, move |x, y, r, _| {
            let band = y % 4 == 0;
            let hole = (6..10).contains(&x) && (7..10).contains(&y);
            let honey = drip && ((x == 7 && (10..=13).contains(&y)) || (x == 8 && (10..=11).contains(&y)) || hole);
            if honey {
                return shade(rgb(245, 175, 30), r.range(0.95, 1.08));
            }
            if hole {
                return rgb(40, 25, 10);
            }
            shade(if band { rgb(160, 115, 55) } else { rgb(210, 170, 90) }, r.range(0.9, 1.05))
        });
    }
    // A crafted hive: planks with a landing board and an entrance.
    a.each(T_HIVE_TOP, |x, y, r, _| {
        let rim = x == 0 || y == 0 || x == 15 || y == 15;
        let grain = (y * 5 + x / 4) % 7 == 0;
        shade(if rim { rgb(120, 85, 45) } else if grain { rgb(170, 125, 70) } else { rgb(195, 150, 90) }, r.range(0.92, 1.05))
    });
    for (tile, state) in [(T_HIVE_SIDE, 0), (T_HIVE_BUSY_SIDE, 1), (T_HIVE_HONEY_SIDE, 2)] {
        a.each(tile, move |x, y, r, _| {
            let rim = x == 0 || x == 15;
            let slat = y % 5 == 0;
            let door = (5..11).contains(&x) && (9..11).contains(&y);
            let board = (3..13).contains(&x) && y == 11;
            if door {
                // Busy: a bee or two in the doorway.
                let bee = state >= 1 && (x == 6 || x == 9);
                return if bee { rgb(240, 200, 40) } else { rgb(35, 22, 10) };
            }
            if board {
                return shade(rgb(140, 100, 55), r.range(0.95, 1.05));
            }
            if state == 2 && (12..=14).contains(&y) && (x == 4 || x == 11) {
                return shade(rgb(245, 170, 25), r.range(0.95, 1.08));
            }
            shade(if rim || slat { rgb(150, 105, 55) } else { rgb(200, 155, 95) }, r.range(0.92, 1.05))
        });
    }
    // Honey block: amber, see-through-ish, with a lighter middle.
    a.each(T_HONEY_BLOCK_TOP, |x, y, r, _| {
        let rim = x == 0 || y == 0 || x == 15 || y == 15;
        let inner = (3..13).contains(&x) && (3..13).contains(&y);
        let mut c = shade(if rim { rgb(215, 130, 15) } else if inner { rgb(250, 190, 50) } else { rgb(240, 160, 30) }, r.range(0.95, 1.05));
        c[3] = 215;
        c
    });
    a.copy(T_HONEY_BLOCK_TOP, T_HONEY_BLOCK_SIDE);
    // Flowers: a yellow puff, a blue star, a purple spike.
    let stem = rgb(55, 130, 35);
    a.each(T_DANDELION, move |x, y, r, _| {
        let d = ((x as f32 - 7.5).powi(2) + (y as f32 - 5.0).powi(2)).sqrt();
        if d < 3.2 {
            shade(rgb(250, 215, 40), r.range(0.88, 1.08))
        } else if (x == 7 || x == 8) && y > 7 || (x == 9 && y == 11) || (x == 10 && y == 10) {
            stem
        } else {
            [0, 0, 0, 0]
        }
    });
    a.each(T_CORNFLOWER, move |x, y, r, _| {
        let (dx, dy) = (x as i32 - 7, y as i32 - 5);
        let petal = (dx.abs() <= 3 && dy == 0) || (dy.abs() <= 3 && dx == 0) || (dx.abs() == dy.abs() && dx.abs() <= 2);
        if dx == 0 && dy == 0 {
            rgb(30, 30, 90)
        } else if petal {
            shade(rgb(70, 110, 230), r.range(0.85, 1.1))
        } else if x == 7 && y > 8 || (x == 6 && y == 12) {
            stem
        } else {
            [0, 0, 0, 0]
        }
    });
    a.each(T_LAVENDER, move |x, y, r, _| {
        let spike = |cx: usize, top: usize| x.abs_diff(cx) <= 1 && y >= top && y < top + 7 && (y + x) % 2 == 0;
        if spike(5, 2) || spike(10, 3) || spike(8, 1) {
            shade(rgb(165, 110, 215), r.range(0.85, 1.1))
        } else if (x == 5 && y >= 9) || (x == 10 && y >= 10) || (x == 8 && y >= 8) {
            stem
        } else {
            [0, 0, 0, 0]
        }
    });
    a.each(T_TORCH_SPROUT, move |x, y, _, _| {
        if (x == 7 || x == 8) && y >= 10 {
            stem
        } else if (y == 9 && (5..11).contains(&x)) || (y == 8 && (x == 5 || x == 10)) {
            rgb(80, 160, 50)
        } else {
            [0, 0, 0, 0]
        }
    });
    a.each(T_TORCHFLOWER, move |x, y, r, _| {
        let (dx, dy) = (x as f32 - 7.5, y as f32 - 4.5);
        let d = (dx * dx + dy * dy).sqrt();
        if d < 1.6 {
            rgb(255, 245, 160)
        } else if d < 3.6 && dy < 1.5 {
            shade(if d < 2.6 { rgb(255, 170, 40) } else { rgb(230, 80, 30) }, r.range(0.9, 1.1))
        } else if (x == 7 || x == 8) && y > 7 || (y == 11 && (5..11).contains(&x)) {
            stem
        } else {
            [0, 0, 0, 0]
        }
    });
    // Honey bottles: a glass bottle filled with each flavour's colour.
    for (i, f) in crate::bees::Flavour::ALL.iter().enumerate() {
        let c = f.colour();
        let col = rgb(c[0], c[1], c[2]);
        a.sprite(T_HONEY_FIRST + i as u16, &BOTTLE_SPRITE, &[('#', rgb(70, 60, 50)), ('c', rgb(150, 110, 70)), ('h', shade(col, 1.2)), ('b', col), ('d', shade(col, 0.75))]);
    }
    // Honeycomb: hexagons.
    a.each(T_HONEYCOMB, |x, y, r, _| {
        let (cx, cy) = (x as f32 - 7.5, y as f32 - 7.5);
        if cx.abs() > 6.5 || cy.abs() > 6.0 || (cx.abs() + cy.abs() * 0.6) > 8.0 {
            return [0, 0, 0, 0];
        }
        let row = y / 4;
        let col = (x + if row % 2 == 1 { 2 } else { 0 }) / 4;
        let edge = y % 4 == 0 || (x + if row % 2 == 1 { 2 } else { 0 }) % 4 == 0;
        let _ = col;
        shade(if edge { rgb(200, 130, 20) } else { rgb(250, 195, 60) }, r.range(0.92, 1.06))
    });
    a.sprite(T_SMOKER, &SMOKER_SPRITE, &[('#', rgb(50, 40, 30)), ('n', rgb(120, 120, 125)), ('i', rgb(170, 170, 175)), ('h', rgb(220, 220, 225)), ('w', rgb(160, 110, 60)), ('s', rgb(200, 200, 200))]);
    a.sprite(T_HIVE_TOOL, &HIVE_TOOL_SPRITE, &[('#', rgb(40, 40, 45)), ('h', rgb(200, 200, 205)), ('r', rgb(190, 40, 40))]);
    a.sprite(T_QUEEN, &QUEEN_SPRITE, &[('#', rgb(150, 180, 190)), ('c', rgb(160, 110, 60)), ('k', rgb(30, 25, 20)), ('y', rgb(245, 200, 40)), ('w', rgb(220, 235, 245))]);
    a.sprite(T_TORCH_SEEDS, &SEEDS_SPRITE, &[('o', rgb(170, 90, 40))]);
    // The bee: yellow with black stripes, a face, and see-through wings.
    a.each(T_BEE, |x, _, r, _| shade(if (x / 3) % 2 == 1 { rgb(40, 30, 20) } else { rgb(240, 195, 40) }, r.range(0.92, 1.05)));
    a.each(T_BEE_FACE, |x, y, r, _| {
        if (y == 6 || y == 7) && (x == 4 || x == 5 || x == 10 || x == 11) {
            rgb(20, 15, 30)
        } else if y >= 12 && (6..10).contains(&x) {
            rgb(60, 40, 20)
        } else {
            shade(rgb(240, 195, 40), r.range(0.92, 1.05))
        }
    });
    a.each(T_BEE_WING, |x, y, _, _| {
        let rim = x == 0 || y == 0 || x == 15 || y == 15;
        if rim { [200, 230, 255, 230] } else { [215, 240, 255, 150] }
    });
}

const SCUTE_SPRITE: [&str; 16] = [
    "................",
    "................",
    "................",
    ".....######.....",
    "....#hhhhhh#....",
    "...#hbbbbbbh#...",
    "...#bbdbbdbb#...",
    "...#bbbbbbbb#...",
    "...#bdbbdbbd#...",
    "...#bbbbbbbb#...",
    "....#bbdbbb#....",
    ".....#dddd#.....",
    "......####......",
    "................",
    "................",
    "................",
];

const WOLF_ARMOR_SPRITE: [&str; 16] = [
    "................",
    "................",
    "..##............",
    ".#hh#...........",
    ".#bb############",
    ".#bbhhhhhhhhhhb#",
    "..#bbbdbbdbbdbb#",
    "..#bbbbbbbbbbbb#",
    "..#bdbbdbbdbbdb#",
    "..#bbbbbbbbbbbb#",
    "..##bb#####bb###",
    "...#bb#...#bb#..",
    "...#bb#...#bb#..",
    "...####...####..",
    "................",
    "................",
];

/// Sneakers, Ribbits, Rollos, the Hush and their bits.
fn paint_critters(a: &mut Atlas) {
    let orange = rgb(220, 110, 40);
    a.each(T_FOX, move |_, _, r, _| shade(orange, r.range(0.9, 1.06)));
    a.each(T_FOX_FACE, move |x, y, r, _| {
        if (y == 5 || y == 6) && (x == 3 || x == 4 || x == 11 || x == 12) {
            rgb(25, 20, 15)
        } else if y >= 9 && (3..13).contains(&x) {
            shade(rgb(245, 240, 230), r.range(0.95, 1.03))
        } else {
            shade(orange, r.range(0.9, 1.06))
        }
    });
    a.each(T_FOX_TAIL, move |_, y, r, _| shade(if y < 5 { rgb(245, 240, 230) } else { orange }, r.range(0.9, 1.06)));
    a.each(T_FOX_DARK, |_, _, r, _| shade(rgb(60, 35, 25), r.range(0.9, 1.08)));
    let green = rgb(110, 150, 60);
    a.each(T_FROG, move |x, y, r, _| shade(if (x * 3 + y * 5) % 11 == 0 { rgb(85, 120, 45) } else { green }, r.range(0.9, 1.06)));
    a.each(T_FROG_FACE, move |x, y, r, _| {
        if y == 11 && (2..14).contains(&x) {
            rgb(60, 40, 30)
        } else if y > 11 {
            shade(rgb(215, 200, 140), r.range(0.95, 1.03))
        } else {
            shade(green, r.range(0.9, 1.06))
        }
    });
    a.each(T_FROG_EYE, |x, y, _, _| if (5..11).contains(&x) && (5..11).contains(&y) { rgb(20, 20, 20) } else { rgb(230, 200, 60) });
    a.each(T_SHELL, |x, y, r, _| {
        let band = y % 4 == 0;
        let plate = (x + (y / 4) * 2) % 4 == 0;
        shade(if band || plate { rgb(140, 85, 70) } else { rgb(200, 130, 110) }, r.range(0.92, 1.05))
    });
    a.each(T_ROLLO_SKIN, |_, _, r, _| shade(rgb(215, 160, 140), r.range(0.92, 1.05)));
    a.each(T_ROLLO_FACE, |x, y, r, _| {
        if y == 6 && (x == 4 || x == 11) {
            rgb(20, 15, 15)
        } else if y >= 11 && (6..10).contains(&x) {
            rgb(120, 70, 60)
        } else {
            shade(rgb(215, 160, 140), r.range(0.92, 1.05))
        }
    });
    a.each(T_HUSH, |x, y, r, p| {
        let n = p.noise3(x as f32 * 0.4, y as f32 * 0.4, 7.0);
        shade(if n > 0.25 { rgb(25, 70, 80) } else { rgb(15, 40, 50) }, r.range(0.9, 1.06))
    });
    a.each(T_HUSH_FACE, |x, y, r, _| {
        // No eyes at all: a gaping jaw.
        if (9..=12).contains(&y) && (3..13).contains(&x) {
            if y == 9 || y == 12 { rgb(200, 230, 225) } else { rgb(5, 15, 20) }
        } else {
            shade(rgb(15, 40, 50), r.range(0.9, 1.06))
        }
    });
    a.each(T_HUSH_GLOW, |_, _, r, _| shade(rgb(70, 220, 230), r.range(0.85, 1.1)));
    // Froglights: pale, glowing, with soft panels.
    for (i, c) in [[245, 225, 150], [200, 240, 180], [235, 200, 235]].into_iter().enumerate() {
        a.each(T_FROGLIGHT + i as u16, move |x, y, r, _| {
            let rim = x == 0 || y == 0 || x == 15 || y == 15 || x == 8 || y == 8;
            shade(rgb(c[0], c[1], c[2]), r.range(0.95, 1.04) * if rim { 0.82 } else { 1.0 })
        });
    }
    let shell = [('#', rgb(80, 50, 40)), ('h', rgb(230, 160, 140)), ('b', rgb(200, 130, 110)), ('d', rgb(140, 85, 70))];
    a.sprite(T_SCUTE, &SCUTE_SPRITE, &shell);
    a.sprite(T_WOLF_ARMOR_ITEM, &WOLF_ARMOR_SPRITE, &shell);
    a.each(T_WOLF_ARMOR_WORN, |x, y, r, _| {
        let plate = (x % 5 == 0) || (y % 4 == 0);
        shade(if plate { rgb(140, 85, 70) } else { rgb(200, 130, 110) }, r.range(0.92, 1.05))
    });
}

const BRUSH_SPRITE: [&str; 16] = [
    "................",
    "............###.",
    "...........#ttt#",
    "..........#ttt#.",
    ".........#hhh#..",
    "........#hhh#...",
    ".......#sss#....",
    "......#sss#.....",
    ".....#sss#......",
    "....#sss#.......",
    "...#sss#........",
    "..#ss#..........",
    ".#ss#...........",
    ".##.............",
    "................",
    "................",
];

const JOURNAL_SPRITE: [&str; 16] = [
    "................",
    "..###########...",
    "..#ccccccccc#...",
    "..#cpppppppc##..",
    "..#cplllllpc#w..",
    "..#cpppppppc#w..",
    "..#cplllllpc#w..",
    "..#cpppppppc#w..",
    "..#cplllpppc#w..",
    "..#cpppppppc#w..",
    "..#cplllllpc#w..",
    "..#cpppppppc#w..",
    "..#ccccccccc#...",
    "..###########...",
    "................",
    "................",
];

const SHARD_SPRITE: [&str; 16] = [
    "................",
    "....#######.....",
    "...#bbbbbbb##...",
    "..#bbbbbbbbbb#..",
    "..#bbbbbbbbbbb#.",
    "..#bbbbbbbbbbb#.",
    "..#bbbbbbbbbbb#.",
    "..#bbbbbbbbbbb#.",
    "..#bbbbbbbbbbb#.",
    "..#bbbbbbbbbb#..",
    "...#bbbbbbbbb#..",
    "...#bbbbbbbb#...",
    "....#bbbbbb#....",
    ".....######.....",
    "................",
    "................",
];

const COIN_SPRITE: [&str; 16] = [
    "................",
    "................",
    ".....######.....",
    "....#hhhhhh#....",
    "...#hbbbbbbh#...",
    "..#hbbddddbbd#..",
    "..#hbdbbbbdbd#..",
    "..#hbdbbbbdbd#..",
    "..#hbdbbbbdbd#..",
    "..#hbbddddbbd#..",
    "...#bbbbbbbd#...",
    "....#dddddd#....",
    ".....######.....",
    "................",
    "................",
    "................",
];

const TABLET_SPRITE: [&str; 16] = [
    "................",
    "....########....",
    "...#bbbbbbbb#...",
    "...#bddbdbdb#...",
    "...#bbbbbbbb#...",
    "...#bdbddbdb#...",
    "...#bbbbbbbb#...",
    "...#bddbbdbb#...",
    "...#bbbbbbbb#...",
    "...#bdbdbddb#...",
    "...#bbbbbbbb#...",
    "...#bddbdbbb#...",
    "...#bbbbbbbb#...",
    "....########....",
    "................",
    "................",
];

const FRAGMENT_SPRITE: [&str; 16] = [
    "................",
    "...######.......",
    "..#pppppp##.....",
    "..#ppprpppp##...",
    "..#pppprppppp#..",
    "..#ppppprpppp#..",
    "..#ppppppxpp#...",
    "..#pppppppp#....",
    "..#ppgggppp#....",
    "..#pggpgggpp#...",
    "..#ppppppppp#...",
    "...#pppppp##....",
    "....######......",
    "................",
    "................",
    "................",
];

const ECHO_SPRITE: [&str; 16] = [
    "................",
    "........#.......",
    ".......#h#......",
    "......#hbh#.....",
    ".....#hbbbd#....",
    "....#hbbbbbd#...",
    "....#bbbbbbd#...",
    "....#bbbbbdd#...",
    ".....#bbbdd#....",
    "......#bdd#.....",
    ".......#d#......",
    "........#.......",
    "................",
    "................",
    "................",
    "................",
];

const TEMPLATE_SPRITE: [&str; 16] = [
    "................",
    "..############..",
    "..#dddddddddd#..",
    "..#dbbbbbbbbd#..",
    "..#dbhhhhhhbd#..",
    "..#dbhrrrrhbd#..",
    "..#dbhrbbrhbd#..",
    "..#dbhrbbrhbd#..",
    "..#dbhrrrrhbd#..",
    "..#dbhhhhhhbd#..",
    "..#dbbbbbbbbd#..",
    "..#dddddddddd#..",
    "..############..",
    "................",
    "................",
    "................",
];

/// Deepslate and sculk, dig sites and their finds, Scorchite.
fn paint_ancient(a: &mut Atlas) {
    // Deepslate: dark grey with vertical streaks.
    a.each(T_DEEPSLATE, |x, y, r, p| {
        let n = p.noise3(x as f32 * 0.9, y as f32 * 0.15, 3.0);
        shade(rgb(78, 78, 84), r.range(0.9, 1.05) * if n > 0.2 { 0.82 } else { 1.0 })
    });
    a.each(T_DEEPSLATE_TOP, |x, y, r, _| {
        let ring = ((x as i32 - 7).abs().max((y as i32 - 7).abs())) % 4 == 0;
        shade(rgb(78, 78, 84), r.range(0.9, 1.05) * if ring { 0.85 } else { 1.0 })
    });
    let pts = a.random_points(9);
    a.each(T_COBBLED_DEEPSLATE, move |x, y, r, _| {
        let (_, edge) = Atlas::cells(&pts, x, y);
        shade(rgb(70, 70, 76), r.range(0.9, 1.08) * if edge < 1.0 { 0.65 } else { 1.0 })
    });
    a.each(T_DEEPSLATE_BRICKS, |x, y, r, _| {
        let row = y / 4;
        let mortar = y % 4 == 3 || (x + if row % 2 == 1 { 4 } else { 0 }) % 8 == 7;
        shade(if mortar { rgb(45, 45, 50) } else { rgb(85, 85, 92) }, r.range(0.92, 1.05))
    });
    a.each(T_DEEPSLATE_TILES, |x, y, r, _| {
        let mortar = y % 4 == 3 || x % 4 == 3;
        shade(if mortar { rgb(40, 40, 45) } else { rgb(70, 70, 78) }, r.range(0.92, 1.05))
    });
    a.each(T_REINFORCED_SIDE, |x, y, r, _| {
        let band = y <= 2 || y >= 13;
        let rivet = band && x % 5 == 2 && (y == 1 || y == 14);
        shade(if rivet { rgb(200, 200, 190) } else if band { rgb(110, 105, 95) } else { rgb(60, 62, 70) }, r.range(0.92, 1.05))
    });
    a.each(T_REINFORCED_TOP, |x, y, r, _| {
        let rim = x <= 1 || y <= 1 || x >= 14 || y >= 14;
        shade(if rim { rgb(110, 105, 95) } else { rgb(60, 62, 70) }, r.range(0.92, 1.05))
    });
    // Sculk: deep teal-black with glowing cyan flecks.
    a.each(T_SCULK, |x, y, r, p| {
        let n = p.noise3(x as f32 * 0.5, y as f32 * 0.5, 11.0);
        if n > 0.42 {
            shade(rgb(60, 200, 210), r.range(0.85, 1.1))
        } else {
            shade(rgb(12, 30, 40), r.range(0.85, 1.15))
        }
    });
    for (tile, lit) in [(T_SENSOR_TOP, false), (T_SENSOR_ACTIVE_TOP, true)] {
        a.each(tile, move |x, y, r, _| {
            let tendril = (x == 4 || x == 11) && (3..13).contains(&y);
            let rim = x == 0 || y == 0 || x == 15 || y == 15;
            if tendril {
                if lit { rgb(140, 255, 250) } else { rgb(60, 190, 200) }
            } else if rim {
                rgb(20, 45, 55)
            } else {
                shade(rgb(15, 50, 60), r.range(0.85, 1.1))
            }
        });
    }
    a.each(T_SENSOR_SIDE, |x, y, r, _| {
        let top = y < 8;
        let tendril = top && (x == 4 || x == 11) && y >= 2;
        // (Solid all over: a full block with holes in its sides shows the world through it.)
        if tendril {
            rgb(60, 190, 200)
        } else if top {
            shade(rgb(10, 32, 40), r.range(0.85, 1.1))
        } else {
            shade(rgb(15, 50, 60), r.range(0.85, 1.1))
        }
    });
    a.each(T_SHRIEKER_TOP, |x, y, r, _| {
        let d = ((x as f32 - 7.5).powi(2) + (y as f32 - 7.5).powi(2)).sqrt();
        if d < 3.0 {
            rgb(10, 15, 20)
        } else if d < 5.0 {
            shade(rgb(220, 215, 190), r.range(0.9, 1.05))
        } else {
            shade(rgb(15, 45, 55), r.range(0.85, 1.1))
        }
    });
    a.each(T_SHRIEKER_SIDE, |x, y, r, _| {
        let jaw = (2..14).contains(&x) && (y == 3 || y == 6) && x % 2 == 0;
        shade(if jaw { rgb(220, 215, 190) } else { rgb(15, 45, 55) }, r.range(0.85, 1.1))
    });
    a.each(T_CATALYST_TOP, |x, y, r, _| {
        let d = ((x as f32 - 7.5).powi(2) + (y as f32 - 7.5).powi(2)).sqrt();
        shade(if d < 3.5 { rgb(120, 240, 230) } else { rgb(30, 35, 40) }, r.range(0.85, 1.1))
    });
    a.each(T_CATALYST_SIDE, |x, y, r, _| {
        let bone = (x == 3 || x == 12) && y < 12 || y == 12;
        shade(if bone { rgb(215, 210, 195) } else { rgb(30, 35, 40) }, r.range(0.88, 1.06))
    });
    a.each(T_SOUL_LANTERN, |x, y, r, _| {
        let cage = x == 3 || x == 12 || y == 3 || y == 13;
        let glow = (4..12).contains(&x) && (4..13).contains(&y);
        if cage && (3..=12).contains(&x) && (3..=13).contains(&y) {
            rgb(50, 50, 55)
        } else if glow {
            shade(rgb(110, 220, 240), r.range(0.9, 1.1))
        } else if (7..9).contains(&x) && y < 3 {
            rgb(50, 50, 55)
        } else {
            [0, 0, 0, 0]
        }
    });
    // Suspicious blocks: sand and gravel with something glinting in them.
    a.copy(T_SAND, T_SUS_SAND);
    a.copy(T_GRAVEL, T_SUS_GRAVEL);
    for (tile, c) in [(T_SUS_SAND, rgb(150, 120, 70)), (T_SUS_GRAVEL, rgb(90, 80, 70))] {
        for (x, y) in [(3, 4), (4, 4), (11, 9), (12, 10), (6, 12), (9, 3), (10, 3)] {
            a.set(tile, x, y, c);
        }
        a.set(tile, 7, 7, rgb(230, 200, 120));
    }
    a.each(T_BENCH_TOP, |x, y, r, _| {
        let rim = x == 0 || y == 0 || x == 15 || y == 15;
        let cloth = (3..13).contains(&x) && (3..13).contains(&y);
        shade(if rim { rgb(110, 80, 45) } else if cloth { rgb(70, 110, 85) } else { rgb(170, 130, 80) }, r.range(0.92, 1.05))
    });
    a.each(T_BENCH_SIDE, |x, y, r, _| {
        let top = y < 3;
        let leg = !(3..=12).contains(&x) && y >= 3;
        let tool = y == 6 && (5..11).contains(&x);
        if tool {
            rgb(200, 200, 205)
        } else if top || leg {
            shade(rgb(150, 110, 65), r.range(0.92, 1.05))
        } else {
            shade(rgb(95, 70, 40), r.range(0.92, 1.05))
        }
    });
    // Pots: terracotta, with a motif.
    let clay = rgb(175, 95, 60);
    a.each(T_POT_TOP, move |x, y, r, _| {
        let d = ((x as f32 - 7.5).powi(2) + (y as f32 - 7.5).powi(2)).sqrt();
        if d < 3.0 {
            rgb(40, 20, 12)
        } else {
            shade(clay, r.range(0.9, 1.05))
        }
    });
    for i in 0..13u16 {
        a.each(T_POT_SIDE_FIRST + i, move |_, y, r, _| {
            let rim = y <= 1 || y >= 14;
            shade(clay, r.range(0.9, 1.05) * if rim { 0.8 } else { 1.0 })
        });
        if i > 0 {
            let motif = crate::archaeology::SHARDS[i as usize - 1].2;
            for (y, row) in motif.iter().enumerate() {
                for (x, ch) in row.chars().enumerate() {
                    if ch == '#' {
                        a.set(T_POT_SIDE_FIRST + i, x + 4, y + 4, rgb(60, 30, 20));
                    }
                }
            }
        }
    }
    a.each(T_GRINDSTONE_SIDE, |x, y, r, _| {
        let d = ((x as f32 - 7.5).powi(2) + (y as f32 - 7.5).powi(2)).sqrt();
        if d < 1.5 {
            rgb(110, 80, 45)
        } else if d < 6.5 {
            shade(rgb(150, 150, 150), r.range(0.85, 1.08))
        } else if y > 11 {
            shade(rgb(130, 95, 55), r.range(0.92, 1.05))
        } else {
            [0, 0, 0, 0]
        }
    });
    a.each(T_GRINDSTONE_TOP, |x, _, r, _| shade(if (5..11).contains(&x) { rgb(150, 150, 150) } else { rgb(130, 95, 55) }, r.range(0.88, 1.05)));
    a.each(T_SMITHING_TOP, |x, y, r, _| {
        let rim = x == 0 || y == 0 || x == 15 || y == 15;
        shade(if rim { rgb(40, 40, 45) } else { rgb(60, 60, 68) }, r.range(0.9, 1.06))
    });
    a.each(T_SMITHING_SIDE, |x, y, r, _| {
        let top = y < 4;
        let hammer = y == 7 && (4..12).contains(&x) || (x == 7 && (7..12).contains(&y));
        if hammer {
            rgb(190, 190, 195)
        } else if top {
            shade(rgb(55, 55, 62), r.range(0.9, 1.05))
        } else {
            shade(rgb(140, 100, 60), r.range(0.92, 1.05))
        }
    });
    a.each(T_OLD_DEBRIS_SIDE, |x, y, r, _| {
        let swirl = ((x as f32 * 0.8).sin() + (y as f32 * 0.6).cos()) > 0.8;
        shade(if swirl { rgb(120, 85, 70) } else { rgb(85, 60, 55) }, r.range(0.88, 1.07))
    });
    a.each(T_OLD_DEBRIS_TOP, |x, y, r, _| {
        let d = ((x as f32 - 7.5).powi(2) + (y as f32 - 7.5).powi(2)).sqrt();
        shade(if (d as i32) % 3 == 0 { rgb(120, 85, 70) } else { rgb(85, 60, 55) }, r.range(0.88, 1.07))
    });
    // Tools and finds.
    let handle = [('#', rgb(40, 30, 20)), ('s', rgb(140, 100, 55))];
    a.sprite(T_BRUSH, &BRUSH_SPRITE, &[handle[0], handle[1], ('h', rgb(215, 125, 80)), ('t', rgb(235, 225, 200))]);
    a.sprite(T_DIAMOND_BRUSH, &BRUSH_SPRITE, &[handle[0], handle[1], ('h', rgb(90, 240, 225)), ('t', rgb(250, 250, 255))]);
    a.sprite(T_JOURNAL, &JOURNAL_SPRITE, &[('#', rgb(40, 25, 15)), ('c', rgb(120, 75, 40)), ('p', rgb(235, 225, 195)), ('l', rgb(120, 110, 100)), ('w', rgb(200, 30, 30))]);
    for (i, (_, _, motif)) in crate::archaeology::SHARDS.iter().enumerate() {
        let tile = T_SHARD_FIRST + i as u16;
        a.sprite(tile, &SHARD_SPRITE, &[('#', rgb(90, 45, 25)), ('b', clay)]);
        for (y, row) in motif.iter().enumerate() {
            for (x, ch) in row.chars().enumerate() {
                if ch == '#' {
                    a.set(tile, x + 4, y + 4, rgb(60, 30, 20));
                }
            }
        }
    }
    // Relics: a shape per kind (charm, mask, disc...), coloured by culture.
    let shapes: [[&str; 10]; 3] = [
        ["...####...", "..#hhhh#..", ".#hbbbbh#.", "#hbbddbbh#", "#hbdbbdbh#", "#hbdbbdbh#", "#hbbddbbh#", ".#hbbbbh#.", "..#hhhh#..", "...####..."],
        ["..######..", ".#hhhhhh#.", "#hbbbbbbh#", "#b##bb##b#", "#bbbbbbbb#", "#bbb##bbb#", ".#bbbbbb#.", ".#b#bb#b#.", "..#bbbb#..", "...####..."],
        ["....##....", "...#hh#...", "..#hbbh#..", "..#bbbb#..", ".#bbbbbb#.", ".#bbbbbb#.", "#bbbbbbbb#", "#dddddddd#", "##########", "....##...."],
    ];
    let palettes = [[[240, 200, 80], [200, 150, 40]], [[180, 120, 70], [120, 80, 45]], [[120, 200, 210], [60, 130, 150]], [[40, 120, 130], [20, 60, 70]]];
    for (i, (_, _, culture)) in crate::archaeology::RELICS.iter().enumerate() {
        let tile = T_RELIC_FIRST + i as u16;
        let [b, d] = palettes[culture.index()];
        let shape = shapes[i % 3];
        for (y, row) in shape.iter().enumerate() {
            for (x, ch) in row.chars().enumerate() {
                let c = match ch {
                    '#' => rgb(35, 25, 20),
                    'h' => shade(rgb(b[0], b[1], b[2]), 1.25),
                    'b' => rgb(b[0], b[1], b[2]),
                    'd' => rgb(d[0], d[1], d[2]),
                    _ => continue,
                };
                a.set(tile, x + 3, y + 3, c);
            }
        }
    }
    a.each(T_ENCRUSTED, |x, y, r, p| {
        let d = ((x as f32 - 7.5).powi(2) + (y as f32 - 7.5).powi(2)).sqrt();
        if d > 6.0 + p.noise3(x as f32 * 0.6, y as f32 * 0.6, 2.0) {
            return [0, 0, 0, 0];
        }
        let glint = (x == 6 && y == 6) || (x == 9 && y == 8);
        if glint { rgb(240, 210, 120) } else { shade(rgb(120, 100, 75), r.range(0.8, 1.1)) }
    });
    a.sprite(T_MAP_FRAGMENT, &FRAGMENT_SPRITE, &[('#', rgb(90, 70, 45)), ('p', rgb(225, 205, 160)), ('r', rgb(170, 60, 40)), ('x', rgb(200, 30, 30)), ('g', rgb(90, 140, 70))]);
    a.sprite(T_COIN, &COIN_SPRITE, &[('#', rgb(90, 65, 20)), ('h', rgb(250, 225, 120)), ('b', rgb(215, 175, 60)), ('d', rgb(160, 120, 35))]);
    a.sprite(T_TABLET, &TABLET_SPRITE, &[('#', rgb(90, 55, 35)), ('b', rgb(185, 120, 80)), ('d', rgb(110, 65, 40))]);
    a.sprite(T_ECHO_SHARD, &ECHO_SPRITE, &[('#', rgb(10, 40, 50)), ('h', rgb(150, 250, 250)), ('b', rgb(40, 150, 160)), ('d', rgb(20, 90, 100))]);
    a.copy(T_COMPASS, T_RECOVERY_COMPASS);
    for (x, y) in [(6, 6), (9, 9), (6, 9), (9, 6)] {
        a.set(T_RECOVERY_COMPASS, x, y, rgb(60, 200, 210));
    }
    let dark = [('#', rgb(25, 18, 20)), ('b', rgb(80, 60, 70)), ('h', rgb(130, 105, 115)), ('d', rgb(55, 40, 48))];
    a.sprite(T_SCRAP, &COIN_SPRITE, &dark);
    a.sprite(T_SCORCHITE_INGOT, &INGOT_SPRITE, &dark);
    a.sprite(T_TEMPLATE, &TEMPLATE_SPRITE, &[('#', rgb(25, 18, 20)), ('d', rgb(70, 60, 50)), ('b', rgb(110, 95, 80)), ('h', rgb(150, 135, 115)), ('r', rgb(200, 90, 40))]);
    let pal = [('#', rgb(73, 54, 21)), ('o', rgb(137, 103, 39)), ('H', rgb(110, 90, 100)), ('h', rgb(60, 45, 55)), ('g', rgb(73, 54, 21))];
    a.sprite(T_SCORCHITE_TOOLS, &PICK, &pal);
    a.sprite(T_SCORCHITE_TOOLS + 1, &SWORD, &pal);
    a.sprite(T_SCORCHITE_TOOLS + 2, &AXE, &pal);
    a.sprite(T_SCORCHITE_TOOLS + 3, &SHOVEL, &pal);
    let base = rgb(85, 65, 75);
    let pal = [('#', shade(base, 0.3)), ('b', base), ('d', shade(base, 0.72)), ('h', shade(base, 1.3))];
    for (slot, rows) in [&HELMET, &CHESTPLATE, &LEGGINGS, &BOOTS].into_iter().enumerate() {
        a.sprite(T_SCORCHITE_ARMOR_ITEMS + slot as u16, rows, &pal);
    }
    a.each(T_SCORCHITE_ARMOR_WORN, move |x, y, r, _| {
        let rim = x == 0 || y == 0 || x == 15 || y == 15;
        let vein = (x * 5 + y * 3) % 17 == 0;
        shade(if vein { rgb(200, 90, 40) } else { base }, r.range(0.9, 1.05) * if rim { 0.7 } else { 1.0 })
    });
}

const DISC_SPRITE: [&str; 16] = [
    "................",
    ".....######.....",
    "...##dddddd##...",
    "..#ddhdddddd#d..",
    "..#dhddddddddd#.",
    ".#ddddllllddddd#",
    ".#dddllllllddd#.",
    ".#dddlll.lllddd#",
    ".#dddllllllddd#.",
    ".#ddddllllddddd#",
    "..#ddddddddddd#.",
    "..#dddddddddd#..",
    "...##dddddd##...",
    ".....######.....",
    "................",
    "................",
];

/// Note blocks, jukeboxes, discs, and the notes that float up.
fn paint_music(a: &mut Atlas) {
    // Note block: dark planks with a speaker grille in the middle.
    a.each(T_NOTE_BLOCK, |x, y, r, _| {
        let rim = x == 0 || y == 0 || x == 15 || y == 15;
        let grille = (4..12).contains(&x) && (4..12).contains(&y) && (x + y) % 2 == 0;
        let base = if rim { rgb(70, 45, 28) } else if grille { rgb(35, 22, 14) } else { rgb(110, 72, 45) };
        shade(base, r.range(0.9, 1.06) * if y % 4 == 3 { 0.9 } else { 1.0 })
    });
    a.each(T_JUKEBOX_SIDE, |x, y, r, _| {
        let frame = x <= 1 || x >= 14 || y <= 1 || y >= 14;
        let inset = (3..13).contains(&x) && (3..13).contains(&y) && (x == 3 || x == 12 || y == 3 || y == 12);
        shade(if frame { rgb(75, 48, 30) } else if inset { rgb(90, 60, 38) } else { rgb(128, 86, 56) }, r.range(0.9, 1.06))
    });
    a.each(T_JUKEBOX_TOP, |x, y, r, _| {
        let frame = x <= 1 || x >= 14 || y <= 1 || y >= 14;
        let slot = (3..13).contains(&x) && (7..9).contains(&y);
        shade(if slot { rgb(20, 14, 10) } else if frame { rgb(75, 48, 30) } else { rgb(100, 66, 42) }, r.range(0.9, 1.06))
    });
    let labels = [rgb(90, 200, 90), rgb(230, 120, 40), rgb(220, 60, 60), rgb(110, 150, 230), rgb(240, 240, 240), rgb(240, 210, 60), rgb(70, 220, 210), rgb(230, 120, 170)];
    for (i, l) in labels.into_iter().enumerate() {
        a.sprite(T_DISC_FIRST + i as u16, &DISC_SPRITE, &[('#', rgb(10, 10, 12)), ('d', rgb(30, 30, 34)), ('h', rgb(90, 90, 100)), ('l', l)]);
    }
    // Sixteen note colours, green (low) round through blue to red (high), four by four.
    a.each(T_NOTE_PARTICLE, |x, y, _, _| {
        let cell = (y / 4) * 4 + x / 4;
        let hue = 0.33 - cell as f32 / 15.0 * 1.0;
        let (lx, ly) = (x % 4, y % 4);
        if (lx == 0 && ly == 3) || (lx == 3 && ly == 0) {
            return [0, 0, 0, 0];
        }
        hsv(hue.rem_euclid(1.0), 0.85, 1.0)
    });
}

fn hsv(h: f32, s: f32, v: f32) -> Rgba {
    let i = (h * 6.0).floor();
    let f = h * 6.0 - i;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - f * s), v * (1.0 - (1.0 - f) * s));
    let (r, g, b) = match i as i32 % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

const ROD_SPRITE: [&str; 16] = [
    "................",
    "............##..",
    "...........#hh#.",
    "..........#hb#..",
    ".........#hb#...",
    "........#hb#....",
    ".......#hb#.....",
    "......#hb#......",
    ".....#hb#.......",
    "....#hb#........",
    "...#hb#.........",
    "..#hb#..........",
    ".#bb#...........",
    ".##.............",
    "................",
    "................",
];
const POWDER_SPRITE: [&str; 16] = [
    "................",
    "................",
    "................",
    "........h.......",
    "......h.bh......",
    ".....hbbbh.h....",
    "....bbhbbbb.....",
    "...hbbbbbhbb....",
    "..bbbhbbbbbbh...",
    "..bbbbbbhbbbbb..",
    ".bbbbbbbbbbbbbd.",
    ".ddbbbbbbbbbddd.",
    "..ddddddddddd...",
    "................",
    "................",
    "................",
];
const TEAR_SPRITE: [&str; 16] = [
    "................",
    ".......#........",
    "......#h#.......",
    "......#h#.......",
    ".....#hbb#......",
    ".....#hbb#......",
    "....#hbbbb#.....",
    "....#hbbbb#.....",
    "...#hbbbbbb#....",
    "...#hbbbbbb#....",
    "...#bbbbbbd#....",
    "...#bbbbbdd#....",
    "....#bbddd#.....",
    ".....#####......",
    "................",
    "................",
];
const CROSSBOW_SPRITE: [&str; 16] = [
    "................",
    ".s..............",
    "..s......####...",
    "...s....#ww#....",
    "....s..#ww#.....",
    ".....s#ww#......",
    "......#w#.......",
    ".....#w#s.......",
    "....#w#..s......",
    "...#ww#...s.....",
    "..#ww#.....s....",
    ".#ww#.......s...",
    "#ww#.........s..",
    "##............s.",
    "................",
    "................",
];
const BACKPACK_SPRITE: [&str; 16] = [
    "................",
    "......####......",
    ".....#....#.....",
    ".....#....#.....",
    "...##########...",
    "..#bbbbbbbbbb#..",
    "..#bttttttttb#..",
    "..#bbbbkkbbbb#..",
    "..#bbbbbbbbbb#..",
    "..#dbbbbbbbbd#..",
    "..#d#######bd#..",
    "..#d#bbbbb#bd#..",
    "..#d#bbbbb#bd#..",
    "..#dd#####ddd#..",
    "...##########...",
    "................",
];
const TOTEM_SPRITE: [&str; 16] = [
    "................",
    ".....######.....",
    "....#gggggg#....",
    "....#gEggEg#....",
    "....#gggggg#....",
    "...##gggggg##...",
    "..#gg#gggg#gg#..",
    "..#g#.#gg#.#g#..",
    "......#gg#......",
    ".....#gGGg#.....",
    ".....#gGGg#.....",
    "......#gg#......",
    ".....#g##g#.....",
    ".....##..##.....",
    "................",
    "................",
];
const BANNER_SPRITE: [&str; 16] = [
    "................",
    ".##############.",
    "...#wwwwwwww#...",
    "...#wwwwwwww#...",
    "...#wwkkkkww#...",
    "...#wkkddkkw#...",
    "...#wkdkkdkw#...",
    "...#wwkkkkww#...",
    "...#wwwkkwww#...",
    "...#wwwkkwww#...",
    "...#wwkkkkww#...",
    "...#wwwwwwww#...",
    "...#wwwwwwww#...",
    "....#wwwwww#....",
    ".....######.....",
    "................",
];

/// The Scorchlands' fortresses, camps and residents; raiders and their things.
fn paint_scorch_and_raids(a: &mut Atlas) {
    a.each(T_SCORCH_BRICKS, |x, y, r, _| {
        let row = y / 4;
        let mortar = y % 4 == 3 || (x + if row % 2 == 1 { 4 } else { 0 }) % 8 == 7;
        shade(if mortar { rgb(28, 12, 14) } else { rgb(70, 30, 34) }, r.range(0.88, 1.08))
    });
    a.each(T_CAGE, |x, y, r, _| {
        let bar = x % 4 == 0 || y % 4 == 0 || x == 15 || y == 15;
        if bar { shade(rgb(40, 40, 46), r.range(0.85, 1.1)) } else if (5..11).contains(&x) && (5..11).contains(&y) { shade(rgb(255, 150, 40), r.range(0.8, 1.1)) } else { [0, 0, 0, 0] }
    });
    a.each(T_SPAWNER, |x, y, r, _| {
        let bar = x % 4 == 0 || y % 4 == 0 || x == 15 || y == 15;
        if bar { shade(rgb(30, 34, 44), r.range(0.85, 1.1)) } else if (6..10).contains(&x) && (6..10).contains(&y) { shade(rgb(90, 140, 230), r.range(0.8, 1.1)) } else { [0, 0, 0, 0] }
    });
    a.each(T_GOLD_BLOCK, |x, y, r, _| {
        let rim = x == 0 || y == 0 || x == 15 || y == 15;
        let shine = (x + y) % 9 == 0 && !rim;
        shade(if rim { rgb(200, 150, 30) } else if shine { rgb(255, 250, 190) } else { rgb(250, 215, 60) }, r.range(0.92, 1.05))
    });
    a.each(T_GILDED, |x, y, r, p| {
        let n = p.noise3(x as f32 * 0.6, y as f32 * 0.6, 21.0);
        if n > 0.35 { shade(rgb(240, 200, 60), r.range(0.85, 1.1)) } else { shade(rgb(52, 40, 44), r.range(0.85, 1.12)) }
    });
    a.each(T_BELL, |x, y, r, _| {
        let band = y % 5 == 2;
        shade(if band { rgb(200, 150, 30) } else { rgb(245, 205, 70) }, r.range(0.88, 1.06) * (1.0 - x as f32 * 0.012))
    });
    a.each(T_FIREBALL, |x, y, r, _| {
        let d = ((x as f32 - 7.5).powi(2) + (y as f32 - 7.5).powi(2)).sqrt();
        if d < 3.5 { rgb(255, 250, 200) } else if d < 6.0 { shade(rgb(255, 170, 40), r.range(0.85, 1.1)) } else { shade(rgb(200, 60, 20), r.range(0.8, 1.1)) }
    });
    a.sprite(T_SIZZLE_ROD, &ROD_SPRITE, &[('#', rgb(120, 60, 10)), ('h', rgb(255, 240, 140)), ('b', rgb(250, 180, 40))]);
    a.sprite(T_SIZZLE_POWDER, &POWDER_SPRITE, &[('h', rgb(255, 240, 150)), ('b', rgb(245, 170, 40)), ('d', rgb(190, 110, 20))]);
    a.sprite(T_WEEPER_TEAR, &TEAR_SPRITE, &[('#', rgb(120, 150, 170)), ('h', rgb(255, 255, 255)), ('b', rgb(210, 235, 245)), ('d', rgb(160, 200, 220))]);
    a.sprite(T_SHROOM_STICK, &ROD, &[('#', rgb(73, 54, 21)), ('o', rgb(137, 103, 39)), ('s', rgb(230, 230, 230)), ('k', rgb(230, 90, 40))]);
    let bow = [('#', rgb(60, 40, 20)), ('w', rgb(130, 95, 55)), ('s', rgb(220, 220, 220))];
    a.sprite(T_CROSSBOW, &CROSSBOW_SPRITE, &bow);
    a.sprite(T_CROSSBOW_LOADED, &CROSSBOW_SPRITE, &bow);
    a.sprite(T_CROSSBOW_LOADED, &["................", "................", "................", "................", "................", "................", "................", "........k.......", ".......k........", "......k.........", ".....k..........", "....k..........."], &[('k', rgb(180, 180, 190))]);
    a.sprite(T_TOTEM, &TOTEM_SPRITE, &[('#', rgb(120, 90, 20)), ('g', rgb(240, 200, 60)), ('G', rgb(120, 200, 90)), ('E', rgb(20, 120, 40))]);
    a.sprite(T_BANNER, &BANNER_SPRITE, &[('#', rgb(90, 60, 30)), ('w', rgb(235, 235, 230)), ('k', rgb(40, 40, 45)), ('d', rgb(140, 30, 30))]);
    // Mob skins.
    let flat = |a: &mut Atlas, t: u16, c: Rgba, var: f32| a.each(t, move |_, _, r, _| shade(c, r.range(1.0 - var, 1.0 + var)));
    flat(a, T_SIZZLER, rgb(250, 200, 60), 0.1);
    flat(a, T_SIZZLER_ROD, rgb(255, 170, 30), 0.12);
    a.each(T_SIZZLER_FACE, |x, y, r, _| {
        let eye = (y == 6 || y == 7) && (x == 4 || x == 5 || x == 10 || x == 11);
        let mouth = y == 11 && (5..11).contains(&x);
        if eye || mouth { rgb(40, 20, 10) } else { shade(rgb(250, 200, 60), r.range(0.9, 1.1)) }
    });
    flat(a, T_WEEPER, rgb(235, 235, 235), 0.05);
    for (tile, open) in [(T_WEEPER_FACE, false), (T_WEEPER_ANGRY, true)] {
        a.each(tile, move |x, y, r, _| {
            let eyes = (x == 3 || x == 4 || x == 11 || x == 12) && if open { (5..8).contains(&y) } else { y == 6 };
            let tear = !open && (x == 4 || x == 11) && (7..14).contains(&y) && y % 2 == 1;
            let mouth = (6..10).contains(&x) && if open { (10..13).contains(&y) } else { y == 11 };
            if eyes {
                if open { rgb(220, 30, 30) } else { rgb(60, 60, 60) }
            } else if tear {
                rgb(140, 200, 240)
            } else if mouth {
                rgb(40, 40, 40)
            } else {
                shade(rgb(235, 235, 235), r.range(0.95, 1.05))
            }
        });
    }
    flat(a, T_STRUTTER, rgb(170, 50, 45), 0.1);
    flat(a, T_STRUTTER_COLD, rgb(120, 90, 160), 0.1);
    a.each(T_STRUTTER_FACE, |x, y, r, _| {
        let eye = y == 5 && (x == 4 || x == 11);
        let mouth = y == 10 && (4..12).contains(&x);
        if eye || mouth { rgb(30, 10, 10) } else { shade(rgb(170, 50, 45), r.range(0.9, 1.1)) }
    });
    flat(a, T_SNOUT, rgb(225, 150, 140), 0.06);
    a.each(T_SNOUT_TUNIC, |x, y, r, _| {
        let trim = y == 0 || y == 15 || x == 7 || x == 8;
        shade(if trim { rgb(230, 190, 50) } else { rgb(100, 70, 40) }, r.range(0.9, 1.08))
    });
    a.each(T_SNOUT_FACE, |x, y, r, _| {
        let eye = y == 6 && (x == 4 || x == 11);
        let tusk = y == 12 && (x == 3 || x == 12);
        if eye { rgb(30, 20, 20) } else if tusk { rgb(240, 240, 220) } else { shade(rgb(225, 150, 140), r.range(0.94, 1.06)) }
    });
    flat(a, T_ILLAGER, rgb(150, 155, 150), 0.06);
    a.each(T_ILLAGER_FACE, |x, y, r, _| {
        let brow = y == 5 && (3..13).contains(&x);
        let eye = y == 7 && (x == 4 || x == 5 || x == 10 || x == 11);
        let mouth = y == 12 && (5..11).contains(&x);
        if brow { rgb(40, 40, 40) } else if eye { rgb(40, 110, 60) } else if mouth { rgb(70, 60, 60) } else { shade(rgb(150, 155, 150), r.range(0.94, 1.06)) }
    });
    flat(a, T_PILFERER_COAT, rgb(85, 70, 60), 0.1);
    flat(a, T_HACKLER_COAT, rgb(55, 60, 65), 0.1);
    a.each(T_INVOICER_ROBE, |x, y, r, _| {
        let trim = x == 7 || x == 8 || y == 15;
        shade(if trim { rgb(220, 180, 50) } else { rgb(30, 30, 36) }, r.range(0.9, 1.08))
    });
    flat(a, T_FEE, rgb(190, 215, 235), 0.08);
    flat(a, T_RAMPAGER, rgb(80, 80, 78), 0.12);
    a.each(T_RAMPAGER_FACE, |x, y, r, _| {
        let eye = y == 5 && (x == 3 || x == 4 || x == 11 || x == 12);
        let mouth = (10..14).contains(&y) && (4..12).contains(&x);
        if eye { rgb(220, 40, 30) } else if mouth { rgb(30, 25, 25) } else { shade(rgb(80, 80, 78), r.range(0.88, 1.1)) }
    });
    a.each(T_BANNER_WORN, |x, y, r, _| {
        let mark = (5..11).contains(&x) && (4..12).contains(&y) && ((x as i32 - 7).abs() + (y as i32 - 8).abs()) < 4;
        shade(if mark { rgb(40, 40, 45) } else { rgb(235, 235, 230) }, r.range(0.94, 1.04))
    });
}

/// Trial Chambers, the new animals, fireworks, trims and tools.
const MACE_SPRITE: [&str; 16] = [
    "................",
    "..........####..",
    ".........#hhhh#.",
    "........#hbhhbh#",
    "........#hhbbhh#",
    "........#hbhhbh#",
    ".........#hhhh#.",
    "........#s####..",
    ".......#s#......",
    "......#s#.......",
    ".....#s#........",
    "....#s#.........",
    "...#s#..........",
    "..#s#...........",
    "..##............",
    "................",
];
const OMINOUS_BOTTLE_SPRITE: [&str; 16] = [
    "................",
    "......####......",
    "......#cc#......",
    "......#gg#......",
    ".....#gggg#.....",
    "....#gkkkkg#....",
    "...#gkkkkkkg#...",
    "...#gkkggkkg#...",
    "...#gkgkkgkg#...",
    "...#gkkggkkg#...",
    "...#gkkkkkkg#...",
    "....#gkkkkg#....",
    ".....######.....",
    "................",
    "................",
    "................",
];

fn paint_trials_and_friends(a: &mut Atlas) {
    let tuff = rgb(95, 100, 92);
    a.each(T_TUFF_BRICKS, |x, y, r, _| {
        let row = y / 4;
        let mortar = y % 4 == 3 || (x + if row % 2 == 1 { 4 } else { 0 }) % 8 == 7;
        shade(if mortar { rgb(60, 64, 58) } else { tuff }, r.range(0.88, 1.08))
    });
    a.each(T_CHISELED_TUFF, |x, y, r, _| {
        let band = !(2..=13).contains(&y);
        let carve = (5..11).contains(&x) && (5..11).contains(&y) && !((6..10).contains(&x) && (6..10).contains(&y));
        shade(if band || carve { rgb(70, 74, 68) } else { tuff }, r.range(0.9, 1.06))
    });
    a.each(T_CHISELED_TUFF_TOP, |x, y, r, _| {
        let ring = (x == 3 || x == 12 || y == 3 || y == 12) && (3..13).contains(&x) && (3..13).contains(&y);
        shade(if ring { rgb(70, 74, 68) } else { tuff }, r.range(0.9, 1.06))
    });
    a.each(T_COPPER_GRATE, |x, y, r, _| {
        let bar = x % 4 == 0 || y % 4 == 0 || x == 15 || y == 15;
        if bar { shade(rgb(200, 115, 75), r.range(0.85, 1.1)) } else { [0, 0, 0, 0] }
    });
    for (t, core) in [(T_TRIAL_SPAWNER, rgb(255, 160, 60)), (T_TRIAL_SPAWNER_SPENT, rgb(60, 60, 70))] {
        a.each(t, |x, y, r, _| {
            let bar = x % 5 == 0 || y % 5 == 0 || x == 15 || y == 15;
            if bar { shade(rgb(45, 50, 55), r.range(0.85, 1.1)) } else if (6..10).contains(&x) && (6..10).contains(&y) { shade(core, r.range(0.8, 1.1)) } else { [0, 0, 0, 0] }
        });
    }
    a.each(T_TRIAL_SPAWNER_TOP, |x, y, r, _| {
        let rim = x < 2 || y < 2 || x > 13 || y > 13;
        shade(if rim { rgb(200, 115, 75) } else { rgb(45, 50, 55) }, r.range(0.85, 1.1))
    });
    let vault_box = |x: usize, y: usize, r: &mut Rng| {
        let rim = x < 2 || y < 2 || x > 13 || y > 13;
        shade(if rim { rgb(200, 115, 75) } else { rgb(55, 60, 66) }, r.range(0.88, 1.08))
    };
    a.each(T_VAULT_FRONT, |x, y, r, _| if (6..10).contains(&x) && (5..11).contains(&y) { shade(rgb(255, 210, 90), r.range(0.85, 1.1)) } else { vault_box(x, y, r) });
    a.each(T_VAULT_OPEN, |x, y, r, _| if (6..10).contains(&x) && (5..11).contains(&y) { rgb(20, 20, 24) } else { vault_box(x, y, r) });
    a.each(T_VAULT_TOP, |x, y, r, _| vault_box(x, y, r));
    a.each(T_LODESTONE_SIDE, |x, y, r, _| {
        let stripe = (x + y) % 6 < 2;
        shade(if stripe { rgb(150, 150, 155) } else { rgb(105, 105, 110) }, r.range(0.9, 1.06))
    });
    a.each(T_LODESTONE_TOP, |x, y, r, _| {
        let d = ((x as f32 - 7.5).powi(2) + (y as f32 - 7.5).powi(2)).sqrt();
        shade(if d < 3.0 { rgb(70, 70, 75) } else if d < 4.5 { rgb(200, 200, 205) } else { rgb(110, 110, 115) }, r.range(0.9, 1.06))
    });
    a.sprite(T_TRIAL_KEY, &KEY_SPRITE, &[('#', rgb(70, 40, 20)), ('h', rgb(255, 220, 120)), ('b', rgb(220, 160, 60))]);
    a.sprite(T_OMINOUS_KEY, &KEY_SPRITE, &[('#', rgb(25, 30, 45)), ('h', rgb(120, 220, 230)), ('b', rgb(60, 130, 160))]);
    a.each(T_OMINOUS_SPAWNER, |x, y, r, _| {
        let bar = x % 5 == 0 || y % 5 == 0 || x == 15 || y == 15;
        if bar { shade(rgb(35, 40, 55), r.range(0.85, 1.1)) } else if (6..10).contains(&x) && (6..10).contains(&y) { shade(rgb(90, 200, 230), r.range(0.8, 1.1)) } else { [0, 0, 0, 0] }
    });
    a.each(T_VAULT_OMINOUS, |x, y, r, _| {
        let rim = x < 2 || y < 2 || x > 13 || y > 13;
        if (6..10).contains(&x) && (5..11).contains(&y) {
            shade(rgb(110, 220, 235), r.range(0.85, 1.1))
        } else {
            shade(if rim { rgb(60, 95, 120) } else { rgb(40, 44, 55) }, r.range(0.88, 1.08))
        }
    });
    a.each(T_HEAVY_CORE, |x, y, r, _| {
        let d = (x as f32 - 7.5).abs().max((y as f32 - 7.5).abs());
        shade(if d < 3.0 { rgb(80, 85, 95) } else if d < 5.0 { rgb(45, 48, 55) } else { rgb(120, 125, 135) }, r.range(0.85, 1.1))
    });
    a.sprite(T_MACE, &MACE_SPRITE, &[('#', rgb(30, 30, 35)), ('h', rgb(170, 175, 185)), ('b', rgb(95, 100, 110)), ('s', rgb(150, 170, 230))]);
    a.sprite(T_OMINOUS_BOTTLE, &OMINOUS_BOTTLE_SPRITE, &[('#', rgb(25, 25, 30)), ('g', rgb(200, 220, 230)), ('k', rgb(60, 130, 160)), ('c', rgb(140, 100, 60))]);
    a.sprite(T_WIND_CHARGE, &ORB_SPRITE, &[('#', rgb(90, 110, 160)), ('h', rgb(240, 245, 255)), ('b', rgb(180, 200, 245)), ('d', rgb(130, 150, 210))]);
    a.sprite(T_BREEZE_ROD, &ROD_SPRITE, &[('#', rgb(60, 70, 110)), ('h', rgb(220, 230, 255)), ('b', rgb(150, 170, 230))]);
    a.sprite(T_GOAT_HORN, &HORN_SPRITE, &[('#', rgb(70, 60, 45)), ('h', rgb(240, 230, 205)), ('b', rgb(200, 185, 150))]);
    a.sprite(T_TRIM_TEMPLATE, &TRIM_SPRITE, &[('#', rgb(40, 40, 45)), ('h', rgb(230, 230, 235)), ('b', rgb(150, 150, 160)), ('g', rgb(80, 180, 160))]);
    a.sprite(T_SPYGLASS, &SPYGLASS_SPRITE, &[('#', rgb(70, 40, 20)), ('c', rgb(205, 120, 80)), ('d', rgb(150, 80, 50)), ('g', rgb(200, 230, 255))]);
    a.sprite(T_BUNDLE, &BUNDLE_SPRITE, &[('#', rgb(70, 45, 25)), ('h', rgb(210, 160, 100)), ('b', rgb(165, 115, 65)), ('s', rgb(230, 220, 200))]);
    // Mob skins.
    a.each(T_BREEZE, |x, y, r, _| {
        let swirl = (x + y * 2) % 7 < 2;
        shade(if swirl { rgb(220, 230, 255) } else { rgb(150, 170, 230) }, r.range(0.85, 1.1))
    });
    a.each(T_BREEZE_FACE, |x, y, r, _| {
        let eye = (3..6).contains(&x) && (5..8).contains(&y) || (10..13).contains(&x) && (5..8).contains(&y);
        if eye { rgb(30, 40, 80) } else { shade(rgb(150, 170, 230), r.range(0.9, 1.08)) }
    });
    a.speckle(T_GOAT, rgb(225, 220, 205), 0.08);
    a.each(T_GOAT_FACE, |x, y, r, _| {
        let eye = (y == 6 || y == 7) && (x == 3 || x == 4 || x == 11 || x == 12);
        let beard = y > 12 && (6..10).contains(&x);
        if eye { rgb(60, 45, 20) } else if beard { rgb(250, 248, 240) } else { shade(rgb(225, 220, 205), r.range(0.9, 1.06)) }
    });
    a.speckle(T_AXOLOTL, rgb(240, 150, 190), 0.06);
    a.each(T_AXOLOTL_FACE, |x, y, r, _| {
        let eye = (y == 5 || y == 6) && (x == 2 || x == 13);
        let mouth = y == 11 && (5..11).contains(&x);
        if eye { rgb(20, 20, 20) } else if mouth { rgb(170, 70, 110) } else { shade(rgb(240, 150, 190), r.range(0.92, 1.06)) }
    });
    a.speckle(T_AXOLOTL_GILL, rgb(210, 60, 120), 0.1);
    a.speckle(T_CAMEL, rgb(210, 170, 110), 0.07);
    a.each(T_CAMEL_FACE, |x, y, r, _| {
        let eye = (y == 4 || y == 5) && (x == 3 || x == 12);
        let nose = y > 11 && (x == 5 || x == 10);
        if eye || nose { rgb(50, 35, 20) } else { shade(rgb(210, 170, 110), r.range(0.92, 1.06)) }
    });
    a.speckle(T_CAMEL_HUMP, rgb(190, 145, 90), 0.08);
    // Firework sparks: a bright dot, one colour each.
    let colours = [rgb(255, 70, 60), rgb(255, 170, 40), rgb(255, 240, 80), rgb(80, 230, 90), rgb(70, 200, 255), rgb(90, 110, 255), rgb(220, 90, 255), rgb(255, 255, 255)];
    for (i, c) in colours.into_iter().enumerate() {
        a.each(T_SPARK_FIRST + i as u16, |x, y, _, _| {
            let d = ((x as f32 - 7.5).powi(2) + (y as f32 - 7.5).powi(2)).sqrt();
            if d < 3.0 { rgb(255, 255, 255) } else if d < 7.5 { c } else { [0, 0, 0, 0] }
        });
    }
    // Trim colours: iron, gold, diamond, copper, emerald-ish green (Zappy red too).
    for (i, c) in [rgb(215, 215, 220), rgb(250, 210, 60), rgb(100, 230, 220), rgb(220, 120, 80), rgb(80, 200, 110), rgb(220, 40, 40)].into_iter().enumerate() {
        a.speckle(T_TRIM_FIRST + i as u16, c, 0.06);
    }
}

const KEY_SPRITE: [&str; 16] = [
    "................",
    "................",
    "....####........",
    "...#hhhh#.......",
    "..#hb##bh#......",
    "..#h#..#h#......",
    "..#hb##bh#......",
    "...#hbbb#####...",
    "....####hhhhh#..",
    ".........#b#b#..",
    ".........#b#b#..",
    "..........#.#...",
    "................",
    "................",
    "................",
    "................",
];
const ORB_SPRITE: [&str; 16] = [
    "................",
    "................",
    "......####......",
    "....##hhbb##....",
    "...#hhbbbbdd#...",
    "...#hbbhbbbd#...",
    "..#hbbhhbbbdd#..",
    "..#hbbbhbbbdd#..",
    "..#bbbbbhbbdd#..",
    "..#bbbbbbhddd#..",
    "...#bbbbbbdd#...",
    "...#dbbbbddd#...",
    "....##dddd##....",
    "......####......",
    "................",
    "................",
];
const HORN_SPRITE: [&str; 16] = [
    "................",
    "................",
    "............##..",
    "...........#hh#.",
    "..........#hb#..",
    ".........#hb#...",
    "........#hhb#...",
    ".......#hhb#....",
    "......#hhbb#....",
    ".....#hhbb#.....",
    "....#hhbb#......",
    "...#hhbbb#......",
    "..#hhbbb#.......",
    "..#bbbb#........",
    "...####.........",
    "................",
];
const TRIM_SPRITE: [&str; 16] = [
    "................",
    "................",
    "...##########...",
    "...#hhhhhhhh#...",
    "...#hbbbbbbh#...",
    "...#hbggggbh#...",
    "...#hbgbbgbh#...",
    "...#hbggggbh#...",
    "...#hbbbbbbh#...",
    "...#hbggggbh#...",
    "...#hbgbbgbh#...",
    "...#hbbbbbbh#...",
    "...#hhhhhhhh#...",
    "...##########...",
    "................",
    "................",
];
const SPYGLASS_SPRITE: [&str; 16] = [
    "................",
    "..........###...",
    ".........#ggg#..",
    "........#cgggc#.",
    ".......#ccgccc#.",
    "......#cccccd#..",
    ".....#cccccd#...",
    "....#dcccd##....",
    "...#ddccd#......",
    "..#dddcd#.......",
    ".#dddd#.........",
    ".#ddd#..........",
    "..###...........",
    "................",
    "................",
    "................",
];
const BUNDLE_SPRITE: [&str; 16] = [
    "................",
    "................",
    "......#ss#......",
    ".....#s##s#.....",
    "......####......",
    ".....#hhhh#.....",
    "....#hhhhhh#....",
    "...#hhbhhbhh#...",
    "..#hhhhhhhhhh#..",
    "..#hbhhhhhhbh#..",
    "..#hhhhhhhhhh#..",
    "..#bhhhhhhhhb#..",
    "...#bbhhhhbb#...",
    "....##bbbb##....",
    "......####......",
    "................",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_tiles_have_a_border_of_their_own_edge() {
        let atlas = build_atlas(1);
        let ui = ui_atlas(&atlas);
        assert_eq!(ui.len(), UI_ATLAS * UI_ATLAS * 4);
        let px = |buf: &[u8], w: usize, x: usize, y: usize| buf[(y * w + x) * 4..(y * w + x) * 4 + 4].to_vec();
        for tile in [T_HUNGER_ICON, T_HEART, T_WATER, T_GRASS_SIDE] {
            let (tx, ty) = ((tile % TILES_PER_ROW) as usize, (tile / TILES_PER_ROW) as usize);
            let (ax, ay, ux, uy) = (tx * TILE, ty * TILE, tx * UI_CELL + 1, ty * UI_CELL + 1);
            for i in 0..TILE {
                // The tile itself, unchanged...
                assert_eq!(px(&ui, UI_ATLAS, ux + i, uy + 3), px(&atlas, ATLAS, ax + i, ay + 3));
                // ...and each border pixel repeats the edge next to it, not the neighbouring tile.
                assert_eq!(px(&ui, UI_ATLAS, ux + TILE, uy + i), px(&atlas, ATLAS, ax + TILE - 1, ay + i), "{tile} right");
                assert_eq!(px(&ui, UI_ATLAS, ux - 1, uy + i), px(&atlas, ATLAS, ax, ay + i), "{tile} left");
                assert_eq!(px(&ui, UI_ATLAS, ux + i, uy + TILE), px(&atlas, ATLAS, ax + i, ay + TILE - 1), "{tile} bottom");
            }
            let (u, v, s) = ui_tile_uv(tile);
            assert_eq!(((u * UI_ATLAS as f32).round() as usize, (v * UI_ATLAS as f32).round() as usize, (s * UI_ATLAS as f32).round() as usize), (ux, uy, TILE));
        }
    }

    #[test]
    fn solid_blocks_have_no_see_through_pixels() {
        // An opaque cube with clear texels shows the world through its sides.
        let atlas = build_atlas(1);
        let size = TILES_PER_ROW as usize * 16;
        let mut bad = Vec::new();
        for id in 0..crate::block::NUM_BLOCKS {
            let b = crate::block::block(id);
            if !b.opaque || b.model != crate::block::Model::Cube {
                continue;
            }
            for &t in &b.tex {
                let (tx, ty) = ((t % TILES_PER_ROW) as usize, (t / TILES_PER_ROW) as usize);
                let clear = (0..16).any(|y| (0..16).any(|x| atlas[((ty * 16 + y) * size + tx * 16 + x) * 4 + 3] < 255));
                if clear {
                    bad.push(b.key);
                }
            }
        }
        bad.dedup();
        assert!(bad.is_empty(), "solid blocks with see-through pixels: {bad:?}");
    }

    #[test]
    fn plant_mipmaps_keep_their_shape_and_colour() {
        let atlas = build_atlas(1);
        let levels = mip_levels(&atlas);
        assert_eq!(levels.len(), 4, "down to one texel a tile");
        let solid = |px: &[u8], size: usize, tile: usize| {
            let (tx, ty) = ((T_TALLGRASS % TILES_PER_ROW) as usize, (T_TALLGRASS / TILES_PER_ROW) as usize);
            let mut n = 0;
            for y in 0..tile {
                for x in 0..tile {
                    let i = ((ty * tile + y) * size + tx * tile + x) * 4;
                    assert!(px[i + 3] == 0 || px[i + 3] == 255, "cut-out stays clear or solid");
                    if px[i + 3] == 255 {
                        n += 1;
                        assert!(px[i + 1] > 60, "visible grass stays green, not darkened by clear texels");
                    }
                }
            }
            n as f32 / (tile * tile) as f32
        };
        let full = solid(&atlas, ATLAS, TILE);
        for (l, px) in levels.iter().enumerate().take(2) {
            let share = solid(px, ATLAS >> (l + 1), TILE >> (l + 1));
            assert!((share - full).abs() < 0.1, "level {}: {share} solid vs {full}", l + 1);
        }
    }
}
