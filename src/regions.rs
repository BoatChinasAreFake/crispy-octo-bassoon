//! Region files: block edits kept on disk, 32×32 chunks to a file, and only
//! read into memory while someone is nearby.
//!
//! A world slot's edits live in `saves/<id>/world.regions/r.<rx>.<rz>.mnrg`
//! (a server's beside its `--world` file). When a chunk arrives, its
//! region's file is read; edits made anywhere mark the region as changed;
//! when nobody is near a region any more it's written out and forgotten.
//! Saving writes every changed region, then the rest of the world
//! (`world.mncr` keeps the player, containers, mobs and so on).
//!
//! Worlds from before region files keep their edits in `world.mncr`; the
//! first save moves them out into regions.
//!
//! Each file carries the mod palette it was written with, so a region left
//! alone while mods came and went still loads with the right blocks.
//! Hosts send a region's edits to joined players when it's read in.

use crate::block::{reg, Id};
use crate::save::{R, W};
use crate::world::World;
use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};

/// Chunks along each side of a region.
pub const REGION: i32 = 32;
const MAGIC: &[u8; 4] = b"MNRG";
const VERSION: u32 = 1;

/// Where a world file's regions go.
pub fn region_dir(world_file: &Path) -> PathBuf {
    world_file.with_extension("regions")
}

pub fn region_of(cx: i32, cz: i32) -> (i32, i32) {
    (cx.div_euclid(REGION), cz.div_euclid(REGION))
}

fn file_of(dir: &Path, (rx, rz): (i32, i32)) -> PathBuf {
    dir.join(format!("r.{rx}.{rz}.mnrg"))
}

/// One chunk's edits, borrowed for writing.
pub type ChunkEdits<'a> = ((i32, i32), &'a HashMap<u32, Id>);

/// Which regions are in memory and which have changed since they were written.
pub struct Regions {
    pub dir: PathBuf,
    loaded: HashSet<(i32, i32)>,
    dirty: HashSet<(i32, i32)>,
    /// Chunks whose edits were just read from disk (for a host to pass on).
    pub news: Vec<(i32, i32)>,
}

/// Pack one region's edits.
pub fn encode(chunks: &[ChunkEdits], palette: &[(Id, String)]) -> Vec<u8> {
    let mut w = W(Vec::new());
    w.0.extend_from_slice(MAGIC);
    w.u32(VERSION);
    w.u32(palette.len() as u32);
    for (id, key) in palette {
        w.u16(*id);
        w.u32(key.len() as u32);
        w.0.extend_from_slice(key.as_bytes());
    }
    w.u32(chunks.len() as u32);
    for &((cx, cz), m) in chunks {
        w.i32(cx);
        w.i32(cz);
        w.u32(m.len() as u32);
        let mut entries: Vec<(u32, Id)> = m.iter().map(|(&i, &id)| (i, id)).collect();
        entries.sort_unstable();
        for (i, id) in entries {
            w.u32(i);
            w.u16(id);
        }
    }
    w.0
}

/// Unpack a region's edits (ids as written) and the palette they use.
#[allow(clippy::type_complexity)]
pub fn decode(bytes: &[u8]) -> io::Result<(Vec<((i32, i32), Vec<(u32, Id)>)>, Vec<(Id, String)>)> {
    let bad = |what: &str| io::Error::new(io::ErrorKind::InvalidData, format!("region file damaged ({what})"));
    let mut r = R(bytes);
    if &r.take::<4>()? != MAGIC {
        return Err(bad("not a region"));
    }
    if r.u32()? != VERSION {
        return Err(bad("unknown version"));
    }
    let mut palette = Vec::new();
    for _ in 0..r.u32()? {
        let id = r.u16()?;
        let key = r.bytes(256)?;
        palette.push((id, String::from_utf8_lossy(&key).into_owned()));
    }
    let n = r.u32()? as usize;
    if n > (REGION * REGION) as usize {
        return Err(bad("too many chunks"));
    }
    let mut chunks = Vec::with_capacity(n);
    for _ in 0..n {
        let key = (r.i32()?, r.i32()?);
        let count = r.u32()? as usize;
        if count > (crate::world::CW * crate::world::CW * crate::world::CH) as usize {
            return Err(bad("too many edits"));
        }
        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            entries.push((r.u32()?, r.u16()?));
        }
        chunks.push((key, entries));
    }
    Ok((chunks, palette))
}

impl World {
    /// Keep this world's edits in region files under `dir`. Edits already in
    /// memory (a fresh world, or one saved before regions) count as changed.
    pub fn use_regions(&mut self, dir: PathBuf) {
        let held: HashSet<(i32, i32)> = self.mods.keys().map(|&(cx, cz)| region_of(cx, cz)).collect();
        self.regions = Some(Regions { dir, loaded: held.clone(), dirty: held, news: Vec::new() });
    }

    /// Read a region's edits in, if they aren't already. Edits made while it
    /// was away (by scripts or other players) win over what's on disk.
    pub(crate) fn ensure_region(&mut self, cx: i32, cz: i32) {
        let key = region_of(cx, cz);
        let Some(regions) = &mut self.regions else { return };
        if !regions.loaded.insert(key) {
            return;
        }
        let path = file_of(&regions.dir, key);
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return,
            Err(e) => {
                // Keep it aside too: writing over it later would lose what's in it.
                eprintln!("Couldn't read {}: {e}; kept as .bad", path.display());
                let _ = std::fs::rename(&path, path.with_extension("bad"));
                return;
            }
        };
        let (chunks, palette) = match decode(&bytes) {
            Ok(x) => x,
            Err(e) => {
                // Keep the damaged file aside rather than writing over it later.
                eprintln!("{}: {e}; kept as .bad", path.display());
                let _ = std::fs::rename(&path, path.with_extension("bad"));
                return;
            }
        };
        let remap = crate::game::palette_remap(reg(), &palette);
        let news = self.log_edits;
        for (chunk, entries) in chunks {
            if region_of(chunk.0, chunk.1) != key {
                continue;
            }
            let m = self.mods.entry(chunk).or_default();
            for (i, id) in entries {
                let id = remap.as_ref().map(|r| r[id as usize]).unwrap_or(id);
                m.entry(i).or_insert(id);
            }
            if let (true, Some(r)) = (news, &mut self.regions) {
                r.news.push(chunk);
            }
        }
    }

    /// Remember an edit to a chunk (loaded or not).
    pub(crate) fn record_edit(&mut self, cx: i32, cz: i32, i: u32, id: Id) {
        self.ensure_region(cx, cz);
        self.mods.entry((cx, cz)).or_default().insert(i, id);
        if let Some(r) = &mut self.regions {
            r.dirty.insert(region_of(cx, cz));
        }
    }

    fn write_region(&self, key: (i32, i32)) -> io::Result<()> {
        let Some(regions) = &self.regions else { return Ok(()) };
        let mut chunks: Vec<ChunkEdits> = self.mods.iter().filter(|(k, m)| region_of(k.0, k.1) == key && !m.is_empty()).map(|(k, m)| (*k, m)).collect();
        chunks.sort_unstable_by_key(|c| c.0);
        let path = file_of(&regions.dir, key);
        if chunks.is_empty() {
            return match std::fs::remove_file(&path) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            };
        }
        std::fs::create_dir_all(&regions.dir)?;
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, encode(&chunks, &crate::game::mod_palette(reg())))?;
        std::fs::rename(tmp, path)
    }

    /// Write every changed region (before saving the rest of the world).
    pub fn flush_regions(&mut self) -> io::Result<()> {
        let Some(regions) = &self.regions else { return Ok(()) };
        let mut dirty: Vec<(i32, i32)> = regions.dirty.iter().copied().collect();
        dirty.sort_unstable();
        let mut result = Ok(());
        for key in dirty {
            match self.write_region(key) {
                Ok(()) => {
                    if let Some(r) = &mut self.regions {
                        r.dirty.remove(&key);
                    }
                }
                Err(e) => result = result.and(Err(e)),
            }
        }
        result
    }

    /// Write out and forget regions with no chunks loaded (or on their way).
    pub(crate) fn drop_idle_regions(&mut self) {
        let Some(regions) = &self.regions else { return };
        let busy: HashSet<(i32, i32)> = self.chunks.keys().chain(self.pending_chunks()).map(|&(cx, cz)| region_of(cx, cz)).collect();
        let idle: Vec<(i32, i32)> = regions.loaded.iter().filter(|k| !busy.contains(k)).copied().collect();
        for key in idle {
            let dirty = self.regions.as_ref().is_some_and(|r| r.dirty.contains(&key));
            if dirty && self.write_region(key).is_err() {
                continue; // try again later (and saving will say what's wrong)
            }
            self.mods.retain(|k, _| region_of(k.0, k.1) != key);
            if let Some(r) = &mut self.regions {
                r.dirty.remove(&key);
                r.loaded.remove(&key);
            }
        }
    }

    /// How many regions are in memory (for the debug screen).
    pub fn regions_loaded(&self) -> usize {
        self.regions.as_ref().map_or(0, |r| r.loaded.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::*;
    use crate::palette::PalettedBlocks;
    use crate::world::{idx, CH, CW};
    use macroquad::math::Vec3;

    fn stone_chunk() -> PalettedBlocks {
        let mut ids = vec![AIR; (CW * CW * CH) as usize];
        for z in 0..CW {
            for x in 0..CW {
                ids[idx(x, 0, z)] = STONE;
            }
        }
        PalettedBlocks::from_ids(&ids)
    }

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("minceraft-regions-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn pack_and_unpack() {
        let mut m = HashMap::new();
        m.insert(7, TORCH);
        m.insert(3, STONE);
        let bytes = encode(&[((-33, 5), &m)], &[(600, "x:thing".into())]);
        let (chunks, palette) = decode(&bytes).unwrap();
        assert_eq!(chunks, vec![((-33, 5), vec![(3, STONE), (7, TORCH)])]);
        assert_eq!(palette, vec![(600, "x:thing".to_string())]);
        assert!(decode(&bytes[..bytes.len() - 1]).is_err());
        assert_eq!(region_of(-1, 31), (-1, 0));
        assert_eq!(region_of(32, -33), (1, -2));
    }

    #[test]
    fn edits_go_to_disk_and_come_back() {
        let dir = temp_dir("roundtrip");
        let mut w = World::new(9);
        w.use_regions(dir.clone());
        w.insert_chunk(0, 0, stone_chunk());
        w.set(3, 1, 4, TORCH);
        // Far away, in a chunk that isn't loaded (a script, say).
        w.set_or_record(40 * CW, 5, 0, GLASS);
        w.flush_regions().unwrap();
        assert!(dir.join("r.0.0.mnrg").exists() && dir.join("r.1.0.mnrg").exists());

        // Everyone walks off: the regions are forgotten.
        w.chunks.clear();
        w.drop_idle_regions();
        assert!(w.mods.is_empty());
        assert_eq!(w.regions_loaded(), 0);

        // A new session: the chunk arrives and its edits with it.
        let mut again = World::new(9);
        again.use_regions(dir.clone());
        again.insert_chunk(0, 0, stone_chunk());
        assert_eq!(again.get(3, 1, 4), TORCH);
        assert_eq!(again.regions_loaded(), 1);
        again.insert_chunk(40, 0, stone_chunk());
        assert_eq!(again.get(40 * CW, 5, 0), GLASS);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn newer_edits_beat_the_disk_and_idle_regions_are_saved() {
        let dir = temp_dir("merge");
        let mut w = World::new(10);
        w.use_regions(dir.clone());
        w.insert_chunk(0, 0, stone_chunk());
        w.set(1, 1, 1, TORCH);
        w.set(2, 1, 1, GLASS);
        // Walking away writes the region without an explicit save.
        w.stream(&[(Vec3::new(5000.0, 64.0, 5000.0), 0)]);
        assert!(dir.join("r.0.0.mnrg").exists());
        assert!(!w.mods.contains_key(&(0, 0)));

        let mut w = World::new(10);
        w.use_regions(dir.clone());
        // An edit arrives for the chunk before it's loaded here.
        w.set_or_record(1, 1, 1, PLANKS);
        w.insert_chunk(0, 0, stone_chunk());
        assert_eq!(w.get(1, 1, 1), PLANKS);
        assert_eq!(w.get(2, 1, 1), GLASS);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn old_saves_move_their_edits_out() {
        let dir = temp_dir("migrate");
        let mut w = World::new(12);
        // As `Game::from_save` leaves a save from before region files.
        w.mods.entry((-1, 70)).or_default().insert(5, TORCH);
        w.use_regions(dir.clone());
        w.flush_regions().unwrap();
        assert!(dir.join("r.-1.2.mnrg").exists());
        let mut again = World::new(12);
        again.use_regions(dir.clone());
        again.ensure_region(-1, 70);
        assert_eq!(again.mods[&(-1, 70)][&5], TORCH);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_damaged_file_is_kept_aside() {
        let dir = temp_dir("damaged");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("r.0.0.mnrg"), b"MNRG\x01\0\0\0garbage").unwrap();
        let mut w = World::new(11);
        w.use_regions(dir.clone());
        w.insert_chunk(0, 0, stone_chunk());
        assert!(dir.join("r.0.0.bad").exists());
        assert_eq!(w.get(0, 0, 0), STONE);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
