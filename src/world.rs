//! Chunked voxel world: storage, threaded terrain generation, edits and raycasts.

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
pub const CH: i32 = 128;
pub const SEA: i32 = 40;
const CHUNK_VOL: usize = (CW * CW * CH) as usize;

#[inline]
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
}

/// How much sky a cell sees, from its column's heights: full sky above everything,
/// dappled shade under foliage, and fading light below solid blocks.
#[inline]
pub fn exposure(height: i32, canopy: i32, y: i32) -> f32 {
    if y < height {
        (1.0 - (height - y) as f32 * 0.09).max(0.0)
    } else if y < canopy {
        CANOPY_SHADE
    } else {
        1.0
    }
}

/// Light under leaves. Foliage scatters light rather than stopping it, so shade
/// under a tree is gentle (and the undersides of leaves aren't pitch black at dusk).
pub const CANOPY_SHADE: f32 = 0.8;

impl Chunk {
    /// A chunk whose heightmaps still need `recompute_heights`.
    pub fn new(blocks: PalettedBlocks) -> Chunk {
        Chunk { blocks, heights: [0; 256], canopy: [0; 256] }
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

#[derive(Clone, Copy, PartialEq)]
pub enum Biome {
    Plains,
    Forest,
    Desert,
    Snowy,
    Ocean,
}

impl Biome {
    pub fn name(self) -> &'static str {
        match self {
            Biome::Plains => "Plains (Flat-ish)",
            Biome::Forest => "Forest (Trees Included)",
            Biome::Desert => "Desert (Dry Humour)",
            Biome::Snowy => "Snowy (Chilly)",
            Biome::Ocean => "Ocean (Wet)",
        }
    }
}

/// Pure function of the seed: safe to share across worker threads.
pub struct Generator {
    pub seed: u32,
    continent: Perlin,
    hills: Perlin,
    ridges: Perlin,
    temp: Perlin,
    moist: Perlin,
    cave_a: Perlin,
    cave_b: Perlin,
    cavern: Perlin,
    /// Mod world generation, copied from the registry when the world is created.
    ores: Vec<OreGen>,
    plants: Vec<PlantGen>,
}

impl Generator {
    pub fn new(seed: u32) -> Self {
        let s = seed as u64;
        Generator {
            seed,
            continent: Perlin::new(s),
            hills: Perlin::new(s + 1),
            ridges: Perlin::new(s + 2),
            temp: Perlin::new(s + 3),
            moist: Perlin::new(s + 4),
            cave_a: Perlin::new(s + 5),
            cave_b: Perlin::new(s + 6),
            cavern: Perlin::new(s + 7),
            ores: reg().ores.clone(),
            plants: reg().plants.clone(),
        }
    }

    /// Surface height and biome for a column.
    pub fn column(&self, x: i32, z: i32) -> (i32, Biome) {
        let (fx, fz) = (x as f32, z as f32);
        let c = self.continent.fbm2(fx / 420.0, fz / 420.0, 4);
        let hills = self.hills.fbm2(fx / 110.0, fz / 110.0, 4);
        let r = 1.0 - self.ridges.fbm2(fx / 180.0, fz / 180.0, 3).abs();
        let mount = ((c - 0.1) / 0.45).clamp(0.0, 1.0);
        let h = SEA as f32 + 3.0 + c * 20.0 + hills * 9.0 * (0.4 + mount) + mount * r * r * 48.0;
        let h = (h as i32).clamp(4, CH - 20);
        let t = self.temp.fbm2(fx / 520.0 + 300.0, fz / 520.0, 3);
        let m = self.moist.fbm2(fx / 380.0, fz / 380.0 - 200.0, 3);
        let biome = if h < SEA - 1 {
            Biome::Ocean
        } else if t < -0.3 || h > 92 {
            Biome::Snowy
        } else if t > 0.25 && m < 0.05 {
            Biome::Desert
        } else if m > 0.08 {
            Biome::Forest
        } else {
            Biome::Plains
        };
        (h, biome)
    }

    /// Cold enough for the sea to freeze (the same temperature that makes snowy biomes).
    fn cold(&self, x: i32, z: i32) -> bool {
        self.temp.fbm2(x as f32 / 520.0 + 300.0, z as f32 / 520.0, 3) < -0.3
    }

    pub fn tree_at(&self, x: i32, z: i32) -> Option<(i32, i32)> {
        let (h, biome) = self.column(x, z);
        let density = match biome {
            Biome::Forest => 0.035,
            Biome::Plains => 0.004,
            Biome::Snowy => 0.012,
            _ => 0.0,
        };
        if h <= SEA + 1 || hash2(self.seed ^ 0x7EE, x, z) >= density {
            return None;
        }
        let trunk = 4 + (hash2(self.seed ^ 0x7E1, x, z) * 3.0) as i32;
        Some((h, trunk))
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
        if surface <= SEA + 1 && y >= surface - 5 {
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

    pub fn generate(&self, cx: i32, cz: i32) -> Vec<Id> {
        let mut b = vec![AIR; CHUNK_VOL];
        let s = self.seed;
        let mut cols = [(0i32, Biome::Plains); 256];
        for lz in 0..CW {
            for lx in 0..CW {
                let (x, z) = (cx * CW + lx, cz * CW + lz);
                let (h, biome) = self.column(x, z);
                cols[(lz * CW + lx) as usize] = (h, biome);
                let beach = (SEA - 1..=SEA + 1).contains(&h) && biome != Biome::Snowy;
                for y in 0..CH.min(h.max(SEA) + 1) {
                    let id = if y == 0 || (y <= 2 && hash3(s, x, y, z) < 0.5) {
                        BEDROCK
                    } else if y < h - 3 {
                        // Deserts sit on a few layers of sandstone.
                        if biome == Biome::Desert && y >= h - 8 { SANDSTONE } else { STONE }
                    } else if y < h {
                        if biome == Biome::Desert || beach { SAND } else { DIRT }
                    } else if y == h {
                        if h < SEA - 1 {
                            if hash2(s ^ 0x6A, x, z) < 0.3 { GRAVEL } else if h > SEA - 6 { SAND } else { DIRT }
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
                let ravine = if h > SEA + 2 && biome != Biome::Ocean { self.ravine_floor(x, z) } else { None };
                for y in 1..h + 1 {
                    let i = idx(lx, y, lz);
                    if b[i] == BEDROCK {
                        continue;
                    }
                    if self.is_cave(x, y, z, h) || ravine.is_some_and(|f| y >= f) {
                        // Deep caverns are flooded.
                        b[i] = if y <= 11 && self.cavern.noise3(x as f32 / 55.0, y as f32 / 28.0, z as f32 / 55.0) > 0.42 { WATER } else { AIR };
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
                        } else if y < 32 && (0.0215..0.0245).contains(&r) && r2 < 0.55 {
                            b[i] = GOLD_ORE;
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
                // Cave decorations: glowing mushrooms and pointy rocks on floors, pointy rocks on ceilings.
                for y in 3..(h - 4).max(3) {
                    if b[idx(lx, y, lz)] != AIR {
                        continue;
                    }
                    let (below, above) = (b[idx(lx, y - 1, lz)], b[idx(lx, y + 1, lz)]);
                    let r = hash3(s ^ 0xCA7E, x, y, z);
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
                    if r < 0.012 {
                        b[idx(lx, top, lz)] = FLOWER;
                    } else if r < 0.11 {
                        b[idx(lx, top, lz)] = TALL_GRASS;
                    } else if r < 0.1125 && biome == Biome::Plains {
                        b[idx(lx, top, lz)] = PUMPKIN;
                    } else if r < 0.125 && biome == Biome::Forest {
                        b[idx(lx, top, lz)] = MUSHROOM;
                    }
                }
                // Pokey Plants in the desert, 1-3 tall.
                if biome == Biome::Desert && top + 3 < CH && b[idx(lx, h, lz)] == SAND && hash2(s ^ 0xCAC, x, z) < 0.005 {
                    let tall = 1 + (hash2(s ^ 0xCAD, x, z) * 3.0) as i32;
                    for y in top..top + tall.min(3) {
                        b[idx(lx, y, lz)] = CACTUS;
                    }
                }
                // Cold seas freeze over.
                if h < SEA && b[idx(lx, SEA, lz)] == WATER && self.cold(x, z) {
                    b[idx(lx, SEA, lz)] = ICE;
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
        for tz in cz * CW - 3..cz * CW + CW + 3 {
            for tx in cx * CW - 3..cx * CW + CW + 3 {
                let Some((h, trunk)) = self.tree_at(tx, tz) else { continue };
                let top = h + trunk;
                let mut put = |x: i32, y: i32, z: i32, id: Id, force: bool| {
                    let (lx, lz) = (x - cx * CW, z - cz * CW);
                    if !(0..CW).contains(&lx) || !(0..CW).contains(&lz) || !(0..CH).contains(&y) {
                        return;
                    }
                    let i = idx(lx, y, lz);
                    if force || b[i] == AIR || b[i] == TALL_GRASS || b[i] == FLOWER {
                        b[i] = id;
                    }
                };
                for dy in -2..=1 {
                    let rad: i32 = if dy <= -1 { 2 } else { 1 };
                    for dz in -rad..=rad {
                        for dx in -rad..=rad {
                            let corner = dx.abs() == rad && dz.abs() == rad;
                            if corner && (dy == 1 || hash3(s ^ 0x1EA, tx + dx, top + dy, tz + dz) < 0.5) {
                                continue;
                            }
                            put(tx + dx, top + dy, tz + dz, LEAVES, false);
                        }
                    }
                }
                for y in h + 1..top {
                    put(tx, y, tz, LOG, true);
                }
                put(tx, h, tz, DIRT, true);
            }
        }
        self.place_structures(cx, cz, &mut b);
        b
    }
}

pub struct Hit {
    pub pos: IVec3,
    pub normal: IVec3,
    pub dist: f32,
}

pub struct World {
    pub generator: Arc<Generator>,
    pub chunks: HashMap<(i32, i32), Chunk>,
    /// Player edits, keyed by chunk then block index. Re-applied when chunks regenerate.
    pub mods: HashMap<(i32, i32), HashMap<u32, Id>>,
    /// Chunks whose mesh is stale.
    pub dirty: HashSet<(i32, i32)>,
    pending: HashSet<(i32, i32)>,
    /// Soil records for every tilled block (see farming.rs); kept in step with the blocks.
    pub farm: HashMap<IVec3, Soil>,
    /// What's inside every chest and furnace (see containers.rs); kept in step with the blocks.
    pub containers: HashMap<IVec3, Container>,
    /// Fill structure chests when their chunks first arrive (off for joined
    /// players: the host has the real contents).
    pub structure_loot: bool,
    /// Local edits waiting to be sent to other players (only filled when `log_edits`).
    pub edit_log: Vec<(i32, i32, i32, Id)>,
    pub log_edits: bool,
    req_tx: Option<Sender<(i32, i32)>>,
    res_rx: Receiver<(i32, i32, PalettedBlocks)>,
}

impl World {
    pub fn new(seed: u32) -> Self {
        let generator = Arc::new(Generator::new(seed));
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
                // Packed here, on the worker, so the main thread never holds a flat copy.
                if tx.send((cx, cz, PalettedBlocks::from_ids(&g.generate(cx, cz)))).is_err() {
                    return;
                }
            });
        }
        World {
            generator,
            chunks: HashMap::new(),
            mods: HashMap::new(),
            dirty: HashSet::new(),
            farm: HashMap::new(),
            containers: HashMap::new(),
            structure_loot: true,
            pending: HashSet::new(),
            edit_log: Vec::new(),
            log_edits: false,
            req_tx: Some(req_tx),
            res_rx,
        }
    }

    pub fn seed(&self) -> u32 {
        self.generator.seed
    }

    /// Queue generation for chunks around a point and absorb finished ones.
    /// Returns the chunk keys that were unloaded.
    /// `centers` are (position, radius) pairs: the local player plus, when hosting,
    /// the remote players, so mobs and physics keep working around them.
    pub fn stream(&mut self, centers: &[(Vec3, i32)]) -> Vec<(i32, i32)> {
        while let Ok((cx, cz, blocks)) = self.res_rx.try_recv() {
            self.pending.remove(&(cx, cz));
            let mut chunk = Chunk::new(blocks);
            if let Some(m) = self.mods.get(&(cx, cz)) {
                for (&i, &id) in m {
                    // Saves and hosts can't be trusted to stay in bounds.
                    if (i as usize) < CHUNK_VOL && valid_block(id) {
                        chunk.blocks.set(i as usize, id);
                    }
                }
            }
            chunk.recompute_heights();
            self.chunks.insert((cx, cz), chunk);
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
        gone
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

    /// 0..1 sky exposure of a cell, used for lighting and mob burning/spawning.
    pub fn sky_light(&self, x: i32, y: i32, z: i32) -> f32 {
        match self.chunks.get(&(x.div_euclid(CW), z.div_euclid(CW))) {
            Some(c) => {
                let i = (z.rem_euclid(CW) * CW + x.rem_euclid(CW)) as usize;
                exposure(c.heights[i] as i32, c.canopy[i] as i32, y)
            }
            None => 1.0,
        }
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
            self.mods.entry((cx, cz)).or_default().insert(idx(x.rem_euclid(CW), y, z.rem_euclid(CW)) as u32, id);
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
        c.recompute_height(lx, lz);
        self.mods.entry((cx, cz)).or_default().insert(i as u32, id);
        let xs: &[i32] = if lx == 0 { &[-1, 0] } else if lx == CW - 1 { &[0, 1] } else { &[0] };
        let zs: &[i32] = if lz == 0 { &[-1, 0] } else if lz == CW - 1 { &[0, 1] } else { &[0] };
        for &dx in xs {
            for &dz in zs {
                self.dirty.insert((cx + dx, cz + dz));
            }
        }
        Some(old)
    }

    /// Set a block even if its chunk isn't loaded here (it's applied when the
    /// chunk generates), and share it with other players. Used by scripts.
    pub fn set_or_record(&mut self, x: i32, y: i32, z: i32, id: Id) {
        if self.chunks.contains_key(&(x.div_euclid(CW), z.div_euclid(CW))) {
            self.set(x, y, z, id);
        } else if (0..CH).contains(&y) && valid_block(id) {
            let (cx, cz) = (x.div_euclid(CW), z.div_euclid(CW));
            self.mods.entry((cx, cz)).or_default().insert(idx(x.rem_euclid(CW), y, z.rem_euclid(CW)) as u32, id);
            if self.log_edits {
                self.edit_log.push((x, y, z, id));
            }
        }
    }

    pub fn set_v(&mut self, p: IVec3, id: Id) {
        self.set(p.x, p.y, p.z, id)
    }

    /// Voxel DDA (Amanatides & Woo).
    pub fn raycast(&self, origin: Vec3, dir: Vec3, max: f32) -> Option<Hit> {
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
            if targetable(id) {
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
                if h > SEA + 1 && biome != Biome::Ocean && self.generator.tree_at(x, z).is_none() {
                    return Vec3::new(x as f32 + 0.5, h as f32 + 1.0, z as f32 + 0.5);
                }
            }
        }
        Vec3::new(0.5, (CH - 10) as f32, 0.5)
    }

    /// Topmost solid block at a column (for respawn and mob spawning).
    pub fn surface_y(&self, x: i32, z: i32) -> i32 {
        for y in (0..CH).rev() {
            let b = self.get(x, y, z);
            if is_solid(b) || b == WATER {
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
