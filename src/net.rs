//! LAN multiplayer transport: a tiny length-prefixed binary protocol over
//! non-blocking TCP, using only the standard library.

use crate::block::Id;
use macroquad::math::Vec3;
use std::io::{self, Read, Write};
use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

pub const DEFAULT_PORT: u16 = 25565;
/// v2: challenge/response login. v3: the host sends its mods to joining players.
/// v4: script effects (UseItem, Effect). v5: two-byte block/item ids, new mobs.
/// v6: mob sizes, arrows in flight, and the bow (Shoot).
/// v7: farming and fishing (Interact, Catch), stricter hosts, and host-checked
/// inventories (held item in PlayerState, Craft, Consume, InventoryCheck, Inventory).
/// v8: chests and furnaces (OpenContainer, CloseContainer, ContainerMove, Container).
/// v9: items on the ground (Drops, Pickup, DropItem), armour in PlayerState,
/// slabs, stairs and doors.
/// v10: tool and armour wear (Give, DropItem, ContainerMove, Container), death
/// drops (DropItem.scatter) and the keep-inventory rule (Welcome).
pub const PROTOCOL: u32 = 10;
/// `Chat.from` for messages from scripts or the server itself (shown without a name).
pub const SYSTEM: u32 = u32::MAX;
/// Drop a connection that has been silent this long (mob snapshots and player
/// states flow many times a second, so silence means the link is dead).
pub const TIMEOUT_SECS: f32 = 30.0;
/// A connection must finish logging in within this time.
pub const LOGIN_SECS: f32 = 15.0;
/// Largest message a host will send (mod packs can be big).
const MAX_FRAME: usize = 8 << 20;
/// Largest message a host accepts from a player: nothing a real client sends is close.
const CLIENT_MAX_FRAME: usize = 256 << 10;
/// Per poll, stop reading after this much is buffered and decode at most this
/// many messages, so one flooding connection can't stall everyone else.
const POLL_READ_CAP: usize = 1 << 20;
const POLL_MSG_CAP: usize = 512;
/// Connections from one address (loopback is exempt, for same-computer play).
const MAX_PER_IP: usize = 3;
/// Wrong passwords from one address before it's locked out, and for how long.
const MAX_LOGIN_FAILURES: u32 = 5;
const LOCKOUT: Duration = Duration::from_secs(600);

#[derive(Clone, Debug, PartialEq)]
pub struct MobSnap {
    pub id: u32,
    pub kind: u8,
    pub pos: Vec3,
    pub yaw: f32,
    pub fuse: f32,
    pub hurt: f32,
    pub burning: bool,
    /// Bloops come in sizes 1, 2 and 4; everything else is 1.
    pub size: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Msg {
    /// client -> host
    Hello { protocol: u32, name: String },
    /// host -> client, first reply
    Welcome { id: u32, seed: u32, time: f32, creative: bool, spawn: Vec3, keep_inventory: bool },
    Kick { reason: String },
    /// Player edits for one chunk (sent on join).
    Mods { cx: i32, cz: i32, entries: Vec<(u32, Id)> },
    /// Block changes, both directions.
    Blocks(Vec<(i32, i32, i32, Id)>),
    PlayerJoin { id: u32, name: String },
    PlayerLeave { id: u32 },
    /// Both directions; the host fills in `id` when relaying.
    /// `held` is the item in hand (the host only believes it if the player owns one).
    PlayerState { id: u32, pos: Vec3, yaw: f32, pitch: f32, flags: u8, held: Id, armor: u16 },
    /// Mobs, primed TNT (position, fuse) and arrows in flight (position, velocity).
    Mobs { mobs: Vec<MobSnap>, tnts: Vec<(Vec3, f32)>, arrows: Vec<(Vec3, Vec3)> },
    /// client -> host
    Attack { mob: u32, dmg: f32, from: Vec3 },
    /// client -> host
    Ignite { x: i32, y: i32, z: i32 },
    /// host -> client
    HurtYou { dmg: f32, cause: String, knock: Vec3 },
    /// host -> client: loot from a mob you killed.
    /// `wear`: how used it is, for tools and armour.
    Give { item: Id, n: u8, wear: u16 },
    Explosion { at: Vec3, r: f32 },
    Sound { sfx: u8, at: Vec3 },
    Time(f32),
    Chat { from: u32, text: String },
    /// host -> client, after Hello: prove you know the password (if any).
    Challenge { nonce: [u8; 16], password: bool },
    /// client -> host: sha256(nonce || password).
    Auth { proof: [u8; 32] },
    /// host -> client, just before Welcome: the host's mods (see mods::encode_pack).
    ModPack { data: Vec<u8> },
    /// client -> host: I right-clicked with this item (for script `on_use_item`).
    UseItem { item: Id },
    /// client -> host: fire a bow from `pos` in direction `dir`.
    Shoot { pos: Vec3, dir: Vec3 },
    /// client -> host: fertiliser or Soil Probe used on the block at x, y, z.
    Interact { x: i32, y: i32, z: i32, item: Id },
    /// client -> host: reeled in a fish at `pos` (the host decides what it is).
    Catch { pos: Vec3, bait: bool },
    /// client -> host: crafted recipe number `recipe` this many times.
    Craft { recipe: u16, times: u8 },
    /// client -> host: used up items (eating, a pearl, yeeting with Q, ...).
    Consume { item: Id, n: u8 },
    /// client -> host: "here's what I think I have" (item counts).
    InventoryCheck { items: Vec<(Id, u32)> },
    /// host -> client: what you actually have (sent when a check doesn't match).
    Inventory { items: Vec<(Id, u32)> },
    /// client -> host: I opened / closed the chest or furnace here.
    OpenContainer { x: i32, y: i32, z: i32 },
    CloseContainer { x: i32, y: i32, z: i32 },
    /// client -> host: I moved `n` of `item` into (`put`) or out of a container slot.
    ContainerMove { x: i32, y: i32, z: i32, slot: u8, item: Id, n: u8, put: bool, wear: u16 },
    /// host -> client: what's in the container you have open, and its furnace gauges (0..1).
    Container { x: i32, y: i32, z: i32, slots: Vec<(Id, u8, u16)>, burn: f32, cook: f32 },
    /// host -> client: every item on the ground: (id, position, item, count).
    Drops(Vec<(u32, Vec3, Id, u8)>),
    /// client -> host: I walked into drop `id` and have room for this many.
    Pickup { id: u32, room: u8 },
    /// client -> host: I threw these (Q), or they didn't fit in my inventory, or
    /// (`scatter`) I died and they fell out of my pockets.
    DropItem { item: Id, n: u8, wear: u16, scatter: bool },
    /// host -> client: a script did something to you.
    Effect { heal: f32, teleport: Option<Vec3>, launch: Option<f32>, take: Option<(Id, u8)> },
}

pub const FLAG_SNEAK: u8 = 1;
pub const FLAG_SWING: u8 = 2;
pub const FLAG_DEAD: u8 = 4;
pub const FLAG_HURT: u8 = 8;

// ------------------------------------------------------------------ encoding

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
    fn v3(&mut self, v: Vec3) {
        self.f32(v.x);
        self.f32(v.y);
        self.f32(v.z);
    }
    fn bytes(&mut self, b: &[u8]) {
        self.0.extend_from_slice(b);
    }
    fn str(&mut self, s: &str) {
        let b = &s.as_bytes()[..s.len().min(1024)];
        self.u32(b.len() as u32);
        self.0.extend_from_slice(b);
    }
}

struct R<'a>(&'a [u8]);
impl R<'_> {
    fn take(&mut self, n: usize) -> io::Result<&[u8]> {
        if self.0.len() < n {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "truncated message"));
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Ok(a)
    }
    fn u8(&mut self) -> io::Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> io::Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> io::Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn i32(&mut self) -> io::Result<i32> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn f32(&mut self) -> io::Result<f32> {
        let v = f32::from_le_bytes(self.take(4)?.try_into().unwrap());
        if v.is_finite() { Ok(v) } else { Err(io::Error::new(io::ErrorKind::InvalidData, "non-finite float")) }
    }
    fn v3(&mut self) -> io::Result<Vec3> {
        Ok(Vec3::new(self.f32()?, self.f32()?, self.f32()?))
    }
    fn arr<const N: usize>(&mut self) -> io::Result<[u8; N]> {
        Ok(self.take(N)?.try_into().unwrap())
    }
    fn str(&mut self) -> io::Result<String> {
        let n = self.u32()? as usize;
        if n > 1024 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "string too long"));
        }
        Ok(String::from_utf8_lossy(self.take(n)?).into_owned())
    }
    /// Element count, sanity-checked against the bytes actually left.
    fn count(&mut self, min_elem: usize) -> io::Result<usize> {
        let n = self.u32()? as usize;
        if n.saturating_mul(min_elem) > self.0.len() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "bad element count"));
        }
        Ok(n)
    }
}

impl Msg {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = W(Vec::with_capacity(32));
        match self {
            Msg::Hello { protocol, name } => {
                w.u8(0);
                w.u32(*protocol);
                w.str(name);
            }
            Msg::Welcome { id, seed, time, creative, spawn, keep_inventory } => {
                w.u8(1);
                w.u32(*id);
                w.u32(*seed);
                w.f32(*time);
                w.u8(*creative as u8);
                w.v3(*spawn);
                w.u8(*keep_inventory as u8);
            }
            Msg::Kick { reason } => {
                w.u8(2);
                w.str(reason);
            }
            Msg::Mods { cx, cz, entries } => {
                w.u8(3);
                w.i32(*cx);
                w.i32(*cz);
                w.u32(entries.len() as u32);
                for &(i, id) in entries {
                    w.u32(i);
                    w.u16(id);
                }
            }
            Msg::Blocks(list) => {
                w.u8(4);
                w.u32(list.len() as u32);
                for &(x, y, z, id) in list {
                    w.i32(x);
                    w.i32(y);
                    w.i32(z);
                    w.u16(id);
                }
            }
            Msg::PlayerJoin { id, name } => {
                w.u8(5);
                w.u32(*id);
                w.str(name);
            }
            Msg::PlayerLeave { id } => {
                w.u8(6);
                w.u32(*id);
            }
            Msg::PlayerState { id, pos, yaw, pitch, flags, held, armor } => {
                w.u8(7);
                w.u32(*id);
                w.v3(*pos);
                w.f32(*yaw);
                w.f32(*pitch);
                w.u8(*flags);
                w.u16(*held);
                w.u16(*armor);
            }
            Msg::Mobs { mobs, tnts, arrows } => {
                w.u8(8);
                w.u32(mobs.len() as u32);
                for m in mobs {
                    w.u32(m.id);
                    w.u8(m.kind);
                    w.v3(m.pos);
                    w.f32(m.yaw);
                    w.f32(m.fuse);
                    w.f32(m.hurt);
                    w.u8(m.burning as u8);
                    w.u8(m.size);
                }
                w.u32(tnts.len() as u32);
                for &(p, f) in tnts {
                    w.v3(p);
                    w.f32(f);
                }
                w.u32(arrows.len() as u32);
                for &(p, v) in arrows {
                    w.v3(p);
                    w.v3(v);
                }
            }
            Msg::Attack { mob, dmg, from } => {
                w.u8(9);
                w.u32(*mob);
                w.f32(*dmg);
                w.v3(*from);
            }
            Msg::Ignite { x, y, z } => {
                w.u8(10);
                w.i32(*x);
                w.i32(*y);
                w.i32(*z);
            }
            Msg::HurtYou { dmg, cause, knock } => {
                w.u8(11);
                w.f32(*dmg);
                w.str(cause);
                w.v3(*knock);
            }
            Msg::Give { item, n, wear } => {
                w.u8(12);
                w.u16(*item);
                w.u8(*n);
                w.u16(*wear);
            }
            Msg::Explosion { at, r } => {
                w.u8(13);
                w.v3(*at);
                w.f32(*r);
            }
            Msg::Sound { sfx, at } => {
                w.u8(14);
                w.u8(*sfx);
                w.v3(*at);
            }
            Msg::Time(t) => {
                w.u8(15);
                w.f32(*t);
            }
            Msg::Chat { from, text } => {
                w.u8(16);
                w.u32(*from);
                w.str(text);
            }
            Msg::Challenge { nonce, password } => {
                w.u8(17);
                w.bytes(nonce);
                w.u8(*password as u8);
            }
            Msg::Auth { proof } => {
                w.u8(18);
                w.bytes(proof);
            }
            Msg::ModPack { data } => {
                w.u8(19);
                w.u32(data.len() as u32);
                w.bytes(data);
            }
            Msg::UseItem { item } => {
                w.u8(20);
                w.u16(*item);
            }
            Msg::Effect { heal, teleport, launch, take } => {
                w.u8(21);
                w.f32(*heal);
                w.u8(teleport.is_some() as u8);
                w.v3(teleport.unwrap_or(Vec3::ZERO));
                w.u8(launch.is_some() as u8);
                w.f32(launch.unwrap_or(0.0));
                w.u8(take.is_some() as u8);
                let (a, b) = take.unwrap_or((0, 0));
                w.u16(a);
                w.u8(b);
            }
            Msg::Shoot { pos, dir } => {
                w.u8(22);
                w.v3(*pos);
                w.v3(*dir);
            }
            Msg::Interact { x, y, z, item } => {
                w.u8(23);
                w.i32(*x);
                w.i32(*y);
                w.i32(*z);
                w.u16(*item);
            }
            Msg::Catch { pos, bait } => {
                w.u8(24);
                w.v3(*pos);
                w.u8(*bait as u8);
            }
            Msg::Craft { recipe, times } => {
                w.u8(25);
                w.u16(*recipe);
                w.u8(*times);
            }
            Msg::Consume { item, n } => {
                w.u8(26);
                w.u16(*item);
                w.u8(*n);
            }
            Msg::InventoryCheck { items } | Msg::Inventory { items } => {
                w.u8(if matches!(self, Msg::InventoryCheck { .. }) { 27 } else { 28 });
                w.u32(items.len() as u32);
                for &(id, n) in items {
                    w.u16(id);
                    w.u32(n);
                }
            }
            Msg::OpenContainer { x, y, z } | Msg::CloseContainer { x, y, z } => {
                w.u8(if matches!(self, Msg::OpenContainer { .. }) { 29 } else { 30 });
                w.i32(*x);
                w.i32(*y);
                w.i32(*z);
            }
            Msg::ContainerMove { x, y, z, slot, item, n, put, wear } => {
                w.u8(31);
                w.i32(*x);
                w.i32(*y);
                w.i32(*z);
                w.u8(*slot);
                w.u16(*item);
                w.u8(*n);
                w.u8(*put as u8);
                w.u16(*wear);
            }
            Msg::Container { x, y, z, slots, burn, cook } => {
                w.u8(32);
                w.i32(*x);
                w.i32(*y);
                w.i32(*z);
                w.u8(slots.len() as u8);
                for &(id, n, wear) in slots {
                    w.u16(id);
                    w.u8(n);
                    w.u16(wear);
                }
                w.f32(*burn);
                w.f32(*cook);
            }
            Msg::Drops(list) => {
                w.u8(33);
                w.u32(list.len() as u32);
                for &(id, pos, item, n) in list {
                    w.u32(id);
                    w.v3(pos);
                    w.u16(item);
                    w.u8(n);
                }
            }
            Msg::Pickup { id, room } => {
                w.u8(34);
                w.u32(*id);
                w.u8(*room);
            }
            Msg::DropItem { item, n, wear, scatter } => {
                w.u8(35);
                w.u16(*item);
                w.u8(*n);
                w.u16(*wear);
                w.u8(*scatter as u8);
            }
        }
        w.0
    }

    pub fn decode(b: &[u8]) -> io::Result<Msg> {
        let mut r = R(b);
        let m = match r.u8()? {
            0 => Msg::Hello { protocol: r.u32()?, name: r.str()? },
            1 => Msg::Welcome { id: r.u32()?, seed: r.u32()?, time: r.f32()?, creative: r.u8()? != 0, spawn: r.v3()?, keep_inventory: r.u8()? != 0 },
            2 => Msg::Kick { reason: r.str()? },
            3 => {
                let (cx, cz) = (r.i32()?, r.i32()?);
                let n = r.count(6)?;
                let mut entries = Vec::with_capacity(n);
                for _ in 0..n {
                    entries.push((r.u32()?, r.u16()?));
                }
                Msg::Mods { cx, cz, entries }
            }
            4 => {
                let n = r.count(14)?;
                let mut list = Vec::with_capacity(n);
                for _ in 0..n {
                    list.push((r.i32()?, r.i32()?, r.i32()?, r.u16()?));
                }
                Msg::Blocks(list)
            }
            5 => Msg::PlayerJoin { id: r.u32()?, name: r.str()? },
            6 => Msg::PlayerLeave { id: r.u32()? },
            7 => Msg::PlayerState { id: r.u32()?, pos: r.v3()?, yaw: r.f32()?, pitch: r.f32()?, flags: r.u8()?, held: r.u16()?, armor: r.u16()? },
            8 => {
                let n = r.count(31)?;
                let mut mobs = Vec::with_capacity(n);
                for _ in 0..n {
                    mobs.push(MobSnap { id: r.u32()?, kind: r.u8()?, pos: r.v3()?, yaw: r.f32()?, fuse: r.f32()?, hurt: r.f32()?, burning: r.u8()? != 0, size: r.u8()? });
                }
                let n = r.count(16)?;
                let mut tnts = Vec::with_capacity(n);
                for _ in 0..n {
                    tnts.push((r.v3()?, r.f32()?));
                }
                let n = r.count(24)?;
                let mut arrows = Vec::with_capacity(n);
                for _ in 0..n {
                    arrows.push((r.v3()?, r.v3()?));
                }
                Msg::Mobs { mobs, tnts, arrows }
            }
            9 => Msg::Attack { mob: r.u32()?, dmg: r.f32()?, from: r.v3()? },
            10 => Msg::Ignite { x: r.i32()?, y: r.i32()?, z: r.i32()? },
            11 => Msg::HurtYou { dmg: r.f32()?, cause: r.str()?, knock: r.v3()? },
            12 => Msg::Give { item: r.u16()?, n: r.u8()?, wear: r.u16()? },
            13 => Msg::Explosion { at: r.v3()?, r: r.f32()? },
            14 => Msg::Sound { sfx: r.u8()?, at: r.v3()? },
            15 => Msg::Time(r.f32()?),
            16 => Msg::Chat { from: r.u32()?, text: r.str()? },
            17 => Msg::Challenge { nonce: r.arr()?, password: r.u8()? != 0 },
            18 => Msg::Auth { proof: r.arr()? },
            19 => {
                let n = r.count(1)?;
                Msg::ModPack { data: r.take(n)?.to_vec() }
            }
            20 => Msg::UseItem { item: r.u16()? },
            21 => {
                let heal = r.f32()?;
                let (has_t, t) = (r.u8()? != 0, r.v3()?);
                let (has_l, l) = (r.u8()? != 0, r.f32()?);
                let (has_take, a, b) = (r.u8()? != 0, r.u16()?, r.u8()?);
                Msg::Effect { heal, teleport: has_t.then_some(t), launch: has_l.then_some(l), take: has_take.then_some((a, b)) }
            }
            22 => Msg::Shoot { pos: r.v3()?, dir: r.v3()? },
            23 => Msg::Interact { x: r.i32()?, y: r.i32()?, z: r.i32()?, item: r.u16()? },
            24 => Msg::Catch { pos: r.v3()?, bait: r.u8()? != 0 },
            25 => Msg::Craft { recipe: r.u16()?, times: r.u8()? },
            26 => Msg::Consume { item: r.u16()?, n: r.u8()? },
            t @ (27 | 28) => {
                let n = r.count(6)?;
                if n > 4096 {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "inventory too long"));
                }
                let mut items = Vec::with_capacity(n);
                for _ in 0..n {
                    items.push((r.u16()?, r.u32()?));
                }
                if t == 27 { Msg::InventoryCheck { items } } else { Msg::Inventory { items } }
            }
            29 => Msg::OpenContainer { x: r.i32()?, y: r.i32()?, z: r.i32()? },
            30 => Msg::CloseContainer { x: r.i32()?, y: r.i32()?, z: r.i32()? },
            31 => Msg::ContainerMove { x: r.i32()?, y: r.i32()?, z: r.i32()?, slot: r.u8()?, item: r.u16()?, n: r.u8()?, put: r.u8()? != 0, wear: r.u16()? },
            32 => {
                let (x, y, z) = (r.i32()?, r.i32()?, r.i32()?);
                let n = r.u8()? as usize;
                let mut slots = Vec::with_capacity(n);
                for _ in 0..n {
                    slots.push((r.u16()?, r.u8()?, r.u16()?));
                }
                Msg::Container { x, y, z, slots, burn: r.f32()?, cook: r.f32()? }
            }
            33 => {
                let n = r.count(19)?;
                let mut list = Vec::with_capacity(n);
                for _ in 0..n {
                    list.push((r.u32()?, r.v3()?, r.u16()?, r.u8()?));
                }
                Msg::Drops(list)
            }
            34 => Msg::Pickup { id: r.u32()?, room: r.u8()? },
            35 => Msg::DropItem { item: r.u16()?, n: r.u8()?, wear: r.u16()?, scatter: r.u8()? != 0 },
            t => return Err(io::Error::new(io::ErrorKind::InvalidData, format!("unknown message type {t}"))),
        };
        Ok(m)
    }
}

// ------------------------------------------------------------------ sockets

/// One framed, non-blocking TCP connection.
pub struct Conn {
    stream: TcpStream,
    rbuf: Vec<u8>,
    wbuf: Vec<u8>,
    pub closed: Option<String>,
    pub opened: Instant,
    last_recv: Instant,
    /// Biggest frame accepted from the other end.
    max_frame: usize,
}

impl Conn {
    fn new(stream: TcpStream) -> io::Result<Conn> {
        stream.set_nonblocking(true)?;
        stream.set_nodelay(true)?;
        let now = Instant::now();
        Ok(Conn { stream, rbuf: Vec::new(), wbuf: Vec::new(), closed: None, opened: now, last_recv: now, max_frame: MAX_FRAME })
    }

    /// Seconds since anything arrived.
    pub fn idle_secs(&self) -> f32 {
        self.last_recv.elapsed().as_secs_f32()
    }

    pub fn peer_addr(&self) -> String {
        self.stream.peer_addr().map(|a| a.to_string()).unwrap_or_else(|_| "?".into())
    }

    pub fn peer_ip(&self) -> Option<IpAddr> {
        self.stream.peer_addr().ok().map(|a| canonical_ip(a.ip()))
    }

    /// Connect to "host", "host:port" or "ip:port" (blocks for at most a few seconds).
    pub fn connect(addr: &str) -> io::Result<Conn> {
        let targets: Vec<SocketAddr> = with_default_port(addr).to_socket_addrs()?.collect();
        let mut last = io::Error::new(io::ErrorKind::NotFound, "no address found");
        for t in targets {
            match TcpStream::connect_timeout(&t, Duration::from_secs(4)) {
                Ok(s) => return Conn::new(s),
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    pub fn send(&mut self, m: &Msg) {
        if self.closed.is_some() {
            return;
        }
        let body = m.encode();
        self.wbuf.extend_from_slice(&(body.len() as u32).to_le_bytes());
        self.wbuf.extend_from_slice(&body);
        if self.wbuf.len() > 32 << 20 {
            self.closed = Some("fell too far behind".into());
        }
    }

    pub fn flush(&mut self) {
        while !self.wbuf.is_empty() && self.closed.is_none() {
            match self.stream.write(&self.wbuf) {
                Ok(0) => self.closed = Some("connection closed".into()),
                Ok(n) => {
                    self.wbuf.drain(..n);
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => self.closed = Some(e.to_string()),
            }
        }
    }

    /// Read everything available and return complete messages.
    pub fn poll(&mut self) -> Vec<Msg> {
        let mut buf = [0u8; 16384];
        while self.closed.is_none() && self.rbuf.len() < POLL_READ_CAP {
            match self.stream.read(&mut buf) {
                Ok(0) => self.closed = Some("connection closed".into()),
                Ok(n) => {
                    self.rbuf.extend_from_slice(&buf[..n]);
                    self.last_recv = Instant::now();
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => self.closed = Some(e.to_string()),
            }
        }
        let mut out = Vec::new();
        let mut at = 0;
        while self.rbuf.len() - at >= 4 && out.len() < POLL_MSG_CAP {
            let len = u32::from_le_bytes(self.rbuf[at..at + 4].try_into().unwrap()) as usize;
            if len > self.max_frame {
                self.closed = Some("oversized message".into());
                break;
            }
            if self.rbuf.len() - at - 4 < len {
                break;
            }
            match Msg::decode(&self.rbuf[at + 4..at + 4 + len]) {
                Ok(m) => out.push(m),
                Err(e) => {
                    self.closed = Some(format!("bad data: {e}"));
                    break;
                }
            }
            at += 4 + len;
        }
        self.rbuf.drain(..at);
        out
    }
}

pub struct Client {
    pub id: u32,
    pub name: String,
    pub conn: Conn,
    /// Challenge sent, waiting for Auth.
    pub nonce: Option<[u8; 16]>,
    /// Logged in: Welcome sent.
    pub joined: bool,
    /// Token bucket for block edits (anti-grief on public servers).
    pub edit_budget: f32,
}

pub struct Server {
    listeners: Vec<TcpListener>,
    pub port: u16,
    pub clients: Vec<Client>,
    next_id: u32,
    pub password: Option<String>,
    pub max_players: usize,
    /// Addresses that may not connect at all (the dedicated server's ban list).
    pub banned: HashSet<IpAddr>,
    /// Recent wrong passwords per address: (count, first failure).
    failures: HashMap<IpAddr, (u32, Instant)>,
    /// Addresses locked out after too many wrong passwords, until when.
    lockouts: HashMap<IpAddr, Instant>,
}

/// IPv4 addresses arriving on a dual-stack socket look like ::ffff:1.2.3.4.
fn canonical_ip(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(ip),
        v4 => v4,
    }
}

impl Server {
    /// Listen on all interfaces (IPv4 and, where available, IPv6), starting at
    /// `first_port` and trying the next few if it's taken.
    pub fn open(first_port: u16) -> io::Result<Server> {
        let mut last = None;
        for port in first_port..first_port.saturating_add(10) {
            let mut listeners = Vec::new();
            // On Linux/macOS "[::]" is dual-stack and also takes IPv4, so the IPv4
            // bind may then fail harmlessly; on Windows both are needed.
            if let Ok(l) = TcpListener::bind(("::", port)) {
                listeners.push(l);
            }
            match TcpListener::bind(("0.0.0.0", port)) {
                Ok(l) => listeners.push(l),
                Err(e) => last = Some(e),
            }
            if !listeners.is_empty() {
                for l in &listeners {
                    l.set_nonblocking(true)?;
                }
                return Ok(Server {
                    listeners,
                    port,
                    clients: Vec::new(),
                    next_id: 1,
                    password: None,
                    max_players: 8,
                    banned: HashSet::new(),
                    failures: HashMap::new(),
                    lockouts: HashMap::new(),
                });
            }
        }
        Err(last.unwrap_or_else(|| io::Error::new(io::ErrorKind::AddrInUse, "no free port")))
    }

    /// Accept pending connections; returns ids of the new clients.
    pub fn accept(&mut self) -> Vec<u32> {
        let mut new = Vec::new();
        let mut streams = Vec::new();
        for l in &self.listeners {
            while let Ok((stream, _)) = l.accept() {
                streams.push(stream);
            }
        }
        let now = Instant::now();
        self.lockouts.retain(|_, until| *until > now);
        for stream in streams {
            // Hard cap on half-open logins so a flood can't exhaust us.
            if self.clients.len() >= self.max_players + 8 {
                continue;
            }
            if let Ok(mut conn) = Conn::new(stream) {
                conn.max_frame = CLIENT_MAX_FRAME;
                if let Some(ip) = conn.peer_ip() {
                    let refuse = if self.banned.contains(&ip) {
                        Some("You are banned from this server.")
                    } else if self.lockouts.contains_key(&ip) {
                        Some("Too many wrong passwords. Try again in a few minutes.")
                    } else if !ip.is_loopback() && self.clients.iter().filter(|c| c.conn.peer_ip() == Some(ip)).count() >= MAX_PER_IP {
                        Some("Too many connections from your address.")
                    } else {
                        None
                    };
                    if let Some(reason) = refuse {
                        conn.send(&Msg::Kick { reason: reason.into() });
                        conn.flush();
                        continue;
                    }
                }
                let id = self.next_id;
                self.next_id += 1;
                self.clients.push(Client { id, name: String::new(), conn, nonce: None, joined: false, edit_budget: 200.0 });
                new.push(id);
            }
        }
        new
    }

    /// Note a wrong password from this client's address; enough of them locks it out.
    /// Returns true if the address is now locked out.
    pub fn record_failure(&mut self, id: u32) -> bool {
        let Some(ip) = self.get(id).and_then(|c| c.conn.peer_ip()) else { return false };
        let now = Instant::now();
        let e = self.failures.entry(ip).or_insert((0, now));
        if now.duration_since(e.1) > LOCKOUT {
            *e = (0, now);
        }
        e.0 += 1;
        if e.0 >= MAX_LOGIN_FAILURES {
            self.failures.remove(&ip);
            self.lockouts.insert(ip, now + LOCKOUT);
            return true;
        }
        false
    }

    pub fn ip_of(&mut self, id: u32) -> Option<IpAddr> {
        self.get(id).and_then(|c| c.conn.peer_ip())
    }

    pub fn joined_count(&self) -> usize {
        self.clients.iter().filter(|c| c.joined).count()
    }

    pub fn kick(&mut self, id: u32, reason: &str) {
        if let Some(c) = self.get(id) {
            c.conn.send(&Msg::Kick { reason: reason.into() });
            c.conn.flush();
            c.conn.closed = Some(format!("kicked: {reason}"));
        }
    }

    pub fn get(&mut self, id: u32) -> Option<&mut Client> {
        self.clients.iter_mut().find(|c| c.id == id)
    }

    pub fn send_to(&mut self, id: u32, m: &Msg) {
        if let Some(c) = self.get(id) {
            c.conn.send(m);
        }
    }

    pub fn broadcast(&mut self, m: &Msg, except: Option<u32>) {
        let body = m.encode();
        for c in self.clients.iter_mut().filter(|c| c.joined && Some(c.id) != except) {
            if c.conn.closed.is_none() {
                c.conn.wbuf.extend_from_slice(&(body.len() as u32).to_le_bytes());
                c.conn.wbuf.extend_from_slice(&body);
            }
        }
    }

    pub fn flush(&mut self) {
        for c in self.clients.iter_mut() {
            c.conn.flush();
        }
    }
}

/// Add the default port unless one is given. Handles "host", "host:port",
/// "1.2.3.4", "1.2.3.4:5", "2001:db8::1" and "[2001:db8::1]:5".
pub fn with_default_port(addr: &str) -> String {
    let addr = addr.trim();
    if let Some(rest) = addr.strip_prefix('[') {
        return if rest.contains("]:") { addr.to_string() } else { format!("{}:{DEFAULT_PORT}", addr) };
    }
    match addr.matches(':').count() {
        0 => format!("{addr}:{DEFAULT_PORT}"),
        1 => addr.to_string(),
        _ => format!("[{addr}]:{DEFAULT_PORT}"), // bare IPv6
    }
}

/// A fresh random-ish challenge. Not cryptographic-grade randomness, but unique
/// per login, which is all the challenge needs to defeat replay.
pub fn make_nonce() -> [u8; 16] {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let mut seed = Vec::new();
    seed.extend_from_slice(&t.to_le_bytes());
    seed.extend_from_slice(&COUNTER.fetch_add(1, Ordering::Relaxed).to_le_bytes());
    seed.extend_from_slice(&(std::process::id() as u64).to_le_bytes());
    seed.extend_from_slice(&(&seed as *const _ as usize as u64).to_le_bytes());
    let h = sha256(&seed);
    h[..16].try_into().unwrap()
}

/// Proof that the client knows the password, without sending it.
pub fn auth_proof(nonce: &[u8; 16], password: &str) -> [u8; 32] {
    let mut data = nonce.to_vec();
    data.extend_from_slice(password.as_bytes());
    sha256(&data)
}

/// Constant-time comparison so response timing doesn't leak how close a guess was.
pub fn proof_matches(a: &[u8; 32], b: &[u8; 32]) -> bool {
    a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Plain SHA-256 (FIPS 180-4).
pub fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
        0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
        0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
        0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let mut msg = data.to_vec();
    let bit_len = (data.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for block in msg.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(block[i * 4..i * 4 + 4].try_into().unwrap());
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (x, v) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *x = x.wrapping_add(v);
        }
    }
    let mut out = [0u8; 32];
    for (i, v) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes());
    }
    out
}

/// This machine's global IPv6 address, if it has one. With IPv6, friends can
/// usually connect directly without any port forwarding (firewall permitting).
pub fn public_ipv6() -> Option<String> {
    let s = std::net::UdpSocket::bind("[::]:0").ok()?;
    s.connect("[2001:4860:4860::8888]:53").ok()?; // picks a route; sends nothing
    match s.local_addr().ok()?.ip() {
        std::net::IpAddr::V6(ip) if (ip.segments()[0] & 0xe000) == 0x2000 => Some(ip.to_string()),
        _ => None,
    }
}

/// Best guess at this machine's LAN address, for telling friends where to connect.
pub fn lan_ip() -> Option<String> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    // No packet is sent: connecting a UDP socket just picks the outgoing interface.
    s.connect("192.168.0.1:9").or_else(|_| s.connect("10.0.0.1:9")).ok()?;
    let ip = s.local_addr().ok()?.ip();
    if ip.is_unspecified() { None } else { Some(ip.to_string()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIAMOND_TEST: Id = crate::block::DIAMOND;

    #[test]
    fn messages_round_trip() {
        let msgs = vec![
            Msg::Hello { protocol: PROTOCOL, name: "Stove".into() },
            Msg::Welcome { id: 3, seed: 42, time: 0.25, creative: true, spawn: Vec3::new(1.0, 2.0, 3.0), keep_inventory: true },
            Msg::Mods { cx: -1, cz: 7, entries: vec![(5, 3), (99, 1234)] },
            Msg::Blocks(vec![(1, 2, 3, 4), (-9, 100, 12, 0x8123)]),
            Msg::PlayerState { id: 2, pos: Vec3::ONE, yaw: 1.5, pitch: -0.2, flags: FLAG_SNEAK | FLAG_SWING, held: 0x8003, armor: 0x4102 },
            Msg::Craft { recipe: 12, times: 64 },
            Msg::Consume { item: 0x8005, n: 1 },
            Msg::InventoryCheck { items: vec![(3, 64), (0x8000, 2)] },
            Msg::Inventory { items: vec![] },
            Msg::OpenContainer { x: -4, y: 60, z: 9 },
            Msg::Drops(vec![(7, Vec3::new(1.0, 64.5, -3.25), 0x8003, 12), (8, Vec3::ZERO, 4, 1)]),
            Msg::Pickup { id: 7, room: 64 },
            Msg::DropItem { item: 0x8010, n: 3, wear: 40, scatter: true },
            Msg::CloseContainer { x: 1, y: 2, z: 3 },
            Msg::ContainerMove { x: 5, y: 6, z: -7, slot: 26, item: 0x8010, n: 64, put: true, wear: 7 },
            Msg::Container { x: 0, y: 1, z: 2, slots: vec![(3, 1, 0), (0, 0, 0), (0x800c, 1, 99)], burn: 0.5, cook: 0.25 },
            Msg::Mobs {
                mobs: vec![MobSnap { id: 9, kind: 1, pos: Vec3::X, yaw: 0.1, fuse: 0.5, hurt: 0.0, burning: true, size: 4 }],
                tnts: vec![(Vec3::Z, 2.0)],
                arrows: vec![(Vec3::Y, Vec3::new(20.0, 3.0, -1.0))],
            },
            Msg::HurtYou { dmg: 3.0, cause: "was groaned".into(), knock: Vec3::Y },
            Msg::Chat { from: 0, text: "hello 🧱".into() },
            Msg::Challenge { nonce: [7; 16], password: true },
            Msg::Auth { proof: [9; 32] },
            Msg::ModPack { data: vec![1, 2, 3] },
            Msg::UseItem { item: 7 },
            Msg::Shoot { pos: Vec3::ONE, dir: -Vec3::Z },
            Msg::Interact { x: -3, y: 60, z: 9, item: 0x8022 },
            Msg::Catch { pos: Vec3::new(1.5, 39.9, -2.0), bait: true },
            Msg::Effect { heal: 2.0, teleport: Some(Vec3::new(1.0, 70.0, -3.0)), launch: None, take: Some((DIAMOND_TEST, 2)) },
            Msg::Effect { heal: 0.0, teleport: None, launch: Some(12.0), take: None },
        ];
        for m in msgs {
            assert_eq!(Msg::decode(&m.encode()).unwrap(), m);
        }
        assert!(Msg::decode(&[200]).is_err());
        assert!(Msg::decode(&[4, 255, 255, 255, 255]).is_err(), "huge counts must be rejected");
    }

    #[test]
    fn sha256_matches_known_vectors() {
        let hex = |b: [u8; 32]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
        assert_eq!(hex(sha256(b"")), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(hex(sha256(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        // Two-block message from FIPS 180-2.
        assert_eq!(
            hex(sha256(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq")),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        let n = make_nonce();
        assert_ne!(n, make_nonce());
        assert!(proof_matches(&auth_proof(&n, "pw"), &auth_proof(&n, "pw")));
        assert!(!proof_matches(&auth_proof(&n, "pw"), &auth_proof(&n, "pW")));
    }

    #[test]
    fn addresses_get_default_ports() {
        assert_eq!(with_default_port("example.com"), "example.com:25565");
        assert_eq!(with_default_port(" 1.2.3.4:7777 "), "1.2.3.4:7777");
        assert_eq!(with_default_port("2001:db8::1"), "[2001:db8::1]:25565");
        assert_eq!(with_default_port("[2001:db8::1]"), "[2001:db8::1]:25565");
        assert_eq!(with_default_port("[2001:db8::1]:9"), "[2001:db8::1]:9");
    }

    #[test]
    fn frames_survive_tcp() {
        let mut server = Server::open(DEFAULT_PORT + 100).unwrap();
        let mut client = Conn::connect(&format!("127.0.0.1:{}", server.port)).unwrap();
        client.send(&Msg::Hello { protocol: PROTOCOL, name: "a".into() });
        client.send(&Msg::Time(0.5));
        client.flush();
        let mut got = Vec::new();
        let start = std::time::Instant::now();
        while got.len() < 2 && start.elapsed() < Duration::from_secs(5) {
            server.accept();
            for c in server.clients.iter_mut() {
                got.extend(c.conn.poll());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(got, vec![Msg::Hello { protocol: PROTOCOL, name: "a".into() }, Msg::Time(0.5)]);
    }
}
