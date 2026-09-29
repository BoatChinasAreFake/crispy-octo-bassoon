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
// Crop tiles are four in a row: T_CROP_* + stage.

/// Mod textures are allocated from here to the end of the atlas (the base game
/// keeps the first 512 tiles; mods look textures up by name, so this can move).
pub const FIRST_MOD_TILE: u16 = 512;

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
            let cx = self.rng.int(2, 13) as i32;
            let cy = self.rng.int(2, 13) as i32;
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
    a.copy(T_PLANKS, T_BED_SIDE);
    for y in 0..9 {
        for x in 0..16 {
            let c = if y < 3 { shade(rgb(240, 240, 240), a.rng.range(0.94, 1.02)) } else { shade(rgb(180, 30, 35), a.rng.range(0.88, 1.05)) };
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
    for (x, c) in [(3usize, rgb(20, 20, 20)), (12, rgb(20, 20, 20))] {
        a.set(T_MOO_FACE, x, 5, c);
        a.set(T_MOO_FACE, x, 6, c);
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
                rgb(170, 210, 230)
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
        let cap = (4..9).contains(&y) && (2..14).contains(&x) && !(y == 4 && (x < 4 || x > 11));
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
        // A narrow cone, point at the top (flipped by nothing: good enough both ways up).
        let half = (y as f32 + 1.0) * 0.28;
        if (x as f32 - 7.5).abs() <= half { shade(rgb(125, 115, 105), r.range(0.8, 1.1)) } else { [0, 0, 0, 0] }
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
            let handle = y < 12 && y >= 2 && (x as f32 - hx).abs() < 1.0;
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
    a.speckle(T_WOOF_SKIN, rgb(200, 196, 190), 0.07);
    a.copy(T_WOOF_SKIN, T_WOOF_FACE);
    for (x, y, c) in [(3, 5, rgb(20, 20, 20)), (4, 5, rgb(255, 255, 255)), (11, 5, rgb(255, 255, 255)), (12, 5, rgb(20, 20, 20))] {
        a.set(T_WOOF_FACE, x, y, c);
    }
    for y in 9..12 {
        for x in 6..10 {
            a.set(T_WOOF_FACE, x, y, if y == 9 { rgb(40, 30, 30) } else { rgb(170, 165, 160) });
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

    a.px
}
