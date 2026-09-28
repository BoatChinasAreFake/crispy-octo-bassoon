//! LAN multiplayer glue. The host runs the real simulation (mobs, TNT, time,
//! explosions); clients simulate only their own player and mirror the rest.
//! Block edits go both ways and the host re-broadcasts them to everyone.

use crate::block::*;
use crate::entity::{Arrow, Mob, MobKind, PrimedTnt};
use crate::game::Game;
use crate::net::*;
use crate::player::Player;
use crate::sound::Sfx;
use std::collections::HashMap;
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
    /// Game clock time each rate-limited action was last allowed (see `peer_rate_ok`).
    last: HashMap<&'static str, f32>,
    /// Chat allowance: refills at one message a second, up to five.
    chat_tokens: f32,
    /// Messages that made no sense (kicked after too many).
    strikes: u32,
}

impl Peer {
    fn new(name: String, pos: Vec3) -> Peer {
        Peer { name, pos, target: pos, yaw: 0.0, pitch: 0.0, flags: 0, anim: 0.0, last: HashMap::new(), chat_tokens: 5.0, strikes: 0 }
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

/// Sounds the host forwards to clients; everything else is produced locally.
fn forwarded(s: Sfx) -> bool {
    matches!(s, Sfx::Oink | Sfx::Groan | Sfx::Hiss | Sfx::MobHurt | Sfx::Baa | Sfx::Warp | Sfx::Cluck | Sfx::Moo | Sfx::Rattle | Sfx::Skitter | Sfx::Bloop | Sfx::Twang | Sfx::Thunk)
}

pub fn sanitize_name(name: &str) -> String {
    let n: String = name.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-').take(16).collect();
    // Nobody gets to be "Server" (script messages and the console use that name).
    let reserved = ["server", "host", "console", "admin", "system"];
    if n.is_empty() || reserved.contains(&n.to_ascii_lowercase().as_str()) { "Stove".into() } else { n }
}

/// How far a player can reach to break, place, hit or poke things (a little
/// more than the local limit, to allow for lag).
const REACH: f32 = 10.0;
/// Nonsense messages tolerated before a kick.
const MAX_STRIKES: u32 = 20;

impl Game {
    pub fn is_host(&self) -> bool {
        matches!(self.net, Some(Net::Host(_)))
    }

    pub fn is_client(&self) -> bool {
        matches!(self.net, Some(Net::Client(_)))
    }

    /// Start hosting this world. Returns the port in use.
    pub fn open_lan(&mut self, name: &str, password: Option<String>) -> std::io::Result<u16> {
        let port = self.open_server(DEFAULT_PORT, password, 8)?;
        self.player_name = sanitize_name(name);
        self.msg(format!("Opened to LAN on port {port}. Tell your friends (or your other computer)."));
        Ok(port)
    }

    /// Start accepting players (used by both player hosting and --server).
    pub fn open_server(&mut self, port: u16, password: Option<String>, max_players: usize) -> std::io::Result<u16> {
        if let Some(Net::Host(s)) = &mut self.net {
            s.password = password.filter(|p| !p.is_empty());
            return Ok(s.port);
        }
        let mut server = Server::open(port)?;
        server.password = password.filter(|p| !p.is_empty());
        server.max_players = max_players.max(1);
        let port = server.port;
        self.net = Some(Net::Host(server));
        self.world.log_edits = true;
        self.my_id = 0;
        Ok(port)
    }

    pub fn has_password(&self) -> bool {
        matches!(&self.net, Some(Net::Host(s)) if s.password.is_some())
    }

    /// Kick a player by name (host only). Returns false if nobody matched.
    pub fn kick_player(&mut self, name: &str, reason: &str) -> bool {
        let Some((&id, _)) = self.peers.iter().find(|(_, p)| p.name.eq_ignore_ascii_case(name)) else { return false };
        if let Some(Net::Host(s)) = &mut self.net {
            s.kick(id, reason);
        }
        true
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
        if id == self.my_id && !self.dedicated {
            return self.player_name.clone();
        }
        if id == 0 && !self.peers.contains_key(&0) {
            return "Server".into();
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
        let me = self.peer_name(self.my_id);
        if self.is_client() {
            // The host runs the scripts; commands aren't echoed as chat.
            if !text.starts_with('/') {
                self.msg(format!("<{me}> {text}"));
            }
            self.net_send_msg(Msg::Chat { from: self.my_id, text });
            return;
        }
        if !self.fire("on_chat", vec![me.clone().into(), text.clone().into()]) {
            return; // a script handled it
        }
        if text.starts_with('/') {
            self.msg(format!("Unknown command {}. Commands come from script mods.", text.split_whitespace().next().unwrap_or("")));
            return;
        }
        self.msg(format!("<{me}> {text}"));
        let m = Msg::Chat { from: self.my_id, text };
        self.net_send_msg(m);
    }

    /// A private line to one remote player, or to everyone with `None`.
    pub fn system_message(&mut self, to: Option<u32>, text: &str) {
        let m = Msg::Chat { from: SYSTEM, text: text.into() };
        match to {
            Some(id) => self.net_send_to(id, m),
            None => self.net_broadcast(m),
        }
    }

    pub fn peer_by_name(&self, name: &str) -> Option<u32> {
        self.peers.iter().find(|(_, p)| p.name.eq_ignore_ascii_case(name)).map(|(&id, _)| id)
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
        let mut left: Vec<(u32, String, String)> = Vec::new();
        match &mut self.net {
            None => return,
            Some(Net::Client(c)) => {
                inbox.extend(c.poll().into_iter().map(|m| (0, m)));
                if let Some(e) = &c.closed {
                    self.net_error = Some(format!("Lost connection to the host ({e})."));
                } else if c.idle_secs() > TIMEOUT_SECS {
                    self.net_error = Some("The host stopped responding (timed out).".into());
                }
            }
            Some(Net::Host(s)) => {
                s.accept();
                for c in s.clients.iter_mut() {
                    inbox.extend(c.conn.poll().into_iter().map(|m| (c.id, m)));
                    c.edit_budget = (c.edit_budget + dt * 60.0).min(200.0);
                    if let Some(p) = self.peers.get_mut(&c.id) {
                        p.chat_tokens = (p.chat_tokens + dt).min(5.0);
                    }
                    if c.conn.closed.is_none() {
                        if !c.joined && c.conn.opened.elapsed().as_secs_f32() > LOGIN_SECS {
                            c.conn.closed = Some("login timed out".into());
                        } else if c.conn.idle_secs() > TIMEOUT_SECS {
                            c.conn.closed = Some("timed out".into());
                        }
                    }
                }
                for c in s.clients.iter().filter(|c| c.conn.closed.is_some()) {
                    if c.joined {
                        left.push((c.id, c.name.clone(), c.conn.closed.clone().unwrap_or_default()));
                    }
                }
                s.clients.retain(|c| c.conn.closed.is_none());
            }
        }
        for (id, name, why) in left {
            self.peers.remove(&id);
            self.net_broadcast(Msg::PlayerLeave { id });
            // Ordinary goodbyes stay short; anything odd is worth a line in the log.
            if why == "connection closed" || why.starts_with("kicked") {
                self.msg(format!("{name} left the game"));
            } else {
                self.msg(format!("{name} left the game ({why})"));
            }
            self.fire("on_player_leave", vec![name.into()]);
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
            // Login: Hello -> Challenge -> Auth -> Welcome. Nothing else is accepted before.
            match m {
                Msg::Hello { protocol, name } => {
                    if protocol != PROTOCOL {
                        server.kick(from, &format!("Version mismatch (host speaks protocol {PROTOCOL}, you speak {protocol}). Update your game."));
                        return;
                    }
                    if server.joined_count() >= server.max_players {
                        server.kick(from, &format!("The server is full ({} players).", server.max_players));
                        return;
                    }
                    let password = server.password.is_some();
                    if let Some(c) = server.get(from) {
                        if c.nonce.is_some() {
                            return;
                        }
                        let nonce = make_nonce();
                        c.name = sanitize_name(&name);
                        c.nonce = Some(nonce);
                        c.conn.send(&Msg::Challenge { nonce, password });
                    }
                }
                Msg::Auth { proof } => {
                    let pw = server.password.clone();
                    let Some(c) = server.get(from) else { return };
                    let Some(nonce) = c.nonce else { return };
                    if let Some(pw) = pw {
                        if !proof_matches(&proof, &auth_proof(&nonce, &pw)) {
                            let who = c.conn.peer_addr();
                            let locked = server.record_failure(from);
                            server.kick(from, "Wrong password.");
                            self.msg(format!("Rejected a login from {who} (wrong password)"));
                            if locked {
                                self.msg(format!("Locked out {who} for 10 minutes after repeated wrong passwords"));
                            }
                            return;
                        }
                    }
                    self.complete_join(from);
                }
                _ => {}
            }
            return;
        }
        self.host_handle_joined(from, m);
    }

    fn complete_join(&mut self, from: u32) {
        let name = match &mut self.net {
            Some(Net::Host(server)) => server.get(from).map(|c| c.name.clone()),
            _ => None,
        };
        let Some(mut name) = name else { return };
        {
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
            let mut roster = Vec::new();
            if !self.dedicated {
                roster.push(Msg::PlayerJoin { id: self.my_id, name: self.player_name.clone() });
            }
            roster.extend(self.peers.iter().map(|(&id, p)| Msg::PlayerJoin { id, name: p.name.clone() }));
            let Some(Net::Host(server)) = &mut self.net else { return };
            if let Some(c) = server.get(from) {
                c.name = name.clone();
                c.joined = true;
                // Mods first: the client needs them before it builds its world.
                c.conn.send(&Msg::ModPack { data: crate::mods::active_pack() });
                c.conn.send(&welcome);
                for m in mods.iter().chain(roster.iter()) {
                    c.conn.send(m);
                }
            }
            server.broadcast(&Msg::PlayerJoin { id: from, name: name.clone() }, Some(from));
            self.peers.insert(from, Peer::new(name.clone(), self.spawn));
            self.msg(format!("{name} joined the game"));
            self.fire("on_player_join", vec![name.clone().into()]);
        }
    }

    fn host_handle_joined(&mut self, from: u32, m: Msg) {
        match m {
            Msg::Blocks(list) => {
                let quiet = list.len() > 4;
                // Anti-grief: edits must be within reach of where that player is,
                // and at a human-ish rate. Rejected edits are corrected on their screen.
                let eye = self.peers.get(&from).map(|p| p.target + Vec3::Y * 1.6);
                let mut budget = match &mut self.net {
                    Some(Net::Host(s)) => s.get(from).map(|c| c.edit_budget).unwrap_or(0.0),
                    _ => 0.0,
                };
                let mut corrections = Vec::new();
                for (x, y, z, id) in list {
                    let center = Vec3::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5);
                    let in_reach = eye.map(|e| e.distance(center) <= 10.0).unwrap_or(false);
                    if !valid_block(id) || !in_reach || budget < 1.0 {
                        corrections.push((x, y, z, self.world.get(x, y, z)));
                        continue;
                    }
                    budget -= 1.0;
                    let old = self.world.get(x, y, z);
                    // Only changes the game's rules allow (no stone-to-diamond, no bedrock breaking).
                    if old != id && !self.edit_allowed(x, y, z, old, id) {
                        corrections.push((x, y, z, old));
                        self.strike(from);
                        continue;
                    }
                    // Scripts may veto what remote players do, just like the host's own actions.
                    if old != id && self.scripts.is_some() {
                        let who = self.peer_name(from);
                        let (hook, key) = if id == AIR || id == WATER { ("on_block_break", old) } else { ("on_block_place", id) };
                        let args = vec![who.into(), (x as rhai::INT).into(), (y as rhai::INT).into(), (z as rhai::INT).into(), reg().key_of(key).into()];
                        if !self.fire(hook, args) {
                            corrections.push((x, y, z, old));
                            continue;
                        }
                    }
                    // Logged, so the host re-broadcasts it to everyone.
                    self.world.set(x, y, z, id);
                    if !quiet && old != id {
                        self.block_change_feedback(IVec3::new(x, y, z), old, id);
                    }
                }
                if let Some(Net::Host(s)) = &mut self.net {
                    if let Some(c) = s.get(from) {
                        c.edit_budget = budget;
                        if !corrections.is_empty() {
                            c.conn.send(&Msg::Blocks(corrections));
                        }
                    }
                }
            }
            Msg::PlayerState { pos, yaw, pitch, flags, .. } => {
                let Some(p) = self.peers.get_mut(&from) else { return };
                // No NaNs, nothing absurd, and no teleporting across the map (the
                // host's own teleports move `target` first, so they pass).
                let sane = pos.is_finite() && yaw.is_finite() && pitch.is_finite() && (-64.0..512.0).contains(&pos.y);
                if !sane || pos.distance(p.target) > 64.0 {
                    let back = p.target;
                    self.net_send_to(from, Msg::Effect { heal: 0.0, teleport: Some(back), launch: None, take: None });
                    self.strike(from);
                    return;
                }
                p.target = pos;
                p.yaw = yaw;
                p.pitch = pitch.clamp(-1.6, 1.6);
                p.flags = flags;
                self.relay(from, Msg::PlayerState { id: from, pos, yaw, pitch, flags });
            }
            Msg::Attack { mob, dmg, .. } => {
                // Hits come from where the player actually is, within reach, at a human pace.
                let Some(eye) = self.peers.get(&from).map(|p| p.target + Vec3::Y * 1.6) else { return };
                if !self.peer_rate_ok(from, "attack", 0.2) || !dmg.is_finite() {
                    return;
                }
                if let Some(m) = self.mobs.iter_mut().find(|m| m.id == mob && (m.body.pos + Vec3::Y * m.body.height * 0.5).distance(eye) <= REACH) {
                    m.damage(dmg.clamp(0.0, 12.0), eye);
                    m.last_attacker = from;
                    let (kind, pos) = (m.kind, m.body.pos);
                    self.sfx(Sfx::hurt_of(kind), Some(pos));
                }
            }
            Msg::Ignite { x, y, z } => {
                let near = self.peers.get(&from).map(|p| (p.target + Vec3::Y * 1.6).distance(Vec3::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5)) <= REACH).unwrap_or(false);
                if !near || !self.peer_rate_ok(from, "ignite", 0.25) {
                    return;
                }
                if self.world.get(x, y, z) == TNT {
                    self.world.set(x, y, z, AIR);
                    let pos = Vec3::new(x as f32, y as f32, z as f32);
                    self.tnts.push(PrimedTnt { pos, fuse: 3.0 });
                    self.sfx(Sfx::Hiss, Some(pos + Vec3::splat(0.5)));
                }
            }
            // A mod item/block effect on a client that wants an explosion.
            Msg::Explosion { at, r } => {
                let near = self.peers.get(&from).map(|p| p.target.distance(at) < 12.0).unwrap_or(false);
                if near && r.is_finite() && self.peer_rate_ok(from, "explosion", 1.0) {
                    self.explode(at, r.clamp(0.5, 6.0), "was caught in a modded explosion");
                }
            }
            Msg::Chat { text, .. } => {
                // No control characters (they'd mess up everyone's chat), and no floods.
                let text: String = text.chars().filter(|c| !c.is_control()).take(200).collect::<String>().trim().to_string();
                if text.is_empty() {
                    return;
                }
                let Some(p) = self.peers.get_mut(&from) else { return };
                if p.chat_tokens < 1.0 {
                    self.system_message(Some(from), "You're sending messages too fast. Take a breath.");
                    return;
                }
                p.chat_tokens -= 1.0;
                let who = self.peer_name(from);
                if !self.fire("on_chat", vec![who.clone().into(), text.clone().into()]) {
                    return;
                }
                if text.starts_with('/') {
                    let cmd = text.split_whitespace().next().unwrap_or("").to_string();
                    self.system_message(Some(from), &format!("Unknown command {cmd}. Commands come from script mods."));
                    return;
                }
                self.msg(format!("<{who}> {text}"));
                self.relay(from, Msg::Chat { from, text });
            }
            Msg::UseItem { item } => {
                if valid_item(item) && self.peer_rate_ok(from, "use", 0.1) {
                    let who = self.peer_name(from);
                    self.fire("on_use_item", vec![who.into(), reg().key_of(item).into()]);
                }
            }
            Msg::Shoot { pos, dir } => {
                // Only from roughly where they are, in a real direction, at a bow's pace.
                let Some(p) = self.peers.get(&from) else { return };
                let near = pos.is_finite() && pos.distance(p.target + Vec3::Y * 1.6) < 3.0;
                if near && dir.is_finite() && dir.length() > 0.5 && self.peer_rate_ok(from, "shoot", 0.4) {
                    self.spawn_arrow(pos, dir.normalize() * Arrow::SPEED * 1.2, Some(from));
                }
            }
            Msg::Interact { x, y, z, item } => {
                let at = Vec3::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5);
                let near = self.peers.get(&from).map(|p| (p.target + Vec3::Y * 1.6).distance(at) <= REACH).unwrap_or(false);
                let tool = matches!(item, BONE_DUST | COMPOST | WOOD_ASH | SOIL_PROBE);
                if near
                    && tool
                    && self.peer_rate_ok(from, "interact", 0.2)
                    && let Some(reply) = self.farm_interact(IVec3::new(x, y, z), item)
                {
                    self.system_message(Some(from), &reply);
                }
            }
            Msg::Catch { pos, bait } => self.host_catch(from, pos, bait),
            // Joined players have no business sending anything else.
            _ => self.strike(from),
        }
    }

    /// Allow `what` from this player at most once per `secs` (by the game clock).
    pub fn peer_rate_ok(&mut self, from: u32, what: &'static str, secs: f32) -> bool {
        let clock = self.clock;
        let Some(p) = self.peers.get_mut(&from) else { return false };
        let last = p.last.entry(what).or_insert(f32::NEG_INFINITY);
        if clock - *last < secs {
            return false;
        }
        *last = clock;
        true
    }

    /// Count a message that broke the rules; too many and they're kicked.
    fn strike(&mut self, from: u32) {
        let Some(p) = self.peers.get_mut(&from) else { return };
        p.strikes += 1;
        if p.strikes == MAX_STRIKES + 1 {
            let name = p.name.clone();
            if let Some(Net::Host(s)) = &mut self.net {
                s.kick(from, "Too many invalid actions.");
            }
            self.msg(format!("Kicked {name}: too many invalid actions"));
        }
    }

    /// The game's rules for a joined player turning `old` into `new` at x, y, z.
    /// (Their own game follows the same rules; this stops modified clients.)
    pub fn edit_allowed(&self, x: i32, y: i32, z: i32, old: Id, new: Id) -> bool {
        use crate::farming::{is_farmland, Crop};
        // Unbreakable stays unbroken (placing into water is fine: it's replaceable).
        if block(old).hardness < 0.0 && !replaceable(old) {
            return false;
        }
        let p = IVec3::new(x, y, z);
        // Crops only appear on farmland, and only as seedlings: the host grows them.
        if let Some((_, stage)) = Crop::of_block(new) {
            let on_farmland = is_farmland(self.world.get(x, y - 1, z));
            return replaceable(old) && on_farmland && (stage == 0 || (self.creative && block(new).creative));
        }
        match new {
            AIR => true,
            // Melting ice, or water running into a hole next to water.
            WATER => old == ICE || [IVec3::X, -IVec3::X, IVec3::Z, -IVec3::Z, IVec3::Y].iter().any(|d| self.world.get_v(p + *d) == WATER),
            // Tilling, and trampling.
            n if is_farmland(n) => matches!(old, GRASS | DIRT | SNOW_GRASS),
            DIRT if is_farmland(old) => true,
            // Placing: only into an empty-ish cell, and only blocks a player could have.
            _ => replaceable(old) && block(new).creative,
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
            Msg::Mobs { mobs, tnts, arrows } => self.sync_mobs(mobs, tnts, arrows),
            Msg::HurtYou { dmg, cause, knock } => {
                self.player.hurt = 0.0;
                self.hurt_player(dmg, &cause);
                self.player.body.vel += knock;
            }
            Msg::Give { item, n } => {
                if valid_item(item) && n > 0 {
                    self.msg(format!("Loot: {n}x {}", item_name(item)));
                    self.give(item, n);
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
            Msg::Chat { from: SYSTEM, text } => self.msg(text),
            Msg::Chat { from, text } => {
                let name = self.peer_name(from);
                self.msg(format!("<{name}> {text}"));
            }
            Msg::Effect { heal, teleport, launch, take } => {
                if heal > 0.0 && self.dead.is_none() {
                    self.player.health = (self.player.health + heal).min(crate::player::MAX_HEALTH);
                }
                if let Some(p) = teleport {
                    self.player.body.pos = p;
                    self.player.body.vel = Vec3::ZERO;
                    self.player.fall_start = p.y;
                }
                if let Some(v) = launch {
                    self.player.body.vel.y = v;
                    self.player.fall_start = self.player.body.pos.y;
                }
                if let Some((item, n)) = take {
                    self.inv.remove(item, n as u32);
                }
            }
            Msg::Hello { .. } | Msg::Welcome { .. } | Msg::Attack { .. } | Msg::Ignite { .. } | Msg::Challenge { .. } | Msg::Auth { .. } | Msg::ModPack { .. } | Msg::UseItem { .. } | Msg::Shoot { .. } | Msg::Interact { .. } | Msg::Catch { .. } => {}
        }
    }

    /// Mirror the host's mob list, keeping local copies so they can be smoothed.
    fn sync_mobs(&mut self, snaps: Vec<MobSnap>, tnts: Vec<(Vec3, f32)>, arrows: Vec<(Vec3, Vec3)>) {
        let mut next = Vec::with_capacity(snaps.len());
        let mut old: Vec<Mob> = std::mem::take(&mut self.mobs);
        for s in snaps {
            let Some(kind) = MobKind::from_index(s.kind) else { continue };
            let mut m = match old.iter().position(|m| m.id == s.id) {
                Some(i) => old.swap_remove(i),
                None => {
                    let mut m = Mob::new(kind, s.pos, &mut self.rng).with_size(s.size);
                    m.id = s.id;
                    m
                }
            };
            m.net_pos = s.pos;
            m.yaw = s.yaw;
            // Starers reuse the fuse field for "angry".
            m.angry = kind == MobKind::Starer && s.fuse > 0.0;
            m.fuse = if m.angry { 0.0 } else { s.fuse };
            m.hurt = m.hurt.max(s.hurt);
            m.burning = s.burning;
            next.push(m);
        }
        self.mobs = next;
        self.tnts = tnts.into_iter().map(|(pos, fuse)| PrimedTnt { pos, fuse }).collect();
        self.arrows = arrows.into_iter().map(|(p, v)| Arrow::from_wire(p, v)).collect();
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
        // Arrows in flight keep moving between snapshots.
        for a in self.arrows.iter_mut().filter(|a| !a.stuck) {
            a.pos += a.vel * dt;
            a.vel.y -= 20.0 * dt;
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
        if self.net_timers[0] <= 0.0 && !self.dedicated {
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
                        kind: m.kind.index(),
                        pos: m.body.pos,
                        yaw: m.yaw,
                        fuse: if m.angry { 1.0 } else { m.fuse },
                        hurt: m.hurt,
                        burning: m.burning,
                        size: m.size as u8,
                    })
                    .collect();
                let tnts = self.tnts.iter().map(|t| (t.pos, t.fuse)).collect();
                let arrows = self.arrows.iter().map(|a| (a.pos, a.wire_vel())).collect();
                self.net_broadcast(Msg::Mobs { mobs, tnts, arrows });
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
        let port = host.open_lan("Hosty", Some("hunter2".into())).unwrap();
        // An edit made before anyone joins must reach the client via the join snapshot.
        let (sx, sy, sz) = (spawn.x as i32 + 2, spawn.y as i32 + 3, spawn.z as i32);
        host.world.set(sx, sy, sz, GLOWROCK);

        // A wrong password is refused.
        let mut bad = Conn::connect(&format!("127.0.0.1:{port}")).unwrap();
        bad.send(&Msg::Hello { protocol: PROTOCOL, name: "Mallory".into() });
        bad.flush();
        let start = Instant::now();
        let kicked = loop {
            host.update(0.016, &idle());
            let msgs = bad.poll();
            if let Some(Msg::Challenge { nonce, password }) = msgs.iter().find(|m| matches!(m, Msg::Challenge { .. })) {
                assert!(*password);
                bad.send(&Msg::Auth { proof: auth_proof(nonce, "hunter3") });
                bad.flush();
            }
            if let Some(Msg::Kick { reason }) = msgs.iter().find(|m| matches!(m, Msg::Kick { .. })) {
                break reason.clone();
            }
            assert!(start.elapsed() < Duration::from_secs(10), "bad login never kicked");
            std::thread::sleep(Duration::from_millis(4));
        };
        assert_eq!(kicked, "Wrong password.");
        assert!(host.peers.is_empty());

        let mut conn = Conn::connect(&format!("127.0.0.1:{port}")).unwrap();
        conn.send(&Msg::Hello { protocol: PROTOCOL, name: "Clienty".into() });
        conn.flush();
        let start = Instant::now();
        let (welcome, leftover) = loop {
            host.update(0.016, &idle());
            let mut msgs = conn.poll();
            if let Some(Msg::Challenge { nonce, .. }) = msgs.iter().find(|m| matches!(m, Msg::Challenge { .. })) {
                conn.send(&Msg::Auth { proof: auth_proof(nonce, "hunter2") });
                conn.flush();
            }
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

        // An edit far outside the player's reach is refused and corrected.
        let (fx, fy, fz) = (sx + 20, sy, sz);
        assert!(client.world.is_loaded(fx, fz) && host.world.is_loaded(fx, fz));
        let before = host.world.get(fx, fy, fz);
        client.world.set_remote(fx, fy, fz, BRICK);
        client.net_send_msg(Msg::Blocks(vec![(fx, fy, fz, BRICK)]));
        assert!(pump(&mut host, &mut client, |_, c| c.world.get(fx, fy, fz) == before));
        assert_eq!(host.world.get(fx, fy, fz), before);

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

    /// Log in over a real socket and return the client's game.
    fn join(host: &mut Game, port: u16, name: &str, password: &str) -> Result<Game, String> {
        let mut conn = Conn::connect(&format!("127.0.0.1:{port}")).map_err(|e| e.to_string())?;
        conn.send(&Msg::Hello { protocol: PROTOCOL, name: name.into() });
        conn.flush();
        let start = Instant::now();
        let (welcome, leftover) = loop {
            host.update(0.016, &idle());
            let mut msgs = conn.poll();
            if let Some(Msg::Kick { reason }) = msgs.iter().find(|m| matches!(m, Msg::Kick { .. })) {
                return Err(reason.clone());
            }
            if let Some(Msg::Challenge { nonce, .. }) = msgs.iter().find(|m| matches!(m, Msg::Challenge { .. })) {
                conn.send(&Msg::Auth { proof: auth_proof(nonce, password) });
                conn.flush();
            }
            if let Some(i) = msgs.iter().position(|m| matches!(m, Msg::Welcome { .. })) {
                let rest = msgs.split_off(i + 1);
                break (msgs.pop().unwrap(), rest);
            }
            if conn.closed.is_some() || start.elapsed() > Duration::from_secs(10) {
                return Err(conn.closed.clone().unwrap_or_else(|| "no welcome".into()));
            }
            std::thread::sleep(Duration::from_millis(4));
        };
        let Msg::Welcome { id, seed, time, creative, spawn } = welcome else { unreachable!() };
        let mut client = Game::new_client(id, seed, time, creative, spawn, conn, name, leftover);
        load_around(&mut client, spawn);
        Ok(client)
    }

    #[test]
    fn hosts_enforce_the_rules() {
        let mut host = Game::new(778, false, false);
        let spawn = host.spawn;
        load_around(&mut host, spawn);
        let port = host.open_lan("Hosty", None).unwrap();
        let mut client = join(&mut host, port, "Sneaky", "").unwrap();
        let id = client.my_id;
        assert!(pump(&mut host, &mut client, |h, _| h.peers.contains_key(&id)));
        client.player.body.pos = spawn;
        assert!(pump(&mut host, &mut client, |h, _| h.peers[&id].target.distance(spawn) < 1.0));

        // The rules themselves.
        let (x, z) = (spawn.x.floor() as i32 + 2, spawn.z.floor() as i32);
        let ground = host.world.surface_y(x, z);
        assert!(host.edit_allowed(x, ground + 1, z, AIR, BRICK), "placing into air");
        assert!(host.edit_allowed(x, ground, z, host.world.get(x, ground, z), AIR), "breaking");
        assert!(!host.edit_allowed(x, ground + 1, z, AIR, BEDROCK), "bedrock isn't placeable");
        assert!(!host.edit_allowed(x, 0, z, BEDROCK, AIR), "bedrock isn't breakable");
        assert!(!host.edit_allowed(x, ground, z, STONE, DIAMOND_ORE), "no alchemy");
        assert!(host.edit_allowed(x, ground, z, GRASS, FARMLAND), "tilling");
        assert!(!host.edit_allowed(x, ground + 1, z, AIR, WHEAT_0), "crops need farmland");
        assert!(!host.edit_allowed(x, ground + 1, z, AIR, WHEAT_0 + 3), "and grow on the host, not the client");

        // A modified client trying to turn a block into diamond ore is corrected.
        let (tx, ty, tz) = (x, ground, z);
        let before = host.world.get(tx, ty, tz);
        client.world.set_remote(tx, ty, tz, DIAMOND_ORE);
        client.net_send_msg(Msg::Blocks(vec![(tx, ty, tz, DIAMOND_ORE)]));
        assert!(pump(&mut host, &mut client, |_, c| c.world.get(tx, ty, tz) == before));
        assert_eq!(host.world.get(tx, ty, tz), before);

        // Teleporting across the map is refused and the client is put back.
        // (NaN positions never get this far: the decoder drops the connection.)
        client.net_send_msg(Msg::PlayerState { id, pos: spawn + Vec3::new(500.0, 0.0, 0.0), yaw: 0.0, pitch: 0.0, flags: 0 });
        for _ in 0..20 {
            host.update(0.016, &idle());
            std::thread::sleep(Duration::from_millis(4));
        }
        assert!(host.peers[&id].target.is_finite() && host.peers[&id].target.distance(spawn) < 5.0);

        // Chat floods are throttled.
        for i in 0..15 {
            client.net_send_msg(Msg::Chat { from: id, text: format!("spam {i}") });
        }
        client.net_send_msg(Msg::Chat { from: id, text: "\u{7}\n".into() });
        let _ = pump(&mut host, &mut client, |_, c| c.messages.iter().any(|m| m.0.contains("too fast")));
        let got = host.messages.iter().filter(|m| m.0.contains("spam")).count();
        assert!((1..=6).contains(&got), "{got} spam lines got through");

        // Soil lives on the host: a probe from the client is answered in chat.
        let (fx, fy, fz) = (x, ground, z + 1);
        host.world.set(fx, fy, fz, FARMLAND);
        client.world.set_remote(fx, fy, fz, FARMLAND);
        client.net_send_msg(Msg::Interact { x: fx, y: fy, z: fz, item: SOIL_PROBE });
        assert!(pump(&mut host, &mut client, |_, c| c.messages.iter().any(|m| m.0.contains("N 60 P 60 K 60"))));

        // Messages a client has no business sending earn strikes, then a kick.
        for _ in 0..=MAX_STRIKES {
            client.net_send_msg(Msg::Time(0.5));
        }
        assert!(pump(&mut host, &mut client, |h, c| !h.peers.contains_key(&id) && c.net_error.is_some()));
    }

    #[test]
    fn repeated_wrong_passwords_lock_you_out() {
        let mut host = Game::new(779, true, false);
        let spawn = host.spawn;
        load_around(&mut host, spawn);
        let port = host.open_lan("Hosty", Some("correct horse".into())).unwrap();
        for _ in 0..5 {
            assert_eq!(join(&mut host, port, "Guesser", "hunter2").err().as_deref(), Some("Wrong password."));
        }
        let locked = join(&mut host, port, "Guesser", "correct horse").err().unwrap_or_default();
        assert!(locked.contains("Too many wrong passwords"), "{locked}");
    }

}
