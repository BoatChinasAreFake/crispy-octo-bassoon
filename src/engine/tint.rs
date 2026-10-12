//! Biome tinting: grass, leaves and water take on their biome's colour, and
//! the colours blend smoothly across biome borders.
//!
//! Each column's biome gives a colour; colours are averaged over the 7x7
//! columns around it, and each vertex takes the average of the four columns
//! touching its corner. So a border fades over a few blocks instead of
//! changing in a hard line, and neighbouring chunks agree at their edges.
//! The textures are painted in plains colours, so a tint is a multiplier
//! (128 means "as painted"; see the shader in render.rs).

use crate::block::*;
use crate::world::{Biome, World, CW};

/// "As painted": no change.
pub const NEUTRAL: [u8; 4] = [128, 128, 128, 255];
/// Columns either side averaged into each column's colour.
const BLUR: i32 = 3;

/// What kind of colour a face takes, if any.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Grass,
    /// Leaves: the grass colour, but they turn further with the seasons.
    Leaves,
    Water,
}

/// Does this face of this block take its biome's colour?
pub fn kind_of(id: Id, _face: usize) -> Option<Kind> {
    match id {
        // (Its sides only on their green fringe; see `side_mark`.)
        GRASS => Some(Kind::Grass),
        TALL_GRASS => Some(Kind::Grass),
        LEAVES | JUNGLE_LEAVES | MANGROVE_LEAVES | ACACIA_LEAVES | DARK_OAK_LEAVES => Some(Kind::Leaves),
        _ if is_water(id) => Some(Kind::Water),
        _ => None,
    }
}

/// Plains grass and leaves, the colours the textures are painted in.
const PLAINS_GRASS: [f32; 3] = [145.0, 189.0, 89.0];
const PLAINS_LEAVES: [f32; 3] = [119.0, 171.0, 47.0];

/// A grass block's side takes the biome colour only on its green fringe:
/// its tint is marked (alpha 0) so the shader tints just the fringe's pixels.
pub fn side_mark(id: Id, face: usize, tint: [u8; 4]) -> [u8; 4] {
    if id == GRASS && face != 2 { [tint[0], tint[1], tint[2], 0] } else { tint }
}

/// A biome's grass colour, as a multiplier on the plains-green textures:
/// from lush jungle green through the cool blue-greens of the north to the
/// dry olive of the savanna and the swamp's murk.
pub fn grass(b: Biome) -> [f32; 3] {
    let c = match b {
        Biome::Plains => PLAINS_GRASS,
        Biome::Forest => [121.0, 192.0, 90.0],
        Biome::Desert | Biome::Savanna => [191.0, 183.0, 85.0],
        Biome::Badlands => [144.0, 129.0, 77.0],
        Biome::Snowy | Biome::IceSpikes | Biome::FrozenOcean => [128.0, 180.0, 151.0],
        Biome::Ocean | Biome::WarmOcean | Biome::LukewarmOcean => [142.0, 185.0, 113.0],
        Biome::Swamp | Biome::Mangrove => [106.0, 112.0, 57.0],
        Biome::Jungle => [89.0, 201.0, 60.0],
        Biome::Taiga => [134.0, 183.0, 131.0],
        Biome::Cherry => [182.0, 219.0, 97.0],
        // Grey-green at most (any grass at its edges fades to grey).
        Biome::PaleGarden => [119.0, 130.0, 114.0],
        Biome::BirchForest => [136.0, 187.0, 103.0],
        // Deep and dark.
        Biome::DarkForest => [80.0, 122.0, 50.0],
        Biome::MushroomIslands => [85.0, 201.0, 63.0],
        Biome::Meadow => [131.0, 187.0, 109.0],
        Biome::StonyPeaks => [154.0, 190.0, 75.0],
    };
    [c[0] / PLAINS_GRASS[0], c[1] / PLAINS_GRASS[1], c[2] / PLAINS_GRASS[2]]
}

/// A biome's leaf colour, as a multiplier (leaves have their own palette,
/// a little deeper than the grass beneath them).
pub fn leaves(b: Biome) -> [f32; 3] {
    let c = match b {
        Biome::Plains => PLAINS_LEAVES,
        Biome::Forest | Biome::BirchForest => [89.0, 174.0, 48.0],
        Biome::Desert | Biome::Savanna => [174.0, 164.0, 42.0],
        Biome::Badlands => [158.0, 129.0, 77.0],
        Biome::Snowy | Biome::IceSpikes | Biome::FrozenOcean => [96.0, 161.0, 123.0],
        Biome::Ocean | Biome::WarmOcean | Biome::LukewarmOcean => [113.0, 167.0, 77.0],
        Biome::Swamp => [106.0, 112.0, 57.0],
        Biome::Mangrove => [141.0, 177.0, 39.0],
        // (Jungle leaves are painted lush already.)
        Biome::Jungle => [78.0, 165.0, 30.0],
        Biome::Taiga => [104.0, 164.0, 100.0],
        Biome::Cherry => [150.0, 196.0, 80.0],
        Biome::PaleGarden => [135.0, 141.0, 118.0],
        Biome::DarkForest => [64.0, 128.0, 36.0],
        Biome::MushroomIslands => [70.0, 175.0, 32.0],
        Biome::Meadow => [99.0, 165.0, 83.0],
        Biome::StonyPeaks => [130.0, 172.0, 30.0],
    };
    // (The painted leaves have little blue in them, and a tint can at most double it.)
    [c[0] / PLAINS_LEAVES[0], c[1] / PLAINS_LEAVES[1], (c[2] / PLAINS_LEAVES[2]).min(1.95)]
}

/// A biome's water colour, as a multiplier.
pub fn water(b: Biome) -> [f32; 3] {
    match b {
        Biome::Plains | Biome::Forest | Biome::Ocean => [1.0, 1.0, 1.0],
        Biome::Swamp => [0.6, 0.92, 0.42],
        Biome::Mangrove => [0.65, 0.95, 0.55],
        Biome::Cherry => [0.95, 1.0, 1.1],
        Biome::PaleGarden => [0.7, 0.78, 0.8],
        Biome::Jungle => [0.8, 1.08, 0.95],
        Biome::Snowy => [0.85, 0.95, 1.18],
        Biome::Taiga => [0.85, 0.98, 1.1],
        Biome::Desert => [0.95, 1.12, 1.05],
        Biome::Badlands => [1.05, 0.98, 0.85],
        Biome::Savanna => [0.95, 1.05, 1.0],
        Biome::BirchForest | Biome::Meadow | Biome::StonyPeaks => [0.95, 1.0, 1.08],
        Biome::DarkForest => [0.75, 0.9, 0.95],
        Biome::MushroomIslands => [1.05, 0.85, 1.1],
        Biome::IceSpikes => [0.85, 0.95, 1.18],
        // Clear turquoise, warmer green-blue, and deep cold blue.
        Biome::WarmOcean => [0.55, 1.25, 1.15],
        Biome::LukewarmOcean => [0.75, 1.12, 1.08],
        Biome::FrozenOcean => [0.7, 0.8, 1.15],
    }
}

fn pack(c: [f32; 3]) -> [u8; 4] {
    let q = |v: f32| (v * 128.0).round().clamp(0.0, 255.0) as u8;
    [q(c[0]), q(c[1]), q(c[2]), 255]
}

/// The blended colours at every column corner of a chunk: (CW + 1)^2 each.
pub struct Field {
    grass: Vec<[u8; 4]>,
    leaves: Vec<[u8; 4]>,
    water: Vec<[u8; 4]>,
}

impl Field {
    /// The overworld chunk at (cx, cz)'s colours; None in the other dimensions
    /// (nothing there is tinted).
    pub fn of(world: &World, cx: i32, cz: i32) -> Option<Field> {
        if world.dim() != crate::dims::Dim::Over {
            return None;
        }
        let biome_at = |x, z| world.generator.column(x, z).1;
        // (Worlds from version 3 blend over about twenty blocks, so one biome's
        // colours fade into the next's instead of changing at a line.)
        Some(if world.generator.opts.version >= 3 { Field::wide(biome_at, cx, cz) } else { Field::from_biomes(biome_at, cx, cz) })
    }

    /// The colours blended widely: biomes sampled every `STEP` blocks (on a
    /// grid shared by every chunk), each sample averaged with its neighbours
    /// `WIDE` steps around, then smoothly in between.
    pub fn wide(biome_at: impl Fn(i32, i32) -> Biome, cx: i32, cz: i32) -> Field {
        const STEP: i32 = 4;
        const WIDE: i32 = 2;
        let (g0x, g0z) = (cx * CW / STEP - WIDE, cz * CW / STEP - WIDE);
        let n = (CW / STEP + 1 + 2 * WIDE) as usize;
        let mut g = vec![[0.0f32; 3]; n * n];
        let mut l = vec![[0.0f32; 3]; n * n];
        let mut w = vec![[0.0f32; 3]; n * n];
        for j in 0..n {
            for i in 0..n {
                let b = biome_at((g0x + i as i32) * STEP, (g0z + j as i32) * STEP);
                g[j * n + i] = grass(b);
                l[j * n + i] = leaves(b);
                w[j * n + i] = water(b);
            }
        }
        let inner = (CW / STEP + 1) as usize;
        let blur = |src: &[[f32; 3]]| -> Vec<[f32; 3]> {
            let mut out = vec![[0.0f32; 3]; inner * inner];
            let k = ((2 * WIDE + 1) * (2 * WIDE + 1)) as f32;
            for j in 0..inner {
                for i in 0..inner {
                    let mut s = [0.0; 3];
                    for dj in 0..=2 * WIDE as usize {
                        for di in 0..=2 * WIDE as usize {
                            let c = src[(j + dj) * n + i + di];
                            (0..3).for_each(|q| s[q] += c[q]);
                        }
                    }
                    out[j * inner + i] = s.map(|v| v / k);
                }
            }
            out
        };
        let corners = |b: &[[f32; 3]], season: [f32; 3]| -> Vec<[u8; 4]> {
            let m = (CW + 1) as usize;
            let mut out = vec![NEUTRAL; m * m];
            for z in 0..m {
                for x in 0..m {
                    let (fx, fz) = (x as f32 / STEP as f32, z as f32 / STEP as f32);
                    let (i, j) = ((fx as usize).min(inner - 2), (fz as usize).min(inner - 2));
                    let (u, v) = (fx - i as f32, fz - j as f32);
                    let at = |a: usize, b: usize| b * inner + a;
                    let mut s = [0.0; 3];
                    for q in 0..3 {
                        let top = b[at(i, j)][q] * (1.0 - u) + b[at(i + 1, j)][q] * u;
                        let bottom = b[at(i, j + 1)][q] * (1.0 - u) + b[at(i + 1, j + 1)][q] * u;
                        s[q] = (top * (1.0 - v) + bottom * v) * season[q];
                    }
                    out[z * m + x] = pack(s);
                }
            }
            out
        };
        let season = crate::seasons::shown();
        Field { grass: corners(&blur(&g), crate::seasons::grass_tint(season)), leaves: corners(&blur(&l), crate::seasons::leaf_tint(season)), water: corners(&blur(&w), [1.0; 3]) }
    }

    /// The colours, given each column's biome.
    pub fn from_biomes(biome_at: impl Fn(i32, i32) -> Biome, cx: i32, cz: i32) -> Field {
        let r = BLUR + 1;
        let span = (CW + 2 * r) as usize;
        let (x0, z0) = (cx * CW - r, cz * CW - r);
        let mut g = vec![[0.0f32; 3]; span * span];
        let mut l = vec![[0.0f32; 3]; span * span];
        let mut w = vec![[0.0f32; 3]; span * span];
        for j in 0..span {
            for i in 0..span {
                let b = biome_at(x0 + i as i32, z0 + j as i32);
                g[j * span + i] = grass(b);
                l[j * span + i] = leaves(b);
                w[j * span + i] = water(b);
            }
        }
        // Box blur, separably, then the four columns round each corner.
        let blur = |src: &[[f32; 3]]| -> Vec<[f32; 3]> {
            let mut h = vec![[0.0f32; 3]; span * span];
            for j in 0..span {
                for i in BLUR as usize..span - BLUR as usize {
                    let mut s = [0.0; 3];
                    for d in -BLUR..=BLUR {
                        let c = src[j * span + (i as i32 + d) as usize];
                        (0..3).for_each(|k| s[k] += c[k]);
                    }
                    h[j * span + i] = s.map(|v| v / (2 * BLUR + 1) as f32);
                }
            }
            let mut out = vec![[0.0f32; 3]; span * span];
            for j in BLUR as usize..span - BLUR as usize {
                for i in 0..span {
                    let mut s = [0.0; 3];
                    for d in -BLUR..=BLUR {
                        let c = h[(j as i32 + d) as usize * span + i];
                        (0..3).for_each(|k| s[k] += c[k]);
                    }
                    out[j * span + i] = s.map(|v| v / (2 * BLUR + 1) as f32);
                }
            }
            out
        };
        let corners = |b: &[[f32; 3]], season: [f32; 3]| -> Vec<[u8; 4]> {
            let n = (CW + 1) as usize;
            let mut out = vec![NEUTRAL; n * n];
            for cz in 0..n {
                for cx in 0..n {
                    // Corner (cx, cz) touches columns cx - 1 and cx (local), which sit
                    // at r + cx - 1 and r + cx in the padded grid.
                    let (i, j) = (r as usize + cx, r as usize + cz);
                    let mut s = [0.0; 3];
                    for (di, dj) in [(1, 1), (0, 1), (1, 0), (0, 0)] {
                        let c = b[(j - dj) * span + (i - di)];
                        (0..3).for_each(|k| s[k] += c[k] * 0.25);
                    }
                    out[cz * n + cx] = pack([s[0] * season[0], s[1] * season[1], s[2] * season[2]]);
                }
            }
            out
        };
        // The season turns grass, and leaves further (see seasons.rs).
        let season = crate::seasons::shown();
        Field { grass: corners(&blur(&g), crate::seasons::grass_tint(season)), leaves: corners(&blur(&l), crate::seasons::leaf_tint(season)), water: corners(&blur(&w), [1.0; 3]) }
    }

    /// The colour at a column corner (local 0..=CW each way).
    pub fn at(&self, kind: Kind, cx: i32, cz: i32) -> [u8; 4] {
        let n = CW + 1;
        let i = (cz.clamp(0, CW) * n + cx.clamp(0, CW)) as usize;
        match kind {
            Kind::Grass => self.grass[i],
            Kind::Leaves => self.leaves[i],
            Kind::Water => self.water[i],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_biome_is_one_colour_and_borders_blend() {
        let plains = Field::from_biomes(|_, _| Biome::Plains, 0, 0);
        assert!((0..=CW).all(|i| plains.at(Kind::Grass, i, i) == NEUTRAL));
        // Desert from x = 8: a smooth ramp across the border, no jumps.
        let split = Field::from_biomes(|x, _| if x < 8 { Biome::Plains } else { Biome::Desert }, 0, 0);
        let row: Vec<u8> = (0..=CW).map(|x| split.at(Kind::Grass, x, 5)[0]).collect();
        assert_eq!(row[0], 128, "far from the border, plains as painted");
        assert_eq!(row[CW as usize], pack(grass(Biome::Desert))[0], "and desert on the far side");
        for w in row.windows(2) {
            assert!(w[1] >= w[0] && w[1] - w[0] <= 10, "blends gently: {row:?}");
        }
        assert!(row.iter().filter(|&&b| b != 128 && b != row[CW as usize]).count() >= 5, "over several blocks: {row:?}");
        // Neighbouring chunks agree along their shared edge.
        let biome = |x: i32, z: i32| if (x * 7 + z * 3).rem_euclid(23) < 9 { Biome::Swamp } else { Biome::Jungle };
        let (a, b) = (Field::from_biomes(biome, 0, 0), Field::from_biomes(biome, 1, 0));
        for z in 0..=CW {
            assert_eq!(a.at(Kind::Water, CW, z), b.at(Kind::Water, 0, z));
            assert_eq!(a.at(Kind::Grass, CW, z), b.at(Kind::Grass, 0, z));
        }
    }

    #[test]
    fn wide_blending_is_gentle_and_seamless() {
        let split = |x: i32, _| if x < 8 { Biome::Plains } else { Biome::Desert };
        let row: Vec<u8> = (-2..=2).flat_map(|cx| (0..CW).map(move |x| (cx, x))).map(|(cx, x)| Field::wide(split, cx, 0).at(Kind::Grass, x, 5)[0]).collect();
        for w in row.windows(2) {
            assert!(w[0].abs_diff(w[1]) <= 4, "blends gently: {row:?}");
        }
        assert!(row.iter().filter(|&&b| b != 128 && b != pack(grass(Biome::Desert))[0]).count() >= 14, "over many blocks: {row:?}");
        let biome = |x: i32, z: i32| if (x / 9 + z / 5).rem_euclid(3) == 0 { Biome::Swamp } else { Biome::Jungle };
        let (a, b) = (Field::wide(biome, 0, 0), Field::wide(biome, 1, 0));
        let (c, d) = (Field::wide(biome, 0, -1), Field::wide(biome, 0, 0));
        for k in 0..=CW {
            assert_eq!(a.at(Kind::Water, CW, k), b.at(Kind::Water, 0, k));
            assert_eq!(c.at(Kind::Grass, k, CW), d.at(Kind::Grass, k, 0));
        }
    }

    #[test]
    fn what_gets_tinted() {
        assert_eq!(kind_of(GRASS, 2), Some(Kind::Grass));
        assert_eq!(kind_of(GRASS, 0), Some(Kind::Grass), "grass sides take it on their fringe");
        assert_eq!(side_mark(GRASS, 0, NEUTRAL)[3], 0, "(marked as a side)");
        assert_eq!(side_mark(GRASS, 2, NEUTRAL), NEUTRAL, "the top is tinted all over");
        assert_eq!(kind_of(LEAVES, 4), Some(Kind::Leaves), "leaves follow the grass, and turn further with the seasons");
        assert_eq!(kind_of(WATER, 2), Some(Kind::Water));
        assert_eq!(kind_of(STONE, 2), None);
        assert_eq!(kind_of(SPRUCE_LEAVES, 2), None, "spruce keeps its own colour");
    }

    #[test]
    fn biomes_look_different() {
        use Biome::*;
        // Every pair of these grassy biomes differs clearly in grass or leaf colour.
        let grassy = [Plains, Forest, Savanna, Badlands, Snowy, Swamp, Jungle, Taiga, DarkForest, BirchForest, Meadow, Cherry, MushroomIslands, PaleGarden];
        let apart = |a: [f32; 3], b: [f32; 3]| (0..3).map(|i| (a[i] - b[i]).abs()).sum::<f32>();
        for (i, &a) in grassy.iter().enumerate() {
            for &b in &grassy[i + 1..] {
                assert!(apart(grass(a), grass(b)) + apart(leaves(a), leaves(b)) > 0.12, "{a:?} and {b:?} look alike");
            }
        }
        // And a tint never more than doubles a colour (what it can store).
        assert!(grassy.iter().all(|&b| grass(b).iter().chain(leaves(b).iter()).all(|&v| v < 2.0)));
    }
}
