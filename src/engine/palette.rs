//! Chunk block storage with per-section palettes, the way Minecraft does it.
//!
//! A chunk column is split into 16x16x16 sections. Each section keeps a small
//! list of the block ids it actually contains (its palette) and stores, per
//! block, just an index into that list, packed into as few bits as needed:
//!
//! - one kind of block (all air, all stone): 0 bits per block, just the palette;
//! - up to 16 kinds: 4 bits; up to 32: 5 bits; ... up to 256: 8 bits;
//! - more than 256 kinds: the ids themselves, 16 bits each ("direct").
//!
//! Like Minecraft, an index never straddles two `u64` words, so with 5, 6 or 7
//! bits a few bits per word go unused. Palettes only grow while playing; they
//! are rebuilt compactly whenever a chunk is (re)generated.

use crate::block::Id;
#[cfg(test)]
use crate::block::AIR;

pub const SECTION: usize = 16 * 16 * 16;

#[derive(Clone)]
pub struct Section {
    /// Ids used in this section. Empty in direct mode.
    palette: Vec<Id>,
    /// Bits per block: 0 (uniform), 4..=8 (palette) or 16 (direct).
    bits: u32,
    data: Vec<u64>,
}

/// Smallest supported width that can index `n` palette entries.
fn bits_for(n: usize) -> u32 {
    match n {
        0 | 1 => 0,
        2..=16 => 4,
        _ if n <= 256 => usize::BITS - (n - 1).leading_zeros(),
        _ => 16,
    }
}

fn words_for(bits: u32) -> usize {
    if bits == 0 { 0 } else { SECTION.div_ceil((64 / bits) as usize) }
}

impl Section {
    pub fn uniform(id: Id) -> Section {
        Section { palette: vec![id], bits: 0, data: Vec::new() }
    }

    /// Pack 4096 ids (in `y, z, x` order) with the smallest palette that fits.
    pub fn from_ids(ids: &[Id]) -> Section {
        debug_assert_eq!(ids.len(), SECTION);
        let mut palette: Vec<Id> = Vec::new();
        // Most sections use few ids; a linear scan beats hashing at that size.
        for &id in ids {
            if !palette.contains(&id) {
                palette.push(id);
                if palette.len() > 256 {
                    break;
                }
            }
        }
        if palette.len() == 1 {
            return Section::uniform(palette[0]);
        }
        let bits = bits_for(palette.len());
        let mut s = Section { palette, bits, data: vec![0; words_for(bits)] };
        if bits == 16 {
            s.palette.clear();
        }
        if bits > 0 {
            for (i, &id) in ids.iter().enumerate() {
                let v = s.index_of(id).expect("id is in the palette");
                s.put(i, v);
            }
        }
        s
    }

    #[inline]
    fn raw(&self, i: usize) -> u32 {
        let per = (64 / self.bits) as usize;
        let shift = (i % per) as u32 * self.bits;
        ((self.data[i / per] >> shift) & ((1u64 << self.bits) - 1)) as u32
    }

    #[inline]
    fn put(&mut self, i: usize, v: u32) {
        let per = (64 / self.bits) as usize;
        let shift = (i % per) as u32 * self.bits;
        let mask = ((1u64 << self.bits) - 1) << shift;
        let w = &mut self.data[i / per];
        *w = (*w & !mask) | ((v as u64) << shift);
    }

    fn index_of(&self, id: Id) -> Option<u32> {
        if self.bits == 16 {
            return Some(id as u32);
        }
        self.palette.iter().position(|&p| p == id).map(|p| p as u32)
    }

    #[inline]
    pub fn get(&self, i: usize) -> Id {
        match self.bits {
            0 => self.palette[0],
            16 => self.raw(i) as Id,
            _ => self.palette[self.raw(i) as usize],
        }
    }

    /// Set block `i`; returns the old id.
    pub fn set(&mut self, i: usize, id: Id) -> Id {
        let old = self.get(i);
        if old == id {
            return old;
        }
        let v = match self.index_of(id) {
            Some(v) => v,
            None => {
                // A new id: grow the palette, and the bit width if it no longer fits.
                self.palette.push(id);
                let need = bits_for(self.palette.len());
                if need > self.bits {
                    self.resize(need);
                }
                self.index_of(id).expect("just added")
            }
        };
        self.put(i, v);
        old
    }

    /// Repack every block at a new width.
    fn resize(&mut self, bits: u32) {
        let ids: Vec<Id> = (0..SECTION).map(|i| self.get_with(i, self.bits)).collect();
        if bits == 16 {
            self.palette.clear();
        }
        self.bits = bits;
        self.data = vec![0; words_for(bits)];
        for (i, id) in ids.into_iter().enumerate() {
            let v = self.index_of(id).expect("repacked id is known");
            self.put(i, v);
        }
    }

    /// `get`, but reading as if the section were still `bits` wide (used mid-resize,
    /// when the palette has already gained its new entry).
    fn get_with(&self, i: usize, bits: u32) -> Id {
        match bits {
            0 => self.palette[0],
            16 => self.raw(i) as Id,
            _ => self.palette[self.raw(i) as usize],
        }
    }

    /// Unpack all 4096 ids into `out`, a word at a time (much faster than `get` in a loop).
    pub fn decode_into(&self, out: &mut [Id]) {
        let out = &mut out[..SECTION];
        match self.bits {
            0 => out.fill(self.palette[0]),
            bits => {
                let per = (64 / bits) as usize;
                let mask = (1u64 << bits) - 1;
                for (w, chunk) in self.data.iter().zip(out.chunks_mut(per)) {
                    let mut w = *w;
                    for o in chunk {
                        let v = (w & mask) as usize;
                        *o = if bits == 16 { v as Id } else { self.palette[v] };
                        w >>= bits;
                    }
                }
            }
        }
    }

    pub fn is_uniform(&self, id: Id) -> bool {
        self.bits == 0 && self.palette[0] == id
    }

    pub fn bits(&self) -> u32 {
        self.bits
    }

    /// Heap bytes used (palette plus packed data).
    pub fn bytes(&self) -> usize {
        self.palette.len() * std::mem::size_of::<Id>() + self.data.len() * 8
    }
}

/// A whole chunk column: `height / 16` sections stacked bottom to top.
#[derive(Clone)]
pub struct PalettedBlocks {
    sections: Vec<Section>,
}

impl PalettedBlocks {
    /// Pack a flat `y, z, x` ordered array (length a multiple of 4096).
    pub fn from_ids(ids: &[Id]) -> PalettedBlocks {
        PalettedBlocks { sections: ids.chunks(SECTION).map(Section::from_ids).collect() }
    }

    /// Block at flat index `i` (as produced by `world::idx`).
    #[inline]
    pub fn get(&self, i: usize) -> Id {
        self.sections[i / SECTION].get(i % SECTION)
    }

    /// Set the block at flat index `i`; returns the old id.
    #[inline]
    pub fn set(&mut self, i: usize, id: Id) -> Id {
        self.sections[i / SECTION].set(i % SECTION, id)
    }

    /// Unpack everything into a flat `y, z, x` array (length = sections * 4096).
    pub fn decode_into(&self, out: &mut [Id]) {
        for (s, chunk) in self.sections.iter().zip(out.chunks_mut(SECTION)) {
            s.decode_into(chunk);
        }
    }

    pub fn sections(&self) -> &[Section] {
        &self.sections
    }

    pub fn bytes(&self) -> usize {
        self.sections.iter().map(Section::bytes).sum()
    }

    /// Total packed bits across all blocks (for the average shown on the F3 screen).
    pub fn total_bits(&self) -> u64 {
        self.sections.iter().map(|s| s.bits() as u64 * SECTION as u64).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::noise::Rng;

    #[test]
    fn widths_grow_as_palettes_do() {
        assert_eq!([1, 2, 16, 17, 32, 33, 64, 65, 128, 129, 256, 257].map(bits_for), [0, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 16]);
        let mut s = Section::uniform(AIR);
        assert_eq!((s.bits(), s.bytes()), (0, 2));
        // Add ids one at a time; the section must always read back what was written.
        let mut shadow = vec![AIR; SECTION];
        for id in 1..=300u16 {
            let i = (id as usize * 997) % SECTION;
            assert_eq!(s.set(i, id), shadow[i]);
            shadow[i] = id;
        }
        assert_eq!(s.bits(), 16, "300 kinds is past the palette limit");
        assert!((0..SECTION).all(|i| s.get(i) == shadow[i]));
    }

    #[test]
    fn random_edits_match_a_plain_array() {
        let mut rng = Rng::new(42);
        let mut shadow: Vec<Id> = (0..SECTION * 8).map(|i| if i < SECTION * 3 { 3 } else { AIR }).collect();
        let mut p = PalettedBlocks::from_ids(&shadow);
        assert!(p.sections()[0].is_uniform(3) && p.sections()[7].is_uniform(AIR));
        for round in 0..20_000 {
            let i = rng.int(0, (SECTION * 8) as i32 - 1) as usize;
            // Mostly a handful of ids, sometimes rare ones (including big mod ids).
            let id = if round % 50 == 0 { 0x8000 + rng.int(0, 999) as Id } else { rng.int(0, 12) as Id };
            assert_eq!(p.set(i, id), shadow[i]);
            shadow[i] = id;
        }
        assert!((0..SECTION * 8).all(|i| p.get(i) == shadow[i]));
        // Repacking from scratch gives the same blocks, and so does bulk decoding.
        let again = PalettedBlocks::from_ids(&shadow);
        assert!((0..SECTION * 8).all(|i| again.get(i) == shadow[i]));
        let mut flat = vec![0; SECTION * 8];
        p.decode_into(&mut flat);
        assert_eq!(flat, shadow);
        again.decode_into(&mut flat);
        assert_eq!(flat, shadow);
    }

    #[test]
    fn typical_terrain_is_small() {
        // Stone below, a few ores, dirt and grass on top, air above: ~4 bits per block.
        let mut ids = vec![AIR; SECTION * 8];
        for (i, id) in ids.iter_mut().enumerate() {
            let y = i / 256;
            *id = match y {
                0..=40 => if i % 97 == 0 { 13 } else { 3 },
                41..=43 => 2,
                44 => 1,
                _ => AIR,
            };
        }
        let p = PalettedBlocks::from_ids(&ids);
        let flat = ids.len() * std::mem::size_of::<Id>();
        assert!(p.bytes() * 3 < flat, "{} bytes vs {flat} flat", p.bytes());
    }
}
