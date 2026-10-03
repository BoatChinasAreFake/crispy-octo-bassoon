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
    Water,
}

/// Does this face of this block take its biome's colour?
pub fn kind_of(id: Id, face: usize) -> Option<Kind> {
    match id {
        GRASS if face == 2 => Some(Kind::Grass),
        LEAVES | JUNGLE_LEAVES | MANGROVE_LEAVES | TALL_GRASS => Some(Kind::Grass),
        _ if is_water(id) => Some(Kind::Water),
        _ => None,
    }
}

/// A biome's grass and leaf colour, as a multiplier on the plains-green textures.
pub fn grass(b: Biome) -> [f32; 3] {
    match b {
        Biome::Plains => [1.0, 1.0, 1.0],
        Biome::Forest => [0.88, 0.98, 0.85],
        Biome::Desert => [1.3, 1.08, 0.6],
        Biome::Badlands => [1.35, 1.0, 0.55],
        Biome::Snowy => [0.85, 0.97, 1.05],
        Biome::Ocean => [0.95, 1.0, 0.95],
        Biome::Swamp => [0.72, 0.78, 0.5],
        Biome::Jungle => [0.78, 1.18, 0.62],
        Biome::Taiga => [0.8, 0.95, 0.92],
        Biome::Cherry => [1.05, 1.08, 0.85],
        Biome::Mangrove => [0.78, 0.9, 0.55],
        Biome::PaleGarden => [0.72, 0.78, 0.72],
    }
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
    }
}

fn pack(c: [f32; 3]) -> [u8; 4] {
    let q = |v: f32| (v * 128.0).round().clamp(0.0, 255.0) as u8;
    [q(c[0]), q(c[1]), q(c[2]), 255]
}

/// The blended colours at every column corner of a chunk: (CW + 1)^2 each.
pub struct Field {
    grass: Vec<[u8; 4]>,
    water: Vec<[u8; 4]>,
}

impl Field {
    /// The overworld chunk at (cx, cz)'s colours; None in the other dimensions
    /// (nothing there is tinted).
    pub fn of(world: &World, cx: i32, cz: i32) -> Option<Field> {
        let mid = (cx * CW + CW / 2) as f32;
        if crate::scorch::in_scorch(mid) || crate::hollow::in_hollow(mid) {
            return None;
        }
        Some(Field::from_biomes(|x, z| world.generator.column(x, z).1, cx, cz))
    }

    /// The colours, given each column's biome.
    pub fn from_biomes(biome_at: impl Fn(i32, i32) -> Biome, cx: i32, cz: i32) -> Field {
        let r = BLUR + 1;
        let span = (CW + 2 * r) as usize;
        let (x0, z0) = (cx * CW - r, cz * CW - r);
        let mut g = vec![[0.0f32; 3]; span * span];
        let mut w = vec![[0.0f32; 3]; span * span];
        for j in 0..span {
            for i in 0..span {
                let b = biome_at(x0 + i as i32, z0 + j as i32);
                g[j * span + i] = grass(b);
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
        let corners = |b: &[[f32; 3]]| -> Vec<[u8; 4]> {
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
                    out[cz * n + cx] = pack(s);
                }
            }
            out
        };
        Field { grass: corners(&blur(&g)), water: corners(&blur(&w)) }
    }

    /// The colour at a column corner (local 0..=CW each way).
    pub fn at(&self, kind: Kind, cx: i32, cz: i32) -> [u8; 4] {
        let n = CW + 1;
        let i = (cz.clamp(0, CW) * n + cx.clamp(0, CW)) as usize;
        match kind {
            Kind::Grass => self.grass[i],
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
        let row: Vec<u8> = (0..=CW).map(|x| split.at(Kind::Grass, x, 5)[2]).collect();
        assert_eq!(row[0], 128, "far from the border, plains as painted");
        assert_eq!(row[CW as usize], pack(grass(Biome::Desert))[2], "and desert on the far side");
        for w in row.windows(2) {
            assert!(w[1] <= w[0] && w[0] - w[1] <= 10, "blends gently: {row:?}");
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
    fn what_gets_tinted() {
        assert_eq!(kind_of(GRASS, 2), Some(Kind::Grass));
        assert_eq!(kind_of(GRASS, 0), None, "grass sides keep their dirt");
        assert_eq!(kind_of(LEAVES, 4), Some(Kind::Grass));
        assert_eq!(kind_of(WATER, 2), Some(Kind::Water));
        assert_eq!(kind_of(STONE, 2), None);
        assert_eq!(kind_of(SPRUCE_LEAVES, 2), None, "spruce keeps its own colour");
    }
}
