//! Procedurally painted 16x16 texture atlas. Zero image files ship with the game:
//! every pixel below is computed from noise, hashes or tiny ASCII sprites.

use crate::noise::{Perlin, Rng};

pub const ATLAS: usize = 256;
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
