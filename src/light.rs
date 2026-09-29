//! Real light: every cell has a sky level and a block level, 0 to 15, spread
//! Minecraft style.
//!
//! - **Sky light** is 15 wherever the sky is straight above (through air,
//!   glass, plants), and loses one level per step sideways or under an
//!   overhang. Leaves and water let it through but dim it.
//! - **Block light** starts at whatever glows (torches 14, lava 15, lamps...)
//!   and loses one level per step.
//! - Solid blocks stop light. Slabs, stairs and other part-blocks take the
//!   light around them (so they look lit) but don't pass it on.
//!
//! Levels are kept per chunk and updated as chunks arrive and blocks change,
//! by spreading increases and retracting removed light (the usual two-queue
//! flood fill), across chunk borders. Unloaded chunks count as dark and pass
//! nothing on, so a chunk arriving only ever adds light.

use crate::block::*;
use crate::world::{idx, World, CH, CW};
use macroquad::math::{ivec3, IVec3};
use std::collections::VecDeque;

pub const MAX: u8 = 15;
const SECTION: usize = (CW * CW * 16) as usize;
const SECTIONS: usize = (CH / 16) as usize;

/// Levels for one chunk, a byte per cell (sky in the high four bits, block in
/// the low four). Sections that are all the same (open sky, solid rock) are
/// kept as just that byte.
#[derive(Clone)]
pub struct LightStore {
    fill: [u8; SECTIONS],
    cells: [Option<Box<[u8; SECTION]>>; SECTIONS],
}

impl Default for LightStore {
    fn default() -> Self {
        LightStore { fill: [0; SECTIONS], cells: Default::default() }
    }
}

impl LightStore {
    #[inline]
    pub fn get(&self, i: usize) -> u8 {
        let s = i / SECTION;
        match &self.cells[s] {
            Some(c) => c[i % SECTION],
            None => self.fill[s],
        }
    }
    #[inline]
    pub fn set(&mut self, i: usize, v: u8) {
        let s = i / SECTION;
        if self.cells[s].is_none() {
            if self.fill[s] == v {
                return;
            }
            self.cells[s] = Some(Box::new([self.fill[s]; SECTION]));
        }
        if let Some(c) = &mut self.cells[s] {
            c[i % SECTION] = v;
        }
    }
    /// Make a whole section one value (only used before anything spreads).
    fn fill_section(&mut self, s: usize, v: u8) {
        self.fill[s] = v;
        self.cells[s] = None;
    }
    /// Sections stored cell by cell (for the debug screen).
    pub fn detailed(&self) -> usize {
        self.cells.iter().filter(|c| c.is_some()).count()
    }
}

/// How light treats a block.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Pass {
    /// Air, glass, plants: one level per step, and sky light falls straight through.
    Clear,
    /// Leaves (one level) and liquids (two): passes light, but dims sky light.
    Dim(u8),
    /// Slabs, stairs and the like: lit by their neighbours, pass nothing on.
    Sink,
    /// Solid: dark inside, stops light.
    Solid,
}

pub fn pass(id: Id) -> Pass {
    if is_opaque(id) {
        Pass::Solid
    } else if dapples_sky(id) {
        Pass::Dim(1)
    } else if is_liquid(id) {
        Pass::Dim(2)
    } else if blocks_sky(id) {
        Pass::Sink
    } else {
        Pass::Clear
    }
}

/// How brightly a block glows (0 to 15). Blocks' `light` is a radius for the
/// old point lights (still used for things that move); levels go further.
pub fn emission(id: Id) -> u8 {
    if is_lava(id) {
        return MAX;
    }
    let r = block(id).light;
    if r <= 0.0 {
        return 0;
    }
    match id {
        TORCH => 14,
        FURNACE_LIT => 13,
        ENCHANTING_TABLE => 7,
        _ => ((r + 6.0).round() as i32).clamp(1, MAX as i32) as u8,
    }
}

/// Brightness (0..1) for a sky level: a gentle curve, so shade under an overhang
/// is soft and a cave a few steps in is properly dark.
pub fn sky_brightness(level: u8) -> f32 {
    (level as f32 / MAX as f32).powf(1.6)
}

/// Brightness (0..1) for a block light level.
pub fn block_brightness(level: u8) -> f32 {
    level as f32 / MAX as f32
}

#[derive(Clone, Copy, PartialEq)]
enum Channel {
    Sky,
    Block,
}

const DIRS: [IVec3; 6] = [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z, IVec3::Y, IVec3::NEG_Y];

/// Where a spread of light reaches next: `None` if it doesn't.
fn step(ch: Channel, from: u8, dir: IVec3, into: Pass) -> Option<u8> {
    let cost = match into {
        Pass::Solid => return None,
        Pass::Clear | Pass::Sink => 1,
        Pass::Dim(c) => c,
    };
    // Open sky pours straight down without fading.
    if ch == Channel::Sky && from == MAX && dir == IVec3::NEG_Y && into == Pass::Clear {
        return Some(MAX);
    }
    from.checked_sub(cost).filter(|&l| l > 0)
}

impl World {
    /// Raw light byte at a cell (unloaded: dark, above the world: open sky).
    #[inline]
    fn raw_light(&self, p: IVec3) -> Option<u8> {
        if p.y >= CH {
            return Some(MAX << 4);
        }
        if p.y < 0 {
            return None;
        }
        let c = self.chunks.get(&(p.x.div_euclid(CW), p.z.div_euclid(CW)))?;
        Some(c.light.get(idx(p.x.rem_euclid(CW), p.y, p.z.rem_euclid(CW))))
    }

    fn level(&self, ch: Channel, p: IVec3) -> Option<u8> {
        self.raw_light(p).map(|b| if ch == Channel::Sky { b >> 4 } else { b & 15 })
    }

    fn set_level(&mut self, ch: Channel, p: IVec3, v: u8) {
        if !(0..CH).contains(&p.y) {
            return;
        }
        let (cx, cz) = (p.x.div_euclid(CW), p.z.div_euclid(CW));
        let (lx, lz) = (p.x.rem_euclid(CW), p.z.rem_euclid(CW));
        let Some(c) = self.chunks.get_mut(&(cx, cz)) else { return };
        let i = idx(lx, p.y, lz);
        let old = c.light.get(i);
        let new = if ch == Channel::Sky { (old & 15) | (v << 4) } else { (old & 0xF0) | v };
        if new == old {
            return;
        }
        c.light.set(i, new);
        // Its mesh (and a neighbour's, at a border) shows this cell's light.
        let xs: &[i32] = if lx == 0 { &[-1, 0] } else if lx == CW - 1 { &[0, 1] } else { &[0] };
        let zs: &[i32] = if lz == 0 { &[-1, 0] } else if lz == CW - 1 { &[0, 1] } else { &[0] };
        for &dx in xs {
            for &dz in zs {
                self.dirty.insert((cx + dx, cz + dz));
            }
        }
    }

    /// Sky level at a cell (0..15).
    pub fn sky_level(&self, x: i32, y: i32, z: i32) -> u8 {
        self.level(Channel::Sky, ivec3(x, y, z)).unwrap_or(MAX)
    }

    /// Block light level at a cell (0..15).
    pub fn block_level(&self, x: i32, y: i32, z: i32) -> u8 {
        self.level(Channel::Block, ivec3(x, y, z)).unwrap_or(0)
    }

    /// Spread light outward from the queued cells (each already holding its level).
    fn spread(&mut self, ch: Channel, mut queue: VecDeque<IVec3>) {
        while let Some(p) = queue.pop_front() {
            let Some(l) = self.level(ch, p) else { continue };
            if l <= 1 {
                continue;
            }
            for d in DIRS {
                let n = p + d;
                if !(0..CH).contains(&n.y) {
                    continue;
                }
                let Some(have) = self.level(ch, n) else { continue };
                let into = pass(self.get_v(n));
                if let Some(v) = step(ch, l, d, into).filter(|&v| v > have) {
                    self.set_level(ch, n, v);
                    if into != Pass::Sink {
                        queue.push_back(n);
                    }
                }
            }
        }
    }

    /// Take away light that came through `p` (which held `level`), then fill
    /// back in from whatever still shines nearby.
    fn retract(&mut self, ch: Channel, p: IVec3, level: u8) {
        let mut gone = VecDeque::from([(p, level)]);
        let mut refill = VecDeque::new();
        while let Some((q, lq)) = gone.pop_front() {
            for d in DIRS {
                let n = q + d;
                let Some(ln) = self.level(ch, n) else { continue };
                if ln == 0 {
                    continue;
                }
                let fed_by_q = ln < lq || (ch == Channel::Sky && d == IVec3::NEG_Y && lq == MAX && ln == MAX);
                if fed_by_q {
                    // Its own glow survives.
                    let own = if ch == Channel::Block { emission(self.get_v(n)) } else { 0 };
                    self.set_level(ch, n, own);
                    gone.push_back((n, ln));
                    if own > 0 {
                        refill.push_back(n);
                    }
                } else {
                    refill.push_back(n);
                }
            }
        }
        self.spread(ch, refill);
    }

    /// A block changed at `p`: bring the light around it up to date.
    pub fn relight(&mut self, p: IVec3, old: Id, new: Id) {
        if pass(old) == pass(new) && emission(old) == emission(new) {
            return;
        }
        if !self.chunks.contains_key(&(p.x.div_euclid(CW), p.z.div_euclid(CW))) {
            return;
        }
        for ch in [Channel::Sky, Channel::Block] {
            let had = self.level(ch, p).unwrap_or(0);
            self.set_level(ch, p, 0);
            if had > 0 {
                self.retract(ch, p, had);
            }
            let mut queue = VecDeque::new();
            if ch == Channel::Block && emission(new) > 0 {
                self.set_level(ch, p, emission(new));
                queue.push_back(p);
            }
            // Whatever lights the neighbours may now come through.
            if pass(new) != Pass::Solid {
                for d in DIRS {
                    if self.level(ch, p + d).unwrap_or(0) > 0 || (ch == Channel::Sky && p.y + d.y >= CH) {
                        queue.push_back(p + d);
                    }
                }
            }
            self.spread(ch, queue);
        }
    }

    /// Light a chunk that just arrived, and let light flow between it and its neighbours.
    pub fn light_new_chunk(&mut self, cx: i32, cz: i32) {
        let (bx, bz) = (cx * CW, cz * CW);
        let mut sky = VecDeque::new();
        let mut blk = VecDeque::new();
        // Straight-down sky: from the top of each column to the first thing that isn't clear.
        let mut tops = [[CH; CW as usize]; CW as usize];
        {
            let Some(c) = self.chunks.get(&(cx, cz)) else { return };
            let mut store = LightStore::default();
            let lowest_top = (0..CW * CW).map(|i| column_top(c, i % CW, i / CW)).min().unwrap_or(0);
            for lz in 0..CW {
                for lx in 0..CW {
                    tops[lz as usize][lx as usize] = column_top(c, lx, lz);
                }
            }
            // Whole sections above every column are open sky.
            for s in 0..SECTIONS {
                if (s as i32) * 16 >= lowest_top {
                    store.fill_section(s, MAX << 4);
                }
            }
            for lz in 0..CW {
                for lx in 0..CW {
                    for y in tops[lz as usize][lx as usize]..CH {
                        store.set(idx(lx, y, lz), MAX << 4);
                    }
                }
            }
            // Glowing blocks.
            for y in 0..CH {
                if c.blocks.sections()[y as usize / 16].is_uniform(AIR) {
                    continue;
                }
                for lz in 0..CW {
                    for lx in 0..CW {
                        let e = emission(c.blocks.get(idx(lx, y, lz)));
                        if e > 0 {
                            let i = idx(lx, y, lz);
                            store.set(i, (store.get(i) & 0xF0) | e);
                            blk.push_back(ivec3(bx + lx, y, bz + lz));
                        }
                    }
                }
            }
            if let Some(c) = self.chunks.get_mut(&(cx, cz)) {
                c.light = store;
            }
        }
        // Sky cells that can spread: each column's lowest open cell, and any open
        // cell beside a taller column (the side of an overhang or a cliff).
        let top_at = |w: &World, x: i32, z: i32| -> i32 {
            match w.chunks.get(&(x.div_euclid(CW), z.div_euclid(CW))) {
                Some(c) => column_top(c, x.rem_euclid(CW), z.rem_euclid(CW)),
                None => 0,
            }
        };
        for lz in 0..CW {
            for lx in 0..CW {
                let (x, z) = (bx + lx, bz + lz);
                let top = tops[lz as usize][lx as usize];
                if top >= CH {
                    continue;
                }
                let near = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().map(|&(dx, dz)| top_at(self, x + dx, z + dz)).max().unwrap_or(0);
                for y in top..=near.min(CH - 1).max(top) {
                    sky.push_back(ivec3(x, y, z));
                }
            }
        }
        // Pull in light from neighbours already here.
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            if !self.chunks.contains_key(&(cx + dx, cz + dz)) {
                continue;
            }
            for y in 0..CH {
                for k in 0..CW {
                    let (x, z) = match (dx, dz) {
                        (1, 0) => (bx + CW, bz + k),
                        (-1, 0) => (bx - 1, bz + k),
                        (0, 1) => (bx + k, bz + CW),
                        _ => (bx + k, bz - 1),
                    };
                    let b = self.raw_light(ivec3(x, y, z)).unwrap_or(0);
                    if b >> 4 > 1 {
                        sky.push_back(ivec3(x, y, z));
                    }
                    if b & 15 > 1 {
                        blk.push_back(ivec3(x, y, z));
                    }
                }
            }
        }
        self.spread(Channel::Sky, sky);
        self.spread(Channel::Block, blk);
    }
}

/// The lowest cell of a column that sees straight up to the sky.
fn column_top(c: &crate::world::Chunk, lx: i32, lz: i32) -> i32 {
    let mut y = CH;
    while y > 0 && pass(c.blocks.get(idx(lx, y - 1, lz))) == Pass::Clear {
        y -= 1;
    }
    y
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::PalettedBlocks;

    /// A world of flat stone chunks (top at y 10) without the generator.
    fn flat(n: i32) -> World {
        let mut w = World::new(1);
        let mut ids = vec![AIR; (CW * CW * CH) as usize];
        for y in 0..10 {
            for z in 0..CW {
                for x in 0..CW {
                    ids[idx(x, y, z)] = STONE;
                }
            }
        }
        for cz in -n..=n {
            for cx in -n..=n {
                w.insert_chunk(cx, cz, PalettedBlocks::from_ids(&ids));
            }
        }
        w
    }

    #[test]
    fn open_sky_and_a_roof() {
        let mut w = flat(1);
        assert_eq!(w.sky_level(3, 10, 3), 15);
        assert_eq!(w.sky_level(3, 5, 3), 0);
        // A roof over a spot: shade underneath, lit from the sides.
        for x in 0..7 {
            for z in 0..7 {
                w.set(x, 12, z, STONE);
            }
        }
        let under = w.sky_level(3, 11, 3);
        assert!((10..15).contains(&under), "{under}");
        assert_eq!(w.sky_level(3, 13, 3), 15);
        // Take the roof away and the sky comes back.
        for x in 0..7 {
            for z in 0..7 {
                w.set(x, 12, z, AIR);
            }
        }
        assert_eq!(w.sky_level(3, 11, 3), 15);
    }

    #[test]
    fn torches_light_up_and_go_dark() {
        let mut w = flat(1);
        // Dig a sealed room and put a torch in it.
        for x in 2..9 {
            for z in 2..9 {
                w.set(x, 5, z, AIR);
            }
        }
        assert_eq!(w.sky_level(5, 5, 5), 0);
        assert_eq!(w.block_level(5, 5, 5), 0);
        w.set(5, 5, 5, TORCH);
        let t = emission(TORCH);
        assert!(t >= 10);
        assert_eq!(w.block_level(5, 5, 5), t);
        assert_eq!(w.block_level(7, 5, 5), t - 2);
        // Light crosses into the next chunk (x 16 is in the neighbour) where there's room.
        for x in 9..=16 {
            w.set(x, 5, 5, AIR);
        }
        w.set(5, 5, 5, AIR);
        w.set(13, 5, 5, TORCH);
        assert_eq!(w.block_level(16, 5, 5), t - 3);
        w.set(13, 5, 5, AIR);
        assert_eq!(w.block_level(16, 5, 5), 0);
        assert_eq!(w.block_level(12, 5, 5), 0);
    }

    #[test]
    fn a_new_chunk_lets_light_in() {
        let mut w = flat(0);
        // A tunnel running out of chunk (0,0) to the east, lit inside.
        for x in 10..16 {
            w.set(x, 5, 8, AIR);
        }
        w.set(10, 5, 8, TORCH);
        assert_eq!(w.block_level(15, 5, 8), emission(TORCH) - 5);
        // The chunk next door arrives with the tunnel continuing; the light follows.
        let mut ids = vec![AIR; (CW * CW * CH) as usize];
        for y in 0..10 {
            for z in 0..CW {
                for x in 0..CW {
                    if !(y == 5 && z == 8 && x < 3) {
                        ids[idx(x, y, z)] = STONE;
                    }
                }
            }
        }
        w.insert_chunk(1, 0, PalettedBlocks::from_ids(&ids));
        assert_eq!(w.block_level(16, 5, 8), emission(TORCH) - 6);
        assert_eq!(w.sky_level(20, 11, 3), 15);
    }
}
