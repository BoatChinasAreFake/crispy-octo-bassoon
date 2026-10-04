//! Region files: everything placed in the world kept on disk, 32×32 chunks
//! to a file, and only read into memory while someone is nearby.
//!
//! A world slot's regions live in `saves/<id>/world.regions/r.<rx>.<rz>.mnrg`
//! (a server's beside its `--world` file). Each holds the block edits for
//! its chunks, plus the soil of tilled blocks, what's in chests, furnaces
//! and the like, the words on signs and the things in item frames.
//!
//! - As chunks are asked for, their region's file is read on a background
//!   thread, so walking into new land doesn't stall a frame; chunks that
//!   arrive before their region wait for it. (Anything that needs a region
//!   right now, like a script editing far away, reads it on the spot.)
//! - Edits made anywhere mark a region as changed.
//! - When nobody is near a region any more it's written out and forgotten.
//! - Saving writes every region in memory that has anything to write, then
//!   the rest of the world (`world.mncr` keeps the players, mobs, items on
//!   the ground and so on).
//!
//! Worlds from before region files keep all this in `world.mncr`; the first
//! save moves it out into regions.
//!
//! Each file carries the mod palette it was written with, so a region left
//! alone while mods came and went still loads with the right blocks and items.
//! Hosts send a region's edits, signs and frames to joined players when it's
//! read in.

use crate::block::{reg, Id, AIR};
use crate::containers::Container;
use crate::farming::Soil;
use crate::inventory::Wear;
use crate::save::{R, W};
use crate::world::World;
use macroquad::math::IVec3;
use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};

/// Chunks along each side of a region.
pub const REGION: i32 = 32;
const MAGIC: &[u8; 4] = b"MNRG";
/// v2 adds soil, containers, signs and frames.
const VERSION: u32 = 2;

/// Where a world file's regions go.
pub fn region_dir(world_file: &Path) -> PathBuf {
    world_file.with_extension("regions")
}

pub fn region_of(cx: i32, cz: i32) -> (i32, i32) {
    (cx.div_euclid(REGION), cz.div_euclid(REGION))
}

/// The region a block is in.
pub fn region_at(p: IVec3) -> (i32, i32) {
    region_of(p.x.div_euclid(crate::world::CW), p.z.div_euclid(crate::world::CW))
}

fn file_of(dir: &Path, (rx, rz): (i32, i32)) -> PathBuf {
    dir.join(format!("r.{rx}.{rz}.mnrg"))
}

/// One chunk's edits, borrowed for writing.
pub type ChunkEdits<'a> = ((i32, i32), &'a HashMap<u32, Id>);
type ReadDone = ((i32, i32), u64, io::Result<Vec<u8>>);
type ReadAsk = ((i32, i32), u64, PathBuf);
/// One chunk's edits as read: (block index, id) pairs.
pub type ChunkRead = ((i32, i32), Vec<(u32, Id)>);

/// Which regions are in memory, which have changed since they were written,
/// and which are being read.
pub struct Regions {
    pub dir: PathBuf,
    loaded: HashSet<(i32, i32)>,
    dirty: HashSet<(i32, i32)>,
    /// Regions being read in the background, and the ticket of the read that counts.
    reading: HashMap<(i32, i32), u64>,
    ticket: u64,
    io_tx: Sender<ReadAsk>,
    io_rx: Receiver<ReadDone>,
    /// Chunks whose region was just read from disk (for a host to pass on).
    pub news: Vec<(i32, i32)>,
}

/// What a region file holds, as written (ids not yet remapped).
#[derive(Default, Debug)]
pub struct RegionData {
    pub chunks: Vec<ChunkRead>,
    pub palette: Vec<(Id, String)>,
    /// Packed by `farming::encode`, `containers::encode` and `decor::encode`.
    pub farm: Vec<u8>,
    pub containers: Vec<u8>,
    pub decor: Vec<u8>,
}

/// Pack one region.
pub fn encode(chunks: &[ChunkEdits], palette: &[(Id, String)], farm: &[u8], containers: &[u8], decor: &[u8]) -> Vec<u8> {
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
    for blob in [farm, containers, decor] {
        w.u32(blob.len() as u32);
        w.0.extend_from_slice(blob);
    }
    w.0
}

/// Unpack a region file.
pub fn decode(bytes: &[u8]) -> io::Result<RegionData> {
    let bad = |what: &str| io::Error::new(io::ErrorKind::InvalidData, format!("region file damaged ({what})"));
    let mut r = R(bytes);
    if &r.take::<4>()? != MAGIC {
        return Err(bad("not a region"));
    }
    let version = r.u32()?;
    if !(1..=VERSION).contains(&version) {
        return Err(bad("unknown version"));
    }
    let mut d = RegionData::default();
    for _ in 0..r.u32()? {
        let id = r.u16()?;
        let key = r.bytes(256)?;
        d.palette.push((id, String::from_utf8_lossy(&key).into_owned()));
    }
    let n = r.u32()? as usize;
    if n > (REGION * REGION) as usize {
        return Err(bad("too many chunks"));
    }
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
        d.chunks.push((key, entries));
    }
    if version >= 2 {
        d.farm = r.bytes(64 << 20)?;
        d.containers = r.bytes(64 << 20)?;
        d.decor = r.bytes(16 << 20)?;
    }
    Ok(d)
}

/// The reading thread: files in, bytes out.
fn spawn_reader() -> (Sender<ReadAsk>, Receiver<ReadDone>) {
    let (req_tx, req_rx) = channel::<ReadAsk>();
    let (res_tx, res_rx) = channel();
    let _ = std::thread::Builder::new().name("regionio".into()).spawn(move || {
        while let Ok((key, ticket, path)) = req_rx.recv() {
            if res_tx.send((key, ticket, std::fs::read(path))).is_err() {
                return;
            }
        }
    });
    (req_tx, res_rx)
}

impl World {
    /// Keep this world's edits (and chests, signs...) in region files under
    /// `dir`. Anything already in memory (a fresh world, or one saved before
    /// regions) counts as changed.
    pub fn use_regions(&mut self, dir: PathBuf) {
        let held: HashSet<(i32, i32)> = self
            .mods
            .keys()
            .map(|&(cx, cz)| region_of(cx, cz))
            .chain(self.farm.keys().chain(self.containers.keys()).chain(self.signs.keys()).chain(self.frames.keys()).map(|&p| region_at(p)))
            .collect();
        let (io_tx, io_rx) = spawn_reader();
        self.regions = Some(Regions { dir, loaded: held.clone(), dirty: held, reading: HashMap::new(), ticket: 0, io_tx, io_rx, news: Vec::new() });
    }

    /// Start reading a region in the background, if it isn't in memory or on its way.
    pub(crate) fn prefetch_region(&mut self, cx: i32, cz: i32) {
        let key = region_of(cx, cz);
        let Some(r) = &mut self.regions else { return };
        if r.loaded.contains(&key) || r.reading.contains_key(&key) {
            return;
        }
        r.ticket += 1;
        if r.io_tx.send((key, r.ticket, file_of(&r.dir, key))).is_ok() {
            r.reading.insert(key, r.ticket);
        }
    }

    /// Is this chunk's region still being read (so the chunk should wait)?
    pub(crate) fn region_pending(&self, cx: i32, cz: i32) -> bool {
        self.regions.as_ref().is_some_and(|r| r.reading.contains_key(&region_of(cx, cz)))
    }

    /// Take in regions the background thread has finished reading.
    pub(crate) fn absorb_region_reads(&mut self) {
        loop {
            let Some(r) = &mut self.regions else { return };
            let Ok((key, ticket, result)) = r.io_rx.try_recv() else { return };
            // A read overtaken by an on-the-spot one (or a later request) doesn't count.
            if r.reading.get(&key) != Some(&ticket) {
                continue;
            }
            r.reading.remove(&key);
            if !r.loaded.insert(key) {
                continue;
            }
            self.take_region(key, result);
        }
    }

    /// Read a region in now, if it isn't already. Things changed while it
    /// was away (by scripts or other players) win over what's on disk.
    pub(crate) fn ensure_region(&mut self, cx: i32, cz: i32) {
        let key = region_of(cx, cz);
        let Some(regions) = &mut self.regions else { return };
        if !regions.loaded.insert(key) {
            return;
        }
        // Whatever the background thread brings back for it is out of date now.
        regions.reading.remove(&key);
        let path = file_of(&regions.dir, key);
        self.take_region(key, std::fs::read(path));
    }

    /// Merge a region file's contents into memory.
    fn take_region(&mut self, key: (i32, i32), result: io::Result<Vec<u8>>) {
        let Some(regions) = &self.regions else { return };
        let path = file_of(&regions.dir, key);
        let decoded = match result {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return,
            Err(e) => Err(e),
            Ok(bytes) => decode(&bytes),
        };
        let d = match decoded {
            Ok(d) => d,
            Err(e) => {
                // Keep the file aside: writing over it later would lose what's in it.
                eprintln!("{}: {e}; kept as .bad", path.display());
                let _ = std::fs::rename(&path, path.with_extension("bad"));
                return;
            }
        };
        let remap = crate::game::palette_remap(reg(), &d.palette);
        let map = |id: Id| remap.as_ref().map(|r| r[id as usize]).unwrap_or(id);
        let ours = |p: &IVec3| region_at(*p) == key;
        let mut news = Vec::new();
        for (chunk, entries) in d.chunks {
            if region_of(chunk.0, chunk.1) != key {
                continue;
            }
            let m = self.mods.entry(chunk).or_default();
            for (i, id) in entries {
                m.entry(i).or_insert(map(id));
            }
            news.push(chunk);
        }
        for (p, soil) in crate::farming::decode(&d.farm) {
            if ours(&p) {
                self.farm.entry(p).or_insert(soil);
            }
        }
        for (p, mut c) in crate::containers::decode(&d.containers, 4) {
            if !ours(&p) {
                continue;
            }
            for s in c.slots.iter_mut() {
                *s = s.map(|(id, n)| (map(id), n)).filter(|(id, _)| *id != AIR);
            }
            self.containers.entry(p).or_insert(c);
        }
        let (signs, frames) = crate::decor::decode(&d.decor);
        for (p, lines) in signs {
            if ours(&p) {
                news.push((p.x.div_euclid(crate::world::CW), p.z.div_euclid(crate::world::CW)));
                self.signs.entry(p).or_insert(lines);
            }
        }
        for (p, (item, wear)) in frames {
            let item = map(item);
            if ours(&p) && item != AIR {
                news.push((p.x.div_euclid(crate::world::CW), p.z.div_euclid(crate::world::CW)));
                self.frames.entry(p).or_insert((item, wear));
            }
        }
        if let (true, Some(r)) = (self.log_edits, &mut self.regions) {
            news.sort_unstable();
            news.dedup();
            r.news.extend(news);
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

    /// Does a region have anything besides block edits in memory? (Chests
    /// cook and signs change without going through `record_edit`, so these
    /// are always written.)
    fn holds_things(&self, key: (i32, i32)) -> bool {
        let ours = |p: &IVec3| region_at(*p) == key;
        self.farm.keys().any(ours) || self.containers.keys().any(ours) || self.signs.keys().any(ours) || self.frames.keys().any(ours)
    }

    fn write_region(&self, key: (i32, i32)) -> io::Result<()> {
        let Some(regions) = &self.regions else { return Ok(()) };
        let mut chunks: Vec<ChunkEdits> = self.mods.iter().filter(|(k, m)| region_of(k.0, k.1) == key && !m.is_empty()).map(|(k, m)| (*k, m)).collect();
        chunks.sort_unstable_by_key(|c| c.0);
        let ours = |p: &IVec3| region_at(*p) == key;
        let farm: HashMap<IVec3, Soil> = self.farm.iter().filter(|(p, _)| ours(p)).map(|(p, s)| (*p, s.clone())).collect();
        let containers: HashMap<IVec3, Container> = self.containers.iter().filter(|(p, _)| ours(p)).map(|(p, c)| (*p, c.clone())).collect();
        let signs: HashMap<IVec3, [String; crate::decor::LINES]> = self.signs.iter().filter(|(p, _)| ours(p)).map(|(p, l)| (*p, l.clone())).collect();
        let frames: HashMap<IVec3, (Id, Wear)> = self.frames.iter().filter(|(p, _)| ours(p)).map(|(p, f)| (*p, *f)).collect();
        let path = file_of(&regions.dir, key);
        if chunks.is_empty() && farm.is_empty() && containers.is_empty() && signs.is_empty() && frames.is_empty() {
            return match std::fs::remove_file(&path) {
                Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            };
        }
        let bytes = encode(&chunks, &crate::game::mod_palette(reg()), &crate::farming::encode(&farm), &crate::containers::encode(&containers), &crate::decor::encode(&signs, &frames));
        std::fs::create_dir_all(&regions.dir)?;
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, bytes)?;
        std::fs::rename(tmp, path)
    }

    /// Write every region in memory that has anything to write (before saving the rest of the world).
    pub fn flush_regions(&mut self) -> io::Result<()> {
        let Some(regions) = &self.regions else { return Ok(()) };
        let mut keys: Vec<(i32, i32)> = regions.dirty.iter().chain(regions.loaded.iter()).copied().collect::<HashSet<_>>().into_iter().collect();
        keys.sort_unstable();
        let mut result = Ok(());
        for key in keys {
            let dirty = self.regions.as_ref().is_some_and(|r| r.dirty.contains(&key));
            if !dirty && !self.holds_things(key) {
                continue;
            }
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
            if (dirty || self.holds_things(key)) && self.write_region(key).is_err() {
                continue; // try again later (and saving will say what's wrong)
            }
            let keep = |p: &IVec3| region_at(*p) != key;
            self.mods.retain(|k, _| region_of(k.0, k.1) != key);
            self.farm.retain(|p, _| keep(p));
            self.containers.retain(|p, _| keep(p));
            self.signs.retain(|p, _| keep(p));
            self.frames.retain(|p, _| keep(p));
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
    use macroquad::math::{ivec3, Vec3};

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
        let bytes = encode(&[((-33, 5), &m)], &[(600, "x:thing".into())], &[1, 2], &[3], &[]);
        let d = decode(&bytes).unwrap();
        assert_eq!(d.chunks, vec![((-33, 5), vec![(3, STONE), (7, TORCH)])]);
        assert_eq!(d.palette, vec![(600, "x:thing".to_string())]);
        assert_eq!((d.farm, d.containers, d.decor), (vec![1, 2], vec![3], vec![]));
        assert!(decode(&bytes[..bytes.len() - 1]).is_err());
        // A version 1 file (edits only) still reads.
        let mut v1 = b"MNRG\x01\0\0\0\0\0\0\0\x01\0\0\0".to_vec();
        v1.extend_from_slice(&[0; 8]);
        v1.extend_from_slice(&1u32.to_le_bytes());
        v1.extend_from_slice(&9u32.to_le_bytes());
        v1.extend_from_slice(&TORCH.to_le_bytes());
        assert_eq!(decode(&v1).unwrap().chunks, vec![((0, 0), vec![(9, TORCH)])]);
        assert_eq!(region_of(-1, 31), (-1, 0));
        assert_eq!(region_of(32, -33), (1, -2));
    }

    #[test]
    fn chests_signs_and_soil_go_with_their_region() {
        let dir = temp_dir("things");
        let mut w = World::new(13);
        w.use_regions(dir.clone());
        w.insert_chunk(0, 0, stone_chunk());
        w.set(2, 1, 2, CHEST);
        w.containers.get_mut(&ivec3(2, 1, 2)).unwrap().slots[0] = Some((DIAMOND, 3));
        w.signs.insert(ivec3(4, 1, 4), ["Hi".into(), String::new(), String::new(), String::new()]);
        w.frames.insert(ivec3(5, 1, 5), (TORCH, 0));
        w.farm.insert(ivec3(6, 0, 6), Default::default());
        w.stream(&[(Vec3::new(9000.0, 64.0, 9000.0), 0)]);
        assert!(w.containers.is_empty() && w.signs.is_empty() && w.frames.is_empty() && w.farm.is_empty());

        let mut w = World::new(13);
        w.use_regions(dir.clone());
        w.insert_chunk(0, 0, stone_chunk());
        assert_eq!(w.containers[&ivec3(2, 1, 2)].slots[0], Some((DIAMOND, 3)));
        assert_eq!(w.signs[&ivec3(4, 1, 4)][0], "Hi");
        assert_eq!(w.frames[&ivec3(5, 1, 5)], (TORCH, 0));
        assert!(w.farm.contains_key(&ivec3(6, 0, 6)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn regions_are_read_in_the_background() {
        let dir = temp_dir("background");
        let mut w = World::new(14);
        w.use_regions(dir.clone());
        w.insert_chunk(0, 0, stone_chunk());
        w.set(3, 70, 3, GLASS);
        w.flush_regions().unwrap();

        // Streamed the ordinary way: asked for, read on the side, then generated.
        let mut w = World::new(14);
        w.use_regions(dir.clone());
        let at = [(Vec3::new(3.0, 64.0, 3.0), 1)];
        for _ in 0..2000 {
            w.stream(&at);
            if w.chunks.contains_key(&(0, 0)) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(w.get(3, 70, 3), GLASS);
        let _ = std::fs::remove_dir_all(&dir);
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
