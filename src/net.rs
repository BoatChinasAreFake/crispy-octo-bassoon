//! LAN multiplayer transport: a tiny length-prefixed binary protocol over
//! non-blocking TCP, using only the standard library.

use macroquad::math::Vec3;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::time::Duration;

pub const DEFAULT_PORT: u16 = 25565;
pub const PROTOCOL: u32 = 1;
const MAX_FRAME: usize = 8 << 20;

#[derive(Clone, Debug, PartialEq)]
pub struct MobSnap {
    pub id: u32,
    pub kind: u8,
    pub pos: Vec3,
    pub yaw: f32,
    pub fuse: f32,
    pub hurt: f32,
    pub burning: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Msg {
    /// client -> host
    Hello { protocol: u32, name: String },
    /// host -> client, first reply
    Welcome { id: u32, seed: u32, time: f32, creative: bool, spawn: Vec3 },
    Kick { reason: String },
    /// Player edits for one chunk (sent on join).
    Mods { cx: i32, cz: i32, entries: Vec<(u32, u8)> },
    /// Block changes, both directions.
    Blocks(Vec<(i32, i32, i32, u8)>),
    PlayerJoin { id: u32, name: String },
    PlayerLeave { id: u32 },
    /// Both directions; the host fills in `id` when relaying.
    PlayerState { id: u32, pos: Vec3, yaw: f32, pitch: f32, flags: u8 },
    Mobs { mobs: Vec<MobSnap>, tnts: Vec<(Vec3, f32)> },
    /// client -> host
    Attack { mob: u32, dmg: f32, from: Vec3 },
    /// client -> host
    Ignite { x: i32, y: i32, z: i32 },
    /// host -> client
    HurtYou { dmg: f32, cause: String, knock: Vec3 },
    /// host -> client: loot from a mob you killed.
    Give { item: u8, n: u8 },
    Explosion { at: Vec3, r: f32 },
    Sound { sfx: u8, at: Vec3 },
    Time(f32),
    Chat { from: u32, text: String },
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
            Msg::Welcome { id, seed, time, creative, spawn } => {
                w.u8(1);
                w.u32(*id);
                w.u32(*seed);
                w.f32(*time);
                w.u8(*creative as u8);
                w.v3(*spawn);
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
                    w.u8(id);
                }
            }
            Msg::Blocks(list) => {
                w.u8(4);
                w.u32(list.len() as u32);
                for &(x, y, z, id) in list {
                    w.i32(x);
                    w.i32(y);
                    w.i32(z);
                    w.u8(id);
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
            Msg::PlayerState { id, pos, yaw, pitch, flags } => {
                w.u8(7);
                w.u32(*id);
                w.v3(*pos);
                w.f32(*yaw);
                w.f32(*pitch);
                w.u8(*flags);
            }
            Msg::Mobs { mobs, tnts } => {
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
                }
                w.u32(tnts.len() as u32);
                for &(p, f) in tnts {
                    w.v3(p);
                    w.f32(f);
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
            Msg::Give { item, n } => {
                w.u8(12);
                w.u8(*item);
                w.u8(*n);
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
        }
        w.0
    }

    pub fn decode(b: &[u8]) -> io::Result<Msg> {
        let mut r = R(b);
        let m = match r.u8()? {
            0 => Msg::Hello { protocol: r.u32()?, name: r.str()? },
            1 => Msg::Welcome { id: r.u32()?, seed: r.u32()?, time: r.f32()?, creative: r.u8()? != 0, spawn: r.v3()? },
            2 => Msg::Kick { reason: r.str()? },
            3 => {
                let (cx, cz) = (r.i32()?, r.i32()?);
                let n = r.count(5)?;
                let mut entries = Vec::with_capacity(n);
                for _ in 0..n {
                    entries.push((r.u32()?, r.u8()?));
                }
                Msg::Mods { cx, cz, entries }
            }
            4 => {
                let n = r.count(13)?;
                let mut list = Vec::with_capacity(n);
                for _ in 0..n {
                    list.push((r.i32()?, r.i32()?, r.i32()?, r.u8()?));
                }
                Msg::Blocks(list)
            }
            5 => Msg::PlayerJoin { id: r.u32()?, name: r.str()? },
            6 => Msg::PlayerLeave { id: r.u32()? },
            7 => Msg::PlayerState { id: r.u32()?, pos: r.v3()?, yaw: r.f32()?, pitch: r.f32()?, flags: r.u8()? },
            8 => {
                let n = r.count(30)?;
                let mut mobs = Vec::with_capacity(n);
                for _ in 0..n {
                    mobs.push(MobSnap { id: r.u32()?, kind: r.u8()?, pos: r.v3()?, yaw: r.f32()?, fuse: r.f32()?, hurt: r.f32()?, burning: r.u8()? != 0 });
                }
                let n = r.count(16)?;
                let mut tnts = Vec::with_capacity(n);
                for _ in 0..n {
                    tnts.push((r.v3()?, r.f32()?));
                }
                Msg::Mobs { mobs, tnts }
            }
            9 => Msg::Attack { mob: r.u32()?, dmg: r.f32()?, from: r.v3()? },
            10 => Msg::Ignite { x: r.i32()?, y: r.i32()?, z: r.i32()? },
            11 => Msg::HurtYou { dmg: r.f32()?, cause: r.str()?, knock: r.v3()? },
            12 => Msg::Give { item: r.u8()?, n: r.u8()? },
            13 => Msg::Explosion { at: r.v3()?, r: r.f32()? },
            14 => Msg::Sound { sfx: r.u8()?, at: r.v3()? },
            15 => Msg::Time(r.f32()?),
            16 => Msg::Chat { from: r.u32()?, text: r.str()? },
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
}

impl Conn {
    fn new(stream: TcpStream) -> io::Result<Conn> {
        stream.set_nonblocking(true)?;
        stream.set_nodelay(true)?;
        Ok(Conn { stream, rbuf: Vec::new(), wbuf: Vec::new(), closed: None })
    }

    /// Connect to "host", "host:port" or "ip:port" (blocks for at most a few seconds).
    pub fn connect(addr: &str) -> io::Result<Conn> {
        let addr = addr.trim();
        let with_port = if addr.rsplit_once(':').map(|(_, p)| p.parse::<u16>().is_ok()).unwrap_or(false) && !addr.ends_with(']') {
            addr.to_string()
        } else {
            format!("{addr}:{DEFAULT_PORT}")
        };
        let targets: Vec<SocketAddr> = with_port.to_socket_addrs()?.collect();
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
        while self.closed.is_none() {
            match self.stream.read(&mut buf) {
                Ok(0) => self.closed = Some("connection closed".into()),
                Ok(n) => self.rbuf.extend_from_slice(&buf[..n]),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => self.closed = Some(e.to_string()),
            }
        }
        let mut out = Vec::new();
        let mut at = 0;
        while self.rbuf.len() - at >= 4 {
            let len = u32::from_le_bytes(self.rbuf[at..at + 4].try_into().unwrap()) as usize;
            if len > MAX_FRAME {
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
    /// Hello received and Welcome sent.
    pub joined: bool,
}

pub struct Server {
    listener: TcpListener,
    pub port: u16,
    pub clients: Vec<Client>,
    next_id: u32,
}

impl Server {
    /// Listen on all interfaces, trying a few ports if the default is taken.
    pub fn open() -> io::Result<Server> {
        let mut last = None;
        for port in DEFAULT_PORT..DEFAULT_PORT + 10 {
            match TcpListener::bind(("0.0.0.0", port)) {
                Ok(listener) => {
                    listener.set_nonblocking(true)?;
                    return Ok(Server { listener, port, clients: Vec::new(), next_id: 1 });
                }
                Err(e) => last = Some(e),
            }
        }
        Err(last.unwrap())
    }

    /// Accept pending connections; returns ids of the new clients.
    pub fn accept(&mut self) -> Vec<u32> {
        let mut new = Vec::new();
        while let Ok((stream, _)) = self.listener.accept() {
            if let Ok(conn) = Conn::new(stream) {
                let id = self.next_id;
                self.next_id += 1;
                self.clients.push(Client { id, name: String::new(), conn, joined: false });
                new.push(id);
            }
        }
        new
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

    #[test]
    fn messages_round_trip() {
        let msgs = vec![
            Msg::Hello { protocol: PROTOCOL, name: "Stove".into() },
            Msg::Welcome { id: 3, seed: 42, time: 0.25, creative: true, spawn: Vec3::new(1.0, 2.0, 3.0) },
            Msg::Mods { cx: -1, cz: 7, entries: vec![(5, 3), (99, 18)] },
            Msg::Blocks(vec![(1, 2, 3, 4), (-9, 100, 12, 0)]),
            Msg::PlayerState { id: 2, pos: Vec3::ONE, yaw: 1.5, pitch: -0.2, flags: FLAG_SNEAK | FLAG_SWING },
            Msg::Mobs {
                mobs: vec![MobSnap { id: 9, kind: 1, pos: Vec3::X, yaw: 0.1, fuse: 0.5, hurt: 0.0, burning: true }],
                tnts: vec![(Vec3::Z, 2.0)],
            },
            Msg::HurtYou { dmg: 3.0, cause: "was groaned".into(), knock: Vec3::Y },
            Msg::Chat { from: 0, text: "hello 🧱".into() },
        ];
        for m in msgs {
            assert_eq!(Msg::decode(&m.encode()).unwrap(), m);
        }
        assert!(Msg::decode(&[200]).is_err());
        assert!(Msg::decode(&[4, 255, 255, 255, 255]).is_err(), "huge counts must be rejected");
    }

    #[test]
    fn frames_survive_tcp() {
        let mut server = Server::open().unwrap();
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
