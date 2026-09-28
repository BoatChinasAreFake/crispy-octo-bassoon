//! MINCERAFT — a native, browser-free block game parody.
//! Rust + raw OpenGL (via miniquad/macroquad). No asset files: everything is
//! generated at startup.

mod block;
mod entity;
mod game;
mod inventory;
mod mesher;
mod multiplayer;
mod net;
mod noise;
mod player;
mod render;
mod save;
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
    Multiplayer,
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

    fn new_world(&mut self, creative: bool) {
        let mut g = Game::new(random_seed(), creative, false);
        g.msg(if creative {
            "Creative mode: infinite blocks, zero consequences. Double-tap Space to fly."
        } else {
            "Survival mode: punch a tree. Press E to craft. Avoid anything that hisses."
        });
        self.start_game(g);
    }

    fn back_to_title(&mut self) {
        if !self.game.menu && !self.game.is_client() {
            self.save();
        }
        self.game.disconnect();
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
        self.game = Game::new(random_seed(), true, true);
        self.splash = SPLASHES[(random_seed() as usize) % SPLASHES.len()];
        self.set_screen(Screen::Title);
    }

    fn save(&mut self) {
        if self.game.is_client() {
            self.status = Some(("Only the host can save this world.".into(), 3.0));
            return;
        }
        let data = self.game.to_save();
        match save::write(&data) {
            Ok(()) => self.status = Some(("World saved.".into(), 3.0)),
            Err(e) => self.status = Some((format!("Save failed: {e}"), 6.0)),
        }
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
            Screen::Options { from_title } | Screen::Help { from_title } => {
                if is_key_pressed(KeyCode::Escape) {
                    self.set_screen(if from_title { Screen::Title } else { Screen::Paused });
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
        let world_audible = !matches!(self.screen, Screen::Paused | Screen::Options { .. } | Screen::Help { .. });
        for (s, at) in std::mem::take(&mut self.game.sounds) {
            if world_audible || s == Sfx::Craft {
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
            Screen::Multiplayer => {
                draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.6));
                self.multiplayer_screen();
            }
            _ => {
                self.hud();
                match self.screen {
                    Screen::Paused => self.pause_screen(),
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
        let mut y = h * 0.40;
        let has_save = save::exists();
        if self.ui.button(Rect::new(x, y, bw, bh), "Continue Saved World", has_save) {
            match save::read() {
                Ok(d) => {
                    let g = Game::from_save(d);
                    self.start_game(g);
                }
                Err(e) => self.status = Some((format!("Couldn't load save: {e}"), 5.0)),
            }
        }
        y += bh + 5.0 * s;
        if self.ui.button(Rect::new(x, y, bw, bh), "New Survival World", true) {
            self.new_world(false);
            return;
        }
        y += bh + 5.0 * s;
        if self.ui.button(Rect::new(x, y, bw, bh), "New Creative World", true) {
            self.new_world(true);
            return;
        }
        y += bh + 5.0 * s;
        if self.ui.button(Rect::new(x, y, bw, bh), "Multiplayer (LAN)", true) {
            drain_chars();
            self.set_screen(Screen::Multiplayer);
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
        if self.ui.button(Rect::new(x, y, bw, bh), "Options", true) {
            self.set_screen(Screen::Options { from_title: false });
        }
        y += bh + 5.0 * s;
        if self.ui.button(Rect::new(x, y, bw, bh), "How to Play", true) {
            self.set_screen(Screen::Help { from_title: false });
        }
        y += bh + 5.0 * s;
        let quit_label = if client { "Disconnect" } else { "Save and Quit to Title" };
        if self.ui.button(Rect::new(x, y, bw, bh), quit_label, true) {
            self.back_to_title();
        }
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
            "Hissers explode. Groaners bite and burn in daylight. Oinkers are friends (and food).",
            "Crafting works anywhere. The crafting table is purely decorative. Satire!",
            "Multiplayer: host opens their world with Esc > Open to LAN; friends use Multiplayer. T to chat.",
        ];
        for (i, l) in lines.iter().enumerate() {
            self.ui.text_centered(l, w / 2.0, h * 0.2 + i as f32 * 13.0 * s, 9.0, if l.is_empty() { WHITE } else { Color::new(0.9, 0.9, 0.9, 1.0) });
        }
        let bw = (160.0 * s).min(w * 0.8);
        if self.ui.button(Rect::new(w / 2.0 - bw / 2.0, h * 0.2 + 14.0 * 13.0 * s, bw, 20.0 * s), "Got it", true) {
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
            let max_scroll = RECIPES.len().saturating_sub(visible_rows) as f32;
            if self.ui.hovered(Rect::new(rx, y0, right_w, panel_h)) {
                let wheel = mouse_wheel().1;
                if wheel.abs() > 0.1 {
                    self.recipe_scroll = (self.recipe_scroll - wheel.signum()).clamp(0.0, max_scroll);
                }
            }
            let first = self.recipe_scroll as usize;
            // Craftable recipes first so progress is obvious.
            let mut order: Vec<usize> = (0..RECIPES.len()).collect();
            order.sort_by_key(|&i| !self.game.inv.can_craft(&RECIPES[i]));
            for (row, &ri) in order.iter().skip(first).take(visible_rows).enumerate() {
                let r = &RECIPES[ri];
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
                for &(item, n) in r.inputs {
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
                        self.game.msg(format!("Crafted {}. Nobody knows how.", item_name(r.output.0)));
                        self.game.sfx(Sfx::Craft, None);
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
    let atlas = texture::build_atlas(1337);
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
        splash: SPLASHES[(random_seed() as usize) % SPLASHES.len()],
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
    };

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
            if !matches!(s.mode.as_str(), "title" | "inventory" | "join" | "internet") || (s.mode == "join" && app.game.is_client()) {
                // Keep the demo camera looking at something interesting.
                app.game.player.pitch = s.pitch;
                app.game.player.yaw = s.yaw;
                if let Some(p) = s.pos {
                    app.game.player.body.pos = p;
                    app.game.player.body.vel = Vec3::ZERO;
                    app.game.player.flying = true;
                }
            }
            if (s.mode == "survival" || s.mode == "creative") && frames == 150 {
                let p = app.game.player.body.pos;
                let mut rng = noise::Rng::new(9);
                for (i, kind) in [entity::MobKind::Oinker, entity::MobKind::Hisser, entity::MobKind::Groaner].into_iter().enumerate() {
                    let at = p + Vec3::new(2.4f32.sin() * 6.0 + (i as f32 - 1.0) * 2.0, 2.0, -2.4f32.cos() * 6.0);
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
