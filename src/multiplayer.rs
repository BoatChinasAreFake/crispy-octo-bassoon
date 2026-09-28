//! LAN multiplayer glue. The host runs the real simulation (mobs, TNT, time,
//! explosions); clients simulate only their own player and mirror the rest.
//! Block edits go both ways and the host re-broadcasts them to everyone.

use crate::block::*;
use crate::entity::{Mob, MobKind, PrimedTnt};
use crate::game::Game;
use crate::net::*;
use crate::player::Player;
use crate::sound::Sfx;
use macroquad::math::{IVec3, Vec3};

pub enum Net {
    Host(Server),
    Client(Conn),
}

pub struct Peer {
    pub name: String,
    /// Smoothed position used for drawing.
    pub pos: Vec3,
    /// Latest position received.
    pub target: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub flags: u8,
    pub anim: f32,
}

impl Peer {
    fn new(name: String, pos: Vec3) -> Peer {
        Peer { name, pos, target: pos, yaw: 0.0, pitch: 0.0, flags: 0, anim: 0.0 }
    }
    pub fn alive(&self) -> bool {
        self.flags & FLAG_DEAD == 0
    }
    pub fn intersects_block(&self, b: IVec3) -> bool {
        let (min, max) = (self.target - Vec3::new(0.3, 0.0, 0.3), self.target + Vec3::new(0.3, 1.8, 0.3));
        let (bmin, bmax) = (b.as_vec3(), b.as_vec3() + Vec3::ONE);
        self.alive() && min.x < bmax.x && max.x > bmin.x && min.y < bmax.y && max.y > bmin.y && min.z < bmax.z && max.z > bmin.z
    }
}

const MOB_KINDS: [MobKind; 3] = [MobKind::Oinker, MobKind::Hisser, MobKind::Groaner];

/// Sounds the host forwards to clients; everything else is produced locally.
fn forwarded(s: Sfx) -> bool {
    matches!(s, Sfx::Oink | Sfx::Groan | Sfx::Hiss | Sfx::MobHurt)
}

pub fn sanitize_name(name: &str) -> String {
    let n: String = name.chars().filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-').take(16).collect();
    if n.is_empty() { "Stove".into() } else { n }
}

impl Game {
    pub fn is_host(&self) -> bool {
        matches!(self.net, Some(Net::Host(_)))
    }

    pub fn is_client(&self) -> bool {
        matches!(self.net, Some(Net::Client(_)))
    }

    /// Start hosting this world. Returns the port in use.
    pub fn open_lan(&mut self, name: &str) -> std::io::Result<u16> {
        let server = Server::open()?;
        let port = server.port;
        self.net = Some(Net::Host(server));
        self.world.log_edits = true;
        self.my_id = 0;
        self.player_name = sanitize_name(name);
        self.msg(format!("Opened to LAN on port {port}. Tell your friends (or your other computer)."));
        Ok(port)
    }

    /// Build a client-side game from the host's Welcome.
    #[allow(clippy::too_many_arguments)]
    pub fn new_client(id: u32, seed: u32, time: f32, creative: bool, spawn: Vec3, conn: Conn, name: &str, leftover: Vec<Msg>) -> Game {
        let mut g = Game::new(seed, creative, false);
        g.time = time;
        g.spawn = spawn;
        g.player = Player::new(spawn);
        g.my_id = id;
        g.player_name = sanitize_name(name);
        g.world.log_edits = true;
        g.net = Some(Net::Client(conn));
        g.pending_msgs = leftover;
        g.msg("Connected! Say hi with T.");
        g
    }

    pub fn player_count(&self) -> usize {
        1 + self.peers.len()
    }

    pub fn peer_name(&self, id: u32) -> String {
        if id == self.my_id {
            return self.player_name.clone();
        }
        self.peers.get(&id).map(|p| p.name.clone()).unwrap_or_else(|| format!("Player{id}"))
    }

    pub fn net_send_msg(&mut self, m: Msg) {
        match &mut self.net {
            Some(Net::Client(c)) => c.send(&m),
            Some(Net::Host(s)) => s.broadcast(&m, None),
            None => {}
        }
    }

    pub fn net_broadcast(&mut self, m: Msg) {
        if let Some(Net::Host(s)) = &mut self.net {
            s.broadcast(&m, None);
        }
    }

    pub fn net_send_to(&mut self, id: u32, m: Msg) {
        if let Some(Net::Host(s)) = &mut self.net {
            s.send_to(id, &m);
        }
    }

    pub fn hurt_peer(&mut self, id: u32, dmg: f32, cause: &str, knock: Vec3) {
        if self.creative {
            return;
        }
        self.net_send_to(id, Msg::HurtYou { dmg, cause: cause.into(), knock });
    }

    pub fn hurt_peers_in_blast(&mut self, at: Vec3, r: f32, cause: &str) {
        if !self.is_host() {
            return;
        }
        let hits: Vec<(u32, f32, Vec3)> = self
            .peers
            .iter()
            .filter(|(_, p)| p.alive())
            .filter_map(|(&id, p)| {
                let d = (p.target + Vec3::Y * 0.9).distance(at);
                (d < r * 2.0).then(|| {
                    let k = 1.0 - d / (r * 2.0);
                    (id, k * r * 5.0, (p.target - at).normalize_or_zero() * k * 14.0 + Vec3::Y * 4.0)
                })
            })
            .collect();
        for (id, dmg, knock) in hits {
            self.hurt_peer(id, dmg, cause, knock);
        }
    }

    pub fn send_chat(&mut self, text: &str) {
        let text: String = text.trim().chars().take(200).collect();
        if text.is_empty() {
            return;
        }
        self.msg(format!("<{}> {}", self.player_name, text));
        let m = Msg::Chat { from: self.my_id, text };
        self.net_send_msg(m);
    }

    pub fn disconnect(&mut self) {
        if let Some(Net::Client(c)) = &mut self.net {
            c.flush();
        }
        self.net = None;
        self.peers.clear();
        self.world.log_edits = false;
    }

    // ------------------------------------------------------------ receive

    pub fn net_receive(&mut self, dt: f32) {
        // Smooth remote players toward their latest positions.
        for p in self.peers.values_mut() {
            let before = p.pos;
            let k = (dt * 12.0).min(1.0);
            p.pos += (p.target - p.pos) * k;
            if p.pos.distance(p.target) > 8.0 {
                p.pos = p.target;
            }
            let moved = Vec3::new(p.pos.x - before.x, 0.0, p.pos.z - before.z).length();
            p.anim += moved * 2.4;
        }

        let mut inbox: Vec<(u32, Msg)> = std::mem::take(&mut self.pending_msgs).into_iter().map(|m| (0, m)).collect();
        let mut left: Vec<(u32, String)> = Vec::new();
        match &mut self.net {
            None => return,
            Some(Net::Client(c)) => {
                inbox.extend(c.poll().into_iter().map(|m| (0, m)));
                if let Some(e) = &c.closed {
                    self.net_error = Some(format!("Lost connection to the host ({e})."));
                }
            }
            Some(Net::Host(s)) => {
                s.accept();
                for c in s.clients.iter_mut() {
                    inbox.extend(c.conn.poll().into_iter().map(|m| (c.id, m)));
                }
                for c in s.clients.iter().filter(|c| c.conn.closed.is_some()) {
                    if c.joined {
                        left.push((c.id, c.name.clone()));
                    }
                }
                s.clients.retain(|c| c.conn.closed.is_none());
            }
        }
        for (id, name) in left {
            self.peers.remove(&id);
            self.net_broadcast(Msg::PlayerLeave { id });
            self.msg(format!("{name} left the game"));
        }
        for (from, m) in inbox {
            if self.is_host() {
                self.host_handle(from, m);
            } else {
                self.client_handle(m);
            }
        }
    }

    fn host_handle(&mut self, from: u32, m: Msg) {
        let Some(Net::Host(server)) = &mut self.net else { return };
        let joined = server.get(from).map(|c| c.joined).unwrap_or(false);
        if !joined {
            // Only Hello is accepted before joining.
            let Msg::Hello { protocol, name } = m else { return };
            if protocol != PROTOCOL {
                if let Some(c) = server.get(from) {
                    c.conn.send(&Msg::Kick { reason: format!("Version mismatch (host speaks protocol {PROTOCOL}, you speak {protocol})") });
                    c.conn.flush();
                    c.conn.closed = Some("kicked".into());
                }
                return;
            }
            let mut name = sanitize_name(&name);
            let taken = |n: &str| n == self.player_name || self.peers.values().any(|p| p.name == n);
            if taken(&name) {
                name = format!("{}{}", name.chars().take(13).collect::<String>(), from);
            }
            let welcome = Msg::Welcome { id: from, seed: self.world.seed(), time: self.time, creative: self.creative, spawn: self.spawn };
            let mods: Vec<Msg> = self
                .world
                .mods
                .iter()
                .map(|(&(cx, cz), m)| Msg::Mods { cx, cz, entries: m.iter().map(|(&i, &b)| (i, b)).collect() })
                .collect();
            let mut roster = vec![Msg::PlayerJoin { id: self.my_id, name: self.player_name.clone() }];
            roster.extend(self.peers.iter().map(|(&id, p)| Msg::PlayerJoin { id, name: p.name.clone() }));
            let Some(Net::Host(server)) = &mut self.net else { return };
            if let Some(c) = server.get(from) {
                c.name = name.clone();
                c.joined = true;
                c.conn.send(&welcome);
                for m in mods.iter().chain(roster.iter()) {
                    c.conn.send(m);
                }
            }
            server.broadcast(&Msg::PlayerJoin { id: from, name: name.clone() }, Some(from));
            self.peers.insert(from, Peer::new(name.clone(), self.spawn));
            self.msg(format!("{name} joined the game"));
            return;
        }
        match m {
            Msg::Blocks(list) => {
                let quiet = list.len() > 4;
                for (x, y, z, id) in list {
                    if id >= NUM_BLOCKS {
                        continue;
                    }
                    // Logged, so the host re-broadcasts it to everyone.
                    let old = self.world.get(x, y, z);
                    self.world.set(x, y, z, id);
                    if !quiet && old != id {
                        self.block_change_feedback(IVec3::new(x, y, z), old, id);
                    }
                }
            }
            Msg::PlayerState { pos, yaw, pitch, flags, .. } => {
                if let Some(p) = self.peers.get_mut(&from) {
                    p.target = pos;
                    p.yaw = yaw;
                    p.pitch = pitch;
                    p.flags = flags;
                }
                self.relay(from, Msg::PlayerState { id: from, pos, yaw, pitch, flags });
            }
            Msg::Attack { mob, dmg, from: at } => {
                if let Some(m) = self.mobs.iter_mut().find(|m| m.id == mob) {
                    m.damage(dmg.clamp(0.0, 12.0), at);
                    m.last_attacker = from;
                    let (kind, pos) = (m.kind, m.body.pos);
                    self.sfx(if kind == MobKind::Oinker { Sfx::Oink } else { Sfx::MobHurt }, Some(pos));
                }
            }
            Msg::Ignite { x, y, z } => {
                if self.world.get(x, y, z) == TNT {
                    self.world.set(x, y, z, AIR);
                    let pos = Vec3::new(x as f32, y as f32, z as f32);
                    self.tnts.push(PrimedTnt { pos, fuse: 3.0 });
                    self.sfx(Sfx::Hiss, Some(pos + Vec3::splat(0.5)));
                }
            }
            Msg::Chat { text, .. } => {
                let text: String = text.chars().take(200).collect();
                self.msg(format!("<{}> {}", self.peer_name(from), text));
                self.relay(from, Msg::Chat { from, text });
            }
            _ => {}
        }
    }

    fn relay(&mut self, except: u32, m: Msg) {
        if let Some(Net::Host(s)) = &mut self.net {
            s.broadcast(&m, Some(except));
        }
    }

    fn client_handle(&mut self, m: Msg) {
        match m {
            Msg::Kick { reason } => self.net_error = Some(format!("Kicked: {reason}")),
            Msg::Mods { cx, cz, entries } => {
                for (i, id) in entries {
                    let (lx, rest) = ((i % 16) as i32, i / 16);
                    let (lz, y) = ((rest % 16) as i32, (rest / 16) as i32);
                    self.world.set_remote(cx * 16 + lx, y, cz * 16 + lz, id);
                }
            }
            Msg::Blocks(list) => {
                let quiet = list.len() > 4;
                for (x, y, z, id) in list {
                    if let Some(old) = self.world.set_remote(x, y, z, id) {
                        if !quiet {
                            self.block_change_feedback(IVec3::new(x, y, z), old, id);
                        }
                    }
                }
            }
            Msg::PlayerJoin { id, name } => {
                if id != self.my_id {
                    self.peers.insert(id, Peer::new(name.clone(), self.spawn));
                    // The roster sent on join arrives before our terrain is ready; only
                    // announce people who join afterwards.
                    if self.ready {
                        self.msg(format!("{name} joined the game"));
                    }
                }
            }
            Msg::PlayerLeave { id } => {
                if let Some(p) = self.peers.remove(&id) {
                    self.msg(format!("{} left the game", p.name));
                }
            }
            Msg::PlayerState { id, pos, yaw, pitch, flags } => {
                if let Some(p) = self.peers.get_mut(&id) {
                    p.target = pos;
                    p.yaw = yaw;
                    p.pitch = pitch;
                    p.flags = flags;
                }
            }
            Msg::Mobs { mobs, tnts } => self.sync_mobs(mobs, tnts),
            Msg::HurtYou { dmg, cause, knock } => {
                self.player.hurt = 0.0;
                self.hurt_player(dmg, &cause);
                self.player.body.vel += knock;
            }
            Msg::Give { item, n } => {
                if (item < NUM_BLOCKS || item >= crate::block::STICK) && n > 0 {
                    self.inv.add(item, n);
                    self.sfx(Sfx::Pop, None);
                    self.msg(format!("Loot: {n}x {}", item_name(item)));
                }
            }
            Msg::Explosion { at, r } => {
                self.sfx(Sfx::Explode, Some(at));
                self.explosion_effects(at, r);
            }
            Msg::Sound { sfx, at } => {
                if let Some(s) = Sfx::from_u8(sfx) {
                    self.sfx(s, Some(at));
                }
            }
            Msg::Time(t) => self.time = t.rem_euclid(1.0),
            Msg::Chat { from, text } => {
                let name = self.peer_name(from);
                self.msg(format!("<{name}> {text}"));
            }
            Msg::Hello { .. } | Msg::Welcome { .. } | Msg::Attack { .. } | Msg::Ignite { .. } => {}
        }
    }

    /// Mirror the host's mob list, keeping local copies so they can be smoothed.
    fn sync_mobs(&mut self, snaps: Vec<MobSnap>, tnts: Vec<(Vec3, f32)>) {
        let mut next = Vec::with_capacity(snaps.len());
        let mut old: Vec<Mob> = std::mem::take(&mut self.mobs);
        for s in snaps {
            let Some(&kind) = MOB_KINDS.get(s.kind as usize) else { continue };
            let mut m = match old.iter().position(|m| m.id == s.id) {
                Some(i) => old.swap_remove(i),
                None => {
                    let mut m = Mob::new(kind, s.pos, &mut self.rng);
                    m.id = s.id;
                    m
                }
            };
            m.net_pos = s.pos;
            m.yaw = s.yaw;
            m.fuse = s.fuse;
            m.hurt = m.hurt.max(s.hurt);
            m.burning = s.burning;
            next.push(m);
        }
        self.mobs = next;
        self.tnts = tnts.into_iter().map(|(pos, fuse)| PrimedTnt { pos, fuse }).collect();
    }

    /// Client-side entity tick: particles plus smoothing the host's mobs.
    pub fn client_entities(&mut self, dt: f32) {
        for m in self.mobs.iter_mut() {
            let before = m.body.pos;
            let k = (dt * 12.0).min(1.0);
            m.body.pos += (m.net_pos - m.body.pos) * k;
            if m.body.pos.distance(m.net_pos) > 8.0 {
                m.body.pos = m.net_pos;
            }
            m.anim += Vec3::new(m.body.pos.x - before.x, 0.0, m.body.pos.z - before.z).length() * 5.0;
            m.hurt = (m.hurt - dt).max(0.0);
        }
        for t in self.tnts.iter_mut() {
            t.fuse -= dt;
        }
        for p in self.particles.iter_mut() {
            p.update(dt, &self.world);
        }
        self.particles.retain(|p| p.life > 0.0);
    }

    // ------------------------------------------------------------ send

    pub fn net_send(&mut self, dt: f32) {
        if self.net.is_none() {
            return;
        }
        // Block edits (both directions; the host echoes everyone's).
        let edits = std::mem::take(&mut self.world.edit_log);
        for chunk in edits.chunks(4096) {
            self.net_send_msg(Msg::Blocks(chunk.to_vec()));
        }

        for t in self.net_timers.iter_mut() {
            *t -= dt;
        }
        if self.net_timers[0] <= 0.0 {
            self.net_timers[0] = 0.05;
            let p = &self.player;
            let mut flags = 0;
            if p.sneaking {
                flags |= FLAG_SNEAK;
            }
            if p.swing > 0.0 {
                flags |= FLAG_SWING;
            }
            if self.dead.is_some() {
                flags |= FLAG_DEAD;
            }
            if p.hurt > 0.2 {
                flags |= FLAG_HURT;
            }
            let m = Msg::PlayerState { id: self.my_id, pos: p.body.pos, yaw: p.yaw, pitch: p.pitch, flags };
            self.net_send_msg(m);
        }
        if self.is_host() {
            if self.net_timers[1] <= 0.0 {
                self.net_timers[1] = 0.1;
                let mobs = self
                    .mobs
                    .iter()
                    .map(|m| MobSnap {
                        id: m.id,
                        kind: MOB_KINDS.iter().position(|k| *k == m.kind).unwrap_or(0) as u8,
                        pos: m.body.pos,
                        yaw: m.yaw,
                        fuse: m.fuse,
                        hurt: m.hurt,
                        burning: m.burning,
                    })
                    .collect();
                let tnts = self.tnts.iter().map(|t| (t.pos, t.fuse)).collect();
                self.net_broadcast(Msg::Mobs { mobs, tnts });
            }
            if self.net_timers[2] <= 0.0 {
                self.net_timers[2] = 2.0;
                self.net_broadcast(Msg::Time(self.time));
            }
            let fwd: Vec<Msg> = self
                .sounds
                .iter()
                .filter_map(|&(s, at)| at.filter(|_| forwarded(s)).map(|at| Msg::Sound { sfx: s.to_u8(), at }))
                .collect();
            for m in fwd {
                self.net_broadcast(m);
            }
        }
        match &mut self.net {
            Some(Net::Client(c)) => c.flush(),
            Some(Net::Host(s)) => s.flush(),
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Controls;
    use crate::player::Input;
    use std::time::{Duration, Instant};

    fn idle() -> Controls {
        Controls {
            input: Input { forward: 0.0, strafe: 0.0, jump: false, jump_pressed: false, sneak: false, sprint: false },
            attack_held: false,
            attack_pressed: false,
            use_held: false,
            use_pressed: false,
            pick: false,
            drop: false,
        }
    }

    fn load_around(g: &mut Game, p: Vec3) {
        let start = Instant::now();
        while g.world.chunks.len() < 25 && start.elapsed() < Duration::from_secs(20) {
            g.world.stream(&[(p, 2)]);
            std::thread::sleep(Duration::from_millis(5));
        }
        g.ready = true;
    }

    /// Tick both sides until `done` holds (or give up).
    fn pump(host: &mut Game, client: &mut Game, mut done: impl FnMut(&Game, &Game) -> bool) -> bool {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(10) {
            host.update(0.016, &idle());
            client.update(0.016, &idle());
            if done(host, client) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(4));
        }
        false
    }

    #[test]
    fn host_and_client_stay_in_sync() {
        let mut host = Game::new(777, true, false);
        let spawn = host.spawn;
        load_around(&mut host, spawn);
        let port = host.open_lan("Hosty").unwrap();
        // An edit made before anyone joins must reach the client via the join snapshot.
        let (sx, sy, sz) = (spawn.x as i32 + 2, spawn.y as i32 + 3, spawn.z as i32);
        host.world.set(sx, sy, sz, GLOWROCK);

        let mut conn = Conn::connect(&format!("127.0.0.1:{port}")).unwrap();
        conn.send(&Msg::Hello { protocol: PROTOCOL, name: "Clienty".into() });
        conn.flush();
        let start = Instant::now();
        let (welcome, leftover) = loop {
            host.update(0.016, &idle());
            let mut msgs = conn.poll();
            if let Some(i) = msgs.iter().position(|m| matches!(m, Msg::Welcome { .. })) {
                let rest = msgs.split_off(i + 1);
                break (msgs.pop().unwrap(), rest);
            }
            assert!(start.elapsed() < Duration::from_secs(10), "no welcome");
            std::thread::sleep(Duration::from_millis(4));
        };
        let Msg::Welcome { id, seed, time, creative, spawn: cspawn } = welcome else { unreachable!() };
        assert_eq!(seed, 777);
        let mut client = Game::new_client(id, seed, time, creative, cspawn, conn, "Clienty", leftover);
        load_around(&mut client, cspawn);

        // Join is visible on both sides, and the pre-join edit arrived.
        assert!(pump(&mut host, &mut client, |h, c| h.peers.len() == 1 && c.peers.contains_key(&0) && c.world.get(sx, sy, sz) == GLOWROCK));
        assert_eq!(client.peers[&0].name, "Hosty");
        assert_eq!(host.peers[&id].name, "Clienty");

        // Client edit reaches the host; host edit reaches the client.
        client.world.set(sx, sy + 1, sz, BRICK);
        host.world.set(sx, sy + 2, sz, PLANKS);
        assert!(pump(&mut host, &mut client, |h, c| h.world.get(sx, sy + 1, sz) == BRICK && c.world.get(sx, sy + 2, sz) == PLANKS));

        // Mobs are owned by the host and mirrored to the client.
        let mut m = Mob::new(MobKind::Oinker, spawn + Vec3::new(3.0, 1.0, 0.0), &mut host.rng);
        m.id = 4242;
        host.mobs.push(m);
        assert!(pump(&mut host, &mut client, |_, c| c.mobs.iter().any(|m| m.id == 4242 && m.kind == MobKind::Oinker)));

        // Player movement is relayed.
        client.player.body.pos = cspawn + Vec3::new(5.0, 0.0, 5.0);
        assert!(pump(&mut host, &mut client, |h, _| h.peers[&id].target.distance(cspawn + Vec3::new(5.0, 0.0, 5.0)) < 1.0));

        // Chat both ways.
        client.send_chat("hello from the client");
        host.send_chat("hello from the host");
        assert!(pump(&mut host, &mut client, |h, c| {
            h.messages.iter().any(|m| m.0 == "<Clienty> hello from the client") && c.messages.iter().any(|m| m.0 == "<Hosty> hello from the host")
        }));

        // Leaving is noticed.
        client.disconnect();
        drop(client);
        let start = Instant::now();
        while !host.peers.is_empty() && start.elapsed() < Duration::from_secs(10) {
            host.update(0.016, &idle());
            std::thread::sleep(Duration::from_millis(4));
        }
        assert!(host.peers.is_empty(), "host never noticed the client leaving");
        assert!(host.messages.iter().any(|m| m.0 == "Clienty left the game"));
    }
}
