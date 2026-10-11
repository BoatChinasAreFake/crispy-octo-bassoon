//! **Squash**: a small, fast LZ77 compressor (in the spirit of LZ4), written
//! out here so the game still needs nothing but the standard library. Region
//! files are squashed on their way to disk (see regions.rs): they're mostly
//! runs of similar edits, and come out several times smaller.
//!
//! The format is a series of sequences, each:
//!
//! - a token byte: the high four bits are the number of literal bytes, the
//!   low four the length of the match after them, minus 4 (15 in either
//!   means "and more": add the bytes that follow, each 0-255, stopping
//!   after the first that isn't 255);
//! - the literal bytes;
//! - the match's distance back, two bytes little-endian (1 or more).
//!
//! The last sequence has literals only, and ends the data.

/// Shortest match worth writing.
const MIN_MATCH: usize = 4;
const HASH_BITS: u32 = 15;
const MAX_DIST: usize = 65535;

fn hash(b: &[u8]) -> usize {
    let v = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
    (v.wrapping_mul(2_654_435_761) >> (32 - HASH_BITS)) as usize
}

fn put_len(out: &mut Vec<u8>, mut n: usize) {
    while n >= 255 {
        out.push(255);
        n -= 255;
    }
    out.push(n as u8);
}

fn sequence(out: &mut Vec<u8>, lits: &[u8], m: Option<(usize, usize)>) {
    let ml = m.map_or(0, |(len, _)| len - MIN_MATCH);
    out.push(((lits.len().min(15) as u8) << 4) | ml.min(15) as u8);
    if lits.len() >= 15 {
        put_len(out, lits.len() - 15);
    }
    out.extend_from_slice(lits);
    if let Some((_, dist)) = m {
        out.extend_from_slice(&(dist as u16).to_le_bytes());
        if ml >= 15 {
            put_len(out, ml - 15);
        }
    }
}

/// Squash some bytes.
pub fn compress(src: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(src.len() / 2 + 16);
    let mut table = vec![usize::MAX; 1 << HASH_BITS];
    let (mut i, mut anchor) = (0, 0);
    while i + MIN_MATCH <= src.len() {
        let h = hash(&src[i..]);
        let cand = table[h];
        table[h] = i;
        if cand != usize::MAX && i - cand <= MAX_DIST && src[cand..cand + MIN_MATCH] == src[i..i + MIN_MATCH] {
            let mut len = MIN_MATCH;
            while i + len < src.len() && src[cand + len] == src[i + len] {
                len += 1;
            }
            sequence(&mut out, &src[anchor..i], Some((len, i - cand)));
            // Remember a few of the positions inside the match too.
            let end = i + len;
            let mut j = i + 1;
            while j + MIN_MATCH <= src.len() && j < end {
                table[hash(&src[j..])] = j;
                j += if len > 64 { 7 } else { 1 };
            }
            i = end;
            anchor = i;
        } else {
            i += 1;
        }
    }
    sequence(&mut out, &src[anchor..], None);
    out
}

/// Unsquash, refusing anything that would come out bigger than `max` bytes
/// (or that's damaged).
pub fn decompress(src: &[u8], max: usize) -> Option<Vec<u8>> {
    let mut out: Vec<u8> = Vec::with_capacity(src.len().saturating_mul(3).min(max));
    let mut i = 0;
    let len_more = |i: &mut usize, mut n: usize| -> Option<usize> {
        loop {
            let b = *src.get(*i)?;
            *i += 1;
            n = n.checked_add(b as usize)?;
            if b != 255 {
                return Some(n);
            }
        }
    };
    loop {
        let token = *src.get(i)?;
        i += 1;
        let mut lits = (token >> 4) as usize;
        if lits == 15 {
            lits = len_more(&mut i, lits)?;
        }
        let end = i.checked_add(lits)?;
        out.extend_from_slice(src.get(i..end)?);
        i = end;
        if out.len() > max {
            return None;
        }
        if i == src.len() {
            return Some(out);
        }
        let dist = u16::from_le_bytes([*src.get(i)?, *src.get(i + 1)?]) as usize;
        i += 2;
        let mut len = (token & 15) as usize;
        if len == 15 {
            len = len_more(&mut i, len)?;
        }
        let len = len + MIN_MATCH;
        if dist == 0 || dist > out.len() || out.len() + len > max {
            return None;
        }
        // Byte by byte: a match may overlap what it's copying (a run).
        let from = out.len() - dist;
        for k in 0..len {
            let b = out[from + k];
            out.push(b);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round(data: &[u8]) -> Vec<u8> {
        let c = compress(data);
        assert_eq!(decompress(&c, data.len()).as_deref(), Some(data));
        c
    }

    #[test]
    fn squash_round_trips_and_shrinks_repetitive_data() {
        round(b"");
        round(b"a");
        round(b"abcd");
        round(b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        // Region-like data: the gaps between edits and a few block ids.
        let mut edits = Vec::new();
        for i in 0..5000u32 {
            edits.push(if i % 16 == 15 { 17 } else { 1 });
            edits.extend_from_slice(&((i / 40 % 3) as u16 + 1).to_le_bytes());
        }
        let c = round(&edits);
        assert!(c.len() * 3 < edits.len(), "{} -> {}", edits.len(), c.len());
        // Noise doesn't shrink, but still round-trips (and grows only a little).
        let mut x = 12345u32;
        let noise: Vec<u8> = (0..20000)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                x as u8
            })
            .collect();
        let c = round(&noise);
        assert!(c.len() < noise.len() + noise.len() / 100 + 16);
        // Long literal runs and long matches use the extra length bytes.
        let mut mixed = noise[..600].to_vec();
        mixed.extend(std::iter::repeat_n(7u8, 3000));
        mixed.extend_from_slice(&noise[..600]);
        round(&mixed);
    }

    #[test]
    fn damaged_or_oversized_data_is_refused() {
        let data: Vec<u8> = (0..4000).map(|i| (i % 7) as u8).collect();
        let c = compress(&data);
        assert!(decompress(&c, 100).is_none(), "over the limit");
        assert!(decompress(&c[..c.len() / 2], data.len()).is_none(), "cut short");
        assert!(decompress(&[0x0f, 0, 0], 100).is_none(), "match before any data");
        // Random damage never panics.
        for k in 0..c.len() {
            let mut bad = c.clone();
            bad[k] ^= 0x5a;
            let _ = decompress(&bad, data.len() * 2);
        }
    }
}
