//! Finding servers: the saved server list, LAN games that turn up by
//! themselves, and asking a server how it's doing.
//!
//! - **The list** (`servers.txt`, beside the settings) keeps the servers you
//!   add, each with the key it showed the first time you reached it. Any
//!   server you join is remembered by key the same way (trust on first
//!   use): if a server's key ever changes, joining stops with a warning, as
//!   someone may be pretending to be it. Removing it from the list (or
//!   pressing Forget Key) clears the old key.
//! - **LAN games.** A host answers a small UDP call (`MCQ1`) on the first
//!   free port from 25565; the Multiplayer screen calls every couple of
//!   seconds (broadcast and to this computer) and lists whoever answers.
//! - **Status.** Each listed server is asked, over a real (encrypted)
//!   connection, its name, players, limit and version (`StatusAsk`); the
//!   time to answer is its ping.

use crate::crypto::{from_hex, to_hex};
use crate::net::{with_default_port, Conn, Msg};
use std::net::{SocketAddr, UdpSocket};
use std::sync::mpsc;
use std::time::{Duration, Instant};

pub const SERVERS_FILE: &str = "servers.txt";
/// UDP ports LAN hosts answer on (the first free one).
pub const LAN_PORTS: std::ops::Range<u16> = 25565..25575;
const ASK: &[u8; 4] = b"MCQ1";
const ANSWER: &[u8; 4] = b"MCA1";
/// A LAN game not heard from for this long is gone.
const LAN_FORGET: Duration = Duration::from_secs(7);
/// How often to call out for LAN games.
const LAN_ASK_EVERY: Duration = Duration::from_millis(2000);

/// A server on the list.
#[derive(Clone, Debug, PartialEq)]
pub struct Saved {
    pub name: String,
    pub addr: String,
}

/// What became of checking a server's key against the one remembered.
#[derive(Debug, PartialEq)]
pub enum KeyCheck {
    /// Never seen before: remembered now.
    New,
    Same,
    /// Different from last time (the old one).
    Changed([u8; 32]),
}

/// The saved servers, and every server's key as first seen.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ServerList {
    pub servers: Vec<Saved>,
    pub keys: Vec<(String, [u8; 32])>,
}

/// The same server written different ways is one entry: "Host" and "host:25565".
pub fn norm(addr: &str) -> String {
    with_default_port(addr).to_lowercase()
}

fn clean(s: &str) -> String {
    s.chars().filter(|c| !c.is_control() && *c != '|').take(48).collect::<String>().trim().to_string()
}

impl ServerList {
    pub fn parse(text: &str) -> ServerList {
        let mut list = ServerList::default();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let parts: Vec<&str> = v.split('|').collect();
            match (k.trim(), parts.as_slice()) {
                ("server", [name, addr]) if !addr.trim().is_empty() => list.servers.push(Saved { name: clean(name), addr: addr.trim().to_string() }),
                ("key", [addr, key]) => {
                    if let Some(key) = from_hex::<32>(key) {
                        list.keys.push((norm(addr), key));
                    }
                }
                _ => {}
            }
        }
        list
    }

    pub fn to_text(&self) -> String {
        let mut s = String::from("# Minceraft's saved servers (server=name|address) and each server's key as first seen.\n");
        for e in &self.servers {
            s += &format!("server={}|{}\n", clean(&e.name), e.addr);
        }
        for (addr, key) in &self.keys {
            s += &format!("key={addr}|{}\n", to_hex(key));
        }
        s
    }

    pub fn load() -> ServerList {
        std::fs::read_to_string(SERVERS_FILE).map(|t| ServerList::parse(&t)).unwrap_or_default()
    }

    pub fn save(&self) {
        let _ = std::fs::write(SERVERS_FILE, self.to_text());
    }

    /// Put a server on the list (unless it's there already). True if added.
    pub fn add(&mut self, name: &str, addr: &str) -> bool {
        let addr = addr.trim();
        if addr.is_empty() || self.servers.iter().any(|e| norm(&e.addr) == norm(addr)) {
            return false;
        }
        let name = if clean(name).is_empty() { addr.to_string() } else { clean(name) };
        self.servers.push(Saved { name, addr: addr.to_string() });
        true
    }

    /// Take a server off the list, and forget its key.
    pub fn remove(&mut self, i: usize) {
        if i < self.servers.len() {
            let e = self.servers.remove(i);
            self.forget_key(&e.addr);
        }
    }

    pub fn key_of(&self, addr: &str) -> Option<[u8; 32]> {
        let a = norm(addr);
        self.keys.iter().find(|(k, _)| *k == a).map(|(_, key)| *key)
    }

    pub fn forget_key(&mut self, addr: &str) {
        let a = norm(addr);
        self.keys.retain(|(k, _)| *k != a);
    }

    /// Check a server's key against the one remembered (remembering it if new).
    pub fn check_key(&mut self, addr: &str, key: [u8; 32]) -> KeyCheck {
        match self.key_of(addr) {
            None => {
                self.keys.push((norm(addr), key));
                KeyCheck::New
            }
            Some(k) if k == key => KeyCheck::Same,
            Some(k) => KeyCheck::Changed(k),
        }
    }

    /// Give a listed server the name it calls itself, if it hasn't one of its own.
    pub fn learn_name(&mut self, addr: &str, name: &str) -> bool {
        let Some(e) = self.servers.iter_mut().find(|e| norm(&e.addr) == norm(addr)) else { return false };
        if e.name == e.addr && !clean(name).is_empty() {
            e.name = clean(name);
            return true;
        }
        false
    }
}

// ------------------------------------------------------------------ status

/// How a server answered.
#[derive(Clone, Debug, PartialEq)]
pub struct Status {
    pub name: String,
    pub players: u16,
    pub max: u16,
    pub protocol: u32,
    pub version: String,
    pub password: bool,
    pub ping_ms: u32,
    pub key: [u8; 32],
}

/// Ask one server how it is (blocks for up to a few seconds).
pub fn ask_status(addr: &str) -> Result<Status, String> {
    let mut conn = Conn::connect(addr).map_err(|e| e.to_string())?;
    conn.send(&Msg::StatusAsk);
    conn.flush();
    let start = Instant::now();
    let mut secure_at = None;
    loop {
        let msgs = conn.poll();
        if conn.secure() && secure_at.is_none() {
            // Our question went the moment the handshake finished.
            secure_at = Some(Instant::now());
        }
        for m in msgs {
            if let Msg::Status { name, players, max, protocol, version, password } = m {
                let ping = secure_at.map(|t| t.elapsed()).unwrap_or_else(|| start.elapsed());
                return Ok(Status { name, players, max, protocol, version, password, ping_ms: ping.as_millis() as u32, key: conn.host_key.unwrap_or_default() });
            }
        }
        if let Some(e) = &conn.closed {
            return Err(e.clone());
        }
        if start.elapsed() > Duration::from_secs(5) {
            return Err("no answer".into());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// Asks servers how they are, in the background.
pub struct Prober {
    tx: mpsc::Sender<(String, Result<Status, String>)>,
    rx: mpsc::Receiver<(String, Result<Status, String>)>,
}

impl Default for Prober {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Prober { tx, rx }
    }
}

impl Prober {
    pub fn ask(&self, addr: &str) {
        let (tx, addr) = (self.tx.clone(), addr.to_string());
        std::thread::spawn(move || {
            let r = ask_status(&addr);
            let _ = tx.send((addr, r));
        });
    }

    /// Answers that have come in: (address, status or why not).
    pub fn answers(&self) -> Vec<(String, Result<Status, String>)> {
        self.rx.try_iter().collect()
    }
}

// ------------------------------------------------------------------ LAN

/// What a LAN host says about itself.
#[derive(Clone, Debug, PartialEq)]
pub struct LanInfo {
    pub port: u16,
    pub players: u16,
    pub max: u16,
    pub protocol: u32,
    pub password: bool,
    pub name: String,
}

impl LanInfo {
    pub fn encode(&self) -> Vec<u8> {
        let mut b = ANSWER.to_vec();
        b.extend_from_slice(&self.port.to_le_bytes());
        b.extend_from_slice(&self.players.to_le_bytes());
        b.extend_from_slice(&self.max.to_le_bytes());
        b.extend_from_slice(&self.protocol.to_le_bytes());
        b.push(self.password as u8);
        b.extend(self.name.bytes().take(64));
        b
    }

    pub fn decode(b: &[u8]) -> Option<LanInfo> {
        if b.len() < 15 || &b[..4] != ANSWER {
            return None;
        }
        let u16_at = |i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
        Some(LanInfo {
            port: u16_at(4),
            players: u16_at(6),
            max: u16_at(8),
            protocol: u32::from_le_bytes(b[10..14].try_into().ok()?),
            password: b[14] != 0,
            name: clean(&String::from_utf8_lossy(&b[15..b.len().min(15 + 64)])),
        })
    }
}

/// The host's side: a socket on the first free LAN port.
pub fn lan_socket() -> Option<UdpSocket> {
    LAN_PORTS.clone().find_map(|p| UdpSocket::bind(("0.0.0.0", p)).ok()).filter(|s| s.set_nonblocking(true).is_ok())
}

/// Answer anyone looking for games.
pub fn answer_lan(sock: &UdpSocket, info: &LanInfo) {
    let mut buf = [0u8; 64];
    let mut n = 0;
    while let Ok((len, from)) = sock.recv_from(&mut buf) {
        if &buf[..len.min(4)] == ASK {
            let _ = sock.send_to(&info.encode(), from);
        }
        n += 1;
        if n > 64 {
            break;
        }
    }
}

/// A game found on the LAN.
#[derive(Clone, Debug)]
pub struct LanGame {
    pub addr: String,
    pub info: LanInfo,
    pub seen: Instant,
}

/// The joining side: calls out for LAN games and keeps track of who answers.
pub struct LanScan {
    sock: UdpSocket,
    last_ask: Option<Instant>,
    pub games: Vec<LanGame>,
    /// Where to call: broadcast, this LAN's broadcast address, and this computer.
    targets: Vec<std::net::Ipv4Addr>,
}

impl LanScan {
    pub fn new() -> Option<LanScan> {
        let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
        sock.set_nonblocking(true).ok()?;
        let _ = sock.set_broadcast(true);
        let mut targets = vec![std::net::Ipv4Addr::BROADCAST, std::net::Ipv4Addr::LOCALHOST];
        if let Some(std::net::IpAddr::V4(ip)) = crate::net::lan_ip().and_then(|s| s.parse().ok()) {
            let o = ip.octets();
            targets.push(std::net::Ipv4Addr::new(o[0], o[1], o[2], 255));
        }
        Some(LanScan { sock, last_ask: None, games: Vec::new(), targets })
    }

    /// Call out now and then, and note the answers.
    pub fn tick(&mut self) {
        let now = Instant::now();
        if self.last_ask.is_none_or(|t| now.duration_since(t) >= LAN_ASK_EVERY) {
            self.last_ask = Some(now);
            for ip in &self.targets {
                for p in LAN_PORTS {
                    let _ = self.sock.send_to(ASK, SocketAddr::from((*ip, p)));
                }
            }
        }
        let mut buf = [0u8; 128];
        while let Ok((len, from)) = self.sock.recv_from(&mut buf) {
            let Some(info) = LanInfo::decode(&buf[..len]) else { continue };
            let ip = if from.ip().is_loopback() { "127.0.0.1".to_string() } else { from.ip().to_string() };
            let addr = format!("{ip}:{}", info.port);
            // The same game heard twice (broadcast and directly): keep one.
            match self.games.iter_mut().find(|g| g.addr == addr) {
                Some(g) => {
                    g.info = info;
                    g.seen = now;
                }
                None => self.games.push(LanGame { addr, info, seen: now }),
            }
        }
        self.games.retain(|g| now.duration_since(g.seen) < LAN_FORGET);
        // This computer's own games show once, as 127.0.0.1.
        let local: Vec<u16> = self.games.iter().filter(|g| g.addr.starts_with("127.")).map(|g| g.info.port).collect();
        let mine = crate::net::lan_ip();
        self.games.retain(|g| !(mine.as_deref().is_some_and(|ip| g.addr.starts_with(&format!("{ip}:"))) && local.contains(&g.info.port)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_round_trips_and_remembers_keys() {
        let mut l = ServerList::default();
        assert!(l.add("", "play.example.org"));
        assert!(l.add("Bob's | place", "10.0.0.5:25570"));
        assert!(!l.add("again", "PLAY.example.org:25565"), "the same server twice");
        assert_eq!(l.check_key("play.example.org", [1; 32]), KeyCheck::New);
        assert_eq!(l.check_key("play.example.org:25565", [1; 32]), KeyCheck::Same);
        assert_eq!(l.check_key("Play.Example.org", [2; 32]), KeyCheck::Changed([1; 32]));
        assert!(l.learn_name("play.example.org", "Example"));
        assert!(!l.learn_name("10.0.0.5:25570", "Other"), "names given by hand stay");
        let back = ServerList::parse(&l.to_text());
        assert_eq!(back, l);
        assert_eq!(back.servers[1].name, "Bob's  place");
        l.remove(0);
        assert_eq!(l.key_of("play.example.org"), None, "removing forgets the key");
    }

    #[test]
    fn lan_answers_round_trip() {
        let info = LanInfo { port: 25566, players: 2, max: 8, protocol: crate::net::PROTOCOL, password: true, name: "Stove's world".into() };
        assert_eq!(LanInfo::decode(&info.encode()), Some(info));
        assert_eq!(LanInfo::decode(b"MCA1"), None);
        assert_eq!(LanInfo::decode(b"nonsense, really nonsense"), None);
    }
}
