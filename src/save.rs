//! Tiny hand-rolled binary save format. Stores the seed, player, inventory and
//! only the blocks the player changed; terrain is regenerated from the seed.

use crate::block::{Id, FIRST_ITEM};
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const MAGIC: &[u8; 4] = b"MNCR";
/// v2 adds the mod palette (names of mod blocks/items); v3 adds script mod
/// variables; v4 adds earned advancements; v5 widens block/item ids to two
/// bytes; v6 adds farm soil and the fishing log; v7 adds what's in chests and
/// furnaces; v8 adds items lying on the ground; v9 adds tool and armour wear,
/// hunger and the keep-inventory rule; v10 adds difficulty, the daylight cycle
/// rule and experience points; v11 widens tool wear to carry enchantments and
/// adds joined players' records and the weather; v12 adds animals worth
/// keeping (tamed, bred, fed). Older saves still load.
pub const VERSION: u32 = 12;

/// Before v5, ids were one byte: blocks below 100, items from 100 up.
pub(crate) fn legacy_id(v: u8) -> Id {
    if v < 100 { v as Id } else { FIRST_ITEM + (v - 100) as Id }
}

pub struct SaveData {
    pub seed: u32,
    pub creative: bool,
    pub time: f32,
    pub pos: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub health: f32,
    pub spawn: [f32; 3],
    pub slots: Vec<Option<(Id, u8)>>,
    pub mods: HashMap<(i32, i32), HashMap<u32, Id>>,
    /// Ids of mod-added blocks/items and their names (save v2+).
    pub palette: Vec<(Id, String)>,
    /// Script mod variables: (mod id, encoded variables) (save v3+).
    pub script_vars: Vec<(String, Vec<u8>)>,
    /// Keys of the advancements earned in this world (save v4+).
    pub advancements: Vec<String>,
    /// Soil records for tilled blocks, packed by `farming::encode` (save v6+).
    pub farm: Vec<u8>,
    /// The fishing log, packed by `FishLog::encode` (save v6+).
    pub fish_log: Vec<u8>,
    /// Chest and furnace contents, packed by `containers::encode` (save v7+).
    pub containers: Vec<u8>,
    /// Items on the ground, packed by `drops::encode` (save v8+).
    pub drops: Vec<u8>,
    /// Wear of whatever's in each of `slots` (save v9+; see `block::durability`).
    pub wear: Vec<u32>,
    /// Hunger and saturation (save v9+; older saves start full).
    pub food: f32,
    pub saturation: f32,
    /// World rule: keep your things when you die (save v9+).
    pub keep_inventory: bool,
    /// World rules: difficulty (`rules::Difficulty` index) and whether the sun moves (save v10+).
    pub difficulty: u8,
    pub daylight_cycle: bool,
    /// Experience points (save v10+).
    pub xp: u32,
    /// Joined players remembered by name, packed by `players::encode` (save v11+).
    pub players: Vec<u8>,
    /// Weather (`weather::Weather` index), seconds until it changes, and whether it changes at all (save v11+).
    pub weather: u8,
    pub weather_timer: f32,
    pub weather_cycle: bool,
    /// How many times the player has enchanted something (save v11+; seeds the table's offers).
    pub enchant_count: u32,
    /// Tamed, bred and fed animals, packed by `animals::encode_mobs` (save v12+).
    pub mobs: Vec<u8>,
    /// The format version it was read from (the container and drop blobs changed in v9).
    pub version: u32,
}


struct W(Vec<u8>);
impl W {
    fn u8(&mut self, v: u8) {
        self.0.push(v)
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes())
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes())
    }
    fn i32(&mut self, v: i32) {
        self.0.extend_from_slice(&v.to_le_bytes())
    }
    fn f32(&mut self, v: f32) {
        self.0.extend_from_slice(&v.to_le_bytes())
    }
}

struct R<'a>(&'a [u8]);
impl R<'_> {
    fn take<const N: usize>(&mut self) -> io::Result<[u8; N]> {
        if self.0.len() < N {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "save file truncated"));
        }
        let (a, b) = self.0.split_at(N);
        self.0 = b;
        Ok(a.try_into().unwrap())
    }
    fn u8(&mut self) -> io::Result<u8> {
        Ok(self.take::<1>()?[0])
    }
    fn u16(&mut self) -> io::Result<u16> {
        Ok(u16::from_le_bytes(self.take()?))
    }
    fn u32(&mut self) -> io::Result<u32> {
        Ok(u32::from_le_bytes(self.take()?))
    }
    fn i32(&mut self) -> io::Result<i32> {
        Ok(i32::from_le_bytes(self.take()?))
    }
    fn f32(&mut self) -> io::Result<f32> {
        Ok(f32::from_le_bytes(self.take()?))
    }
    /// A u32 length followed by that many bytes, at most `max`.
    fn bytes(&mut self, max: usize) -> io::Result<Vec<u8>> {
        let n = self.u32()? as usize;
        if n > max || n > self.0.len() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "save file damaged (bad length)"));
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Ok(a.to_vec())
    }
}

pub fn write_to(path: &std::path::Path, d: &SaveData) -> io::Result<()> {
    let mut w = W(Vec::new());
    w.0.extend_from_slice(MAGIC);
    w.u32(VERSION);
    w.u32(d.seed);
    w.u8(d.creative as u8);
    w.f32(d.time);
    for v in d.pos.iter().chain(d.spawn.iter()) {
        w.f32(*v);
    }
    w.f32(d.yaw);
    w.f32(d.pitch);
    w.f32(d.health);
    w.u32(d.slots.len() as u32);
    for s in &d.slots {
        let (id, n) = s.unwrap_or((0, 0));
        w.u16(id);
        w.u8(n);
    }
    w.u32(d.mods.len() as u32);
    for (&(cx, cz), m) in &d.mods {
        w.i32(cx);
        w.i32(cz);
        w.u32(m.len() as u32);
        for (&i, &id) in m {
            w.u32(i);
            w.u16(id);
        }
    }
    w.u32(d.palette.len() as u32);
    for (id, key) in &d.palette {
        w.u16(*id);
        w.u32(key.len() as u32);
        w.0.extend_from_slice(key.as_bytes());
    }
    w.u32(d.script_vars.len() as u32);
    for (id, blob) in &d.script_vars {
        w.u32(id.len() as u32);
        w.0.extend_from_slice(id.as_bytes());
        w.u32(blob.len() as u32);
        w.0.extend_from_slice(blob);
    }
    w.u32(d.advancements.len() as u32);
    for key in &d.advancements {
        w.u32(key.len() as u32);
        w.0.extend_from_slice(key.as_bytes());
    }
    for blob in [&d.farm, &d.fish_log, &d.containers, &d.drops] {
        w.u32(blob.len() as u32);
        w.0.extend_from_slice(blob);
    }
    w.u32(d.wear.len() as u32);
    for v in &d.wear {
        w.u32(*v);
    }
    w.f32(d.food);
    w.f32(d.saturation);
    w.u8(d.keep_inventory as u8);
    w.u8(d.difficulty);
    w.u8(d.daylight_cycle as u8);
    w.u32(d.xp);
    w.u32(d.players.len() as u32);
    w.0.extend_from_slice(&d.players);
    w.u8(d.weather);
    w.f32(d.weather_timer);
    w.u8(d.weather_cycle as u8);
    w.u32(d.enchant_count);
    w.u32(d.mobs.len() as u32);
    w.0.extend_from_slice(&d.mobs);
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::File::create(&tmp)?.write_all(&w.0)?;
    std::fs::rename(tmp, path)
}

pub fn read_from(path: &std::path::Path) -> io::Result<SaveData> {
    let mut buf = Vec::new();
    std::fs::File::open(path)?.read_to_end(&mut buf)?;
    let mut r = R(&buf);
    if &r.take::<4>()? != MAGIC {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "not a Minceraft save"));
    }
    let version = r.u32()?;
    if !(1..=VERSION).contains(&version) {
        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("save is from a newer version ({version})")));
    }
    // One-byte ids before v5.
    let wide = version >= 5;
    let seed = r.u32()?;
    let creative = r.u8()? != 0;
    let time = r.f32()?;
    let pos = [r.f32()?, r.f32()?, r.f32()?];
    let spawn = [r.f32()?, r.f32()?, r.f32()?];
    let (yaw, pitch, health) = (r.f32()?, r.f32()?, r.f32()?);
    let n = r.u32()? as usize;
    let mut slots = Vec::with_capacity(n);
    for _ in 0..n {
        let id = if wide { r.u16()? } else { legacy_id(r.u8()?) };
        let c = r.u8()?;
        slots.push(if c > 0 { Some((id, c)) } else { None });
    }
    let mut mods = HashMap::new();
    for _ in 0..r.u32()? {
        let key = (r.i32()?, r.i32()?);
        let mut m = HashMap::new();
        for _ in 0..r.u32()? {
            let i = r.u32()?;
            m.insert(i, if wide { r.u16()? } else { legacy_id(r.u8()?) });
        }
        mods.insert(key, m);
    }
    let mut palette = Vec::new();
    if version >= 2 {
        for _ in 0..r.u32()? {
            let id = if wide { r.u16()? } else { legacy_id(r.u8()?) };
            let len = r.u32()? as usize;
            if len > 256 {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "bad palette entry"));
            }
            let mut key = Vec::with_capacity(len);
            for _ in 0..len {
                key.push(r.u8()?);
            }
            palette.push((id, String::from_utf8_lossy(&key).into_owned()));
        }
    }
    let mut script_vars = Vec::new();
    if version >= 3 {
        for _ in 0..r.u32()? {
            let id = r.bytes(256)?;
            let blob = r.bytes(8 << 20)?;
            script_vars.push((String::from_utf8_lossy(&id).into_owned(), blob));
        }
    }
    let mut advancements = Vec::new();
    if version >= 4 {
        for _ in 0..r.u32()? {
            advancements.push(String::from_utf8_lossy(&r.bytes(256)?).into_owned());
        }
    }
    let (farm, fish_log) = if version >= 6 { (r.bytes(64 << 20)?, r.bytes(1 << 20)?) } else { (Vec::new(), Vec::new()) };
    let containers = if version >= 7 { r.bytes(64 << 20)? } else { Vec::new() };
    let drops = if version >= 8 { r.bytes(16 << 20)? } else { Vec::new() };
    let (mut wear, mut food, mut saturation, mut keep_inventory) = (Vec::new(), 20.0, 5.0, false);
    if version >= 9 {
        let n = r.u32()? as usize;
        if n > 4096 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "bad wear list"));
        }
        for _ in 0..n {
            wear.push(if version >= 11 { r.u32()? } else { r.u16()? as u32 });
        }
        food = r.f32()?;
        saturation = r.f32()?;
        keep_inventory = r.u8()? != 0;
    }
    let (mut difficulty, mut daylight_cycle, mut xp) = (2, true, 0);
    if version >= 10 {
        difficulty = r.u8()?;
        daylight_cycle = r.u8()? != 0;
        xp = r.u32()?.min(1 << 24);
    }
    let players = if version >= 11 { r.bytes(16 << 20)? } else { Vec::new() };
    let (mut weather, mut weather_timer, mut weather_cycle, mut enchant_count) = (0, 600.0, true, 0);
    if version >= 11 {
        weather = r.u8()?;
        weather_timer = r.f32()?;
        weather_cycle = r.u8()? != 0;
        enchant_count = r.u32()?;
        if !weather_timer.is_finite() {
            weather_timer = 600.0;
        }
    }
    let mobs = if version >= 12 { r.bytes(4 << 20)? } else { Vec::new() };
    let ok = |v: f32, d: f32| if v.is_finite() { v.clamp(0.0, 20.0) } else { d };
    let (food, saturation) = (ok(food, 20.0), ok(saturation, 5.0));
    Ok(SaveData { seed, creative, time, pos, yaw, pitch, health, spawn, slots, mods, palette, script_vars, advancements, farm, fish_log, containers, drops, wear, food, saturation, keep_inventory, difficulty, daylight_cycle, xp, players, weather, weather_timer, weather_cycle, enchant_count, mobs, version })
}

// ------------------------------------------------------------------ world slots
//
// Each world lives in its own folder: saves/<id>/world.mncr, plus
// saves/<id>/info.txt holding its display name. The folder id is derived from
// the name when the world is created and never changes (renaming only edits
// info.txt), so script data and anything else keyed by folder stays put.

pub const WORLD_FILE: &str = "world.mncr";
const INFO_FILE: &str = "info.txt";
/// Where the game kept its single save before world slots existed.
const LEGACY_FILE: &str = "world.mncr";

pub fn saves_dir() -> PathBuf {
    PathBuf::from("saves")
}

pub struct WorldEntry {
    pub id: String,
    pub name: String,
    pub creative: bool,
    pub seed: u32,
    pub last_played: Option<SystemTime>,
    pub size: u64,
    /// Set when the save couldn't be read (shown instead of mode/seed).
    pub problem: Option<String>,
}

/// Folder ids are plain names: no paths, no dots, nothing surprising.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn world_file(root: &Path, id: &str) -> PathBuf {
    root.join(id).join(WORLD_FILE)
}

/// Seed and game mode from a save's header, without reading the rest.
pub fn read_summary(path: &Path) -> io::Result<(u32, bool)> {
    let mut head = [0u8; 17];
    std::fs::File::open(path)?.read_exact(&mut head)?;
    let mut r = R(&head);
    if &r.take::<4>()? != MAGIC {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "not a Minceraft save"));
    }
    let version = r.u32()?;
    if !(1..=VERSION).contains(&version) {
        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("from a newer version ({version})")));
    }
    Ok((r.u32()?, r.u8()? != 0))
}

pub fn read_name(root: &Path, id: &str) -> String {
    std::fs::read_to_string(root.join(id).join(INFO_FILE))
        .ok()
        .and_then(|s| s.lines().find_map(|l| l.strip_prefix("name=").map(|n| n.trim().to_string())))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| id.to_string())
}

pub fn clean_name(name: &str) -> String {
    let n: String = name.chars().filter(|c| !c.is_control()).take(32).collect();
    let n = n.trim().to_string();
    if n.is_empty() { "New World".into() } else { n }
}

pub fn write_name(root: &Path, id: &str, name: &str) -> io::Result<()> {
    if !valid_id(id) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "bad world id"));
    }
    std::fs::create_dir_all(root.join(id))?;
    std::fs::write(root.join(id).join(INFO_FILE), format!("name={}\n", clean_name(name)))
}

/// Every world with a save file, most recently played first.
pub fn list_worlds(root: &Path) -> Vec<WorldEntry> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else { return out };
    for e in entries.flatten() {
        let id = e.file_name().to_string_lossy().to_string();
        let file = world_file(root, &id);
        if !valid_id(&id) || !file.is_file() {
            continue;
        }
        let meta = std::fs::metadata(&file).ok();
        let (seed, creative, problem) = match read_summary(&file) {
            Ok((seed, creative)) => (seed, creative, None),
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => (0, false, Some("the save file is damaged or incomplete".to_string())),
            Err(e) => (0, false, Some(e.to_string())),
        };
        out.push(WorldEntry {
            name: read_name(root, &id),
            id,
            creative,
            seed,
            last_played: meta.as_ref().and_then(|m| m.modified().ok()),
            size: meta.map(|m| m.len()).unwrap_or(0),
            problem,
        });
    }
    // Playable worlds first, most recent at the top; unreadable ones at the bottom.
    out.sort_by(|a, b| a.problem.is_some().cmp(&b.problem.is_some()).then(b.last_played.cmp(&a.last_played)).then(a.name.cmp(&b.name)));
    out
}

/// A fresh folder id for a world with this name ("My Base" -> "my-base", "my-base-2", ...).
pub fn new_world_id(root: &Path, name: &str) -> String {
    let mut base: String = name
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    base.truncate(40);
    if base.is_empty() {
        base = "world".into();
    }
    let mut id = base.clone();
    let mut n = 2;
    while root.join(&id).exists() {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

pub fn delete_world(root: &Path, id: &str) -> io::Result<()> {
    if !valid_id(id) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "bad world id"));
    }
    std::fs::remove_dir_all(root.join(id))
}

/// Move a save from before world slots (saves/world.mncr) into its own slot.
/// Returns the new id if something was moved.
pub fn migrate_legacy(root: &Path) -> io::Result<Option<String>> {
    let old = root.join(LEGACY_FILE);
    if !old.is_file() {
        return Ok(None);
    }
    let id = new_world_id(root, "My World");
    std::fs::create_dir_all(root.join(&id))?;
    std::fs::rename(&old, world_file(root, &id))?;
    write_name(root, &id, "My World")?;
    Ok(Some(id))
}

/// A seed from what the player typed: a number is used as-is, any other text
/// is hashed (so "cheese" always makes the same world), empty means random.
pub fn parse_seed(text: &str, random: u32) -> u32 {
    let t = text.trim();
    if t.is_empty() {
        return random;
    }
    if let Ok(n) = t.parse::<i64>() {
        return n as u32;
    }
    t.bytes().fold(0x811C_9DC5u32, |h, b| (h ^ b as u32).wrapping_mul(0x0100_0193))
}

/// "just now", "5 minutes ago", "3 days ago"...
pub fn ago(t: Option<SystemTime>) -> String {
    let Some(secs) = t.and_then(|t| SystemTime::now().duration_since(t).ok()).map(|d| d.as_secs()) else { return "never".into() };
    let (n, unit) = match secs {
        0..=59 => return "just now".into(),
        60..=3599 => (secs / 60, "minute"),
        3600..=86_399 => (secs / 3600, "hour"),
        _ => (secs / 86_400, "day"),
    };
    format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("minceraft-worlds-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn data(seed: u32, creative: bool) -> SaveData {
        SaveData {
            seed,
            creative,
            time: 0.0,
            pos: [0.0; 3],
            yaw: 0.0,
            pitch: 0.0,
            health: 20.0,
            spawn: [0.0; 3],
            slots: vec![None; 36],
            mods: HashMap::new(),
            palette: Vec::new(),
            script_vars: Vec::new(),
            advancements: Vec::new(),
            farm: Vec::new(),
            fish_log: Vec::new(),
            containers: Vec::new(),
            drops: Vec::new(),
            wear: Vec::new(),
            food: 20.0,
            saturation: 5.0,
            keep_inventory: false,
            difficulty: 2,
            daylight_cycle: true,
            xp: 0,
            players: Vec::new(),
            weather: 0,
            weather_timer: 600.0,
            weather_cycle: true,
            enchant_count: 3,
            mobs: vec![1, 2, 3],
            version: VERSION,
        }
    }

    #[test]
    fn one_byte_saves_are_widened() {
        use crate::block::{COBBLE, DIAMOND, TORCH};
        // A v4 save written by hand: one-byte ids everywhere.
        let mut w = W(Vec::new());
        w.0.extend_from_slice(MAGIC);
        w.u32(4);
        w.u32(77); // seed
        w.u8(0); // survival
        for v in [0.3f32, 1.0, 50.0, 2.0, 0.0, 50.0, 0.0, 0.5, -0.1, 18.0] {
            w.f32(v); // time, pos, spawn, yaw, pitch, health
        }
        w.u32(2); // slots
        w.u8(103); // diamond, as an old item id
        w.u8(5);
        w.u8(4); // cobblestone
        w.u8(64);
        w.u32(1); // one edited chunk
        w.i32(0);
        w.i32(-1);
        w.u32(1);
        w.u32(1234);
        w.u8(21); // a torch
        w.u32(1); // palette: an old mod item
        w.u8(118);
        w.u32(9);
        w.0.extend_from_slice(b"aaa:thing");
        w.u32(0); // script vars
        w.u32(0); // advancements
        let root = tmp("legacy-ids");
        let path = root.join("old.mncr");
        std::fs::write(&path, &w.0).unwrap();
        let d = read_from(&path).unwrap();
        assert_eq!(d.slots, vec![Some((DIAMOND, 5)), Some((COBBLE, 64))]);
        assert_eq!(d.mods[&(0, -1)][&1234], TORCH);
        assert_eq!(d.palette, vec![(legacy_id(118), "aaa:thing".to_string())]);
        assert_eq!(legacy_id(118), FIRST_ITEM + 18);
        // Written back out, it's a v5 save that reads the same.
        write_to(&path, &d).unwrap();
        let again = read_from(&path).unwrap();
        assert_eq!((again.slots, again.palette), (d.slots, d.palette));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn advancements_round_trip() {
        let root = tmp("adv");
        let path = root.join("w.mncr");
        let mut d = data(5, false);
        d.advancements = vec!["getting_wood".into(), "dimonds".into()];
        write_to(&path, &d).unwrap();
        assert_eq!(read_from(&path).unwrap().advancements, d.advancements);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn world_slots_list_rename_delete() {
        let root = tmp("slots");
        let a = new_world_id(&root, "My Base!");
        assert_eq!(a, "my-base");
        write_name(&root, &a, "My Base!").unwrap();
        write_to(&world_file(&root, &a), &data(7, false)).unwrap();
        let b = new_world_id(&root, "my base");
        assert_eq!(b, "my-base-2", "ids must not collide");
        write_name(&root, &b, "  Creative Playground  ").unwrap();
        write_to(&world_file(&root, &b), &data(99, true)).unwrap();
        // A folder without a save and a stray file are ignored.
        std::fs::create_dir_all(root.join("empty")).unwrap();
        std::fs::write(root.join("notes.txt"), "hi").unwrap();

        let list = list_worlds(&root);
        assert_eq!(list.len(), 2);
        let base = list.iter().find(|w| w.id == "my-base").unwrap();
        assert_eq!((base.name.as_str(), base.seed, base.creative), ("My Base!", 7, false));
        let play = list.iter().find(|w| w.id == "my-base-2").unwrap();
        assert_eq!((play.name.as_str(), play.seed, play.creative), ("Creative Playground", 99, true));
        assert!(play.size > 0 && play.problem.is_none());

        write_name(&root, &a, "Renamed").unwrap();
        assert_eq!(read_name(&root, &a), "Renamed");
        delete_world(&root, &b).unwrap();
        assert_eq!(list_worlds(&root).len(), 1);
        assert!(delete_world(&root, "../outside").is_err());
        assert!(delete_world(&root, "").is_err());

        // A damaged save still shows up, with the problem noted.
        std::fs::create_dir_all(root.join("broken")).unwrap();
        std::fs::write(world_file(&root, "broken"), b"nope").unwrap();
        let list = list_worlds(&root);
        assert_eq!(list.last().unwrap().id, "broken", "unreadable worlds sort last");
        assert!(list.last().unwrap().problem.as_deref().unwrap().contains("damaged"));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn old_single_save_is_migrated() {
        let root = tmp("legacy");
        write_to(&root.join("world.mncr"), &data(1234, false)).unwrap();
        let id = migrate_legacy(&root).unwrap().unwrap();
        assert!(!root.join("world.mncr").exists());
        let list = list_worlds(&root);
        assert_eq!(list.len(), 1);
        assert_eq!((list[0].id.as_str(), list[0].name.as_str(), list[0].seed), (id.as_str(), "My World", 1234));
        assert_eq!(migrate_legacy(&root).unwrap(), None, "only once");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn seeds_from_text() {
        assert_eq!(parse_seed("", 5), 5);
        assert_eq!(parse_seed(" 42 ", 5), 42);
        assert_eq!(parse_seed("-1", 5), u32::MAX);
        assert_eq!(parse_seed("cheese", 5), parse_seed("cheese", 9));
        assert_ne!(parse_seed("cheese", 5), parse_seed("chese", 5));
        assert_eq!(ago(None), "never");
        assert_eq!(ago(Some(SystemTime::now())), "just now");
        assert_eq!(ago(SystemTime::now().checked_sub(std::time::Duration::from_secs(7200))), "2 hours ago");
    }
}
