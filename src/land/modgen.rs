//! Mod world generation (see MODDING.md): biomes, structures built from a
//! block template, and whole dimensions with a portal of their own.
//!
//! - A **biome** takes over a share of one or more base biomes' land (or of
//!   a mod dimension) in big soft-edged patches: its own top and under
//!   blocks, plants and trees.
//! - A **structure** is a block template (layers of rows of letters, each
//!   letter a block) stamped onto the ground on a grid, in the biomes it
//!   names; chests in it are filled from its loot list the first time
//!   they're loaded.
//! - A **dimension** has its own rolling land (its own blocks, height and
//!   hills, and a sea of any liquid), its own sky colour and light, and a
//!   portal: a frame of its `frame` block lit with a Sparker, like the
//!   Scorchlands' obsidian one. Portals lead between it and the Overworld,
//!   block for block.
//!
//! Everything here comes from the registry (built from the mods), so it's
//! the same on every machine that has the same mods.

use crate::block::*;
use crate::dims::Dim;
use crate::noise::{hash2, hash3};
use crate::world::{idx, Biome, Generator, CH, CW};
use macroquad::math::{ivec3, IVec3};

/// A mod tree: a trunk of `log` with a round crown of `leaves`.
#[derive(Clone, Debug)]
pub struct ModTree {
    pub log: Id,
    pub leaves: Id,
    pub chance: f32,
    pub height: i32,
}

#[derive(Clone, Debug)]
pub struct ModBiome {
    pub key: String,
    pub name: &'static str,
    /// The base biomes whose land it takes a share of (in the Overworld).
    pub within: Vec<Biome>,
    /// Or: the mod dimension it's in (an index into `Registry::dims`).
    pub dim: Option<usize>,
    pub share: f32,
    pub size: f32,
    pub top: Option<Id>,
    pub under: Option<Id>,
    pub plants: Vec<(Id, f32)>,
    pub tree: Option<ModTree>,
    /// Optional target on the Overworld's shared climate axes (revision 8+).
    pub climate: Option<ClimateTarget>,
}

/// Each specified axis is in -1..=1; omitted axes do not affect distance.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClimateTarget {
    pub temperature: Option<f32>,
    pub moisture: Option<f32>,
    pub continentalness: Option<f32>,
    pub erosion: Option<f32>,
}

impl ClimateTarget {
    /// A target occupies nearby climate only; a lone hot/dry mod cannot
    /// take every cold/wet inland column just by being the nearest candidate.
    pub fn accepts(&self, p: &crate::land::shape::Place) -> bool {
        [(self.temperature, p.t), (self.moisture, p.m), (self.continentalness, p.c), (self.erosion, p.e)]
            .iter().all(|&(target, actual)| target.is_none_or(|t| (actual - t).abs() <= 0.35))
    }

    pub fn distance(&self, p: &crate::land::shape::Place) -> f32 {
        let axes = [(self.temperature, p.t), (self.moisture, p.m), (self.continentalness, p.c), (self.erosion, p.e)];
        let mut distance = 0.0;
        let mut count = 0;
        for (target, actual) in axes {
            if let Some(target) = target {
                distance += ((actual - target) / 2.0).powi(2);
                count += 1;
            }
        }
        distance / count.max(1) as f32
    }
}

/// Where a structure may go: a base biome, or a mod one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Place {
    Base(Biome),
    Mod(usize),
}

#[derive(Clone, Debug)]
pub struct ModStructure {
    pub key: String,
    pub name: &'static str,
    /// The mod dimension it's built in (None: the Overworld).
    pub dim: Option<usize>,
    /// The biomes it's built in (none listed: anywhere on dry land).
    pub places: Vec<Place>,
    /// Grid cell size in chunks, chance per cell, and how far it's sunk into the ground.
    pub spacing: i32,
    pub chance: f32,
    pub sink: i32,
    /// Width (x), height (y) and depth (z), and the blocks, `(y * d + z) * w + x`;
    /// None leaves what's there.
    pub size: [i32; 3],
    pub blocks: Vec<Option<Id>>,
    /// Chests in it get: (item, up to how many, chance).
    pub loot: Vec<(Id, u8, f32)>,
}

impl ModStructure {
    pub fn at(&self, x: i32, y: i32, z: i32) -> Option<Id> {
        let [w, _, d] = self.size;
        self.blocks.get(((y * d + z) * w + x) as usize).copied().flatten()
    }
}

#[derive(Clone, Debug)]
pub struct ModDim {
    pub key: String,
    pub name: &'static str,
    pub stone: Id,
    pub top: Id,
    pub under: Id,
    /// The sea's liquid (AIR: none) and its level.
    pub liquid: Id,
    pub sea_level: i32,
    /// Ground level, how far hills rise and fall from it, and how wide they are.
    pub height: i32,
    pub hills: f32,
    pub scale: f32,
    pub sky: [f32; 3],
    /// Light everywhere, 0 (dark) to 1 (noon), on top of block light.
    pub light: f32,
    /// The portal's frame block, and its sheets (across x, across z).
    pub frame: Id,
    pub portal: [Id; 2],
}

/// The mod dimension a portal sheet leads to (an index into `Registry::dims`).
pub fn portal_dim(id: Id) -> Option<usize> {
    reg().dims.iter().position(|d| d.portal.contains(&id))
}

/// Does a structure's "in" list allow this column?
fn fits(places: &[Place], base: Biome, mod_biome: Option<usize>) -> bool {
    places.is_empty() || places.iter().any(|p| match *p {
        Place::Base(b) => b == base && mod_biome.is_none(),
        Place::Mod(m) => Some(m) == mod_biome,
    })
}

impl Generator {
    /// The mod dimension this generates, if it's one.
    pub fn mod_dim(&self) -> Option<&ModDim> {
        match self.dim {
            Dim::Mod(k) => self.dims.get(k as usize),
            _ => None,
        }
    }

    /// Ground height in a mod dimension.
    pub fn mod_height(&self, d: &ModDim, x: i32, z: i32) -> i32 {
        let s = d.scale.max(8.0);
        let n = self.hills.fbm2(x as f32 / s + 311.0, z as f32 / s - 177.0, 3);
        (d.height as f32 + n * d.hills).round().clamp(4.0, (CH - 8) as f32) as i32
    }

    /// The mod biome at this column, if any takes it over (`base`: the base biome there).
    pub fn mod_biome_at(&self, x: i32, z: i32, base: Biome) -> Option<usize> {
        if self.biomes.is_empty() {
            return None;
        }
        let here = match self.dim {
            Dim::Mod(k) => Some(k as usize),
            Dim::Over => None,
            _ => return None,
        };
        // Legacy patches retain their index salts and first-match ordering.
        // Target biomes never participate in old worlds or mod dimensions.
        let patch = |i: usize, b: &ModBiome| {
            if b.share >= 0.999 { return true; }
            let off = 7919.0 * (i as f32 + 1.0);
            let n = self.continent.noise2((x as f32 + off) / b.size.max(16.0), (z as f32 - off) / b.size.max(16.0));
            n > 0.45 - b.share * 0.9
        };
        let legacy = self.biomes.iter().enumerate().find_map(|(i, b)| {
            if b.climate.is_some() { return None; }
            let allowed = match here {
                Some(k) => b.dim == Some(k),
                None => b.dim.is_none() && b.within.contains(&base),
            };
            if !allowed {
                return None;
            }
            patch(i, b).then_some(i)
        });
        if legacy.is_some() || here.is_some() || self.opts.version < 8 || !self.biomes.iter().any(|b| b.climate.is_some()) {
            return legacy;
        }
        let p = self.place_v7(x, z);
        if !crate::land::shape::mod_climate_space(&p, base) { return None; }
        self.biomes.iter().enumerate().filter_map(|(i, b)| {
            let target = b.climate.as_ref()?;
            (b.dim.is_none() && target.accepts(&p) && (b.within.is_empty() || b.within.contains(&base)) && patch(i, b))
                .then_some((i, target.distance(&p), b.key.as_str()))
        }).min_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.2.cmp(b.2))).map(|c| c.0)
    }

    /// Dress a finished column in its mod biome: ground, plants. (Overworld and mod dimensions.)
    pub(crate) fn mod_biome_column(&self, b: &mut [Id], lx: i32, lz: i32, x: i32, z: i32, base: Biome) {
        let Some(i) = self.mod_biome_at(x, z, base) else { return };
        let mb = &self.biomes[i];
        // The surface: the highest solid block with air (or a plant) above.
        let Some(h) = (1..CH - 1).rev().find(|&y| is_solid(b[idx(lx, y, lz)]) && !is_solid(b[idx(lx, y + 1, lz)]) && !is_liquid(b[idx(lx, y + 1, lz)])) else { return };
        if let Some(top) = mb.top {
            b[idx(lx, h, lz)] = top;
        }
        if let Some(under) = mb.under {
            for y in (h - 3).max(1)..h {
                let i = idx(lx, y, lz);
                if matches!(b[i], DIRT | SAND | GRAVEL | STONE | SANDSTONE | RED_SAND | MUD | SNOW_BLOCK) || reg().dims.iter().any(|d| d.under == b[i]) {
                    b[i] = under;
                }
            }
        }
        // Its own plants instead of the base biome's.
        let above = idx(lx, h + 1, lz);
        if b[above] != AIR && !is_solid(b[above]) && !is_liquid(b[above]) {
            b[above] = AIR;
        }
        for (pi, &(plant, chance)) in mb.plants.iter().enumerate() {
            if hash2(self.seed ^ (0xB10E + i as u32 * 31 + pi as u32 * 7), x, z) < chance {
                b[above] = plant;
                break;
            }
        }
    }

    /// A mod biome's tree rooted at this column: (ground height, the tree).
    pub(crate) fn mod_tree_at(&self, x: i32, z: i32, h: i32, base: Biome) -> Option<(i32, &ModTree)> {
        let i = self.mod_biome_at(x, z, base)?;
        let t = self.biomes[i].tree.as_ref()?;
        (hash2(self.seed ^ (0x7EE + i as u32 * 97), x, z) < t.chance).then_some((h, t))
    }

    /// Plant mod biomes' trees in this chunk (they may reach in from neighbours).
    pub(crate) fn mod_trees(&self, cx: i32, cz: i32, b: &mut [Id], column: impl Fn(i32, i32) -> (i32, Biome)) {
        if !self.biomes.iter().any(|m| m.tree.is_some()) {
            return;
        }
        for tz in cz * CW - 3..cz * CW + CW + 3 {
            for tx in cx * CW - 3..cx * CW + CW + 3 {
                let (h, base) = column(tx, tz);
                let Some((h, t)) = self.mod_tree_at(tx, tz, h, base) else { continue };
                let trunk = t.height + (hash2(self.seed ^ 0x7EF, tx, tz) * 2.0) as i32;
                // (Only on dry land.)
                if (0..CW).contains(&(tx - cx * CW)) && (0..CW).contains(&(tz - cz * CW)) && is_liquid(b[idx(tx - cx * CW, h + 1, tz - cz * CW)]) {
                    continue;
                }
                let mut put = |p: IVec3, id: Id, over: bool| {
                    let (lx, lz) = (p.x - cx * CW, p.z - cz * CW);
                    if (0..CW).contains(&lx) && (0..CW).contains(&lz) && (1..CH).contains(&p.y) {
                        let i = idx(lx, p.y, lz);
                        if over || b[i] == AIR || (!is_solid(b[i]) && !is_liquid(b[i])) {
                            b[i] = id;
                        }
                    }
                };
                for y in 1..=trunk {
                    put(ivec3(tx, h + y, tz), t.log, true);
                }
                let top = ivec3(tx, h + trunk, tz);
                for dy in -2..=2i32 {
                    let r = if dy >= 1 { 1 } else { 2 };
                    for dz in -r..=r {
                        for dx in -r..=r {
                            if dx * dx + dz * dz + dy * dy <= r * r + 1 && !(dx == 0 && dz == 0 && dy <= 0) && hash3(self.seed ^ 0x1EB, tx + dx, h + trunk + dy, tz + dz) < 0.92 {
                                put(top + ivec3(dx, dy, dz), t.leaves, false);
                            }
                        }
                    }
                }
            }
        }
    }

    /// A chunk of a mod dimension.
    pub fn generate_mod(&self, cx: i32, cz: i32) -> Vec<Id> {
        let mut b = vec![AIR; crate::world::CHUNK_VOL];
        let Some(d) = self.mod_dim() else { return b };
        let s = self.seed;
        let k = match self.dim {
            Dim::Mod(k) => k as usize,
            _ => 0,
        };
        for lz in 0..CW {
            for lx in 0..CW {
                let (x, z) = (cx * CW + lx, cz * CW + lz);
                let h = self.mod_height(d, x, z);
                let wet = d.liquid != AIR && h < d.sea_level;
                for y in 0..CH.min(h.max(if d.liquid != AIR { d.sea_level } else { 0 }) + 1) {
                    let id = if y == 0 || (y <= 2 && hash3(s, x, y, z) < 0.5) {
                        BEDROCK
                    } else if y < h - 3 {
                        d.stone
                    } else if y < h {
                        d.under
                    } else if y == h {
                        if wet { d.under } else { d.top }
                    } else {
                        d.liquid
                    };
                    b[idx(lx, y, lz)] = id;
                }
                // Its ores, then its biomes and plants.
                for y in 1..h {
                    let i = idx(lx, y, lz);
                    for (oi, ore) in self.ores.iter().enumerate().filter(|(_, o)| o.dim == Some(k)) {
                        if b[i] == ore.replace && (ore.min_y..=ore.max_y).contains(&y) {
                            let salt = 0x5100 + oi as u32 * 7;
                            if hash3(s ^ salt, x >> 1, y >> 1, z >> 1) < ore.chance * 3.0 && hash3(s ^ (salt + 1), x, y, z) < 0.4 {
                                b[i] = ore.block;
                                break;
                            }
                        }
                    }
                }
                if !wet {
                    self.mod_biome_column(&mut b, lx, lz, x, z, Biome::Plains);
                    let top = h + 1;
                    if top < CH && b[idx(lx, top, lz)] == AIR {
                        let below = b[idx(lx, h, lz)];
                        for (pi, p) in self.plants.iter().enumerate().filter(|(_, p)| p.dim == Some(k)) {
                            if below == p.on && hash2(s ^ (0x9100 + pi as u32 * 13), x, z) < p.chance {
                                b[idx(lx, top, lz)] = p.block;
                                break;
                            }
                        }
                    }
                }
            }
        }
        self.mod_trees(cx, cz, &mut b, |x, z| (self.mod_height(d, x, z), Biome::Plains));
        self.place_mod_structures(cx, cz, &mut b);
        b
    }

    /// The structures that could reach into chunk (cx, cz): (structure, origin column, its ground height).
    fn mod_structure_origins(&self, cx: i32, cz: i32) -> Vec<(usize, IVec3)> {
        let here = match self.dim {
            Dim::Over => None,
            Dim::Mod(k) => Some(k as usize),
            _ => return Vec::new(),
        };
        let mut out = Vec::new();
        for (si, st) in self.structures.iter().enumerate() {
            if st.dim != here || (here.is_none() && self.opts.structures == 0) {
                continue;
            }
            let sp = st.spacing.max(2);
            for gz in (cz - 2).div_euclid(sp)..=(cz + 2).div_euclid(sp) {
                for gx in (cx - 2).div_euclid(sp)..=(cx + 2).div_euclid(sp) {
                    let salt = 0x57_0000 + si as u32 * 131;
                    if hash2(self.seed ^ salt, gx, gz) >= st.chance {
                        continue;
                    }
                    let ox = (gx * sp + (hash2(self.seed ^ (salt + 1), gx, gz) * (sp - 1) as f32) as i32) * CW + 8;
                    let oz = (gz * sp + (hash2(self.seed ^ (salt + 2), gx, gz) * (sp - 1) as f32) as i32) * CW + 8;
                    // Only if its chunk is near enough to reach this one.
                    let [w, _, dd] = st.size;
                    let (x0, z0) = (ox - w / 2, oz - dd / 2);
                    let (x1, z1) = (x0 + w - 1, z0 + dd - 1);
                    let apron = if self.opts.version >= 8 && self.dim == Dim::Over { crate::structures::SETTLE_RADIUS } else { 0 };
                    if x1 + apron < cx * CW || x0 - apron >= cx * CW + CW || z1 + apron < cz * CW || z0 - apron >= cz * CW + CW {
                        continue;
                    }
                    let (h, base, dry) = match self.mod_dim() {
                        Some(d) => {
                            let h = self.mod_height(d, ox, oz);
                            (h, Biome::Plains, d.liquid == AIR || h >= d.sea_level)
                        }
                        None => {
                            let (h, b) = self.column(ox, oz);
                            (h, b, h > self.sea() && !b.is_ocean())
                        }
                    };
                    if !dry || !fits(&st.places, base, self.mod_biome_at(ox, oz, base)) {
                        continue;
                    }
                    if self.opts.version >= 8 && !self.footprint_fits(ivec3(ox, h, oz), (x0 - ox, x1 - ox, z0 - oz, z1 - oz), 3, |b, m| fits(&st.places, b, m)) {
                        continue;
                    }
                    out.push((si, ivec3(ox, h + 1 - st.sink, oz)));
                }
            }
        }
        out
    }

    /// Stamp mod structures into a chunk's blocks.
    pub(crate) fn place_mod_structures(&self, cx: i32, cz: i32, b: &mut [Id]) {
        for (si, o) in self.mod_structure_origins(cx, cz) {
            let st = &self.structures[si];
            let [w, h, d] = st.size;
            let (x0, z0) = (o.x - w / 2, o.z - d / 2);
            if self.opts.version >= 8 && self.dim == Dim::Over {
                let mut blocks = Vec::new();
                for y in 0..h {
                    for z in 0..d {
                        for x in 0..w {
                            if let Some(id) = st.at(x, y, z) { blocks.push((ivec3(x0 + x, o.y + y, z0 + z), id)); }
                        }
                    }
                }
                self.settle(o, &blocks, cx, cz, b);
            }
            for y in 0..h {
                for z in 0..d {
                    for x in 0..w {
                        let Some(id) = st.at(x, y, z) else { continue };
                        let (lx, lz, py) = (x0 + x - cx * CW, z0 + z - cz * CW, o.y + y);
                        if (0..CW).contains(&lx) && (0..CW).contains(&lz) && (1..CH).contains(&py) {
                            b[idx(lx, py, lz)] = id;
                        }
                    }
                }
            }
        }
    }

    /// Chests that mod structures put in this chunk: (where, structure, a seed for its loot).
    pub fn mod_structure_chests(&self, cx: i32, cz: i32) -> Vec<(IVec3, usize, u32)> {
        let mut out = Vec::new();
        for (si, o) in self.mod_structure_origins(cx, cz) {
            let st = &self.structures[si];
            if st.loot.is_empty() {
                continue;
            }
            let [w, h, d] = st.size;
            let (x0, z0) = (o.x - w / 2, o.z - d / 2);
            for y in 0..h {
                for z in 0..d {
                    for x in 0..w {
                        if st.at(x, y, z).is_some_and(crate::chests::is_chest) {
                            let p = ivec3(x0 + x, o.y + y, z0 + z);
                            if p.x.div_euclid(CW) == cx && p.z.div_euclid(CW) == cz {
                                out.push((p, si, (hash3(self.seed ^ 0x100F, p.x, p.y, p.z) * u32::MAX as f32) as u32));
                            }
                        }
                    }
                }
            }
        }
        out
    }

    /// The nearest place structure `si` was built, near `from` (for /locate).
    pub fn nearest_mod_structure(&self, si: usize, from: IVec3, chunks: i32) -> Option<IVec3> {
        let (cx, cz) = (from.x.div_euclid(CW), from.z.div_euclid(CW));
        let mut best: Option<(i32, IVec3)> = None;
        for r in 0..chunks {
            for dz in -r..=r {
                for dx in -r..=r {
                    if dx.abs().max(dz.abs()) != r {
                        continue;
                    }
                    for (s, o) in self.mod_structure_origins(cx + dx, cz + dz) {
                        let dist = (o.x - from.x).pow(2) + (o.z - from.z).pow(2);
                        if s == si && best.is_none_or(|b| dist < b.0) {
                            best = Some((dist, o));
                        }
                    }
                }
            }
            if best.is_some() && r > 2 {
                break;
            }
        }
        best.map(|b| b.1)
    }
}

/// A mod structure chest's contents.
pub fn mod_loot(si: usize, seed: u32) -> crate::containers::Container {
    let mut c = crate::containers::Container::for_block(CHEST);
    let Some(st) = reg().structures.get(si) else { return c };
    let mut rng = crate::noise::Rng::new(seed as u64);
    for &(item, most, chance) in &st.loot {
        if rng.chance(chance) {
            let n = rng.int(1, most.max(1) as i32) as u8;
            let slot = rng.int(0, c.slots.len() as i32 - 1) as usize;
            if c.slots[slot].is_none() {
                c.slots[slot] = Some((item, n.min(max_stack(item))));
            } else if let Some(free) = c.slots.iter().position(|s| s.is_none()) {
                c.slots[free] = Some((item, n.min(max_stack(item))));
            }
        }
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::GenOptions;

    const LAND_MOD: &str = r#"
[mod]
name = Moon Cheese

[block cheese]
texture = white
[block rind]
texture = white
[block moon_rock]
texture = stone
[block mooncheese_portal_frame]
texture = white

[biome cheese_fields]
name = Cheese Fields
in = plains, forest, birch_forest, savanna, meadow, taiga, snowy, desert, jungle, dark_forest, cherry_grove
share = 1.0
top = cheese
under = rind
plant = cheese 0.05
tree = rind cheese 0.02 4

[structure cheese_hut]
in = cheese_fields
spacing = 4
chance = 1.0
loot = diamond 3 1.0
block # = rind
block c = chest
block . = air
layer
###
#c#
###
layer
#.#
...
#.#
layer
###
###
###

[dimension moon]
name = The Moon
stone = moon_rock
top = cheese
under = rind
height = 60
hills = 6
frame = mooncheese_portal_frame
sky = #102030
light = 0.6

[biome craters]
in = moon
share = 1.0
plant = moon_rock 0.03

[ore cheese_veins]
block = cheese
replace = moon_rock
dimension = moon
chance = 0.03
min_y = 2
max_y = 50
"#;

    #[test]
    fn review_mod_settling_apron_reaches_the_next_chunk() {
        let text = include_str!("../../example-mods/climate/mod.txt").replace("#####","#############").replace("#...#","#...........#").replace("#.c.#","#.....c.....#").replace("##.##","######.######");
        crate::mods::with_mods(&[("climate",&text)], |_| {
            let g = Generator::with(424242,GenOptions::DEFAULT);
            let o = (-80..80).flat_map(|cz| (-80..80).map(move |cx| (cx,cz)))
                .find_map(|(cx,cz)| g.mod_structure_origins(cx,cz).into_iter().map(|(_,o)|o).find(|o| {
                    let (h,b)=g.column(o.x+8,o.z); h>g.sea() && !b.is_ocean() && b!=Biome::River
                })).unwrap();
            let (cx,cz) = ((o.x+8).div_euclid(CW),o.z.div_euclid(CW));
            assert_eq!(cx,o.x.div_euclid(CW)+1);
            let mut b = vec![AIR;(CW*CW*CH) as usize];
            for lz in 0..CW { for lx in 0..CW { for y in 0..o.y-4 { b[idx(lx,y,lz)]=STONE; } } }
            g.place_mod_structures(cx,cz,&mut b);
            assert!(is_solid(b[idx(0,o.y-2,o.z.rem_euclid(CW))]),"the apron continues beyond the template's chunk");
        });
    }

    #[test]
    fn climate_biomes_choose_the_nearest_target_and_protect_landmarks() {
        let text = "[mod]\nname = Climate\n[biome z_dry]\nshare = 1\nclimate_moisture = -0.3\n[biome a_wet]\nshare = 1\nclimate_moisture = 0.3\n";
        crate::mods::with_mods(&[("climate", text)], |reg| {
            assert!(reg.mods[0].errors.is_empty());
            let g = Generator::with(77, GenOptions::DEFAULT);
            let old = Generator::with(77, GenOptions { version: 7, ..GenOptions::DEFAULT });
            let mut chosen = [0, 0];
            let mut protected = 0;
            for z in (-2500..2500).step_by(64) {
                for x in (-2500..2500).step_by(64) {
                    let (_, base) = g.column(x, z);
                    let p = g.place_v7(x, z);
                    assert_eq!(old.mod_biome_at(x, z, base), None);
                    if crate::land::shape::mod_climate_space(&p, base) && p.m.abs() <= 0.65 {
                        let index = g.mod_biome_at(x, z, base).unwrap();
                        let expected = if p.m < 0.0 { 0 } else { 1 };
                        assert_eq!(index, expected);
                        chosen[index] += 1;
                    } else {
                        assert_eq!(g.mod_biome_at(x, z, base), None, "protected {base:?}");
                        protected += 1;
                    }
                }
            }
            assert!(chosen.iter().all(|&n| n > 100) && protected > 100);
            let mut tie = Generator::with(77, GenOptions::DEFAULT);
            tie.biomes[0].climate = tie.biomes[1].climate.clone();
            let (x, z) = (-2000..2000).step_by(64).flat_map(|z| (-2000..2000).step_by(64).map(move |x| (x,z))).find(|&(x,z)| tie.mod_biome_at(x,z,tie.column(x,z).1).is_some()).unwrap();
            assert_eq!(tie.mod_biome_at(x,z,tie.column(x,z).1), Some(1), "ties use the stable biome key");
            tie.biomes.swap(0, 1);
            assert_eq!(tie.mod_biome_at(x,z,tie.column(x,z).1), Some(0));
        });
    }

    #[test]
    fn climate_example_and_wide_templates_have_chests_and_foundations() {
        let example = include_str!("../../example-mods/climate/mod.txt");
        let wide = example.replace("#####","#################").replace("#...#","#...............#").replace("#.c.#","#.......c.......#").replace("##.##","########.########");
        for text in [example,wide.as_str()] {
        crate::mods::with_mods(&[("climate", text)], |reg| {
            assert!(reg.mods[0].errors.is_empty(), "{:?}",reg.mods[0].errors);
            let g = Generator::with(424242, GenOptions::DEFAULT);
            let (si,o)=(-80..80).flat_map(|cz|(-80..80).map(move|cx|(cx,cz))).find_map(|(cx,cz)|g.mod_structure_origins(cx,cz).first().copied()).expect("a climate-target hut");
            let st=&reg.structures[si];
            let [w,h,d]=st.size;
            let (x0,z0)=(o.x-w/2,o.z-d/2);
            let mut chunks=std::collections::HashMap::new();
            let mut chests=0;
            for z in 0..d { for x in 0..w {
                let (xx,zz)=(x0+x,z0+z);
                assert_eq!(g.mod_biome_at(xx,zz,g.column(xx,zz).1),Some(0));
                let key=(xx.div_euclid(CW),zz.div_euclid(CW));
                let b=chunks.entry(key).or_insert_with(||g.generate(key.0,key.1));
                for y in 0..h {
                    if let Some(id)=st.at(x,y,z) {
                        assert_eq!(b[idx(xx.rem_euclid(CW),o.y+y,zz.rem_euclid(CW))],id);
                        if id==CHEST {
                            chests+=1;
                            assert!(g.mod_structure_chests(key.0,key.1).iter().any(|&(p,index,_)|p==ivec3(xx,o.y+y,zz) && index==si));
                        }
                    }
                }
                assert!(is_solid(b[idx(xx.rem_euclid(CW),o.y-1,zz.rem_euclid(CW))]), "supported floor at {xx},{zz}");
            }}
            assert!(chests>0);
            if w>16 { assert!(chunks.len()>1,"the wide hut crosses a chunk boundary"); }
        });
        }
    }

    #[test]
    fn targetless_mod_patches_keep_their_released_selection() {
        crate::mods::with_mods(&[("moon", LAND_MOD)], |_| {
            let old = Generator::with(77, GenOptions { version: 7, ..GenOptions::DEFAULT });
            let new = Generator::with(77, GenOptions::DEFAULT);
            for z in (-2000..2000).step_by(48) {
                for x in (-2000..2000).step_by(48) {
                    let base = old.column(x,z).1;
                    assert_eq!(new.mod_biome_at(x,z,base), old.mod_biome_at(x,z,base));
                }
            }
        });
    }

    #[test]
    fn mod_biomes_structures_and_dimensions_generate() {
        crate::mods::with_mods(&[("moon", LAND_MOD)], |reg| {
            let errs: Vec<&String> = reg.mods.iter().flat_map(|m| m.errors.iter()).collect();
            assert!(errs.is_empty(), "{errs:?}");
            assert_eq!(reg.biomes.len(), 2);
            assert_eq!(reg.structures.len(), 1);
            assert_eq!(reg.dims.len(), 1);
            let cheese = reg.lookup("moon:cheese").unwrap();
            let rind = reg.lookup("moon:rind").unwrap();
            // The Overworld: cheese fields everywhere there's plains or forest, with huts.
            let g = Generator::with(5, GenOptions::DEFAULT);
            let mut tops = 0;
            let mut huts = 0;
            for cz in -6..6 {
                for cx in -6..6 {
                    let b = g.generate(cx, cz);
                    for lz in 0..CW {
                        for lx in 0..CW {
                            let (h, biome) = g.column(cx * CW + lx, cz * CW + lz);
                            if h > g.sea() + 1 && biome == Biome::Plains && b[idx(lx, h, lz)] == cheese {
                                tops += 1;
                            }
                        }
                    }
                    huts += g.mod_structure_chests(cx, cz).len();
                }
            }
            assert!(tops > 50, "cheese fields: {tops}");
            assert!(huts > 0, "huts with chests");
            let c = mod_loot(0, 99);
            assert!(c.slots.iter().flatten().any(|s| s.0 == DIAMOND && s.1 <= 3));
            // The Moon: its own ground, ores and portal blocks.
            let m = Generator::with_dim(5, GenOptions::DEFAULT, Dim::Mod(0));
            let b = m.generate(0, 0);
            let h = m.mod_height(&reg.dims[0], 3, 3);
            assert!((50..=70).contains(&h));
            assert!(b.iter().any(|&id| id == reg.lookup("moon:moon_rock").unwrap()));
            assert!(b.contains(&rind));
            assert!(portal_dim(reg.dims[0].portal[0]) == Some(0) && portal_dim(reg.dims[0].portal[1]) == Some(0));
            assert_eq!(Dim::from_index(Dim::Mod(0).index()), Some(Dim::Mod(0)));
            assert_eq!(Dim::Mod(0).name(), "The Moon");
        });
    }

    #[test]
    fn a_mod_portal_leads_to_its_dimension_and_back() {
        crate::mods::with_mods(&[("moon", LAND_MOD)], |reg| {
            let frame = reg.lookup("moon:mooncheese_portal_frame").unwrap();
            let mut g = crate::game::tests::arena(974);
            let base = IVec3::new(3, 50, -3);
            for dx in -1..=2 {
                for dy in -1..=3 {
                    if dx == -1 || dx == 2 || dy == -1 || dy == 3 {
                        g.world.set_v(base + IVec3::new(dx, dy, 0), frame);
                    }
                }
            }
            // An obsidian-lit portal it isn't; its own one it is.
            assert!(crate::scorch::portal_frame_of(&g.world, base, true, None).is_none());
            g.inv.slots = [None; 36];
            g.inv.slots[0] = Some((SPARKER, 1));
            g.inv.selected = 0;
            assert!(g.use_sparker(base - IVec3::Y, IVec3::Y));
            assert_eq!(g.world.get_v(base), reg.dims[0].portal[0], "lit with the Moon's sheets");
            g.player.body.pos = base.as_vec3() + macroquad::math::Vec3::new(1.0, 0.0, 0.5);
            for _ in 0..50 {
                g.portal_tick(0.05);
            }
            assert_eq!((g.dim, g.realm_dim()), (Dim::Mod(0), Dim::Mod(0)));
            assert!(g.advancements.has("off_the_map"));
            let there = g.player.body.pos;
            assert!((there.x - base.x as f32).abs() < 40.0 && (there.z - base.z as f32).abs() < 40.0, "the same place there: {there}");
            assert_eq!(portal_dim(g.world.get_v(there.floor().as_ivec3())), Some(0), "arrived in one of its portals");
            assert_eq!(g.sky_color(), reg.dims[0].sky);
            assert!(g.elsewhere());
            // Out and back in: home.
            g.player.body.pos += macroquad::math::Vec3::new(0.0, 0.0, 2.0);
            g.portal_tick(0.05);
            g.player.body.pos = there;
            // (After the cooldown, a couple of seconds in it.)
            for _ in 0..160 {
                g.portal_tick(0.05);
            }
            assert_eq!((g.dim, g.realm_dim()), (Dim::Over, Dim::Over));
            // Saved and loaded, the Moon's realm comes back by name.
            let sections = g.save_realms();
            assert!(sections.iter().any(|(k, _)| k == "realm_mod_moon_moon"), "{:?}", sections.iter().map(|s| &s.0).collect::<Vec<_>>());
        });
    }
}
