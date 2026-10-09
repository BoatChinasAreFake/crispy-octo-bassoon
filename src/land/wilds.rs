//! The Scorchlands' biomes: it isn't all Scorchrock and lava any more.
//!
//! - **Crimson Forests**: red Nylium underfoot, huge Crimson Fungi (stems
//!   capped with wart and the odd glowing Shroomlight), roots, fungi and
//!   Weeping Vines hanging from everything. Red spores drift about.
//! - **Teal Forests**: the same, but teal, and calmer. Teal spores.
//! - **Basalt Deltas**: Basalt and Blackstone, basalt pillars, pools of lava
//!   on the floor and Magma Blocks that burn your feet (sneak across them).
//!   Thick grey ash in the air.
//! - **Soul Sand Valleys**: Sorrow Sand and Soul Soil, great fossil ribs of
//!   Bone Blocks, and blue Soul Fire that burns for ever. Pale blue haze.
//!
//! The old Scorchrock caverns ("the Wastes") fill the gaps between them.
//! Bone Dust on a fungus standing on its own Nylium grows a huge one.
//! Biomes are a pure function of the seed (like the surface's), so the
//! haze, the motes and the advancements for finding them need no syncing.

use crate::block::*;
use crate::game::Game;
use crate::noise::{hash2, hash3};
use crate::scorch::{LAVA_SEA, SCORCH_X};
use crate::sound::{Mat, Sfx};
use crate::texture::*;
use crate::world::{idx, Generator, World, CH, CW};
use macroquad::math::{ivec3, IVec3, Vec3};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ScorchBiome {
    /// Bare Scorchrock caverns over the lava sea (how it all used to be).
    Wastes,
    CrimsonForest,
    TealForest,
    BasaltDeltas,
    SoulValley,
}

impl ScorchBiome {
    pub fn name(self) -> &'static str {
        match self {
            ScorchBiome::Wastes => "Scorched Wastes (Toasty)",
            ScorchBiome::CrimsonForest => "Crimson Forest (Seeing Red)",
            ScorchBiome::TealForest => "Teal Forest (Oddly Calming)",
            ScorchBiome::BasaltDeltas => "Basalt Deltas (Ashes to Ashes)",
            ScorchBiome::SoulValley => "Soul Sand Valley (Sighs a Lot)",
        }
    }

    /// For /locate: "crimson", "teal_forest", "basalt", "soul"...
    pub fn from_name(s: &str) -> Option<ScorchBiome> {
        let s = s.trim().to_ascii_lowercase().replace([' ', '-'], "_");
        Some(match s.as_str() {
            "wastes" | "scorched_wastes" | "nether_wastes" => ScorchBiome::Wastes,
            "crimson" | "crimson_forest" => ScorchBiome::CrimsonForest,
            "teal" | "teal_forest" | "warped" | "warped_forest" => ScorchBiome::TealForest,
            "basalt" | "basalt_deltas" | "deltas" => ScorchBiome::BasaltDeltas,
            "soul" | "soul_valley" | "soul_sand_valley" => ScorchBiome::SoulValley,
            _ => return None,
        })
    }

    /// The fungus forests.
    pub fn forest(self) -> bool {
        matches!(self, ScorchBiome::CrimsonForest | ScorchBiome::TealForest)
    }

    /// The air's colour (the fog and the sky), and how close the fog comes (1: as usual).
    pub fn haze(self) -> ([f32; 3], f32) {
        match self {
            ScorchBiome::Wastes => ([0.24, 0.06, 0.03], 1.0),
            ScorchBiome::CrimsonForest => ([0.3, 0.04, 0.05], 0.95),
            ScorchBiome::TealForest => ([0.05, 0.15, 0.16], 1.0),
            ScorchBiome::BasaltDeltas => ([0.33, 0.31, 0.35], 0.55),
            ScorchBiome::SoulValley => ([0.1, 0.18, 0.2], 0.85),
        }
    }

    /// Bits of whatever drifts in the air here (a tile to cut them from), and how many.
    fn motes(self) -> Option<(u16, usize)> {
        match self {
            ScorchBiome::Wastes => None,
            ScorchBiome::CrimsonForest => Some((T_CRIMSON_WART, 3)),
            ScorchBiome::TealForest => Some((T_TEAL_NYLIUM_TOP, 3)),
            ScorchBiome::BasaltDeltas => Some((T_BONE_BLOCK_SIDE, 8)),
            ScorchBiome::SoulValley => Some((T_SOUL_FIRE, 2)),
        }
    }
}

/// The blocks a fungus forest is made of: (nylium, stem, wart, fungus, roots).
pub fn forest_blocks(teal: bool) -> (Id, Id, Id, Id, Id) {
    if teal {
        (TEAL_NYLIUM, TEAL_STEM, TEAL_WART, TEAL_FUNGUS, TEAL_ROOTS)
    } else {
        (CRIMSON_NYLIUM, CRIMSON_STEM, CRIMSON_WART, CRIMSON_FUNGUS, CRIMSON_ROOTS)
    }
}

pub fn is_nylium(id: Id) -> bool {
    matches!(id, CRIMSON_NYLIUM | TEAL_NYLIUM)
}

pub fn is_fungus(id: Id) -> bool {
    matches!(id, CRIMSON_FUNGUS | TEAL_FUNGUS)
}

/// Soul Fire burns on these (and for ever).
pub fn is_soul_ground(id: Id) -> bool {
    matches!(id, SOUL_SOIL | SORROW_SAND)
}

/// Is a huge fungus's cap allowed to grow into this cell?
fn open(id: Id) -> bool {
    id == AIR || matches!(id, CRIMSON_ROOTS | TEAL_ROOTS | CRIMSON_FUNGUS | TEAL_FUNGUS | WEEPING_VINES | EMBER_SHROOM)
}

/// A huge fungus standing on `base` (the cell above its nylium): a stem
/// `height` tall, a cap of wart with Shroomlights in it, and (crimson ones)
/// vines hanging off the cap's rim.
pub fn huge_fungus(base: IVec3, height: i32, teal: bool, seed: u32) -> Vec<(IVec3, Id)> {
    let (_, stem, wart, _, _) = forest_blocks(teal);
    let mut out = Vec::new();
    for y in 0..height {
        out.push((base + IVec3::Y * y, stem));
    }
    let top = base.y + height;
    let r: i32 = if height >= 7 { 3 } else { 2 };
    for dy in -2..=1 {
        let rad = if dy == 1 { r - 1 } else { r };
        for dz in -rad..=rad {
            for dx in -rad..=rad {
                let rim = dx.abs() == rad || dz.abs() == rad;
                let corner = dx.abs() == rad && dz.abs() == rad;
                let p = ivec3(base.x + dx, top + dy, base.z + dz);
                let h = hash3(seed, p.x, p.y, p.z);
                // A lid, a full layer under it, then a ragged skirt.
                let here = match dy {
                    1 => !corner || h < 0.5,
                    0 => !corner,
                    -1 => rim && (!corner || h < 0.4),
                    _ => rim && !corner && h < 0.55,
                };
                if !here || (dx == 0 && dz == 0 && dy < 1) {
                    continue;
                }
                out.push((p, if hash3(seed ^ 0x5B, p.x, p.y, p.z) < 0.08 { SHROOMLIGHT } else { wart }));
                // Crimson caps drip vines from the bottom of the skirt.
                if !teal && dy == -2 && h < 0.3 {
                    let n = 1 + (hash3(seed ^ 0x77, p.x, p.y, p.z) * 3.0) as i32;
                    for k in 1..=n {
                        out.push((p - IVec3::Y * k, WEEPING_VINES));
                    }
                }
            }
        }
    }
    out
}

impl Generator {
    /// Which of the Scorchlands' biomes a column is in.
    pub fn scorch_biome(&self, x: i32, z: i32) -> ScorchBiome {
        // A block down here is eight up top, so biomes are smaller to match.
        let bs = self.opts.biome_scale().sqrt();
        let (fx, fz) = (x as f32 / (110.0 * bs), z as f32 / (110.0 * bs));
        let heat = self.temp.fbm2(fx + 913.0, fz - 407.0, 3);
        let damp = self.moist.fbm2(fx - 655.0, fz + 821.0, 3);
        if heat > 0.1 {
            if damp > 0.0 { ScorchBiome::CrimsonForest } else { ScorchBiome::BasaltDeltas }
        } else if heat < -0.1 {
            if damp > 0.0 { ScorchBiome::TealForest } else { ScorchBiome::SoulValley }
        } else {
            ScorchBiome::Wastes
        }
    }

    /// Solid rock (before anything else is done to it) at this spot of the Scorchlands' caverns?
    pub fn scorch_solid(&self, x: i32, y: i32, z: i32) -> bool {
        let n = self.scorch.noise3(x as f32 / 38.0, y as f32 / 26.0, z as f32 / 38.0) + self.scorch.noise3(x as f32 / 13.0, y as f32 / 10.0, z as f32 / 13.0) * 0.3;
        let edge = ((y - 64) as f32 / 58.0).powi(2);
        n + edge * 1.1 > 0.18
    }

    /// The first floor at or above `from` with `room` clear blocks over it (the cell above the floor).
    pub fn scorch_floor(&self, x: i32, z: i32, from: i32, room: i32) -> Option<i32> {
        let mut y = from.max(LAVA_SEA + 2);
        while y < CH - 12 - room {
            if self.scorch_solid(x, y - 1, z) && !self.scorch_solid(x, y, z) {
                match (1..room).find(|k| self.scorch_solid(x, y + k, z)) {
                    None => return Some(y),
                    Some(k) => y += k + 1,
                }
            } else {
                y += 1;
            }
        }
        None
    }

    /// A huge fungus growing from this column: where it stands, how tall, and which kind.
    pub fn fungus_at(&self, x: i32, z: i32) -> Option<(IVec3, i32, bool)> {
        let s = self.seed ^ 0xF0F;
        if x < SCORCH_X + 20 || hash2(s, x, z) >= 0.045 {
            return None;
        }
        let biome = self.scorch_biome(x, z);
        if !biome.forest() {
            return None;
        }
        let from = LAVA_SEA + 2 + (hash2(s ^ 1, x, z) * 40.0) as i32;
        let y = self.scorch_floor(x, z, from, 10)?;
        let height = 4 + (hash2(s ^ 2, x, z) * 5.0) as i32;
        Some((ivec3(x, y, z), height, biome == ScorchBiome::TealForest))
    }

    /// Dress a freshly made chunk of the Scorchlands for its biomes.
    pub fn decorate_scorch(&self, cx: i32, cz: i32, b: &mut [Id]) {
        let s = self.seed ^ 0x5C0_4C4;
        let solid_rock = |id: Id| matches!(id, SCORCHROCK | EMBERSAND | SORROW_SAND);
        let mut any_forest = false;
        for lz in 0..CW {
            for lx in 0..CW {
                let (x, z) = (cx * CW + lx, cz * CW + lz);
                if x < SCORCH_X + 16 {
                    continue;
                }
                let biome = self.scorch_biome(x, z);
                match biome {
                    ScorchBiome::Wastes => {}
                    ScorchBiome::CrimsonForest | ScorchBiome::TealForest => {
                        any_forest = true;
                        let teal = biome == ScorchBiome::TealForest;
                        let (nylium, _, _, fungus, roots) = forest_blocks(teal);
                        for y in LAVA_SEA + 1..CH - 5 {
                            let i = idx(lx, y, lz);
                            if b[i] == EMBER_SHROOM {
                                b[i] = AIR;
                            }
                            let j = idx(lx, y - 1, lz);
                            if b[i] == AIR && solid_rock(b[j]) {
                                b[j] = nylium;
                                let r = hash3(s ^ 0x31, x, y, z);
                                b[i] = if r < 0.1 {
                                    roots
                                } else if r < 0.135 {
                                    fungus
                                } else {
                                    AIR
                                };
                            }
                            // Vines hang from the crimson ceilings.
                            let above = b[idx(lx, y + 1, lz)];
                            if !teal && b[i] == AIR && above == SCORCHROCK && hash3(s ^ 0x32, x, y, z) < 0.035 {
                                let n = 1 + (hash3(s ^ 0x33, x, y, z) * 5.0) as i32;
                                for k in 0..n {
                                    let c = idx(lx, y - k, lz);
                                    if y - k <= LAVA_SEA || b[c] != AIR {
                                        break;
                                    }
                                    b[c] = WEEPING_VINES;
                                }
                            }
                        }
                    }
                    ScorchBiome::BasaltDeltas => {
                        for y in 1..CH - 1 {
                            let i = idx(lx, y, lz);
                            if solid_rock(b[i]) {
                                let n = self.scorch.noise3(x as f32 / 7.0, y as f32 / 5.0 + 300.0, z as f32 / 7.0);
                                b[i] = if n > 0.0 { BASALT } else { BLACKSTONE };
                            } else if b[i] == EMBER_SHROOM {
                                b[i] = AIR;
                            }
                        }
                        for y in LAVA_SEA + 2..CH - 5 {
                            let (i, j) = (idx(lx, y, lz), idx(lx, y - 1, lz));
                            if b[i] != AIR || !matches!(b[j], BASALT | BLACKSTONE) {
                                continue;
                            }
                            // Pools of lava in dips in the floor (walled in, so they stay put).
                            let pool = self.scorch.noise3(x as f32 / 6.0, 91.0, z as f32 / 6.0) > 0.3;
                            let walled = (1..CW - 1).contains(&lx) && (1..CW - 1).contains(&lz) && [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().all(|&(dx, dz)| is_solid(b[idx(lx + dx, y - 1, lz + dz)])) && is_solid(b[idx(lx, y - 2, lz)]);
                            if pool && walled {
                                b[j] = LAVA;
                            } else if hash3(s ^ 0x41, x, y, z) < 0.05 {
                                b[j] = MAGMA_BLOCK;
                            }
                        }
                        // Basalt pillars.
                        if hash2(s ^ 0xBA5, x, z) < 0.012
                            && let Some(y) = self.scorch_floor(x, z, LAVA_SEA + 2 + (hash2(s ^ 0xBA6, x, z) * 30.0) as i32, 3)
                        {
                            let h = 3 + (hash2(s ^ 0xBA7, x, z) * 8.0) as i32;
                            for k in 0..h {
                                let c = idx(lx, y + k, lz);
                                if y + k >= CH - 2 || b[c] != AIR {
                                    break;
                                }
                                b[c] = BASALT;
                            }
                        }
                    }
                    ScorchBiome::SoulValley => {
                        for y in LAVA_SEA + 1..CH - 5 {
                            let i = idx(lx, y, lz);
                            if b[i] == EMBER_SHROOM {
                                b[i] = AIR;
                            }
                            if b[i] != AIR || !solid_rock(b[idx(lx, y - 1, lz)]) {
                                continue;
                            }
                            // A few blocks deep of Sorrow Sand and Soul Soil, in patches.
                            let sandy = self.scorch.noise3(x as f32 / 9.0, 55.0, z as f32 / 9.0) > 0.0;
                            for k in 1..=3 {
                                let c = idx(lx, y - k, lz);
                                if solid_rock(b[c]) {
                                    b[c] = if sandy { SORROW_SAND } else { SOUL_SOIL };
                                }
                            }
                            if !sandy && hash3(s ^ 0x51, x, y, z) < 0.01 {
                                b[i] = SOUL_FIRE;
                            }
                        }
                    }
                }
            }
        }
        // The ribs of something very big and very dead.
        let (mx, mz) = (cx * CW + 8, cz * CW + 8);
        if mx >= SCORCH_X + 24 && self.scorch_biome(mx, mz) == ScorchBiome::SoulValley && hash2(s ^ 0xF055, cx, cz) < 0.3
            && let Some(y) = self.scorch_floor(mx, mz, LAVA_SEA + 2 + (hash2(s ^ 0xF056, cx, cz) * 30.0) as i32, 7)
        {
            for (p, id) in fossil(ivec3(8, y, 8), hash2(s ^ 0xF057, cx, cz) < 0.5) {
                let c = idx(p.x, p.y, p.z);
                if (0..CW).contains(&p.x) && (0..CW).contains(&p.z) && (1..CH - 1).contains(&p.y) && b[c] == AIR {
                    b[c] = id;
                }
            }
        }
        // Huge fungi, including ones from the next chunk over whose caps reach in.
        if !any_forest && !(-1..=1).any(|dz| (-1..=1).any(|dx| self.scorch_biome(cx * CW + 8 + dx * CW, cz * CW + 8 + dz * CW).forest())) {
            return;
        }
        for z in cz * CW - 3..cz * CW + CW + 3 {
            for x in cx * CW - 3..cx * CW + CW + 3 {
                let Some((base, height, teal)) = self.fungus_at(x, z) else { continue };
                let stem = forest_blocks(teal).1;
                for (p, id) in huge_fungus(base, height, teal, self.seed ^ x as u32 ^ (z as u32).rotate_left(16)) {
                    let (lx, lz) = (p.x - cx * CW, p.z - cz * CW);
                    if !(0..CW).contains(&lx) || !(0..CW).contains(&lz) || !(1..CH - 1).contains(&p.y) {
                        continue;
                    }
                    let c = idx(lx, p.y, lz);
                    if (id == stem && b[c] != BEDROCK) || open(b[c]) {
                        b[c] = id;
                    }
                }
            }
        }
    }
}

/// A fossil: a spine with ribs arching down either side (local to the chunk, around `mid`).
fn fossil(mid: IVec3, along_x: bool) -> Vec<(IVec3, Id)> {
    let mut out = Vec::new();
    let at = |a: i32, h: i32, c: i32| if along_x { mid + ivec3(a, h, c) } else { mid + ivec3(c, h, a) };
    for a in -4..=4 {
        out.push((at(a, 5, 0), BONE_BLOCK));
        if a % 2 == 0 && a.abs() < 4 {
            for (c, h) in [(1, 5), (2, 4), (3, 3), (3, 2), (3, 1), (2, 0)] {
                out.push((at(a, h, c), BONE_BLOCK));
                out.push((at(a, h, -c), BONE_BLOCK));
            }
        }
    }
    out
}

/// Block definitions (appended to the registry, in id order from CRIMSON_NYLIUM).
pub fn defs() -> Vec<BlockDef> {
    use Model::{Cross, Cube};
    let mut v = Vec::new();
    for teal in [false, true] {
        let (k, n, t) = if teal { ("teal", "Teal", [T_TEAL_NYLIUM_TOP, T_TEAL_NYLIUM_SIDE, T_TEAL_STEM_SIDE, T_TEAL_STEM_TOP, T_TEAL_WART, T_TEAL_FUNGUS, T_TEAL_ROOTS]) } else { ("crimson", "Crimson", [T_CRIMSON_NYLIUM_TOP, T_CRIMSON_NYLIUM_SIDE, T_CRIMSON_STEM_SIDE, T_CRIMSON_STEM_TOP, T_CRIMSON_WART, T_CRIMSON_FUNGUS, T_CRIMSON_ROOTS]) };
        let (_, stem, wart, fungus, roots) = forest_blocks(teal);
        let leak = |s: String| -> &'static str { Box::leak(s.into_boxed_str()) };
        // (Nylium is Scorchrock with a coat on: it drops the rock.)
        v.push(def(leak(format!("{k}_nylium")), leak(format!("{n} Nylium (Fuzzy Rock)")), Cube, true, true, [t[0], t[1], T_SCORCHROCK], 0.4, 1, true, SCORCHROCK, 0.0, S_STONE));
        v.push(def(leak(format!("{k}_stem")), leak(format!("{n} Stem (A Mushroom's Idea of a Tree)")), Cube, true, true, [t[3], t[2], t[3]], 2.0, 0, false, stem, 0.0, S_WOOD));
        v.push(def(leak(format!("{k}_wart_block")), leak(format!("{n} Wart Block (Squishy)")), Cube, true, true, [t[4]; 3], 1.0, 0, false, wart, 0.0, S_GRASS));
        v.push(def(leak(format!("{k}_fungus")), leak(format!("{n} Fungus ({})", if teal { "Tuskers Hate It" } else { "Tuskers Love It" })), Cross, false, false, [t[5]; 3], 0.0, 0, false, fungus, 0.0, S_GRASS));
        v.push(def(leak(format!("{k}_roots")), leak(format!("{n} Roots (Rootin' Tootin')")), Cross, false, false, [t[6]; 3], 0.0, 0, false, roots, 0.0, S_GRASS));
        if !teal {
            v.push(def("weeping_vines", "Weeping Vines (Having a Little Cry)", Cross, false, false, [T_WEEPING_VINES; 3], 0.0, 0, false, WEEPING_VINES, 0.0, S_GRASS));
            v.push(def("shroomlight", "Shroomlight (Glows, Squishily)", Cube, true, true, [T_SHROOMLIGHT; 3], 1.0, 0, false, SHROOMLIGHT, 15.0, S_GRASS));
        }
    }
    v.push(def("basalt", "Basalt (Volcanic, Stripy)", Cube, true, true, [T_BASALT_TOP, T_BASALT_SIDE, T_BASALT_TOP], 1.25, 1, true, BASALT, 0.0, S_STONE));
    v.push(def("blackstone", "Blackstone (Goth Cobblestone)", Cube, true, true, [T_BLACKSTONE; 3], 1.5, 1, true, BLACKSTONE, 0.0, S_STONE));
    v.push(def("magma_block", "Magma Block (Hot Feet Guaranteed)", Cube, true, true, [T_MAGMA; 3], 0.5, 1, true, MAGMA_BLOCK, 3.0, S_STONE));
    v.push(def("soul_soil", "Soul Soil (Still Sighing)", Cube, true, true, [T_SOUL_SOIL; 3], 0.5, 0, false, SOUL_SOIL, 0.0, S_SAND));
    v.push(def("bone_block", "Bone Block (Someone's Spine)", Cube, true, true, [T_BONE_BLOCK_TOP, T_BONE_BLOCK_SIDE, T_BONE_BLOCK_TOP], 2.0, 1, true, BONE_BLOCK, 0.0, S_STONE));
    let mut soul = def("soul_fire", "Soul Fire (Cold-Looking, Isn't)", Cross, false, false, [T_SOUL_FIRE; 3], 0.0, 0, false, AIR, 10.0, S_GRASS);
    soul.creative = false;
    v.push(soul);
    v.push(def("blackstone_bricks", "Blackstone Bricks (Snout Architecture)", Cube, true, true, [T_BLACKSTONE_BRICKS; 3], 1.5, 1, true, BLACKSTONE_BRICKS, 0.0, S_STONE));
    v.push(def("cracked_blackstone_bricks", "Cracked Blackstone Bricks (Lived In)", Cube, true, true, [T_CRACKED_BLACKSTONE_BRICKS; 3], 1.5, 1, true, CRACKED_BLACKSTONE_BRICKS, 0.0, S_STONE));
    v.push(def("gilded_blackstone", "Gilded Blackstone (Bling, Geologically)", Cube, true, true, [T_GILDED_BLACKSTONE; 3], 1.5, 1, true, GILDED_BLACKSTONE, 0.0, S_STONE));
    v
}

/// Recipes for the new blocks.
pub fn recipes() -> Vec<Recipe> {
    let r = |inputs: &[(Id, u8)], output: (Id, u8)| Recipe { inputs: inputs.to_vec(), output };
    vec![
        // Stems make ordinary planks (and burn like them).
        r(&[(CRIMSON_STEM, 1)], (PLANKS, 4)),
        r(&[(TEAL_STEM, 1)], (PLANKS, 4)),
        r(&[(BLACKSTONE, 4)], (BLACKSTONE_BRICKS, 4)),
        r(&[(BONE_DUST, 9)], (BONE_BLOCK, 1)),
        r(&[(BONE_BLOCK, 1)], (BONE_DUST, 9)),
        // What the biomes' creatures leave (see beasts.rs).
        r(&[(MAGMA_CREAM, 4)], (MAGMA_BLOCK, 1)),
        r(&[(IRON, 1), (SOUL_EMBER, 1)], (SOUL_LANTERN, 1)),
    ]
}

impl World {
    /// The Scorchlands biome under a point (anywhere else: the Wastes).
    pub fn scorch_biome_at(&self, p: Vec3) -> ScorchBiome {
        if crate::scorch::in_scorch(p.x) {
            self.generator.scorch_biome(p.x.floor() as i32, p.z.floor() as i32)
        } else {
            ScorchBiome::Wastes
        }
    }

    /// The air down here near `p`, blended over the biomes round about.
    pub fn scorch_haze(&self, p: Vec3) -> ([f32; 3], f32) {
        let mut c = [0.0; 3];
        let mut d = 0.0;
        let pts = [(0.0, 0.0), (10.0, 0.0), (-10.0, 0.0), (0.0, 10.0), (0.0, -10.0)];
        for (dx, dz) in pts {
            let (hc, hd) = self.generator.scorch_biome((p.x + dx).floor() as i32, (p.z + dz).floor() as i32).haze();
            for i in 0..3 {
                c[i] += hc[i] / pts.len() as f32;
            }
            d += hd / pts.len() as f32;
        }
        (c, d)
    }
}

impl Game {
    /// Spores, ash and embers drifting about in the Scorchlands' biomes (just a sight).
    pub fn motes_tick(&mut self, dt: f32) {
        if self.dedicated || self.menu || !self.in_scorch() {
            return;
        }
        self.mote_acc += dt;
        if self.mote_acc < 0.1 {
            return;
        }
        self.mote_acc = 0.0;
        let me = self.player.body.pos;
        let Some((tile, n)) = self.world.scorch_biome_at(me).motes() else { return };
        for _ in 0..n {
            let r = &mut self.rng;
            let at = me + Vec3::new(r.range(-12.0, 12.0), r.range(-3.0, 8.0), r.range(-12.0, 12.0));
            if self.world.get_v(at.floor().as_ivec3()) != AIR {
                continue;
            }
            let r = &mut self.rng;
            let vel = Vec3::new(r.range(-0.3, 0.3), r.range(-0.4, 0.1), r.range(-0.3, 0.3));
            self.particles.push(crate::entity::Particle { pos: at, vel, life: r.range(2.0, 4.0), tile, uv: [r.range(0.2, 0.7), r.range(0.2, 0.7)], size: 0.05, gravity: 0.0 });
        }
    }

    /// Advancements for finding the Scorchlands' biomes.
    pub fn scorch_explore(&mut self) {
        let key = match self.world.scorch_biome_at(self.player.body.pos) {
            ScorchBiome::CrimsonForest => "seeing_red",
            ScorchBiome::TealForest => "teal_appeal",
            ScorchBiome::BasaltDeltas => "delta_force",
            ScorchBiome::SoulValley => "soul_searching",
            ScorchBiome::Wastes => return,
        };
        self.advance(key);
    }

    /// Standing on a Magma Block burns (sneak, or drink Fire Resistance).
    pub fn magma_feet(&mut self, under: Id) {
        if under == MAGMA_BLOCK && !self.player.sneaking && !self.creative && self.player.hurt <= 0.0 && !self.has_effect(crate::potions::Potion::FireResistance) {
            self.hurt_player(1.0, "discovered the floor was lava. Well, magma");
            self.advance("hot_foot");
        }
    }

    /// Bone Dust on a fungus standing on its own Nylium: up comes a huge one
    /// (where the world lives; joined players ask the host). `who` is the
    /// grower's record key. Returns whether it grew.
    pub fn grow_fungus(&mut self, pos: IVec3, who: &str) -> bool {
        let id = self.world.get_v(pos);
        if !is_fungus(id) {
            return false;
        }
        let teal = id == TEAL_FUNGUS;
        let (nylium, stem, ..) = forest_blocks(teal);
        if self.world.get_v(pos - IVec3::Y) != nylium {
            self.msg("It needs to be on its own Nylium to grow.");
            return false;
        }
        let height = self.rng.int(4, 8);
        let seed = self.rng.int(0, i32::MAX) as u32;
        let parts = huge_fungus(pos, height, teal, seed);
        // The stem needs a clear run up.
        if parts.iter().any(|&(p, b)| b == stem && p != pos && !open(self.world.get_v(p))) {
            return false;
        }
        for (p, b) in parts {
            if open(self.world.get_v(p)) || p == pos {
                self.world.set_v(p, b);
            }
        }
        self.sfx(Sfx::Place(Mat::Grass), Some(pos.as_vec3() + Vec3::splat(0.5)));
        self.advance_for(who, "fungal_growth");
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scorch::SCORCH_ORIGIN;

    /// Somewhere well inside the Scorchlands that's in `want`.
    fn find(g: &Generator, want: ScorchBiome) -> (i32, i32) {
        for r in 0..400 {
            for k in 0..8 {
                let a = k as f32 * std::f32::consts::FRAC_PI_4;
                let (x, z) = (SCORCH_ORIGIN + (a.cos() * r as f32 * 8.0) as i32, (a.sin() * r as f32 * 8.0) as i32);
                if (-24..=24).step_by(8).all(|dx| (-24..=24).step_by(8).all(|dz| g.scorch_biome(x + dx, z + dz) == want)) {
                    return (x, z);
                }
            }
        }
        panic!("no {want:?}");
    }

    #[test]
    fn every_biome_turns_up_and_none_takes_over() {
        let g = Generator::new(4242);
        let mut counts = std::collections::HashMap::new();
        for z in (-4000..4000).step_by(40) {
            for x in (0..8000).step_by(40) {
                *counts.entry(g.scorch_biome(SCORCH_X + 100 + x, z)).or_insert(0) += 1;
            }
        }
        let total: i32 = counts.values().sum();
        for b in [ScorchBiome::Wastes, ScorchBiome::CrimsonForest, ScorchBiome::TealForest, ScorchBiome::BasaltDeltas, ScorchBiome::SoulValley] {
            let share = *counts.get(&b).unwrap_or(&0) as f32 / total as f32;
            assert!((0.06..0.5).contains(&share), "{b:?}: {share}");
        }
    }

    #[test]
    fn each_biome_is_made_of_its_own_stuff() {
        let g = Generator::new(4242);
        let count = |b: &[Id], ids: &[Id]| b.iter().filter(|x| ids.contains(x)).count();
        for (biome, ids, min) in [
            (ScorchBiome::CrimsonForest, &[CRIMSON_NYLIUM, CRIMSON_STEM, CRIMSON_WART][..], 30),
            (ScorchBiome::TealForest, &[TEAL_NYLIUM, TEAL_STEM, TEAL_WART][..], 30),
            (ScorchBiome::BasaltDeltas, &[BASALT, BLACKSTONE][..], 1000),
            (ScorchBiome::SoulValley, &[SORROW_SAND, SOUL_SOIL][..], 30),
        ] {
            let (x, z) = find(&g, biome);
            let (cx, cz) = (x.div_euclid(CW), z.div_euclid(CW));
            let mut n = 0;
            for dz in -1..=1 {
                for dx in -1..=1 {
                    n += count(&g.generate(cx + dx, cz + dz), ids);
                }
            }
            assert!(n >= min, "{biome:?}: only {n}");
        }
        // The forests grow huge fungi, with stems and caps.
        let (x, z) = find(&g, ScorchBiome::CrimsonForest);
        let (cx, cz) = (x.div_euclid(CW), z.div_euclid(CW));
        let stems: usize = (-2..=2).flat_map(|dz| (-2..=2).map(move |dx| (dx, dz))).map(|(dx, dz)| count(&g.generate(cx + dx, cz + dz), &[CRIMSON_STEM])).sum();
        assert!(stems > 8, "{stems} stems");
    }

    #[test]
    fn a_huge_fungus_has_a_stem_a_cap_and_lights() {
        let parts = huge_fungus(ivec3(0, 50, 0), 7, false, 3);
        let stem = parts.iter().filter(|p| p.1 == CRIMSON_STEM).count();
        assert_eq!(stem, 7);
        assert!(parts.iter().filter(|p| p.1 == CRIMSON_WART).count() > 20);
        assert!(parts.iter().all(|p| p.0.y >= 50 || p.1 == WEEPING_VINES));
        let teal = huge_fungus(ivec3(0, 50, 0), 5, true, 3);
        assert!(teal.iter().any(|p| p.1 == TEAL_WART) && !teal.iter().any(|p| p.1 == WEEPING_VINES));
    }

    #[test]
    fn bone_dust_grows_a_fungus_on_its_own_nylium_only() {
        let mut g = crate::game::tests::arena(171);
        let me = crate::players::record_key(&g.player_name);
        let at = ivec3(4, 50, 4);
        g.world.set_v(at - IVec3::Y, TEAL_NYLIUM);
        g.world.set_v(at, CRIMSON_FUNGUS);
        assert!(!g.grow_fungus(at, &me), "crimson won't grow on teal");
        g.world.set_v(at - IVec3::Y, CRIMSON_NYLIUM);
        assert!(g.grow_fungus(at, &me));
        assert_eq!(g.world.get_v(at), CRIMSON_STEM);
        assert!((4..12).any(|y| g.world.get_v(at + IVec3::Y * y) == CRIMSON_WART || g.world.get_v(at + ivec3(2, y, 0)) == CRIMSON_WART));
        assert!(g.advancements.has("fungal_growth"));
    }

    #[test]
    fn magma_burns_unless_you_sneak() {
        let mut g = crate::game::tests::arena(172);
        g.creative = false;
        let h = g.player.health;
        g.player.sneaking = true;
        g.magma_feet(MAGMA_BLOCK);
        assert_eq!(g.player.health, h);
        g.player.sneaking = false;
        g.magma_feet(MAGMA_BLOCK);
        assert!(g.player.health < h);
    }
}
