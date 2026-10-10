//! Chunked voxel world: storage, threaded terrain generation, edits and raycasts.

use crate::dims::Dim;
use crate::block::*;
use crate::noise::{hash2, hash3, Perlin};
use crate::containers::{is_container, Container};
use crate::farming::{is_farmland, Soil};
use crate::palette::PalettedBlocks;
use macroquad::math::{ivec3, IVec3, Vec3};
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

pub const CW: i32 = 16;
/// The height of the world (every dimension's).
pub const CH: i32 = 256;
/// Sea level in worlds made before the world grew to 256 (generator version
/// below 3; see `GenOptions::sea`), and in new ones.
pub const OLD_SEA: i32 = 40;
pub const NEW_SEA: i32 = 63;
/// How deep (below the sea) the shallow coastal shelf goes before the sea floor drops away.
const SEA_SHELF: f32 = 2.0;
const CHUNK_VOL: usize = (CW * CW * CH) as usize;

#[inline]
/// 0 below `a`, 1 above `b`, and a smooth S between.
pub fn smoothstep(a: f32, b: f32, v: f32) -> f32 {
    let k = ((v - a) / (b - a)).clamp(0.0, 1.0);
    k * k * (3.0 - 2.0 * k)
}

pub fn idx(lx: i32, y: i32, lz: i32) -> usize {
    ((y * CW + lz) * CW + lx) as usize
}

pub struct Chunk {
    /// Block ids, packed per 16-block-tall section (see palette.rs). Index with `idx`.
    pub blocks: PalettedBlocks,
    /// Per column: one above the highest sky-blocking block.
    pub heights: [u8; 256],
    /// Per column: one above the highest block that blocks or dapples sunlight
    /// (so at least `heights`; higher where there are leaves).
    pub canopy: [u8; 256],
    /// Sky and block light levels (see light.rs).
    pub light: crate::light::LightStore,
}


impl Chunk {
    /// A chunk whose heightmaps still need `recompute_heights`.
    pub fn new(blocks: PalettedBlocks) -> Chunk {
        Chunk { blocks, heights: [0; 256], canopy: [0; 256], light: Default::default() }
    }
    fn recompute_height(&mut self, lx: i32, lz: i32) {
        let (mut h, mut canopy) = (0, 0);
        for y in (0..CH).rev() {
            let id = self.blocks.get(idx(lx, y, lz));
            if canopy == 0 && (blocks_sky(id) || dapples_sky(id)) {
                canopy = y + 1;
            }
            if blocks_sky(id) {
                h = y + 1;
                break;
            }
        }
        self.heights[(lz * CW + lx) as usize] = h as u8;
        self.canopy[(lz * CW + lx) as usize] = canopy as u8;
    }
    fn recompute_heights(&mut self) {
        for lz in 0..CW {
            for lx in 0..CW {
                self.recompute_height(lx, lz);
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Biome {
    Plains,
    Forest,
    Desert,
    Snowy,
    Ocean,
    /// Low, wet and warm-ish: mud, shallow pools, lily pads, droopy oaks, Bloops.
    Swamp,
    /// Hot and wet: tall trees with wide crowns, melons, Squawkers.
    Jungle,
    /// Hot and very dry: red sand over bands of terracotta, dead bushes, more gold.
    Badlands,
    /// Cool and damp: spruce forests and Woofers.
    Taiga,
    /// Mild hills under pink cherry trees, the grass strewn with petals.
    Cherry,
    /// Warm coastal mudflats: mangroves standing on their roots in the shallows.
    Mangrove,
    /// A grey, still forest of pale oaks; something in the trunks wakes at night.
    PaleGarden,
    /// Warm, dry grassland dotted with flat-topped acacias.
    Savanna,
    /// A bright forest of white birches.
    BirchForest,
    /// A gloomy forest of dark oaks so close together it's dim at noon (and the odd giant mushroom).
    DarkForest,
    /// Lonely islands far out at sea: mycelium, giant mushrooms, red-spotted Mooers, and no monsters.
    MushroomIslands,
    /// Snowy flats with great spikes of packed ice.
    IceSpikes,
    /// High, gentle grassland thick with flowers.
    Meadow,
    /// Bare stone mountaintops.
    StonyPeaks,
    /// Seas by temperature (`Ocean` is the ordinary, cool one; see seas.rs):
    /// clear warm water and coral, lukewarm seagrass, frozen ice and icebergs.
    WarmOcean,
    LukewarmOcean,
    FrozenOcean,
}

impl Biome {
    pub const ALL: [Biome; 22] = [
        Biome::Plains,
        Biome::Forest,
        Biome::Desert,
        Biome::Snowy,
        Biome::Ocean,
        Biome::Swamp,
        Biome::Jungle,
        Biome::Badlands,
        Biome::Taiga,
        Biome::Cherry,
        Biome::Mangrove,
        Biome::PaleGarden,
        Biome::Savanna,
        Biome::BirchForest,
        Biome::DarkForest,
        Biome::MushroomIslands,
        Biome::IceSpikes,
        Biome::Meadow,
        Biome::StonyPeaks,
        Biome::WarmOcean,
        Biome::LukewarmOcean,
        Biome::FrozenOcean,
    ];

    /// Any of the seas.
    pub fn is_ocean(self) -> bool {
        matches!(self, Biome::Ocean | Biome::WarmOcean | Biome::LukewarmOcean | Biome::FrozenOcean)
    }

    /// For /locate: "plains", "snowy", "badlands"...
    pub fn from_name(s: &str) -> Option<Biome> {
        let s = s.trim().to_ascii_lowercase();
        Biome::ALL.into_iter().find(|b| format!("{b:?}").to_ascii_lowercase() == s || b.name().to_ascii_lowercase().starts_with(&s))
    }

    pub fn name(self) -> &'static str {
        match self {
            Biome::Plains => "Plains (Flat-ish)",
            Biome::Forest => "Forest (Trees Included)",
            Biome::Desert => "Desert (Dry Humour)",
            Biome::Snowy => "Snowy (Chilly)",
            Biome::Ocean => "Ocean (Wet)",
            Biome::Swamp => "Swamp (Squelchy)",
            Biome::Jungle => "Jungle (Humid)",
            Biome::Badlands => "Badlands (Stripy)",
            Biome::Taiga => "Taiga (Pointy Trees)",
            Biome::Cherry => "Cherry Grove (Aggressively Pink)",
            Biome::Mangrove => "Mangrove Swamp (Rooty)",
            Biome::PaleGarden => "Pale Garden (Don't Look Away)",
            Biome::Savanna => "Savanna (Flat Tops)",
            Biome::BirchForest => "Birch Forest (Pale and Interesting)",
            Biome::DarkForest => "Dark Forest (Bring a Torch)",
            Biome::MushroomIslands => "Mushroom Islands (Fungi to Be With)",
            Biome::IceSpikes => "Ice Spikes (Pointy, Chilly)",
            Biome::Meadow => "Meadow (Hay Fever)",
            Biome::StonyPeaks => "Stony Peaks (Rock Bottom, but Up)",
            Biome::WarmOcean => "Warm Ocean (Tropical-ish)",
            Biome::LukewarmOcean => "Lukewarm Ocean (Bathwater)",
            Biome::FrozenOcean => "Frozen Ocean (Brr)",
        }
    }

    /// Too dry for rain (it just stays cloudy).
    pub fn dry(self) -> bool {
        matches!(self, Biome::Desert | Biome::Badlands | Biome::Savanna)
    }

    /// What kind of tree grows here.
    pub fn tree(self) -> TreeKind {
        match self {
            Biome::Snowy | Biome::Taiga | Biome::IceSpikes | Biome::StonyPeaks => TreeKind::Spruce,
            Biome::Savanna => TreeKind::Acacia,
            Biome::BirchForest | Biome::Meadow => TreeKind::Birch,
            Biome::DarkForest => TreeKind::DarkOak,
            Biome::MushroomIslands => TreeKind::RedMushroom,
            Biome::Jungle => TreeKind::Jungle,
            Biome::Swamp => TreeKind::Swamp,
            Biome::Cherry => TreeKind::Cherry,
            Biome::Mangrove => TreeKind::Mangrove,
            Biome::PaleGarden => TreeKind::PaleOak,
            _ => TreeKind::Oak,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TreeKind {
    Oak,
    /// Tall and conical, with needles.
    Spruce,
    /// Very tall, with a wide flat crown.
    Jungle,
    /// An oak with a wide crown and leaves hanging off it.
    Swamp,
    /// A short trunk under a wide, round, pink crown.
    Cherry,
    /// Up on a tangle of roots, with a leafy crown.
    Mangrove,
    /// A thick trunk under a broad, flat, pale crown hung with moss.
    PaleOak,
    /// A trunk that leans over to one side under a wide, flat crown.
    Acacia,
    /// Tall and slim, white bark, a small round crown.
    Birch,
    /// A thick, short trunk under a broad dark crown.
    DarkOak,
    /// Giant mushrooms: a stalk under a domed red cap, or a flat brown one.
    RedMushroom,
    BrownMushroom,
}

impl TreeKind {
    pub fn log(self) -> Id {
        match self {
            TreeKind::Spruce => SPRUCE_LOG,
            TreeKind::Jungle => JUNGLE_LOG,
            TreeKind::Cherry => CHERRY_LOG,
            TreeKind::Mangrove => MANGROVE_LOG,
            TreeKind::PaleOak => PALE_OAK_LOG,
            TreeKind::Acacia => ACACIA_LOG,
            TreeKind::Birch => BIRCH_LOG,
            TreeKind::DarkOak => DARK_OAK_LOG,
            TreeKind::RedMushroom | TreeKind::BrownMushroom => MUSHROOM_STEM,
            _ => LOG,
        }
    }
    pub fn leaves(self) -> Id {
        match self {
            TreeKind::Spruce => SPRUCE_LEAVES,
            TreeKind::Jungle => JUNGLE_LEAVES,
            TreeKind::Cherry => CHERRY_LEAVES,
            TreeKind::Mangrove => MANGROVE_LEAVES,
            TreeKind::PaleOak => PALE_OAK_LEAVES,
            TreeKind::Acacia => ACACIA_LEAVES,
            TreeKind::Birch => BIRCH_LEAVES,
            TreeKind::DarkOak => DARK_OAK_LEAVES,
            TreeKind::RedMushroom => RED_MUSHROOM_BLOCK,
            TreeKind::BrownMushroom => BROWN_MUSHROOM_BLOCK,
            _ => LEAVES,
        }
    }
    /// How tall its trunk grows, from a 0..1 roll.
    pub fn trunk(self, roll: f32) -> i32 {
        let (lo, span) = match self {
            TreeKind::Oak => (4, 3.0),
            TreeKind::Spruce => (6, 4.0),
            TreeKind::Jungle => (9, 5.0),
            TreeKind::Swamp => (4, 3.0),
            TreeKind::Cherry => (4, 2.0),
            TreeKind::Mangrove => (6, 3.0),
            TreeKind::PaleOak => (6, 3.0),
            TreeKind::Acacia => (3, 2.0),
            TreeKind::Birch => (5, 3.0),
            TreeKind::DarkOak => (5, 3.0),
            TreeKind::RedMushroom | TreeKind::BrownMushroom => (4, 3.0),
        };
        lo + (roll * span) as i32
    }
    /// Where its blocks go, relative to the ground block under it: (offset,
    /// block, whether it pushes through what's there). `roll` gives a
    /// repeatable 0..1 per cell (the generator hashes, saplings use dice).
    /// Every leaf is within reach of the trunk (see `trees::LEAF_REACH`).
    pub fn shape(self, trunk: i32, mut roll: impl FnMut(IVec3) -> f32) -> Vec<(IVec3, Id, bool)> {
        let (log, leaves) = (self.log(), self.leaves());
        let top = trunk; // the highest log, above the ground block at 0
        let mut out = Vec::new();
        let leaf = |o: IVec3, out: &mut Vec<(IVec3, Id, bool)>| out.push((o, leaves, false));
        match self {
            TreeKind::Oak => {
                for dy in -2..=1 {
                    let rad: i32 = if dy <= -1 { 2 } else { 1 };
                    for dz in -rad..=rad {
                        for dx in -rad..=rad {
                            let corner = dx.abs() == rad && dz.abs() == rad;
                            let o = ivec3(dx, top + dy, dz);
                            if corner && (dy == 1 || roll(o) < 0.5) {
                                continue;
                            }
                            leaf(o, &mut out);
                        }
                    }
                }
            }
            TreeKind::Spruce => {
                // A cone: a tuft on top, then rings widening and narrowing down the trunk.
                leaf(ivec3(0, top + 1, 0), &mut out);
                let mut y = top;
                let mut k = 0;
                while y >= 3 {
                    let rad = [1, 2, 1, 2, 3, 2, 3][k.min(6)];
                    for dz in -rad..=rad {
                        for dx in -rad..=rad {
                            if dx == 0 && dz == 0 || dx * dx + dz * dz > rad * rad + 1 {
                                continue;
                            }
                            leaf(ivec3(dx, y, dz), &mut out);
                        }
                    }
                    y -= 1;
                    k += 1;
                }
            }
            TreeKind::Jungle => {
                // A wide, flat crown round the top of the trunk.
                for dy in -1..=1 {
                    let rad: i32 = if dy == 1 { 2 } else { 3 };
                    for dz in -rad..=rad {
                        for dx in -rad..=rad {
                            let o = ivec3(dx, top + dy, dz);
                            if dx * dx + dz * dz > rad * rad || (dx == 0 && dz == 0 && dy < 1) || (dx.abs() + dz.abs() == rad + 1 && roll(o) < 0.5) {
                                continue;
                            }
                            leaf(o, &mut out);
                        }
                    }
                }
                // Tufts sprouting from the trunk lower down.
                for y in 3..top - 2 {
                    for d in [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z] {
                        let o = d + IVec3::Y * y;
                        if roll(o) < 0.12 {
                            leaf(o, &mut out);
                        }
                    }
                }
            }
            TreeKind::Swamp => {
                for dy in -1..=1 {
                    let rad: i32 = if dy == 1 { 2 } else { 3 };
                    for dz in -rad..=rad {
                        for dx in -rad..=rad {
                            let o = ivec3(dx, top + dy, dz);
                            // No far corners (they'd be out of reach of the trunk).
                            if dx.abs() + dz.abs() > rad + 1 || (dx == 0 && dz == 0 && dy < 1) {
                                continue;
                            }
                            leaf(o, &mut out);
                        }
                    }
                }
                // Leaves hanging down from the crown's edge.
                for (dx, dz) in [(2, 0), (-2, 0), (0, 2), (0, -2), (2, 1), (-1, 2), (1, -2), (-2, -1)] {
                    let o = ivec3(dx, top - 2, dz);
                    if roll(o) < 0.5 {
                        leaf(o, &mut out);
                    }
                }
            }
            TreeKind::PaleOak => {
                // A thick trunk (two by two), one with a Creaking Heart in it now and then.
                for (dx, dz) in [(1, 0), (0, 1), (1, 1)] {
                    for y in 1..=top {
                        out.push((ivec3(dx, y, dz), log, true));
                    }
                }
                if roll(ivec3(0, -64, 0)) < 0.12 {
                    out.push((ivec3(0, 3, 0), CREAKING_HEART, true));
                }
                // A broad, flat crown.
                for dy in -1..=1 {
                    let rad: i32 = if dy == 1 { 2 } else { 3 };
                    for dz in -rad..=rad + 1 {
                        for dx in -rad..=rad + 1 {
                            let (ex, ez) = (if dx > 0 { dx - 1 } else { dx }, if dz > 0 { dz - 1 } else { dz });
                            let o = ivec3(dx, top + dy, dz);
                            if ex * ex + ez * ez > rad * rad || (ex * ex + ez * ez == rad * rad && roll(o) < 0.5) {
                                continue;
                            }
                            leaf(o, &mut out);
                        }
                    }
                }
                // Pale moss hanging from its underside.
                for (dx, dz) in [(3, 0), (-3, 1), (1, 3), (0, -3), (2, 2), (-2, -2), (3, 2), (-2, 3)] {
                    let o = ivec3(dx, top - 2, dz);
                    if roll(o) < 0.6 {
                        out.push((o, PALE_HANGING_MOSS, false));
                    }
                }
            }
            TreeKind::Cherry => {
                // A wide, rounded blossom crown, with petals' worth hanging below.
                for dy in -1..=2 {
                    let rad: i32 = match dy {
                        -1 => 2,
                        2 => 1,
                        _ => 3,
                    };
                    for dz in -rad..=rad {
                        for dx in -rad..=rad {
                            let o = ivec3(dx, top + dy, dz);
                            if dx * dx + dz * dz > rad * rad || (dx == 0 && dz == 0 && dy < 1) || (dx * dx + dz * dz == rad * rad && roll(o) < 0.4) {
                                continue;
                            }
                            leaf(o, &mut out);
                        }
                    }
                }
                // Blossom trailing below the crown's edge.
                for (dx, dz) in [(2, 0), (-2, 0), (0, 2), (0, -2), (1, 2), (-2, 1)] {
                    let o = ivec3(dx, top - 2, dz);
                    if roll(o) < 0.45 {
                        leaf(o, &mut out);
                    }
                }
            }
            TreeKind::Acacia => {
                // The trunk leans off to one side, then a wide, flat crown on top.
                let dirs = [ivec3(1, 0, 0), ivec3(-1, 0, 0), ivec3(0, 0, 1), ivec3(0, 0, -1)];
                let d = dirs[(roll(ivec3(0, -32, 0)) * 4.0) as usize % 4];
                out.push((d + IVec3::Y * (top + 1), log, true));
                out.push((d * 2 + IVec3::Y * (top + 2), log, true));
                let crown = d * 2 + IVec3::Y * (top + 3);
                for dz in -2..=2i32 {
                    for dx in -2..=2i32 {
                        let o = crown + ivec3(dx, 0, dz);
                        if dx.abs() == 2 && dz.abs() == 2 {
                            continue;
                        }
                        leaf(o, &mut out);
                        if dx.abs() <= 1 && dz.abs() <= 1 && !(dx.abs() == 1 && dz.abs() == 1 && roll(o) < 0.5) {
                            leaf(o + IVec3::Y, &mut out);
                        }
                    }
                }
            }
            TreeKind::Birch => {
                // Like an oak's, but smaller and higher.
                for dy in -2..=1 {
                    let rad: i32 = if dy <= -1 { 2 } else { 1 };
                    for dz in -rad..=rad {
                        for dx in -rad..=rad {
                            let o = ivec3(dx, top + dy, dz);
                            if dx.abs() == rad && dz.abs() == rad && (dy == 1 || rad == 1 || roll(o) < 0.6) {
                                continue;
                            }
                            leaf(o, &mut out);
                        }
                    }
                }
            }
            TreeKind::DarkOak => {
                // Two by two, short and thick, under a broad dark crown.
                for (dx, dz) in [(1, 0), (0, 1), (1, 1)] {
                    for y in 1..=top {
                        out.push((ivec3(dx, y, dz), log, true));
                    }
                }
                for dy in -1..=1 {
                    let rad: i32 = if dy == 1 { 2 } else { 3 };
                    for dz in -rad..=rad + 1 {
                        for dx in -rad..=rad + 1 {
                            let (ex, ez) = (if dx > 0 { dx - 1 } else { dx }, if dz > 0 { dz - 1 } else { dz });
                            let o = ivec3(dx, top + dy, dz);
                            if ex * ex + ez * ez > rad * rad + 1 || (ex * ex + ez * ez >= rad * rad && roll(o) < 0.4) {
                                continue;
                            }
                            leaf(o, &mut out);
                        }
                    }
                }
            }
            TreeKind::RedMushroom => {
                // A domed cap: a flat top, and sides hanging down round the stalk.
                for dz in -1..=1 {
                    for dx in -1..=1 {
                        leaf(ivec3(dx, top + 1, dz), &mut out);
                    }
                }
                for dy in -2..=0 {
                    for dz in -2..=2i32 {
                        for dx in -2..=2i32 {
                            let ring = dx.abs() == 2 || dz.abs() == 2;
                            if ring && !(dx.abs() == 2 && dz.abs() == 2) {
                                leaf(ivec3(dx, top + dy, dz), &mut out);
                            }
                        }
                    }
                }
            }
            TreeKind::BrownMushroom => {
                // A wide, flat cap.
                for dz in -3..=3i32 {
                    for dx in -3..=3i32 {
                        if dx.abs() == 3 && dz.abs() == 3 {
                            continue;
                        }
                        leaf(ivec3(dx, top + 1, dz), &mut out);
                    }
                }
            }
            TreeKind::Mangrove => {
                // Roots arching out from the trunk down into the mud and water.
                for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1), (1, -1), (-1, 1)] {
                    let lean = if dx != 0 && dz != 0 { 1 } else { 2 };
                    for y in 0..=lean {
                        let o = ivec3(dx, y, dz);
                        if roll(o) < 0.8 {
                            out.push((o, MANGROVE_ROOTS, true));
                        }
                    }
                }
                for dy in -1..=1 {
                    let rad: i32 = if dy == 1 { 1 } else { 2 };
                    for dz in -rad..=rad {
                        for dx in -rad..=rad {
                            let o = ivec3(dx, top + dy, dz);
                            if (dx.abs() == rad && dz.abs() == rad && roll(o) < 0.5) || (dx == 0 && dz == 0 && dy < 1) {
                                continue;
                            }
                            leaf(o, &mut out);
                        }
                    }
                }
                // A few leaves trailing down.
                for (dx, dz) in [(2, 0), (-2, 0), (0, 2), (0, -2)] {
                    let o = ivec3(dx, top - 2, dz);
                    if roll(o) < 0.5 {
                        leaf(o, &mut out);
                    }
                }
            }
        }
        for y in 1..=top {
            out.push((ivec3(0, y, 0), log, true));
        }
        out
    }
}

/// World generation options, chosen when a world is made (see the Create
/// World screen) and kept with it. Worlds from before there were options
/// are `LEGACY`, so they carry on generating exactly as they always did.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GenOptions {
    /// 0: legacy (the old rules), 1: these options, 2: rarer villages
    /// (and outposts kept away from them), bigger and rarer fortresses, less
    /// Deep Dark, taller ice spikes, and structures that settle into the land.
    /// 3: the sea at 63 (and the land with it; see `sea`). 4: Woodland
    /// Mansions in the dark forests (see mansion.rs). 5: hot springs in
    /// snowy mountains (see springs.rs).
    pub version: u8,
    /// Structures: 0 none, 1 few, 2 normal, 3 lots.
    pub structures: u8,
    /// Biome size: 0 small (as they used to be), 1 normal, 2 large, 3 huge.
    pub biome_size: u8,
    /// Terrain: 0 flat-ish, 1 normal, 2 hilly, 3 amplified.
    pub terrain: u8,
}

impl GenOptions {
    pub const LEGACY: GenOptions = GenOptions { version: 0, structures: 2, biome_size: 0, terrain: 1 };
    /// What a new world gets unless you choose otherwise.
    pub const DEFAULT: GenOptions = GenOptions { version: 5, structures: 2, biome_size: 1, terrain: 1 };

    pub fn pack(self) -> u32 {
        u32::from_le_bytes([self.structures, self.biome_size, self.terrain, self.version])
    }

    pub fn unpack(v: u32) -> GenOptions {
        let [structures, biome_size, terrain, version] = v.to_le_bytes();
        if version == 0 {
            return GenOptions::LEGACY;
        }
        GenOptions { version: version.min(5), structures: structures.min(3), biome_size: biome_size.min(3), terrain: terrain.min(3) }
    }

    /// Sea level: version 3 (the world grew to 256 tall) lifts it to 63;
    /// older worlds keep theirs, and their land with it.
    pub fn sea(self) -> i32 {
        if self.version >= 3 { NEW_SEA } else { OLD_SEA }
    }

    /// How much bigger than the old ones biomes are.
    pub fn biome_scale(self) -> f32 {
        // (From version 3 they're measured afresh, and smaller: "normal" had grown too big.)
        if self.version >= 3 {
            return [0.65, 1.0, 1.5, 2.2][self.biome_size as usize % 4];
        }
        [1.0, 2.2, 3.0, 4.5][self.biome_size as usize % 4]
    }

    /// How much taller than normal hills and mountains rise.
    pub fn relief(self) -> f32 {
        [0.35, 1.0, 1.45, 2.1][self.terrain as usize % 4]
    }

    pub fn structures_name(self) -> &'static str {
        ["None", "Few", "Normal", "Lots"][self.structures as usize % 4]
    }

    pub fn biome_name(self) -> &'static str {
        ["Small", "Normal", "Large", "Huge"][self.biome_size as usize % 4]
    }

    pub fn terrain_name(self) -> &'static str {
        ["Flat-ish", "Normal", "Hilly", "Amplified"][self.terrain as usize % 4]
    }
}

/// Pure function of the seed (and the world's options): safe to share across worker threads.
pub struct Generator {
    pub seed: u32,
    pub opts: GenOptions,
    /// Which dimension this generates (see dims.rs): it works in that
    /// dimension's generator coordinates, its own plus `dim.gen_x()`.
    pub dim: Dim,
    continent: Perlin,
    hills: Perlin,
    ridges: Perlin,
    pub(crate) temp: Perlin,
    pub(crate) moist: Perlin,
    cave_a: Perlin,
    cave_b: Perlin,
    pub(crate) cavern: Perlin,
    /// The Scorchlands' caverns (see scorch.rs).
    pub(crate) scorch: Perlin,
    /// Mod world generation, copied from the registry when the world is created.
    ores: Vec<OreGen>,
    plants: Vec<PlantGen>,
}

impl Generator {
    /// A generator with the old rules (tests).
    #[cfg(test)]
    #[cfg(test)]
    pub fn new(seed: u32) -> Self {
        Generator::with(seed, GenOptions::LEGACY)
    }

    #[cfg(test)]
    pub fn with(seed: u32, opts: GenOptions) -> Self {
        Generator::with_dim(seed, opts, Dim::Over)
    }

    pub fn with_dim(seed: u32, opts: GenOptions, dim: Dim) -> Self {
        let s = seed as u64;
        Generator {
            seed,
            opts,
            dim,
            continent: Perlin::new(s),
            hills: Perlin::new(s + 1),
            ridges: Perlin::new(s + 2),
            temp: Perlin::new(s + 3),
            moist: Perlin::new(s + 4),
            cave_a: Perlin::new(s + 5),
            cave_b: Perlin::new(s + 6),
            cavern: Perlin::new(s + 7),
            scorch: Perlin::new(s + 8),
            ores: reg().ores.clone(),
            plants: reg().plants.clone(),
        }
    }

    /// Sea level (see `GenOptions::sea`).
    pub fn sea(&self) -> i32 {
        self.opts.sea()
    }

    /// Surface height and biome for a column.
    pub fn column(&self, x: i32, z: i32) -> (i32, Biome) {
        if self.opts.version >= 3 {
            return self.column_v3(x, z);
        }
        let (fx, fz) = (x as f32, z as f32);
        // Bigger biomes come with bigger land and sea to hold them.
        let bs = self.opts.biome_scale();
        let cs = 420.0 * bs.sqrt();
        let c = self.continent.fbm2(fx / cs, fz / cs, 4);
        let hills = self.hills.fbm2(fx / 110.0, fz / 110.0, 4);
        let r = 1.0 - self.ridges.fbm2(fx / 180.0, fz / 180.0, 3).abs();
        let mount = ((c - 0.1) / 0.45).clamp(0.0, 1.0);
        let relief = self.opts.relief();
        // (Below the sea, relief only matters by half: oceans stay oceans.)
        let lift = c * 20.0 + hills * 9.0 * (0.4 + mount) + mount * r * r * 48.0;
        let lift = if lift > -3.0 { lift * relief } else { lift * (0.5 + relief * 0.5) };
        let h = self.sea() as f32 + 3.0 + lift;
        // Tall peaks ease off toward the top of the world instead of being cut flat.
        // (Heights here are from the sea, so older worlds' land stays put.)
        let knee = (self.sea() + 44) as f32;
        let h = if h > knee { knee + (h - knee) / (1.0 + (h - knee) / 24.0) } else { h };
        // The sea floor falls away offshore: shallow by the coast, deep out at sea.
        let d = self.sea() as f32 - 1.0 - h;
        let h = if d > 0.0 {
            let deep = d * 1.6 + (d - SEA_SHELF).max(0.0) * 1.4;
            // (Easing off toward the bottom, so it never reaches the world's floor.)
            self.sea() as f32 - 1.0 - deep / (1.0 + deep / 60.0)
        } else {
            h
        };
        let h = (h as i32).clamp(4, self.sea() + 68);
        let t = self.temp.fbm2(fx / (520.0 * bs) + 300.0, fz / (520.0 * bs), 3);
        let m = self.moist.fbm2(fx / (380.0 * bs), fz / (380.0 * bs) - 200.0, 3);
        // Swamps: where it's wet but not hot, the land sinks toward the sea.
        let swampy = ((m - 0.18) / 0.1).clamp(0.0, 1.0) * (1.0 - ((t - 0.15) / 0.1).clamp(0.0, 1.0)) * ((t + 0.15) / 0.1).clamp(0.0, 1.0);
        // ...to a bumpy level just under it: pools and islands of mud and grass.
        let bumps = self.sea() as f32 - 0.6 + self.hills.noise2(fx / 9.0, fz / 9.0) * 2.2;
        let h = if swampy > 0.0 && h > self.sea() - 3 { (h as f32 + (bumps - h as f32) * swampy.min(0.95)).round() as i32 } else { h };
        // Badlands: hot and dry land rises into steep, stripy hills.
        let mesa = ((t - 0.4) / 0.1).clamp(0.0, 1.0) * ((0.05 - m) / 0.1).clamp(0.0, 1.0);
        let h = if mesa > 0.0 && h > self.sea() { (h + ((h - self.sea()) as f32 * 0.9 * mesa) as i32).min(self.sea() + 68) } else { h };
        let biome = if h < self.sea() - 1 {
            // Seas by temperature.
            if t > 0.3 {
                Biome::WarmOcean
            } else if t > 0.12 {
                Biome::LukewarmOcean
            } else if t < -0.3 {
                Biome::FrozenOcean
            } else {
                Biome::Ocean
            }
        } else if h > self.sea() + 52 && t > -0.05 {
            // Mountaintops too warm for snow are bare rock.
            Biome::StonyPeaks
        } else if t < -0.3 || h > self.sea() + 52 {
            // Here and there in the snow, fields of ice spikes.
            if h <= self.sea() + 52 && self.temp.noise2(fx / 140.0 - 700.0, fz / 140.0 + 300.0) > 0.3 { Biome::IceSpikes } else { Biome::Snowy }
        } else if swampy > 0.5 && h <= self.sea() + 2 {
            Biome::Swamp
        } else if t < -0.12 && m > -0.05 {
            Biome::Taiga
        } else if t > 0.4 && m < 0.05 {
            Biome::Badlands
        } else if t > 0.25 && m < 0.05 {
            Biome::Desert
        } else if t > 0.25 && m < 0.15 {
            // Warm, but not desert-dry: grassland and acacias.
            Biome::Savanna
        } else if t > 0.15 && m > 0.15 && h <= self.sea() + 3 {
            // Where the jungle meets the sea: mangroves on the mudflats.
            Biome::Mangrove
        } else if t > 0.15 && m > 0.15 {
            Biome::Jungle
        } else if t > 0.0 && t < 0.2 && m > -0.02 && m < 0.1 && h > self.sea() + 5 {
            // Mild, slightly damp hills: cherry groves.
            Biome::Cherry
        } else if (-0.12..-0.02).contains(&t) && m > 0.14 {
            // Cool and damp: the pale forest.
            Biome::PaleGarden
        } else if (-0.02..0.15).contains(&t) && m > 0.22 {
            // Mild and wetter still: the dark forest.
            Biome::DarkForest
        } else if m > 0.08 && (0.05..0.15).contains(&t) {
            Biome::BirchForest
        } else if m > 0.08 {
            Biome::Forest
        } else if h > self.sea() + 16 && m > -0.08 && (-0.12..0.15).contains(&t) {
            // High, gentle grassland: meadows.
            Biome::Meadow
        } else {
            Biome::Plains
        };
        // Far out in deep water, now and then, the sea floor rises into a mushroom island.
        let (h, biome) = if biome.is_ocean() && h < self.sea() - 4 {
            let k = self.mushroom_isle(x, z);
            if k > 0.0 {
                // A steep shore, then a low hump up to five blocks above the sea.
                let top = self.sea() as f32 + 1.0 + (k - 0.5).max(0.0) * 10.0;
                let hh = (h as f32 + (top - h as f32) * (k * 2.0).min(1.0)).round() as i32;
                (hh, if hh >= self.sea() - 1 { Biome::MushroomIslands } else { biome })
            } else {
                (h, biome)
            }
        } else {
            (h, biome)
        };
        // Mangrove mudflats sit right at sea level, in and out of the water.
        let h = if biome == Biome::Mangrove { h.min(bumps.round() as i32) } else { h };
        (h, biome)
    }

    /// Temperature and moisture at a column, in worlds from version 3 (roughly
    /// -0.7 to 0.7 each). Their borders are bent by a little noise, so no
    /// climate edge runs in a straight line.
    pub fn climate(&self, x: i32, z: i32) -> (f32, f32) {
        let (fx, fz) = (x as f32, z as f32);
        let bs = self.opts.biome_scale();
        let warp = 26.0 * bs.sqrt();
        let (wx, wz) = (self.hills.noise2(fx / 61.0 + 410.0, fz / 61.0) * warp, self.hills.noise2(fx / 61.0, fz / 61.0 - 520.0) * warp);
        let t = self.temp.fbm2((fx + wx) / (400.0 * bs) + 300.0, (fz + wz) / (400.0 * bs), 3);
        let m = self.moist.fbm2((fx - wz) / (300.0 * bs), (fz + wx) / (300.0 * bs) - 200.0, 3);
        (t, m)
    }

    /// Surface height and biome for a column, in worlds from version 3.
    ///
    /// - Biomes come in temperature bands, so the land changes by degrees:
    ///   snow gives way to taiga before anything temperate, and the dry heat
    ///   to savanna before plains. Moisture picks within a band.
    /// - Everything that bends the land for a biome (swamps sinking to the
    ///   water, badlands rising) fades in over a wide margin, so there are no
    ///   cliffs where one biome meets another.
    fn column_v3(&self, x: i32, z: i32) -> (i32, Biome) {
        let (fx, fz) = (x as f32, z as f32);
        let sea = self.sea();
        let seaf = sea as f32;
        let bs = self.opts.biome_scale();
        // (Land and sea keep the size they had: only the biomes on them got smaller.)
        let cs = 620.0 * bs.sqrt();
        let c = self.continent.fbm2(fx / cs, fz / cs, 4);
        let hills = self.hills.fbm2(fx / 110.0, fz / 110.0, 4);
        let r = 1.0 - self.ridges.fbm2(fx / 180.0, fz / 180.0, 3).abs();
        let mount = ((c - 0.1) / 0.45).clamp(0.0, 1.0);
        let relief = self.opts.relief();
        let lift = c * 20.0 + hills * 9.0 * (0.4 + mount) + mount * r * r * 48.0;
        let lift = if lift > -3.0 { lift * relief } else { lift * (0.5 + relief * 0.5) };
        let mut h = seaf + 3.0 + lift;
        let knee = seaf + 44.0;
        if h > knee {
            h = knee + (h - knee) / (1.0 + (h - knee) / 24.0);
        }
        let d = seaf - 1.0 - h;
        if d > 0.0 {
            let deep = d * 1.6 + (d - SEA_SHELF).max(0.0) * 1.4;
            h = seaf - 1.0 - deep / (1.0 + deep / 60.0);
        }
        let (t, m) = self.climate(x, z);
        // Swamps: wet and mild. The land sinks toward a bumpy level just under
        // the sea, fading in with the wet, and not at all on high ground or
        // out at sea (so it never makes a step).
        let swampy = smoothstep(0.12, 0.3, m) * smoothstep(-0.28, -0.12, t) * (1.0 - smoothstep(0.1, 0.26, t));
        let bumps = seaf - 0.6 + self.hills.noise2(fx / 9.0, fz / 9.0) * 2.2;
        let low = (1.0 - smoothstep(seaf + 4.0, seaf + 26.0, h)) * smoothstep(seaf - 14.0, seaf - 3.0, h);
        h += (bumps - h) * swampy * low * 0.95;
        // Mangroves: the same where it's wet and hot (mudflats, in and out of the water).
        let marsh = smoothstep(0.1, 0.24, m) * smoothstep(0.16, 0.28, t);
        h += (bumps - h) * marsh * low * 0.9;
        // Badlands: hot and dry land rises into steep hills (by how high it already is).
        let mesa = smoothstep(0.3, 0.46, t) * (1.0 - smoothstep(-0.08, 0.06, m));
        h += (h - seaf).max(0.0) * 0.9 * mesa;
        let h = (h.round() as i32).clamp(4, sea + 68);
        let biome = if h < sea - 1 {
            if t > 0.3 {
                Biome::WarmOcean
            } else if t > 0.12 {
                Biome::LukewarmOcean
            } else if t < -0.32 {
                Biome::FrozenOcean
            } else {
                Biome::Ocean
            }
        } else if h > sea + 52 {
            if t > -0.05 { Biome::StonyPeaks } else { Biome::Snowy }
        } else if t < -0.32 {
            // Frozen.
            if self.temp.noise2(fx / 140.0 - 700.0, fz / 140.0 + 300.0) > 0.3 { Biome::IceSpikes } else { Biome::Snowy }
        } else if t < -0.17 {
            // Cold: always taiga, between the snow and everything milder.
            Biome::Taiga
        } else if t < 0.22 {
            // Temperate.
            if swampy > 0.5 && h <= sea + 2 {
                Biome::Swamp
            } else if m > 0.24 && t > -0.02 {
                Biome::DarkForest
            } else if m > 0.16 && t < -0.04 {
                Biome::PaleGarden
            } else if m > 0.06 && (0.04..0.16).contains(&t) {
                Biome::BirchForest
            } else if m > 0.06 {
                Biome::Forest
            } else if (0.02..0.18).contains(&t) && (-0.04..0.06).contains(&m) && h > sea + 5 {
                Biome::Cherry
            } else if h > sea + 16 && m > -0.1 {
                Biome::Meadow
            } else {
                Biome::Plains
            }
        } else if t < 0.36 {
            // Warm: savanna between the temperate land and the heat, or jungle where it's wet.
            if m > 0.16 {
                if h <= sea + 1 && marsh > 0.3 { Biome::Mangrove } else { Biome::Jungle }
            } else {
                Biome::Savanna
            }
        } else if m > 0.12 {
            if h <= sea + 1 && marsh > 0.3 { Biome::Mangrove } else { Biome::Jungle }
        } else if mesa > 0.5 {
            Biome::Badlands
        } else {
            Biome::Desert
        };
        // Far out in deep water, now and then, the sea floor rises into a mushroom island.
        let (h, biome) = if biome.is_ocean() && h < sea - 4 {
            let k = self.mushroom_isle(x, z);
            if k > 0.0 {
                let top = seaf + 1.0 + (k - 0.5).max(0.0) * 10.0;
                let hh = (h as f32 + (top - h as f32) * (k * 2.0).min(1.0)).round() as i32;
                (hh, if hh >= sea - 1 { Biome::MushroomIslands } else { biome })
            } else {
                (h, biome)
            }
        } else {
            (h, biome)
        };
        (h, biome)
    }

    /// The biome whose ground covers a column: its own, but near a border
    /// sometimes the neighbour's, so edges come out ragged rather than ruled
    /// (worlds from version 3; older ones use the column's own).
    pub fn ground_biome(&self, x: i32, z: i32, own: Biome) -> Biome {
        if self.opts.version < 3 {
            return own;
        }
        let jx = ((hash2(self.seed ^ 0xB1E, x, z) - 0.5) * 9.0) as i32;
        let jz = ((hash2(self.seed ^ 0xB1F, x, z) - 0.5) * 9.0) as i32;
        let other = self.column(x + jx, z + jz).1;
        // (Only between land biomes: the shore stays where the water is.)
        if other.is_ocean() || own.is_ocean() || matches!(other, Biome::StonyPeaks | Biome::MushroomIslands) || matches!(own, Biome::StonyPeaks | Biome::MushroomIslands) { own } else { other }
    }

    /// Cold enough for the sea to freeze (the same temperature that makes snowy biomes).
    pub fn cold(&self, x: i32, z: i32) -> bool {
        if self.opts.version >= 3 {
            return self.climate(x, z).0 < -0.32;
        }
        let bs = self.opts.biome_scale();
        self.temp.fbm2(x as f32 / (520.0 * bs) + 300.0, z as f32 / (520.0 * bs), 3) < -0.3
    }

    /// Where the generator put Creaking Hearts in chunk (cx, cz)'s pale oaks.
    pub fn creaking_hearts(&self, cx: i32, cz: i32) -> Vec<IVec3> {
        let s = self.seed;
        let mut v = Vec::new();
        for tz in cz * CW..cz * CW + CW {
            for tx in cx * CW..cx * CW + CW {
                if let Some((h, _, TreeKind::PaleOak)) = self.tree_at(tx, tz)
                    && hash3(s ^ 0x1EA, tx, h - 64, tz) < 0.12
                {
                    v.push(ivec3(tx, h + 3, tz));
                }
            }
        }
        v
    }

    /// A tree rooted at this column: ground height, trunk height and kind.
    pub fn tree_at(&self, x: i32, z: i32) -> Option<(i32, i32, TreeKind)> {
        let (h, biome) = self.column(x, z);
        let density = match biome {
            Biome::Forest => 0.035,
            Biome::Plains => 0.004,
            Biome::Snowy => 0.012,
            Biome::Taiga => 0.03,
            Biome::Jungle => 0.045,
            Biome::Swamp => 0.012,
            Biome::Cherry => 0.02,
            Biome::Mangrove => 0.016,
            Biome::PaleGarden => 0.04,
            Biome::Savanna => 0.006,
            Biome::BirchForest => 0.035,
            Biome::DarkForest => 0.07,
            Biome::MushroomIslands => 0.008,
            Biome::Meadow => 0.0015,
            _ => 0.0,
        };
        // Swamp trees and mangroves stand in the shallows too.
        let ground = if matches!(biome, Biome::Swamp | Biome::Mangrove) { self.sea() - 2 } else { self.sea() + 1 };
        if h <= ground || hash2(self.seed ^ 0x7EE, x, z) >= density {
            return None;
        }
        let kind = match biome.tree() {
            // Half the giant mushrooms are brown; dark forests grow the odd one too.
            TreeKind::RedMushroom if hash2(self.seed ^ 0x3B0, x, z) < 0.5 => TreeKind::BrownMushroom,
            TreeKind::DarkOak if hash2(self.seed ^ 0x3B1, x, z) < 0.08 => [TreeKind::RedMushroom, TreeKind::BrownMushroom][(hash2(self.seed ^ 0x3B2, x, z) * 2.0) as usize % 2],
            // A meadow's trees are oaks as often as birches.
            TreeKind::Birch if biome == Biome::Meadow && hash2(self.seed ^ 0x3B3, x, z) < 0.5 => TreeKind::Oak,
            k => k,
        };
        Some((h, kind.trunk(hash2(self.seed ^ 0x7E1, x, z)), kind))
    }

    /// Ravines: long, narrow cracks down from the surface. Returns the bottom
    /// of the crack at this column, if one runs through it.
    pub fn ravine_floor(&self, x: i32, z: i32) -> Option<i32> {
        const REGION: i32 = 96;
        let (rx, rz) = (x.div_euclid(REGION), z.div_euclid(REGION));
        let s = self.seed ^ 0x2A_71E;
        let mut best: Option<i32> = None;
        for dz in -1..=1 {
            for dx in -1..=1 {
                let (gx, gz) = (rx + dx, rz + dz);
                if hash2(s, gx, gz) > 0.35 {
                    continue;
                }
                let start = Vec3::new((gx * REGION) as f32 + hash2(s ^ 1, gx, gz) * REGION as f32, 0.0, (gz * REGION) as f32 + hash2(s ^ 2, gx, gz) * REGION as f32);
                let a = hash2(s ^ 3, gx, gz) * std::f32::consts::TAU;
                let len = 50.0 + hash2(s ^ 4, gx, gz) * 40.0;
                let dir = Vec3::new(a.cos(), 0.0, a.sin());
                let rel = Vec3::new(x as f32 + 0.5, 0.0, z as f32 + 0.5) - start;
                let t = rel.dot(dir) / len;
                if !(0.0..=1.0).contains(&t) {
                    continue;
                }
                let dist = (rel - dir * (t * len)).length();
                let bulge = (t * std::f32::consts::PI).sin();
                let width = 0.6 + 3.0 * bulge;
                if dist >= width {
                    continue;
                }
                let floor = 10 + (8.0 * (1.0 - bulge) + (dist / width).powi(2) * 10.0) as i32;
                best = Some(best.map_or(floor, |b| b.min(floor)));
            }
        }
        best
    }

    fn is_cave(&self, x: i32, y: i32, z: i32, surface: i32) -> bool {
        if y <= 1 {
            return false;
        }
        // Keep the sea floor sealed so oceans don't pour into nowhere.
        if surface <= self.sea() + 1 && y >= surface - 5 {
            return false;
        }
        let (fx, fy, fz) = (x as f32, y as f32, z as f32);
        let a = self.cave_a.noise3(fx / 30.0, fy / 20.0, fz / 30.0);
        let b = self.cave_b.noise3(fx / 30.0, fy / 20.0, fz / 30.0);
        if a.abs() < 0.075 && b.abs() < 0.075 {
            return true;
        }
        y < 36 && self.cavern.noise3(fx / 55.0, fy / 28.0, fz / 55.0) > 0.42
    }

    /// Chunk (cx, cz) of this generator's dimension (in its own coordinates).
    pub fn generate(&self, cx: i32, cz: i32) -> Vec<Id> {
        match self.dim {
            Dim::Over => self.generate_over(cx, cz),
            // (The other dimensions are generated in generator coordinates; see dims.rs.)
            Dim::Scorch => self.generate_scorch(cx + self.dim.gen_cx(), cz),
            Dim::Hollow => self.generate_hollow(cx + self.dim.gen_cx(), cz),
        }
    }

    fn generate_over(&self, cx: i32, cz: i32) -> Vec<Id> {
        let mut b = vec![AIR; CHUNK_VOL];
        let s = self.seed;
        let mut cols = [(0i32, Biome::Plains); 256];
        let geodes = self.geodes_for_chunk(cx, cz);
        for lz in 0..CW {
            for lx in 0..CW {
                let (x, z) = (cx * CW + lx, cz * CW + lz);
                let (h, biome) = self.column(x, z);
                cols[(lz * CW + lx) as usize] = (h, biome);
                // (What the ground is made of: ragged at the borders; see `ground_biome`.)
                let biome = self.ground_biome(x, z, biome);
                let beach = (self.sea() - 1..=self.sea() + 1).contains(&h) && !matches!(biome, Biome::Snowy | Biome::Swamp | Biome::Badlands | Biome::MushroomIslands | Biome::IceSpikes | Biome::StonyPeaks);
                let peaks = biome == Biome::StonyPeaks;
                // (Mangrove mudflats are muddier swamps.)
                let swamp = matches!(biome, Biome::Swamp | Biome::Mangrove);
                let mangrove = biome == Biome::Mangrove;
                let badlands = biome == Biome::Badlands;
                // Steep badlands slopes show their stripes; flatter ground is red sand.
                let steep = badlands && [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|&(dx, dz)| (self.column(x + dx, z + dz).0 - h).abs() >= 2);
                // Badlands stripes wobble a little up and down across the land.
                let wobble = (self.hills.noise2(x as f32 / 40.0, z as f32 / 40.0) * 3.0) as i32;
                for y in 0..CH.min(h.max(self.sea()) + 1) {
                    let id = if y == 0 || (y <= 2 && hash3(s, x, y, z) < 0.5) {
                        BEDROCK
                    } else if badlands && y >= h - 14 && (y < h || (y == h && steep)) {
                        // Bands of terracotta.
                        TERRACOTTA + [0, 1, 0, 2, 3, 0, 1, 2][((y + wobble).rem_euclid(24) / 3) as usize] as Id
                    } else if y < h - 3 {
                        // Deserts sit on a few layers of sandstone.
                        if biome == Biome::Desert && y >= h - 8 { SANDSTONE } else { STONE }
                    } else if y < h {
                        if biome == Biome::Desert || beach { SAND } else if swamp && hash2(s ^ 0x3D, x, z) < 0.5 { MUD } else if peaks { STONE } else { DIRT }
                    } else if y == h && h >= self.sea() - 1 && matches!(biome, Biome::StonyPeaks | Biome::MushroomIslands | Biome::IceSpikes) {
                        match biome {
                            // Bare rock, with patches of gravel and calcite.
                            Biome::StonyPeaks => {
                                let r = hash2(s ^ 0x57E, x >> 2, z >> 2);
                                if r < 0.15 { GRAVEL } else if r < 0.25 { CALCITE } else { STONE }
                            }
                            Biome::MushroomIslands => MYCELIUM,
                            _ => SNOW_BLOCK,
                        }
                    } else if y == h {
                        if swamp {
                            let mud = if mangrove { 0.65 } else { 0.3 };
                            if hash2(s ^ 0x3D, x, z) < mud { MUD } else if h < self.sea() { DIRT } else { GRASS }
                        } else if h < self.sea() - 1 {
                            if hash2(s ^ 0x6A, x, z) < 0.3 { GRAVEL } else if h > self.sea() - 6 { SAND } else { DIRT }
                        } else if badlands && !steep {
                            RED_SAND
                        } else if biome == Biome::Desert || beach {
                            SAND
                        } else if biome == Biome::Snowy {
                            SNOW_GRASS
                        } else {
                            GRASS
                        }
                    } else {
                        WATER
                    };
                    b[idx(lx, y, lz)] = id;
                }
                // Caves, ravines and ores
                let ravine = if h > self.sea() + 2 && !biome.is_ocean() { self.ravine_floor(x, z) } else { None };
                for y in 1..h + 1 {
                    let i = idx(lx, y, lz);
                    if b[i] == BEDROCK {
                        continue;
                    }
                    if self.is_cave(x, y, z, h) || ravine.is_some_and(|f| y >= f) {
                        // The deepest caverns are lakes of lava.
                        b[i] = if y <= 10 && self.cavern.noise3(x as f32 / 55.0, y as f32 / 28.0, z as f32 / 55.0) > 0.42 { LAVA } else { AIR };
                        continue;
                    }
                    if b[i] == STONE {
                        let r = hash3(s ^ 0x0E, x >> 1, y >> 1, z >> 1);
                        let r2 = hash3(s ^ 0x0F, x, y, z);
                        if r < 0.012 && r2 < 0.7 {
                            b[i] = COAL_ORE;
                        } else if y < 64 && (0.012..0.019).contains(&r) && r2 < 0.6 {
                            b[i] = IRON_ORE;
                        } else if y < 18 && (0.019..0.0215).contains(&r) && r2 < 0.5 {
                            b[i] = DIAMOND_ORE;
                        } else if (y < 32 || (badlands && y < 70)) && (0.0215..0.0245).contains(&r) && r2 < 0.55 {
                            b[i] = GOLD_ORE;
                        } else if y < 16 && (0.0245..0.029).contains(&r) && r2 < 0.55 {
                            b[i] = ZAP_ORE;
                        } else if (16..72).contains(&y) && (0.029..0.037).contains(&r) && r2 < 0.6 {
                            b[i] = COPPER_ORE;
                        }
                    }
                    for (oi, ore) in self.ores.iter().enumerate() {
                        if b[i] == ore.replace && (ore.min_y..=ore.max_y).contains(&y) {
                            let salt = 0x5100 + oi as u32 * 7;
                            // Small 2x2x2-ish veins; roughly `chance` of the eligible blocks.
                            if hash3(s ^ salt, x >> 1, y >> 1, z >> 1) < ore.chance * 3.0 && hash3(s ^ (salt + 1), x, y, z) < 0.4 {
                                b[i] = ore.block;
                                break;
                            }
                        }
                    }
                }
                // The Deep Dark (see deepdark.rs): deepslate, wide caverns, sculk that listens.
                if h > crate::deepdark::DEEP_TOP + 6 && self.deep_dark(x, z) {
                    let top = crate::deepdark::DEEP_TOP;
                    for y in 1..top {
                        let i = idx(lx, y, lz);
                        match b[i] {
                            BEDROCK => continue,
                            STONE => b[i] = DEEPSLATE,
                            LAVA => b[i] = AIR,
                            _ => {}
                        }
                        if self.deep_cavern(x, y, z) {
                            b[i] = AIR;
                        }
                    }
                    for y in 2..top {
                        let (i, below) = (idx(lx, y, lz), idx(lx, y - 1, lz));
                        if b[i] == AIR && matches!(b[below], DEEPSLATE | STONE) && self.sculk_patch(x, y, z) {
                            b[below] = SCULK;
                            if let Some(f) = crate::deepdark::sculk_feature(hash3(s ^ 0x5C, x, y, z)) {
                                b[i] = f;
                            }
                        }
                    }
                }
                // Cave biomes and geodes (see caves.rs), above the Deep Dark.
                let deep = h > crate::deepdark::DEEP_TOP + 6 && self.deep_dark(x, z);
                self.cave_column(&mut b, (lx, lz), (x, z), h, if deep { crate::deepdark::DEEP_TOP + 2 } else { 3 });
                if !geodes.is_empty() {
                    self.geode_column(&mut b, lx, lz, x, z, &geodes);
                }
                // Cave decorations: glowing mushrooms and pointy rocks on floors, pointy rocks on ceilings.
                for y in 3..(h - 4).max(3) {
                    if b[idx(lx, y, lz)] != AIR {
                        continue;
                    }
                    let (below, above) = (b[idx(lx, y - 1, lz)], b[idx(lx, y + 1, lz)]);
                    let r = hash3(s ^ 0xCA7E, x, y, z);
                    // Pointy rocks hang from cave floors (below==STONE) and ceilings (above==STONE); both intentionally place POINTY_ROCK.
                    #[allow(clippy::if_same_then_else)]
                    if below == STONE && r < 0.012 {
                        b[idx(lx, y, lz)] = GLOWSHROOM;
                    } else if below == STONE && r < 0.03 {
                        b[idx(lx, y, lz)] = POINTY_ROCK;
                    } else if above == STONE && r > 0.975 {
                        b[idx(lx, y, lz)] = POINTY_ROCK;
                    }
                }
                // Plants on grass
                let top = h + 1;
                if top < CH && b[idx(lx, h, lz)] == GRASS && b[idx(lx, top, lz)] == AIR {
                    let r = hash2(s ^ 0xF10, x, z);
                    // Meadows: patches of plains and forest thick with flowers (bees love them).
                    let meadow = biome == Biome::Meadow || (matches!(biome, Biome::Plains | Biome::Forest) && hash2(s ^ 0xF12, x >> 4, z >> 4) < 0.15);
                    if biome == Biome::PaleGarden {
                        // All grey: pale moss everywhere, nothing green or bright on
                        // it but the odd Eyeblossom (open at night).
                        b[idx(lx, h, lz)] = PALE_MOSS;
                        if r < 0.03 {
                            b[idx(lx, top, lz)] = EYEBLOSSOM;
                        }
                    } else if r < 0.012 || (meadow && r < 0.07) {
                        b[idx(lx, top, lz)] = flower_for(biome, hash2(s ^ 0xF11, x >> 1, z >> 1));
                    } else if meadow && r < 0.16 {
                        // Wildflowers between the flowers.
                        b[idx(lx, top, lz)] = WILDFLOWERS;
                    } else if matches!(biome, Biome::Swamp | Biome::Mangrove) && r < 0.02 {
                        // Where the fireflies live.
                        b[idx(lx, top, lz)] = FIREFLY_BUSH;
                    } else if biome == Biome::Cherry && r < 0.4 {
                        // Petals everywhere under the cherry trees.
                        b[idx(lx, top, lz)] = PINK_PETALS;
                    } else if r < 0.11 || (biome == Biome::Savanna && r < 0.24) {
                        // (Savannas are long grass all over.)
                        b[idx(lx, top, lz)] = TALL_GRASS;
                    } else if r < 0.1125 && biome == Biome::Plains {
                        b[idx(lx, top, lz)] = PUMPKIN;
                    } else if r < 0.125 && matches!(biome, Biome::Forest | Biome::Taiga) {
                        b[idx(lx, top, lz)] = MUSHROOM;
                    } else if r < 0.3 && matches!(biome, Biome::Forest | Biome::Taiga) && hash2(s ^ 0x1EAF, x >> 2, z >> 2) < 0.45 {
                        // Drifts of fallen leaves under the trees.
                        b[idx(lx, top, lz)] = LEAF_LITTER;
                    } else if r < 0.2 && biome == Biome::Jungle {
                        // Jungles are thick with undergrowth.
                        b[idx(lx, top, lz)] = TALL_GRASS;
                    } else if r < 0.206 && biome == Biome::Jungle {
                        b[idx(lx, top, lz)] = MELON;
                    } else if biome == Biome::Jungle && hash2(s ^ 0xBA3B, x >> 3, z >> 3) < 0.35 && r < 0.32 {
                        // Bamboo grows in groves.
                        let tall = 4 + (hash2(s ^ 0xBA3C, x, z) * 9.0) as i32;
                        for y in top..(top + tall).min(CH - 1) {
                            b[idx(lx, y, lz)] = BAMBOO;
                        }
                    }
                }
                // Mushrooms on mycelium.
                if top < CH && b[idx(lx, h, lz)] == MYCELIUM && b[idx(lx, top, lz)] == AIR && hash2(s ^ 0x3C0, x, z) < 0.04 {
                    b[idx(lx, top, lz)] = MUSHROOM;
                }
                // Ice spikes: cones of packed ice, now and then a very tall one.
                if biome == Biome::IceSpikes {
                    let spike = self.ice_spike(x, z);
                    for y in top..(top + spike).min(CH - 2) {
                        b[idx(lx, y, lz)] = PACKED_ICE;
                    }
                }
                // Coral reefs on warm, shallow sea floors (and a little in lukewarm ones).
                let reefs = match biome {
                    Biome::WarmOcean => 0.5,
                    Biome::LukewarmOcean => 0.12,
                    _ => 0.0,
                };
                let reef = (self.sea() - 18..self.sea() - 3).contains(&h) && hash2(s ^ 0xC0A1, x >> 3, z >> 3) < reefs && hash2(s ^ 0xC0A2, x, z) < 0.75;
                if !reef && biome.is_ocean() && h < self.sea() - 1 && b[idx(lx, h + 1, lz)] == WATER {
                    // Kelp, seagrass and sea pickles on the rest of the sea floor (see seas.rs).
                    if let Some((plant, tall)) = crate::seas::floor_plant(self, biome, x, z, self.sea() - h) {
                        for y in h + 1..=h + tall {
                            b[idx(lx, y, lz)] = plant;
                        }
                    }
                }
                // Frozen seas: icebergs of packed ice, snow on top.
                if biome == Biome::FrozenOcean {
                    let berg = self.iceberg(x, z);
                    if berg > 0 {
                        for y in (self.sea() - berg / 2).max(h + 1)..=self.sea() + berg {
                            b[idx(lx, y, lz)] = PACKED_ICE;
                        }
                        b[idx(lx, self.sea() + berg + 1, lz)] = SNOW_BLOCK;
                    }
                }
                if reef {
                    let kind = CORAL_FIRST + (hash2(s ^ 0xC0A3, x >> 1, z >> 1) * 4.0) as Id;
                    b[idx(lx, h, lz)] = kind;
                    // Knobbly: some reach up a block or two.
                    let up = (hash2(s ^ 0xC0A4, x, z) * 3.0) as i32;
                    for y in h + 1..=h + up {
                        if b[idx(lx, y, lz)] == WATER {
                            b[idx(lx, y, lz)] = kind;
                        }
                    }
                }
                // Dead bushes on red sand; lily pads on swamp water.
                if top < CH && badlands && b[idx(lx, h, lz)] == RED_SAND && b[idx(lx, top, lz)] == AIR && hash2(s ^ 0xDB, x, z) < 0.02 {
                    b[idx(lx, top, lz)] = DEAD_BUSH;
                }
                if swamp && h < self.sea() && b[idx(lx, self.sea(), lz)] == WATER && hash2(s ^ 0x111, x, z) < 0.08 {
                    b[idx(lx, self.sea() + 1, lz)] = LILY_PAD;
                }
                // Pokey Plants in the desert (and badlands), 1-3 tall (on dry land only).
                if biome.dry() && top + 3 < CH && matches!(b[idx(lx, h, lz)], SAND | RED_SAND) && b[idx(lx, top, lz)] == AIR && hash2(s ^ 0xCAC, x, z) < 0.005 {
                    let tall = 1 + (hash2(s ^ 0xCAD, x, z) * 3.0) as i32;
                    for y in top..top + tall.min(3) {
                        b[idx(lx, y, lz)] = CACTUS;
                    }
                }
                // Cold seas freeze over.
                if h < self.sea() && b[idx(lx, self.sea(), lz)] == WATER && self.cold(x, z) {
                    b[idx(lx, self.sea(), lz)] = ICE;
                }
                if top < CH && b[idx(lx, top, lz)] == AIR {
                    let below = b[idx(lx, h, lz)];
                    for (pi, p) in self.plants.iter().enumerate() {
                        if below == p.on && hash2(s ^ (0x9100 + pi as u32 * 13), x, z) < p.chance {
                            b[idx(lx, top, lz)] = p.block;
                            break;
                        }
                    }
                }
            }
        }
        // Trees may overhang from neighbouring chunks, so scan a margin around this one.
        for tz in cz * CW - 4..cz * CW + CW + 4 {
            for tx in cx * CW - 4..cx * CW + CW + 4 {
                let Some((h, trunk, kind)) = self.tree_at(tx, tz) else { continue };
                let base = ivec3(tx, h, tz);
                for (o, id, force) in kind.shape(trunk, |o| hash3(s ^ 0x1EA, base.x + o.x, base.y + o.y, base.z + o.z)) {
                    let p = base + o;
                    let (lx, lz) = (p.x - cx * CW, p.z - cz * CW);
                    if !(0..CW).contains(&lx) || !(0..CW).contains(&lz) || !(0..CH).contains(&p.y) {
                        continue;
                    }
                    let i = idx(lx, p.y, lz);
                    // Mangrove roots in the water (or at the water's edge) are waterlogged.
                    let id = if id == MANGROVE_ROOTS && (is_water(b[i]) || (p.y <= self.sea() && [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|&(dx, dz)| self.column(p.x + dx, p.z + dz).0 < p.y))) {
                        MANGROVE_ROOTS_WET
                    } else {
                        id
                    };
                    if force || matches!(b[i], AIR | TALL_GRASS | FLOWER | LILY_PAD | PINK_PETALS | PALE_HANGING_MOSS | LEAF_LITTER | WILDFLOWERS | EYEBLOSSOM | FIREFLY_BUSH) {
                        b[i] = id;
                    }
                }
                // Some oaks in flowery places have a bee nest hanging off the trunk.
                if kind == TreeKind::Oak && hash2(s ^ 0xBEE, tx, tz) < 0.07 && matches!(self.column(tx, tz).1, Biome::Plains | Biome::Forest) {
                    let side = [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z][(hash2(s ^ 0xBEF, tx, tz) * 4.0) as usize % 4];
                    // Just under the canopy, where you can see it.
                    let p = base + side + IVec3::Y * (trunk - 3).max(1);
                    let (lx, lz) = (p.x - cx * CW, p.z - cz * CW);
                    if (0..CW).contains(&lx) && (0..CW).contains(&lz) && (0..CH).contains(&p.y) && matches!(b[idx(lx, p.y, lz)], AIR | LEAVES) {
                        b[idx(lx, p.y, lz)] = BEE_NEST;
                    }
                }
                // Roots: dirt under the trunk (swamp trees stand in water on it).
                let (lx, lz) = (tx - cx * CW, tz - cz * CW);
                if (0..CW).contains(&lx) && (0..CW).contains(&lz) {
                    b[idx(lx, h, lz)] = DIRT;
                }
            }
        }
        self.place_structures(cx, cz, &mut b);
        self.place_crypts(cx, cz, &mut b);
        b
    }
}

impl Generator {
    /// How far into a mushroom island this column is (0: not at all, 1: its
    /// middle). They're rare: a few in every thousand blocks of sea, each one
    /// a lumpy round island 50 to 100 blocks across.
    pub fn mushroom_isle(&self, x: i32, z: i32) -> f32 {
        const GRID: i32 = 640;
        let s = self.seed ^ 0x15E;
        let (gx, gz) = (x.div_euclid(GRID), z.div_euclid(GRID));
        let mut best: f32 = 0.0;
        for dz in -1..=1 {
            for dx in -1..=1 {
                let (cx, cz) = (gx + dx, gz + dz);
                if hash2(s, cx, cz) > 0.4 {
                    continue;
                }
                let mid = (cx * GRID + 100 + (hash2(s ^ 1, cx, cz) * 440.0) as i32, cz * GRID + 100 + (hash2(s ^ 2, cx, cz) * 440.0) as i32);
                let radius = 25.0 + hash2(s ^ 3, cx, cz) * 25.0;
                let d = (((x - mid.0).pow(2) + (z - mid.1).pow(2)) as f32).sqrt();
                // (Lumpy edges.)
                let lumps = self.hills.noise2(x as f32 / 14.0 + 77.0, z as f32 / 14.0) * 8.0;
                best = best.max(1.0 - (d + lumps) / radius);
            }
        }
        best.max(0.0)
    }

    /// How tall an ice spike stands at this column (0: none). Spikes stand
    /// on a grid, one per 12 blocks or so, each a cone from its middle.
    pub fn ice_spike(&self, x: i32, z: i32) -> i32 {
        const GRID: i32 = 12;
        let s = self.seed ^ 0x1CE5;
        let (gx, gz) = (x.div_euclid(GRID), z.div_euclid(GRID));
        let mut best = 0;
        for dz in -1..=1 {
            for dx in -1..=1 {
                let (cx, cz) = (gx + dx, gz + dz);
                if hash2(s, cx, cz) > 0.55 {
                    continue;
                }
                let mid = (cx * GRID + 2 + (hash2(s ^ 1, cx, cz) * 8.0) as i32, cz * GRID + 2 + (hash2(s ^ 2, cx, cz) * 8.0) as i32);
                let d = ((x - mid.0).pow(2) + (z - mid.1).pow(2)) as f32;
                let tall = hash2(s ^ 3, cx, cz) < 0.15;
                let here = if self.opts.version >= 2 {
                    // Taller, with a broad foot and a slender point (thinner the higher it goes).
                    let (height, width) = if tall { (30.0 + hash2(s ^ 4, cx, cz) * 18.0, 3.0) } else { (10.0 + hash2(s ^ 4, cx, cz) * 12.0, 3.4) };
                    height * (1.0 - d.sqrt() / width).max(0.0).powf(1.8)
                } else {
                    let (height, width) = if tall { (24.0 + hash2(s ^ 4, cx, cz) * 12.0, 1.6) } else { (6.0 + hash2(s ^ 4, cx, cz) * 8.0, 2.4) };
                    height * (1.0 - d.sqrt() / width).max(0.0)
                };
                if here >= 1.0 && self.column(mid.0, mid.1).1 == Biome::IceSpikes {
                    best = best.max(here as i32);
                }
            }
        }
        best
    }
}

/// Which flower grows here, from a 0..1 roll (patchy, so flowers come in clumps).
pub fn flower_for(biome: Biome, r: f32) -> Id {
    let list: &[Id] = match biome {
        Biome::Plains => &[FLOWER, DANDELION, DANDELION, CORNFLOWER, LAVENDER],
        Biome::Forest => &[FLOWER, DANDELION, LAVENDER],
        Biome::Taiga => &[CORNFLOWER, LAVENDER],
        Biome::Swamp => &[CORNFLOWER, FLOWER],
        Biome::Cherry => &[FLOWER, LAVENDER, LAVENDER],
        Biome::Meadow => &[CORNFLOWER, LAVENDER, DANDELION, FLOWER, CORNFLOWER],
        Biome::BirchForest => &[FLOWER, DANDELION, CORNFLOWER],
        Biome::Savanna => &[DANDELION],
        _ => &[FLOWER, DANDELION],
    };
    list[(r * list.len() as f32) as usize % list.len()]
}

pub struct Hit {
    pub pos: IVec3,
    pub normal: IVec3,
    pub dist: f32,
}

/// A chunk from a generator thread: where, its blocks, and its light on its own.
type Generated = (i32, i32, PalettedBlocks, crate::light::LightStore);

pub struct World {
    pub generator: Arc<Generator>,
    pub chunks: HashMap<(i32, i32), Chunk>,
    /// Player edits, keyed by chunk then block index. Re-applied when chunks
    /// regenerate. With region files, only the regions near someone are here.
    pub mods: HashMap<(i32, i32), HashMap<u32, Id>>,
    /// Where edits are kept on disk, if anywhere (see regions.rs).
    pub regions: Option<crate::regions::Regions>,
    /// Chunks whose mesh is stale.
    pub dirty: HashSet<(i32, i32)>,
    pending: HashSet<(i32, i32)>,
    /// Generated chunks waiting for their region file to be read.
    waiting: Vec<Generated>,
    /// Soil records for every tilled block (see farming.rs); kept in step with the blocks.
    pub farm: HashMap<IVec3, Soil>,
    /// What's inside every chest and furnace (see containers.rs); kept in step with the blocks.
    pub containers: HashMap<IVec3, Container>,
    /// Each player's Personal Chest storage, by `stash::stash_key` (see stash.rs).
    pub stashes: HashMap<i32, Container>,
    /// Everyone's backpack packs, by player (see backpacks.rs).
    pub backpacks: HashMap<i32, Container>,
    /// Llamas' packs, by mob id (see wildlife.rs).
    pub packs: HashMap<u32, Container>,
    /// Fill structure chests when their chunks first arrive (off for joined
    /// players: the host has the real contents).
    pub structure_loot: bool,
    /// Cells where water or lava may need to move (see liquids.rs). Only
    /// collected where the world lives (`simulate_liquids`).
    pub liquid_dirty: HashSet<IVec3>,
    pub simulate_liquids: bool,
    /// Cells where Zappy Dust contraptions may need updating (see wiring.rs); same rules.
    pub zap_dirty: HashSet<IVec3>,
    /// What's written on every sign, and what hangs in every item frame (see decor.rs).
    pub signs: HashMap<IVec3, [String; 4]>,
    /// Signs' colours and glow (see qol.rs).
    pub sign_styles: HashMap<IVec3, u8>,
    pub frames: HashMap<IVec3, (Id, crate::inventory::Wear)>,
    /// Huts whose chests were just filled for the first time: a Hmmer should
    /// move in (where to stand, and its seed; see villagers.rs).
    pub new_huts: Vec<(Vec3, u32)>,
    /// Villages whose square chest was just filled: a Clanker should move in (where).
    pub new_clankers: Vec<Vec3>,
    /// Mineshafts' loot carts, and who lives in igloo basements, waiting to move in (see temples.rs).
    pub new_carts: Vec<(Vec3, crate::containers::Container)>,
    pub new_residents: Vec<(Vec3, crate::entity::MobKind)>,
    /// Every sapling in loaded or edited chunks, and leaves that should check
    /// whether they still hang on to a tree (see trees.rs).
    pub saplings: HashSet<IVec3>,
    /// Cells to check for sand and gravel with nothing under them (see falling.rs).
    pub fall_dirty: HashSet<IVec3>,
    /// Jukeboxes (to find the nearest one playing; see music.rs).
    pub jukeboxes: HashSet<IVec3>,
    /// Sizzler Cages (see fortress.rs).
    pub cages: HashSet<IVec3>,
    /// Every fire burning (see fire.rs).
    pub fires: HashSet<IVec3>,
    /// Every comparator (they watch containers; see contraptions.rs).
    pub comparators: HashSet<IVec3>,
    /// Every beacon (see beacon.rs).
    pub beacons: HashSet<IVec3>,
    /// Conduits in loaded chunks (see monument.rs).
    pub conduits: HashSet<IVec3>,
    /// Every cell of Spelunker's Rope (see rope.rs), for maps.
    pub ropes: HashSet<IVec3>,
    /// Lightning rods (see homecraft.rs), and Cooking Pots (cooking.rs, for their steam).
    pub rods: HashSet<IVec3>,
    pub cook_pots: HashSet<IVec3>,
    /// Underground cells players have dug out (see caveins.rs). Not saved:
    /// a room left alone long enough to reload has settled.
    pub dug: HashSet<IVec3>,
    pub leaf_checks: HashSet<IVec3>,
    /// Local edits waiting to be sent to other players (only filled when `log_edits`).
    pub edit_log: Vec<(i32, i32, i32, Id)>,
    pub log_edits: bool,
    req_tx: Option<Sender<(i32, i32)>>,
    res_rx: Receiver<Generated>,
}

impl World {
    /// A world with the old generation rules (tests).
    #[cfg(test)]
    pub fn new(seed: u32) -> Self {
        World::with_options(seed, GenOptions::LEGACY)
    }

    pub fn with_options(seed: u32, opts: GenOptions) -> Self {
        World::with_dim(seed, opts, Dim::Over)
    }

    /// A world for dimension `dim` (see dims.rs).
    pub fn with_dim(seed: u32, opts: GenOptions, dim: Dim) -> Self {
        let generator = Arc::new(Generator::with_dim(seed, opts, dim));
        let (req_tx, req_rx) = channel::<(i32, i32)>();
        let (res_tx, res_rx) = channel();
        let req_rx = Arc::new(Mutex::new(req_rx));
        let workers = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).saturating_sub(1).clamp(1, 6);
        for _ in 0..workers {
            let (rx, tx, g) = (req_rx.clone(), res_tx.clone(), generator.clone());
            // Named so the panic hook can tell them apart from the (unnamed) audio thread.
            let builder = std::thread::Builder::new().name("chunkgen".into());
            let _ = builder.spawn(move || loop {
                let job = rx.lock().ok().and_then(|r| r.recv().ok());
                let Some((cx, cz)) = job else { return };
                // Packed and lit here, on the worker, so the main thread never holds
                // a flat copy and only has to join the light up at the borders.
                let blocks = PalettedBlocks::from_ids(&g.generate(cx, cz));
                let light = crate::light::light_alone(&blocks);
                if tx.send((cx, cz, blocks, light)).is_err() {
                    return;
                }
            });
        }
        World {
            generator,
            chunks: HashMap::new(),
            mods: HashMap::new(),
            regions: None,
            dirty: HashSet::new(),
            farm: HashMap::new(),
            containers: HashMap::new(),
            stashes: HashMap::new(),
            backpacks: HashMap::new(),
            packs: HashMap::new(),
            structure_loot: true,
            liquid_dirty: HashSet::new(),
            fall_dirty: HashSet::new(),
            jukeboxes: HashSet::new(),
            cages: HashSet::new(),
            zap_dirty: HashSet::new(),
            new_huts: Vec::new(),
            new_clankers: Vec::new(),
            new_carts: Vec::new(),
            new_residents: Vec::new(),
            saplings: HashSet::new(),
            fires: HashSet::new(),
            comparators: HashSet::new(),
            beacons: HashSet::new(),
            conduits: HashSet::new(),
            ropes: HashSet::new(),
            rods: HashSet::new(),
            cook_pots: HashSet::new(),
            dug: HashSet::new(),
            leaf_checks: HashSet::new(),
            signs: HashMap::new(),
            sign_styles: HashMap::new(),
            frames: HashMap::new(),
            simulate_liquids: true,
            pending: HashSet::new(),
            waiting: Vec::new(),
            edit_log: Vec::new(),
            log_edits: false,
            req_tx: Some(req_tx),
            res_rx,
        }
    }

    pub fn seed(&self) -> u32 {
        self.generator.seed
    }

    /// Which dimension this is.
    /// Sea level (see `GenOptions::sea`).
    pub fn sea(&self) -> i32 {
        self.generator.sea()
    }

    pub fn dim(&self) -> Dim {
        self.generator.dim
    }

    pub fn is_scorch(&self) -> bool {
        self.dim() == Dim::Scorch
    }

    pub fn is_hollow(&self) -> bool {
        self.dim() == Dim::Hollow
    }

    /// The structure that starts in chunk (cx, cz), in this dimension's own coordinates.
    pub fn site(&self, cx: i32, cz: i32) -> Option<crate::structures::Site> {
        let off = self.dim().gen_x();
        self.generator.site(cx + self.dim().gen_cx(), cz).map(|s| crate::structures::Site { origin: s.origin - ivec3(off, 0, 0), ..s })
    }

    /// The kind of structure whose middle is within `r` of `p` (this dimension's coordinates).
    pub fn site_near(&self, p: Vec3, r: f32) -> Option<crate::structures::Kind> {
        self.generator.site_near(p + Vec3::new(self.dim().gen_x() as f32, 0.0, 0.0), r)
    }

    /// The nearest structure of a kind to `at` (this dimension's coordinates).
    pub fn nearest_site(&self, kind: crate::structures::Kind, at: Vec3, radius: i32) -> Option<IVec3> {
        // (Mansions are far apart: they have their own search, and only grow up here.)
        if kind == crate::structures::Kind::Mansion {
            return (self.dim() == crate::dims::Dim::Over).then(|| self.generator.nearest_mansion(at)).flatten();
        }
        let off = self.dim().gen_x();
        self.generator.nearest_site(kind, at + Vec3::new(off as f32, 0.0, 0.0), radius).map(|p| p - ivec3(off, 0, 0))
    }

    /// Queue generation for chunks around a point and absorb finished ones.
    /// Returns the chunk keys that were unloaded.
    /// `centers` are (position, radius) pairs: the local player plus, when hosting,
    /// the remote players, so mobs and physics keep working around them.
    pub fn stream(&mut self, centers: &[(Vec3, i32)]) -> Vec<(i32, i32)> {
        // Lighting a new chunk takes a few milliseconds; spread arrivals over frames.
        let start = std::time::Instant::now();
        let in_time = || start.elapsed().as_secs_f32() < 0.008;
        // Chunks whose region file is being read wait for it (see regions.rs).
        self.absorb_region_reads();
        let mut i = 0;
        while i < self.waiting.len() && in_time() {
            let (cx, cz) = (self.waiting[i].0, self.waiting[i].1);
            if self.region_pending(cx, cz) {
                i += 1;
                continue;
            }
            let (_, _, blocks, light) = self.waiting.swap_remove(i);
            self.pending.remove(&(cx, cz));
            if !self.chunks.contains_key(&(cx, cz)) {
                self.insert_chunk_lit(cx, cz, blocks, Some(light));
            }
        }
        while in_time() {
            let Ok((cx, cz, blocks, light)) = self.res_rx.try_recv() else { break };
            if self.region_pending(cx, cz) {
                self.waiting.push((cx, cz, blocks, light));
                continue;
            }
            self.pending.remove(&(cx, cz));
            if self.chunks.contains_key(&(cx, cz)) {
                continue; // made on the spot meanwhile (see `load_now`)
            }
            self.insert_chunk_lit(cx, cz, blocks, Some(light));
        }
        self.request_chunks(centers)
    }

    /// A freshly generated chunk: replay edits, fill chests, wake liquids, mark for meshing.
    pub(crate) fn insert_chunk(&mut self, cx: i32, cz: i32, blocks: PalettedBlocks) {
        self.insert_chunk_lit(cx, cz, blocks, None);
    }

    /// The same, with the light a generator thread worked out for the chunk
    /// on its own (used unless edits changed its blocks since).
    pub(crate) fn insert_chunk_lit(&mut self, cx: i32, cz: i32, blocks: PalettedBlocks, light: Option<crate::light::LightStore>) {
        self.ensure_region(cx, cz);
        let mut chunk = Chunk::new(blocks);
        let edited = self.mods.get(&(cx, cz)).is_some_and(|m| !m.is_empty());
        if let Some(m) = self.mods.get(&(cx, cz)) {
            for (&i, &id) in m {
                // Saves and hosts can't be trusted to stay in bounds.
                if (i as usize) < CHUNK_VOL && valid_block(id) {
                    chunk.blocks.set(i as usize, id);
                    if id == SAPLING || id == FIRE || id == CONDUIT || id == ROPE || crate::homecraft::is_rod(id) || crate::cooking::is_pot(id) || crate::contraptions::is_comparator(id) || crate::beacon::is_beacon(id) || crate::music::is_jukebox(id) {
                        let (lx, rest) = ((i % CW as u32) as i32, i / CW as u32);
                        let (lz, y) = ((rest % CW as u32) as i32, (rest / CW as u32) as i32);
                        let p = ivec3(cx * CW + lx, y, cz * CW + lz);
                        match id {
                            SAPLING => self.saplings.insert(p),
                            FIRE => self.fires.insert(p),
                            CONDUIT => self.conduits.insert(p),
                            ROPE => self.ropes.insert(p),
                            b if crate::homecraft::is_rod(b) => self.rods.insert(p),
                            b if crate::cooking::is_pot(b) => self.cook_pots.insert(p),
                            b if crate::beacon::is_beacon(b) => self.beacons.insert(p),
                            b if crate::music::is_jukebox(b) => self.jukeboxes.insert(p),
                            _ => self.comparators.insert(p),
                        };
                    }
                    // Liquids and contraptions pick up where they left off.
                    if self.simulate_liquids && (is_liquid(id) || is_zappy(id)) {
                        let (lx, rest) = ((i % CW as u32) as i32, i / CW as u32);
                        let (lz, y) = ((rest % CW as u32) as i32, (rest / CW as u32) as i32);
                        let p = ivec3(cx * CW + lx, y, cz * CW + lz);
                        if is_liquid(id) {
                            self.liquid_dirty.insert(p);
                        } else {
                            self.zap_dirty.insert(p);
                        }
                    }
                }
            }
        }
        chunk.recompute_heights();
        match light.filter(|_| !edited) {
            Some(light) => {
                chunk.light = light;
                self.chunks.insert((cx, cz), chunk);
                self.light_prelit_chunk(cx, cz);
            }
            None => {
                self.chunks.insert((cx, cz), chunk);
                self.light_new_chunk(cx, cz);
            }
        }
        if self.structure_loot {
            self.fill_structure_chests(cx, cz);
        }
        for dz in -1..=1 {
            for dx in -1..=1 {
                if self.chunks.contains_key(&(cx + dx, cz + dz)) {
                    self.dirty.insert((cx + dx, cz + dz));
                }
            }
        }
    }

    /// Ask the workers for chunks near `centers`, and drop ones nobody is near.
    fn request_chunks(&mut self, centers: &[(Vec3, i32)]) -> Vec<(i32, i32)> {
        let chunk_of = |p: Vec3| ((p.x / CW as f32).floor() as i32, (p.z / CW as f32).floor() as i32);
        let mut wanted: Vec<(i32, i32, i32)> = Vec::new();
        for (ci, &(center, radius)) in centers.iter().enumerate() {
            let (pcx, pcz) = chunk_of(center);
            for dz in -radius - 1..=radius + 1 {
                for dx in -radius - 1..=radius + 1 {
                    let d2 = dx * dx + dz * dz;
                    if d2 <= (radius + 1) * (radius + 1) {
                        let k = (pcx + dx, pcz + dz);
                        if !self.chunks.contains_key(&k) && !self.pending.contains(&k) {
                            // The local player (first center) always goes first.
                            wanted.push((d2 + if ci == 0 { 0 } else { 4 }, k.0, k.1));
                        }
                    }
                }
            }
        }
        wanted.sort_unstable();
        wanted.dedup_by_key(|w| (w.1, w.2));
        let budget = 24usize.saturating_sub(self.pending.len());
        // Their region files start loading now, so they're usually ready first.
        for &(_, cx, cz) in wanted.iter().take(budget) {
            self.prefetch_region(cx, cz);
        }
        if let Some(tx) = &self.req_tx {
            for &(_, cx, cz) in wanted.iter().take(budget) {
                if tx.send((cx, cz)).is_ok() {
                    self.pending.insert((cx, cz));
                }
            }
        }

        let gone: Vec<(i32, i32)> = self
            .chunks
            .keys()
            .filter(|&&(cx, cz)| {
                centers.iter().all(|&(c, r)| {
                    let (pcx, pcz) = chunk_of(c);
                    (cx - pcx).pow(2) + (cz - pcz).pow(2) > (r + 3) * (r + 3)
                })
            })
            .copied()
            .collect();
        for k in &gone {
            self.chunks.remove(k);
            self.dirty.remove(k);
        }
        if !gone.is_empty() {
            self.drop_idle_regions();
        }
        gone
    }

    /// Chunks asked for but not yet generated.
    pub(crate) fn pending_chunks(&self) -> impl Iterator<Item = &(i32, i32)> {
        self.pending.iter()
    }

    pub fn is_loaded(&self, x: i32, z: i32) -> bool {
        self.chunks.contains_key(&(x.div_euclid(CW), z.div_euclid(CW)))
    }

    /// A chunk can be meshed once all eight neighbours exist.
    pub fn neighbours_ready(&self, cx: i32, cz: i32) -> bool {
        (-1..=1).all(|dz| (-1..=1).all(|dx| self.chunks.contains_key(&(cx + dx, cz + dz))))
    }

    #[inline]
    pub fn get(&self, x: i32, y: i32, z: i32) -> Id {
        if y < 0 {
            return BEDROCK;
        }
        if y >= CH {
            return AIR;
        }
        match self.chunks.get(&(x.div_euclid(CW), z.div_euclid(CW))) {
            Some(c) => c.blocks.get(idx(x.rem_euclid(CW), y, z.rem_euclid(CW))),
            None => AIR,
        }
    }

    pub fn get_v(&self, p: IVec3) -> Id {
        self.get(p.x, p.y, p.z)
    }

    /// 0..1 sky light of a cell (1: open sky), used for mob burning and
    /// spawning, crops, rain and shading things that move (see light.rs).
    pub fn sky_light(&self, x: i32, y: i32, z: i32) -> f32 {
        crate::light::sky_brightness(self.sky_level(x, y, z))
    }

    /// 0..1 sky light of a cell as it looks on screen, for drawing things that
    /// move so they match the terrain around them.
    pub fn sky_shade(&self, x: i32, y: i32, z: i32) -> f32 {
        crate::light::shade(self.sky_level(x, y, z))
    }

    /// How lit a small thing at `p` looks: the brighter of its own cell and the
    /// one above (so an item lying under a log or a ledge isn't drawn black
    /// just because the block overhead is solid).
    pub fn shade_near(&self, p: macroquad::math::Vec3) -> f32 {
        let (x, y, z) = (p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
        self.sky_shade(x, y, z).max(self.sky_shade(x, y + 1, z))
    }

    pub fn set(&mut self, x: i32, y: i32, z: i32, id: Id) {
        if self.set_inner(x, y, z, id).is_some() && self.log_edits {
            self.edit_log.push((x, y, z, id));
        }
    }

    /// Apply an edit that came from another player: not echoed back out.
    /// Returns the previous block if anything changed.
    pub fn set_remote(&mut self, x: i32, y: i32, z: i32, id: Id) -> Option<Id> {
        if !valid_block(id) {
            return None;
        }
        if !self.chunks.contains_key(&(x.div_euclid(CW), z.div_euclid(CW))) && (0..CH).contains(&y) {
            // Not loaded here: remember it so the chunk is right when it generates.
            let (cx, cz) = (x.div_euclid(CW), z.div_euclid(CW));
            self.record_edit(cx, cz, idx(x.rem_euclid(CW), y, z.rem_euclid(CW)) as u32, id);
            return None;
        }
        self.set_inner(x, y, z, id)
    }

    fn set_inner(&mut self, x: i32, y: i32, z: i32, id: Id) -> Option<Id> {
        if !(0..CH).contains(&y) {
            return None;
        }
        let (cx, cz) = (x.div_euclid(CW), z.div_euclid(CW));
        let (lx, lz) = (x.rem_euclid(CW), z.rem_euclid(CW));
        let c = self.chunks.get_mut(&(cx, cz))?;
        let i = idx(lx, y, lz);
        if c.blocks.get(i) == id {
            return None;
        }
        let old = c.blocks.set(i, id);
        // Tilling makes a soil record; anything else replacing farmland removes it.
        let p = ivec3(x, y, z);
        if is_farmland(id) {
            self.farm.entry(p).or_default();
        } else if is_farmland(old) {
            self.farm.remove(&p);
        }
        // Placing a chest or furnace makes it an empty inventory; breaking one
        // throws it away (spill it first). A furnace lighting up keeps its own.
        if is_container(id) {
            self.containers.entry(p).or_insert_with(|| Container::for_block(id));
        } else if is_container(old) {
            self.containers.remove(&p);
        }
        // Signs lose their words and frames their contents with the block (spill first).
        if crate::decor::is_sign(old) && !crate::decor::is_sign(id) {
            self.signs.remove(&p);
            self.sign_styles.remove(&p);
        }
        if crate::decor::is_frame(old) && !crate::decor::is_frame(id) {
            self.frames.remove(&p);
        }
        c.recompute_height(lx, lz);
        self.record_edit(cx, cz, i as u32, id);
        if id == SAPLING {
            self.saplings.insert(p);
        } else if old == SAPLING {
            self.saplings.remove(&p);
        }
        if id == FIRE {
            self.fires.insert(p);
        } else if old == FIRE {
            self.fires.remove(&p);
        }
        if crate::contraptions::is_comparator(id) {
            self.comparators.insert(p);
        } else if crate::contraptions::is_comparator(old) {
            self.comparators.remove(&p);
        }
        if crate::beacon::is_beacon(id) {
            self.beacons.insert(p);
        } else if crate::beacon::is_beacon(old) {
            self.beacons.remove(&p);
        }
        if id == CONDUIT {
            self.conduits.insert(p);
        } else if old == CONDUIT {
            self.conduits.remove(&p);
        }
        if id == ROPE {
            self.ropes.insert(p);
        } else if old == ROPE {
            self.ropes.remove(&p);
        }
        if crate::homecraft::is_rod(id) {
            self.rods.insert(p);
        } else if crate::homecraft::is_rod(old) {
            self.rods.remove(&p);
        }
        if crate::cooking::is_pot(id) {
            self.cook_pots.insert(p);
        } else if crate::cooking::is_pot(old) {
            self.cook_pots.remove(&p);
        }
        if crate::fortress::is_cage(id) {
            self.cages.insert(p);
        } else if crate::fortress::is_cage(old) {
            self.cages.remove(&p);
        }
        if crate::music::is_jukebox(id) {
            self.jukeboxes.insert(p);
        } else if crate::music::is_jukebox(old) {
            self.jukeboxes.remove(&p);
        }
        let treeish = |b: Id| is_log(b) || is_leaves(b);
        if self.simulate_liquids && treeish(old) && !treeish(id) {
            self.wake_leaves(p);
        }
        // Fences and panes join whatever is beside them (where the world lives; see carpentry.rs).
        let glassy = |b: Id| b == GLASS || crate::carpentry::is_stained_glass(b);
        if self.simulate_liquids && (is_opaque(old) != is_opaque(id) || crate::carpentry::family(old) != crate::carpentry::family(id) || glassy(old) != glassy(id)) {
            self.reshape_joins(p);
        }
        if self.simulate_liquids {
            // Sand and gravel: one put here may have nothing under it, and one above may have lost its footing.
            if crate::falling::is_gravity(id) {
                self.fall_dirty.insert(p);
            }
            let above = p + IVec3::Y;
            if crate::falling::is_gravity(self.get_v(above)) || self.get_v(above) == POINTY_ROCK {
                self.fall_dirty.insert(above);
            }
            // A Pointy Rock hanging from here may have lost what it hangs from.
            let below = p - IVec3::Y;
            if below.y >= 0 && self.get_v(below) == POINTY_ROCK {
                self.fall_dirty.insert(below);
            }
            self.wake_liquids(p, is_liquid(id) || is_liquid(old));
            self.wake_zappy(p, is_zappy(id) || is_zappy(old) || is_door(id) || id == TNT || crate::scorch::is_portal(old));
        }
        let xs: &[i32] = if lx == 0 { &[-1, 0] } else if lx == CW - 1 { &[0, 1] } else { &[0] };
        let zs: &[i32] = if lz == 0 { &[-1, 0] } else if lz == CW - 1 { &[0, 1] } else { &[0] };
        for &dx in xs {
            for &dz in zs {
                self.dirty.insert((cx + dx, cz + dz));
            }
        }
        self.relight(p, old, id);
        Some(old)
    }

    /// A cell changed: it and its neighbours may need to flow (only if a
    /// liquid is involved, which is rare, so this stays cheap).
    fn wake_liquids(&mut self, p: IVec3, involved: bool) {
        const SIDES: [IVec3; 6] = [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z];
        if involved || SIDES.iter().any(|d| is_liquid(self.get_v(p + *d))) {
            self.liquid_dirty.insert(p);
            for d in SIDES {
                self.liquid_dirty.insert(p + d);
            }
        }
    }

    /// The same for Zappy Dust: a switch flipped or a wire laid wakes its neighbours.
    fn wake_zappy(&mut self, p: IVec3, involved: bool) {
        const SIDES: [IVec3; 6] = [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z];
        let watched = |id: Id| is_zappy(id) || crate::scorch::is_portal(id);
        if involved || SIDES.iter().any(|d| watched(self.get_v(p + *d))) {
            self.zap_dirty.insert(p);
            for d in SIDES {
                self.zap_dirty.insert(p + d);
            }
        }
    }

    /// Set a block even if its chunk isn't loaded here (it's applied when the
    /// chunk generates), and share it with other players. Used by scripts.
    pub fn set_or_record(&mut self, x: i32, y: i32, z: i32, id: Id) {
        if self.chunks.contains_key(&(x.div_euclid(CW), z.div_euclid(CW))) {
            self.set(x, y, z, id);
        } else if (0..CH).contains(&y) && valid_block(id) {
            let (cx, cz) = (x.div_euclid(CW), z.div_euclid(CW));
            self.record_edit(cx, cz, idx(x.rem_euclid(CW), y, z.rem_euclid(CW)) as u32, id);
            if self.log_edits {
                self.edit_log.push((x, y, z, id));
            }
        }
    }

    pub fn set_v(&mut self, p: IVec3, id: Id) {
        self.set(p.x, p.y, p.z, id)
    }

    /// Voxel DDA (Amanatides & Woo), stopping at anything a player can point at.
    pub fn raycast(&self, origin: Vec3, dir: Vec3, max: f32) -> Option<Hit> {
        self.raycast_where(origin, dir, max, targetable)
    }

    /// The same, also stopping at water and lava sources (buckets reach through flows to them).
    pub fn raycast_liquid(&self, origin: Vec3, dir: Vec3, max: f32) -> Option<Hit> {
        self.raycast_where(origin, dir, max, |id| targetable(id) || id == WATER || id == LAVA)
    }

    fn raycast_where(&self, origin: Vec3, dir: Vec3, max: f32, stop: impl Fn(Id) -> bool) -> Option<Hit> {
        let mut p = ivec3(origin.x.floor() as i32, origin.y.floor() as i32, origin.z.floor() as i32);
        let step = ivec3(dir.x.signum() as i32, dir.y.signum() as i32, dir.z.signum() as i32);
        let inv = |d: f32| if d.abs() < 1e-9 { f32::INFINITY } else { 1.0 / d.abs() };
        let t_delta = [inv(dir.x), inv(dir.y), inv(dir.z)];
        let frac = |o: f32, d: f32| if d > 0.0 { o.floor() + 1.0 - o } else { o - o.floor() };
        let mut t_max = [frac(origin.x, dir.x) * t_delta[0], frac(origin.y, dir.y) * t_delta[1], frac(origin.z, dir.z) * t_delta[2]];
        let mut normal = IVec3::ZERO;
        let mut t = 0.0;
        while t <= max {
            let id = self.get_v(p);
            if stop(id) {
                if block(id).model != Model::Shaped {
                    return Some(Hit { pos: p, normal, dist: t });
                }
                // Slabs, stairs, doors: only their boxes count, with the face actually hit.
                let cell = p.as_vec3();
                let (boxes, n) = block_boxes(id);
                let mut best: Option<(f32, IVec3)> = None;
                for &(a, b) in &boxes[..n] {
                    if let Some((bt, bn)) = ray_box(origin, dir, cell + Vec3::from_array(a), cell + Vec3::from_array(b))
                        && bt <= max
                        && best.map(|(t0, _)| bt < t0).unwrap_or(true)
                    {
                        best = Some((bt, bn));
                    }
                }
                if let Some((bt, bn)) = best {
                    return Some(Hit { pos: p, normal: bn, dist: bt });
                }
            }
            let axis = if t_max[0] < t_max[1] { if t_max[0] < t_max[2] { 0 } else { 2 } } else if t_max[1] < t_max[2] { 1 } else { 2 };
            t = t_max[axis];
            t_max[axis] += t_delta[axis];
            normal = IVec3::ZERO;
            match axis {
                0 => {
                    p.x += step.x;
                    normal.x = -step.x;
                }
                1 => {
                    p.y += step.y;
                    normal.y = -step.y;
                }
                _ => {
                    p.z += step.z;
                    normal.z = -step.z;
                }
            }
        }
        None
    }

    /// A dry, grassy-ish place near the origin to start.
    pub fn find_spawn(&self) -> Vec3 {
        for r in 0..200i32 {
            for (dx, dz) in [(r, 0), (0, r), (-r, 0), (0, -r), (r, r), (-r, -r)] {
                let (x, z) = (dx * 4, dz * 4);
                let (h, biome) = self.generator.column(x, z);
                // Solid ground: no cave mouth or ravine to drop straight into.
                let solid = (h - 4..=h).all(|y| !self.generator.is_cave(x, y, z, h)) && self.generator.ravine_floor(x, z).is_none();
                if h > self.sea() + 1 && !biome.is_ocean() && solid && self.generator.tree_at(x, z).is_none() {
                    return Vec3::new(x as f32 + 0.5, h as f32 + 1.0, z as f32 + 0.5);
                }
            }
        }
        Vec3::new(0.5, (CH - 10) as f32, 0.5)
    }

    /// The lowest height rain and snow reach in a column (above any roof,
    /// overhang or canopy). Unloaded columns: the top of the world.
    pub fn rain_top(&self, x: i32, z: i32) -> i32 {
        let (cx, cz) = (x.div_euclid(CW), z.div_euclid(CW));
        match self.chunks.get(&(cx, cz)) {
            Some(c) => c.canopy[(z.rem_euclid(CW) * CW + x.rem_euclid(CW)) as usize] as i32,
            None => CH,
        }
    }

    /// Topmost solid block at a column (for respawn and mob spawning).
    pub fn surface_y(&self, x: i32, z: i32) -> i32 {
        for y in (0..CH).rev() {
            let b = self.get(x, y, z);
            if is_solid(b) || is_liquid(b) {
                return y;
            }
        }
        0
    }
}

impl Drop for World {
    fn drop(&mut self) {
        // Closing the request channel lets the worker threads exit.
        self.req_tx = None;
    }
}

/// Ray against a box: distance along `dir` and the normal of the face entered.
/// A ray starting inside reports distance 0 and a zero normal.
pub fn ray_box(o: Vec3, d: Vec3, min: Vec3, max: Vec3) -> Option<(f32, IVec3)> {
    let (mut t0, mut t1) = (0.0f32, f32::MAX);
    let mut normal = IVec3::ZERO;
    for i in 0..3 {
        if d[i].abs() < 1e-9 {
            if o[i] < min[i] || o[i] > max[i] {
                return None;
            }
            continue;
        }
        let inv = 1.0 / d[i];
        let (mut a, mut b) = ((min[i] - o[i]) * inv, (max[i] - o[i]) * inv);
        let mut n = IVec3::ZERO;
        n[i] = if d[i] > 0.0 { -1 } else { 1 };
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
        if a > t0 {
            t0 = a;
            normal = n;
        }
        t1 = t1.min(b);
        if t0 > t1 {
            return None;
        }
    }
    Some((t0, normal))
}

#[cfg(test)]
mod biome_tests {
    use super::*;

    #[test]
    fn every_biome_turns_up_with_its_own_ground() {
        let g = Generator::new(2024);
        let mut seen: HashMap<&str, (i32, i32)> = HashMap::new();
        // (And somewhere well up from the shore, for looking at what grows there.)
        let mut inland: HashMap<&str, (i32, i32)> = HashMap::new();
        for z in (-6000..6000).step_by(40) {
            for x in (-6000..6000).step_by(40) {
                let (h, b) = g.column(x, z);
                seen.entry(b.name()).or_insert((x, z));
                if h > g.sea() + 3 {
                    inland.entry(b.name()).or_insert((x, z));
                }
            }
        }
        for b in Biome::ALL {
            assert!(seen.contains_key(b.name()), "no {} found", b.name());
        }
        // Look at a chunk in the middle of each new biome.
        let top = |b: &Vec<Id>, lx: i32, lz: i32| (0..CH).rev().map(|y| b[idx(lx, y, lz)]).find(|&id| id != AIR && !is_leaves(id) && !is_log(id) && block(id).model != Model::Cross);
        let has = |biome: Biome, what: &dyn Fn(Id) -> bool| {
            let (x, z) = seen[biome.name()];
            let chunk = g.generate(x.div_euclid(CW), z.div_euclid(CW));
            (0..CW).any(|lz| (0..CW).any(|lx| top(&chunk, lx, lz).is_some_and(what))) || chunk.iter().any(|&id| what(id))
        };
        assert!(has(Biome::Badlands, &|id| id == RED_SAND || (TERRACOTTA..TERRACOTTA + 4).contains(&id)));
        assert!(has(Biome::Swamp, &|id| id == MUD || id == WATER));
        assert!(has(Biome::Cherry, &|id| id == CHERRY_LEAVES || id == PINK_PETALS));
        assert!(has(Biome::Mangrove, &|id| id == MANGROVE_ROOTS || id == MANGROVE_LEAVES));
        assert!(has(Biome::Mangrove, &|id| id == MUD));
        // v0.3's: each with its own trees or ground (looked at inland).
        let has_inland = |biome: Biome, what: &dyn Fn(Id) -> bool| {
            let (x, z) = inland[biome.name()];
            let chunk = g.generate(x.div_euclid(CW), z.div_euclid(CW));
            chunk.iter().any(|&id| what(id))
        };
        assert!(has_inland(Biome::Savanna, &|id| id == ACACIA_LEAVES || id == ACACIA_LOG || id == TALL_GRASS));
        assert!(has_inland(Biome::BirchForest, &|id| id == BIRCH_LEAVES));
        assert!(has_inland(Biome::DarkForest, &|id| id == DARK_OAK_LEAVES));
        assert!(has(Biome::MushroomIslands, &|id| id == MYCELIUM));
        assert!(has_inland(Biome::IceSpikes, &|id| id == PACKED_ICE || id == SNOW_BLOCK));
        assert!(has_inland(Biome::StonyPeaks, &|id| id == STONE || id == CALCITE));
        assert!(has_inland(Biome::Meadow, &|id| id == WILDFLOWERS || id == CORNFLOWER || id == LAVENDER));
        // Cacti only grow on dry land, and mangrove roots in water are wet.
        let mut wet = 0;
        for (name, near) in [(Biome::Desert.name(), 6), (Biome::Badlands.name(), 6), (Biome::Mangrove.name(), 6)] {
            let (x, z) = seen[name];
            for cz in -near..=near {
                for cx in -near..=near {
                    let (ccx, ccz) = (x.div_euclid(CW) + cx, z.div_euclid(CW) + cz);
                    let chunk = g.generate(ccx, ccz);
                    for y in 1..CH - 1 {
                        for lz in 0..CW {
                            for lx in 0..CW {
                                let id = chunk[idx(lx, y, lz)];
                                if id == CACTUS {
                                    assert!(y > g.sea() && !is_water(chunk[idx(lx, y + 1, lz)]), "a cactus in the water at {} {y} {}", ccx * CW + lx, ccz * CW + lz);
                                }
                                if id == MANGROVE_ROOTS_WET {
                                    wet += 1;
                                }
                                if id == MANGROVE_ROOTS && (1..CW - 1).contains(&lx) && (1..CW - 1).contains(&lz) {
                                    let beside = [chunk[idx(lx + 1, y, lz)], chunk[idx(lx - 1, y, lz)], chunk[idx(lx, y, lz + 1)], chunk[idx(lx, y, lz - 1)]];
                                    assert!(!beside.iter().any(|&b| is_water(b)), "dry roots beside water at {} {y} {}", ccx * CW + lx, ccz * CW + lz);
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(wet > 0, "some mangroves stand in water");
        // Not too rare, not everywhere.
        let mut count: HashMap<Biome, usize> = HashMap::new();
        for z in (-6000..6000).step_by(40) {
            for x in (-6000..6000).step_by(40) {
                *count.entry(g.column(x, z).1).or_default() += 1;
            }
        }
        let total = 300 * 300;
        for b in [Biome::Cherry, Biome::Mangrove] {
            let share = count[&b] as f32 / total as f32;
            assert!((0.004..0.12).contains(&share), "{} covers {:.1}% of the world", b.name(), share * 100.0);
        }
    }

    #[test]
    fn spawn_is_on_solid_ground() {
        for seed in 0..12 {
            let w = World::new(seed);
            let s = w.find_spawn();
            let (x, z) = (s.x.floor() as i32, s.z.floor() as i32);
            let chunk = w.generator.generate(x.div_euclid(CW), z.div_euclid(CW));
            let under = chunk[idx(x.rem_euclid(CW), s.y as i32 - 1, z.rem_euclid(CW))];
            assert!(is_solid(under), "seed {seed}: spawn at {s} stands on {}", block(under).name);
        }
    }
}

#[cfg(test)]
mod v3_tests {
    use super::*;

    /// Median length of a land biome's stretch along lines across the world.
    fn median_run(g: &Generator) -> i32 {
        let mut runs = Vec::new();
        for z in (-4000..4000).step_by(400) {
            let (mut prev, mut run) = (None, 0);
            for x in (-4000..4000).step_by(8) {
                let b = g.column(x, z).1;
                if Some(b) == prev {
                    run += 8;
                } else {
                    if prev.is_some_and(|p: Biome| !p.is_ocean()) {
                        runs.push(run);
                    }
                    (prev, run) = (Some(b), 8);
                }
            }
        }
        runs.sort_unstable();
        runs[runs.len() / 2]
    }

    #[test]
    fn new_worlds_change_by_degrees() {
        let g = Generator::with(2024, GenOptions::DEFAULT);
        use Biome::*;
        let frozen = |b: Biome| matches!(b, Snowy | IceSpikes);
        let mild = |b: Biome| matches!(b, Plains | Forest | BirchForest | DarkForest | Cherry | Meadow | PaleGarden | Swamp);
        let hot = |b: Biome| matches!(b, Desert | Badlands);
        let warm = |b: Biome| matches!(b, Savanna | Jungle | Mangrove);
        let mut count: HashMap<Biome, usize> = HashMap::new();
        let mut worst_swamp_step = 0;
        for z in (-3000..3000).step_by(24) {
            for x in (-3000..3000).step_by(2) {
                let ((h1, a), (h2, b)) = (g.column(x, z), g.column(x + 2, z));
                *count.entry(a).or_insert(0) += 1;
                // Snow meets taiga (or the sea, or peaks), never the mild or hot lands.
                // (Snow on high mountaintops can meet anything that climbs that high.)
                let high = h1.max(h2) > g.sea() + 40;
                assert!(high || !(frozen(a) && (mild(b) || hot(b) || warm(b))) && !(frozen(b) && (mild(a) || hot(a) || warm(a))), "{a:?} next to {b:?} at {x},{z}");
                // Desert and badlands meet savanna or jungle, not the temperate lands.
                assert!(!(hot(a) && (mild(b) || b == Taiga)) && !(hot(b) && (mild(a) || a == Taiga)), "{a:?} next to {b:?} at {x},{z}");
                if [a, b].iter().any(|b| matches!(b, Swamp | Mangrove)) {
                    worst_swamp_step = worst_swamp_step.max((h1 - h2).abs());
                }
            }
        }
        assert!(worst_swamp_step <= 4, "swamps meet their neighbours gently (worst step {worst_swamp_step} over 2 blocks)");
        let total: usize = count.values().sum();
        for (b, n) in &count {
            assert!(*n as f32 / (total as f32) < 0.25, "{b:?} covers too much");
        }
        // Smaller than "normal" was before version 3.
        let before = Generator::with(2024, GenOptions { version: 2, ..GenOptions::DEFAULT });
        let (now, then) = (median_run(&g), median_run(&before));
        assert!(now * 3 < then * 2 && now >= 16, "median stretch {now} blocks (it was {then})");
    }

    #[test]
    fn ground_cover_is_ragged_at_borders() {
        let g = Generator::with(7, GenOptions::DEFAULT);
        // Somewhere taiga meets plains: the ground there mixes the two.
        let (mut mixed, mut seen) = (0, 0);
        for z in (-2000..2000).step_by(37) {
            for x in (-2000..2000).step_by(3) {
                let own = g.column(x, z).1;
                if own == Biome::Plains && g.column(x + 3, z).1 == Biome::Taiga {
                    seen += 1;
                    mixed += (-4..=4).filter(|d| g.ground_biome(x + d, z, g.column(x + d, z).1) != g.column(x + d, z).1).count();
                }
            }
        }
        assert!(seen > 5 && mixed > seen, "{mixed} mixed columns at {seen} borders");
        // Older worlds keep their straight edges.
        let old = Generator::with(7, GenOptions { version: 2, ..GenOptions::DEFAULT });
        assert_eq!(old.ground_biome(5, 5, Biome::Plains), Biome::Plains);
    }
}
