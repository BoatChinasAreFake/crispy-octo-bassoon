//! MINCERAFT — a native, browser-free block game parody.
//! Rust + raw OpenGL (via miniquad/macroquad). No asset files: everything is
//! generated at startup.

mod advancements;
mod block;
mod entity;
mod game;
mod inventory;
mod mesher;
mod mods;
mod multiplayer;
mod net;
mod noise;
mod player;
mod render;
mod save;
mod scripting;
mod server;
mod upnp;
mod sound;
mod texture;
mod ui;
mod world;

use block::*;
use game::{Controls, Game};
use macroquad::prelude::*;
use player::Input;
use render::Renderer;
use sound::{Audio, Sfx};
use ui::Ui;

const SPLASHES: &[&str] = &[
    "100% browser-free!",
    "Compiled, not interpreted!",
    "Now with 0 image files!",
    "Not affiliated with anyone!",
    "Punch trees! (Legally distinct)",
    "Hisss... boom.",
    "Dimonds are forever-ish!",
    "Contains Oinkers!",
    "Crafting tables sold separately (they're decorative)!",
    "Made of cubes!",
    "Legally distinct from everything!",
    "Mince responsibly!",
    "Water physics: on vacation!",
    "Tested on at least one computer!",
    "Now in 3D!",
    "Stove approves!",
    "Groaners hate sunlight!",
    "Also try: going outside!",
    "Don't look at the Starers!",
    "Fluffers: ethically sheared!",
    "The cake is NOT a lie!",
    "Beds skip nights! Budget cuts shrank them!",
    "Gold: shiny, useless, beloved!",
    "Advancement made: reading this!",
    "Do not hug the Pokey Plant!",
    "Now 34% more parody!",
];

#[derive(Clone, Copy, PartialEq)]
enum Screen {
    Title,
    Playing,
    Paused,
    Inventory,
    Dead,
    Options { from_title: bool },
    Help { from_title: bool },
    Advancements,
    Multiplayer,
    Mods,
    Worlds,
    CreateWorld,
    RenameWorld,
    DeleteWorld,
}

struct Settings {
    render_distance: i32,
    fov: f32,
    sensitivity: f32,
    fullscreen: bool,
}

struct App {
    screen: Screen,
    game: Game,
    renderer: Renderer,
    ui: Ui,
    settings: Settings,
    splash: &'static str,
    last_mouse: Option<Vec2>,
    show_debug: bool,
    recipe_scroll: f32,
    quit: bool,
    status: Option<(String, f32)>,
    fps: f32,
    audio: Audio,
    // ---- multiplayer
    mp_name: String,
    mp_addr: String,
    mp_focus: usize,
    /// Address to connect to on the next frame (so "Connecting..." gets drawn first).
    connect_next: Option<String>,
    /// Connected, waiting for the host's Welcome.
    joining: Option<(net::Conn, f64)>,
    /// Chat line being typed, if the chat box is open.
    chat: Option<String>,
    last_view_proj: Mat4,
    lan_addr: Option<String>,
    /// Password for joining, and for hosting if set.
    mp_password: String,
    /// UPnP request running in the background.
    upnp_job: Option<std::sync::mpsc::Receiver<Result<upnp::Mapping, String>>>,
    upnp_mapping: Option<upnp::Mapping>,
    internet_status: Option<String>,
    // ---- mods
    /// The procedurally painted base atlas; mod textures are layered on a copy.
    base_atlas: Vec<u8>,
    /// Registry generation the GPU atlas was built from.
    atlas_gen: u32,
    mods_scroll: usize,
    adv_scroll: usize,
    /// Joined a server, so its mods (not ours) are active.
    using_server_mods: bool,
    // ---- world slots
    /// Folder id of the world being played (where Save writes).
    current_world: Option<String>,
    worlds: Vec<save::WorldEntry>,
    world_sel: Option<usize>,
    world_scroll: usize,
    last_click: (usize, f64),
    form_name: String,
    form_seed: String,
    form_creative: bool,
    form_focus: usize,
}

/// A random splash text, including ones added by mods.
fn pick_splash() -> &'static str {
    let extra = &block::reg().splashes;
    let n = SPLASHES.len() + extra.len();
    let i = (random_seed() as usize) % n;
    if i < SPLASHES.len() { SPLASHES[i] } else { extra[i - SPLASHES.len()].as_str() }
}

/// Feed typed characters into a text buffer.
fn type_into(buf: &mut String, max: usize) {
    while let Some(c) = get_char_pressed() {
        if !c.is_control() && buf.chars().count() < max {
            buf.push(c);
        }
    }
    if is_key_pressed(KeyCode::Backspace) {
        buf.pop();
    }
}

fn players(n: usize) -> String {
    if n == 1 { "1 player".into() } else { format!("{n} players") }
}

fn drain_chars() {
    while get_char_pressed().is_some() {}
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Minceraft".to_owned(),
        window_width: 1280,
        window_height: 720,
        high_dpi: false,
        sample_count: 4,
        window_resizable: true,
        ..Default::default()
    }
}

fn random_seed() -> u32 {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(1);
    (t as u64 ^ (t >> 64) as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) as u32
}

fn grab(on: bool) {
    set_cursor_grab(on);
    show_mouse(!on);
}

impl App {
    fn set_screen(&mut self, s: Screen) {
        let playing = s == Screen::Playing && self.chat.is_none();
        grab(playing);
        self.last_mouse = None;
        if matches!(self.screen, Screen::Inventory) && s != Screen::Inventory {
            self.game.inv.return_cursor();
        }
        self.screen = s;
    }

    fn start_game(&mut self, game: Game) {
        let mut gl = unsafe { get_internal_gl() };
        gl.flush();
        self.renderer.clear(gl.quad_context);
        self.game = game;
        self.set_screen(Screen::Playing);
    }

    fn open_worlds(&mut self) {
        let root = save::saves_dir();
        match save::migrate_legacy(&root) {
            Ok(Some(_)) => self.status = Some(("Your old save is now the world \"My World\".".into(), 6.0)),
            Ok(None) => {}
            Err(e) => self.status = Some((format!("Couldn't move your old save: {e}"), 6.0)),
        }
        self.worlds = save::list_worlds(&root);
        self.world_sel = if self.worlds.is_empty() { None } else { Some(0) };
        self.world_scroll = 0;
        self.set_screen(Screen::Worlds);
    }

    fn play_world(&mut self, i: usize) {
        let Some(w) = self.worlds.get(i) else { return };
        let id = w.id.clone();
        match save::read_from(&save::world_file(&save::saves_dir(), &id)) {
            Ok(d) => {
                let mut g = Game::from_save(d);
                g.start_scripts();
                self.current_world = Some(id);
                self.start_game(g);
            }
            Err(e) => self.status = Some((format!("Couldn't load \"{}\": {e}", w.name), 6.0)),
        }
    }

    fn create_world(&mut self) {
        let root = save::saves_dir();
        let name = save::clean_name(&self.form_name);
        let id = save::new_world_id(&root, &name);
        if let Err(e) = save::write_name(&root, &id, &name) {
            self.status = Some((format!("Couldn't create the world: {e}"), 6.0));
            return;
        }
        let seed = save::parse_seed(&self.form_seed, random_seed());
        self.current_world = Some(id);
        self.new_world(self.form_creative, seed);
        // Save straight away so it's in the list even if the game is closed abruptly.
        self.save_quietly();
    }

    fn new_world(&mut self, creative: bool, seed: u32) {
        let mut g = Game::new(seed, creative, false);
        g.msg(if creative {
            "Creative mode: infinite blocks, zero consequences. Double-tap Space to fly."
        } else {
            "Survival mode: punch a tree. Press E to craft. Avoid anything that hisses."
        });
        g.start_scripts();
        self.start_game(g);
    }

    fn back_to_title(&mut self) {
        if !self.game.menu && !self.game.is_client() {
            self.save();
        }
        self.game.disconnect();
        self.current_world = None;
        self.chat = None;
        self.lan_addr = None;
        self.internet_status = None;
        self.upnp_job = None;
        if let Some(m) = self.upnp_mapping.take() {
            // Tidy up the router's port forward without blocking the menu.
            std::thread::spawn(move || upnp::close(&m));
        }
        let mut gl = unsafe { get_internal_gl() };
        gl.flush();
        self.renderer.clear(gl.quad_context);
        if self.using_server_mods {
            // Back to our own mods after playing on someone else's server.
            self.using_server_mods = false;
            mods::install_local();
        }
        self.game = Game::new(random_seed(), true, true);
        self.splash = pick_splash();
        self.set_screen(Screen::Title);
    }

    fn save(&mut self) {
        if self.game.is_client() {
            self.status = Some(("Only the host can save this world.".into(), 3.0));
            return;
        }
        match self.write_current_world() {
            Some(Ok(())) => self.status = Some(("World saved.".into(), 3.0)),
            Some(Err(e)) => self.status = Some((format!("Save failed: {e}"), 6.0)),
            None => {}
        }
    }

    fn save_quietly(&mut self) {
        if let Some(Err(e)) = self.write_current_world() {
            self.status = Some((format!("Save failed: {e}"), 6.0));
        }
    }

    /// Write the game into its world slot (None if it has no slot, e.g. a joined server).
    fn write_current_world(&mut self) -> Option<std::io::Result<()>> {
        let id = self.current_world.clone()?;
        let data = self.game.to_save();
        Some(save::write_to(&save::world_file(&save::saves_dir(), &id), &data))
    }

    fn controls(&mut self) -> Controls {
        let playing = self.screen == Screen::Playing && self.chat.is_none();
        let key = |k: KeyCode| playing && is_key_down(k);
        let mut forward = 0.0;
        let mut strafe = 0.0;
        if key(KeyCode::W) || key(KeyCode::Up) {
            forward += 1.0;
        }
        if key(KeyCode::S) || key(KeyCode::Down) {
            forward -= 1.0;
        }
        if key(KeyCode::D) || key(KeyCode::Right) {
            strafe += 1.0;
        }
        if key(KeyCode::A) || key(KeyCode::Left) {
            strafe -= 1.0;
        }
        let mouse = |b: MouseButton| playing && is_mouse_button_down(b);
        let mouse_p = |b: MouseButton| playing && is_mouse_button_pressed(b);
        Controls {
            input: Input {
                forward,
                strafe,
                jump: key(KeyCode::Space),
                jump_pressed: playing && is_key_pressed(KeyCode::Space),
                sneak: key(KeyCode::LeftShift) || key(KeyCode::RightShift),
                sprint: key(KeyCode::LeftControl) || key(KeyCode::R),
            },
            attack_held: mouse(MouseButton::Left),
            attack_pressed: mouse_p(MouseButton::Left),
            use_held: mouse(MouseButton::Right),
            use_pressed: mouse_p(MouseButton::Right),
            pick: mouse_p(MouseButton::Middle),
            drop: playing && is_key_pressed(KeyCode::Q),
        }
    }

    fn mouse_look(&mut self) {
        let m: Vec2 = mouse_position().into();
        if self.screen == Screen::Playing {
            if let Some(last) = self.last_mouse {
                let d = m - last;
                if d.length() < 400.0 {
                    let s = 0.0026 * self.settings.sensitivity;
                    let p = &mut self.game.player;
                    p.yaw = (p.yaw + d.x * s).rem_euclid(std::f32::consts::TAU);
                    p.pitch = (p.pitch - d.y * s).clamp(-1.55, 1.55);
                }
            }
        }
        self.last_mouse = Some(m);
    }

    fn handle_keys(&mut self) {
        if let Some(line) = &mut self.chat {
            type_into(line, 200);
            if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter) {
                let text = line.clone();
                self.chat = None;
                self.game.send_chat(&text);
                self.set_screen(Screen::Playing);
            } else if is_key_pressed(KeyCode::Escape) {
                self.chat = None;
                self.set_screen(Screen::Playing);
            }
            return;
        }
        match self.screen {
            Screen::Worlds => {
                if is_key_pressed(KeyCode::Escape) {
                    self.set_screen(Screen::Title);
                } else if is_key_pressed(KeyCode::Enter) {
                    if let Some(i) = self.world_sel.filter(|&i| self.worlds.get(i).map(|w| w.problem.is_none()).unwrap_or(false)) {
                        self.play_world(i);
                    }
                } else if !self.worlds.is_empty() && (is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::Up)) {
                    let n = self.worlds.len();
                    let cur = self.world_sel.unwrap_or(0);
                    self.world_sel = Some(if is_key_pressed(KeyCode::Down) { (cur + 1).min(n - 1) } else { cur.saturating_sub(1) });
                }
                return;
            }
            Screen::CreateWorld => {
                let (buf, max) = if self.form_focus == 0 { (&mut self.form_name, 32) } else { (&mut self.form_seed, 40) };
                type_into(buf, max);
                if is_key_pressed(KeyCode::Tab) {
                    self.form_focus = 1 - self.form_focus;
                }
                if is_key_pressed(KeyCode::Escape) {
                    self.set_screen(Screen::Worlds);
                } else if is_key_pressed(KeyCode::Enter) {
                    self.create_world();
                }
                return;
            }
            Screen::RenameWorld => {
                type_into(&mut self.form_name, 32);
                if is_key_pressed(KeyCode::Escape) {
                    self.set_screen(Screen::Worlds);
                } else if is_key_pressed(KeyCode::Enter) {
                    self.finish_rename();
                }
                return;
            }
            Screen::DeleteWorld => {
                if is_key_pressed(KeyCode::Escape) {
                    self.set_screen(Screen::Worlds);
                }
                return;
            }
            _ => {}
        }
        if self.screen == Screen::Multiplayer {
            let (buf, max) = match self.mp_focus {
                0 => (&mut self.mp_name, 16),
                1 => (&mut self.mp_addr, 128),
                _ => (&mut self.mp_password, 64),
            };
            type_into(buf, max);
            if is_key_pressed(KeyCode::Tab) {
                self.mp_focus = (self.mp_focus + 1) % 3;
            }
            if is_key_pressed(KeyCode::Escape) {
                self.joining = None;
                self.set_screen(Screen::Title);
            }
            if is_key_pressed(KeyCode::Enter) && self.joining.is_none() && self.connect_next.is_none() {
                self.connect_next = Some(self.mp_addr.clone());
            }
            return;
        }
        match self.screen {
            Screen::Playing => {
                if is_key_pressed(KeyCode::T) || is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Slash) {
                    drain_chars();
                    let start = if is_key_pressed(KeyCode::Slash) { "/" } else { "" };
                    self.chat = Some(start.to_string());
                    self.set_screen(Screen::Playing);
                    return;
                }
                if is_key_pressed(KeyCode::Escape) {
                    self.set_screen(Screen::Paused);
                } else if is_key_pressed(KeyCode::E) || is_key_pressed(KeyCode::Tab) {
                    self.recipe_scroll = 0.0;
                    self.set_screen(Screen::Inventory);
                }
                let keys = [KeyCode::Key1, KeyCode::Key2, KeyCode::Key3, KeyCode::Key4, KeyCode::Key5, KeyCode::Key6, KeyCode::Key7, KeyCode::Key8, KeyCode::Key9];
                for (i, k) in keys.iter().enumerate() {
                    if is_key_pressed(*k) {
                        self.game.inv.selected = i;
                        self.game.held_name = 2.0;
                    }
                }
                let wheel = mouse_wheel().1;
                if wheel.abs() > 0.1 {
                    let d = if wheel > 0.0 { 8 } else { 1 };
                    self.game.inv.selected = (self.game.inv.selected + d) % 9;
                    self.game.held_name = 2.0;
                }
                if is_key_pressed(KeyCode::F5) {
                    self.game.third_person = !self.game.third_person;
                }
            }
            Screen::Inventory => {
                if is_key_pressed(KeyCode::Escape) || is_key_pressed(KeyCode::E) || is_key_pressed(KeyCode::Tab) {
                    self.set_screen(Screen::Playing);
                }
            }
            Screen::Paused => {
                if is_key_pressed(KeyCode::Escape) {
                    self.set_screen(Screen::Playing);
                }
            }
            Screen::Advancements => {
                if is_key_pressed(KeyCode::Escape) {
                    self.set_screen(Screen::Paused);
                }
            }
            Screen::Options { from_title } | Screen::Help { from_title } => {
                if is_key_pressed(KeyCode::Escape) {
                    self.set_screen(if from_title { Screen::Title } else { Screen::Paused });
                }
            }
            Screen::Mods => {
                if is_key_pressed(KeyCode::Escape) {
                    self.set_screen(Screen::Title);
                }
            }
            _ => {}
        }
        if is_key_pressed(KeyCode::F3) {
            self.show_debug = !self.show_debug;
        }
        if is_key_pressed(KeyCode::F11) {
            self.settings.fullscreen = !self.settings.fullscreen;
            set_fullscreen(self.settings.fullscreen);
        }
    }

    fn frame(&mut self) {
        let dt = get_frame_time().min(0.05);
        self.fps = self.fps * 0.95 + (1.0 / get_frame_time().max(1e-4)) * 0.05;
        self.ui.begin_frame();
        self.handle_keys();
        self.mouse_look();

        let controls = self.controls();
        // Multiplayer worlds never pause: other people are still in them.
        let simulate = matches!(self.screen, Screen::Playing | Screen::Inventory | Screen::Title | Screen::Dead) || self.game.net.is_some();
        if simulate {
            self.game.update(dt, &controls);
        }
        if let Some(e) = self.game.net_error.take() {
            self.back_to_title();
            self.status = Some((e, 8.0));
        }
        self.update_joining();
        self.poll_upnp();
        if self.game.dead.is_some() && self.screen != Screen::Dead {
            self.set_screen(Screen::Dead);
        }
        if let Some((_, t)) = &mut self.status {
            *t -= dt;
            if *t <= 0.0 {
                self.status = None;
            }
        }

        // Mods changed (reload, or joined a server with mods): repaint the atlas.
        if block::generation() != self.atlas_gen {
            self.atlas_gen = block::generation();
            let mut atlas = self.base_atlas.clone();
            texture::apply_mod_textures(&mut atlas);
            let gl = unsafe { get_internal_gl() };
            self.renderer.update_atlas(gl.quad_context, &atlas);
            self.ui.tex.update_from_bytes(texture::ATLAS as u32, texture::ATLAS as u32, &atlas);
        }

        // 3D world
        let sky = self.game.sky_color();
        clear_background(Color::new(sky[0], sky[1], sky[2], 1.0));
        {
            let mut gl = unsafe { get_internal_gl() };
            gl.flush();
            let rd = self.settings.render_distance;
            self.game.stream(&mut self.renderer, gl.quad_context, rd);
            let aspect = screen_width() / screen_height().max(1.0);
            let cam = self.game.camera(aspect, self.settings.fov);
            self.last_view_proj = cam.view_proj;
            let geo = self.game.build_geo(&cam, rd);
            let fp = self.game.frame_params(&cam, &self.renderer, rd);
            self.renderer.draw(gl.quad_context, &fp, &geo);
        }

        set_default_camera();
        self.draw_ui();
        self.play_sounds(dt);
    }

    /// Drive the connect -> Hello -> Welcome handshake without freezing the menu.
    fn update_joining(&mut self) {
        if let Some(addr) = self.connect_next.take() {
            match net::Conn::connect(&addr) {
                Ok(mut conn) => {
                    conn.send(&net::Msg::Hello { protocol: net::PROTOCOL, name: multiplayer::sanitize_name(&self.mp_name) });
                    conn.flush();
                    self.joining = Some((conn, get_time()));
                }
                Err(e) => self.status = Some((format!("Couldn't connect to {addr}: {e}"), 6.0)),
            }
        }
        let Some((conn, started)) = &mut self.joining else { return };
        let mut msgs = conn.poll();
        if let Some(net::Msg::ModPack { data }) = msgs.iter().find(|m| matches!(m, net::Msg::ModPack { .. })) {
            match mods::decode_pack(data) {
                Ok(pack) => {
                    let n = pack.len();
                    mods::install_sources(pack, &[]);
                    self.using_server_mods = true;
                    if n > 0 {
                        self.status = Some((format!("Loaded {n} mod(s) from the server."), 5.0));
                    }
                }
                Err(e) => {
                    self.joining = None;
                    self.status = Some((format!("The server sent broken mods: {e}"), 6.0));
                    return;
                }
            }
        }
        if let Some(net::Msg::Challenge { nonce, password }) = msgs.iter().find(|m| matches!(m, net::Msg::Challenge { .. })) {
            if *password && self.mp_password.is_empty() {
                self.joining = None;
                self.status = Some(("This server needs a password. Type it in the Password box.".into(), 6.0));
                return;
            }
            conn.send(&net::Msg::Auth { proof: net::auth_proof(nonce, &self.mp_password) });
        }
        conn.flush();
        if let Some(i) = msgs.iter().position(|m| matches!(m, net::Msg::Welcome { .. })) {
            let leftover = msgs.split_off(i + 1);
            let Some(net::Msg::Welcome { id, seed, time, creative, spawn }) = msgs.pop() else { return };
            let (conn, _) = self.joining.take().unwrap();
            let g = Game::new_client(id, seed, time, creative, spawn, conn, &self.mp_name, leftover);
            self.start_game(g);
            return;
        }
        let failure = if let Some(net::Msg::Kick { reason }) = msgs.iter().find(|m| matches!(m, net::Msg::Kick { .. })) {
            Some(format!("Kicked: {reason}"))
        } else if let Some(e) = &conn.closed {
            Some(format!("Connection failed: {e}"))
        } else if get_time() - *started > 10.0 {
            Some("The host didn't answer. Check the address, and that the world is open / the port is forwarded.".into())
        } else {
            None
        };
        if let Some(f) = failure {
            self.joining = None;
            self.status = Some((f, 6.0));
        }
    }

    fn worlds_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        self.ui.text_centered("Select World", w / 2.0, h * 0.07, 16.0, WHITE);
        let pw = (300.0 * s).min(w * 0.92);
        let x = w / 2.0 - pw / 2.0;
        let row_h = 30.0 * s;
        let top = h * 0.07 + 10.0 * s;
        let bottom = h - 56.0 * s;
        let rows = (((bottom - top) / row_h).floor() as usize).max(1);

        if self.worlds.is_empty() {
            self.ui.text_centered("No worlds yet. Create one below!", w / 2.0, top + 40.0 * s, 11.0, WHITE);
        }
        let wheel = mouse_wheel().1;
        if wheel.abs() > 0.1 {
            let max = self.worlds.len().saturating_sub(rows);
            self.world_scroll = if wheel > 0.0 { self.world_scroll.saturating_sub(1) } else { (self.world_scroll + 1).min(max) };
        }
        let mut play: Option<usize> = None;
        for i in self.world_scroll..(self.world_scroll + rows).min(self.worlds.len()) {
            let wd = &self.worlds[i];
            let y = top + (i - self.world_scroll) as f32 * row_h;
            let r = Rect::new(x, y, pw, row_h - 3.0 * s);
            let selected = self.world_sel == Some(i);
            let hov = self.ui.hovered(r);
            let bg = if selected { Color::new(0.25, 0.3, 0.45, 0.95) } else if hov { Color::new(0.18, 0.18, 0.22, 0.95) } else { Color::new(0.1, 0.1, 0.12, 0.9) };
            draw_rectangle(r.x, r.y, r.w, r.h, bg);
            if selected {
                draw_rectangle_lines(r.x, r.y, r.w, r.h, s, WHITE);
            }
            // A little grass (or glowrock, for creative) block as the world's icon.
            let icon = r.h - 6.0 * s;
            self.ui.icon(if wd.creative { block::GLOWROCK } else { block::GRASS }, r.x + 3.0 * s, r.y + 3.0 * s, icon);
            let tx = r.x + icon + 9.0 * s;
            let text_w = r.w - icon - 14.0 * s;
            self.ui.text(&self.ui.fit(&wd.name, 11.0, text_w), tx, r.y + 12.0 * s, 11.0, WHITE);
            let detail = match &wd.problem {
                Some(p) => format!("Can't be opened: {p}"),
                None => format!(
                    "{}  -  seed {}  -  played {}  -  {} KB",
                    if wd.creative { "Creative" } else { "Survival" },
                    wd.seed,
                    save::ago(wd.last_played),
                    wd.size.div_ceil(1024)
                ),
            };
            let col = if wd.problem.is_some() { Color::new(1.0, 0.5, 0.4, 1.0) } else { Color::new(0.75, 0.75, 0.75, 1.0) };
            self.ui.text(&self.ui.fit(&detail, 8.0, text_w), tx, r.y + 22.0 * s, 8.0, col);
            if hov && self.ui.clicked {
                // Double-click to play.
                let now = get_time();
                if self.last_click.0 == i && now - self.last_click.1 < 0.4 {
                    play = Some(i);
                }
                self.last_click = (i, now);
                self.world_sel = Some(i);
            }
        }

        let sel = self.world_sel.filter(|&i| i < self.worlds.len());
        let playable = sel.map(|i| self.worlds[i].problem.is_none()).unwrap_or(false);
        let gap = 5.0 * s;
        let bh = 20.0 * s;
        let half = (pw - gap) / 2.0;
        let quarter = (pw - 3.0 * gap) / 4.0;
        let y1 = h - 50.0 * s;
        let y2 = y1 + bh + gap;
        if self.ui.button(Rect::new(x, y1, half, bh), "Play Selected World", playable) {
            play = sel;
        }
        if self.ui.button(Rect::new(x + half + gap, y1, half, bh), "Create New World", true) {
            self.form_name = "New World".into();
            self.form_seed.clear();
            self.form_creative = false;
            self.form_focus = 0;
            drain_chars();
            self.set_screen(Screen::CreateWorld);
            return;
        }
        if self.ui.button(Rect::new(x, y2, quarter, bh), "Rename", sel.is_some()) {
            self.form_name = self.worlds[sel.unwrap()].name.clone();
            drain_chars();
            self.set_screen(Screen::RenameWorld);
            return;
        }
        if self.ui.button(Rect::new(x + quarter + gap, y2, quarter, bh), "Delete", sel.is_some()) {
            self.set_screen(Screen::DeleteWorld);
            return;
        }
        if self.ui.button(Rect::new(x + 2.0 * (quarter + gap), y2, quarter * 2.0 + gap, bh), "Back", true) {
            self.set_screen(Screen::Title);
            return;
        }
        if let Some(i) = play.filter(|&i| self.worlds.get(i).map(|w| w.problem.is_none()).unwrap_or(false)) {
            self.play_world(i);
        }
    }

    fn create_world_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        self.ui.text_centered("Create New World", w / 2.0, h * 0.14, 16.0, WHITE);
        let bw = (220.0 * s).min(w * 0.85);
        let bh = 20.0 * s;
        let x = w / 2.0 - bw / 2.0;
        let mut y = h * 0.26;
        self.ui.text("World name", x, y - 3.0 * s, 9.0, GRAY);
        if self.ui.text_field(Rect::new(x, y, bw, bh), &self.form_name, self.form_focus == 0) {
            self.form_focus = 0;
        }
        y += bh + 14.0 * s;
        self.ui.text("Seed (leave empty for a random world; any text works)", x, y - 3.0 * s, 9.0, GRAY);
        if self.ui.text_field(Rect::new(x, y, bw, bh), &self.form_seed, self.form_focus == 1) {
            self.form_focus = 1;
        }
        y += bh + 8.0 * s;
        let mode = if self.form_creative { "Game Mode: Creative" } else { "Game Mode: Survival" };
        if self.ui.button(Rect::new(x, y, bw, bh), mode, true) {
            self.form_creative = !self.form_creative;
        }
        y += bh + 2.0 * s;
        let hint = if self.form_creative { "Fly, infinite blocks, no damage." } else { "Gather, craft, and try not to get hissed at." };
        self.ui.text_centered(hint, w / 2.0, y + 9.0 * s, 8.0, GRAY);
        y += 18.0 * s;
        let half = (bw - 5.0 * s) / 2.0;
        if self.ui.button(Rect::new(x, y, half, bh), "Create World", true) {
            self.create_world();
            return;
        }
        if self.ui.button(Rect::new(x + half + 5.0 * s, y, half, bh), "Cancel", true) {
            self.set_screen(Screen::Worlds);
        }
    }

    fn rename_world_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        self.ui.text_centered("Rename World", w / 2.0, h * 0.25, 16.0, WHITE);
        let bw = (220.0 * s).min(w * 0.85);
        let bh = 20.0 * s;
        let x = w / 2.0 - bw / 2.0;
        let y = h * 0.38;
        self.ui.text_field(Rect::new(x, y, bw, bh), &self.form_name, true);
        let half = (bw - 5.0 * s) / 2.0;
        if self.ui.button(Rect::new(x, y + bh + 10.0 * s, half, bh), "Save Name", true) {
            self.finish_rename();
            return;
        }
        if self.ui.button(Rect::new(x + half + 5.0 * s, y + bh + 10.0 * s, half, bh), "Cancel", true) {
            self.set_screen(Screen::Worlds);
        }
    }

    fn finish_rename(&mut self) {
        if let Some(wd) = self.world_sel.and_then(|i| self.worlds.get_mut(i)) {
            match save::write_name(&save::saves_dir(), &wd.id, &self.form_name) {
                Ok(()) => wd.name = save::clean_name(&self.form_name),
                Err(e) => self.status = Some((format!("Couldn't rename: {e}"), 5.0)),
            }
        }
        self.set_screen(Screen::Worlds);
    }

    fn delete_world_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        let Some(wd) = self.world_sel.and_then(|i| self.worlds.get(i)) else {
            self.set_screen(Screen::Worlds);
            return;
        };
        let (id, name) = (wd.id.clone(), wd.name.clone());
        self.ui.text_centered(&format!("Delete \"{name}\"?"), w / 2.0, h * 0.3, 16.0, WHITE);
        self.ui.text_centered("Its builds, inventory and script data will be gone for good.", w / 2.0, h * 0.3 + 20.0 * s, 10.0, Color::new(1.0, 0.6, 0.5, 1.0));
        let bw = (220.0 * s).min(w * 0.85);
        let bh = 20.0 * s;
        let x = w / 2.0 - bw / 2.0;
        let half = (bw - 5.0 * s) / 2.0;
        let y = h * 0.45;
        if self.ui.button(Rect::new(x, y, half, bh), "Delete Forever", true) {
            match save::delete_world(&save::saves_dir(), &id) {
                Ok(()) => self.status = Some((format!("Deleted \"{name}\"."), 4.0)),
                Err(e) => self.status = Some((format!("Couldn't delete: {e}"), 6.0)),
            }
            self.worlds = save::list_worlds(&save::saves_dir());
            self.world_sel = if self.worlds.is_empty() { None } else { Some(0) };
            self.world_scroll = 0;
            self.set_screen(Screen::Worlds);
            return;
        }
        if self.ui.button(Rect::new(x + half + 5.0 * s, y, half, bh), "Cancel", true) {
            self.set_screen(Screen::Worlds);
        }
    }

    fn multiplayer_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        self.ui.text_centered("Multiplayer", w / 2.0, h * 0.11, 16.0, WHITE);
        let bw = (240.0 * s).min(w * 0.85);
        let bh = 20.0 * s;
        let x = w / 2.0 - bw / 2.0;
        let mut y = h * 0.19;
        let masked = "*".repeat(self.mp_password.chars().count());
        let fields: [(&str, &str); 3] = [
            ("Your name", &self.mp_name),
            ("Server address: IP, IP:port, [IPv6]:port or a hostname", &self.mp_addr),
            ("Password (leave empty if the server has none)", &masked),
        ];
        let mut clicked = None;
        for (i, (label, value)) in fields.iter().enumerate() {
            self.ui.text(label, x, y - 3.0 * s, 9.0, GRAY);
            if self.ui.text_field(Rect::new(x, y, bw, bh), value, self.mp_focus == i) {
                clicked = Some(i);
            }
            y += bh + 14.0 * s;
        }
        if let Some(i) = clicked {
            self.mp_focus = i;
        }
        y -= 4.0 * s;
        let busy = self.joining.is_some() || self.connect_next.is_some();
        let label = if busy { "Connecting..." } else { "Join Server" };
        if self.ui.button(Rect::new(x, y, bw, bh), label, !busy) {
            self.connect_next = Some(self.mp_addr.clone());
        }
        y += bh + 5.0 * s;
        if self.ui.button(Rect::new(x, y, bw, bh), "Back", true) {
            self.joining = None;
            self.set_screen(Screen::Title);
        }
        y += bh + 14.0 * s;
        for line in [
            "Host from a world: Esc > \"Open to LAN\" or \"Open to Internet\". Your password above protects it.",
            "Or run a dedicated server: minceraft --server --password <pw>. Default port 25565.",
            "Tab switches fields. Enter joins.",
        ] {
            self.ui.text_centered(line, w / 2.0, y, 9.0, Color::new(0.85, 0.85, 0.85, 1.0));
            y += 12.0 * s;
        }
    }

    fn reload_mods(&mut self) {
        let infos = mods::install_local();
        let problems: usize = infos.iter().filter(|m| m.enabled).map(|m| m.errors.len()).sum();
        let on = infos.iter().filter(|m| m.enabled).count();
        self.status = Some((format!("Reloaded: {on} mod(s) on{}", if problems > 0 { format!(", {problems} problem(s)") } else { String::new() }), 5.0));
        // The title-screen world was generated with the old mods.
        let mut gl = unsafe { get_internal_gl() };
        gl.flush();
        self.renderer.clear(gl.quad_context);
        self.game = Game::new(random_seed(), true, true);
        self.splash = pick_splash();
    }

    fn mods_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        self.ui.text_centered("Mods", w / 2.0, h * 0.08, 16.0, WHITE);
        let dir = std::fs::canonicalize(mods::mods_dir()).unwrap_or_else(|_| std::env::current_dir().unwrap_or_default().join("mods"));
        self.ui.text_centered(&format!("Folder: {}", dir.display()), w / 2.0, h * 0.08 + 14.0 * s, 8.0, GRAY);

        let infos = block::reg().mods.clone();
        let pw = (320.0 * s).min(w * 0.92);
        let x = w / 2.0 - pw / 2.0;
        let row_h = 38.0 * s;
        let top = h * 0.08 + 22.0 * s;
        let bottom = h - 34.0 * s;
        let rows = (((bottom - top) / row_h).floor() as usize).max(1);
        if infos.is_empty() {
            let lines = [
                "No mods installed yet.",
                "A mod is a folder with a mod.txt inside, placed in the folder above.",
                "Try the example: copy example-mods/cheese into mods/, then press Reload.",
                "MODDING.md explains everything a mod can add.",
            ];
            for (i, l) in lines.iter().enumerate() {
                self.ui.text_centered(l, w / 2.0, top + (20.0 + i as f32 * 13.0) * s, 10.0, WHITE);
            }
        }
        let wheel = mouse_wheel().1;
        if wheel.abs() > 0.1 {
            let max = infos.len().saturating_sub(rows);
            self.mods_scroll = if wheel > 0.0 { self.mods_scroll.saturating_sub(1) } else { (self.mods_scroll + 1).min(max) };
        }
        let mut toggle: Option<(String, bool)> = None;
        for (i, m) in infos.iter().enumerate().skip(self.mods_scroll).take(rows) {
            let y = top + (i - self.mods_scroll) as f32 * row_h;
            draw_rectangle(x, y, pw, row_h - 3.0 * s, Color::new(0.12, 0.12, 0.15, 0.95));
            draw_rectangle_lines(x, y, pw, row_h - 3.0 * s, s, if m.enabled { Color::new(0.4, 0.8, 0.4, 1.0) } else { GRAY });
            let title = format!("{}{}{}", m.name, if m.version.is_empty() { String::new() } else { format!(" v{}", m.version) }, if m.author.is_empty() { String::new() } else { format!(" by {}", m.author) });
            let text_w = pw - 75.0 * s;
            self.ui.text(&self.ui.fit(&title, 10.0, text_w), x + 5.0 * s, y + 11.0 * s, 10.0, if m.enabled { WHITE } else { GRAY });
            let detail = if !m.enabled {
                "Disabled".to_string()
            } else {
                let (b, it, r) = m.added;
                let desc = if m.description.is_empty() { String::new() } else { format!("{}  -  ", m.description) };
                let scripts = if m.scripts > 0 { format!(", {} script(s)", m.scripts) } else { String::new() };
                format!("{desc}{b} blocks, {it} items, {r} recipes{scripts}")
            };
            self.ui.text(&self.ui.fit(&detail, 8.0, text_w), x + 5.0 * s, y + 22.0 * s, 8.0, Color::new(0.8, 0.8, 0.8, 1.0));
            if let Some(e) = m.errors.first() {
                let more = if m.errors.len() > 1 { format!("  (+{} more)", m.errors.len() - 1) } else { String::new() };
                self.ui.text(&self.ui.fit(&format!("! {e}{more}"), 8.0, pw - 10.0 * s), x + 5.0 * s, y + 32.0 * s, 8.0, Color::new(1.0, 0.5, 0.4, 1.0));
            }
            let bw = 60.0 * s;
            let label = if m.enabled { "On" } else { "Off" };
            if self.ui.button(Rect::new(x + pw - bw - 5.0 * s, y + 6.0 * s, bw, 18.0 * s), label, true) {
                toggle = Some((m.id.clone(), !m.enabled));
            }
        }
        if let Some((id, on)) = toggle {
            match mods::set_enabled(&mods::mods_dir(), &id, on) {
                Ok(()) => self.reload_mods(),
                Err(e) => self.status = Some((format!("Couldn't save mod settings: {e}"), 5.0)),
            }
        }
        let bw = (120.0 * s).min(pw / 2.0 - 4.0 * s);
        let by = h - 28.0 * s;
        if self.ui.button(Rect::new(w / 2.0 - bw - 4.0 * s, by, bw, 20.0 * s), "Reload Mods", true) {
            self.reload_mods();
        }
        if self.ui.button(Rect::new(w / 2.0 + 4.0 * s, by, bw, 20.0 * s), "Done", true) {
            self.set_screen(Screen::Title);
        }
    }

    /// Start hosting (if not already). Returns the port.
    fn host_now(&mut self) -> Option<u16> {
        if self.game.is_host() {
            if let Some(multiplayer::Net::Host(srv)) = &self.game.net {
                return Some(srv.port);
            }
        }
        let pw = Some(self.mp_password.clone()).filter(|p| !p.is_empty());
        match self.game.open_lan(&self.mp_name, pw) {
            Ok(port) => {
                let ip = net::lan_ip().unwrap_or_else(|| "this computer's IP".into());
                let addr = format!("{ip}:{port}");
                self.status = Some((format!("Hosting! Friends on your network can join at {addr}"), 8.0));
                self.lan_addr = Some(addr);
                Some(port)
            }
            Err(e) => {
                self.status = Some((format!("Couldn't start hosting: {e}"), 6.0));
                None
            }
        }
    }

    fn open_to_internet(&mut self) {
        let Some(port) = self.host_now() else { return };
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(upnp::open_port(port));
        });
        self.upnp_job = Some(rx);
        self.internet_status = Some("Asking your router to open the port (UPnP)...".into());
    }

    fn poll_upnp(&mut self) {
        let Some(rx) = &self.upnp_job else { return };
        let Ok(result) = rx.try_recv() else { return };
        self.upnp_job = None;
        let port = match &self.game.net {
            Some(multiplayer::Net::Host(s)) => s.port,
            _ => net::DEFAULT_PORT,
        };
        let v6 = net::public_ipv6().map(|ip| format!("  or  [{ip}]:{port}")).unwrap_or_default();
        self.internet_status = Some(match result {
            Ok(m) => {
                let text = match &m.external_ip {
                    Some(ip) if m.behind_second_nat() => format!(
                        "Port opened, but your ISP puts you behind a second NAT ({ip}), so outsiders can't reach you.{}",
                        if v6.is_empty() { " Try a dedicated server on a VPS.".to_string() } else { format!(" Share IPv6 instead:{v6}") }
                    ),
                    Some(ip) => format!("Online! Friends can join at {ip}:{port}{v6}"),
                    None => format!("Port {port} opened. Share your public IP (search \"what is my IP\"){v6}"),
                };
                self.upnp_mapping = Some(m);
                text
            }
            Err(e) => format!("{e} Forward TCP port {port} to this computer in your router settings.{v6}"),
        });
        if let (Some(t), Some(_)) = (&self.internet_status, &self.upnp_mapping) {
            self.game.msg(t.clone());
        }
    }

    /// Names floating above other players' heads.
    fn name_tags(&self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        let eye = self.game.player.eye();
        for p in self.game.peers.values().filter(|p| p.alive()) {
            let at = p.pos + Vec3::Y * 2.15;
            if at.distance(eye) > 64.0 {
                continue;
            }
            let clip = self.last_view_proj * at.extend(1.0);
            if clip.w < 0.1 {
                continue;
            }
            let (sx, sy) = ((clip.x / clip.w * 0.5 + 0.5) * w, (0.5 - clip.y / clip.w * 0.5) * h);
            let tw = self.ui.text_width(&p.name, 9.0);
            draw_rectangle(sx - tw / 2.0 - 3.0 * s, sy - 10.0 * s, tw + 6.0 * s, 12.0 * s, Color::new(0.0, 0.0, 0.0, 0.45));
            self.ui.text_centered(&p.name, sx, sy, 9.0, WHITE);
        }
    }

    fn play_sounds(&mut self, dt: f32) {
        let listener = self.game.player.eye();
        if self.ui.pressed.replace(false) {
            self.audio.play(Sfx::Click, None, listener);
        }
        // Keep the game world quiet while paused or in menus layered over it.
        let world_audible = !matches!(self.screen, Screen::Paused | Screen::Options { .. } | Screen::Help { .. } | Screen::Advancements);
        for (s, at) in std::mem::take(&mut self.game.sounds) {
            if world_audible || s == Sfx::Craft || s == Sfx::Fanfare {
                self.audio.play(s, at, listener);
            }
        }
        let in_game = !self.game.menu && self.screen != Screen::Dead;
        self.audio.update_music(dt, in_game);
    }

    fn draw_ui(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        match self.screen {
            Screen::Title => self.title_screen(),
            Screen::Options { from_title } => {
                draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.55));
                self.options_screen(from_title);
            }
            Screen::Help { from_title } => {
                draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.7));
                self.help_screen(from_title);
            }
            Screen::Mods => {
                draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.7));
                self.mods_screen();
            }
            Screen::Worlds | Screen::CreateWorld | Screen::RenameWorld | Screen::DeleteWorld => {
                draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.65));
                match self.screen {
                    Screen::Worlds => self.worlds_screen(),
                    Screen::CreateWorld => self.create_world_screen(),
                    Screen::RenameWorld => self.rename_world_screen(),
                    _ => self.delete_world_screen(),
                }
            }
            Screen::Multiplayer => {
                draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.6));
                self.multiplayer_screen();
            }
            _ => {
                self.hud();
                match self.screen {
                    Screen::Paused => self.pause_screen(),
                    Screen::Advancements => self.advancements_screen(),
                    Screen::Inventory => self.inventory_screen(),
                    Screen::Dead => self.death_screen(),
                    _ => {}
                }
            }
        }
        if let Some((msg, _)) = &self.status {
            self.ui.text_centered(msg, w / 2.0, h - 8.0 * s, 9.0, GOLD);
        }
    }

    fn hud(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        let g = &self.game;
        if !g.menu && g.player.head_in_water(&g.world) {
            draw_rectangle(0.0, 0.0, w, h, Color::new(0.1, 0.2, 0.6, 0.35));
        }
        if g.player.hurt > 0.0 {
            draw_rectangle(0.0, 0.0, w, h, Color::new(0.8, 0.0, 0.0, g.player.hurt * 0.5));
        }
        if !g.ready {
            draw_rectangle(0.0, 0.0, w, h, Color::new(0.1, 0.07, 0.05, 1.0));
            self.ui.text_centered("Generating terrain (artisanally)...", w / 2.0, h / 2.0, 12.0, WHITE);
            self.ui.text_centered(&format!("{} chunks ready", self.renderer.chunks.len()), w / 2.0, h / 2.0 + 18.0 * s, 9.0, GRAY);
            return;
        }
        if self.screen == Screen::Playing {
            self.ui.crosshair();
        }
        self.name_tags();

        // Hotbar
        let slot = 20.0 * s;
        let x0 = w / 2.0 - slot * 4.5;
        let y0 = h - slot - 4.0 * s;
        draw_rectangle(x0 - 2.0 * s, y0 - 2.0 * s, slot * 9.0 + 4.0 * s, slot + 4.0 * s, Color::new(0.0, 0.0, 0.0, 0.45));
        for i in 0..9 {
            let x = x0 + i as f32 * slot;
            draw_rectangle_lines(x, y0, slot, slot, s, Color::new(0.6, 0.6, 0.6, 0.6));
            self.ui.stack(g.inv.slots[i], x, y0, slot, !g.creative);
        }
        let sel = x0 + g.inv.selected as f32 * slot;
        draw_rectangle_lines(sel - s, y0 - s, slot + 2.0 * s, slot + 2.0 * s, 2.0 * s, WHITE);
        if !g.creative {
            self.ui.hearts(g.player.health, x0, y0 - 12.0 * s);
        }
        if g.held_name > 0.0 {
            let held = g.inv.held();
            if held != AIR {
                let a = g.held_name.min(1.0);
                self.ui.text_centered(item_name(held), w / 2.0, y0 - if g.creative { 6.0 } else { 16.0 } * s, 10.0, Color::new(1.0, 1.0, 1.0, a));
            }
        }

        // Chat-ish messages (all recent ones stay visible while typing)
        let typing = self.chat.is_some();
        for (i, (m, t)) in g.messages.iter().rev().enumerate() {
            let a = if typing { 1.0 } else { t.min(1.0) };
            let y = h - 40.0 * s - i as f32 * 11.0 * s;
            let tw = self.ui.text_width(m, 9.0);
            draw_rectangle(4.0 * s, y - 9.0 * s, tw + 6.0 * s, 11.0 * s, Color::new(0.0, 0.0, 0.0, 0.4 * a));
            self.ui.text(m, 7.0 * s, y, 9.0, Color::new(1.0, 1.0, 1.0, a));
        }

        if let Some(line) = &self.chat {
            let y = h - 30.0 * s;
            draw_rectangle(4.0 * s, y - 10.0 * s, w - 8.0 * s, 13.0 * s, Color::new(0.0, 0.0, 0.0, 0.6));
            let caret = if (get_time() * 2.0) as i64 % 2 == 0 { "_" } else { " " };
            self.ui.text(&format!("> {line}{caret}"), 7.0 * s, y, 9.0, WHITE);
        }

        if self.show_debug {
            let p = g.player.body.pos;
            let (hgt, biome) = g.world.generator.column(p.x.floor() as i32, p.z.floor() as i32);
            let facing = ["north (-Z)", "east (+X)", "south (+Z)", "west (-X)"][((g.player.yaw / std::f32::consts::FRAC_PI_2 + 0.5).floor() as i32).rem_euclid(4) as usize];
            let hours = ((g.time * 24.0 + 6.0) % 24.0) as i32;
            let lines = [
                format!("Minceraft (native build) {:.0} fps", self.fps),
                format!("XYZ: {:.2} / {:.2} / {:.2}", p.x, p.y, p.z),
                format!("Facing: {facing}"),
                format!("Biome: {} (surface {hgt})", biome.name()),
                format!("Chunks: {} meshed, {} loaded", self.renderer.chunks.len(), g.world.chunks.len()),
                format!("Mobs: {}  Particles: {}", g.mobs.len(), g.particles.len()),
                format!("Time: {:02}:00  Daylight: {:.2}", hours, g.daylight()),
                format!("Seed: {}  Mode: {}", g.world.seed(), if g.creative { "Creative" } else { "Survival" }),
                format!("Blocks broken: {}", g.stat_blocks_broken),
                match &g.net {
                    None => "Network: single player".to_string(),
                    Some(multiplayer::Net::Host(srv)) => format!("Network: hosting on port {} ({} players)", srv.port, g.player_count()),
                    Some(multiplayer::Net::Client(_)) => format!("Network: connected as {} ({} players)", g.player_name, g.player_count()),
                },
            ];
            for (i, l) in lines.iter().enumerate() {
                self.ui.text(l, 4.0 * s, (12.0 + i as f32 * 10.0) * s, 9.0, WHITE);
            }
        }
        self.toast();
    }

    fn title_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.25));
        let px = (w / 60.0).min(h / 22.0);
        self.ui.logo(w / 2.0, h * 0.12, px);
        let pulse = 1.0 + (get_time() * 6.0).sin().abs() as f32 * 0.08;
        let size = self.ui.font(11.0 * pulse);
        let tw = measure_text(self.splash, None, size, 1.0).width;
        let (sx, sy) = (w / 2.0 + px * 16.0, h * 0.12 + px * 6.2);
        draw_text_ex(self.splash, sx - tw / 2.0, sy, TextParams { font_size: size, color: GOLD, rotation: -0.35, ..Default::default() });

        let bw = (200.0 * s).min(w * 0.8);
        let bh = 20.0 * s;
        let x = w / 2.0 - bw / 2.0;
        let mut y = h * 0.45;
        if self.ui.button(Rect::new(x, y, bw, bh), "Singleplayer", true) {
            self.open_worlds();
            return;
        }
        y += bh + 5.0 * s;
        let half = (bw - 5.0 * s) / 2.0;
        if self.ui.button(Rect::new(x, y, half, bh), "Multiplayer", true) {
            drain_chars();
            self.set_screen(Screen::Multiplayer);
            return;
        }
        let n_mods = block::reg().mods.iter().filter(|m| m.enabled).count();
        let mods_label = if n_mods > 0 { format!("Mods ({n_mods})") } else { "Mods".to_string() };
        if self.ui.button(Rect::new(x + half + 5.0 * s, y, half, bh), &mods_label, true) {
            self.mods_scroll = 0;
            self.set_screen(Screen::Mods);
            return;
        }
        y += bh + 5.0 * s;
        let half = (bw - 5.0 * s) / 2.0;
        if self.ui.button(Rect::new(x, y, half, bh), "Options", true) {
            self.set_screen(Screen::Options { from_title: true });
        }
        if self.ui.button(Rect::new(x + half + 5.0 * s, y, half, bh), "How to Play", true) {
            self.set_screen(Screen::Help { from_title: true });
        }
        y += bh + 5.0 * s;
        if self.ui.button(Rect::new(x, y, bw, bh), "Quit to Desktop", true) {
            self.quit = true;
        }
        self.ui.text("Minceraft 1.0 (Rust, no browser)", 4.0 * s, h - 5.0 * s, 8.0, WHITE);
        let c = "Not affiliated with any block-game company. Please don't sue.";
        let cw = self.ui.text_width(c, 8.0);
        self.ui.text(c, w - cw - 4.0 * s, h - 5.0 * s, 8.0, WHITE);
    }

    fn pause_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.5));
        self.ui.text_centered("Game Paused (the world politely waits)", w / 2.0, h * 0.25, 14.0, WHITE);
        let bw = (200.0 * s).min(w * 0.8);
        let bh = 20.0 * s;
        let x = w / 2.0 - bw / 2.0;
        let mut y = h * 0.29;
        if self.ui.button(Rect::new(x, y, bw, bh), "Back to Game", true) {
            self.set_screen(Screen::Playing);
        }
        y += bh + 5.0 * s;
        let client = self.game.is_client();
        if client {
            let host = self.game.peers.get(&0).map(|p| format!("{}'s world", p.name)).unwrap_or_else(|| "a dedicated server".into());
            self.ui.text_centered(&format!("Playing on {host} ({})", players(self.game.player_count())), w / 2.0, y + bh * 0.65, 10.0, GRAY);
        } else if self.game.is_host() {
            let addr = self.lan_addr.clone().unwrap_or_default();
            let lock = if self.game.has_password() { ", password protected" } else { ", no password" };
            self.ui.text_centered(&format!("Hosting at {addr}  ({}{lock})", players(self.game.player_count())), w / 2.0, y + bh * 0.65, 10.0, GOLD);
        } else if self.ui.button(Rect::new(x, y, bw, bh), "Open to LAN", true) {
            self.host_now();
        }
        if !client {
            y += bh + 5.0 * s;
            if let Some(t) = &self.internet_status {
                // Long messages wrap onto two lines.
                let (a, b) = match t.char_indices().filter(|(_, c)| *c == ' ').map(|(i, _)| i).find(|&i| i > 60) {
                    Some(i) if t.len() > 80 => (&t[..i], &t[i + 1..]),
                    _ => (t.as_str(), ""),
                };
                self.ui.text_centered(a, w / 2.0, y + bh * 0.4, 9.0, Color::new(0.7, 0.9, 1.0, 1.0));
                self.ui.text_centered(b, w / 2.0, y + bh * 0.4 + 11.0 * s, 9.0, Color::new(0.7, 0.9, 1.0, 1.0));
            } else if self.ui.button(Rect::new(x, y, bw, bh), "Open to Internet", true) {
                self.open_to_internet();
            }
        }
        y += bh + 5.0 * s;
        if self.ui.button(Rect::new(x, y, bw, bh), "Save World", !client) {
            self.save();
        }
        y += bh + 5.0 * s;
        let adv = format!("Advancements ({}/{})", self.game.advancements.count(), advancements::ALL.len());
        if self.ui.button(Rect::new(x, y, bw, bh), &adv, true) {
            self.set_screen(Screen::Advancements);
        }
        y += bh + 5.0 * s;
        let half = (bw - 5.0 * s) / 2.0;
        if self.ui.button(Rect::new(x, y, half, bh), "Options", true) {
            self.set_screen(Screen::Options { from_title: false });
        }
        if self.ui.button(Rect::new(x + half + 5.0 * s, y, half, bh), "How to Play", true) {
            self.set_screen(Screen::Help { from_title: false });
        }
        y += bh + 5.0 * s;
        let quit_label = if client { "Disconnect" } else { "Save and Quit to Title" };
        if self.ui.button(Rect::new(x, y, bw, bh), quit_label, true) {
            self.back_to_title();
        }
    }

    fn advancements_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.6));
        let title = format!("Advancements: {}/{} (they're per world, like memories)", self.game.advancements.count(), advancements::ALL.len());
        self.ui.text_centered(&title, w / 2.0, h * 0.08, 14.0, WHITE);
        let p = &self.game.advancements;
        let cols = if w >= 2.0 * 220.0 * s { 2 } else { 1 };
        let col_w = ((w - 20.0 * s) / cols as f32).min(290.0 * s);
        let row_h = 21.0 * s;
        let per_col = advancements::ALL.len().div_ceil(cols);
        let x0 = w / 2.0 - col_w * cols as f32 / 2.0;
        let y0 = h * 0.12;
        let max_rows = ((h * 0.84 - y0) / row_h).floor().max(1.0) as usize;
        let max_scroll = per_col.saturating_sub(max_rows);
        let wheel = mouse_wheel().1;
        if wheel.abs() > 0.1 {
            self.adv_scroll = if wheel > 0.0 { self.adv_scroll.saturating_sub(1) } else { self.adv_scroll + 1 };
        }
        self.adv_scroll = self.adv_scroll.min(max_scroll);
        for (i, a) in advancements::ALL.iter().enumerate() {
            let (c, r) = (i / per_col, i % per_col);
            let Some(r) = r.checked_sub(self.adv_scroll).filter(|&r| r < max_rows) else { continue };
            let (x, y) = (x0 + c as f32 * col_w, y0 + r as f32 * row_h);
            let got = p.has(a.key);
            draw_rectangle(x + 2.0 * s, y, col_w - 4.0 * s, row_h - 2.0 * s, if got { Color::new(0.25, 0.22, 0.08, 0.9) } else { Color::new(0.12, 0.12, 0.14, 0.85) });
            let tint = if got { WHITE } else { Color::new(0.3, 0.3, 0.3, 1.0) };
            self.ui.tile(texture::T_TROPHY, x + 4.0 * s, y + 1.0 * s, row_h - 4.0 * s, tint);
            let tx = x + row_h + 4.0 * s;
            let max = col_w - row_h - 10.0 * s;
            self.ui.text(&self.ui.fit(a.title, 9.0, max), tx, y + 9.0 * s, 9.0, if got { GOLD } else { GRAY });
            self.ui.text(&self.ui.fit(a.desc, 7.0, max), tx, y + 17.0 * s, 7.0, if got { WHITE } else { Color::new(0.55, 0.55, 0.55, 1.0) });
        }
        if max_scroll > 0 {
            self.ui.text_centered("(scroll for more)", w / 2.0, y0 + max_rows as f32 * row_h + 6.0 * s, 7.0, GRAY);
        }
        let bw = (160.0 * s).min(w * 0.8);
        if self.ui.button(Rect::new(w / 2.0 - bw / 2.0, h * 0.88, bw, 20.0 * s), "Done", true) {
            self.set_screen(Screen::Paused);
        }
    }

    /// "Advancement Made!" toast in the top-right corner.
    fn toast(&self) {
        let Some(&(a, t)) = self.game.toasts.first() else { return };
        let (w, s) = (screen_width(), self.ui.s);
        let tw = (170.0 * s).min(w - 8.0 * s);
        let th = 30.0 * s;
        // Slide in, hold, slide out.
        let slide = ((5.0 - t) / 0.3).min(t / 0.3).clamp(0.0, 1.0);
        let x = w - (tw + 4.0 * s) * slide;
        let y = 4.0 * s;
        draw_rectangle(x, y, tw, th, Color::new(0.13, 0.1, 0.05, 0.95));
        draw_rectangle_lines(x, y, tw, th, 1.5 * s, GOLD);
        self.ui.tile(texture::T_TROPHY, x + 4.0 * s, y + 4.0 * s, th - 8.0 * s, WHITE);
        let tx = x + th + 2.0 * s;
        self.ui.text("Advancement Made!", tx, y + 12.0 * s, 9.0, Color::new(1.0, 0.95, 0.4, 1.0));
        self.ui.text(&self.ui.fit(a.title, 9.0, tw - th - 6.0 * s), tx, y + 24.0 * s, 9.0, WHITE);
    }

    fn options_screen(&mut self, from_title: bool) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        self.ui.text_centered("Options", w / 2.0, h * 0.2, 16.0, WHITE);
        let bw = (220.0 * s).min(w * 0.8);
        let bh = 20.0 * s;
        let x = w / 2.0 - bw / 2.0;
        let small = bh * 1.3;
        let mut y = h * 0.3;
        let row = |ui: &Ui, label: String, y: f32| -> i32 {
            let mut d = 0;
            if ui.button(Rect::new(x, y, small, bh), "-", true) {
                d = -1;
            }
            let r = Rect::new(x + small + 4.0 * s, y, bw - 2.0 * small - 8.0 * s, bh);
            draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.15, 0.15, 0.18, 0.9));
            ui.text_centered(&label, r.x + r.w / 2.0, r.y + r.h * 0.68, 10.0, WHITE);
            if ui.button(Rect::new(x + bw - small, y, small, bh), "+", true) {
                d = 1;
            }
            d
        };
        let st = &mut self.settings;
        st.render_distance = (st.render_distance + row(&self.ui, format!("Render Distance: {} chunks", st.render_distance), y)).clamp(3, 16);
        y += bh + 5.0 * s;
        st.fov = (st.fov + 5.0 * row(&self.ui, format!("FOV: {:.0}", st.fov), y) as f32).clamp(50.0, 110.0);
        y += bh + 5.0 * s;
        st.sensitivity = (st.sensitivity + 0.1 * row(&self.ui, format!("Mouse Sensitivity: {:.0}%", st.sensitivity * 100.0), y) as f32).clamp(0.1, 3.0);
        y += bh + 5.0 * s;
        let vol = &mut self.audio.volume;
        *vol = (*vol + 0.1 * row(&self.ui, format!("Sound Volume: {:.0}%", *vol * 100.0), y) as f32).clamp(0.0, 1.0);
        *vol = (*vol * 10.0).round() / 10.0;
        y += bh + 5.0 * s;
        let music = if self.audio.music_on { "Music: ON (occasionally, tastefully)" } else { "Music: OFF" };
        if self.ui.button(Rect::new(x, y, bw, bh), music, true) {
            self.audio.music_on = !self.audio.music_on;
        }
        y += bh + 5.0 * s;
        let st = &mut self.settings;
        let fs = if st.fullscreen { "Fullscreen: ON (F11)" } else { "Fullscreen: OFF (F11)" };
        if self.ui.button(Rect::new(x, y, bw, bh), fs, true) {
            self.settings.fullscreen = !self.settings.fullscreen;
            set_fullscreen(self.settings.fullscreen);
        }
        y += bh + 12.0 * s;
        if self.ui.button(Rect::new(x, y, bw, bh), "Done", true) {
            self.set_screen(if from_title { Screen::Title } else { Screen::Paused });
        }
    }

    fn help_screen(&mut self, from_title: bool) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        self.ui.text_centered("How to Play Minceraft", w / 2.0, h * 0.1, 16.0, WHITE);
        let lines = [
            "WASD / arrows ... walk          Space ... jump / swim up",
            "Ctrl or R ... sprint            Shift ... sneak (won't fall off ledges)",
            "Left mouse ... mine / attack    Right mouse ... place / eat / light TNT with a torch",
            "1-9 / wheel ... pick hotbar     Middle mouse ... pick block (creative)",
            "E or Tab ... inventory + crafting    Q ... yeet held item",
            "F5 ... third person    F3 ... debug info    F11 ... fullscreen    Esc ... pause",
            "Creative: double-tap Space to fly, Shift to descend.",
            "",
            "Survival tips: punch a Tree Chunk, craft Planks, then Sticks, then a Wooden Pickaxe.",
            "Stone needs a pickaxe. Iron needs stone tier. Dimonds need iron tier.",
            "Hissers explode. Groaners bite and burn in daylight. Oinkers and Fluffers are friends (and food).",
            "Never look a Starer in the eye. Right-click a bed at night to skip it. Pokey Plants poke.",
            "Crafting works anywhere. The crafting table is purely decorative. Satire!",
            "Multiplayer: host opens their world with Esc > Open to LAN; friends use Multiplayer. T to chat.",
        ];
        for (i, l) in lines.iter().enumerate() {
            self.ui.text_centered(l, w / 2.0, h * 0.2 + i as f32 * 13.0 * s, 9.0, if l.is_empty() { WHITE } else { Color::new(0.9, 0.9, 0.9, 1.0) });
        }
        let bw = (160.0 * s).min(w * 0.8);
        if self.ui.button(Rect::new(w / 2.0 - bw / 2.0, h * 0.2 + 15.0 * 13.0 * s, bw, 20.0 * s), "Got it", true) {
            self.set_screen(if from_title { Screen::Title } else { Screen::Paused });
        }
    }

    fn death_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.5, 0.0, 0.0, 0.45));
        self.ui.text_centered("You died! (skill issue)", w / 2.0, h * 0.3, 20.0, WHITE);
        if let Some(d) = &self.game.dead {
            self.ui.text_centered(d, w / 2.0, h * 0.3 + 20.0 * s, 10.0, Color::new(1.0, 0.85, 0.85, 1.0));
        }
        let bw = (200.0 * s).min(w * 0.8);
        let bh = 20.0 * s;
        let x = w / 2.0 - bw / 2.0;
        if self.ui.button(Rect::new(x, h * 0.5, bw, bh), "Respawn", true) {
            self.game.respawn();
            self.set_screen(Screen::Playing);
        }
        if self.ui.button(Rect::new(x, h * 0.5 + bh + 5.0 * s, bw, bh), "Rage Quit to Title", true) {
            self.game.respawn();
            self.back_to_title();
        }
    }

    fn inventory_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.5));
        let slot = 20.0 * s;
        let creative = self.game.creative;
        let left_w = slot * 9.0 + 12.0 * s;
        let right_w = (170.0 * s).min(w - left_w - 30.0 * s);
        let total_w = left_w + 8.0 * s + right_w;
        let panel_h = (slot * 9.5).min(h - 20.0 * s).max(slot * 7.0);
        let x0 = (w - total_w) / 2.0;
        let y0 = (h - panel_h) / 2.0;
        draw_rectangle(x0, y0, left_w, panel_h, ui::PANEL);
        draw_rectangle_lines(x0, y0, left_w, panel_h, s, WHITE);
        let mut tooltip: Option<String> = None;

        let sx = x0 + 6.0 * s;
        let inv_top = y0 + 18.0 * s;
        if creative {
            self.ui.text("Creative Palette (click to grab 64)", sx, y0 + 12.0 * s, 9.0, WHITE);
            let items = creative_items();
            for (i, &item) in items.iter().enumerate() {
                let (cx, cy) = (sx + (i % 9) as f32 * slot, inv_top + (i / 9) as f32 * slot);
                let (l, r, hov) = self.ui.slot(Some((item, 1)), cx, cy, slot, false);
                if hov {
                    tooltip = Some(item_name(item).to_string());
                }
                if l || r {
                    let n = if r { 1 } else { max_stack(item) };
                    self.game.inv.cursor = Some((item, n));
                }
            }
        } else {
            self.ui.text("Inventory", sx, y0 + 12.0 * s, 10.0, WHITE);
            for i in 9..36 {
                let j = i - 9;
                let (cx, cy) = (sx + (j % 9) as f32 * slot, inv_top + (j / 9) as f32 * slot);
                let (l, r, hov) = self.ui.slot(self.game.inv.slots[i], cx, cy, slot, false);
                if hov {
                    tooltip = self.game.inv.slots[i].map(|s| item_name(s.0).to_string());
                }
                if l {
                    self.game.inv.click(i);
                }
                if r {
                    self.game.inv.right_click(i);
                }
            }
        }
        let hot_y = y0 + panel_h - slot - 6.0 * s;
        self.ui.text("Hotbar", sx, hot_y - 3.0 * s, 9.0, GRAY);
        for i in 0..9 {
            let cx = sx + i as f32 * slot;
            let (l, r, hov) = self.ui.slot(self.game.inv.slots[i], cx, hot_y, slot, i == self.game.inv.selected);
            if hov {
                tooltip = self.game.inv.slots[i].map(|s| item_name(s.0).to_string());
            }
            if l {
                if creative && self.game.inv.cursor.is_none() {
                    self.game.inv.slots[i] = None;
                } else {
                    self.game.inv.click(i);
                }
            }
            if r {
                self.game.inv.right_click(i);
            }
        }
        if creative && self.game.inv.cursor.is_some() && self.ui.clicked {
            let hov_any = self.ui.hovered(Rect::new(x0, y0, left_w, panel_h));
            if !hov_any {
                self.game.inv.cursor = None;
            }
        }

        // Crafting list
        if !creative {
            let rx = x0 + left_w + 8.0 * s;
            draw_rectangle(rx, y0, right_w, panel_h, ui::PANEL);
            draw_rectangle_lines(rx, y0, right_w, panel_h, s, WHITE);
            self.ui.text("Pocket Crafting (tables are decorative)", rx + 5.0 * s, y0 + 12.0 * s, 8.0, WHITE);
            let row_h = 22.0 * s;
            let list_top = y0 + 18.0 * s;
            let visible_rows = ((panel_h - 22.0 * s) / row_h).floor() as usize;
            let max_scroll = recipes().len().saturating_sub(visible_rows) as f32;
            if self.ui.hovered(Rect::new(rx, y0, right_w, panel_h)) {
                let wheel = mouse_wheel().1;
                if wheel.abs() > 0.1 {
                    self.recipe_scroll = (self.recipe_scroll - wheel.signum()).clamp(0.0, max_scroll);
                }
            }
            let first = self.recipe_scroll as usize;
            // Craftable recipes first so progress is obvious.
            let mut order: Vec<usize> = (0..recipes().len()).collect();
            order.sort_by_key(|&i| !self.game.inv.can_craft(&recipes()[i]));
            for (row, &ri) in order.iter().skip(first).take(visible_rows).enumerate() {
                let r = &recipes()[ri];
                let ok = self.game.inv.can_craft(r);
                let ry = list_top + row as f32 * row_h;
                let rect = Rect::new(rx + 4.0 * s, ry, right_w - 8.0 * s, row_h - 2.0 * s);
                let hov = self.ui.hovered(rect);
                let bg = if ok && hov { Color::new(0.3, 0.5, 0.3, 0.95) } else if ok { Color::new(0.2, 0.32, 0.2, 0.9) } else { Color::new(0.2, 0.2, 0.22, 0.9) };
                draw_rectangle(rect.x, rect.y, rect.w, rect.h, bg);
                let isz = row_h - 6.0 * s;
                self.ui.stack(Some(r.output), rect.x + 2.0 * s, rect.y + 1.0 * s, isz, true);
                let mut ix = rect.x + isz + 6.0 * s;
                self.ui.text("<", ix, rect.y + rect.h * 0.65, 9.0, GRAY);
                ix += 8.0 * s;
                for &(item, n) in &r.inputs {
                    let have = self.game.inv.count(item) >= n as u32;
                    self.ui.icon(item, ix, rect.y + 3.0 * s, isz * 0.8);
                    self.ui.text(&format!("{n}"), ix + isz * 0.8, rect.y + rect.h * 0.8, 8.0, if have { WHITE } else { Color::new(1.0, 0.4, 0.4, 1.0) });
                    ix += isz * 0.8 + 12.0 * s;
                }
                if hov {
                    let ins: Vec<String> = r.inputs.iter().map(|&(i, n)| format!("{n}x {}", item_name(i))).collect();
                    tooltip = Some(format!("{}x {}  <=  {}", r.output.1, item_name(r.output.0), ins.join(" + ")));
                    if self.ui.clicked && ok {
                        let times = if is_key_down(KeyCode::LeftShift) { 64 } else { 1 };
                        for _ in 0..times {
                            if !self.game.inv.craft(r) {
                                break;
                            }
                        }
                        let via_gold = r.inputs.iter().any(|&(i, _)| i == GOLD_INGOT) && r.output.0 == PICK_WOOD;
                        self.game.on_crafted(r.output.0, via_gold);
                    }
                }
            }
            if max_scroll > 0.0 {
                self.ui.text("scroll for more", rx + right_w - 70.0 * s, y0 + panel_h - 3.0 * s, 7.0, GRAY);
            }
        }

        if let Some(c) = self.game.inv.cursor {
            let (mx, my) = mouse_position();
            self.ui.stack(Some(c), mx - slot / 2.0, my - slot / 2.0, slot, true);
        } else if let Some(t) = tooltip {
            self.ui.tooltip(&t);
        }
    }
}

/// Headless-ish verification helper: `--screenshot out.png [--mode title|survival|creative|inventory|night|options] [--frames N]`.
struct ShotArgs {
    path: String,
    mode: String,
    frames: u32,
    time: Option<f32>,
    yaw: f32,
    pitch: f32,
    pos: Option<Vec3>,
    addr: String,
    password: String,
    chat: Vec<String>,
}

fn parse_args() -> Option<ShotArgs> {
    let args: Vec<String> = std::env::args().collect();
    let get = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    get("--screenshot").map(|path| ShotArgs {
        path,
        mode: get("--mode").unwrap_or_else(|| "title".into()),
        frames: get("--frames").and_then(|f| f.parse().ok()).unwrap_or(240),
        time: get("--time").and_then(|f| f.parse().ok()),
        addr: get("--addr").unwrap_or_else(|| "127.0.0.1".into()),
        password: get("--password").unwrap_or_default(),
        chat: args.windows(2).filter(|w| w[0] == "--chat").map(|w| w[1].clone()).collect(),
        yaw: get("--yaw").and_then(|f| f.parse().ok()).unwrap_or(2.4),
        pitch: get("--pitch").and_then(|f| f.parse().ok()).unwrap_or(-0.25),
        pos: get("--pos").and_then(|p| {
            let v: Vec<f32> = p.split(',').filter_map(|x| x.parse().ok()).collect();
            (v.len() == 3).then(|| Vec3::new(v[0], v[1], v[2]))
        }),
    })
}

/// The platform audio backend panics on its own thread when there's no sound device.
/// Treat that as "no audio" instead of printing a scary backtrace and spamming errors.
fn install_audio_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let name = std::thread::current().name().map(str::to_owned);
        if name.is_none() {
            if !sound::AUDIO_DEAD.swap(true, std::sync::atomic::Ordering::Relaxed) {
                eprintln!("Minceraft: no usable audio device ({info}). Continuing in silence.");
            }
            return;
        }
        default(info)
    }));
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // Headless modes run before any window (or GPU) is touched.
    if args.iter().any(|a| a == "--server") {
        std::process::exit(server::run(&args));
    }
    if let Some(i) = args.iter().position(|a| a == "--export-sounds") {
        let dir = args.get(i + 1).map(String::as_str).unwrap_or("sounds");
        match sound::export_wavs(std::path::Path::new(dir)) {
            Ok(n) => println!("Wrote {n} sounds to {dir}/"),
            Err(e) => eprintln!("Couldn't export sounds: {e}"),
        }
        return;
    }
    macroquad::Window::from_config(window_conf(), game_main());
}

async fn game_main() {
    let mod_infos = mods::install_local();
    let base_atlas = texture::build_atlas(1337);
    let mut atlas = base_atlas.clone();
    texture::apply_mod_textures(&mut atlas);
    let renderer = {
        let gl = unsafe { get_internal_gl() };
        Renderer::new(gl.quad_context, &atlas)
    };
    let tex = Texture2D::from_rgba8(texture::ATLAS as u16, texture::ATLAS as u16, &atlas);
    tex.set_filter(FilterMode::Nearest);

    install_audio_panic_hook();
    let audio = Audio::load().await;

    let shot = parse_args();
    let mut app = App {
        screen: Screen::Title,
        game: Game::new(random_seed(), true, true),
        renderer,
        ui: Ui::new(tex),
        settings: Settings { render_distance: 8, fov: 72.0, sensitivity: 1.0, fullscreen: false },
        splash: pick_splash(),
        last_mouse: None,
        show_debug: false,
        recipe_scroll: 0.0,
        quit: false,
        status: None,
        fps: 60.0,
        audio,
        mp_name: format!("Stove{}", random_seed() % 1000),
        mp_addr: "127.0.0.1".into(),
        mp_focus: 1,
        connect_next: None,
        joining: None,
        chat: None,
        last_view_proj: Mat4::IDENTITY,
        lan_addr: None,
        mp_password: String::new(),
        upnp_job: None,
        upnp_mapping: None,
        internet_status: None,
        base_atlas,
        atlas_gen: block::generation(),
        mods_scroll: 0,
        adv_scroll: 0,
        using_server_mods: false,
        current_world: None,
        worlds: Vec::new(),
        world_sel: None,
        world_scroll: 0,
        last_click: (usize::MAX, 0.0),
        form_name: String::new(),
        form_seed: String::new(),
        form_creative: false,
        form_focus: 0,
    };
    let broken: Vec<&block::ModInfo> = mod_infos.iter().filter(|m| m.enabled && !m.errors.is_empty()).collect();
    if let Some(m) = broken.first() {
        app.status = Some((format!("Mod \"{}\" has {} problem(s): see the Mods screen.", m.name, m.errors.len()), 8.0));
    }

    if let Some(s) = &shot {
        match s.mode.as_str() {
            "survival" | "creative" | "inventory" | "night" => {
                let mut g = Game::new(424242, s.mode == "creative", false);
                if s.mode == "inventory" {
                    for (item, n) in [(LOG, 12), (COBBLE, 20), (COAL, 5), (IRON, 3), (DIAMOND, 2), (GUNPOWDER, 5), (SAND, 9), (PORKCHOP, 3)] {
                        g.inv.add(item, n);
                    }
                }
                if let Some(t) = s.time {
                    g.time = t;
                }
                g.start_scripts();
                app.start_game(g);
                app.show_debug = true;
                if s.mode == "inventory" {
                    app.set_screen(Screen::Inventory);
                }
            }
            "host" => {
                app.start_game(Game::new(424242, true, false));
                app.game.open_lan("Hosty", None).expect("open to LAN");
                app.show_debug = true;
            }
            "showcase" => {
                app.start_game(Game::new(424242, true, false));
                app.game.open_lan("Hosty", None).expect("open to LAN");
                app.show_debug = false;
            }
            "parody" => {
                app.start_game(Game::new(424242, true, false));
                app.show_debug = false;
            }
            "advancements" => {
                let mut g = Game::new(424242, false, false);
                for a in advancements::ALL.iter().step_by(3) {
                    g.advancements.grant(a.key);
                }
                app.start_game(g);
                app.set_screen(Screen::Advancements);
            }
            "mods" => {
                app.game = Game::new(424242, true, true);
                app.set_screen(Screen::Mods);
            }
            "worlds" => {
                app.game = Game::new(424242, true, true);
                app.open_worlds();
            }
            "createform" => {
                app.game = Game::new(424242, true, true);
                app.form_name = "Cheese Kingdom".into();
                app.form_seed = "cheese".into();
                app.form_focus = 1;
                app.set_screen(Screen::CreateWorld);
            }
            "newworld" => {
                // Goes through the real "Create World" path.
                app.form_name = "Harness Test World".into();
                app.form_seed = "cheese".into();
                app.form_creative = true;
                app.create_world();
                app.show_debug = true;
            }
            "palette" => {
                app.start_game(Game::new(424242, true, false));
                app.set_screen(Screen::Inventory);
            }
            "internet" => {
                app.start_game(Game::new(424242, true, false));
                app.mp_password = "sekrit".into();
                app.open_to_internet();
                app.set_screen(Screen::Paused);
            }
            "join" => {
                app.mp_name = "Joiny".into();
                app.mp_password = s.password.clone();
                app.connect_next = Some(s.addr.clone());
                app.set_screen(Screen::Multiplayer);
            }
            "options" => {
                app.game = Game::new(424242, true, true);
                app.set_screen(Screen::Options { from_title: true });
            }
            _ => {
                app.game = Game::new(424242, true, true);
            }
        }
    }

    let mut frames = 0u32;
    loop {
        if let Some(s) = &shot {
            if !matches!(s.mode.as_str(), "title" | "inventory" | "join" | "internet" | "mods" | "palette" | "worlds" | "newworld" | "createform") || (s.mode == "join" && app.game.is_client()) {
                // Keep the demo camera looking at something interesting.
                app.game.player.pitch = s.pitch;
                app.game.player.yaw = s.yaw;
                if let Some(p) = s.pos {
                    app.game.player.body.pos = p;
                    app.game.player.body.vel = Vec3::ZERO;
                    app.game.player.flying = true;
                }
            }
            if frames >= 150 && frames < 150 + s.chat.len() as u32 {
                // Type the scripted chat lines one per frame.
                let line = s.chat[(frames - 150) as usize].clone();
                app.game.send_chat(&line);
            }
            if (s.mode == "showcase" || s.mode == "parody") && frames == 120 {
                // A little display of every mod block (or the newest base ones), on stone plinths in front of the player.
                let p = app.game.player.body.pos;
                let (fx, fz) = (2.4f32.sin(), -2.4f32.cos());
                let r = block::reg();
                let ids = if s.mode == "parody" { block::GOLD_ORE..block::NUM_BLOCKS } else { block::NUM_BLOCKS..r.blocks.len() as block::Id };
                let n = ids.len() as f32;
                for (i, id) in ids.enumerate() {
                    let side = if s.mode == "parody" { (i as f32 - (n - 1.0) / 2.0) * 1.3 } else { i as f32 * 1.6 - 3.0 };
                    let at = p + Vec3::new(fx * 5.0 - fz * side, 0.0, fz * 5.0 + fx * side);
                    let (x, y, z) = (at.x.floor() as i32, at.y.floor() as i32, at.z.floor() as i32);
                    app.game.world.set(x, y, z, block::STONE);
                    app.game.world.set(x, y + 1, z, id);
                }
            }
            if s.mode == "parody" && frames == 140 {
                app.game.advance("dimonds");
            }
            if matches!(s.mode.as_str(), "survival" | "creative" | "parody") && frames == 150 {
                let p = app.game.player.body.pos;
                let mut rng = noise::Rng::new(9);
                let dist = if s.mode == "parody" { 9.0 } else { 6.0 };
                for (i, kind) in entity::MobKind::ALL.into_iter().enumerate() {
                    let at = p + Vec3::new(2.4f32.sin() * dist + (i as f32 - 2.0) * 2.4, 2.0, -2.4f32.cos() * dist);
                    let m = entity::Mob::new(kind, at, &mut rng);
                    app.game.mobs.push(m);
                }
            }
        }
        app.frame();
        frames += 1;
        if let Some(s) = &shot {
            if frames == s.frames {
                get_screen_data().export_png(&s.path);
                break;
            }
        }
        if app.quit || is_quit_requested() {
            if !app.game.menu && !app.game.is_client() {
                app.save();
            }
            app.game.disconnect();
            break;
        }
        next_frame().await;
    }
}
