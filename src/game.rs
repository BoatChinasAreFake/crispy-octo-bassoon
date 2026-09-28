//! Gameplay: the world plus everything living in it, and scene assembly.

use crate::block::*;
use crate::entity::*;
use crate::inventory::Inventory;
use crate::mesher::mesh_chunk;
use crate::multiplayer::{Net, Peer};
use crate::net::Msg;
use crate::noise::{hash2, Perlin, Rng};
use crate::player::{Input, Player, MAX_HEALTH};
use crate::render::{DynGeo, FrameParams, Pass, Renderer};
use crate::save::SaveData;
use crate::scripting::{Cmd, ScriptHost};
use rhai::{Dynamic, INT};
use crate::sound::{material, Sfx};
use crate::texture::*;
use crate::world::{Hit, World, CH, SEA};
use macroquad::math::{ivec3, IVec3, Mat4, Vec3, Vec4};
use macroquad::miniquad::RenderingBackend;
use std::collections::BTreeMap;
use std::f32::consts::{PI, TAU};

pub const DAY_SECONDS: f32 = 600.0;

pub struct Controls {
    pub input: Input,
    pub attack_held: bool,
    pub attack_pressed: bool,
    pub use_held: bool,
    pub use_pressed: bool,
    pub pick: bool,
    pub drop: bool,
}

pub enum Target {
    Block(Hit),
    Mob(usize),
}

pub struct Camera {
    pub pos: Vec3,
    pub dir: Vec3,
    pub view_proj: Mat4,
}

pub struct Game {
    pub world: World,
    pub player: Player,
    pub mobs: Vec<Mob>,
    pub particles: Vec<Particle>,
    pub tnts: Vec<PrimedTnt>,
    pub inv: Inventory,
    pub creative: bool,
    /// Fraction of a day, 0 = sunrise.
    pub time: f32,
    pub clock: f32,
    pub messages: Vec<(String, f32)>,
    pub breaking: Option<(IVec3, f32)>,
    pub rng: Rng,
    attack_cd: f32,
    use_cd: f32,
    pub spawn: Vec3,
    pub shake: f32,
    /// Title-screen panorama: no player simulation.
    pub menu: bool,
    pub target: Option<Target>,
    pub dead: Option<String>,
    pub third_person: bool,
    pub ready: bool,
    pub held_name: f32,
    spawn_timer: f32,
    stars: Vec<Vec3>,
    clouds: Perlin,
    pub stat_blocks_broken: u32,
    /// Sound effects requested this frame (effect, world position if positional).
    pub sounds: Vec<(Sfx, Option<Vec3>)>,
    dig_tick: f32,
    step_dist: f32,
    was_in_water: bool,
    // ---- multiplayer (see multiplayer.rs)
    pub net: Option<Net>,
    /// Other players, keyed by id (0 is the host).
    pub peers: BTreeMap<u32, Peer>,
    pub my_id: u32,
    pub player_name: String,
    pub next_mob_id: u32,
    pub net_timers: [f32; 3],
    /// Messages that arrived together with the Welcome, handled on the first update.
    pub pending_msgs: Vec<Msg>,
    /// Set when the connection drops; the app returns to the title screen.
    pub net_error: Option<String>,
    /// Headless `--server`: no local player at all.
    pub dedicated: bool,
    /// Script mods (only where the world lives: single player, host, server).
    pub scripts: Option<ScriptHost>,
    /// Script variables loaded from the save, handed to the scripts when they start.
    saved_script_vars: Vec<(String, Vec<u8>)>,
}

impl Game {
    pub fn new(seed: u32, creative: bool, menu: bool) -> Self {
        let world = World::new(seed);
        let spawn = world.find_spawn();
        let mut rng = Rng::new(seed as u64 ^ 0xC0FFEE);
        let stars = (0..350)
            .map(|_| Vec3::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), rng.range(-1.0, 1.0)).normalize_or_zero())
            .collect();
        let mut inv = Inventory::new();
        if creative {
            for (i, b) in [GRASS, COBBLE, PLANKS, LOG, GLASS, BRICK, GLOWROCK, TORCH, TNT].iter().enumerate() {
                inv.slots[i] = Some((*b, 64));
            }
        }
        let mut player = Player::new(spawn);
        player.yaw = 0.6;
        Game {
            world,
            player,
            mobs: Vec::new(),
            particles: Vec::new(),
            tnts: Vec::new(),
            inv,
            creative,
            time: 0.02,
            clock: 0.0,
            messages: Vec::new(),
            breaking: None,
            rng,
            attack_cd: 0.0,
            use_cd: 0.0,
            spawn,
            shake: 0.0,
            menu,
            target: None,
            dead: None,
            third_person: false,
            ready: false,
            held_name: 0.0,
            spawn_timer: 0.0,
            stars,
            clouds: Perlin::new(seed as u64 ^ 0xC10D),
            stat_blocks_broken: 0,
            sounds: Vec::new(),
            dig_tick: 0.0,
            step_dist: 0.0,
            was_in_water: false,
            net: None,
            peers: BTreeMap::new(),
            my_id: 0,
            player_name: "Stove".into(),
            next_mob_id: 1,
            net_timers: [0.0; 3],
            pending_msgs: Vec::new(),
            net_error: None,
            dedicated: false,
            scripts: None,
            saved_script_vars: Vec::new(),
        }
    }

    pub fn from_save(d: SaveData) -> Self {
        let mut g = Game::new(d.seed, d.creative, false);
        g.saved_script_vars = d.script_vars.clone();
        let remap = palette_remap(reg(), &d.palette);
        g.world.mods = d.mods;
        if let Some(remap) = &remap {
            for m in g.world.mods.values_mut() {
                for id in m.values_mut() {
                    *id = remap[*id as usize];
                }
            }
        }
        g.time = d.time;
        g.player.body.pos = Vec3::from_array(d.pos);
        g.player.fall_start = d.pos[1];
        g.player.yaw = d.yaw;
        g.player.pitch = d.pitch;
        g.player.health = d.health.max(1.0);
        g.spawn = Vec3::from_array(d.spawn);
        g.inv.slots = [None; 36];
        for (i, s) in d.slots.into_iter().take(36).enumerate() {
            g.inv.slots[i] = match (s, &remap) {
                (Some((id, n)), Some(r)) => Some((r[id as usize], n)).filter(|(id, _)| *id != AIR),
                (s, _) => s,
            };
        }
        g.msg("Welcome back. The world missed you (it's a HashMap, it can't feel).");
        g
    }

    pub fn to_save(&mut self) -> SaveData {
        self.inv.return_cursor();
        SaveData {
            seed: self.world.seed(),
            creative: self.creative,
            time: self.time,
            pos: self.player.body.pos.to_array(),
            yaw: self.player.yaw,
            pitch: self.player.pitch,
            health: self.player.health,
            spawn: self.spawn.to_array(),
            slots: self.inv.slots.to_vec(),
            mods: self.world.mods.clone(),
            palette: mod_palette(reg()),
            script_vars: self.export_script_vars(),
        }
    }

    pub fn msg(&mut self, s: impl Into<String>) {
        let s = s.into();
        if self.dedicated {
            println!("[{}] {s}", crate::server::timestamp());
        }
        self.messages.push((s, 7.0));
        if self.messages.len() > 6 {
            self.messages.remove(0);
        }
    }

    pub fn sun_angle(&self) -> f32 {
        self.time * TAU
    }

    pub fn daylight(&self) -> f32 {
        let s = self.sun_angle().sin();
        let t = ((s + 0.15) / 0.4).clamp(0.0, 1.0);
        0.18 + 0.82 * t * t * (3.0 - 2.0 * t)
    }

    pub fn is_night(&self) -> bool {
        self.sun_angle().sin() < -0.05
    }

    pub fn sky_color(&self) -> [f32; 3] {
        let d = ((self.daylight() - 0.18) / 0.82).clamp(0.0, 1.0);
        let day = [0.55, 0.75, 1.0];
        let night = [0.02, 0.03, 0.08];
        let mut c = [0.0; 3];
        for i in 0..3 {
            c[i] = night[i] + (day[i] - night[i]) * d;
        }
        // Sunrise / sunset glow
        let s = self.sun_angle().sin().abs();
        let glow = (1.0 - s / 0.25).clamp(0.0, 1.0) * 0.5;
        c[0] += (1.0 - c[0]) * glow;
        c[1] += (0.55 - c[1]) * glow * 0.6;
        c[2] *= 1.0 - glow * 0.5;
        c
    }

    /// Stream chunks and upload fresh meshes. Returns true once the player's area is ready.
    pub fn stream(&mut self, renderer: &mut Renderer, ctx: &mut dyn RenderingBackend, radius: i32) {
        let center = self.player.body.pos;
        let mut centers = vec![(center, radius)];
        if self.is_host() {
            // Keep the world simulated around remote players too.
            centers.extend(self.peers.values().map(|p| (p.target, radius.min(5))));
        }
        for k in self.world.stream(&centers) {
            renderer.drop_chunk(ctx, k);
        }
        let (pcx, pcz) = ((center.x / 16.0).floor() as i32, (center.z / 16.0).floor() as i32);
        let near = (radius + 1) * (radius + 1);
        let mut dirty: Vec<(i32, (i32, i32))> = self
            .world
            .dirty
            .iter()
            .filter(|&&(cx, cz)| (cx - pcx).pow(2) + (cz - pcz).pow(2) <= near && self.world.neighbours_ready(cx, cz))
            .map(|&(cx, cz)| ((cx - pcx).pow(2) + (cz - pcz).pow(2), (cx, cz)))
            .collect();
        dirty.sort_unstable();
        let start = macroquad::time::get_time();
        for (i, (_, k)) in dirty.into_iter().enumerate() {
            // Always finish a few (edits near the player), then stop at ~6 ms.
            if i >= 3 && macroquad::time::get_time() - start > 0.006 {
                break;
            }
            let mesh = mesh_chunk(&self.world, k.0, k.1);
            renderer.set_chunk(ctx, k, mesh);
            self.world.dirty.remove(&k);
        }
        if !self.ready {
            let p = self.player.body.pos;
            let key = ((p.x / 16.0).floor() as i32, (p.z / 16.0).floor() as i32);
            if renderer.chunks.contains_key(&key) {
                self.ready = true;
                if !self.menu {
                    self.settle_player();
                }
            }
        }
    }

    /// Make sure we don't start inside terrain.
    fn settle_player(&mut self) {
        let p = self.player.body.pos;
        let (x, z) = (p.x.floor() as i32, p.z.floor() as i32);
        let mut y = p.y.floor() as i32;
        while y < CH - 2 && (is_solid(self.world.get(x, y, z)) || is_solid(self.world.get(x, y + 1, z))) {
            y += 1;
        }
        self.player.body.pos.y = y as f32;
        self.player.fall_start = y as f32;
    }

    pub fn camera(&self, aspect: f32, fov_deg: f32) -> Camera {
        let (pos, dir) = if self.menu {
            let t = self.clock * 0.04;
            let dir = Vec3::new(t.sin() * 0.97, -0.22, -t.cos() * 0.97).normalize();
            (self.spawn + Vec3::Y * 14.0, dir)
        } else {
            let dir = self.player.look_dir();
            let mut eye = self.player.eye();
            if self.shake > 0.0 {
                let s = self.shake * 0.25;
                eye += Vec3::new((self.clock * 53.0).sin() * s, (self.clock * 71.0).sin() * s, (self.clock * 37.0).cos() * s);
            }
            if self.third_person {
                let back = self.world.raycast(eye, -dir, 4.0).map(|h| (h.dist - 0.3).max(0.2)).unwrap_or(4.0);
                (eye - dir * back, dir)
            } else {
                let b = self.player.bob;
                let bob = Vec3::new(0.0, (b * 2.0).sin().abs() * 0.06, 0.0);
                (eye + bob, dir)
            }
        };
        let fov = if !self.menu && self.player.sprinting { fov_deg + 8.0 } else { fov_deg };
        let proj = Mat4::perspective_rh_gl(fov.to_radians(), aspect, 0.05, 1000.0);
        let view = Mat4::look_at_rh(pos, pos + dir, Vec3::Y);
        Camera { pos, dir, view_proj: proj * view }
    }

    pub fn update(&mut self, dt: f32, c: &Controls) {
        self.net_receive(dt);
        self.update_local(dt, c);
        self.net_send(dt);
    }

    fn update_local(&mut self, dt: f32, c: &Controls) {
        self.clock += dt;
        self.time = (self.time + dt / DAY_SECONDS) % 1.0;
        self.shake = (self.shake - dt * 1.5).max(0.0);
        self.held_name = (self.held_name - dt).max(0.0);
        for m in self.messages.iter_mut() {
            m.1 -= dt;
        }
        self.messages.retain(|m| m.1 > 0.0);
        if self.menu || !self.ready || self.dead.is_some() {
            return;
        }
        let p = self.player.body.pos;
        if !self.world.is_loaded(p.x.floor() as i32, p.z.floor() as i32) {
            return;
        }

        let fall = self.player.update(dt, &c.input, &self.world, self.creative);
        if fall > 0.0 {
            self.sfx(Sfx::Thud, None);
            self.hurt_player(fall, "hit the ground too hard (the ground is fine)");
        }
        self.footsteps(dt);
        if self.player.body.pos.y < -30.0 {
            self.hurt_player(100.0, "fell out of the world. Classic.");
        }

        self.update_target();
        self.attack_cd = (self.attack_cd - dt).max(0.0);
        self.use_cd = (self.use_cd - dt).max(0.0);
        self.handle_actions(dt, c);
        self.update_entities(dt);
        self.script_tick(dt);
    }

    // ------------------------------------------------------------ scripting

    /// Load the active mods' scripts and run their `on_load`.
    pub fn start_scripts(&mut self) {
        self.start_scripts_with(&crate::mods::active_sources());
    }

    pub fn start_scripts_with(&mut self, sources: &[crate::mods::ModSource]) {
        if self.is_client() || self.menu || self.scripts.is_some() {
            return;
        }
        let (mut host, mut problems) = ScriptHost::new(sources);
        // Variables saved with this world come back before on_load runs.
        problems.extend(host.import_vars(&std::mem::take(&mut self.saved_script_vars)));
        for p in problems {
            self.msg(format!("Script error {p}"));
        }
        if host.is_empty() {
            // No scripts running, but keep any saved variables for next time.
            self.saved_script_vars = host.export_vars().0;
        } else {
            let ids = host.mod_ids().join(", ");
            self.scripts = Some(host);
            if self.dedicated {
                self.msg(format!("Scripts running: {ids}"));
            }
            self.fire("on_load", vec![]);
        }
    }

    /// Script variables for the save file (running mods plus kept data of absent ones).
    fn export_script_vars(&mut self) -> Vec<(String, Vec<u8>)> {
        let Some(host) = &self.scripts else { return self.saved_script_vars.clone() };
        let (vars, skipped) = host.export_vars();
        for s in skipped {
            self.msg(format!("Script variable not saved: {s}"));
        }
        vars
    }

    /// Run a script event. Returns false if a script cancelled the default action.
    pub fn fire(&mut self, hook: &str, args: Vec<Dynamic>) -> bool {
        let Some(mut host) = self.scripts.take() else { return true };
        let r = host.call(self, hook, args);
        self.scripts = Some(host);
        for line in r.log {
            self.msg(line);
        }
        for e in r.errors {
            eprintln!("script error: {e}");
            self.msg(format!("Script error {e}"));
        }
        self.apply_cmds(r.cmds);
        r.allow
    }

    fn script_tick(&mut self, dt: f32) {
        let Some(host) = &mut self.scripts else { return };
        host.tick_acc += dt;
        if host.tick_acc < crate::scripting::TICK {
            return;
        }
        host.tick_acc -= crate::scripting::TICK;
        self.fire("on_tick", vec![Dynamic::from(crate::scripting::TICK as rhai::FLOAT)]);
    }

    pub fn is_local_player(&self, name: &str) -> bool {
        !self.dedicated && (name.is_empty() || name.eq_ignore_ascii_case(&self.player_name))
    }

    pub fn player_names(&self) -> Vec<String> {
        let mut v = if self.dedicated { vec![] } else { vec![self.player_name.clone()] };
        v.extend(self.peers.values().map(|p| p.name.clone()));
        v
    }

    pub fn player_position(&self, name: &str) -> Option<Vec3> {
        if self.is_local_player(name) {
            return Some(self.player.body.pos);
        }
        self.peer_by_name(name).and_then(|id| self.peers.get(&id)).map(|p| p.target)
    }

    fn apply_cmds(&mut self, cmds: Vec<Cmd>) {
        let effect = |heal: f32, teleport: Option<Vec3>, launch: Option<f32>, take: Option<(u8, u8)>| Msg::Effect { heal, teleport, launch, take };
        for c in cmds {
            match c {
                Cmd::SetBlock(x, y, z, id) => self.world.set_or_record(x, y, z, id),
                Cmd::Broadcast(t) => {
                    self.msg(t.clone());
                    self.system_message(None, &t);
                }
                Cmd::Message(p, t) => {
                    if self.is_local_player(&p) {
                        self.msg(t);
                    } else if let Some(id) = self.peer_by_name(&p) {
                        self.system_message(Some(id), &t);
                    } else if self.dedicated && p.eq_ignore_ascii_case("server") {
                        self.msg(t);
                    }
                }
                Cmd::Give(p, item, n) => {
                    if self.is_local_player(&p) {
                        self.give(item, n);
                    } else if let Some(id) = self.peer_by_name(&p) {
                        self.net_send_to(id, Msg::Give { item, n });
                    }
                }
                Cmd::Take(p, item, n) => {
                    if self.is_local_player(&p) {
                        self.inv.remove(item, n as u32);
                    } else if let Some(id) = self.peer_by_name(&p) {
                        self.net_send_to(id, effect(0.0, None, None, Some((item, n))));
                    }
                }
                Cmd::Heal(p, n) => {
                    if self.is_local_player(&p) {
                        self.player.health = (self.player.health + n).min(MAX_HEALTH);
                    } else if let Some(id) = self.peer_by_name(&p) {
                        self.net_send_to(id, effect(n, None, None, None));
                    }
                }
                Cmd::Damage(p, n) => {
                    if self.is_local_player(&p) {
                        self.player.hurt = 0.0;
                        self.hurt_player(n, "was smitten by a script");
                    } else if let Some(id) = self.peer_by_name(&p) {
                        self.hurt_peer(id, n, "was smitten by a script", Vec3::ZERO);
                    }
                }
                Cmd::Teleport(p, to) => {
                    if self.is_local_player(&p) {
                        self.player.body.pos = to;
                        self.player.body.vel = Vec3::ZERO;
                        self.player.fall_start = to.y;
                    } else if let Some(id) = self.peer_by_name(&p) {
                        self.net_send_to(id, effect(0.0, Some(to), None, None));
                        if let Some(peer) = self.peers.get_mut(&id) {
                            peer.target = to;
                        }
                    }
                }
                Cmd::Launch(p, v) => {
                    if self.is_local_player(&p) {
                        self.player.body.vel.y = v;
                        self.player.fall_start = self.player.body.pos.y;
                    } else if let Some(id) = self.peer_by_name(&p) {
                        self.net_send_to(id, effect(0.0, None, Some(v), None));
                    }
                }
                Cmd::Explode(at, r) => self.explode(at, r, "was blown up by a script"),
                Cmd::Spawn(k, at) => {
                    let kind = [MobKind::Oinker, MobKind::Hisser, MobKind::Groaner][(k as usize).min(2)];
                    self.alloc_mob(kind, at);
                }
                Cmd::SetTime(t) => {
                    self.time = t;
                    self.net_broadcast(Msg::Time(t));
                }
                Cmd::Sound(s, at) => {
                    self.sfx(s, Some(at));
                    self.net_broadcast(Msg::Sound { sfx: s.to_u8(), at });
                }
            }
        }
    }

    pub fn sfx(&mut self, s: Sfx, at: Option<Vec3>) {
        if !self.menu {
            self.sounds.push((s, at));
        }
    }

    fn footsteps(&mut self, dt: f32) {
        let in_water = self.player.body.in_water;
        if in_water && !self.was_in_water {
            self.sfx(Sfx::Splash, None);
        }
        self.was_in_water = in_water;
        let b = &self.player.body;
        if !b.on_ground || in_water || self.player.flying {
            return;
        }
        self.step_dist += Vec3::new(b.vel.x, 0.0, b.vel.z).length() * dt;
        if self.step_dist > 1.9 {
            self.step_dist = 0.0;
            let feet = b.pos - Vec3::Y * 0.2;
            let under = self.world.get(feet.x.floor() as i32, feet.y.floor() as i32, feet.z.floor() as i32);
            if is_solid(under) {
                self.sfx(Sfx::Step(material(under)), None);
            }
        }
    }

    fn reach(&self) -> f32 {
        if self.creative { 6.5 } else { 5.0 }
    }

    fn update_target(&mut self) {
        let eye = self.player.eye();
        let dir = self.player.look_dir();
        let reach = self.reach();
        let hit = self.world.raycast(eye, dir, reach);
        let block_dist = hit.as_ref().map(|h| h.dist).unwrap_or(f32::MAX);
        let mut best: Option<(usize, f32)> = None;
        for (i, m) in self.mobs.iter().enumerate() {
            if let Some(t) = ray_aabb(eye, dir, m.body.min(), m.body.max()) {
                if t < reach.min(block_dist) && best.map(|b| t < b.1).unwrap_or(true) {
                    best = Some((i, t));
                }
            }
        }
        self.target = match (best, hit) {
            (Some((i, _)), _) => Some(Target::Mob(i)),
            (None, Some(h)) => Some(Target::Block(h)),
            _ => None,
        };
    }

    fn handle_actions(&mut self, dt: f32, c: &Controls) {
        let held = self.inv.held();
        if c.attack_pressed {
            self.player.swing = 1.0;
        }
        if c.attack_held && self.player.swing < 0.3 {
            self.player.swing = 1.0;
        }

        match &self.target {
            Some(Target::Mob(i)) => {
                self.breaking = None;
                if c.attack_pressed && self.attack_cd <= 0.0 {
                    let i = *i;
                    let dmg = attack_damage(held) * if self.player.body.vel.y < -1.0 { 1.5 } else { 1.0 };
                    let from = self.player.body.pos;
                    if self.is_client() {
                        // The host owns mobs: ask it to apply the hit (it echoes the sound back).
                        let mob = self.mobs[i].id;
                        self.net_send_msg(Msg::Attack { mob, dmg, from });
                        self.mobs[i].hurt = 0.5;
                    } else {
                        self.mobs[i].damage(dmg, from);
                        self.mobs[i].last_attacker = 0;
                        let (kind, at) = (self.mobs[i].kind, self.mobs[i].body.pos);
                        self.sfx(if kind == MobKind::Oinker { Sfx::Oink } else { Sfx::MobHurt }, Some(at));
                    }
                    self.attack_cd = 0.35;
                }
            }
            Some(Target::Block(h)) => {
                let pos = h.pos;
                if c.attack_held {
                    let id = self.world.get_v(pos);
                    if self.creative {
                        if self.attack_cd <= 0.0 {
                            self.break_block(pos, false);
                            self.attack_cd = 0.22;
                        }
                    } else {
                        let (t, _) = break_time(id, held);
                        let progress = match self.breaking {
                            Some((p, prog)) if p == pos => prog,
                            _ => 0.0,
                        };
                        let progress = if t <= 0.0 { 1.0 } else { progress + dt / t };
                        if self.rng.chance(dt * 10.0) {
                            self.block_particles(pos, 1);
                        }
                        self.dig_tick -= dt;
                        if self.dig_tick <= 0.0 {
                            self.dig_tick = 0.25;
                            self.sfx(Sfx::Hit(material(id)), Some(pos.as_vec3() + Vec3::splat(0.5)));
                        }
                        if progress >= 1.0 {
                            let (_, drops) = break_time(id, held);
                            self.break_block(pos, drops);
                            self.breaking = None;
                            self.attack_cd = 0.15;
                        } else {
                            self.breaking = Some((pos, progress));
                        }
                    }
                } else {
                    self.breaking = None;
                }
            }
            None => self.breaking = None,
        }
        if !c.attack_held {
            self.breaking = None;
        }

        if (c.use_pressed || (c.use_held && self.use_cd <= 0.0)) && self.use_cd <= 0.0 {
            self.use_cd = 0.25;
            self.use_item();
        }
        if !c.use_held {
            self.use_cd = 0.0;
        }

        if c.pick {
            if let Some(Target::Block(h)) = &self.target {
                let id = self.world.get_v(h.pos);
                if self.creative && is_block_item(id) {
                    self.inv.slots[self.inv.selected] = Some((id, 64));
                    self.held_name = 2.0;
                }
            }
        }
        if c.drop {
            let held = self.inv.held();
            if held != AIR {
                self.inv.consume_held();
                self.msg(format!("Yeeted 1x {} into the void.", item_name(held)));
            }
        }
    }

    fn use_item(&mut self) {
        let held = self.inv.held();
        if held != AIR {
            if self.is_client() {
                self.net_send_msg(Msg::UseItem { item: held });
            } else {
                let me = self.player_name.clone();
                if !self.fire("on_use_item", vec![me.into(), reg().key_of(held).into()]) {
                    return;
                }
            }
        }
        if let Some(heal) = food_value(held) {
            if self.player.health < MAX_HEALTH || self.creative {
                self.player.health = (self.player.health + heal).min(MAX_HEALTH);
                if !self.creative {
                    self.inv.consume_held();
                }
                self.sfx(Sfx::Eat, None);
                self.msg(if held == GOO { "You ate Groaner Goo. You feel... gooey." } else { "*nom* Oinkchop acquired (internally)." });
                return;
            }
        }
        if let Some((actions, consume)) = use_actions(held) {
            let at = self.player.eye() + self.player.look_dir() * 2.0;
            self.run_actions(actions, at);
            self.player.swing = 1.0;
            if consume && !self.creative {
                self.inv.consume_held();
            }
            return;
        }
        let Some(Target::Block(h)) = &self.target else { return };
        let (hit_pos, normal) = (h.pos, h.normal);
        let hit_id = self.world.get_v(hit_pos);
        if hit_id == TNT && (held == TORCH || held == AIR) {
            if self.is_client() {
                self.world.set_remote(hit_pos.x, hit_pos.y, hit_pos.z, AIR);
                self.net_send_msg(Msg::Ignite { x: hit_pos.x, y: hit_pos.y, z: hit_pos.z });
                self.msg("Hisss... wait, that's the TNT. RUN.");
                return;
            }
            self.world.set_v(hit_pos, AIR);
            self.tnts.push(PrimedTnt { pos: hit_pos.as_vec3(), fuse: 3.0 });
            self.sfx(Sfx::Hiss, Some(hit_pos.as_vec3() + Vec3::splat(0.5)));
            self.msg("Hisss... wait, that's the TNT. RUN.");
            return;
        }
        if hit_id == TABLE && !is_block_item(held) {
            self.msg("It's decorative! Press E to craft anywhere. Revolutionary.");
            return;
        }
        if !is_block_item(held) {
            return;
        }
        let place = if replaceable(hit_id) { hit_pos } else { hit_pos + normal };
        if place.y < 0 || place.y >= CH || !replaceable(self.world.get_v(place)) {
            return;
        }
        let below = self.world.get_v(place - IVec3::Y);
        match held {
            FLOWER | TALL_GRASS if !matches!(below, GRASS | DIRT | SNOW_GRASS) => return,
            TORCH if !is_solid(below) => return,
            _ => {}
        }
        if is_solid(held) {
            if self.player.body.intersects_block(place.x, place.y, place.z) {
                return;
            }
            if self.mobs.iter().any(|m| m.body.intersects_block(place.x, place.y, place.z)) {
                return;
            }
            if self.peers.values().any(|p| p.intersects_block(place)) {
                return;
            }
        }
        if !self.is_client() {
            let args = vec![self.player_name.clone().into(), (place.x as INT).into(), (place.y as INT).into(), (place.z as INT).into(), reg().key_of(held).into()];
            if !self.fire("on_block_place", args) {
                return;
            }
        }
        self.world.set_v(place, held);
        self.sfx(Sfx::Place(material(held)), Some(place.as_vec3() + Vec3::splat(0.5)));
        self.player.swing = 1.0;
        if !self.creative {
            self.inv.consume_held();
        }
    }

    pub fn break_block(&mut self, pos: IVec3, drops: bool) {
        let id = self.world.get_v(pos);
        if !targetable(id) || block(id).hardness < 0.0 {
            return;
        }
        if !self.is_client() {
            let args = vec![self.player_name.clone().into(), (pos.x as INT).into(), (pos.y as INT).into(), (pos.z as INT).into(), reg().key_of(id).into()];
            if !self.fire("on_block_break", args) {
                return;
            }
        }
        self.block_particles(pos, 14);
        self.world.set_v(pos, AIR);
        self.sfx(Sfx::Break(material(id)), Some(pos.as_vec3() + Vec3::splat(0.5)));
        let on_break: &'static [Action] = &block(id).on_break;
        if !on_break.is_empty() {
            self.run_actions(on_break, pos.as_vec3() + Vec3::splat(0.5));
        }
        self.stat_blocks_broken += 1;
        if drops && !self.creative {
            let d = block(id).drop;
            if d != AIR {
                self.give(d, 1);
            }
            if id == LEAVES && self.rng.chance(0.08) {
                self.give(STICK, 1);
            }
            if id == GRAVEL && self.rng.chance(0.1) {
                self.give(COAL, 1);
                self.msg("Found coal in the gravel. Don't ask.");
            }
        }
        // Plants and torches pop off with their support.
        let above = pos + IVec3::Y;
        let a = self.world.get_v(above);
        if block(a).model == Model::Cross {
            self.world.set_v(above, AIR);
            if !self.creative && block(a).drop != AIR {
                self.give(block(a).drop, 1);
            }
        }
        // Water flows (lazily) into the hole.
        for n in [IVec3::X, -IVec3::X, IVec3::Z, -IVec3::Z, IVec3::Y] {
            if self.world.get_v(pos + n) == WATER && pos.y <= SEA {
                self.world.set_v(pos, WATER);
                break;
            }
        }
    }

    /// Carry out mod-defined behaviour (see MODDING.md).
    pub fn run_actions(&mut self, actions: &[Action], at: Vec3) {
        for a in actions {
            match a {
                Action::Heal(n) => {
                    self.player.health = (self.player.health + n).min(MAX_HEALTH);
                    self.sfx(Sfx::Eat, None);
                }
                Action::Explode(r) => {
                    if self.is_client() {
                        self.net_send_msg(Msg::Explosion { at, r: *r });
                    } else {
                        self.explode(at, *r, "was blown up by a mod");
                    }
                }
                Action::Launch(v) => {
                    self.player.body.vel.y = *v;
                    self.player.fall_start = self.player.body.pos.y;
                }
                Action::Message(m) => self.msg(*m),
                Action::Give(item, n) => self.give(*item, *n),
                Action::SetTime(t) => {
                    if self.is_client() {
                        self.msg("Only the host can change the time.");
                    } else {
                        self.time = *t;
                        self.net_broadcast(Msg::Time(*t));
                    }
                }
                Action::Spawn(k) => {
                    if self.is_client() {
                        self.msg("Only the host can spawn mobs.");
                    } else {
                        let kind = [MobKind::Oinker, MobKind::Hisser, MobKind::Groaner][(*k as usize).min(2)];
                        self.alloc_mob(kind, at + Vec3::Y * 0.5);
                    }
                }
            }
        }
    }

    fn give(&mut self, item: u8, n: u8) {
        self.sfx(Sfx::Pop, None);
        if self.inv.add(item, n) > 0 {
            self.msg("Inventory full. The item has been respectfully ignored.");
        }
    }

    fn block_particles(&mut self, pos: IVec3, n: usize) {
        let id = self.world.get_v(pos);
        self.block_particles_tile(pos, block(id).tex[1], n);
    }

    fn block_particles_tile(&mut self, pos: IVec3, tile: u16, n: usize) {
        if self.dedicated {
            return;
        }
        for _ in 0..n {
            let r = &mut self.rng;
            let p = pos.as_vec3() + Vec3::new(r.range(0.1, 0.9), r.range(0.1, 0.9), r.range(0.1, 0.9));
            self.particles.push(Particle {
                pos: p,
                vel: Vec3::new(r.range(-2.0, 2.0), r.range(0.5, 4.0), r.range(-2.0, 2.0)),
                life: r.range(0.4, 1.0),
                tile,
                uv: [r.int(0, 3) as f32 * 0.25, r.int(0, 3) as f32 * 0.25],
                size: r.range(0.06, 0.14),
                gravity: 18.0,
            });
        }
    }

    fn smoke(&mut self, at: Vec3, n: usize, spread: f32) {
        if self.dedicated {
            return;
        }
        for _ in 0..n {
            let r = &mut self.rng;
            self.particles.push(Particle {
                pos: at + Vec3::new(r.range(-spread, spread), r.range(-spread, spread), r.range(-spread, spread)),
                vel: Vec3::new(r.range(-1.5, 1.5), r.range(0.5, 3.0), r.range(-1.5, 1.5)),
                life: r.range(0.6, 1.6),
                tile: T_CLOUD,
                uv: [0.0, 0.0],
                size: r.range(0.15, 0.45),
                gravity: -1.0,
            });
        }
    }

    pub fn hurt_player(&mut self, amount: f32, cause: &str) {
        if self.creative || self.dead.is_some() || self.player.hurt > 0.0 {
            return;
        }
        self.player.health -= amount;
        self.player.hurt = 0.5;
        self.sfx(Sfx::Hurt, None);
        if self.player.health <= 0.0 {
            self.player.health = 0.0;
            self.dead = Some(format!("Stove {cause}"));
        }
    }

    pub fn explode(&mut self, at: Vec3, r: f32, cause: &str) {
        self.sfx(Sfx::Explode, Some(at));
        self.net_broadcast(Msg::Explosion { at, r });
        self.hurt_peers_in_blast(at, r, cause);
        let c = ivec3(at.x.floor() as i32, at.y.floor() as i32, at.z.floor() as i32);
        let ri = r.ceil() as i32;
        for dy in -ri..=ri {
            for dz in -ri..=ri {
                for dx in -ri..=ri {
                    let p = c + ivec3(dx, dy, dz);
                    let d = (p.as_vec3() + Vec3::splat(0.5)).distance(at);
                    if d > r * self.rng.range(0.7, 1.05) {
                        continue;
                    }
                    let id = self.world.get_v(p);
                    match id {
                        AIR | WATER | BEDROCK => {}
                        TNT => {
                            self.world.set_v(p, AIR);
                            let fuse = self.rng.range(0.3, 0.9);
                            self.tnts.push(PrimedTnt { pos: p.as_vec3(), fuse });
                        }
                        _ => {
                            self.world.set_v(p, AIR);
                            if !self.creative && self.rng.chance(0.25) && block(id).drop != AIR && block(id).pick_tier <= 1 {
                                self.give(block(id).drop, 1);
                            }
                        }
                    }
                }
            }
        }
        self.explosion_effects(at, r);
        let pd = (self.player.body.pos + Vec3::Y * 0.9).distance(at);
        if pd < r * 2.0 && !self.dedicated {
            let dmg = (1.0 - pd / (r * 2.0)) * r * 5.0;
            self.player.hurt = 0.0;
            self.hurt_player(dmg, cause);
            let push = (self.player.body.pos - at).normalize_or_zero() * (1.0 - pd / (r * 2.0)) * 14.0;
            self.player.body.vel += push + Vec3::Y * 4.0;
        }
        for m in self.mobs.iter_mut() {
            let d = (m.body.pos + Vec3::Y * 0.5).distance(at);
            if d < r * 2.0 {
                m.hurt = 0.0;
                m.damage((1.0 - d / (r * 2.0)) * r * 5.0, at);
            }
        }
    }

    /// Mob AI targets and damage recipients: (player id, chest position).
    pub fn player_targets(&self) -> Vec<(u32, Vec3)> {
        let mut t = Vec::new();
        if self.dead.is_none() && !self.dedicated {
            t.push((self.my_id, self.player.body.pos + Vec3::Y * 0.9));
        }
        t.extend(self.peers.iter().filter(|(_, p)| p.alive()).map(|(&id, p)| (id, p.target + Vec3::Y * 0.9)));
        t
    }

    /// Smoke and screen shake (also used when a remote explosion is reported).
    pub fn explosion_effects(&mut self, at: Vec3, r: f32) {
        self.smoke(at, 45, r * 0.6);
        let pd = (self.player.body.pos + Vec3::Y * 0.9).distance(at);
        self.shake = (self.shake + (1.0 - (pd / 40.0).min(1.0)) * 1.2).min(1.5);
    }

    /// Particles and sound for a block someone else changed.
    pub fn block_change_feedback(&mut self, pos: IVec3, old: u8, new: u8) {
        let center = pos.as_vec3() + Vec3::splat(0.5);
        if new == AIR || new == WATER {
            if targetable(old) {
                let tile = block(old).tex[1];
                self.block_particles_tile(pos, tile, 10);
                self.sfx(Sfx::Break(material(old)), Some(center));
            }
        } else {
            self.sfx(Sfx::Place(material(new)), Some(center));
        }
    }

    /// One tick of a headless dedicated server.
    pub fn server_tick(&mut self, dt: f32) {
        self.net_receive(dt);
        self.clock += dt;
        self.time = (self.time + dt / DAY_SECONDS) % 1.0;
        for m in self.messages.iter_mut() {
            m.1 -= dt;
        }
        self.messages.retain(|m| m.1 > 0.0);
        let centers: Vec<(Vec3, i32)> = self.peers.values().map(|p| (p.target, 4)).collect();
        self.world.stream(&centers);
        self.world.dirty.clear(); // nothing to draw
        if !self.peers.is_empty() {
            self.update_entities(dt);
        }
        self.script_tick(dt);
        self.net_send(dt);
        self.sounds.clear();
    }

    fn update_entities(&mut self, dt: f32) {
        if self.is_client() {
            self.client_entities(dt);
            return;
        }
        let ppos = self.player.body.pos + Vec3::Y * 0.9;
        let targets = self.player_targets();
        let visible = !self.creative;
        let daylight = self.daylight();
        let mut events = Vec::new();
        let mut noises = Vec::new();
        for m in self.mobs.iter_mut() {
            let p = m.body.pos;
            if !self.world.is_loaded(p.x.floor() as i32, p.z.floor() as i32) {
                continue;
            }
            let fuse_before = m.fuse;
            // Chase whoever is closest.
            let (target_id, target) = targets
                .iter()
                .copied()
                .min_by(|a, b| a.1.distance_squared(p).total_cmp(&b.1.distance_squared(p)))
                .unwrap_or((u32::MAX, ppos));
            let evs = m.update(dt, &self.world, target, visible && target_id != u32::MAX, daylight, &mut self.rng);
            events.extend(evs.into_iter().map(|e| (target_id, target, e)));
            if fuse_before == 0.0 && m.fuse > 0.0 {
                noises.push((Sfx::Hiss, m.body.pos));
            }
            // Idle chatter.
            if self.rng.chance(dt * 0.1) && targets.iter().any(|t| m.body.pos.distance(t.1) < 20.0) {
                match m.kind {
                    MobKind::Oinker => noises.push((Sfx::Oink, m.body.pos)),
                    MobKind::Groaner => noises.push((Sfx::Groan, m.body.pos)),
                    MobKind::Hisser => {}
                }
            }
        }
        for (s, at) in noises {
            self.sfx(s, Some(at));
        }
        // Keep mobs from stacking inside each other.
        for i in 0..self.mobs.len() {
            for j in i + 1..self.mobs.len() {
                let d = self.mobs[j].body.pos - self.mobs[i].body.pos;
                let flat = Vec3::new(d.x, 0.0, d.z);
                let min = self.mobs[i].body.half + self.mobs[j].body.half;
                let l = flat.length();
                if l < min && l > 1e-4 && d.y.abs() < 1.5 {
                    let push = flat / l * (min - l) * 0.5;
                    self.mobs[i].body.pos -= push;
                    self.mobs[j].body.pos += push;
                }
            }
        }
        for (target_id, target, e) in events {
            match e {
                MobEvent::HurtPlayer(d, cause) if target_id != self.my_id => {
                    self.hurt_peer(target_id, d, cause, Vec3::Y * 3.0);
                    let _ = target;
                }
                MobEvent::HurtPlayer(d, cause) => {
                    self.hurt_player(d, cause);
                    let knock = (self.player.body.pos - ppos).normalize_or_zero();
                    self.player.body.vel += knock * 3.0 + Vec3::Y * 3.0;
                }
                MobEvent::Explode(at, r, cause) => self.explode(at, r, cause),
                MobEvent::Smoke(at) => self.smoke(at, 1, 0.2),
            }
        }
        let mut i = 0;
        while i < self.mobs.len() {
            let m = &self.mobs[i];
            let far = (self.dedicated || m.body.pos.distance(self.player.body.pos) > 110.0) && self.peers.values().all(|p| m.body.pos.distance(p.target) > 110.0);
            if m.health <= 0.0 || far {
                let m = self.mobs.swap_remove(i);
                if m.health <= 0.0 && m.health > -50.0 {
                    let at = m.body.pos + Vec3::Y * 0.5;
                    self.smoke(at, 10, 0.3);
                    let killer = if m.last_attacker == self.my_id && !self.dedicated { self.player_name.clone() } else { self.peers.get(&m.last_attacker).map(|p| p.name.clone()).unwrap_or_default() };
                    let args = vec![m.kind.name().to_ascii_lowercase().into(), (at.x as rhai::FLOAT).into(), (at.y as rhai::FLOAT).into(), (at.z as rhai::FLOAT).into(), killer.into()];
                    self.fire("on_mob_death", args);
                    if let Some((item, n)) = m.loot(&mut self.rng) {
                        if m.last_attacker != self.my_id && self.peers.contains_key(&m.last_attacker) {
                            if !self.creative {
                                self.net_send_to(m.last_attacker, Msg::Give { item, n });
                            }
                        } else if !self.creative {
                            self.give(item, n);
                            self.msg(format!("{} dropped {}x {}", m.kind.name(), n, item_name(item)));
                        }
                    }
                }
                continue;
            }
            i += 1;
        }

        for p in self.particles.iter_mut() {
            p.update(dt, &self.world);
        }
        self.particles.retain(|p| p.life > 0.0);
        if self.particles.len() > 1500 {
            let n = self.particles.len() - 1500;
            self.particles.drain(0..n);
        }

        for t in self.tnts.iter_mut() {
            t.fuse -= dt;
        }
        let boom: Vec<Vec3> = self.tnts.iter().filter(|t| t.fuse <= 0.0).map(|t| t.pos + Vec3::splat(0.5)).collect();
        self.tnts.retain(|t| t.fuse > 0.0);
        for at in boom {
            self.explode(at, 4.0, "went out with a bang (TNT)");
        }

        self.spawn_timer -= dt;
        if self.spawn_timer <= 0.0 {
            self.spawn_timer = 1.0;
            self.try_spawn();
        }
    }

    fn alloc_mob(&mut self, kind: MobKind, pos: Vec3) {
        let mut m = Mob::new(kind, pos, &mut self.rng);
        m.id = self.next_mob_id;
        self.next_mob_id += 1;
        self.mobs.push(m);
    }

    fn try_spawn(&mut self) {
        // Spawn around a random player so everyone gets company.
        let mut centers = if self.dedicated { vec![] } else { vec![self.player.body.pos] };
        centers.extend(self.peers.values().map(|p| p.target));
        if centers.is_empty() {
            return;
        }
        let p = centers[self.rng.int(0, centers.len() as i32 - 1) as usize];
        let passive = self.mobs.iter().filter(|m| !m.kind.hostile()).count();
        let hostile = self.mobs.len() - passive;
        let a = self.rng.range(0.0, TAU);
        let d = self.rng.range(24.0, 56.0);
        let (x, z) = ((p.x + a.cos() * d).floor() as i32, (p.z + a.sin() * d).floor() as i32);
        if !self.world.is_loaded(x, z) {
            return;
        }
        let y = self.world.surface_y(x, z);
        let top = self.world.get(x, y, z);
        let clear = |w: &World, y: i32| !is_solid(w.get(x, y, z)) && !is_solid(w.get(x, y + 1, z)) && w.get(x, y, z) != WATER;
        if !self.is_night() && passive < 8 && top == GRASS && clear(&self.world, y + 1) {
            for i in 0..self.rng.int(1, 3) {
                let pos = Vec3::new(x as f32 + 0.5 + i as f32 * 0.7, y as f32 + 1.0, z as f32 + 0.5);
                self.alloc_mob(MobKind::Oinker, pos);
            }
            return;
        }
        if hostile >= 12 + 4 * self.peers.len() {
            return;
        }
        let kind = if self.rng.chance(0.45) { MobKind::Hisser } else { MobKind::Groaner };
        if self.is_night() && is_solid(top) && clear(&self.world, y + 1) {
            let pos = Vec3::new(x as f32 + 0.5, y as f32 + 1.0, z as f32 + 0.5);
            self.alloc_mob(kind, pos);
            return;
        }
        // Caves are always spooky.
        let cy = self.rng.int(4, y.max(5));
        if cy + 1 < y && is_solid(self.world.get(x, cy - 1, z)) && clear(&self.world, cy) && self.world.sky_light(x, cy, z) < 0.15 {
            let pos = Vec3::new(x as f32 + 0.5, cy as f32, z as f32 + 0.5);
            self.alloc_mob(kind, pos);
        }
    }

    pub fn respawn(&mut self) {
        self.dead = None;
        self.player = Player::new(self.spawn);
        if self.net.is_none() {
            self.mobs.retain(|m| !m.kind.hostile());
        }
        self.ready = false;
        self.msg("Respawned. Inventory kept, because we're nice.");
    }

    /// Build all per-frame geometry: sky, clouds, entities, highlights, hand.
    pub fn build_geo(&self, cam: &Camera, render_distance: i32) -> DynGeo {
        let mut g = DynGeo::default();
        let eye = cam.pos;
        let a = self.sun_angle();
        let sun_dir = Vec3::new(a.cos(), a.sin(), 0.25).normalize();

        // Stars fade in at night.
        let night = 1.0 - ((self.daylight() - 0.18) / 0.5).clamp(0.0, 1.0);
        if night > 0.01 {
            g.begin(Pass::Sky, [1.0, 1.0, 1.0, night], true);
            let rot = Mat4::from_rotation_z(a);
            for (i, s) in self.stars.iter().enumerate() {
                let d = rot.transform_vector3(*s);
                let size = 0.25 + (i % 3) as f32 * 0.12;
                sky_quad(&mut g, eye + d * 150.0, d, size, T_WHITE);
            }
        }
        g.begin(Pass::Sky, [1.0; 4], true);
        sky_quad(&mut g, eye + sun_dir * 150.0, sun_dir, 16.0, T_SUN);
        sky_quad(&mut g, eye - sun_dir * 150.0, -sun_dir, 11.0, T_MOON);

        // Clouds: a scrolling blocky layer.
        let cloud_y = 112.0;
        let cell = 12.0;
        let scroll = self.clock * 1.2 + self.time * DAY_SECONDS;
        let (ox, oz) = ((eye.x + scroll) / cell, eye.z / cell);
        let reach = ((render_distance * 16) as f32 / cell) as i32 + 4;
        g.begin(Pass::Blend, [1.0, 1.0, 1.0, 0.82], false);
        for j in -reach..=reach {
            for i in -reach..=reach {
                let (ci, cj) = (ox.floor() as i32 + i, oz.floor() as i32 + j);
                if self.clouds.noise2(ci as f32 * 0.17, cj as f32 * 0.17) + hash2(7, ci, cj) * 0.12 < 0.12 {
                    continue;
                }
                let x0 = ci as f32 * cell - scroll;
                let z0 = cj as f32 * cell;
                let c = [
                    Vec3::new(x0, cloud_y, z0 + cell),
                    Vec3::new(x0 + cell, cloud_y, z0 + cell),
                    Vec3::new(x0 + cell, cloud_y, z0),
                    Vec3::new(x0, cloud_y, z0),
                ];
                g.quad(c, T_CLOUD, [0.0, 0.0, 1.0, 1.0], [1.0, 1.0]);
            }
        }

        // Mobs
        for m in &self.mobs {
            if m.body.pos.distance(eye) < (render_distance * 16) as f32 {
                m.draw(&mut g, &self.world);
            }
        }
        // Other players
        for p in self.peers.values().filter(|p| p.alive()) {
            let sky = self.world.sky_light(p.pos.x.floor() as i32, (p.pos.y + 1.0).floor() as i32, p.pos.z.floor() as i32);
            let tint = if p.flags & crate::net::FLAG_HURT != 0 { [1.0, 0.5, 0.5, 1.0] } else { [1.0; 4] };
            g.begin(Pass::Opaque, tint, false);
            let sneak = if p.flags & crate::net::FLAG_SNEAK != 0 { 0.12 } else { 0.0 };
            let root = Mat4::from_translation(p.pos - Vec3::Y * sneak) * Mat4::from_rotation_y(-p.yaw);
            draw_model(&mut g, &root, &STOVE, p.anim, sky, true);
        }
        // The player, in third person
        if self.third_person && !self.menu {
            let p = &self.player;
            let sky = self.world.sky_light(p.body.pos.x.floor() as i32, (p.body.pos.y + 1.0).floor() as i32, p.body.pos.z.floor() as i32);
            let tint = if p.hurt > 0.3 { [1.0, 0.5, 0.5, 1.0] } else { [1.0; 4] };
            g.begin(Pass::Opaque, tint, false);
            let root = Mat4::from_translation(p.body.pos) * Mat4::from_rotation_y(-p.yaw);
            draw_model(&mut g, &root, &STOVE, p.bob * 2.0, sky, true);
        }
        // Primed TNT
        for t in &self.tnts {
            let flash = (t.fuse * 8.0).sin() > 0.0;
            g.begin(Pass::Opaque, if flash { [3.0, 3.0, 3.0, 1.0] } else { [1.0; 4] }, false);
            let s = 1.0 + (1.0 - t.fuse.min(1.0)) * 0.15;
            let m = Mat4::from_translation(t.pos + Vec3::splat(0.5)) * Mat4::from_scale(Vec3::splat(s)) * Mat4::from_translation(Vec3::splat(-0.5));
            let sky = self.world.sky_light(t.pos.x as i32, t.pos.y as i32 + 1, t.pos.z as i32);
            g.cube(&m, [T_TNT_SIDE, T_TNT_SIDE, T_TNT_TOP, T_TNT_BOTTOM, T_TNT_SIDE, T_TNT_SIDE], sky, [0.0, 0.0, 1.0, 1.0]);
        }
        // Particles
        g.begin(Pass::Opaque, [1.0; 4], false);
        for p in &self.particles {
            p.draw(&mut g, &self.world);
        }

        if self.menu || self.dead.is_some() {
            return g;
        }

        // Block highlight and cracks
        if let Some(Target::Block(h)) = &self.target {
            let id = self.world.get_v(h.pos);
            let (min, max) = if block(id).model == Model::Cross {
                (h.pos.as_vec3() + Vec3::new(0.2, 0.0, 0.2), h.pos.as_vec3() + Vec3::new(0.8, 0.8, 0.8))
            } else {
                (h.pos.as_vec3(), h.pos.as_vec3() + Vec3::ONE)
            };
            g.begin(Pass::Blend, [0.0, 0.0, 0.0, 0.55], true);
            // Edges sit entirely outside the block so the (now depth-tested) outline never z-fights.
            outline(&mut g, min - Vec3::splat(0.014), max + Vec3::splat(0.014), 0.012);
            if let Some((bp, prog)) = self.breaking {
                if bp == h.pos {
                    let stage = ((prog * 5.0) as u16).min(4);
                    g.begin(Pass::Blend, [1.0; 4], false);
                    let m = Mat4::from_translation(min - Vec3::splat(0.006)) * Mat4::from_scale(max - min + Vec3::splat(0.012));
                    g.cube(&m, [T_CRACK0 + stage; 6], 1.0, [0.0, 0.0, 1.0, 1.0]);
                }
            }
        }

        if !self.third_person {
            self.draw_hand(&mut g, cam);
        }
        g
    }

    fn draw_hand(&self, g: &mut DynGeo, cam: &Camera) {
        let fwd = cam.dir;
        let right = fwd.cross(Vec3::Y).normalize();
        let up = right.cross(fwd);
        let basis = Mat4::from_cols(right.extend(0.0), up.extend(0.0), (-fwd).extend(0.0), cam.pos.extend(1.0));
        let e = self.player.eye();
        let sky = self.world.sky_light(e.x.floor() as i32, e.y.floor() as i32, e.z.floor() as i32);
        let s = self.player.swing;
        let swing = (s * PI).sin();
        let bob = self.player.bob;
        let local = Mat4::from_translation(Vec3::new(0.42 - swing * 0.15, -0.42 + (bob * 2.0).sin().abs() * 0.03 + swing * 0.12, -0.72 - swing * 0.1))
            * Mat4::from_rotation_x(-swing * 0.9);
        let held = self.inv.held();
        let tint = if self.player.hurt > 0.3 { [1.0, 0.6, 0.6, 1.0] } else { [1.0; 4] };
        g.begin(Pass::Overlay, tint, false);
        if held == AIR {
            // A short forearm poking in from the bottom-right corner, angled up and inward,
            // with a shirt sleeve at the near end so it reads as an arm rather than a plank.
            let arm = basis * local * Mat4::from_translation(Vec3::new(0.1, -0.1, 0.1)) * Mat4::from_rotation_y(0.35) * Mat4::from_rotation_x(0.6);
            let w = 0.16;
            let sleeve = arm * Mat4::from_translation(Vec3::new(-w / 2.0 - 0.006, -w / 2.0 - 0.006, 0.0)) * Mat4::from_scale(Vec3::new(w + 0.012, w + 0.012, 0.25));
            let hand = arm * Mat4::from_translation(Vec3::new(-w / 2.0, -w / 2.0, -0.3)) * Mat4::from_scale(Vec3::new(w, w, 0.3));
            let light = sky.max(0.2);
            g.cube(&sleeve, [T_STOVE_SHIRT; 6], light, [0.0, 0.0, 1.0, 1.0]);
            g.cube(&hand, [T_SKIN; 6], light, [0.0, 0.0, 1.0, 1.0]);
        } else if is_block_item(held) && block(held).model == Model::Cube {
            let tiles = {
                let t = block(held).tex;
                [t[1], t[1], t[0], t[2], t[1], t[1]]
            };
            let m = basis * local * Mat4::from_rotation_y(0.75) * Mat4::from_translation(Vec3::splat(-0.14)) * Mat4::from_scale(Vec3::splat(0.28));
            g.cube(&m, tiles, sky.max(0.2), [0.0, 0.0, 1.0, 1.0]);
        } else {
            let tile = if is_block_item(held) { block(held).tex[1] } else { item_tile(held) };
            let m = basis * local * Mat4::from_rotation_y(-0.5) * Mat4::from_rotation_z(0.2);
            let s = 0.42;
            let c = [Vec3::new(-s / 2.0, -s / 2.0, 0.0), Vec3::new(s / 2.0, -s / 2.0, 0.0), Vec3::new(s / 2.0, s / 2.0, 0.0), Vec3::new(-s / 2.0, s / 2.0, 0.0)].map(|p| m.transform_point3(p));
            g.quad(c, tile, [0.0, 0.0, 1.0, 1.0], [1.0, sky.max(0.2)]);
            g.quad([c[1], c[0], c[3], c[2]], tile, [1.0, 0.0, 0.0, 1.0], [1.0, sky.max(0.2)]);
        }
    }

    pub fn frame_params(&self, cam: &Camera, renderer: &Renderer, render_distance: i32) -> FrameParams {
        let underwater = !self.menu && self.player.head_in_water(&self.world);
        let sky = self.sky_color();
        let far = (render_distance * 16) as f32;
        let (fog_color, fog_start, fog_end) = if underwater {
            ([0.05, 0.12, 0.35], 0.0, 22.0)
        } else {
            (sky, far * 0.55, far - 4.0)
        };
        // Held torches glow too.
        let mut extra = Vec::new();
        let held = self.inv.held();
        if !self.menu && is_block_item(held) && block(held).light > 0.0 {
            let e = self.player.eye();
            extra.push([e.x, e.y, e.z, block(held).light * 0.8]);
        }
        let lights: [Vec4; 16] = renderer.nearby_lights(cam.pos, &extra);
        FrameParams { view_proj: cam.view_proj, cam_pos: cam.pos, fog_color, fog_start, fog_end, daylight: self.daylight(), lights }
    }
}

/// Names of every mod-added block and item, so saves survive mods being added or removed.
fn mod_palette(r: &Registry) -> Vec<(u8, String)> {
    let blocks = (NUM_BLOCKS..r.blocks.len() as u8).map(|id| (id, r.blocks[id as usize].key.to_string()));
    let items = (FIRST_MOD_ITEM as usize..FIRST_ITEM as usize + r.items.len()).map(|id| (id as u8, r.key_of(id as u8).to_string()));
    blocks.chain(items).collect()
}

/// Map ids in a save to ids in the current registry. Mod things that no longer
/// exist become air (blocks) or vanish (items). None when nothing needs changing.
fn palette_remap(r: &Registry, palette: &[(u8, String)]) -> Option<Vec<u8>> {
    let mut map: Vec<u8> = (0..=255u8).collect();
    // Any mod-range id the save doesn't mention is unknown.
    for id in NUM_BLOCKS..FIRST_ITEM {
        map[id as usize] = AIR;
    }
    for id in FIRST_MOD_ITEM..=255 {
        map[id as usize] = AIR;
    }
    for (old, key) in palette {
        map[*old as usize] = r.lookup(key).unwrap_or(AIR);
    }
    // Only the ids the save actually uses matter.
    let unchanged = palette.iter().all(|(old, key)| r.lookup(key) == Some(*old));
    (!unchanged).then_some(map)
}

fn sky_quad(g: &mut DynGeo, center: Vec3, dir: Vec3, size: f32, tile: u16) {
    let u = Vec3::Z.cross(dir).normalize_or_zero();
    let u = if u.length_squared() < 0.5 { Vec3::X } else { u };
    let v = dir.cross(u);
    let c = [center - u * size - v * size, center + u * size - v * size, center + u * size + v * size, center - u * size + v * size];
    g.quad(c, tile, [0.0, 0.0, 1.0, 1.0], [1.0, 1.0]);
}

/// Twelve thin boxes along the edges of an AABB.
fn outline(g: &mut DynGeo, min: Vec3, max: Vec3, t: f32) {
    let s = max - min;
    let edges: [(Vec3, Vec3); 12] = [
        (Vec3::new(0.0, 0.0, 0.0), Vec3::new(s.x, t, t)),
        (Vec3::new(0.0, s.y - t, 0.0), Vec3::new(s.x, t, t)),
        (Vec3::new(0.0, 0.0, s.z - t), Vec3::new(s.x, t, t)),
        (Vec3::new(0.0, s.y - t, s.z - t), Vec3::new(s.x, t, t)),
        (Vec3::new(0.0, 0.0, 0.0), Vec3::new(t, s.y, t)),
        (Vec3::new(s.x - t, 0.0, 0.0), Vec3::new(t, s.y, t)),
        (Vec3::new(0.0, 0.0, s.z - t), Vec3::new(t, s.y, t)),
        (Vec3::new(s.x - t, 0.0, s.z - t), Vec3::new(t, s.y, t)),
        (Vec3::new(0.0, 0.0, 0.0), Vec3::new(t, t, s.z)),
        (Vec3::new(s.x - t, 0.0, 0.0), Vec3::new(t, t, s.z)),
        (Vec3::new(0.0, s.y - t, 0.0), Vec3::new(t, t, s.z)),
        (Vec3::new(s.x - t, s.y - t, 0.0), Vec3::new(t, t, s.z)),
    ];
    for (o, sz) in edges {
        let m = Mat4::from_translation(min + o) * Mat4::from_scale(sz);
        g.cube(&m, [T_WHITE; 6], 1.0, [0.0, 0.0, 1.0, 1.0]);
    }
}

/// Slab-method ray/AABB test; returns entry distance.
fn ray_aabb(o: Vec3, d: Vec3, min: Vec3, max: Vec3) -> Option<f32> {
    let (mut t0, mut t1) = (0.0f32, f32::MAX);
    for i in 0..3 {
        if d[i].abs() < 1e-8 {
            if o[i] < min[i] || o[i] > max[i] {
                return None;
            }
            continue;
        }
        let inv = 1.0 / d[i];
        let (mut a, mut b) = ((min[i] - o[i]) * inv, (max[i] - o[i]) * inv);
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
        t0 = t0.max(a);
        t1 = t1.min(b);
        if t0 > t1 {
            return None;
        }
    }
    Some(t0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::Inventory;

    fn loaded_world(seed: u32) -> World {
        let mut w = World::new(seed);
        let start = std::time::Instant::now();
        while w.chunks.len() < 9 && start.elapsed().as_secs() < 20 {
            w.stream(&[(Vec3::ZERO, 1)]);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        w
    }

    #[test]
    fn generation_is_deterministic() {
        let g = crate::world::Generator::new(99);
        assert_eq!(g.generate(3, -2), g.generate(3, -2));
        assert_ne!(g.generate(0, 0), crate::world::Generator::new(100).generate(0, 0));
    }

    #[test]
    fn raycast_hits_ground_and_edits_persist() {
        let mut w = loaded_world(7);
        let top = w.surface_y(4, 4);
        let hit = w.raycast(Vec3::new(4.5, top as f32 + 3.5, 4.5), -Vec3::Y, 10.0).expect("should hit terrain");
        assert_eq!(hit.normal, IVec3::Y);
        assert_eq!(hit.pos.y, top);
        w.set(4, top + 1, 4, BRICK);
        assert_eq!(w.get(4, top + 1, 4), BRICK);
        assert_eq!(w.mods.values().map(|m| m.len()).sum::<usize>(), 1);
    }

    #[test]
    fn crafting_consumes_inputs() {
        let mut inv = Inventory::new();
        inv.add(LOG, 1);
        let planks = recipes().iter().find(|r| r.output.0 == PLANKS).unwrap();
        assert!(inv.craft(planks));
        assert_eq!(inv.count(LOG), 0);
        assert_eq!(inv.count(PLANKS), 4);
        assert!(!inv.craft(planks));
        assert_eq!(inv.add(PICK_WOOD, 1), 0);
        assert_eq!(inv.count(PICK_WOOD), 1);
    }

    #[test]
    fn pickaxe_tiers_gate_drops() {
        assert!(!break_time(DIAMOND_ORE, PICK_STONE).1);
        assert!(break_time(DIAMOND_ORE, PICK_IRON).1);
        assert!(!break_time(STONE, AIR).1);
        assert!(break_time(STONE, PICK_WOOD).0 < break_time(STONE, AIR).0);
        assert!(break_time(BEDROCK, PICK_DIAMOND).0.is_infinite());
    }

    #[test]
    fn explosions_make_craters() {
        let mut g = Game::new(5, true, false);
        g.world = loaded_world(5);
        let top = g.world.surface_y(0, 0);
        let solid_before = (-2..=2).flat_map(|x| (-2..=2).map(move |z| (x, z))).filter(|&(x, z)| is_solid(g.world.get(x, top - 1, z))).count();
        g.explode(Vec3::new(0.5, top as f32, 0.5), 3.0, "test");
        let solid_after = (-2..=2).flat_map(|x| (-2..=2).map(move |z| (x, z))).filter(|&(x, z)| is_solid(g.world.get(x, top - 1, z))).count();
        assert!(solid_after < solid_before);
    }

    #[test]
    fn saves_survive_mods_changing() {
        use crate::mods::{build, ModSource};
        let mk = |text: &str| {
            let mut files = std::collections::BTreeMap::new();
            files.insert("mod.txt".to_string(), text.as_bytes().to_vec());
            files
        };
        let a = ModSource { id: "aaa".into(), files: mk("[block one]\n[block two]\n[item gem]\n") };
        let b = ModSource { id: "bbb".into(), files: mk("[block red]\n") };
        let with_both = build(&[a.clone(), b.clone()], &[]);
        let palette = mod_palette(&with_both);
        let red = with_both.lookup("bbb:red").unwrap();
        let gem = with_both.lookup("aaa:gem").unwrap();
        assert_eq!(red, NUM_BLOCKS + 2);

        // Same mods: nothing to do.
        assert!(palette_remap(&with_both, &palette).is_none());
        // Mod "aaa" removed: "red" moves down to the first mod slot, aaa's things vanish.
        let only_b = build(&[b], &[]);
        let map = palette_remap(&only_b, &palette).unwrap();
        assert_eq!(map[red as usize], NUM_BLOCKS);
        assert_eq!(map[(NUM_BLOCKS) as usize], AIR);
        assert_eq!(map[gem as usize], AIR);
        assert_eq!(map[STONE as usize], STONE);
        assert_eq!(map[DIAMOND as usize], DIAMOND);
    }

    #[test]
    fn mod_actions_do_things() {
        let mut g = Game::new(9, false, false);
        g.world = loaded_world(9);
        g.player.health = 5.0;
        g.run_actions(&[Action::Heal(4.0), Action::Launch(15.0), Action::Give(DIAMOND, 2), Action::Message("hi")], g.spawn);
        assert_eq!(g.player.health, 9.0);
        assert_eq!(g.player.body.vel.y, 15.0);
        assert_eq!(g.inv.count(DIAMOND), 2);
        assert!(g.messages.iter().any(|m| m.0 == "hi"));
        let top = g.world.surface_y(0, 0);
        let solid = |g: &Game| (-1..=1).filter(|&x| is_solid(g.world.get(x, top, 0))).count();
        let before = solid(&g);
        g.run_actions(&[Action::Explode(3.0)], Vec3::new(0.5, top as f32 + 0.5, 0.5));
        assert!(solid(&g) < before);
    }

    #[test]
    fn scripts_drive_the_game() {
        use crate::mods::ModSource;
        let mut files = std::collections::BTreeMap::new();
        files.insert(
            "main.rhai".to_string(),
            br#"
            fn on_load() { set_var("loaded", true); }
            fn on_chat(player, text) {
                if text == "/tower" {
                    let p = player_pos(player);
                    fill(p[0] + 2, p[1], p[2], p[0] + 2, p[1] + 4, p[2], "glass");
                    give(player, "diamond", 5);
                    message(player, "Tower built!");
                    return false;
                }
                if text == "/loaded" { message(player, `loaded=${get_var("loaded")}`); return false; }
            }
            fn on_block_break(player, x, y, z, block) {
                if block == "glass" { message(player, "The glass is protected."); return false; }
            }
            fn on_use_item(player, item) { if item == "diamond" { launch(player, 20); } }
            "#
            .to_vec(),
        );
        let (host, problems) = ScriptHost::new(&[ModSource { id: "cmds".into(), files }]);
        assert!(problems.is_empty(), "{problems:?}");
        let mut g = Game::new(21, false, false);
        g.world = loaded_world(21);
        let top = g.world.surface_y(0, 0);
        g.player.body.pos = Vec3::new(0.5, top as f32 + 1.0, 0.5);
        g.scripts = Some(host);
        g.fire("on_load", vec![]);

        g.send_chat("/tower");
        for dy in 0..5 {
            assert_eq!(g.world.get(2, top + 1 + dy, 0), GLASS);
        }
        assert_eq!(g.inv.count(DIAMOND), 5);
        assert!(g.messages.iter().any(|m| m.0 == "Tower built!"));
        // The command wasn't echoed as chat.
        assert!(!g.messages.iter().any(|m| m.0.contains("/tower")));
        g.send_chat("/loaded");
        assert!(g.messages.iter().any(|m| m.0 == "loaded=true"));
        g.send_chat("/nope");
        assert!(g.messages.iter().any(|m| m.0.starts_with("Unknown command /nope")));

        // Cancelling a block break.
        g.break_block(ivec3(2, top + 1, 0), true);
        assert_eq!(g.world.get(2, top + 1, 0), GLASS);
        // Item use.
        g.inv.selected = g.inv.slots.iter().position(|s| s.map(|s| s.0) == Some(DIAMOND)).unwrap();
        g.use_item();
        assert_eq!(g.player.body.vel.y, 20.0);
    }

    #[test]
    fn script_variables_survive_saving_and_loading() {
        use crate::mods::ModSource;
        let mut files = std::collections::BTreeMap::new();
        files.insert(
            "main.rhai".to_string(),
            br#"
            fn on_load() {
                let loads = get_var("loads");
                if loads == () { loads = 0; }
                set_var("loads", loads + 1);
            }
            fn on_chat(player, text) {
                if text == "/sethome" { set_var(`home_${player}`, [10.5, 70.0, -4.5]); return false; }
                if text == "/home" {
                    let h = get_var(`home_${player}`);
                    if h == () { message(player, "no home"); } else { teleport(player, h[0], h[1], h[2]); }
                    message(player, `loads=${get_var("loads")}`);
                    return false;
                }
            }
            "#
            .to_vec(),
        );
        let src = vec![ModSource { id: "homes".into(), files }];
        let dir = std::env::temp_dir().join(format!("minceraft-vars-{}", std::process::id()));
        let path = dir.join("world.mncr");

        let mut g = Game::new(33, false, false);
        g.start_scripts_with(&src);
        g.send_chat("/sethome");
        crate::save::write_to(&path, &g.to_save()).unwrap();

        // Reopen the world: the home and the load counter are still there.
        let mut back = Game::from_save(crate::save::read_from(&path).unwrap());
        back.start_scripts_with(&src);
        back.send_chat("/home");
        assert_eq!(back.player.body.pos, Vec3::new(10.5, 70.0, -4.5));
        assert!(back.messages.iter().any(|m| m.0 == "loads=2"), "{:?}", back.messages);

        // Opened once with the mod switched off, then saved: the data must survive.
        let mut without = Game::from_save(crate::save::read_from(&path).unwrap());
        without.start_scripts_with(&[]);
        crate::save::write_to(&path, &without.to_save()).unwrap();
        let mut again = Game::from_save(crate::save::read_from(&path).unwrap());
        again.start_scripts_with(&src);
        again.send_chat("/home");
        // Loaded by the first save (1), the mod-less session didn't run on_load, now 2.
        assert!(again.messages.iter().any(|m| m.0 == "loads=2"), "{:?}", again.messages);
        assert_eq!(again.player.body.pos, Vec3::new(10.5, 70.0, -4.5));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn save_round_trip() {
        let dir = std::env::temp_dir().join(format!("minceraft-test-{}", std::process::id()));
        let path = dir.join("world.mncr");
        let mut g = Game::new(1234, false, false);
        g.inv.add(DIAMOND, 7);
        g.world.mods.entry((2, -3)).or_default().insert(99, TNT);
        g.time = 0.4;
        crate::save::write_to(&path, &g.to_save()).unwrap();
        let back = Game::from_save(crate::save::read_from(&path).unwrap());
        assert_eq!(back.world.seed(), 1234);
        assert_eq!(back.inv.count(DIAMOND), 7);
        assert_eq!(back.world.mods[&(2, -3)][&99], TNT);
        assert!((back.time - 0.4).abs() < 1e-6);
        assert!(!back.creative);
        std::fs::remove_dir_all(&dir).ok();
    }
}
