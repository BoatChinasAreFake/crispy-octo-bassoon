//! Tiny hand-rolled binary save format. Stores the seed, player, inventory and
//! only the blocks the player changed; terrain is regenerated from the seed.

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::path::PathBuf;

const MAGIC: &[u8; 4] = b"MNCR";
const VERSION: u32 = 1;

pub struct SaveData {
    pub seed: u32,
    pub creative: bool,
    pub time: f32,
    pub pos: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub health: f32,
    pub spawn: [f32; 3],
    pub slots: Vec<Option<(u8, u8)>>,
    pub mods: HashMap<(i32, i32), HashMap<u32, u8>>,
}

pub fn save_path() -> PathBuf {
    PathBuf::from("saves").join("world.mncr")
}

pub fn exists() -> bool {
    save_path().exists()
}

struct W(Vec<u8>);
impl W {
    fn u8(&mut self, v: u8) {
        self.0.push(v)
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
    fn u32(&mut self) -> io::Result<u32> {
        Ok(u32::from_le_bytes(self.take()?))
    }
    fn i32(&mut self) -> io::Result<i32> {
        Ok(i32::from_le_bytes(self.take()?))
    }
    fn f32(&mut self) -> io::Result<f32> {
        Ok(f32::from_le_bytes(self.take()?))
    }
}

pub fn write(d: &SaveData) -> io::Result<()> {
    write_to(&save_path(), d)
}

pub fn read() -> io::Result<SaveData> {
    read_from(&save_path())
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
        w.u8(id);
        w.u8(n);
    }
    w.u32(d.mods.len() as u32);
    for (&(cx, cz), m) in &d.mods {
        w.i32(cx);
        w.i32(cz);
        w.u32(m.len() as u32);
        for (&i, &id) in m {
            w.u32(i);
            w.u8(id);
        }
    }
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
    if &r.take::<4>()? != MAGIC || r.u32()? != VERSION {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "not a Minceraft save"));
    }
    let seed = r.u32()?;
    let creative = r.u8()? != 0;
    let time = r.f32()?;
    let pos = [r.f32()?, r.f32()?, r.f32()?];
    let spawn = [r.f32()?, r.f32()?, r.f32()?];
    let (yaw, pitch, health) = (r.f32()?, r.f32()?, r.f32()?);
    let n = r.u32()? as usize;
    let mut slots = Vec::with_capacity(n);
    for _ in 0..n {
        let (id, c) = (r.u8()?, r.u8()?);
        slots.push(if c > 0 { Some((id, c)) } else { None });
    }
    let mut mods = HashMap::new();
    for _ in 0..r.u32()? {
        let key = (r.i32()?, r.i32()?);
        let mut m = HashMap::new();
        for _ in 0..r.u32()? {
            let i = r.u32()?;
            m.insert(i, r.u8()?);
        }
        mods.insert(key, m);
    }
    Ok(SaveData { seed, creative, time, pos, yaw, pitch, health, spawn, slots, mods })
}
