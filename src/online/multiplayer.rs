//! LAN multiplayer glue. The host runs the real simulation (mobs, TNT, time,
//! explosions); clients simulate only their own player and mirror the rest.
//! Block edits go both ways and the host re-broadcasts them to everyone.

use crate::block::*;
use crate::entity::{Arrow, Mob, MobKind, PrimedTnt};
use crate::dims::Dim;
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
    /// What they're wearing (see `Inventory::armor_look`).
    pub armor: u16,
    /// Their armour trims (see `trims::look`).
    pub trims: u32,
    pub anim: f32,
    /// Game clock time each rate-limited action was last allowed (see `peer_rate_ok`).
    last: HashMap<&'static str, f32>,
    /// Chat allowance: refills at one message a second, up to five.
    chat_tokens: f32,
    /// Messages that made no sense (kicked after too many).
    strikes: u32,
    /// What the host knows they own (see ledger.rs).
    pub ledger: crate::ledger::Ledger,
    /// Their last report of their inventory, health and hunger (see players.rs).
    pub report: Option<crate::players::Report>,
    /// How they look (see nametags.rs).
    pub skin: u8,
    /// Their game mode (the host decides; see modes.rs).
    pub mode: crate::modes::GameMode,
    /// Their latest statistics (see stats.rs), kept with the world.
    pub stats: Vec<u8>,
    /// Where they were at the last footstep check (see deepdark.rs).
    pub last_step: Vec3,
    /// Which dimension they're in (see realms.rs).
    pub dim: crate::dims::Dim,
}

impl Peer {
    pub(crate) fn new(name: String, pos: Vec3) -> Peer {
        Peer { name, pos, target: pos, yaw: 0.0, pitch: 0.0, flags: 0, armor: 0, trims: 0, anim: 0.0, last: HashMap::new(), chat_tokens: 5.0, strikes: 0, ledger: Default::default(), report: None, skin: 0, mode: crate::modes::GameMode::Survival, stats: Vec::new(), last_step: pos, dim: Default::default() }
    }
    pub fn alive(&self) -> bool {
        self.flags & (FLAG_DEAD | FLAG_GHOST) == 0
    }
    pub fn intersects_block(&self, b: IVec3) -> bool {
        let (min, max) = (self.target - Vec3::new(0.3, 0.0, 0.3), self.target + Vec3::new(0.3, 1.8, 0.3));
        let (bmin, bmax) = (b.as_vec3(), b.as_vec3() + Vec3::ONE);
        self.alive() && min.x < bmax.x && max.x > bmin.x && min.y < bmax.y && max.y > bmin.y && min.z < bmax.z && max.z > bmin.z
    }
}

/// Sounds the host forwards to clients; everything else is produced locally.
fn forwarded(s: Sfx) -> bool {
    matches!(s, Sfx::Note(..) | Sfx::Oink | Sfx::Groan | Sfx::Hiss | Sfx::MobHurt | Sfx::Baa | Sfx::Warp | Sfx::Cluck | Sfx::Moo | Sfx::Rattle | Sfx::Skitter | Sfx::Bloop | Sfx::Twang | Sfx::Thunk | Sfx::Gust | Sfx::Bleat | Sfx::Firework | Sfx::Horn | Sfx::Thud)
}

pub fn sanitize_name(name: &str) -> String {
    let n: String = name.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-').take(16).collect();
    // Nobody gets to be "Server" (script messages and the console use that name).
    let reserved = ["server", "host", "console", "admin", "system"];
    if n.is_empty() || reserved.contains(&n.to_ascii_lowercase().as_str()) { "Stove".into() } else { n }
}

/// How far a player can reach to break, place, hit or poke things (a little
/// more than the local limit, to allow for lag).
pub(crate) const REACH: f32 = 10.0;
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
    pub fn new_client(id: u32, seed: u32, worldgen: u32, time: f32, creative: bool, spawn: Vec3, conn: Conn, name: &str, leftover: Vec<Msg>) -> Game {
        let mut g = Game::new_with(seed, creative, false, crate::world::GenOptions::unpack(worldgen));
        g.time = time;
        g.spawn = spawn;
        g.player = Player::new(spawn);
        g.my_id = id;
        g.player_name = sanitize_name(name);
        g.world.log_edits = true;
        g.world.structure_loot = false;
        g.world.simulate_liquids = false;
        g.net = Some(Net::Client(conn));
        g.pending_msgs = leftover;
        g.msg("Connected! Say hi with T.");
        g
    }

    pub fn player_count(&self) -> usize {
        1 + self.all_peers().count()
    }

    pub fn peer_name(&self, id: u32) -> String {
        if id == self.my_id && !self.dedicated {
            return self.player_name.clone();
        }
        if id == 0 && !self.peers.contains_key(&0) {
            return "Server".into();
        }
        self.peer_ref(id).map(|p| p.name.clone()).unwrap_or_else(|| format!("Player{id}"))
    }

    /// To the host, or (the host) to everyone in this dimension.
    pub fn net_send_msg(&mut self, m: Msg) {
        match &mut self.net {
            Some(Net::Client(c)) => c.send(&m),
            Some(Net::Host(_)) => self.net_broadcast(m),
            None => {}
        }
    }

    /// The host: to everyone in the active dimension (what happens in a world
    /// only matters to the people in it; see realms.rs).
    pub fn net_broadcast(&mut self, m: Msg) {
        let here: Vec<u32> = self.peers.keys().copied().collect();
        if let Some(Net::Host(s)) = &mut self.net {
            s.broadcast_where(&m, |id| here.contains(&id));
        }
    }

    /// The host: to everyone, wherever they are (chat, the time, who's here).
    pub fn net_broadcast_all(&mut self, m: Msg) {
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
        // Their own game mode decides (not the host's): creative and spectating players can't be hurt.
        if self.peer_free(id) {
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
        // Waypoints are your own business.
        if crate::qol::is_command(&text) {
            for l in self.waypoint_command(&text) {
                self.msg(l);
            }
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
            match self.admin_command(crate::admin::Caller::Host, &text) {
                Some(lines) => {
                    for l in lines {
                        self.msg(l);
                    }
                }
                None => self.msg(format!("Unknown command {}. Try /help, or commands from script mods.", text.split_whitespace().next().unwrap_or(""))),
            }
            return;
        }
        self.msg(format!("<{me}> {text}"));
        let m = Msg::Chat { from: self.my_id, text };
        self.net_broadcast_all(m);
    }

    /// A private line to one remote player, or to everyone with `None`.
    pub fn system_message(&mut self, to: Option<u32>, text: &str) {
        let m = Msg::Chat { from: SYSTEM, text: text.into() };
        match to {
            Some(id) => self.net_send_to(id, m),
            None => self.net_broadcast_all(m),
        }
    }

    pub fn peer_by_name(&self, name: &str) -> Option<u32> {
        self.all_peers().find(|(_, p)| p.name.eq_ignore_ascii_case(name)).map(|(&id, _)| id)
    }

    pub fn disconnect(&mut self) {
        // One last report, so the host remembers exactly how we left.
        if self.is_client() {
            let m = self.report_msg();
            self.net_send_msg(m);
        }
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
        for p in self.peers.values_mut().chain(self.parked.values_mut().flat_map(|r| r.peers.values_mut())) {
            let before = p.pos;
            let k = (dt * 12.0).min(1.0);
            p.pos += (p.target - p.pos) * k;
            if p.pos.distance(p.target) > 8.0 {
                p.pos = p.target;
            }
            let moved = Vec3::new(p.pos.x - before.x, 0.0, p.pos.z - before.z).length();
            crate::entity::step_anim(&mut p.anim, moved / dt.max(1e-4), dt, 2.4);
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
                    if let Some(p) = self.peers.get_mut(&c.id).or_else(|| self.parked.values_mut().find_map(|r| r.peers.get_mut(&c.id))) {
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
            }
        }
        // Everything that arrived first (a leaving player's last words included),
        // then the goodbyes.
        for (from, m) in inbox {
            if self.is_host() {
                self.host_handle(from, m);
            } else {
                self.client_handle(m);
            }
        }
        // (Including anyone kicked just now.)
        if let Some(Net::Host(s)) = &mut self.net {
            for c in s.clients.iter().filter(|c| c.conn.closed.is_some() && c.joined) {
                left.push((c.id, c.name.clone(), c.conn.closed.clone().unwrap_or_default()));
            }
            s.reap();
        }
        for (id, name, why) in left {
            self.remember_peer(id);
            if let Some(d) = self.peer_dim(id) {
                self.in_realm(d, |g| {
                    g.peers.remove(&id);
                    g.forget_viewer(id);
                });
            }
            self.net_broadcast_all(Msg::PlayerLeave { id });
            // Ordinary goodbyes stay short; anything odd is worth a line in the log.
            if why == "connection closed" || why.starts_with("kicked") {
                self.msg(format!("{name} left the game"));
            } else {
                self.msg(format!("{name} left the game ({why})"));
            }
            self.fire("on_player_leave", vec![name.into()]);
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
                    if !self.admin.admits(&sanitize_name(&name)) {
                        let who = server.get(from).map(|c| c.conn.peer_addr()).unwrap_or_default();
                        server.kick(from, "You're not on this server's allow-list.");
                        self.msg(format!("Turned away {} from {who} (not on the allow-list)", sanitize_name(&name)));
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
                    if let Some(pw) = pw
                        && !proof_matches(&proof, &auth_proof(&nonce, &pw)) {
                            let who = c.conn.peer_addr();
                            let locked = server.record_failure(from);
                            server.kick(from, "Wrong password.");
                            self.msg(format!("Rejected a login from {who} (wrong password)"));
                            if locked {
                                self.msg(format!("Locked out {who} for 10 minutes after repeated wrong passwords"));
                            }
                            return;
                        }
                    self.complete_join(from);
                }
                _ => {}
            }
            return;
        }
        // Whatever they do happens where they are.
        let Some(dim) = self.peer_dim(from) else { return };
        self.in_realm(dim, |g| g.host_handle_joined(from, m));
    }

    fn complete_join(&mut self, from: u32) {
        let name = match &mut self.net {
            Some(Net::Host(server)) => server.get(from).map(|c| c.name.clone()),
            _ => None,
        };
        let Some(mut name) = name else { return };
        {
            let taken = |n: &str| n == self.player_name || self.all_peers().any(|(_, p)| p.name == n);
            if taken(&name) {
                name = format!("{}{}", name.chars().take(13).collect::<String>(), from);
            }
            let welcome = Msg::Welcome { id: from, seed: self.world.seed(), time: self.time, creative: self.default_creative, spawn: self.spawn, keep_inventory: self.rules.keep_inventory, worldgen: self.world.generator.opts.pack() };
            let mods: Vec<Msg> = self
                .world
                .mods
                .iter()
                .map(|(&(cx, cz), m)| Msg::Mods { cx, cz, entries: m.iter().map(|(&i, &b)| (i, b)).collect() })
                .collect();
            let mut roster = Vec::new();
            if !self.away() {
                roster.push(Msg::PlayerJoin { id: self.my_id, name: self.player_name.clone() });
            }
            roster.extend(self.all_peers().map(|(&id, p)| Msg::PlayerJoin { id, name: p.name.clone() }));
            // How everyone looks, and what the mobs are called.
            if !self.away() {
                roster.push(Msg::PlayerSkin { id: self.my_id, skin: self.skin });
            }
            roster.extend(self.all_peers().map(|(&id, p)| Msg::PlayerSkin { id, skin: p.skin }));
            // Who's somewhere else (so they aren't drawn here).
            if !self.dedicated && self.dim != Dim::Over {
                roster.push(Msg::Dimension { id: self.my_id, dim: self.dim.index(), x: 0.0, y: 0.0, z: 0.0 });
            }
            roster.extend(self.all_peers().filter(|(_, p)| p.dim != Dim::Over).map(|(&id, p)| Msg::Dimension { id, dim: p.dim.index(), x: p.target.x, y: p.target.y, z: p.target.z }));
            roster.extend(self.mob_names.iter().map(|(&mob, name)| Msg::MobName { mob, name: name.clone() }));
            roster.push(self.rules_msg());
            roster.push(Msg::Weather { kind: self.weather.kind.index() });
            // Words on signs and things in frames.
            roster.extend(self.world.signs.iter().map(|(p, l)| Msg::SignText { x: p.x, y: p.y, z: p.z, lines: l.to_vec() }));
            roster.extend(self.world.sign_styles.iter().map(|(p, &style)| Msg::SignStyle { x: p.x, y: p.y, z: p.z, style, item: AIR }));
            roster.extend(self.world.frames.iter().map(|(p, &(item, wear))| Msg::FrameItem { x: p.x, y: p.y, z: p.z, item, wear }));
            roster.extend(self.banners.iter().map(|(p, &(design, facing))| Msg::Banner { x: p.x, y: p.y, z: p.z, design, facing, up: true }));
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
            if let Some(p) = self.peers.get_mut(&from) {
                p.mode = if self.default_creative { crate::modes::GameMode::Creative } else { crate::modes::GameMode::Survival };
            }
            self.msg(format!("{name} joined the game"));
            self.welcome_back(from, &name);
            self.fire("on_player_join", vec![name.clone().into()]);
        }
    }

    fn rules_msg(&self) -> Msg {
        let r = self.rules;
        Msg::Rules { keep_inventory: r.keep_inventory, difficulty: r.difficulty.index(), daylight_cycle: r.daylight_cycle, weather_cycle: r.weather_cycle, hardcore: r.hardcore, seasons: r.seasons, border: r.border }
    }

    /// Change the world's rules (the owner's World Settings) and tell everyone.
    pub fn set_rules(&mut self, rules: crate::rules::WorldRules) {
        if self.is_client() {
            return;
        }
        self.rules = rules;
        let m = self.rules_msg();
        self.net_broadcast_all(m);
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
                        let (hook, key) = if id == AIR || is_liquid(id) { ("on_block_break", old) } else { ("on_block_place", id) };
                        let args = vec![who.into(), (x as rhai::INT).into(), (y as rhai::INT).into(), (z as rhai::INT).into(), reg().key_of(key).into()];
                        if !self.fire(hook, args) {
                            corrections.push((x, y, z, old));
                            continue;
                        }
                    }
                    // And only with items they really have, at the speed their tools allow
                    // (last, because a break that goes through drops its items).
                    // A Hollow Box's contents travel in its item's wear (see boxes.rs).
                    let packed = if id == HOLLOW_BOX { crate::boxes::box_wear(self.verified_ench(from)) } else { 0 };
                    if old != id && !self.ledger_edit(from, IVec3::new(x, y, z), old, id) {
                        corrections.push((x, y, z, old));
                        continue;
                    }
                    // A broken chest or furnace spills what was inside (and a frame what it held).
                    if crate::containers::is_container(old) && !crate::containers::is_container(id) {
                        self.spill_container(IVec3::new(x, y, z));
                    }
                    if crate::decor::is_frame(old) && !crate::decor::is_frame(id) {
                        self.spill_frame(IVec3::new(x, y, z));
                        self.spill_lectern(IVec3::new(x, y, z));
                        self.spill_banner(IVec3::new(x, y, z));
                    }
                    // Hives, pots and sculk notice (and the Deep Dark hears it).
                    if old != id && crate::ledger::is_break(old, id) {
                        self.block_gone(IVec3::new(x, y, z), old);
                    } else if old != id {
                        self.vibrate(center, Some(from));
                    }
                    // Logged, so the host re-broadcasts it to everyone.
                    self.world.set(x, y, z, id);
                    if crate::masonry::is_powder(id) {
                        self.harden_powder(IVec3::new(x, y, z));
                    }
                    if packed != 0 {
                        self.unpack_box(IVec3::new(x, y, z), packed);
                    }
                    if matches!(id, PUMPKIN | JACK) && old != id {
                        let who = self.peer_name(from);
                        self.try_build_copper_golem(IVec3::new(x, y, z), &crate::players::record_key(&who));
                    }
                    if id == ROPE && old != id {
                        self.unroll(IVec3::new(x, y, z));
                    }
                    if id == CHARRED_SKULL && old != id {
                        let who = self.peer_name(from);
                        self.try_build_wilter(IVec3::new(x, y, z), &crate::players::record_key(&who));
                    }
                    if id == BANNER && old != BANNER {
                        let design = if self.verified_held(from) == BANNER { self.verified_ench(from) } else { 0 };
                        let facing = self.peers.get(&from).map(|p| crate::banners::facing_from_yaw(p.yaw)).unwrap_or(0);
                        self.put_up_banner(IVec3::new(x, y, z), design, facing);
                    }
                    // Half a door takes the other half with it.
                    if is_door(old) && !is_door(id) {
                        self.remove_door_partner(IVec3::new(x, y, z), old);
                    }
                    if !quiet && old != id {
                        self.block_change_feedback(IVec3::new(x, y, z), old, id);
                    }
                }
                if let Some(Net::Host(s)) = &mut self.net
                    && let Some(c) = s.get(from) {
                        c.edit_budget = budget;
                        if !corrections.is_empty() {
                            c.conn.send(&Msg::Blocks(corrections));
                        }
                    }
            }
            Msg::PlayerState { pos, yaw, pitch, flags, held, held_ench, armor, trims, .. } => {
                self.set_peer_held(from, held, held_ench);
                let Some(p) = self.peers.get_mut(&from) else { return };
                // Just died: their experience spills (items come separately, see `drop_everything`).
                if p.flags & FLAG_DEAD == 0 && flags & FLAG_DEAD != 0 {
                    p.flags = flags;
                    self.peer_died(from);
                }
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
                p.armor = armor;
                p.trims = trims;
                self.relay(from, Msg::PlayerState { id: from, pos, yaw, pitch, flags, held, held_ench, armor, trims });
            }
            Msg::Attack { mob, dmg, .. } => {
                // Hits come from where the player actually is, within reach, at a human pace.
                let Some(eye) = self.peers.get(&from).map(|p| p.target + Vec3::Y * 1.6) else { return };
                if !self.peer_rate_ok(from, "attack", 0.2) || !dmg.is_finite() {
                    return;
                }
                // No harder than the weapon they really own (x1.5 for a falling crit).
                let sharpness = crate::enchant::level((self.verified_ench(from) as u32) << 16, crate::enchant::Enchant::Sharpness);
                // (Strength is drunk on their own machine; the host saw the bottle go.)
                let strength = self.strong.get(&from).map_or(0.0, |&(_, amplifier)| crate::potions::strength_bonus(amplifier));
                let held = self.verified_held(from);
                // (A Mace adds whatever their fall was worth; the host can't see falls, so it allows a long one.)
                let smash = if held == MACE { crate::combat::smash_bonus(40.0) } else if is_spear(held) { crate::combat::LUNGE_MAX } else { 0.0 };
                let dmg = dmg.clamp(0.0, (attack_damage_with(held, sharpness) + strength) * 1.5 + smash);
                if let Some(m) = self.mobs.iter_mut().find(|m| m.id == mob && (m.body.pos + Vec3::Y * m.body.height * 0.5).distance(eye) <= REACH) {
                    m.damage(dmg, eye);
                    m.last_attacker = from;
                    let (kind, pos) = (m.kind, m.body.pos);
                    self.sfx(Sfx::hurt_of(kind), Some(pos));
                    let held = self.verified_held(from);
                    self.host_wear(from, held, hit_wear(held));
                    if let Some(name) = self.peers.get(&from).map(|p| crate::players::record_key(&p.name)) {
                        self.sic_pets(&name, mob);
                    }
                    if let Some(i) = self.mobs.iter().position(|o| o.id == mob) {
                        self.creaking_hit(i);
                    }
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
                    match self.admin_command(crate::admin::Caller::Player(from), &text) {
                        Some(lines) => {
                            for l in lines {
                                self.system_message(Some(from), &l);
                            }
                        }
                        None => {
                            let cmd = text.split_whitespace().next().unwrap_or("").to_string();
                            self.system_message(Some(from), &format!("Unknown command {cmd}. Try /help, or commands from script mods."));
                        }
                    }
                    return;
                }
                self.msg(format!("<{who}> {text}"));
                self.relay_all(from, Msg::Chat { from, text });
            }
            Msg::Splash { item, at } => self.host_splash(from, item, at),
            Msg::RideMob { mob, pos, yaw, off } => self.host_ride_mob(from, mob, pos, yaw, off),
            Msg::MobName { mob, name } => {
                if self.peer_rate_ok(from, "name", 0.5) {
                    self.host_mob_name(from, mob, name);
                }
            }
            Msg::PlayerSkin { skin, .. } => {
                let skin = skin % crate::nametags::SKINS.len() as u8;
                if let Some(p) = self.peers.get_mut(&from) {
                    p.skin = skin;
                }
                self.relay_all(from, Msg::PlayerSkin { id: from, skin });
            }
            Msg::UseItem { item } => {
                if item == GLASS_BOTTLE && self.peer_rate_ok(from, "bottle", 0.1) {
                    self.host_fill_bottle(from);
                }
                if item == BUNDLE {
                    self.host_bundle_tip(from);
                    return;
                }
                if item == ROCKET && self.peer_rate_ok(from, "rocket", 0.25) {
                    self.host_rocket(from);
                    return;
                }
                if item == CROSSBOW && self.peer_rate_ok(from, "crossbow", 1.0) && self.peer_has(from, CROSSBOW) {
                    self.host_crossbow(from);
                    return;
                }
                if item == OMINOUS_BOTTLE && self.peer_rate_ok(from, "bottle", 1.0) {
                    if self.peer_take(from, OMINOUS_BOTTLE, 1) {
                        self.give_effect_to(from, crate::potions::Potion::BadOmen, crate::raids::OMEN_SECS);
                    }
                    return;
                }
                if item == GOAT_HORN && self.peer_rate_ok(from, "horn", 5.0) && self.peer_has(from, GOAT_HORN) {
                    if let Some(at) = self.peers.get(&from).map(|p| p.target + Vec3::Y * 1.6) {
                        self.sfx(Sfx::Horn, Some(at));
                    }
                    return;
                }
                if item == WIND_CHARGE && self.peer_rate_ok(from, "wind", 0.4) {
                    self.host_throw_wind_charge(from);
                    return;
                }
                // A thrown spear leaves their hands and flies from where they look.
                if is_spear(item) && self.peer_rate_ok(from, "spear", 0.6) && self.peer_has(from, item) {
                    let ench = if self.verified_held(from) == item { self.verified_ench(from) } else { 0 };
                    if let Some(p) = self.peers.get(&from) {
                        let dir = Vec3::new(p.yaw.sin() * p.pitch.cos(), p.pitch.sin(), -p.yaw.cos() * p.pitch.cos());
                        let eye = p.target + Vec3::Y * 1.6;
                        if self.peer_take(from, item, 1) {
                            self.throw_spear_from(eye + dir * 0.5, dir, from, item, (ench as u32) << 16);
                        }
                    }
                    return;
                }
                if valid_item(item) && self.peer_rate_ok(from, "use", 0.1) && self.peer_has(from, item) {
                    let who = self.peer_name(from);
                    self.fire("on_use_item", vec![who.into(), reg().key_of(item).into()]);
                    // Mod items that give things: the host hands them over (and notes it).
                    if let Some((actions, _)) = use_actions(item) {
                        for a in actions {
                            if let Action::Give(i, n) = a {
                                self.give_peer(from, *i, *n);
                            }
                        }
                    }
                }
            }
            Msg::BundleUse { tag, item, n, put } => self.host_bundle_use(from, tag, item, n, put),
            Msg::BookWrite { tag, title, pages, sign } => self.host_book_write(from, tag, title, pages, sign),
            Msg::BookAsk { tag, x, y, z } => self.host_book_ask(from, tag, IVec3::new(x, y, z)),
            Msg::LecternTake { x, y, z } => self.host_lectern_take(from, IVec3::new(x, y, z)),
            Msg::Loom { design, dye } => self.host_loom(from, design, dye),
            Msg::SortContainer { x, y, z } => self.host_sort(from, IVec3::new(x, y, z)),
            Msg::RegularAsk { mob } => self.host_regular_ask(from, mob),
            Msg::CampfirePut { x, y, z, item } => self.host_campfire_put(from, IVec3::new(x, y, z), item),
            Msg::Mend { points } => self.host_mend(from, points),
            Msg::FrostWalk => self.host_frost_walk(from),
            Msg::SignStyle { x, y, z, item, .. } => self.host_sign_style(from, IVec3::new(x, y, z), item),
            Msg::ChestUpgrade { x, y, z } => self.host_chest_upgrade(from, IVec3::new(x, y, z)),
            Msg::Interact { x, y, z, item } if self.world.get(x, y, z) == LECTERN && crate::books::is_book(item) => {
                let p = IVec3::new(x, y, z);
                if self.peers.get(&from).is_some_and(|q| q.target.distance(p.as_vec3()) < 8.0) {
                    self.host_lectern_put(from, p, item);
                }
            }
            Msg::Shoot { pos, dir } => {
                // Only from roughly where they are, in a real direction, at a bow's pace.
                let Some(p) = self.peers.get(&from) else { return };
                let near = pos.is_finite() && pos.distance(p.target + Vec3::Y * 1.6) < 3.0;
                let armed = self.peer_has(from, BOW);
                if near && armed && dir.is_finite() && dir.length() > 0.5 && self.peer_rate_ok(from, "shoot", 0.4) && self.peer_take(from, ARROW, 1) {
                    self.spawn_arrow(pos, dir.normalize() * Arrow::SPEED * 1.2, Some(from));
                    self.host_wear(from, BOW, 1);
                }
            }
            Msg::Interact { x, y, z, item: TRIAL_KEY | OMINOUS_TRIAL_KEY } if matches!(self.world.get(x, y, z), VAULT | VAULT_OMINOUS) => {
                let p = IVec3::new(x, y, z);
                let near = self.peers.get(&from).is_some_and(|q| (q.target + Vec3::Y * 1.6).distance(p.as_vec3() + Vec3::splat(0.5)) <= REACH);
                if near && self.peer_rate_ok(from, "vault", 0.5) {
                    self.host_use_vault(from, p);
                }
            }
            Msg::Interact { x, y, z, item: BONE_DUST } if crate::wilds::is_fungus(self.world.get(x, y, z)) => {
                let p = IVec3::new(x, y, z);
                let near = self.peers.get(&from).is_some_and(|q| (q.target + Vec3::Y * 1.6).distance(p.as_vec3() + Vec3::splat(0.5)) <= REACH);
                if near && self.peer_rate_ok(from, "interact", 0.2) && self.peer_take(from, BONE_DUST, 1) {
                    let who = self.peers.get(&from).map(|q| crate::players::record_key(&q.name)).unwrap_or_default();
                    self.grow_fungus(p, &who);
                }
            }
            Msg::Interact { x, y, z, item: BONE_DUST } if self.world.get(x, y, z) == MOSS_BLOCK => {
                let p = IVec3::new(x, y, z);
                let near = self.peers.get(&from).is_some_and(|q| (q.target + Vec3::Y * 1.6).distance(p.as_vec3() + Vec3::splat(0.5)) <= REACH);
                if near && self.peer_rate_ok(from, "interact", 0.2) && self.peer_take(from, BONE_DUST, 1) {
                    self.grow_moss(p);
                }
            }
            Msg::Interact { x, y, z, .. } if self.world.get(x, y, z) == BELL => {
                let p = IVec3::new(x, y, z);
                let near = self.peers.get(&from).is_some_and(|q| (q.target + Vec3::Y * 1.6).distance(p.as_vec3() + Vec3::splat(0.5)) <= REACH);
                if near && self.peer_rate_ok(from, "bell", 0.5) {
                    self.ring_bell_at(p.as_vec3() + Vec3::splat(0.5));
                }
            }
            Msg::Interact { x, y, z, item } if crate::bees::is_hive(self.world.get(x, y, z)) || self.world.get(x, y, z) == RESTORATION_BENCH => {
                let p = IVec3::new(x, y, z);
                let near = self.peers.get(&from).map(|q| (q.target + Vec3::Y * 1.6).distance(p.as_vec3() + Vec3::splat(0.5)) <= REACH).unwrap_or(false);
                if near && self.peer_has(from, item) && self.peer_rate_ok(from, "interact", 0.2) {
                    if self.world.get_v(p) == RESTORATION_BENCH {
                        self.host_restore(from, p, item);
                    } else {
                        self.host_hive_use(from, p, item);
                    }
                }
            }
            Msg::Excavate { x, y, z, cracks } => self.host_excavate(from, IVec3::new(x, y, z), cracks),
            Msg::Respawn => {
                if self.realm_dim() != Dim::Over && self.peer_rate_ok(from, "respawn", 1.0) {
                    let to = self.spawn;
                    self.move_peer(from, Dim::Over, to);
                }
            }
            Msg::Died { cause } => {
                // Told to everyone, like Minecraft's death messages.
                if self.peer_rate_ok(from, "died", 1.0) {
                    let cause: String = cause.chars().filter(|c| !c.is_control()).take(120).collect();
                    let line = format!("{} {cause}", self.peer_name(from));
                    self.msg(line.clone());
                    self.system_message(None, &line);
                }
            }
            Msg::Smith { x, y, z, grind, a, a_ench, b, b_ench } => self.host_smith(from, IVec3::new(x, y, z), grind, a, a_ench, b, b_ench),
            Msg::Deflect { at, dir } => {
                if self.peer_rate_ok(from, "deflect", 0.2) {
                    self.host_deflect(from, at, dir);
                }
            }
            Msg::Interact { x, y, z, item } => {
                let at = Vec3::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5);
                let near = self.peers.get(&from).map(|p| (p.target + Vec3::Y * 1.6).distance(at) <= REACH).unwrap_or(false);
                let tool = matches!(item, BONE_DUST | COMPOST | WOOD_ASH | SOIL_PROBE);
                // The probe is kept; fertiliser is used up (and must really be theirs).
                if near
                    && tool
                    && self.peer_has(from, item)
                    && self.peer_rate_ok(from, "interact", 0.2)
                    && (item == SOIL_PROBE || self.peer_take(from, item, 1))
                    && let Some(reply) = self.farm_interact(IVec3::new(x, y, z), item)
                {
                    self.system_message(Some(from), &reply);
                }
            }
            Msg::Catch { pos, bait } => self.host_catch(from, pos, bait),
            Msg::Craft { recipe, times } => self.host_craft(from, recipe, times),
            Msg::Consume { item, n } => self.host_consume(from, item, n),
            Msg::OpenContainer { x, y, z } => {
                if self.peer_rate_ok(from, "open", 0.1) {
                    self.host_open(from, IVec3::new(x, y, z));
                }
            }
            Msg::CloseContainer { x, y, z } => self.host_close(from, IVec3::new(x, y, z)),
            Msg::Pickup { id, room } => self.host_pickup(from, id, room),
            Msg::Stats { data } => {
                if data.len() <= crate::players::MAX_STATS
                    && let Some(p) = self.peers.get_mut(&from)
                {
                    p.stats = data;
                }
            }
            Msg::PlayerData { slots, health, food, saturation } => {
                if self.peer_rate_ok(from, "report", 1.0) {
                    self.host_report(from, slots, health, food, saturation);
                }
            }
            Msg::Repair { x, y, z, item, material, used, combine, ench, other_ench } => {
                if self.peer_rate_ok(from, "repair", 0.2) {
                    self.host_repair(from, IVec3::new(x, y, z), item, material, used, combine, ench, other_ench);
                }
            }
            Msg::MobInteract { mob, item } => {
                if self.peer_rate_ok(from, "mob", 0.15) {
                    self.host_mob_interact(from, mob, item);
                }
            }
            Msg::Trade { mob, index } => {
                if self.peer_rate_ok(from, "trade", 0.1) {
                    self.host_trade(from, mob, index);
                }
            }
            Msg::UsePortal { x, y, z } => {
                if self.peer_rate_ok(from, "portal", 2.0) {
                    self.host_use_portal(from, IVec3::new(x, y, z));
                }
            }
            Msg::VehicleUse { id, action } => {
                if self.peer_rate_ok(from, "vehicle", 0.05) {
                    self.host_vehicle_use(from, id, action);
                }
            }
            Msg::Ride { id, pos, yaw } => self.host_ride(from, id, pos, yaw),
            Msg::SignText { x, y, z, lines } => {
                if self.peer_rate_ok(from, "sign", 0.2) {
                    self.host_sign(from, IVec3::new(x, y, z), lines);
                }
            }
            Msg::FrameUse { x, y, z, item, wear, put } => {
                if self.peer_rate_ok(from, "frame", 0.05) {
                    self.host_frame_use(from, IVec3::new(x, y, z), item, wear, put);
                }
            }
            Msg::PlaceVehicle { kind, pos, yaw } => {
                if self.peer_rate_ok(from, "place_vehicle", 0.3) {
                    self.host_place_vehicle(from, kind, pos, yaw);
                }
            }
            Msg::Enchant { x, y, z, item, choice } => {
                if self.peer_rate_ok(from, "enchant", 0.2) {
                    self.host_enchant(from, IVec3::new(x, y, z), item, choice);
                }
            }
            Msg::DropItem { item, n, wear, scatter } => {
                // Dying drops a whole inventory at once; throwing is one at a time.
                if scatter || self.peer_rate_ok(from, "throw", 0.05) {
                    self.host_throw(from, item, n, wear, scatter);
                }
            }
            Msg::ContainerMove { x, y, z, slot, item, n, put, wear } => self.host_container_move(from, IVec3::new(x, y, z), slot as usize, item, n, put, wear),
            Msg::InventoryCheck { items } => {
                if self.peer_rate_ok(from, "check", 1.0) {
                    self.host_inventory_check(from, items);
                }
            }
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
        // Crops only appear on farmland, and only as seedlings: the host grows them.
        if let Some((_, stage)) = Crop::of_block(new) {
            let on_farmland = is_farmland(self.world.get(x, y - 1, z));
            return replaceable(old) && on_farmland && (stage == 0 || (self.creative && block(new).creative));
        }
        // Doors open and close; a door's top half only goes on its own bottom half.
        if let (Some((f, o, t)), Some((nf, no, nt))) = (door_state(old), door_state(new)) {
            return f == nf && t == nt && o != no && door_base(old) == door_base(new);
        }
        if let (Some((f, o, true)), Some(base)) = (door_state(new), door_base(new)) {
            return replaceable(old) && self.world.get(x, y - 1, z) == door_of(base, f, o, false);
        }
        // Fire only where it can burn (and blue only on soul ground).
        if new == FIRE || new == SOUL_FIRE {
            let p = IVec3::new(x, y, z);
            return crate::fire::can_burn_at(&self.world, p) && (new == FIRE) != crate::wilds::is_soul_ground(self.world.get_v(p - IVec3::Y));
        }
        // Portals light only inside a real obsidian frame.
        if matches!(new, PORTAL_X | PORTAL_Z) {
            let p = IVec3::new(x, y, z);
            return old == AIR && crate::scorch::portal_frame(&self.world, p, new == PORTAL_X).is_some();
        }
        // Gates and trapdoors swing; fences and panes may be placed at any join (the host reshapes them).
        if let (Some(a), Some(b)) = (crate::carpentry::family(old), crate::carpentry::family(new)) {
            return a == b && (crate::carpentry::is_gate(a) || a == TRAPDOOR_FIRST);
        }
        // Levers flip both ways, buttons only go in (the host lets them out).
        if matches!((old, new), (LEVER, LEVER_ON) | (LEVER_ON, LEVER) | (BUTTON, BUTTON_ON) | (CANDLE, CANDLE_LIT) | (CANDLE_LIT, CANDLE) | (CAVE_VINES_LIT, CAVE_VINES)) {
            return true;
        }
        // Beacons switch effect (and take a star, but never give one back).
        if crate::beacon::is_beacon(old) && crate::beacon::is_beacon(new) {
            return !crate::beacon::starred(old) || crate::beacon::starred(new);
        }
        // Note blocks retune; jukeboxes take a disc and give it back (the ledger checks the disc).
        if (crate::music::is_note_block(old) && crate::music::is_note_block(new)) || (crate::music::is_jukebox(old) && crate::music::is_jukebox(new)) {
            return true;
        }
        // Comparators switch mode (the host works out whether they're on).
        if crate::contraptions::is_comparator(old) && crate::contraptions::is_comparator(new) {
            return crate::contraptions::comparator_state(old).0 == crate::contraptions::comparator_state(new).0;
        }
        // Two slabs make a block.
        if slab_of(old).is_some() {
            return new == AIR || new == made_of(old);
        }
        match new {
            AIR => true,
            // Melting ice, or pouring out a bucket (the ledger checks they have one).
            WATER => old == ICE || replaceable(old) || crate::liquids::open(old),
            LAVA => replaceable(old) || crate::liquids::open(old),
            // Tilling, and trampling.
            n if is_farmland(n) => matches!(old, GRASS | DIRT | SNOW_GRASS),
            DIRT if is_farmland(old) => true,
            // Placing: only into an empty-ish cell, and only blocks a player could have.
            _ => replaceable(old) && placing_item(new).is_some(),
        }
    }

    /// The host: pass on to everyone else in this dimension.
    fn relay(&mut self, except: u32, m: Msg) {
        let here: Vec<u32> = self.peers.keys().copied().filter(|&id| id != except).collect();
        if let Some(Net::Host(s)) = &mut self.net {
            s.broadcast_where(&m, |id| here.contains(&id));
        }
    }

    /// ... and to everyone else anywhere.
    fn relay_all(&mut self, except: u32, m: Msg) {
        if let Some(Net::Host(s)) = &mut self.net {
            s.broadcast(&m, Some(except));
        }
    }

    fn client_handle(&mut self, m: Msg) {
        match m {
            Msg::Kick { reason } => self.net_error = Some(format!("Kicked: {reason}")),
            Msg::Dimension { id, dim, x, y, z } => {
                let Some(dim) = Dim::from_index(dim) else { return };
                let to = Vec3::new(x, y, z);
                if id == self.my_id {
                    if to.is_finite() {
                        self.client_change_dimension(dim, to);
                    }
                } else {
                    self.client_peer_dimension(id, dim);
                }
            }
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
                    if let Some(old) = self.world.set_remote(x, y, z, id)
                        && !quiet {
                            self.block_change_feedback(IVec3::new(x, y, z), old, id);
                        }
                }
            }
            Msg::PlayerJoin { id, name } => {
                if id != self.my_id {
                    // (They start in the Overworld; we're told if not.)
                    let mut p = Peer::new(name.clone(), self.spawn);
                    p.dim = Dim::Over;
                    self.in_realm(Dim::Over, |g| g.peers.insert(id, p));
                    // The roster sent on join arrives before our terrain is ready; only
                    // announce people who join afterwards.
                    if self.ready {
                        self.msg(format!("{name} joined the game"));
                    }
                }
            }
            Msg::PlayerLeave { id } => {
                if let Some(p) = self.remove_peer(id) {
                    self.msg(format!("{} left the game", p.name));
                }
            }
            Msg::PlayerState { id, pos, yaw, pitch, flags, armor, trims, .. } => {
                if let Some(p) = self.peers.get_mut(&id) {
                    p.target = pos;
                    p.yaw = yaw;
                    p.pitch = pitch;
                    p.flags = flags;
                    p.armor = armor;
                    p.trims = trims;
                }
            }
            Msg::Mobs { mobs, tnts, arrows, falling, fireballs } => {
                self.fireballs = fireballs.into_iter().map(|(pos, vel, big)| crate::fortress::Fireball { pos, vel, big, life: 1.0, shooter: 0, returned: None }).collect();
                self.falling = falling.into_iter().map(|(pos, vel, id)| crate::falling::FallingBlock { pos, vel, id, from: pos.y }).filter(|f| valid_block(f.id)).collect();
                self.sync_mobs(mobs, tnts, arrows)
            }
            Msg::HurtYou { dmg, cause, knock } => {
                // Whatever hit us came from the opposite way to the knock.
                let flat = Vec3::new(knock.x, 0.0, knock.z);
                let from = (flat.length() > 0.01).then(|| self.player.body.pos + Vec3::Y * 0.9 - flat.normalize() * 2.0);
                // (A gust of wind only pushes.)
                if dmg > 0.0 {
                    self.player.hurt = 0.0;
                    self.hurt_player_from(dmg, &cause, from, false);
                }
                self.player.body.vel += self.steadied(knock);
            }
            Msg::Give { item, n, wear } => {
                if valid_item(item) && n > 0 {
                    self.give_worn(item, n, wear);
                    self.inv_sync.note_host(item, n as i64);
                }
            }
            Msg::Inventory { items } => self.apply_inventory(items),
            Msg::Container { x, y, z, slots, burn, cook } => self.apply_container(IVec3::new(x, y, z), slots, burn, cook),
            Msg::Drops(list) => self.apply_drops(list),
            Msg::Orbs(list) => self.apply_orbs(list),
            Msg::Xp { points } => self.xp_update(points),
            Msg::Restore { pos, xp, slots, health, food, saturation } => self.apply_restore(pos, xp, slots, health, food, saturation),
            Msg::Rules { keep_inventory, difficulty, daylight_cycle, weather_cycle, hardcore, seasons, border } => {
                self.rules = crate::rules::WorldRules { keep_inventory, difficulty: crate::rules::Difficulty::from_index(difficulty), daylight_cycle, weather_cycle, hardcore, seasons, border };
            }
            Msg::Stats { data } => self.stats = crate::stats::Stats::decode(&data),
            Msg::GameMode { mode } => {
                let mode = crate::modes::GameMode::from_index(mode);
                self.set_mode(mode);
                self.msg(format!("Your game mode is now {}.", mode.name()));
            }
            Msg::Enchanted { item, ench, count } => self.apply_enchanted(item, ench, count),
            Msg::Vehicles(list) => self.apply_vehicles(list),
            Msg::SignText { x, y, z, lines } => {
                self.world.signs.insert(IVec3::new(x, y, z), crate::decor::clean_lines(&lines));
            }
            Msg::FrameItem { x, y, z, item, wear } => self.apply_frame(IVec3::new(x, y, z), item, wear),
            Msg::Weather { kind } => self.weather.kind = crate::weather::Weather::from_index(kind),
            Msg::Lightning { at } => {
                if at.is_finite() {
                    self.lightning_effects(at);
                }
            }
            Msg::Explosion { at, r } => {
                self.sfx(Sfx::Explode, Some(at));
                self.explosion_effects(at, r);
            }
            Msg::BundleState { old, new, contents } => self.bundle_state(old, new, contents),
            Msg::BookState { old, new, signed, open, title, author, pages } => self.book_state(old, new, signed, open, title, author, pages),
            Msg::Regular { mob, trades } => self.regular_state(mob, trades),
            Msg::SignStyle { x, y, z, style, .. } => self.apply_sign_style(IVec3::new(x, y, z), style),
            Msg::Banner { x, y, z, design, facing, up } => self.banner_msg(IVec3::new(x, y, z), design, facing, up),
            Msg::Firework { at, colour } => {
                if at.is_finite() {
                    self.firework_sparks(at, colour);
                }
            }
            Msg::Sound { sfx, at } => {
                if let Some(s) = Sfx::from_wire(sfx) {
                    self.sfx(s, Some(at));
                }
            }
            Msg::Time(t) => {
                // The day count rides in front of the time of day (see skies.rs).
                self.time = t.rem_euclid(1.0);
                self.day = t.floor().max(0.0) as u32;
            }
            Msg::MountMob { mob } => self.mount_mob(mob),
            Msg::MobName { mob, name } => {
                let name = crate::nametags::clean_name(&name);
                if !name.is_empty() {
                    self.mob_names.insert(mob, name);
                }
            }
            Msg::PlayerSkin { id, skin } => {
                if let Some(p) = self.peers.get_mut(&id) {
                    p.skin = skin % crate::nametags::SKINS.len() as u8;
                }
            }
            Msg::PotionEffect { item } => {
                if let Some((p, false)) = crate::potions::potion_of(item) {
                    self.apply_potion(p);
                }
            }
            Msg::BeaconEffect { item } => {
                if let Some((p, false)) = crate::potions::potion_of(item) {
                    self.beacon_effect(p);
                }
            }
            Msg::Chat { from: SYSTEM, text } => self.msg(text),
            Msg::Chat { from, text } => {
                let name = self.peer_name(from);
                self.msg(format!("<{name}> {text}"));
            }
            Msg::Darkness { secs } => self.darkness = self.darkness.max(secs.clamp(0.0, 30.0)),
            Msg::Raid { state, wave, waves, left } => {
                self.raid_hud = (state != 0).then_some((state, wave, waves, left, if state == 1 { 3.0 } else { 6.0 }));
            }
            Msg::TimedEffect { effect, secs, amplifier } => {
                if let Some(&p) = crate::potions::EFFECTS.get(effect as usize) {
                    if secs > 0.0 {
                        self.timed_effect_amplified(p, secs.min(3600.0), amplifier.min(3));
                    } else {
                        self.effects.retain(|active| active.kind != p);
                    }
                }
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
                    self.inv_sync.note_host(item, -(n as i64));
                }
            }
            Msg::Hello { .. } | Msg::Welcome { .. } | Msg::Attack { .. } | Msg::Ignite { .. } | Msg::Challenge { .. } | Msg::Auth { .. } | Msg::ModPack { .. } | Msg::UseItem { .. } | Msg::Shoot { .. } | Msg::Interact { .. } | Msg::Catch { .. } | Msg::Craft { .. } | Msg::Consume { .. } | Msg::InventoryCheck { .. } | Msg::OpenContainer { .. } | Msg::CloseContainer { .. } | Msg::ContainerMove { .. } | Msg::Pickup { .. } | Msg::DropItem { .. } | Msg::Repair { .. } | Msg::PlayerData { .. } | Msg::Enchant { .. } | Msg::MobInteract { .. } | Msg::Trade { .. } | Msg::UsePortal { .. } | Msg::VehicleUse { .. } | Msg::Ride { .. } | Msg::PlaceVehicle { .. } | Msg::FrameUse { .. } | Msg::Splash { .. } | Msg::RideMob { .. } | Msg::Excavate { .. } | Msg::Smith { .. } | Msg::Died { .. } | Msg::Deflect { .. } | Msg::BundleUse { .. } | Msg::BookWrite { .. } | Msg::BookAsk { .. } | Msg::LecternTake { .. } | Msg::Loom { .. } | Msg::SortContainer { .. } | Msg::RegularAsk { .. } | Msg::CampfirePut { .. } | Msg::Mend { .. } | Msg::FrostWalk | Msg::ChestUpgrade { .. } | Msg::Respawn => {}
        }
    }

    /// Mirror the host's mob list, keeping local copies so they can be smoothed.
    fn sync_mobs(&mut self, snaps: Vec<MobSnap>, tnts: Vec<(Vec3, f32)>, arrows: Vec<ArrowSnap>) {
        let mut next = Vec::with_capacity(snaps.len());
        let mut old: Vec<Mob> = std::mem::take(&mut self.mobs);
        for s in snaps {
            let Some(kind) = MobKind::from_index(s.kind) else { continue };
            // (A mob that changed kind, like a sheared Mushmooer, is made afresh.)
            let mut m = match old.iter().position(|m| m.id == s.id && m.kind == kind) {
                Some(i) => old.swap_remove(i),
                None => {
                    let mut m = Mob::new(kind, s.pos, &mut self.rng).with_size(s.size & 15);
                    m.id = s.id;
                    m
                }
            };
            m.net_pos = s.pos;
            m.yaw = s.yaw;
            m.variant = s.size >> 4;
            // Starers (and Weepers) reuse the fuse field for "angry".
            m.angry = matches!(kind, MobKind::Starer | MobKind::Weeper) && s.fuse > 0.0;
            m.fuse = if m.angry { 0.0 } else { s.fuse };
            if matches!(kind, MobKind::Hmmer | MobKind::Wanderer | MobKind::Sneaker | MobKind::CopperGolem) {
                m.seed = s.fuse as u32;
                m.fuse = 0.0;
            }
            if m.is_boss() {
                m.health = s.fuse;
                m.fuse = 0.0;
            }
            m.hurt = m.hurt.max(s.hurt);
            m.burning = s.burning;
            // Animals: just what's needed to draw them (and guess at shearing).
            if (s.flags & MOB_BABY != 0) != (m.baby > 0.0) {
                m.set_baby(if s.flags & MOB_BABY != 0 { 1.0 } else { 0.0 });
            }
            m.sheared = s.flags & MOB_SHEARED != 0;
            m.owner = (s.flags & MOB_TAMED != 0).then(String::new);
            m.sitting = s.flags & MOB_SITTING != 0;
            m.love = if s.flags & MOB_LOVE != 0 { 1.0 } else { 0.0 };
            m.saddled = s.flags & MOB_SADDLED != 0;
            if matches!(m.kind, MobKind::Soggy | MobKind::Pilferer | MobKind::Snout) {
                m.seed = (s.flags & MOB_ARMED != 0) as u32;
            }
            next.push(m);
        }
        self.mobs = next;
        self.tnts = tnts.into_iter().map(|(pos, fuse)| PrimedTnt { pos, fuse }).collect();
        self.arrows = arrows.into_iter().map(|a| Arrow::from_wire(a.pos, a.vel, a.appearance)).collect();
    }

    /// Client-side entity tick: particles plus smoothing the host's mobs.
    pub fn client_entities(&mut self, dt: f32) {
        // (A Camel's passenger lets the host move it.)
        let mounted = self.mounted.filter(|_| self.seat_no == 0);
        for m in self.mobs.iter_mut() {
            // The Galloper we're riding goes where we steer it.
            if Some(m.id) == mounted {
                continue;
            }
            let before = m.body.pos;
            let k = (dt * 12.0).min(1.0);
            m.body.pos += (m.net_pos - m.body.pos) * k;
            if m.body.pos.distance(m.net_pos) > 8.0 {
                m.body.pos = m.net_pos;
            }
            let moved = Vec3::new(m.body.pos.x - before.x, 0.0, m.body.pos.z - before.z).length();
            crate::entity::step_anim(&mut m.anim, moved / dt.max(1e-4), dt, 5.0);
            m.flutter(before.y - m.body.pos.y > 0.5 * dt, dt);
            m.hurt = (m.hurt - dt).max(0.0);
            // Babies stay babies until the host says otherwise.
            if m.baby > 0.0 {
                m.baby = 1.0;
            }
        }
        let love: Vec<Vec3> = self.mobs.iter().filter(|m| m.love > 0.0).map(|m| m.body.pos + Vec3::Y * m.body.height).collect();
        for p in love {
            if self.rng.chance(dt * 2.0) {
                self.hearts(p, 1);
            }
        }
        // Arrows in flight keep moving between snapshots.
        for a in self.arrows.iter_mut().filter(|a| !a.stuck) {
            a.pos += a.vel * dt;
            a.vel.y -= 20.0 * dt;
        }
        for t in self.tnts.iter_mut() {
            t.fuse -= dt;
        }
        self.falling_tick(dt);
        self.fireballs_tick(dt);
        // (Just the raid bar and the Bell's glow, on this side.)
        self.raids_tick(dt);
        for p in self.particles.iter_mut() {
            p.update(dt, &self.world);
        }
        self.particles.retain(|p| p.life > 0.0);
        self.client_drops(dt);
        self.client_orbs(dt);
    }

    // ------------------------------------------------------------ send

    pub fn net_send(&mut self, dt: f32) {
        if self.net.is_none() {
            return;
        }
        for t in self.net_timers.iter_mut() {
            *t -= dt;
        }
        let mobs_due = self.is_host() && self.net_timers[1] <= 0.0;
        if mobs_due {
            self.net_timers[1] = 0.1;
        }
        let (drops_due, orbs_due) = if self.is_host() { (self.send_drops(dt), self.send_orbs(dt)) } else { (false, false) };
        self.net_send_world(mobs_due);
        // The same for the other dimensions people are in (see realms.rs).
        if self.is_host() {
            for d in self.realm_dims() {
                if d == self.realm_dim() || self.parked.get(&d).is_none_or(|r| r.peers.is_empty()) {
                    continue;
                }
                self.in_realm(d, |g| {
                    if drops_due {
                        g.broadcast_drops();
                    }
                    if orbs_due {
                        g.broadcast_orbs();
                    }
                    g.net_send_world(mobs_due);
                    g.sounds.clear();
                });
            }
        }
        if self.net_timers[0] <= 0.0 && !self.away() {
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
            if self.spectator {
                flags |= FLAG_GHOST;
            }
            if p.gliding {
                flags |= FLAG_GLIDE;
            }
            if self.sleeping.is_some() {
                flags |= crate::net::FLAG_SLEEP;
            }
            let m = Msg::PlayerState { id: self.my_id, pos: p.body.pos, yaw: p.yaw, pitch: p.pitch, flags, held: self.inv.held(), held_ench: crate::enchant::enchants(self.inv.wear[self.inv.selected]), armor: self.inv.armor_look(), trims: crate::trims::look(&self.inv.armor, &self.inv.armor_wear) };
            self.net_send_msg(m);
        }
        if self.is_host() && self.net_timers[2] <= 0.0 {
            self.net_timers[2] = 2.0;
            self.net_broadcast_all(self.time_msg());
        }
        match &mut self.net {
            Some(Net::Client(c)) => c.flush(),
            Some(Net::Host(s)) => s.flush(),
            None => {}
        }
    }

    /// What's happened in the active dimension, to the people in it (the
    /// host also sends where the mobs are when `mobs_due`).
    fn net_send_world(&mut self, mobs_due: bool) {
        // Block edits (both directions; the host echoes everyone's).
        let edits = std::mem::take(&mut self.world.edit_log);
        for chunk in edits.chunks(4096) {
            self.net_send_msg(Msg::Blocks(chunk.to_vec()));
        }
        // What was just read from a region file goes to everyone (they may be near it).
        let news = self.world.regions.as_mut().map(|r| std::mem::take(&mut r.news)).unwrap_or_default();
        if matches!(self.net, Some(Net::Host(_))) && !news.is_empty() {
            for &(cx, cz) in &news {
                if let Some(m) = self.world.mods.get(&(cx, cz)) {
                    let entries = m.iter().map(|(&i, &b)| (i, b)).collect();
                    self.net_send_msg(Msg::Mods { cx, cz, entries });
                }
            }
            // And the signs and frames in them.
            let chunks: std::collections::HashSet<(i32, i32)> = news.into_iter().collect();
            let inside = |p: &IVec3| chunks.contains(&(p.x.div_euclid(16), p.z.div_euclid(16)));
            let mut out: Vec<Msg> = self.world.signs.iter().filter(|(p, _)| inside(p)).map(|(p, l)| Msg::SignText { x: p.x, y: p.y, z: p.z, lines: l.to_vec() }).collect();
            out.extend(self.world.frames.iter().filter(|(p, _)| inside(p)).map(|(p, &(item, wear))| Msg::FrameItem { x: p.x, y: p.y, z: p.z, item, wear }));
            out.extend(self.world.sign_styles.iter().filter(|(p, _)| inside(p)).map(|(p, &style)| Msg::SignStyle { x: p.x, y: p.y, z: p.z, style, item: AIR }));
            for m in out {
                self.net_send_msg(m);
            }
        }
        if self.is_host() {
            if mobs_due {
                let mobs = self
                    .mobs
                    .iter()
                    .map(|m| MobSnap {
                        id: m.id,
                        kind: m.kind.index(),
                        pos: m.body.pos,
                        yaw: m.yaw,
                        // Starers send "angry" and Hmmers their seed (it decides their trades) here.
                        // (Sneakers send what they're carrying.)
                        // (Bosses send their health, for the boss bar.)
                        fuse: if m.is_boss() { m.health } else if matches!(m.kind, MobKind::Hmmer | MobKind::Wanderer | MobKind::Sneaker | MobKind::CopperGolem) { m.seed as f32 } else if m.angry && matches!(m.kind, MobKind::Starer | MobKind::Weeper) { 1.0 } else { m.fuse },
                        hurt: m.hurt,
                        burning: m.burning,
                        // Its size (low half) and colouring (high half).
                        size: (m.size as u8).min(15) | (m.variant & 15) << 4,
                        flags: ((m.baby > 0.0) as u8 * MOB_BABY) | (m.sheared as u8 * MOB_SHEARED) | (m.owner.is_some() as u8 * MOB_TAMED) | (m.sitting as u8 * MOB_SITTING) | ((m.love > 0.0) as u8 * MOB_LOVE) | (m.saddled as u8 * MOB_SADDLED) | ((matches!(m.kind, MobKind::Soggy | MobKind::Pilferer | MobKind::Snout) && m.seed == 1) as u8 * MOB_ARMED),
                    })
                    .collect();
                let tnts = self.tnts.iter().map(|t| (t.pos, t.fuse)).collect();
                let arrows = self
                    .arrows
                    .iter()
                    .map(|a| ArrowSnap { pos: a.pos, vel: a.wire_vel(), appearance: a.appearance })
                    .collect();
                let falling = self.falling.iter().map(|f| (f.pos, f.vel, f.id)).collect();
                let fireballs = self.fireballs.iter().map(|f| (f.pos, f.vel, f.big)).collect();
                self.net_broadcast(Msg::Mobs { mobs, tnts, arrows, falling, fireballs });
            }
            let fwd: Vec<Msg> = self
                .sounds
                .iter()
                .filter_map(|&(s, at)| at.filter(|_| forwarded(s)).map(|at| Msg::Sound { sfx: s.to_wire(), at }))
                .collect();
            for m in fwd {
                self.net_broadcast(m);
            }
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
            drop_all: false,
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

    /// Keep the client standing on the host's `item` drop (it may still be
    /// flying) until they hold `want` of them. The host's own player is kept
    /// well away, or it could get there first.
    fn walk_to_drop(host: &mut Game, client: &mut Game, item: Id, want: u32) {
        for _ in 0..600 {
            if let Some(d) = host.drops.iter().find(|d| d.item == item) {
                client.player.body.pos = d.body.pos;
                host.player.body.pos = d.body.pos + Vec3::new(0.0, 0.0, 12.0);
            }
            host.update(0.016, &idle());
            client.update(0.016, &idle());
            if client.inv.count(item) == want {
                return;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        panic!("never picked up {}", item_name(item));
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
    fn projectile_appearance_snapshot_decodes_and_syncs_to_client() {
        let appearance = ProjectileAppearance { model: ProjectileModel::Cube, tile: Some(crate::texture::T_STONE), scale: 1.75 };
        let snapshot = Msg::Mobs {
            mobs: vec![],
            tnts: vec![],
            arrows: vec![ArrowSnap { pos: Vec3::new(1.0, 70.0, 2.0), vel: Vec3::X * 12.0, appearance }],
            falling: vec![],
            fireballs: vec![],
        };
        let Msg::Mobs { mobs, tnts, arrows, .. } = Msg::decode(&snapshot.encode()).expect("snapshot decodes") else { panic!("not mobs") };
        let mut client = Game::new(7, true, false);
        client.sync_mobs(mobs, tnts, arrows);
        assert_eq!(client.arrows.len(), 1);
        let arrow = &client.arrows[0];
        assert_eq!(arrow.appearance, appearance);
        assert_eq!(arrow.damage, 0.0, "host-only damage is not in the snapshot");
        assert_eq!(arrow.shooter, None, "host-only attribution is not in the snapshot");
        assert!(!arrow.modded, "host-only attribution is not in the snapshot");
        assert_eq!(arrow.effect, None, "host-only effects are not in the snapshot");
    }

    #[test]
    fn projectile_effect_remote_message_is_bounded_and_host_only() {
        let mut client = Game::new(8, true, false);
        client.client_handle(Msg::TimedEffect {
            effect: crate::potions::Potion::Strength.effect_index(),
            secs: 12.0,
            amplifier: u8::MAX,
        });
        assert_eq!(client.effect_amplifier(crate::potions::Potion::Strength), Some(3));
        client.client_handle(Msg::TimedEffect {
            effect: crate::potions::Potion::Strength.effect_index(),
            secs: 20.0,
            amplifier: 0,
        });
        assert_eq!(client.effect_amplifier(crate::potions::Potion::Strength), Some(3), "weaker refresh cannot downgrade");

        let mut host = Game::new(9, true, false);
        host.peers.insert(4, Peer::new("Modified Client".into(), host.spawn));
        host.host_handle_joined(4, Msg::TimedEffect {
            effect: crate::potions::Potion::Speed.effect_index(),
            secs: 300.0,
            amplifier: 3,
        });
        assert!(host.effects.is_empty(), "clients cannot apply timed effects to the host");
        assert_eq!(host.peers[&4].strikes, 1, "client-authored timed effects are rejected");
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
        let Msg::Welcome { id, seed, time, creative, spawn: cspawn, worldgen, .. } = welcome else { unreachable!() };
        assert_eq!(seed, 777);
        let mut client = Game::new_client(id, seed, worldgen, time, creative, cspawn, conn, "Clienty", leftover);
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
        let Msg::Welcome { id, seed, time, creative, spawn, worldgen, .. } = welcome else { unreachable!() };
        let mut client = Game::new_client(id, seed, worldgen, time, creative, spawn, conn, name, leftover);
        load_around(&mut client, spawn);
        Ok(client)
    }

    #[test]
    fn allow_list_and_operators() {
        let mut host = Game::new(779, false, false);
        let spawn = host.spawn;
        load_around(&mut host, spawn);
        let port = host.open_lan("Hosty", None).unwrap();
        host.admin_command(crate::admin::Caller::Host, "allowlist add Friendly");
        host.admin_command(crate::admin::Caller::Host, "allowlist on");
        let err = join(&mut host, port, "Stranger", "").err().expect("strangers are turned away");
        assert!(err.contains("allow-list"), "{err}");
        let mut client = join(&mut host, port, "Friendly", "").unwrap();
        let id = client.my_id;
        assert!(pump(&mut host, &mut client, |h, _| h.peers.contains_key(&id)));

        // Not an operator yet: commands are refused, but /list works for anyone.
        client.send_chat("/time night");
        client.send_chat("/list");
        assert!(pump(&mut host, &mut client, |_, c| c.messages.iter().any(|m| m.0.contains("Only operators")) && c.messages.iter().any(|m| m.0.contains("2 online"))));
        assert!((host.time - 0.55).abs() > 0.01);

        host.admin_command(crate::admin::Caller::Host, "op friendly");
        assert!(pump(&mut host, &mut client, |_, c| c.messages.iter().any(|m| m.0.contains("made you an operator"))));
        // Now the same player can use the commands.
        client.send_chat("/time noon");
        client.send_chat("/info friendly");
        assert!(pump(&mut host, &mut client, |h, c| (h.time - 0.25).abs() < 0.01 && c.messages.iter().any(|m| m.0.contains("health"))));
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
        client.net_send_msg(Msg::PlayerState { id, pos: spawn + Vec3::new(500.0, 0.0, 0.0), yaw: 0.0, pitch: 0.0, flags: 0, held: 0, held_ench: 0, armor: 0, trims: 0 });
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
        for _ in 0..20 {
            host.update(0.016, &idle());
            std::thread::sleep(Duration::from_millis(4));
        }
        assert!(!client.messages.iter().any(|m| m.0.contains("N 60")), "no probe, no reading");
        host.give_peer(id, SOIL_PROBE, 1);
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


    #[test]
    fn inventories_are_checked_by_the_host() {
        let mut host = Game::new(780, false, false);
        let spawn = host.spawn;
        load_around(&mut host, spawn);
        let port = host.open_lan("Hosty", None).unwrap();
        let mut client = join(&mut host, port, "Crafty", "").unwrap();
        let id = client.my_id;
        client.player.body.pos = spawn;
        assert!(pump(&mut host, &mut client, |h, _| h.peers.get(&id).map(|p| p.target.distance(spawn) < 1.0).unwrap_or(false)));
        let (x, z) = (spawn.x.floor() as i32 + 1, spawn.z.floor() as i32 + 1);
        let top = host.world.surface_y(x, z);
        for y in top + 1..top + 4 {
            host.world.set(x, y, z, AIR);
            client.world.set_remote(x, y, z, AIR);
        }

        // Placing a block they don't have: refused and undone.
        client.inv.slots[0] = Some((BRICK, 5)); // conjured by a modified client
        client.world.set(x, top + 1, z, BRICK);
        assert!(pump(&mut host, &mut client, |_, c| c.world.get(x, top + 1, z) == AIR));
        assert_eq!(host.world.get(x, top + 1, z), AIR);

        // ...and the next inventory check takes the fake bricks away.
        assert!(pump(&mut host, &mut client, |_, c| c.inv.count(BRICK) == 0), "fake items survived the check");

        // Items the host really gave can be placed, and are used up in its ledger.
        host.give_peer(id, DIRT, 3);
        assert!(pump(&mut host, &mut client, |_, c| c.inv.count(DIRT) == 3));
        client.world.set(x, top + 1, z, DIRT);
        client.inv.remove(DIRT, 1);
        assert!(pump(&mut host, &mut client, |h, _| h.world.get(x, top + 1, z) == DIRT));
        assert_eq!(host.peers[&id].ledger.bag.count(DIRT), 2);

        // Breaking drops the block on the host's ground (dirt digs quickly by hand)...
        client.break_block(IVec3::new(x, top + 1, z), true);
        assert!(pump(&mut host, &mut client, |h, _| h.world.get(x, top + 1, z) == AIR && !h.drops.is_empty()));
        assert_eq!(client.inv.count(DIRT), 2, "nothing until it's picked up");
        // ...and walking into it picks it up, through the ledger.
        walk_to_drop(&mut host, &mut client, DIRT, 3);
        assert!(host.drops.is_empty());
        assert_eq!(host.peers[&id].ledger.bag.count(DIRT), 3);
        client.player.body.pos = spawn;

        // Mining faster than bare hands allow is refused: stone takes ~7.5s by hand.
        let stone = IVec3::new(x, top - 1, z);
        host.world.set_v(stone, STONE);
        client.world.set_remote(stone.x, stone.y, stone.z, STONE);
        client.world.set_v(stone - IVec3::Y, AIR); // an instant first dig...
        client.world.set_v(stone, AIR); // ...then the stone straight away
        assert!(pump(&mut host, &mut client, |_, c| c.world.get_v(stone) == STONE), "insta-mined stone stuck");
        assert_eq!(host.world.get_v(stone), STONE);

        // Crafting with conjured ingredients doesn't count; the check corrects it.
        client.inv.add(DIAMOND, 3);
        client.inv.add(STICK, 2);
        let recipe = recipes().iter().position(|r| r.output.0 == PICK_DIAMOND).unwrap();
        assert!(client.inv.craft(&recipes()[recipe]));
        client.net_send_msg(Msg::Craft { recipe: recipe as u16, times: 1 });
        assert!(pump(&mut host, &mut client, |_, c| c.inv.count(PICK_DIAMOND) == 0 && c.inv.count(DIAMOND) == 0));
        assert_eq!(host.peers[&id].ledger.bag.count(PICK_DIAMOND), 0);

        // Honest crafting goes through: 1 log -> 4 planks.
        host.give_peer(id, LOG, 1);
        assert!(pump(&mut host, &mut client, |_, c| c.inv.count(LOG) == 1));
        let recipe = recipes().iter().position(|r| r.inputs == vec![(LOG, 1)]).unwrap();
        assert!(client.inv.craft(&recipes()[recipe]));
        client.net_send_msg(Msg::Craft { recipe: recipe as u16, times: 1 });
        assert!(pump(&mut host, &mut client, |h, _| h.peers[&id].ledger.bag.count(PLANKS) == 4));
        // Nothing to correct: the counts agree after a few checks.
        for _ in 0..400 {
            host.update(0.016, &idle());
            client.update(0.016, &idle());
        }
        assert_eq!(client.inv.count(PLANKS), 4);
        assert_eq!(client.inv.counts(), host.peers[&id].ledger.bag.items().into_iter().collect(), "client and host agree");

        // A sword they don't own doesn't hit any harder than a fist.
        client.inv.slots[client.inv.selected] = Some((SWORD_DIAMOND, 1)); // conjured
        let mut m = Mob::new(MobKind::Mooer, spawn + Vec3::new(1.5, 0.0, 0.0), &mut host.rng);
        m.id = 777;
        host.mobs.push(m);
        assert!(pump(&mut host, &mut client, |h, _| h.peers[&id].ledger.held == SWORD_DIAMOND));
        let before = host.mobs.iter().find(|m| m.id == 777).unwrap().health;
        client.net_send_msg(Msg::Attack { mob: 777, dmg: 12.0, from: spawn });
        assert!(pump(&mut host, &mut client, |h, _| h.mobs.iter().find(|m| m.id == 777).map(|m| m.health < before).unwrap_or(true)));
        let after = host.mobs.iter().find(|m| m.id == 777).map(|m| m.health).unwrap_or(before - 99.0);
        assert!(before - after <= attack_damage(AIR) * 1.5 + 1e-3, "hit for {}", before - after);
    }

    #[test]
    fn doors_stairs_and_drops_through_the_host() {
        let mut host = Game::new(782, false, false);
        let spawn = host.spawn;
        load_around(&mut host, spawn);
        let port = host.open_lan("Hosty", None).unwrap();
        let mut client = join(&mut host, port, "Buildy", "").unwrap();
        let id = client.my_id;
        client.player.body.pos = spawn;
        assert!(pump(&mut host, &mut client, |h, _| h.peers.get(&id).map(|p| p.target.distance(spawn) < 1.0).unwrap_or(false)));
        let (x, z) = (spawn.x.floor() as i32 + 2, spawn.z.floor() as i32);
        let ground = host.world.surface_y(x, z);
        for dx in 0..3 {
            for y in ground + 1..ground + 4 {
                host.world.set(x + dx, y, z, AIR);
            }
            host.world.set(x + dx, ground, z, STONE);
        }
        assert!(pump(&mut host, &mut client, |_, c| (0..3).all(|dx| c.world.get(x + dx, ground, z) == STONE && c.world.get(x + dx, ground + 1, z) == AIR)));
        host.give_peer(id, DOOR, 1);
        host.give_peer(id, stairs(0, 0), 3);
        assert!(pump(&mut host, &mut client, |_, c| c.inv.count(DOOR) == 1 && c.inv.count(stairs(0, 0)) == 3));

        // A door: both halves, paid for with one door item.
        let (bottom, top) = (IVec3::new(x, ground + 1, z), IVec3::new(x, ground + 2, z));
        client.world.set_v(bottom, door(1, false, false));
        client.world.set_v(top, door(1, false, true));
        client.inv.remove(DOOR, 1);
        assert!(pump(&mut host, &mut client, |h, _| h.world.get_v(bottom) == door(1, false, false) && h.world.get_v(top) == door(1, false, true)));
        assert_eq!(host.peers[&id].ledger.bag.count(DOOR), 0);
        // A door top floating on its own is refused.
        let lone = IVec3::new(x + 1, ground + 2, z);
        client.world.set_v(lone, door(1, false, true));
        assert!(pump(&mut host, &mut client, |_, c| c.world.get_v(lone) == AIR));
        // Opening it is free.
        client.world.set_v(bottom, door(1, true, false));
        client.world.set_v(top, door(1, true, true));
        assert!(pump(&mut host, &mut client, |h, _| h.world.get_v(top) == door(1, true, true)));

        // Stairs facing any way cost the stairs item.
        let step = IVec3::new(x + 2, ground + 1, z);
        client.world.set_v(step, stairs(0, 3));
        client.inv.remove(stairs(0, 0), 1);
        assert!(pump(&mut host, &mut client, |h, _| h.world.get_v(step) == stairs(0, 3)));
        assert_eq!(host.peers[&id].ledger.bag.count(stairs(0, 0)), 2);

        // Throwing takes them from the ledger and puts them on the host's ground.
        client.inv.slots = [None; 36];
        client.inv.slots[client.inv.selected] = Some((stairs(0, 0), 2));
        client.throw_held(true);
        assert!(pump(&mut host, &mut client, |h, _| h.drops.iter().any(|d| d.item == stairs(0, 0) && d.n == 2)));
        assert_eq!(host.peers[&id].ledger.bag.count(stairs(0, 0)), 0);
        // Throwing what you don't have makes nothing.
        client.net_send_msg(Msg::DropItem { item: DIAMOND, n: 64, wear: 0, scatter: false });
        for _ in 0..30 {
            host.update(0.016, &idle());
            client.update(0.016, &idle());
        }
        assert!(!host.drops.iter().any(|d| d.item == DIAMOND));
        // Asking for a drop from across the map gets nothing either.
        let far = host.drops[0].id;
        client.player.body.pos = spawn + Vec3::new(30.0, 0.0, 0.0);
        client.net_send_msg(Msg::Pickup { id: far, room: 64 });
        for _ in 0..30 {
            host.update(0.016, &idle());
            client.update(0.016, &idle());
        }
        assert!(host.drops.iter().any(|d| d.id == far));
        client.player.body.pos = spawn;

        // Breaking the door (after a moment: doors take a second by hand) takes both halves, one drop.
        for _ in 0..120 {
            host.update(0.016, &idle());
            client.update(0.016, &idle());
        }
        host.drops.clear();
        client.break_block(bottom, true);
        assert!(pump(&mut host, &mut client, |h, _| h.world.get_v(bottom) == AIR && h.world.get_v(top) == AIR));
        assert_eq!(host.drops.iter().filter(|d| d.item == DOOR).count(), 1);
    }

    #[test]
    fn wear_and_death_through_the_host() {
        let mut host = Game::new(783, false, false);
        let spawn = host.spawn;
        load_around(&mut host, spawn);
        let port = host.open_lan("Hosty", None).unwrap();
        let mut client = join(&mut host, port, "Clumsy", "").unwrap();
        let id = client.my_id;
        client.player.body.pos = spawn;
        assert!(pump(&mut host, &mut client, |h, _| h.peers.get(&id).map(|p| p.target.distance(spawn) < 1.0).unwrap_or(false)));

        // The host counts uses per kind of tool: 59 uses wear out one wooden pickaxe.
        host.give_peer(id, PICK_WOOD, 1);
        host.host_wear(id, PICK_WOOD, 30);
        assert_eq!(host.peers[&id].ledger.bag.count(PICK_WOOD), 1);
        host.host_wear(id, PICK_WOOD, 30);
        assert_eq!(host.peers[&id].ledger.bag.count(PICK_WOOD), 0, "worn out on the host too");
        // Tools you don't own don't count.
        host.host_wear(id, PICK_DIAMOND, 5000);

        // A used sword keeps its wear on the ground and back in your hands.
        host.give_peer(id, SWORD_STONE, 1);
        assert!(pump(&mut host, &mut client, |_, c| c.inv.count(SWORD_STONE) == 1));
        let slot = client.inv.slots.iter().position(|s| *s == Some((SWORD_STONE, 1))).unwrap();
        client.inv.selected = slot;
        client.inv.wear[slot] = 77;
        client.throw_held(false);
        assert!(pump(&mut host, &mut client, |h, _| h.drops.iter().any(|d| d.item == SWORD_STONE && d.wear == 77)));
        walk_to_drop(&mut host, &mut client, SWORD_STONE, 1);
        let slot = client.inv.slots.iter().position(|s| *s == Some((SWORD_STONE, 1))).unwrap();
        assert_eq!(client.inv.wear[slot], 77);

        // Dying drops everything on the host's ground, out of the ledger.
        host.drops.clear();
        host.give_peer(id, DIAMOND, 4);
        assert!(pump(&mut host, &mut client, |_, c| c.inv.count(DIAMOND) == 4));
        client.player.hurt = 0.0;
        client.hurt_player(100.0, "fell over in a test");
        assert!(client.inv.counts().is_empty());
        assert!(pump(&mut host, &mut client, |h, _| h.drops.iter().any(|d| d.item == DIAMOND && d.n == 4) && h.drops.iter().any(|d| d.item == SWORD_STONE)));
        assert_eq!(host.peers[&id].ledger.bag.count(DIAMOND), 0);
        assert_eq!(host.peers[&id].ledger.bag.count(SWORD_STONE), 0);
        client.respawn();
    }

    #[test]
    fn experience_anvils_and_rules_through_the_host() {
        use crate::xp::{level_of, points_for_level};
        let mut host = Game::new(784, false, false);
        let spawn = host.spawn;
        load_around(&mut host, spawn);
        let port = host.open_lan("Hosty", None).unwrap();
        let mut client = join(&mut host, port, "Smithy", "").unwrap();
        let id = client.my_id;
        client.player.body.pos = spawn;
        assert!(pump(&mut host, &mut client, |h, _| h.peers.get(&id).map(|p| p.target.distance(spawn) < 1.0).unwrap_or(false)));
        host.player.body.pos = spawn + Vec3::new(0.0, 0.0, 20.0); // out of the orbs' way

        // Rules come with joining, and follow changes.
        let mut rules = host.rules;
        rules.difficulty = crate::rules::Difficulty::Hard;
        rules.keep_inventory = true;
        host.set_rules(rules);
        assert!(pump(&mut host, &mut client, |_, c| c.rules == rules));

        // Orbs near the client are theirs, counted by the host.
        host.spawn_orbs(spawn + Vec3::new(2.0, 1.0, 0.0), 120);
        assert!(pump(&mut host, &mut client, |h, c| h.orbs.is_empty() && c.xp == 120));
        assert_eq!(host.peers[&id].ledger.xp, 120);

        // An anvil repair: the host takes the iron and the levels.
        let (x, z) = (spawn.x.floor() as i32 + 1, spawn.z.floor() as i32);
        let anvil = IVec3::new(x, host.world.surface_y(x, z) + 1, z);
        host.world.set_v(anvil, ANVIL);
        assert!(pump(&mut host, &mut client, |_, c| c.world.get_v(anvil) == ANVIL));
        host.give_peer(id, PICK_IRON, 1);
        host.give_peer(id, IRON, 3);
        assert!(pump(&mut host, &mut client, |_, c| c.inv.count(PICK_IRON) == 1 && c.inv.count(IRON) == 3));
        client.open_anvil(anvil);
        let slot = client.inv.slots.iter().position(|s| *s == Some((PICK_IRON, 1))).unwrap();
        client.inv.wear[slot] = 120;
        client.inv.click(slot);
        client.anvil_click(0, false);
        let slot = client.inv.slots.iter().position(|s| *s == Some((IRON, 3))).unwrap();
        client.inv.click(slot);
        client.anvil_click(1, false);
        let before = level_of(client.xp).0;
        client.anvil_take();
        assert_eq!(client.inv.cursor_wear, 0);
        assert!(pump(&mut host, &mut client, |h, _| h.peers[&id].ledger.bag.count(IRON) == 1));
        assert_eq!(level_of(host.peers[&id].ledger.xp).0, before - 2);
        assert!(pump(&mut host, &mut client, |h, c| c.xp == h.peers[&id].ledger.xp));
        client.inv.cursor = None;
        client.close_anvil();

        // A repair they can't afford (or with iron they don't have) is refused.
        host.peers.get_mut(&id).unwrap().ledger.xp = points_for_level(1);
        for _ in 0..20 {
            host.update(0.016, &idle()); // past the repair rate limit
            client.update(0.016, &idle());
        }
        client.net_send_msg(Msg::Repair { x: anvil.x, y: anvil.y, z: anvil.z, item: PICK_IRON, material: IRON, used: 4, combine: false, ench: 0, other_ench: 0 });
        assert!(pump(&mut host, &mut client, |_, c| c.xp == points_for_level(1)));
        assert_eq!(host.peers[&id].ledger.bag.count(IRON), 1);

        // Keep-inventory worlds keep experience too; otherwise dying spills it.
        client.player.hurt = 0.0;
        client.hurt_player(100.0, "fell on an anvil");
        for _ in 0..30 {
            host.update(0.016, &idle());
            client.update(0.016, &idle());
        }
        assert!(host.orbs.is_empty() && host.peers[&id].ledger.xp == points_for_level(1));
        client.respawn();
        rules.keep_inventory = false;
        host.set_rules(rules);
        assert!(pump(&mut host, &mut client, |h, c| !c.rules.keep_inventory && h.peers[&id].alive()));
        host.peers.get_mut(&id).unwrap().ledger.xp = points_for_level(3);
        client.player.hurt = 0.0;
        client.hurt_player(100.0, "fell on an anvil again");
        assert!(pump(&mut host, &mut client, |h, c| h.peers[&id].ledger.xp == 0 && c.xp == 0 && !h.orbs.is_empty()));
    }

    #[test]
    fn players_are_remembered() {
        let mut host = Game::new(785, false, false);
        let spawn = host.spawn;
        load_around(&mut host, spawn);
        let port = host.open_lan("Hosty", None).unwrap();
        let mut client = join(&mut host, port, "Returny", "").unwrap();
        let id = client.my_id;
        client.player.body.pos = spawn + Vec3::new(3.0, 0.0, 1.0);
        host.give_peer(id, DIAMOND, 7);
        host.give_peer_worn(id, PICK_IRON, 1, 42);
        host.give_peer_xp(id, 55);
        assert!(pump(&mut host, &mut client, |_, c| c.inv.count(DIAMOND) == 7 && c.inv.count(PICK_IRON) == 1 && c.xp == 55));
        let slot = client.inv.slots.iter().position(|s| *s == Some((PICK_IRON, 1))).unwrap();
        assert_eq!(client.inv.wear[slot], 42);
        client.player.hunger.food = 9.0;
        let at = client.player.body.pos;
        // Leaving sends one last report; the host remembers it.
        client.disconnect();
        drop(client);
        let start = Instant::now();
        while !host.peers.is_empty() && start.elapsed() < Duration::from_secs(10) {
            host.update(0.016, &idle());
            std::thread::sleep(Duration::from_millis(4));
        }
        let rec = host.saved_players.get("returny").expect("remembered").clone();
        assert_eq!(rec.xp, 55);
        assert!(rec.bag.contains(&(DIAMOND, 7)));
        assert_eq!(rec.report.food, 9.0);
        // It survives the host saving and loading the world.
        let back = Game::from_save(host.to_save());
        assert_eq!(back.saved_players.get("returny"), Some(&rec));

        // Coming back (any capitals) puts it all back, and the ledger with it.
        let mut client = join(&mut host, port, "RETURNY", "").unwrap();
        let id = client.my_id;
        assert!(pump(&mut host, &mut client, |_, c| c.inv.count(DIAMOND) == 7 && c.xp == 55));
        let slot = client.inv.slots.iter().position(|s| *s == Some((PICK_IRON, 1))).unwrap();
        assert_eq!(client.inv.wear[slot], 42);
        assert_eq!(client.player.hunger.food, 9.0);
        assert!(client.player.body.pos.distance(at) < 1.0);
        assert_eq!(host.peers[&id].ledger.bag.count(DIAMOND), 7);
        assert!(!host.saved_players.contains_key("returny"), "taken while they're here");
        // And the inventory check agrees with the host.
        for _ in 0..250 {
            host.update(0.016, &idle());
            client.update(0.016, &idle());
        }
        assert_eq!(client.inv.count(DIAMOND), 7);
    }

    #[test]
    fn buckets_go_through_the_ledger() {
        let mut host = Game::new(787, false, false);
        let spawn = host.spawn;
        load_around(&mut host, spawn);
        let port = host.open_lan("Hosty", None).unwrap();
        let mut client = join(&mut host, port, "Sloshy", "").unwrap();
        let id = client.my_id;
        let (x, z) = (spawn.x.floor() as i32 + 2, spawn.z.floor() as i32);
        let y = host.world.surface_y(x, z) + 1;
        let pond = IVec3::new(x, y, z);
        // A source walled in, so it doesn't run anywhere.
        for d in [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z] {
            host.world.set_v(pond + d, COBBLE);
        }
        host.world.set_v(pond, WATER);
        assert!(pump(&mut host, &mut client, |_, c| c.world.get_v(pond) == WATER));
        host.give_peer(id, BUCKET, 1);
        assert!(pump(&mut host, &mut client, |_, c| c.inv.count(BUCKET) == 1));
        // Scooping it up: the host swaps the bucket for a full one.
        client.world.set_v(pond, AIR);
        assert!(pump(&mut host, &mut client, |h, _| h.world.get_v(pond) == AIR));
        let l = &host.peers[&id].ledger;
        assert_eq!((l.bag.count(BUCKET), l.bag.count(WATER_BUCKET)), (0, 1));
        // Pouring it back is fine; pouring lava they don't have is not.
        client.world.set_v(pond, WATER);
        assert!(pump(&mut host, &mut client, |h, _| h.world.get_v(pond) == WATER));
        assert_eq!(host.peers[&id].ledger.bag.count(BUCKET), 1);
        let dry = pond + IVec3::Y * 2;
        client.world.set_v(dry, LAVA);
        for _ in 0..60 {
            host.update(0.016, &idle());
            client.update(0.016, &idle());
        }
        assert_eq!(host.world.get_v(dry), AIR);
        assert!(pump(&mut host, &mut client, |_, c| c.world.get_v(dry) == AIR), "the client is put right");
    }

    #[test]
    fn feeding_goes_through_the_host() {
        let mut host = Game::new(788, false, false);
        let spawn = host.spawn;
        load_around(&mut host, spawn);
        let port = host.open_lan("Hosty", None).unwrap();
        let mut client = join(&mut host, port, "Farmy", "").unwrap();
        let id = client.my_id;
        host.give_peer(id, WHEAT, 3);
        assert!(pump(&mut host, &mut client, |_, c| c.inv.count(WHEAT) == 3));
        host.mobs.clear();
        host.alloc_mob(MobKind::Mooer, spawn + Vec3::new(2.0, 0.5, 0.0));
        let cow = host.mobs[0].id;
        assert!(pump(&mut host, &mut client, |_, c| c.mobs.iter().any(|m| m.id == cow)));
        // Feeding it: the host takes the wheat and the Mooer falls in love (and says so).
        let slot = client.inv.slots.iter().position(|s| s.is_some_and(|s| s.0 == WHEAT)).unwrap();
        client.inv.selected = slot;
        client.net_send_msg(Msg::MobInteract { mob: cow, item: WHEAT });
        assert!(pump(&mut host, &mut client, |h, c| h.mobs[0].love > 0.0 && c.inv.count(WHEAT) == 2 && c.mobs.iter().any(|m| m.id == cow && m.love > 0.0)));
        assert_eq!(host.peers[&id].ledger.bag.count(WHEAT), 2);
        // Food they don't have does nothing.
        host.mobs[0].love = 0.0;
        for _ in 0..20 {
            host.update(0.016, &idle());
            client.update(0.016, &idle());
        }
        client.net_send_msg(Msg::MobInteract { mob: cow, item: CARROT });
        for _ in 0..30 {
            host.update(0.016, &idle());
            client.update(0.016, &idle());
        }
        assert_eq!(host.mobs[0].love, 0.0);
    }

    #[test]
    fn joined_players_flip_switches() {
        let mut host = Game::new(789, false, false);
        let spawn = host.spawn;
        load_around(&mut host, spawn);
        let port = host.open_lan("Hosty", None).unwrap();
        let mut client = join(&mut host, port, "Sparky", "").unwrap();
        let (x, z) = (spawn.x.floor() as i32 + 2, spawn.z.floor() as i32);
        let y = host.world.surface_y(x, z) + 1;
        let (lever, lamp, button) = (IVec3::new(x, y, z), IVec3::new(x + 1, y, z), IVec3::new(x, y, z + 2));
        host.world.set_v(lever, LEVER);
        host.world.set_v(lamp, LAMP);
        host.world.set_v(button, BUTTON);
        assert!(pump(&mut host, &mut client, |_, c| c.world.get_v(lamp) == LAMP && c.world.get_v(button) == BUTTON));
        // A flipped lever lights the lamp, on the host and back on the client.
        client.world.set_v(lever, LEVER_ON);
        assert!(pump(&mut host, &mut client, |h, c| h.world.get_v(lamp) == LAMP_ON && c.world.get_v(lamp) == LAMP_ON));
        // A pressed button pops back out by itself.
        client.world.set_v(button, BUTTON_ON);
        assert!(pump(&mut host, &mut client, |h, _| h.world.get_v(button) == BUTTON_ON));
        assert!(pump(&mut host, &mut client, |h, c| h.world.get_v(button) == BUTTON && c.world.get_v(button) == BUTTON));
        // Clients can't power dust or plates themselves.
        client.world.set_v(lamp + IVec3::X, WIRE_ON);
        for _ in 0..40 {
            host.update(0.016, &idle());
            client.update(0.016, &idle());
        }
        assert_eq!(host.world.get_v(lamp + IVec3::X), AIR);
    }

    #[test]
    fn books_at_the_anvil_are_checked() {
        use crate::enchant::{with_level, Enchant};
        let mut host = Game::new(790, false, false);
        let spawn = host.spawn;
        load_around(&mut host, spawn);
        let port = host.open_lan("Hosty", None).unwrap();
        let mut client = join(&mut host, port, "Booky", "").unwrap();
        let id = client.my_id;
        let (x, z) = (spawn.x.floor() as i32 + 1, spawn.z.floor() as i32);
        let anvil = IVec3::new(x, host.world.surface_y(x, z) + 1, z);
        host.world.set_v(anvil, ANVIL);
        let sharp = with_level(0, Enchant::Sharpness, 2);
        host.give_peer(id, SWORD_IRON, 1);
        host.give_peer_worn(id, ENCHANTED_BOOK, 1, sharp);
        host.give_peer_xp(id, crate::xp::points_for_level(5));
        assert!(pump(&mut host, &mut client, |_, c| c.inv.count(ENCHANTED_BOOK) == 1 && c.world.get_v(anvil) == ANVIL));
        // A forged book (Sharpness V they never had) does nothing.
        let forged = (with_level(0, Enchant::Sharpness, 5) >> 16) as u16;
        client.net_send_msg(Msg::Repair { x: anvil.x, y: anvil.y, z: anvil.z, item: SWORD_IRON, material: ENCHANTED_BOOK, used: 1, combine: false, ench: 0, other_ench: forged });
        for _ in 0..40 {
            host.update(0.016, &idle());
            client.update(0.016, &idle());
        }
        assert_eq!(host.peers[&id].ledger.bag.count(ENCHANTED_BOOK), 1);
        // The real one goes on: the book's gone, the sword is enchanted, levels spent.
        client.net_send_msg(Msg::Repair { x: anvil.x, y: anvil.y, z: anvil.z, item: SWORD_IRON, material: ENCHANTED_BOOK, used: 1, combine: false, ench: 0, other_ench: (sharp >> 16) as u16 });
        assert!(pump(&mut host, &mut client, |h, _| h.peers[&id].ledger.bag.count(ENCHANTED_BOOK) == 0));
        let l = &host.peers[&id].ledger;
        assert_eq!(l.enchanted.get(&(SWORD_IRON, (sharp >> 16) as u16)), Some(&1));
        assert_eq!(crate::xp::level_of(l.xp).0, 3);
    }

    #[test]
    fn modded_traders_and_boss_health_reach_joined_players() {
        let src = "[mob merchant]\nname = Merchant\ntexture = stone\ntrade = wheat 4 -> bread 2\n\n[mob king]\nname = King\ntexture = stone\nhealth = 50\nhostile = true\nboss = true\n";
        crate::mods::with_mods(&[("shop", src)], |_reg| {
            let mut host = Game::new(792, false, false);
            let spawn = host.spawn;
            load_around(&mut host, spawn);
            let port = host.open_lan("Hosty", None).unwrap();
            let mut client = join(&mut host, port, "Tradey", "").unwrap();
            let id = client.my_id;
            host.mobs.clear();
            let merchant = host.alloc_mob(MobKind::from_name("shop:merchant").unwrap(), spawn + Vec3::new(2.0, 0.5, 0.0));
            let king = host.alloc_mob(MobKind::from_name("shop:king").unwrap(), spawn + Vec3::new(-6.0, 0.5, 0.0));
            if let Some(k) = host.mobs.iter_mut().find(|m| m.id == king) {
                k.health = 20.0;
            }
            // (The Hollow Wyrm's bar, too.)
            let wyrm = host.alloc_mob(MobKind::Wyrm, spawn + Vec3::new(0.0, 30.0, 20.0));
            if let Some(w) = host.mobs.iter_mut().find(|m| m.id == wyrm) {
                w.health = 77.0;
            }
            host.give_peer(id, WHEAT, 8);
            assert!(pump(&mut host, &mut client, |_, c| c.inv.count(WHEAT) == 8 && c.mobs.iter().any(|m| m.id == merchant)));
            // The boss bar on a joined player's screen shows the host's health.
            assert!(pump(&mut host, &mut client, |_, c| c.mobs.iter().any(|m| m.id == king && (m.health - 20.0).abs() < 0.5) && c.mobs.iter().any(|m| m.id == wyrm && (m.health - 77.0).abs() < 0.5)));
            client.open_trade(merchant);
            let (title, list) = client.trade_list().expect("talking");
            assert_eq!(title, "Merchant");
            client.make_trade(0);
            assert!(pump(&mut host, &mut client, |_, c| c.inv.count(BREAD) == 2 && c.inv.count(WHEAT) == 4));
            assert_eq!(host.peers[&id].ledger.bag.count(BREAD), 2);
            assert_eq!(list[0].give[0], (WHEAT, 4));
        });
    }

    #[test]
    fn trading_goes_through_the_host() {
        use crate::villagers::{trades, Job};
        let mut host = Game::new(791, false, false);
        let spawn = host.spawn;
        load_around(&mut host, spawn);
        let port = host.open_lan("Hosty", None).unwrap();
        let mut client = join(&mut host, port, "Tradey", "").unwrap();
        let id = client.my_id;
        host.mobs.clear();
        let seed = (0..64u32).find(|&s| Job::of(s) == Job::Farmer).unwrap();
        host.world.new_huts.push((spawn + Vec3::new(2.0, 0.5, 0.0), seed));
        host.house_hmmers();
        let hmmer = host.mobs[0].id;
        host.give_peer(id, WHEAT, 40);
        assert!(pump(&mut host, &mut client, |_, c| c.inv.count(WHEAT) == 40 && c.mobs.iter().any(|m| m.id == hmmer && m.seed == seed)));
        // The client sees the same trades, and makes one.
        client.open_trade(hmmer);
        let list = client.trade_list().expect("talking").1;
        assert_eq!(list, trades(seed));
        let wheat = list.iter().position(|t| t.give[0].0 == WHEAT).unwrap();
        client.make_trade(wheat);
        assert!(pump(&mut host, &mut client, |_, c| c.inv.count(GOLD_INGOT) == 1 && c.inv.count(WHEAT) == 20));
        let l = &host.peers[&id].ledger;
        assert_eq!((l.bag.count(WHEAT), l.bag.count(GOLD_INGOT)), (20, 1));
        // Trading wheat they don't have (as far as the host knows) gets nothing.
        host.peers.get_mut(&id).unwrap().ledger.bag.take(WHEAT, 20);
        for _ in 0..20 {
            host.update(0.016, &idle());
            client.update(0.016, &idle());
        }
        client.net_send_msg(Msg::Trade { mob: hmmer, index: wheat as u8 });
        for _ in 0..40 {
            host.update(0.016, &idle());
            client.update(0.016, &idle());
        }
        assert_eq!(host.peers[&id].ledger.bag.count(GOLD_INGOT), 1);
    }

    #[test]
    fn portals_take_joined_players_through() {
        let mut host = Game::new(792, false, false);
        let spawn = host.spawn;
        load_around(&mut host, spawn);
        let port = host.open_lan("Hosty", None).unwrap();
        let mut client = join(&mut host, port, "Travelly", "").unwrap();
        let id = client.my_id;
        let (x, z) = (spawn.x.floor() as i32 + 3, spawn.z.floor() as i32);
        let base = IVec3::new(x, host.world.surface_y(x, z) + 1, z);
        crate::scorch::build_portal(&mut host.world, base);
        assert!(pump(&mut host, &mut client, |_, c| c.world.get_v(base) == PORTAL_X));
        // Walk in and wait: the host builds the other end and moves us there.
        client.player.body.pos = base.as_vec3() + Vec3::new(1.0, 0.0, 0.5);
        for _ in 0..30 {
            host.update(0.016, &idle());
            client.update(0.016, &idle());
        }
        for _ in 0..50 {
            client.portal_tick(0.05);
        }
        assert!(pump(&mut host, &mut client, |_, c| c.dim == Dim::Scorch && c.realm_dim() == Dim::Scorch));
        assert_eq!(host.peer_dim(id), Some(Dim::Scorch));
        assert_eq!(host.peer_ref(id).unwrap().dim, Dim::Scorch);
        // The host (still at home) keeps the Scorchlands going around them.
        assert_eq!(host.realm_dim(), Dim::Over);
        assert!(pump(&mut host, &mut client, |h, _| h.parked.get(&Dim::Scorch).is_some_and(|r| !r.world.chunks.is_empty())));
        // A portal they aren't standing in does nothing.
        client.net_send_msg(Msg::UsePortal { x: base.x, y: base.y, z: base.z });
        for _ in 0..40 {
            host.update(0.016, &idle());
            client.update(0.016, &idle());
        }
        assert_eq!(client.dim, Dim::Scorch);
        // Dying there brings them home.
        client.dead = Some("fell in lava".into());
        client.respawn();
        assert!(pump(&mut host, &mut client, |_, c| c.dim == Dim::Over));
        assert_eq!(host.peer_dim(id), Some(Dim::Over));
    }

    #[test]
    fn vehicles_through_the_host() {
        let mut host = Game::new(793, false, false);
        let spawn = host.spawn;
        load_around(&mut host, spawn);
        let port = host.open_lan("Hosty", None).unwrap();
        let mut client = join(&mut host, port, "Rowy", "").unwrap();
        let id = client.my_id;
        host.give_peer(id, BOAT, 1);
        assert!(pump(&mut host, &mut client, |_, c| c.inv.count(BOAT) == 1));
        // Put a boat down: the host takes the boat from the ledger and everyone sees it.
        let at = spawn + Vec3::new(2.0, 0.0, 0.0);
        client.net_send_msg(Msg::PlaceVehicle { kind: crate::vehicles::BOAT_KIND, pos: at, yaw: 0.0 });
        assert!(pump(&mut host, &mut client, |h, c| h.vehicles.len() == 1 && c.vehicles.len() == 1));
        assert_eq!(host.peers[&id].ledger.bag.count(BOAT), 0);
        // Get in and drive it: the host follows along.
        client.mount(0);
        assert!(pump(&mut host, &mut client, |h, _| h.vehicles[0].rider == id + 1));
        let moved = at + Vec3::new(0.0, 0.0, -3.0);
        client.vehicles[0].pos = moved;
        assert!(pump(&mut host, &mut client, |h, _| h.vehicles[0].pos.distance(moved) < 0.5));
        // Out, then hit it into an item.
        for _ in 0..10 {
            host.update(0.016, &idle());
            client.update(0.016, &idle());
        }
        client.dismount();
        assert!(pump(&mut host, &mut client, |h, _| h.vehicles[0].rider == 0));
        for _ in 0..3 {
            for _ in 0..12 {
                host.update(0.016, &idle());
                client.update(0.016, &idle());
            }
            client.net_send_msg(Msg::VehicleUse { id: host.vehicles.first().map(|v| v.id).unwrap_or(0), action: 2 });
        }
        assert!(pump(&mut host, &mut client, |h, c| h.vehicles.is_empty() && c.vehicles.is_empty()));
        assert!(host.drops.iter().any(|d| d.item == BOAT));
    }

    #[test]
    fn signs_and_frames_are_shared() {
        let mut host = Game::new(794, false, false);
        let spawn = host.spawn;
        load_around(&mut host, spawn);
        let (x, y, z) = (spawn.x.floor() as i32 + 2, spawn.y.floor() as i32 + 1, spawn.z.floor() as i32);
        let (sign, frame) = (IVec3::new(x, y, z), IVec3::new(x + 1, y, z));
        host.world.set_v(sign - IVec3::Y, STONE);
        host.world.set_v(sign, SIGN_FIRST);
        host.world.set_v(frame + IVec3::X, STONE);
        host.world.set_v(frame, FRAME_FIRST + 1);
        host.set_sign(sign, &["Old".into(), "news".into()]);
        let port = host.open_lan("Hosty", None).unwrap();
        let mut client = join(&mut host, port, "Signy", "").unwrap();
        let id = client.my_id;
        // Joining brings the words along.
        assert!(pump(&mut host, &mut client, |_, c| c.world.signs.get(&sign).is_some_and(|l| l[0] == "Old")));
        // The client rewrites it; everyone sees.
        client.set_sign(sign, &["New".into(), "news".into()]);
        assert!(pump(&mut host, &mut client, |h, _| h.world.signs[&sign][0] == "New"));
        // A frame: only things they have go in.
        client.net_send_msg(Msg::FrameUse { x: frame.x, y: frame.y, z: frame.z, item: DIAMOND, wear: 0, put: true });
        for _ in 0..30 {
            host.update(0.016, &idle());
            client.update(0.016, &idle());
        }
        assert!(host.world.frames.is_empty());
        host.give_peer(id, DIAMOND, 1);
        assert!(pump(&mut host, &mut client, |_, c| c.inv.count(DIAMOND) == 1));
        let slot = client.inv.slots.iter().position(|s| s.is_some_and(|s| s.0 == DIAMOND)).unwrap();
        client.inv.selected = slot;
        assert!(client.use_frame(frame));
        assert!(pump(&mut host, &mut client, |h, _| h.world.frames.get(&frame) == Some(&(DIAMOND, 0))));
        assert_eq!(host.peers[&id].ledger.bag.count(DIAMOND), 0);
        // Knock it out: it lands on the host's ground.
        for _ in 0..10 {
            host.update(0.016, &idle());
            client.update(0.016, &idle());
        }
        assert!(client.hit_frame(frame));
        assert!(pump(&mut host, &mut client, |h, c| h.world.frames.is_empty() && c.world.frames.is_empty() && h.drops.iter().any(|d| d.item == DIAMOND)));
    }

    #[test]
    fn enchanting_is_checked_by_the_host() {
        use crate::enchant::{enchants, with_level, Enchant};
        use crate::xp::points_for_level;
        let mut host = Game::new(786, false, false);
        let spawn = host.spawn;
        load_around(&mut host, spawn);
        let port = host.open_lan("Hosty", None).unwrap();
        let mut client = join(&mut host, port, "Glowy", "").unwrap();
        let id = client.my_id;
        let (x, z) = (spawn.x.floor() as i32 + 1, spawn.z.floor() as i32);
        let table = IVec3::new(x, host.world.surface_y(x, z) + 1, z);
        host.world.set_v(table, ENCHANTING_TABLE);
        assert!(pump(&mut host, &mut client, |_, c| c.world.get_v(table) == ENCHANTING_TABLE));
        host.give_peer(id, PICK_IRON, 1);
        host.give_peer(id, GOLD_INGOT, 3);
        host.give_peer_xp(id, points_for_level(10));
        assert!(pump(&mut host, &mut client, |_, c| c.inv.count(PICK_IRON) == 1 && c.inv.count(GOLD_INGOT) == 3 && c.xp == points_for_level(10)));

        client.open_enchanting(table);
        for item in [PICK_IRON, GOLD_INGOT] {
            let slot = client.inv.slots.iter().position(|s| s.is_some_and(|s| s.0 == item)).unwrap();
            client.enchant_quick_put(slot);
        }
        let offer = client.enchant_offers().expect("offers")[0];
        client.enchant_pick(0);
        let bits = enchants(offer.1);
        assert!(bits != 0);
        // The host charges a level and a gold ingot, and knows about the enchantments.
        assert!(pump(&mut host, &mut client, |h, c| h.peers[&id].ledger.enchanted.get(&(PICK_IRON, bits)) == Some(&1) && c.enchant_count == 1));
        let l = &host.peers[&id].ledger;
        assert_eq!((l.bag.count(GOLD_INGOT), l.enchant_count, crate::xp::level_of(l.xp).0), (2, 1, 9));
        client.close_enchanting();

        // Held, the host believes it; a made-up enchantment it doesn't.
        let slot = client.inv.slots.iter().position(|s| *s == Some((PICK_IRON, 1))).unwrap();
        client.inv.selected = slot;
        assert!(slot < 9, "the pickaxe comes back to the hotbar");
        assert!(pump(&mut host, &mut client, |h, _| h.verified_ench(id) == bits));
        let real = client.inv.wear[slot];
        client.inv.wear[slot] = with_level(real, Enchant::Sharpness, 5);
        assert!(pump(&mut host, &mut client, |h, _| h.peers[&id].ledger.held_ench != bits));
        assert_eq!(host.verified_ench(id), 0);

        // A forged enchantment thrown on the ground arrives plain.
        client.throw_held(false);
        assert!(pump(&mut host, &mut client, |h, _| h.drops.iter().any(|d| d.item == PICK_IRON)));
        let d = host.drops.iter().find(|d| d.item == PICK_IRON).unwrap();
        assert_eq!(d.wear >> 16, 0);
        assert_eq!(host.peers[&id].ledger.enchanted.get(&(PICK_IRON, bits)), Some(&1), "the real one is still theirs");

        // Enchanting without the gold (the host's count) is refused.
        host.peers.get_mut(&id).unwrap().ledger.bag.take(GOLD_INGOT, 2);
        for _ in 0..20 {
            host.update(0.016, &idle());
            client.update(0.016, &idle());
        }
        client.net_send_msg(Msg::Enchant { x: table.x, y: table.y, z: table.z, item: PICK_IRON, choice: 0 });
        for _ in 0..60 {
            host.update(0.016, &idle());
            client.update(0.016, &idle());
        }
        assert_eq!(host.peers[&id].ledger.enchant_count, 1);
    }

        #[test]
    fn chests_are_shared_through_the_host() {
        let mut host = Game::new(781, false, false);
        let spawn = host.spawn;
        load_around(&mut host, spawn);
        let port = host.open_lan("Hosty", None).unwrap();
        let mut client = join(&mut host, port, "Chesty", "").unwrap();
        let id = client.my_id;
        client.player.body.pos = spawn;
        assert!(pump(&mut host, &mut client, |h, _| h.peers.get(&id).map(|p| p.target.distance(spawn) < 1.0).unwrap_or(false)));
        let (x, z) = (spawn.x.floor() as i32 + 1, spawn.z.floor() as i32 + 1);
        let pos = IVec3::new(x, host.world.surface_y(x, z) + 1, z);
        host.world.set_v(pos, CHEST);
        host.world.containers.get_mut(&pos).unwrap().slots[0] = Some((DIAMOND, 3));
        assert!(pump(&mut host, &mut client, |_, c| c.world.get_v(pos) == CHEST));

        // Opening it shows the host's contents.
        client.open_container(pos);
        assert!(pump(&mut host, &mut client, |_, c| c.world.containers[&pos].slots[0] == Some((DIAMOND, 3))));

        // Taking them: the host's chest empties and its ledger gains them.
        client.container_click(0, false, false);
        assert_eq!(client.inv.cursor, Some((DIAMOND, 3)));
        assert!(pump(&mut host, &mut client, |h, _| h.world.containers[&pos].slots[0].is_none()));
        assert_eq!(host.peers[&id].ledger.bag.count(DIAMOND), 3);

        // Putting two back, somewhere else: the other way round.
        client.container_click(5, true, false); // right-click: one at a time
        client.container_click(5, true, false);
        assert!(pump(&mut host, &mut client, |h, _| h.world.containers[&pos].slots[5] == Some((DIAMOND, 2))));
        assert_eq!(host.peers[&id].ledger.bag.count(DIAMOND), 1);

        // Conjured items don't go in: the chest is corrected on their screen.
        client.inv.cursor = Some((GOLD_INGOT, 10));
        client.container_click(2, false, false);
        assert_eq!(client.world.containers[&pos].slots[2], Some((GOLD_INGOT, 10)));
        assert!(pump(&mut host, &mut client, |_, c| c.world.containers[&pos].slots[2].is_none()));
        assert!(host.world.containers[&pos].slots[2].is_none());
        // Nor can they take what isn't there.
        client.net_send_msg(Msg::ContainerMove { x: pos.x, y: pos.y, z: pos.z, slot: 5, item: DIAMOND, n: 60, put: false, wear: 0 });
        client.net_send_msg(Msg::ContainerMove { x: pos.x, y: pos.y, z: pos.z, slot: 9, item: DIAMOND, n: 1, put: false, wear: 0 });
        for _ in 0..30 {
            host.update(0.016, &idle());
            client.update(0.016, &idle());
        }
        assert_eq!(host.world.containers[&pos].slots[5], Some((DIAMOND, 2)));
        assert_eq!(host.peers[&id].ledger.bag.count(DIAMOND), 1);

        // Closed: moves are no longer accepted at all.
        client.inv.cursor = None;
        client.close_container();
        assert!(pump(&mut host, &mut client, |h, _| h.viewers.is_empty()));
        client.net_send_msg(Msg::ContainerMove { x: pos.x, y: pos.y, z: pos.z, slot: 5, item: DIAMOND, n: 1, put: false, wear: 0 });
        for _ in 0..30 {
            host.update(0.016, &idle());
            client.update(0.016, &idle());
        }
        assert_eq!(host.world.containers[&pos].slots[5], Some((DIAMOND, 2)));
    }

    #[test]
    fn joined_players_pack_bundles_through_the_host() {
        let mut host = Game::new(785, false, false);
        let spawn = host.spawn;
        load_around(&mut host, spawn);
        let port = host.open_lan("Hosty", None).unwrap();
        let mut client = join(&mut host, port, "Packer", "").unwrap();
        let id = client.my_id;
        client.player.body.pos = spawn;
        assert!(pump(&mut host, &mut client, |h, _| h.peers.get(&id).is_some_and(|p| p.target.distance(spawn) < 1.0)));
        host.give_peer(id, BUNDLE, 1);
        host.give_peer(id, COBBLE, 40);
        assert!(pump(&mut host, &mut client, |_, c| c.inv.count(BUNDLE) == 1 && c.inv.count(COBBLE) == 40));
        let b = client.inv.slots.iter().position(|s| s.is_some_and(|s| s.0 == BUNDLE)).unwrap();
        let c = client.inv.slots.iter().position(|s| s.is_some_and(|s| s.0 == COBBLE)).unwrap();
        client.inv.click(c);
        assert!(client.bundle_click(b, true, false));
        assert!(pump(&mut host, &mut client, |_, c| crate::boxes::box_id(c.inv.wear[b]) != 0));
        let tag = crate::boxes::box_id(client.inv.wear[b]);
        assert_eq!(client.bundle_contents(client.inv.wear[b]), vec![(COBBLE, 40)]);
        assert_eq!(host.peers[&id].ledger.bag.count(COBBLE), 0, "the host took the cobble");
        assert!(host.peers[&id].ledger.owns_enchanted(BUNDLE, tag));
        // Out again: it lands in the inventory.
        assert!(client.bundle_click(b, false, true));
        assert!(pump(&mut host, &mut client, |_, c| c.inv.count(COBBLE) == 40 && c.inv.wear[b] == 0));
        assert_eq!(host.peers[&id].ledger.bag.count(COBBLE), 40);
        assert!(!host.boxes.contains_key(&tag));
    }

    #[test]
    fn joined_players_write_and_sign_books_through_the_host() {
        let mut host = Game::new(786, false, false);
        let spawn = host.spawn;
        load_around(&mut host, spawn);
        let port = host.open_lan("Hosty", None).unwrap();
        let mut client = join(&mut host, port, "Writer", "").unwrap();
        let id = client.my_id;
        client.player.body.pos = spawn;
        assert!(pump(&mut host, &mut client, |h, _| h.peers.get(&id).is_some_and(|p| p.target.distance(spawn) < 1.0)));
        host.give_peer(id, BOOK_AND_QUILL, 1);
        assert!(pump(&mut host, &mut client, |_, c| c.inv.count(BOOK_AND_QUILL) == 1));
        let slot = client.inv.slots.iter().position(|s| s.is_some_and(|s| s.0 == BOOK_AND_QUILL)).unwrap();
        client.finish_writing(slot, vec!["Dear diary".into()], Some("Diary".into()));
        assert!(pump(&mut host, &mut client, |_, c| c.inv.slots[slot] == Some((WRITTEN_BOOK, 1))));
        let tag = crate::boxes::box_id(client.inv.wear[slot]);
        assert_eq!(host.books.get(&tag).map(|b| (b.title.as_str(), b.author.as_str())), Some(("Diary", "Writer")));
        assert!(host.peers[&id].ledger.owns_enchanted(WRITTEN_BOOK, tag));
        assert_eq!(host.peers[&id].ledger.bag.count(BOOK_AND_QUILL), 0);
    }

}
