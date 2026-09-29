//! MINCERAFT — a native, browser-free block game parody.
//! Rust + raw OpenGL (via miniquad/macroquad). No asset files: everything is
//! generated at startup.

mod admin;
mod access;
mod advancements;
mod animals;
mod anvil;
mod beacon;
mod block;
mod building;
mod carpentry;
mod combat;
mod containers;
mod contraptions;
mod decor;
mod drops;
mod enchant;
mod entity;
mod farming;
mod fire;
mod fishing;
mod game;
mod golems;
mod hollow;
mod hoppers;
mod horses;
mod hunger;
mod inventory;
mod keybinds;
mod ledger;
mod light;
mod liquids;
mod mesher;
mod mods;
mod multiplayer;
mod nametags;
mod navigation;
mod net;
mod noise;
mod pad;
mod palette;
mod player;
mod potions;
mod players;
mod regions;
mod render;
mod rules;
mod save;
mod scorch;
mod scripting;
mod server;
mod structures;
mod trees;
mod settings;
mod upnp;
mod vehicles;
mod villagers;
mod sound;
mod texture;
mod ui;
mod weather;
mod wiring;
mod world;
mod xp;

use block::*;
use game::{Controls, Game};
use macroquad::prelude::*;
use player::Input;
use render::Renderer;
use sound::{Audio, Sfx};
use settings::Settings;
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
    /// A chest or furnace is open (see containers.rs).
    Container,
    /// An anvil is open (see anvil.rs).
    Anvil,
    /// An enchanting table is open (see enchant.rs).
    Enchant,
    /// Trading with a Hmmer (see villagers.rs).
    Trade,
    /// Writing on a sign (see decor.rs).
    Sign,
    /// Writing a Name Tag for a mob (see nametags.rs).
    NameTag,
    /// Keep inventory, difficulty, daylight cycle (the world's owner can change them).
    WorldSettings,
    Dead,
    Options { from_title: bool },
    /// Rebinding keys and buttons (reached from Options).
    Controls { from_title: bool },
    Help { from_title: bool },
    Advancements,
    FishLog,
    Multiplayer,
    Mods,
    Worlds,
    CreateWorld,
    RenameWorld,
    DeleteWorld,
}

struct App {
    screen: Screen,
    game: Game,
    renderer: Renderer,
    ui: Ui,
    settings: Settings,
    /// Screenshot runs neither read nor write settings.txt.
    no_settings_file: bool,
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
    /// Lines already sent (Up/Down bring them back), which one is showing,
    /// and how far the chat log is scrolled back.
    chat_sent: Vec<String>,
    /// The name being written on a Name Tag.
    name_line: String,
    /// Sound subtitles on screen (see access.rs).
    captions: access::Captions,
    chat_pick: Option<usize>,
    chat_scroll: usize,
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
    /// Map colours per block, the map picture, and seconds until it's redrawn (see navigation.rs).
    map_colors: Vec<[u8; 3]>,
    map_tex: Option<Texture2D>,
    map_timer: f32,
    /// Lines being written on a sign, and which line.
    sign_lines: [String; 4],
    sign_line: usize,
    /// A game controller, if one is plugged in, and what it did this frame.
    pad: pad::Pad,
    pad_frame: pad::PadFrame,
    /// The action (and which of its two slots) waiting for a new key.
    rebinding: Option<(keybinds::Action, bool)>,
    /// Skip the click that started rebinding, so it isn't taken as the new binding.
    rebind_armed: bool,
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
    form_keep: bool,
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
        if self.screen == Screen::Container && s != Screen::Container {
            self.game.close_container();
        }
        if self.screen == Screen::Anvil && s != Screen::Anvil {
            self.game.close_anvil();
        }
        if self.screen == Screen::Enchant && s != Screen::Enchant {
            self.game.close_enchanting();
        }
        if self.screen == Screen::Trade && s != Screen::Trade {
            self.game.trading = None;
            self.game.inv.return_cursor();
        }
        if self.screen == Screen::Sign && s != Screen::Sign {
            // Done writing: put it on the sign.
            if let Some(pos) = self.game.editing_sign.take() {
                let lines = self.sign_lines.clone();
                self.game.set_sign(pos, &lines);
            }
        }
        if s == Screen::Sign {
            self.sign_lines = Default::default();
            self.sign_line = 0;
            drain_chars();
        }
        // Leaving a screen where settings change: keep them for next time.
        if matches!(self.screen, Screen::Options { .. } | Screen::Controls { .. } | Screen::Multiplayer) && self.screen != s {
            self.save_settings();
        }
        self.screen = s;
    }

    /// Everything worth remembering between runs, gathered from where it lives.
    fn current_settings(&self) -> Settings {
        Settings { volume: self.audio.volume, music_on: self.audio.music_on, mp_name: self.mp_name.clone(), mp_addr: self.mp_addr.clone(), ..self.settings.clone() }
    }

    fn save_settings(&mut self) {
        if self.no_settings_file {
            return;
        }
        if let Err(e) = self.current_settings().save(&settings::path()) {
            self.status = Some((format!("Couldn't save settings: {e}"), 5.0));
        }
    }

    fn start_game(&mut self, game: Game) {
        let mut gl = unsafe { get_internal_gl() };
        gl.flush();
        self.renderer.clear(gl.quad_context);
        self.game = game;
        // A world of our own keeps its block edits in region files beside its save.
        if let Some(id) = &self.current_world {
            let dir = regions::region_dir(&save::world_file(&save::saves_dir(), id));
            self.game.world.use_regions(dir);
        }
        // We look like our settings say (joined players tell the host).
        let skin = self.settings.skin;
        self.game.set_skin(skin);
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
        self.new_world(self.form_creative, self.form_keep, seed);
        // Save straight away so it's in the list even if the game is closed abruptly.
        self.save_quietly();
    }

    fn new_world(&mut self, creative: bool, keep_inventory: bool, seed: u32) {
        let mut g = Game::new(seed, creative, false);
        g.rules.keep_inventory = keep_inventory;
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
        if let Err(e) = self.game.world.flush_regions() {
            return Some(Err(e));
        }
        let data = self.game.to_save();
        Some(save::write_to(&save::world_file(&save::saves_dir(), &id), &data))
    }

    fn controls(&mut self) -> Controls {
        use keybinds::Action as A;
        let playing = self.screen == Screen::Playing && self.chat.is_none() && self.rebinding.is_none();
        let b = &self.settings.binds;
        let down = |a: A| playing && b.down(a);
        let hit = |a: A| playing && b.pressed(a);
        let pad = if playing { self.pad_frame } else { pad::PadFrame::default() };
        let axis = |plus: A, minus: A, stick: f32| (down(plus) as i32 - down(minus) as i32) as f32 + stick;
        Controls {
            input: Input {
                forward: axis(A::Forward, A::Back, pad.walk[1]).clamp(-1.0, 1.0),
                strafe: axis(A::Right, A::Left, pad.walk[0]).clamp(-1.0, 1.0),
                jump: down(A::Jump) || pad.jump,
                jump_pressed: hit(A::Jump) || pad.jump_pressed,
                sneak: down(A::Sneak) || pad.sneak,
                sprint: down(A::Sprint) || pad.sprint,
            },
            attack_held: down(A::Attack) || pad.attack,
            attack_pressed: hit(A::Attack) || pad.attack_pressed,
            use_held: down(A::Use) || pad.use_held,
            use_pressed: hit(A::Use) || pad.use_pressed,
            pick: hit(A::PickBlock),
            drop: hit(A::Drop) || pad.drop,
            drop_all: is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl),
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
        // The right stick looks around, faster the further it's pushed.
        let look = self.pad_frame.look;
        if self.screen == Screen::Playing && look != [0.0; 2] {
            let s = get_frame_time().min(0.05) * 2.8 * self.settings.sensitivity;
            let p = &mut self.game.player;
            p.yaw = (p.yaw + look[0] * s).rem_euclid(std::f32::consts::TAU);
            p.pitch = (p.pitch + look[1] * s * 0.8).clamp(-1.55, 1.55);
        }
    }

    fn handle_keys(&mut self) {
        if let Some(line) = &mut self.chat {
            type_into(line, 200);
            // Up and Down bring back what you said before; Page Up/Down and the wheel scroll the log.
            let recall = if is_key_pressed(KeyCode::Up) && !self.chat_sent.is_empty() {
                Some(self.chat_pick.map_or(0, |p| p + 1).min(self.chat_sent.len() - 1))
            } else if is_key_pressed(KeyCode::Down) {
                self.chat_pick.and_then(|p| p.checked_sub(1))
            } else {
                self.chat_pick
            };
            if recall != self.chat_pick {
                self.chat_pick = recall;
                match recall {
                    Some(p) => *line = self.chat_sent[self.chat_sent.len() - 1 - p].clone(),
                    None => line.clear(),
                }
            }
            let wheel = mouse_wheel().1;
            if is_key_pressed(KeyCode::PageUp) || wheel > 0.1 {
                self.chat_scroll = (self.chat_scroll + 3).min(self.game.chat_log.len().saturating_sub(1));
            } else if is_key_pressed(KeyCode::PageDown) || wheel < -0.1 {
                self.chat_scroll = self.chat_scroll.saturating_sub(3);
            }
            if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter) {
                let text = line.clone();
                self.chat = None;
                if !text.trim().is_empty() && self.chat_sent.last() != Some(&text) {
                    self.chat_sent.push(text.clone());
                    if self.chat_sent.len() > 50 {
                        self.chat_sent.remove(0);
                    }
                }
                self.chat_pick = None;
                self.chat_scroll = 0;
                self.game.send_chat(&text);
                self.set_screen(Screen::Playing);
            } else if is_key_pressed(KeyCode::Escape) {
                self.chat = None;
                self.chat_pick = None;
                self.chat_scroll = 0;
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
                let binds = &self.settings.binds;
                let command = binds.pressed(keybinds::Action::Command);
                if binds.pressed(keybinds::Action::Chat) || command {
                    drain_chars();
                    let start = if command { "/" } else { "" };
                    self.chat = Some(start.to_string());
                    self.set_screen(Screen::Playing);
                    return;
                }
                if is_key_pressed(KeyCode::Escape) || self.pad_frame.pause {
                    self.set_screen(Screen::Paused);
                } else if self.settings.binds.pressed(keybinds::Action::Inventory) || self.pad_frame.inventory {
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
                let step = if wheel.abs() > 0.1 { -wheel.signum() as i32 } else { self.pad_frame.hotbar };
                if step != 0 {
                    self.game.inv.selected = (self.game.inv.selected as i32 + step).rem_euclid(9) as usize;
                    self.game.held_name = 2.0;
                }
                if self.settings.binds.pressed(keybinds::Action::Perspective) || self.pad_frame.perspective {
                    self.game.third_person = !self.game.third_person;
                }
            }
            Screen::Inventory | Screen::Container | Screen::Anvil | Screen::Enchant | Screen::Trade => {
                let pad = self.pad_frame;
                if is_key_pressed(KeyCode::Escape) || self.settings.binds.pressed(keybinds::Action::Inventory) || pad.inventory || pad.back {
                    self.set_screen(Screen::Playing);
                }
            }
            Screen::Paused => {
                if is_key_pressed(KeyCode::Escape) || self.pad_frame.pause || self.pad_frame.back {
                    self.set_screen(Screen::Playing);
                }
            }
            Screen::NameTag => {
                if is_key_pressed(KeyCode::Escape) {
                    self.finish_name_tag(false);
                } else if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter) {
                    self.finish_name_tag(true);
                } else {
                    type_into(&mut self.name_line, nametags::NAME_LEN);
                }
            }
            Screen::Sign => {
                if is_key_pressed(KeyCode::Escape) {
                    self.set_screen(Screen::Playing);
                } else if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter) || is_key_pressed(KeyCode::Down) {
                    self.sign_line = (self.sign_line + 1) % 4;
                } else if is_key_pressed(KeyCode::Up) {
                    self.sign_line = (self.sign_line + 3) % 4;
                } else {
                    type_into(&mut self.sign_lines[self.sign_line], decor::LINE_LEN);
                }
            }
            Screen::Advancements | Screen::FishLog | Screen::WorldSettings => {
                if is_key_pressed(KeyCode::Escape) {
                    self.set_screen(Screen::Paused);
                }
            }
            Screen::Controls { from_title } => {
                // Esc while waiting for a key clears that slot (see `controls_screen`).
                if self.rebinding.is_none() && is_key_pressed(KeyCode::Escape) {
                    self.set_screen(Screen::Options { from_title });
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
        self.pad_frame = self.pad.poll();
        self.handle_keys();
        self.mouse_look();

        let controls = self.controls();
        // Multiplayer worlds never pause: other people are still in them.
        let simulate = matches!(self.screen, Screen::Playing | Screen::Inventory | Screen::Container | Screen::Anvil | Screen::Enchant | Screen::Trade | Screen::Title | Screen::Dead) || self.game.net.is_some();
        if simulate {
            self.game.update(dt, &controls);
        }
        // Right-clicked a chest or furnace: show it. Broken under us: close it.
        if self.game.open.is_some() && self.screen == Screen::Playing {
            self.set_screen(Screen::Container);
        } else if self.screen == Screen::Container && !self.game.container_still_there() {
            self.set_screen(Screen::Playing);
        }
        if self.game.anvil.is_some() && self.screen == Screen::Playing {
            self.set_screen(Screen::Anvil);
        } else if self.screen == Screen::Anvil && !self.game.anvil_still_there() {
            self.set_screen(Screen::Playing);
        }
        if self.game.enchanting.is_some() && self.screen == Screen::Playing {
            self.set_screen(Screen::Enchant);
        } else if self.screen == Screen::Enchant && !self.game.enchanting_still_there() {
            self.set_screen(Screen::Playing);
        }
        if self.game.editing_sign.is_some() && self.screen == Screen::Playing {
            self.set_screen(Screen::Sign);
        }
        if self.game.naming.is_some() && self.screen == Screen::Playing {
            drain_chars();
            self.set_screen(Screen::NameTag);
        }
        if self.game.trading.is_some() && self.screen == Screen::Playing {
            self.set_screen(Screen::Trade);
        } else if self.screen == Screen::Trade && self.game.trade_list().is_none() {
            self.set_screen(Screen::Playing);
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
            self.map_colors = navigation::block_colors(&atlas);
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
            let Some(net::Msg::Welcome { id, seed, time, creative, spawn, keep_inventory }) = msgs.pop() else { return };
            let (conn, _) = self.joining.take().unwrap();
            let mut g = Game::new_client(id, seed, time, creative, spawn, conn, &self.mp_name, leftover);
            g.rules.keep_inventory = keep_inventory;
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
            self.form_keep = false;
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
        y += bh + 4.0 * s;
        let keep = if self.form_keep { "Keep Inventory: ON (dying costs nothing)" } else { "Keep Inventory: OFF (you drop everything)" };
        if self.ui.button(Rect::new(x, y, bw, bh), keep, true) {
            self.form_keep = !self.form_keep;
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
    /// Words on signs nearby, floating where the sign is.
    fn sign_text(&self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        let eye = self.game.player.eye();
        for (pos, lines) in &self.game.world.signs {
            let at = pos.as_vec3() + Vec3::new(0.5, 0.78, 0.5);
            let d = at.distance(eye);
            if d > 16.0 || lines.iter().all(|l| l.is_empty()) {
                continue;
            }
            let clip = self.last_view_proj * at.extend(1.0);
            if clip.w < 0.1 {
                continue;
            }
            let (sx, sy) = ((clip.x / clip.w * 0.5 + 0.5) * w, (0.5 - clip.y / clip.w * 0.5) * h);
            // Smaller further away.
            let size = (9.0 * 6.0 / d.max(2.0)).clamp(5.0, 11.0);
            let line_h = size * 1.25 * s;
            let wmax = lines.iter().map(|l| self.ui.text_width(l, size)).fold(0.0, f32::max);
            let top = sy - line_h * 2.0;
            draw_rectangle(sx - wmax / 2.0 - 3.0 * s, top - line_h * 0.8, wmax + 6.0 * s, line_h * 4.0 + 2.0 * s, Color::new(0.35, 0.25, 0.12, 0.55));
            for (i, l) in lines.iter().enumerate() {
                self.ui.text_centered(l, sx, top + i as f32 * line_h, size, WHITE);
            }
        }
    }

    fn sign_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.55));
        let (bw, bh) = (220.0 * s, 110.0 * s);
        let (x0, y0) = ((w - bw) / 2.0, (h - bh) / 2.0 - 20.0 * s);
        draw_rectangle(x0, y0, bw, bh, Color::new(0.62, 0.47, 0.28, 1.0));
        draw_rectangle_lines(x0, y0, bw, bh, 2.0 * s, Color::new(0.35, 0.24, 0.12, 1.0));
        self.ui.text_centered("Write on the sign (Enter: next line, Esc: done)", w / 2.0, y0 - 8.0 * s, 9.0, WHITE);
        for (i, l) in self.sign_lines.iter().enumerate() {
            let y = y0 + 24.0 * s + i as f32 * 22.0 * s;
            let cursor = if i == self.sign_line && (get_time() * 2.0) as i64 % 2 == 0 { "_" } else { "" };
            self.ui.text_centered(&format!("{l}{cursor}"), w / 2.0, y, 11.0, Color::new(0.1, 0.07, 0.03, 1.0));
        }
        if self.ui.button(Rect::new(w / 2.0 - 50.0 * s, y0 + bh + 10.0 * s, 100.0 * s, 20.0 * s), "Done", true) {
            self.set_screen(Screen::Playing);
        }
    }

    /// Writing a Name Tag for a mob.
    fn name_tag_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.55));
        let (bw, bh) = (200.0 * s, 34.0 * s);
        let (x0, y0) = ((w - bw) / 2.0, (h - bh) / 2.0 - 20.0 * s);
        draw_rectangle(x0, y0, bw, bh, Color::new(0.84, 0.75, 0.55, 1.0));
        draw_rectangle_lines(x0, y0, bw, bh, 2.0 * s, Color::new(0.45, 0.35, 0.2, 1.0));
        self.ui.text_centered("Name it (Enter: done, Esc: never mind)", w / 2.0, y0 - 8.0 * s, 9.0, WHITE);
        let cursor = if (get_time() * 2.0) as i64 % 2 == 0 { "_" } else { "" };
        self.ui.text_centered(&format!("{}{cursor}", self.name_line), w / 2.0, y0 + 22.0 * s, 11.0, Color::new(0.1, 0.07, 0.03, 1.0));
        if self.ui.button(Rect::new(w / 2.0 - 50.0 * s, y0 + bh + 10.0 * s, 100.0 * s, 20.0 * s), "Done", true) {
            self.finish_name_tag(true);
        }
    }

    /// Leave the Name Tag screen, naming the mob or not.
    fn finish_name_tag(&mut self, confirm: bool) {
        if let Some(id) = self.game.naming.take()
            && confirm
        {
            let name = self.name_line.clone();
            self.game.name_mob(id, &name);
        }
        self.name_line.clear();
        self.set_screen(Screen::Playing);
    }

    /// Sound subtitles, bottom right (see access.rs).
    fn captions_hud(&self) {
        let (w, h, s) = (screen_width(), screen_height(), self.ui.s);
        for (i, (text, side, left)) in self.captions.lines.iter().rev().enumerate() {
            let line = match side {
                -1 => format!("< {text}"),
                1 => format!("{text} >"),
                _ => text.to_string(),
            };
            let a = left.min(1.0);
            let tw = self.ui.text_width(&line, 8.0);
            let (x, y) = (w - tw - 10.0 * s, h - 60.0 * s - i as f32 * 10.0 * s);
            draw_rectangle(x - 3.0 * s, y - 8.0 * s, tw + 6.0 * s, 10.0 * s, Color::new(0.0, 0.0, 0.0, 0.6 * a));
            self.ui.text(&line, x, y, 8.0, Color::new(1.0, 1.0, 1.0, a));
        }
    }

    /// The Hollow Wyrm's health, across the top while it's near.
    fn boss_bar(&self) {
        let g = &self.game;
        let Some(m) = g.mobs.iter().find(|m| m.kind == entity::MobKind::Wyrm && m.body.pos.distance(g.player.body.pos) < 150.0) else { return };
        let (w, s) = (screen_width(), self.ui.s);
        let bw = (180.0 * s).min(w * 0.7);
        let (x, y) = (w / 2.0 - bw / 2.0, 18.0 * s);
        let frac = (m.health / m.kind.max_health()).clamp(0.0, 1.0);
        self.ui.text_centered("Hollow Wyrm", w / 2.0, y - 3.0 * s, 9.0, Color::new(0.85, 0.6, 1.0, 1.0));
        draw_rectangle(x, y, bw, 5.0 * s, Color::new(0.1, 0.05, 0.12, 0.9));
        draw_rectangle(x, y, bw * frac, 5.0 * s, Color::new(0.75, 0.25, 0.9, 1.0));
    }

    /// Potion effects and their time left, down the left side.
    fn effects_hud(&self) {
        let s = self.ui.s;
        let h = screen_height();
        for (i, (p, left)) in self.game.effects.iter().enumerate() {
            let y = h * 0.35 + i as f32 * 12.0 * s;
            let c = p.colour();
            let secs = left.max(0.0) as i32;
            draw_rectangle(4.0 * s, y - 7.0 * s, 6.0 * s, 6.0 * s, Color::from_rgba(c[0], c[1], c[2], 255));
            self.ui.text(&format!("{} {}:{:02}", p.name(), secs / 60, secs % 60), 13.0 * s, y, 8.0, WHITE);
        }
    }

    /// A compass and a map, while you hold them.
    fn navigation_hud(&mut self, dt: f32) {
        let (w, _) = (screen_width(), screen_height());
        let s = self.ui.s;
        let held = self.game.inv.held();
        if held == block::COMPASS {
            let r = 22.0 * s;
            let (cx, cy) = (w / 2.0, 34.0 * s);
            draw_circle(cx, cy, r + 2.0 * s, Color::new(0.2, 0.2, 0.22, 0.9));
            draw_circle(cx, cy, r, Color::new(0.92, 0.9, 0.84, 0.95));
            let g = &self.game;
            let a = navigation::compass_needle(g.player.body.pos, g.player.yaw, g.spawn, g.clock);
            let (dx, dy) = (a.sin(), -a.cos());
            draw_line(cx, cy, cx + dx * r * 0.85, cy + dy * r * 0.85, 3.0 * s, Color::new(0.85, 0.1, 0.1, 1.0));
            draw_line(cx, cy, cx - dx * r * 0.5, cy - dy * r * 0.5, 3.0 * s, Color::new(0.3, 0.3, 0.35, 1.0));
            if !scorch::in_scorch(g.player.body.pos.x) {
                let dist = Vec2::new(g.spawn.x - g.player.body.pos.x, g.spawn.z - g.player.body.pos.z).length();
                self.ui.text_centered(&format!("Home: {} blocks", dist as i32), cx, cy + r + 12.0 * s, 8.0, WHITE);
            }
        }
        if held == block::MAP {
            self.map_timer -= dt;
            if self.map_timer <= 0.0 || self.map_tex.is_none() {
                self.map_timer = 0.5;
                let px = navigation::map_pixels(&self.game.world, self.game.player.body.pos, &self.map_colors);
                let n = navigation::MAP_SIZE as u16;
                match &self.map_tex {
                    Some(t) => t.update_from_bytes(n as u32, n as u32, &px),
                    None => {
                        let t = Texture2D::from_rgba8(n, n, &px);
                        t.set_filter(FilterMode::Nearest);
                        self.map_tex = Some(t);
                    }
                }
            }
            if let Some(t) = &self.map_tex {
                let size = 150.0 * s;
                let (x, y) = (w - size - 10.0 * s, 10.0 * s);
                draw_rectangle(x - 4.0 * s, y - 4.0 * s, size + 8.0 * s, size + 8.0 * s, Color::new(0.55, 0.43, 0.26, 1.0));
                draw_texture_ex(t, x, y, WHITE, DrawTextureParams { dest_size: Some(vec2(size, size)), ..Default::default() });
                // You are here (pointing the way you face).
                let (cx, cy) = (x + size / 2.0, y + size / 2.0);
                let yaw = self.game.player.yaw;
                let (fx, fy) = (yaw.sin(), -yaw.cos());
                let (rx, ry) = (-fy, fx);
                let k = 6.0 * s;
                draw_triangle(vec2(cx + fx * k, cy + fy * k), vec2(cx - fx * k * 0.6 + rx * k * 0.6, cy - fy * k * 0.6 + ry * k * 0.6), vec2(cx - fx * k * 0.6 - rx * k * 0.6, cy - fy * k * 0.6 - ry * k * 0.6), Color::new(0.9, 0.1, 0.1, 1.0));
            }
        }
    }

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
        // Mobs with Name Tags.
        for m in &self.game.mobs {
            let Some(name) = self.game.mob_names.get(&m.id) else { continue };
            let at = m.body.pos + Vec3::Y * (m.body.height + 0.4);
            if at.distance(eye) > 32.0 {
                continue;
            }
            let clip = self.last_view_proj * at.extend(1.0);
            if clip.w < 0.1 {
                continue;
            }
            let (sx, sy) = ((clip.x / clip.w * 0.5 + 0.5) * w, (0.5 - clip.y / clip.w * 0.5) * h);
            let tw = self.ui.text_width(name, 8.0);
            draw_rectangle(sx - tw / 2.0 - 3.0 * s, sy - 9.0 * s, tw + 6.0 * s, 11.0 * s, Color::new(0.0, 0.0, 0.0, 0.4));
            self.ui.text_centered(name, sx, sy, 8.0, Color::new(1.0, 0.95, 0.8, 1.0));
        }
    }

    fn play_sounds(&mut self, dt: f32) {
        let listener = self.game.player.eye();
        if self.ui.pressed.replace(false) {
            self.audio.play(Sfx::Click, None, listener);
        }
        // Keep the game world quiet while paused or in menus layered over it.
        let world_audible = !matches!(self.screen, Screen::Paused | Screen::Options { .. } | Screen::Controls { .. } | Screen::Help { .. } | Screen::Advancements | Screen::FishLog);
        let yaw = self.game.player.yaw;
        for (s, at) in std::mem::take(&mut self.game.sounds) {
            if world_audible || s == Sfx::Craft || s == Sfx::Fanfare {
                self.audio.play(s, at, listener);
                // Captions for what's heard (only what's close enough to hear).
                let heard = at.is_none_or(|p| p.distance(listener) < 24.0);
                if self.settings.subtitles && heard && let Some(text) = access::caption(s) {
                    self.captions.add(text, access::side(listener, yaw, at));
                }
            }
        }
        self.captions.tick(dt);
        self.game.colour_blind = self.settings.colour_blind;
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
            Screen::Controls { from_title } => {
                draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.7));
                self.controls_screen(from_title);
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
                if !self.game.menu && self.game.ready {
                    self.navigation_hud(get_frame_time().min(0.05));
                    self.effects_hud();
                    self.boss_bar();
                    self.captions_hud();
                }
                match self.screen {
                    Screen::Paused => self.pause_screen(),
                    Screen::Advancements => self.advancements_screen(),
                    Screen::FishLog => self.fish_log_screen(),
                    Screen::Inventory => self.inventory_screen(),
                    Screen::Container => self.container_screen(),
                    Screen::Anvil => self.anvil_screen(),
                    Screen::Enchant => self.enchant_screen(),
                    Screen::Trade => self.trade_screen(),
                    Screen::Sign => self.sign_screen(),
                    Screen::NameTag => self.name_tag_screen(),
                    Screen::WorldSettings => self.world_settings_screen(),
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
        if !g.menu && (g.on_fire > 0.0 || g.player.body.in_lava) {
            // On fire: an orange haze and flames licking up the edges.
            draw_rectangle(0.0, 0.0, w, h, Color::new(1.0, 0.45, 0.05, if g.player.body.in_lava { 0.55 } else { 0.18 }));
            let size = 48.0 * s;
            let flick = (g.clock * 9.0).sin() * 4.0 * s;
            for i in 0..((w / size) as i32 + 1) {
                let x = i as f32 * size;
                let lift = if i % 2 == 0 { flick } else { -flick };
                self.ui.tile(texture::T_FLAME, x, h - size * 1.4 + lift, size * 1.2, Color::new(1.0, 1.0, 1.0, 0.8));
            }
        }
        if !g.ready {
            draw_rectangle(0.0, 0.0, w, h, Color::new(0.1, 0.07, 0.05, 1.0));
            self.ui.text_centered("Generating terrain (artisanally)...", w / 2.0, h / 2.0, 12.0, WHITE);
            self.ui.text_centered(&format!("{} chunks ready", self.renderer.chunks.len()), w / 2.0, h / 2.0 + 18.0 * s, 9.0, GRAY);
            return;
        }
        self.sign_text();
        if self.screen == Screen::Playing {
            self.ui.crosshair();
            // The weapon charging back up after a swing (see combat.rs).
            let charge = self.game.attack_charge();
            if charge < 1.0 && !self.game.creative {
                let (bw, bh) = (18.0 * s, 2.0 * s);
                let (bx, by) = (w / 2.0 - bw / 2.0, h / 2.0 + 10.0 * s);
                draw_rectangle(bx - 1.0, by - 1.0, bw + 2.0, bh + 2.0, Color::new(0.0, 0.0, 0.0, 0.6));
                draw_rectangle(bx, by, bw * charge, bh, Color::new(0.9, 0.9, 0.9, 0.9));
            }
            if self.game.blocking {
                self.ui.tile(texture::T_SHIELD, w / 2.0 + 10.0 * s, h / 2.0 - 8.0 * s, 16.0 * s, Color::new(1.0, 1.0, 1.0, 0.8));
            }
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
            self.ui.stack_worn(g.inv.slots[i], g.inv.wear[i], x, y0, slot, !g.creative);
        }
        let sel = x0 + g.inv.selected as f32 * slot;
        draw_rectangle_lines(sel - s, y0 - s, slot + 2.0 * s, slot + 2.0 * s, 2.0 * s, WHITE);
        if !g.creative {
            // Experience bar just above the hotbar, with the level in the middle.
            let (level, progress) = g.level();
            let (bw, by) = (slot * 9.0, y0 - 6.0 * s);
            draw_rectangle(x0, by, bw, 4.0 * s, Color::new(0.0, 0.0, 0.0, 0.6));
            draw_rectangle(x0, by + s, bw * progress, 2.0 * s, Color::new(0.5, 0.95, 0.2, 1.0));
            if level > 0 {
                let t = level.to_string();
                self.ui.text_centered(&t, x0 + bw / 2.0 + s, by + 2.0 * s, 10.0, BLACK);
                self.ui.text_centered(&t, x0 + bw / 2.0, by + s, 10.0, Color::new(0.55, 1.0, 0.3, 1.0));
            }
            self.ui.hearts(g.player.health, x0, y0 - 18.0 * s);
            let points = g.inv.armor_points();
            if points > 0 {
                self.ui.armor_bar(points, x0, y0 - 29.0 * s);
            }
            self.ui.hunger_bar(g.player.hunger.food, x0 + slot * 9.0, y0 - 18.0 * s);
        }
        if g.held_name > 0.0 {
            let held = g.inv.held();
            if held != AIR {
                let a = g.held_name.min(1.0);
                self.ui.text_centered(item_name(held), w / 2.0, y0 - if g.creative { 6.0 } else { 16.0 } * s, 10.0, Color::new(1.0, 1.0, 1.0, a));
            }
        }

        // Chat-ish messages. While typing, the whole log (scrollable) instead.
        let typing = self.chat.is_some();
        let lines: Vec<(&str, f32)> = if typing {
            let n = g.chat_log.len();
            let end = n.saturating_sub(self.chat_scroll);
            g.chat_log.iter().take(end).rev().take(14).map(|m| (m.as_str(), 1.0)).collect()
        } else {
            g.messages.iter().rev().map(|(m, t)| (m.as_str(), t.min(1.0))).collect()
        };
        for (i, (m, a)) in lines.into_iter().enumerate() {
            let y = h - 40.0 * s - i as f32 * 11.0 * s;
            let tw = self.ui.text_width(m, 9.0);
            draw_rectangle(4.0 * s, y - 9.0 * s, tw + 6.0 * s, 11.0 * s, Color::new(0.0, 0.0, 0.0, 0.4 * a));
            self.ui.text(m, 7.0 * s, y, 9.0, Color::new(1.0, 1.0, 1.0, a));
        }
        if typing && self.chat_scroll > 0 {
            self.ui.text(&format!("(scrolled back {} lines: Page Down to return)", self.chat_scroll), 7.0 * s, h - 40.0 * s - 14.5 * 11.0 * s, 8.0, GRAY);
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
                {
                    // The closest thing the generator built, within a few chunks.
                    let (pcx, pcz) = ((p.x / 16.0).floor() as i32, (p.z / 16.0).floor() as i32);
                    let near = (-6..=6)
                        .flat_map(|dz| (-6..=6).map(move |dx| (pcx + dx, pcz + dz)))
                        .filter_map(|(cx, cz)| g.world.generator.site(cx, cz))
                        .min_by_key(|s| (s.origin.x as f32 - p.x).hypot(s.origin.z as f32 - p.z) as i32);
                    match near {
                        Some(s) => format!("Nearest structure: {} at {}, {}, {}", s.kind.name(), s.origin.x, s.origin.y, s.origin.z),
                        None => "Nearest structure: none nearby".to_string(),
                    }
                },
                format!("Chunks: {} meshed, {} loaded; {} regions of edits in memory", self.renderer.chunks.len(), g.world.chunks.len(), g.world.regions_loaded()),
                {
                    // Palette-packed block storage vs. two bytes per block.
                    let (bytes, bits) = g.world.chunks.values().fold((0, 0), |(b, t), c| (b + c.blocks.bytes(), t + c.blocks.total_bits()));
                    let blocks = (g.world.chunks.len() * (world::CW * world::CW * world::CH) as usize).max(1);
                    format!("Block memory: {:.1} MB ({:.1} bits/block, flat would be {:.1} MB)", bytes as f64 / 1e6, bits as f64 / blocks as f64, blocks as f64 * 2.0 / 1e6)
                },
                {
                    // Light where you stand, and how much of it is stored cell by cell.
                    let e = g.player.eye().floor().as_ivec3();
                    let detailed: usize = g.world.chunks.values().map(|c| c.light.detailed()).sum();
                    format!(
                        "Light: sky {} block {}  ({} of {} sections stored in full)",
                        g.world.sky_level(e.x, e.y, e.z),
                        g.world.block_level(e.x, e.y, e.z),
                        detailed,
                        g.world.chunks.len() * (world::CH / 16) as usize
                    )
                },
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
        self.fishing_hud();
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
        let half = (bw - 5.0 * s) / 2.0;
        if self.ui.button(Rect::new(x, y, half, bh), "Save World", !client) {
            self.save();
        }
        if self.ui.button(Rect::new(x + half + 5.0 * s, y, half, bh), "World Settings", true) {
            self.set_screen(Screen::WorldSettings);
        }
        y += bh + 5.0 * s;
        let half = (bw - 5.0 * s) / 2.0;
        let adv = format!("Advancements ({}/{})", self.game.advancements.count(), advancements::ALL.len());
        if self.ui.button(Rect::new(x, y, half, bh), &adv, true) {
            self.set_screen(Screen::Advancements);
        }
        if self.ui.button(Rect::new(x + half + 5.0 * s, y, half, bh), "Fishing Log", true) {
            self.set_screen(Screen::FishLog);
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

    fn fish_log_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.6));
        let log = &self.game.fish_log;
        self.ui.text_centered("Fishing Log", w / 2.0, h * 0.1, 16.0, WHITE);
        let sub = format!("Angler level {}  ({} xp)  -  lines snapped: {}  (they were THIS big)", log.level(), log.xp, log.snapped);
        self.ui.text_centered(&sub, w / 2.0, h * 0.1 + 16.0 * s, 9.0, GOLD);
        let pw = (320.0 * s).min(w - 20.0 * s);
        let x = w / 2.0 - pw / 2.0;
        let mut y = h * 0.1 + 30.0 * s;
        if log.species.is_empty() {
            self.ui.text_centered("Nothing yet. Craft a Fishing Stick (3 sticks, 2 string) and find some water.", w / 2.0, y + 12.0 * s, 9.0, GRAY);
        }
        let row_h = 20.0 * s;
        for (key, (count, best)) in &log.species {
            if y > h * 0.8 {
                break;
            }
            let id = block::reg().lookup(key).unwrap_or(AIR);
            draw_rectangle(x, y, pw, row_h - 2.0 * s, Color::new(0.12, 0.14, 0.2, 0.9));
            self.ui.icon(id, x + 3.0 * s, y + 1.0 * s, row_h - 4.0 * s);
            self.ui.text(&self.ui.fit(item_name(id), 9.0, pw * 0.55), x + row_h + 4.0 * s, y + 13.0 * s, 9.0, WHITE);
            let stats = if *best > 0.0 { format!("x{count}   best {best:.0}cm") } else { format!("x{count}") };
            let sw = self.ui.text_width(&stats, 9.0);
            self.ui.text(&stats, x + pw - sw - 6.0 * s, y + 13.0 * s, 9.0, GOLD);
            y += row_h;
        }
        let tips = "Tips: dawn and dusk are best. Deep, wide water beats puddles. Worms help. Don't reel in on a nibble.";
        self.ui.text_centered(tips, w / 2.0, h * 0.84, 8.0, Color::new(0.7, 0.85, 1.0, 1.0));
        let bw = (160.0 * s).min(w * 0.8);
        if self.ui.button(Rect::new(w / 2.0 - bw / 2.0, h * 0.88, bw, 20.0 * s), "Done", true) {
            self.set_screen(Screen::Paused);
        }
    }

    /// While fishing: the bite alert and, with a big one on the line, the tug-of-war bars.
    fn fishing_hud(&self) {
        let Some(b) = &self.game.bobber else { return };
        let (w, h, s) = (screen_width(), screen_height(), self.ui.s);
        if matches!(b.state, fishing::BobberState::Biting { .. }) && b.fight.is_none() {
            let pulse = 1.0 + (get_time() * 18.0).sin().abs() as f32 * 0.3;
            self.ui.text_centered("!", w / 2.0, h / 2.0 - 16.0 * s, 26.0 * pulse, Color::new(1.0, 0.9, 0.2, 1.0));
        }
        if let Some(f) = &b.fight {
            let bw = (180.0 * s).min(w * 0.6);
            let x = w / 2.0 - bw / 2.0;
            let y = h * 0.62;
            let bar = |y: f32, v: f32, col: Color, label: &str| {
                draw_rectangle(x - 2.0 * s, y - 2.0 * s, bw + 4.0 * s, 10.0 * s + 4.0 * s, Color::new(0.0, 0.0, 0.0, 0.6));
                draw_rectangle(x, y, bw * v.clamp(0.0, 1.0), 10.0 * s, col);
                self.ui.text(label, x, y - 4.0 * s, 8.0, WHITE);
            };
            let t = f.tension;
            let tcol = if t > 0.75 { Color::new(0.95, 0.2, 0.15, 1.0) } else if t > 0.5 { Color::new(0.95, 0.7, 0.2, 1.0) } else { Color::new(0.4, 0.85, 0.4, 1.0) };
            bar(y, f.progress, Color::new(0.3, 0.6, 1.0, 1.0), "Reel it in (hold right-click)");
            bar(y + 26.0 * s, t, tcol, if t > 0.75 { "LINE TENSION - EASE OFF!" } else { "Line tension" });
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
        st.render_distance = (st.render_distance + row(&self.ui, format!("Render Distance: {} chunks", st.render_distance), y)).clamp(3, settings::MAX_RENDER_DISTANCE);
        y += bh + 5.0 * s;
        st.fov = (st.fov + 5.0 * row(&self.ui, format!("FOV: {:.0}", st.fov), y) as f32).clamp(50.0, 110.0);
        y += bh + 5.0 * s;
        st.sensitivity = (st.sensitivity + 0.1 * row(&self.ui, format!("Mouse Sensitivity: {:.0}%", st.sensitivity * 100.0), y) as f32).clamp(0.1, 3.0);
        y += bh + 5.0 * s;
        let vol = &mut self.audio.volume;
        *vol = (*vol + 0.1 * row(&self.ui, format!("Sound Volume: {:.0}%", *vol * 100.0), y) as f32).clamp(0.0, 1.0);
        *vol = (*vol * 10.0).round() / 10.0;
        y += bh + 5.0 * s;
        // Two to a row from here on.
        let half = (bw - 4.0 * s) / 2.0;
        let (left, right) = (x, x + half + 4.0 * s);
        let music = if self.audio.music_on { "Music: ON" } else { "Music: OFF" };
        if self.ui.button(Rect::new(left, y, half, bh), music, true) {
            self.audio.music_on = !self.audio.music_on;
        }
        let fs = if self.settings.fullscreen { "Fullscreen: ON" } else { "Fullscreen: OFF" };
        if self.ui.button(Rect::new(right, y, half, bh), fs, true) {
            self.settings.fullscreen = !self.settings.fullscreen;
            set_fullscreen(self.settings.fullscreen);
        }
        y += bh + 5.0 * s;
        if self.ui.button(Rect::new(left, y, half, bh), "Controls...", true) {
            self.set_screen(Screen::Controls { from_title });
        }
        let skin = format!("Skin: {}", nametags::skin_name(self.settings.skin));
        if self.ui.button(Rect::new(right, y, half, bh), &skin, true) {
            self.settings.skin = (self.settings.skin + 1) % nametags::SKINS.len() as u8;
            let k = self.settings.skin;
            self.game.set_skin(k);
        }
        y += bh + 5.0 * s;
        let subs = if self.settings.subtitles { "Subtitles: ON" } else { "Subtitles: OFF" };
        if self.ui.button(Rect::new(left, y, half, bh), subs, true) {
            self.settings.subtitles = !self.settings.subtitles;
        }
        let cb = if self.settings.colour_blind { "Colour-blind: ON" } else { "Colour-blind: OFF" };
        if self.ui.button(Rect::new(right, y, half, bh), cb, true) {
            self.settings.colour_blind = !self.settings.colour_blind;
        }
        y += bh + 12.0 * s;
        if self.ui.button(Rect::new(x, y, bw, bh), "Done", true) {
            self.set_screen(if from_title { Screen::Title } else { Screen::Paused });
        }
    }

    /// Every action with its two bindings; click one, then press the new key.
    fn controls_screen(&mut self, from_title: bool) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        self.ui.text_centered("Controls", w / 2.0, h * 0.09, 16.0, WHITE);
        // Take the next key or button for the slot being changed (Esc empties it).
        if let Some((action, secondary)) = self.rebinding {
            if !self.rebind_armed {
                self.rebind_armed = true;
            } else if is_key_pressed(KeyCode::Escape) {
                self.settings.binds.set(action, secondary, None);
                self.rebinding = None;
            } else if let Some(b) = keybinds::Bind::just_pressed() {
                self.settings.binds.set(action, secondary, Some(b));
                self.rebinding = None;
            }
        }
        let col_w = ((w - 24.0 * s) / 2.0).min(260.0 * s);
        let bh = 15.0 * s;
        let per_col = keybinds::ACTIONS.len().div_ceil(2);
        let top = h * 0.14;
        for (i, &action) in keybinds::ACTIONS.iter().enumerate() {
            let (col, row) = (i / per_col, i % per_col);
            let x = w / 2.0 + if col == 0 { -col_w - 4.0 * s } else { 4.0 * s };
            let y = top + row as f32 * (bh + 3.0 * s);
            self.ui.text(action.label(), x, y + bh * 0.7, 9.0, WHITE);
            let binds = self.settings.binds.get(action);
            let bw = col_w * 0.28;
            for (slot, bind) in binds.iter().enumerate() {
                let r = Rect::new(x + col_w - (2 - slot) as f32 * (bw + 3.0 * s), y, bw, bh);
                let waiting = self.rebinding == Some((action, slot == 1));
                let label = if waiting { "> press <".to_string() } else { bind.map(|b| b.display()).unwrap_or_else(|| "-".into()) };
                if self.ui.button(r, &label, self.rebinding.is_none() || waiting) {
                    self.rebinding = Some((action, slot == 1));
                    self.rebind_armed = false;
                }
            }
        }
        let y = top + per_col as f32 * (bh + 3.0 * s) + 4.0 * s;
        let hint = if self.rebinding.is_some() {
            "Press a key or mouse button (Esc: leave it empty).".to_string()
        } else {
            match &self.pad.name {
                Some(name) => format!("Controller: {name} (sticks walk and look; triggers mine and place; A jumps; Y inventory)"),
                None => "No controller found. Plug one in any time.".to_string(),
            }
        };
        self.ui.text_centered(&hint, w / 2.0, y + 8.0 * s, 8.0, Color::new(0.8, 0.8, 0.8, 1.0));
        let bw = (110.0 * s).min(w * 0.4);
        let by = y + 16.0 * s;
        if self.ui.button(Rect::new(w / 2.0 - bw - 4.0 * s, by, bw, 18.0 * s), "Reset to Defaults", self.rebinding.is_none()) {
            self.settings.binds = Default::default();
        }
        if self.ui.button(Rect::new(w / 2.0 + 4.0 * s, by, bw, 18.0 * s), "Done", self.rebinding.is_none()) {
            self.set_screen(Screen::Options { from_title });
        }
    }

    fn help_screen(&mut self, from_title: bool) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        self.ui.text_centered("How to Play Minceraft", w / 2.0, h * 0.1, 16.0, WHITE);
        use keybinds::Action as A;
        let k = |a: A| self.settings.binds.describe(a);
        let walk = format!("{}/{}/{}/{}", k(A::Forward), k(A::Left), k(A::Back), k(A::Right));
        let keys = [
            format!("{walk} ... walk    {} ... jump / swim up", k(A::Jump)),
            format!("{} ... sprint    {} ... sneak (won't fall off ledges)", k(A::Sprint), k(A::Sneak)),
            format!("{} ... mine / attack    {} ... place / eat / light TNT with a torch", k(A::Attack), k(A::Use)),
            format!("1-9 / wheel ... pick hotbar    {} ... pick block (creative)", k(A::PickBlock)),
            format!("{} ... inventory + crafting    {} ... throw held item (Ctrl: stack)", k(A::Inventory), k(A::Drop)),
            format!("{} ... third person    F3 ... debug info    F11 ... fullscreen    Esc ... pause", k(A::Perspective)),
        ];
        let lines = [
            keys[0].as_str(),
            keys[1].as_str(),
            keys[2].as_str(),
            keys[3].as_str(),
            keys[4].as_str(),
            keys[5].as_str(),
            "Creative: double-tap Space to fly, Shift to descend.",
            "",
            "Survival tips: punch a Tree Chunk, craft Planks, then Sticks, then a Wooden Pickaxe.",
            "Stone needs a pickaxe. Iron needs stone tier. Dimonds need iron tier. Tools wear out. Eat to heal.",
            "Hissers explode. Groaners and Rattlers burn in daylight. Bloops split. Webbers climb. Farm animals are friends (and food).",
            "Never look a Starer in the eye. Beds skip the night. Bows need Pointy Sticks. Pokey Plants poke.",
            "Farming: hoe the dirt, plant, keep it watered and lit, feed the soil, rotate crops. A Soil Probe explains.",
            "Fishing: cast, wait for the real bite (not the nibbles!), reel in. Big fish: mind the line tension.",
            "Crafting works anywhere (the table is decorative. Satire!). Chests hold things; Furnaces cook with coal or wood.",
            "Multiplayer: host opens their world with Esc > Open to LAN; friends use Multiplayer. T to chat.",
            "Change any key in Options > Controls. Game controllers work too (sticks walk and look).",
        ];
        for (i, l) in lines.iter().enumerate() {
            self.ui.text_centered(l, w / 2.0, h * 0.18 + i as f32 * 11.5 * s, 9.0, if l.is_empty() { WHITE } else { Color::new(0.9, 0.9, 0.9, 1.0) });
        }
        let bw = (160.0 * s).min(w * 0.8);
        if self.ui.button(Rect::new(w / 2.0 - bw / 2.0, h * 0.18 + 18.5 * 11.5 * s, bw, 20.0 * s), "Got it", true) {
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
        if let Some(p) = self.game.death_spot() {
            let line = if self.game.rules.keep_inventory || self.game.creative {
                "Your inventory is safe. This world is kind.".to_string()
            } else {
                format!("Your things are on the ground at {}, {}, {} for five minutes.", p.x, p.y, p.z)
            };
            self.ui.text_centered(&line, w / 2.0, h * 0.3 + 34.0 * s, 9.0, Color::new(1.0, 0.95, 0.7, 1.0));
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

    fn container_screen(&mut self) {
        use containers::{FUEL, INPUT, OUTPUT};
        let Some(pos) = self.game.open else { return };
        let kind = containers::store_kind(&self.game.world, &self.game.vehicles, pos);
        let Some(c) = containers::store_ref(&self.game.world, &self.game.vehicles, pos).cloned() else { return };
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.5));
        let slot = 20.0 * s;
        let top_h = if containers::is_three_slot(kind) { slot * 3.2 } else { slot * 3.0 };
        let panel_w = slot * 9.0 + 12.0 * s;
        let panel_h = 18.0 * s + top_h + 18.0 * s + slot * 3.0 + 6.0 * s + slot + 8.0 * s;
        let x0 = (w - panel_w) / 2.0;
        let y0 = ((h - panel_h) / 2.0).max(4.0 * s);
        draw_rectangle(x0, y0, panel_w, panel_h, ui::PANEL);
        draw_rectangle_lines(x0, y0, panel_w, panel_h, s, WHITE);
        let sx = x0 + 6.0 * s;
        let shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
        let mut tooltip: Option<String> = None;
        self.ui.text(block(kind).name, sx, y0 + 12.0 * s, 10.0, WHITE);
        let top = y0 + 18.0 * s;
        // (slot index, x, y) for the container's own slots.
        let spots: Vec<(usize, f32, f32)> = if containers::is_three_slot(kind) {
            let cx = sx + slot * 2.5;
            vec![(INPUT, cx, top), (FUEL, cx, top + slot * 2.2), (OUTPUT, sx + slot * 5.5, top + slot * 1.1)]
        } else {
            (0..c.slots.len()).map(|i| (i, sx + (i % 9) as f32 * slot, top + (i / 9) as f32 * slot)).collect()
        };
        for &(i, x, y) in &spots {
            let (l, r, hov) = self.ui.slot_worn(c.slots[i], c.wear[i], x, y, slot, false);
            if hov {
                tooltip = label(c.slots[i], c.wear[i]);
            }
            if l || r {
                self.game.container_click(i, r, shift && l);
            }
        }
        if kind == BREWING_STAND {
            let progress = (c.cook / potions::BREW_SECS).clamp(0.0, 1.0);
            let ax = sx + slot * 3.9;
            let fy = top + slot * 1.1;
            self.ui.tile(texture::T_ARROW_UI, ax, fy, slot * 1.2, Color::new(0.3, 0.3, 0.3, 1.0));
            self.ui.tile_part(texture::T_ARROW_UI, ax, fy, slot * 1.2, progress, false, WHITE);
            let hint = match (c.slots[INPUT], c.slots[FUEL]) {
                (None, _) => "Ingredient on top: Glowshroom, Zappy Dust, Ember Shroom, Carrot, Feather (or Hisspowder)".to_string(),
                (_, None) => "A Water Bottle (or a potion) below".to_string(),
                (Some((i, _)), Some((b, _))) if potions::brew(b, i).is_none() => format!("{} does nothing to {}.", item_name(i), item_name(b)),
                _ if c.slots[OUTPUT].is_some() => "Take the potion out first.".to_string(),
                _ => "Bubbling...".to_string(),
            };
            self.ui.text(&hint, sx + slot * 0.2, top + slot * 3.1, 7.0, GRAY);
        }
        if containers::is_furnace(kind) {
            let (burn, cook) = c.gauges();
            let (fx, fy) = (sx + slot * 2.5, top + slot * 1.1);
            self.ui.tile(texture::T_FLAME, fx, fy, slot, Color::new(0.3, 0.3, 0.3, 1.0));
            self.ui.tile_part(texture::T_FLAME, fx, fy, slot, burn, true, WHITE);
            let ax = sx + slot * 3.9;
            self.ui.tile(texture::T_ARROW_UI, ax, fy, slot * 1.2, Color::new(0.3, 0.3, 0.3, 1.0));
            self.ui.tile_part(texture::T_ARROW_UI, ax, fy, slot * 1.2, cook, false, WHITE);
            let hint = match c.slots[INPUT] {
                Some((id, _)) if containers::smelt(id).is_none() => format!("{} won't cook. It's been asked.", item_name(id)),
                Some(_) if burn <= 0.0 && c.slots[FUEL].is_none() => "Needs fuel: coal, wood, sticks...".to_string(),
                None => "Raw food, sand or cobble goes on top".to_string(),
                _ => String::new(),
            };
            self.ui.text(&hint, sx + slot * 5.0, top + slot * 2.9, 7.0, GRAY);
        }
        let inv_y = top + top_h + 14.0 * s;
        self.ui.text("Inventory (shift-click to move stacks)", sx, inv_y - 4.0 * s, 8.0, GRAY);
        for i in 9..36 {
            let j = i - 9;
            let (cx, cy) = (sx + (j % 9) as f32 * slot, inv_y + (j / 9) as f32 * slot);
            let (l, r, hov) = self.ui.slot_worn(self.game.inv.slots[i], self.game.inv.wear[i], cx, cy, slot, false);
            if hov {
                tooltip = label(self.game.inv.slots[i], self.game.inv.wear[i]);
            }
            self.inventory_slot_click(i, l, r, shift);
        }
        let hot_y = inv_y + slot * 3.0 + 6.0 * s;
        for i in 0..9 {
            let (l, r, hov) = self.ui.slot_worn(self.game.inv.slots[i], self.game.inv.wear[i], sx + i as f32 * slot, hot_y, slot, i == self.game.inv.selected);
            if hov {
                tooltip = label(self.game.inv.slots[i], self.game.inv.wear[i]);
            }
            self.inventory_slot_click(i, l, r, shift);
        }
        if let Some(cur) = self.game.inv.cursor {
            let (mx, my) = mouse_position();
            self.ui.stack_worn(Some(cur), self.game.inv.cursor_wear, mx - slot / 2.0, my - slot / 2.0, slot, true);
        } else if let Some(t) = tooltip {
            self.ui.tooltip(&t);
        }
    }

    fn world_settings_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.6));
        self.ui.text_centered("World Settings", w / 2.0, h * 0.2, 16.0, WHITE);
        let owner = !self.game.is_client();
        let bw = (240.0 * s).min(w * 0.85);
        let bh = 20.0 * s;
        let x = w / 2.0 - bw / 2.0;
        let mut y = h * 0.3;
        let mut rules = self.game.rules;
        let keep = if rules.keep_inventory { "Keep Inventory: ON (dying costs nothing)" } else { "Keep Inventory: OFF (you drop everything)" };
        if self.ui.button(Rect::new(x, y, bw, bh), keep, owner) {
            rules.keep_inventory = !rules.keep_inventory;
        }
        y += bh + 5.0 * s;
        if self.ui.button(Rect::new(x, y, bw, bh), &format!("Difficulty: {}", rules.difficulty.name()), owner) {
            rules.difficulty = rules::Difficulty::from_index((rules.difficulty.index() + 1) % 4);
        }
        y += bh + 2.0 * s;
        self.ui.text_centered(rules.difficulty.blurb(), w / 2.0, y + 9.0 * s, 8.0, GRAY);
        y += 16.0 * s;
        let day = if rules.daylight_cycle { "Daylight Cycle: ON" } else { "Daylight Cycle: OFF (the sun is taking a break)" };
        if self.ui.button(Rect::new(x, y, bw, bh), day, owner) {
            rules.daylight_cycle = !rules.daylight_cycle;
        }
        y += bh + 5.0 * s;
        let half = (bw - 5.0 * s) / 2.0;
        let cycle = if rules.weather_cycle { "Weather Cycle: ON" } else { "Weather Cycle: OFF" };
        if self.ui.button(Rect::new(x, y, half, bh), cycle, owner) {
            rules.weather_cycle = !rules.weather_cycle;
        }
        use weather::Weather;
        let now = match self.game.weather.kind {
            Weather::Clear => "Weather: Clear",
            Weather::Rain => "Weather: Rain",
            Weather::Thunder => "Weather: Thunderstorm",
        };
        if self.ui.button(Rect::new(x + half + 5.0 * s, y, half, bh), now, owner) {
            let next = Weather::from_index((self.game.weather.kind.index() + 1) % 3);
            self.game.set_weather(next);
        }
        if rules != self.game.rules {
            self.game.set_rules(rules);
        }
        y += bh + 8.0 * s;
        if !owner {
            self.ui.text_centered("The host decides these. You just live here.", w / 2.0, y + 6.0 * s, 9.0, GRAY);
            y += 14.0 * s;
        }
        if self.ui.button(Rect::new(x, y, bw, bh), "Done", true) {
            self.set_screen(Screen::Paused);
        }
    }

    fn anvil_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.5));
        let slot = 20.0 * s;
        let panel_w = slot * 9.0 + 12.0 * s;
        let top_h = slot * 2.2;
        let panel_h = 18.0 * s + top_h + 18.0 * s + slot * 3.0 + 6.0 * s + slot + 8.0 * s;
        let x0 = (w - panel_w) / 2.0;
        let y0 = ((h - panel_h) / 2.0).max(4.0 * s);
        draw_rectangle(x0, y0, panel_w, panel_h, ui::PANEL);
        draw_rectangle_lines(x0, y0, panel_w, panel_h, s, WHITE);
        let sx = x0 + 6.0 * s;
        let shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
        let mut tooltip: Option<String> = None;
        let Some(pos) = self.game.anvil.as_ref().map(|a| a.pos) else { return };
        self.ui.text(block(self.game.world.get_v(pos)).name, sx, y0 + 12.0 * s, 10.0, WHITE);
        let top = y0 + 20.0 * s;
        let (slots, wear) = self.game.anvil.as_ref().map(|a| (a.slots, a.wear)).unwrap_or_default();
        for (i, x) in [(0, sx + slot * 0.5), (1, sx + slot * 2.5)] {
            let (l, r, hov) = self.ui.slot_worn(slots[i], wear[i], x, top, slot, false);
            if hov {
                tooltip = label(slots[i], wear[i]).or(Some(["The worn thing", "Its material, or another one like it"][i].to_string()));
            }
            if l || r {
                self.game.anvil_click(i, r);
            }
        }
        self.ui.text("+", sx + slot * 1.75, top + slot * 0.7, 12.0, WHITE);
        let plan = self.game.anvil_plan();
        let ax = sx + slot * 4.0;
        self.ui.tile(texture::T_ARROW_UI, ax, top, slot * 1.2, if plan.is_some() { WHITE } else { Color::new(0.3, 0.3, 0.3, 1.0) });
        let out_x = sx + slot * 6.0;
        let result = plan.map(|(item, _)| Some((item, 1))).unwrap_or(None);
        let (l, _, hov) = self.ui.slot_worn(result, plan.map(|(_, r)| r.wear).unwrap_or(0), out_x, top, slot, false);
        if hov {
            tooltip = plan.and_then(|(item, r)| label(Some((item, 1)), r.wear));
        }
        if l {
            self.game.anvil_take();
        }
        if let Some((_, r)) = plan {
            let level = self.game.level().0;
            let ok = self.game.creative || level >= r.cost;
            let what = if r.book { "Enchant" } else if r.combine { "Merge" } else { "Repair" };
            let text = format!("{what} cost: {} level{}{}", r.cost, if r.cost == 1 { "" } else { "s" }, if ok { "" } else { "  (Too Expensive!)" });
            self.ui.text(&text, sx, top + slot * 1.75, 8.0, if ok { Color::new(0.5, 1.0, 0.4, 1.0) } else { Color::new(1.0, 0.4, 0.4, 1.0) });
        } else {
            self.ui.text("Worn tool on the left, what it's made of (a twin, or an enchanted book) on the right.", sx, top + slot * 1.75, 7.0, GRAY);
        }
        let inv_y = top + top_h + 12.0 * s;
        self.ui.text(&format!("Inventory (level {})", self.game.level().0), sx, inv_y - 4.0 * s, 8.0, GRAY);
        for i in 9..36 {
            let j = i - 9;
            let (cx, cy) = (sx + (j % 9) as f32 * slot, inv_y + (j / 9) as f32 * slot);
            let (l, r, hov) = self.ui.slot_worn(self.game.inv.slots[i], self.game.inv.wear[i], cx, cy, slot, false);
            if hov {
                tooltip = label(self.game.inv.slots[i], self.game.inv.wear[i]);
            }
            self.inventory_slot_click(i, l, r, shift);
        }
        let hot_y = inv_y + slot * 3.0 + 6.0 * s;
        for i in 0..9 {
            let (l, r, hov) = self.ui.slot_worn(self.game.inv.slots[i], self.game.inv.wear[i], sx + i as f32 * slot, hot_y, slot, i == self.game.inv.selected);
            if hov {
                tooltip = label(self.game.inv.slots[i], self.game.inv.wear[i]);
            }
            self.inventory_slot_click(i, l, r, shift);
        }
        if let Some(cur) = self.game.inv.cursor {
            let (mx, my) = mouse_position();
            self.ui.stack_worn(Some(cur), self.game.inv.cursor_wear, mx - slot / 2.0, my - slot / 2.0, slot, true);
        } else if let Some(t) = tooltip {
            self.ui.tooltip(&t);
        }
    }

    fn enchant_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.5));
        let slot = 20.0 * s;
        let panel_w = slot * 9.0 + 12.0 * s;
        let top_h = slot * 3.4;
        let panel_h = 18.0 * s + top_h + 18.0 * s + slot * 3.0 + 6.0 * s + slot + 8.0 * s;
        let x0 = (w - panel_w) / 2.0;
        let y0 = ((h - panel_h) / 2.0).max(4.0 * s);
        draw_rectangle(x0, y0, panel_w, panel_h, ui::PANEL);
        draw_rectangle_lines(x0, y0, panel_w, panel_h, s, WHITE);
        let sx = x0 + 6.0 * s;
        let shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
        let mut tooltip: Option<String> = None;
        let Some((pos, item, wear, gold)) = self.game.enchanting.as_ref().map(|e| (e.pos, e.item, e.wear, e.gold)) else { return };
        let shelves = self.game.bookshelves(pos);
        self.ui.text(&format!("Enchanting Table ({shelves} bookshel{})", if shelves == 1 { "f" } else { "ves" }), sx, y0 + 12.0 * s, 10.0, WHITE);
        let top = y0 + 20.0 * s;
        for (i, x, stack, wr, hint) in [(0, sx, item, wear, "Something to enchant"), (1, sx + slot * 1.2, gold, 0, "Gold ingots (1 to 3)")] {
            let (l, r, hov) = self.ui.slot_worn(stack, wr, x, top + slot * 0.6, slot, false);
            if hov {
                tooltip = label(stack, wr).or(Some(hint.to_string()));
            }
            if l || r {
                self.game.enchant_click(i, r);
            }
        }
        // The three offers, cheapest first; hovering hints at what's in store.
        let offers = self.game.enchant_offers();
        let bx = sx + slot * 2.7;
        let bw = panel_w - (bx - x0) - 6.0 * s;
        let bh = slot * 0.95;
        let level = self.game.level().0;
        for choice in 0..3 {
            let y = top + choice as f32 * (bh + 2.0 * s);
            let r = Rect::new(bx, y, bw, bh);
            let Some((need, bits)) = offers.map(|o| o[choice]) else {
                draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.15, 0.12, 0.1, 1.0));
                continue;
            };
            let blocked = self.game.enchant_blocked(choice);
            let hover = r.contains(mouse_position().into());
            let bg = if blocked.is_some() { Color::new(0.2, 0.16, 0.14, 1.0) } else if hover { Color::new(0.45, 0.3, 0.6, 1.0) } else { Color::new(0.32, 0.22, 0.42, 1.0) };
            draw_rectangle(r.x, r.y, r.w, r.h, bg);
            draw_rectangle_lines(r.x, r.y, r.w, r.h, s, Color::new(0.6, 0.5, 0.7, 1.0));
            // Only the first enchantment is revealed (Minecraft keeps some mystery too).
            let first = enchant::Enchant::ALL.iter().find(|e| enchant::level(bits, **e) > 0).map(|e| enchant::describe(enchant::with_level(0, *e, enchant::level(bits, *e)))).unwrap_or_default();
            let color = if blocked.is_some() { GRAY } else { Color::new(0.85, 1.0, 0.6, 1.0) };
            self.ui.text(&format!("{first} . . . ?"), r.x + 4.0 * s, r.y + bh * 0.45, 8.0, color);
            let cost = choice + 1;
            let req = format!("Level {need}+   costs {cost} level{} and {cost} gold", if cost == 1 { "" } else { "s" });
            self.ui.text(&req, r.x + 4.0 * s, r.y + bh * 0.85, 7.0, if level >= need || self.game.creative { GRAY } else { Color::new(1.0, 0.4, 0.4, 1.0) });
            if hover {
                let what = enchant::Enchant::ALL.iter().find(|e| enchant::level(bits, **e) > 0).map(|e| e.name()).unwrap_or("Something");
                tooltip = Some(blocked.clone().unwrap_or_else(|| format!("{what}, and maybe more")));
                if is_mouse_button_pressed(MouseButton::Left) && self.game.inv.cursor.is_none() {
                    self.game.enchant_pick(choice);
                }
            }
        }
        if offers.is_none() {
            let hint = if item.is_some() { "That can't be enchanted (or already is)." } else { "Put in a tool, weapon or armour, and some gold." };
            self.ui.text(hint, bx + 4.0 * s, top + bh * 1.6, 8.0, GRAY);
        }
        let inv_y = top + top_h + 12.0 * s;
        self.ui.text(&format!("Inventory (level {level})"), sx, inv_y - 4.0 * s, 8.0, GRAY);
        for i in 9..36 {
            let j = i - 9;
            let (cx, cy) = (sx + (j % 9) as f32 * slot, inv_y + (j / 9) as f32 * slot);
            let (l, r, hov) = self.ui.slot_worn(self.game.inv.slots[i], self.game.inv.wear[i], cx, cy, slot, false);
            if hov {
                tooltip = label(self.game.inv.slots[i], self.game.inv.wear[i]);
            }
            self.inventory_slot_click(i, l, r, shift);
        }
        let hot_y = inv_y + slot * 3.0 + 6.0 * s;
        for i in 0..9 {
            let (l, r, hov) = self.ui.slot_worn(self.game.inv.slots[i], self.game.inv.wear[i], sx + i as f32 * slot, hot_y, slot, i == self.game.inv.selected);
            if hov {
                tooltip = label(self.game.inv.slots[i], self.game.inv.wear[i]);
            }
            self.inventory_slot_click(i, l, r, shift);
        }
        if let Some(cur) = self.game.inv.cursor {
            let (mx, my) = mouse_position();
            self.ui.stack_worn(Some(cur), self.game.inv.cursor_wear, mx - slot / 2.0, my - slot / 2.0, slot, true);
        } else if let Some(t) = tooltip {
            self.ui.tooltip(&t);
        }
    }

    fn trade_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.5));
        let slot = 20.0 * s;
        let Some((job, list)) = self.game.trade_list() else { return };
        let row = slot * 1.1;
        let panel_w = slot * 9.0 + 12.0 * s;
        let panel_h = 24.0 * s + row * list.len() as f32 + 18.0 * s + slot * 3.0 + 6.0 * s + slot + 8.0 * s;
        let x0 = (w - panel_w) / 2.0;
        let y0 = ((h - panel_h) / 2.0).max(4.0 * s);
        draw_rectangle(x0, y0, panel_w, panel_h, ui::PANEL);
        draw_rectangle_lines(x0, y0, panel_w, panel_h, s, WHITE);
        let sx = x0 + 6.0 * s;
        let mut tooltip: Option<String> = None;
        self.ui.text(&format!("Hmmer: {}", job.name()), sx, y0 + 12.0 * s, 10.0, WHITE);
        let top = y0 + 20.0 * s;
        let (mx, my) = mouse_position();
        for (i, t) in list.iter().enumerate() {
            let y = top + i as f32 * row;
            let r = Rect::new(sx, y, panel_w - 12.0 * s, slot);
            let afford = self.game.creative || villagers::can_afford(|id| self.game.inv.count(id), t);
            let hover = r.contains(vec2(mx, my));
            draw_rectangle(r.x, r.y, r.w, r.h, if hover && afford { Color::new(0.35, 0.3, 0.2, 1.0) } else { Color::new(0.18, 0.16, 0.14, 1.0) });
            let mut x = sx;
            for (id, n) in t.give {
                if id != AIR {
                    self.ui.stack_worn(Some((id, n)), 0, x, y, slot, false);
                    if hover && mx < x + slot && mx >= x {
                        tooltip = label(Some((id, n)), 0);
                    }
                }
                x += slot;
            }
            self.ui.tile(texture::T_ARROW_UI, sx + slot * 2.3, y, slot, if afford { WHITE } else { Color::new(0.4, 0.4, 0.4, 1.0) });
            let gx = sx + slot * 3.6;
            self.ui.stack_worn(Some(t.get), t.wear, gx, y, slot, false);
            self.ui.text(&label(Some(t.get), t.wear).unwrap_or_default(), gx + slot * 1.2, y + slot * 0.62, 7.0, if afford { WHITE } else { GRAY });
            if hover && mx >= gx {
                tooltip = label(Some(t.get), t.wear);
            }
            if hover && is_mouse_button_pressed(MouseButton::Left) && self.game.inv.cursor.is_none() {
                self.game.make_trade(i);
            }
        }
        let inv_y = top + row * list.len() as f32 + 12.0 * s;
        self.ui.text("Inventory", sx, inv_y - 4.0 * s, 8.0, GRAY);
        for i in 9..36 {
            let j = i - 9;
            let (cx, cy) = (sx + (j % 9) as f32 * slot, inv_y + (j / 9) as f32 * slot);
            let (l, r, hov) = self.ui.slot_worn(self.game.inv.slots[i], self.game.inv.wear[i], cx, cy, slot, false);
            if hov {
                tooltip = label(self.game.inv.slots[i], self.game.inv.wear[i]);
            }
            self.inventory_slot_click(i, l, r, false);
        }
        let hot_y = inv_y + slot * 3.0 + 6.0 * s;
        for i in 0..9 {
            let (l, r, hov) = self.ui.slot_worn(self.game.inv.slots[i], self.game.inv.wear[i], sx + i as f32 * slot, hot_y, slot, i == self.game.inv.selected);
            if hov {
                tooltip = label(self.game.inv.slots[i], self.game.inv.wear[i]);
            }
            self.inventory_slot_click(i, l, r, false);
        }
        if let Some(cur) = self.game.inv.cursor {
            self.ui.stack_worn(Some(cur), self.game.inv.cursor_wear, mx - slot / 2.0, my - slot / 2.0, slot, true);
        } else if let Some(t) = tooltip {
            self.ui.tooltip(&t);
        }
    }

    fn inventory_slot_click(&mut self, i: usize, l: bool, r: bool, shift: bool) {
        if l && shift && self.game.anvil.is_some() {
            self.game.anvil_quick_put(i);
        } else if l && shift && self.game.enchanting.is_some() {
            self.game.enchant_quick_put(i);
        } else if l && shift {
            self.game.container_quick_put(i);
        } else if l {
            self.game.inv.click(i);
        } else if r {
            self.game.inv.right_click(i);
        }
    }

    fn inventory_screen(&mut self) {
        let (w, h) = (screen_width(), screen_height());
        let s = self.ui.s;
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.5));
        let slot = 20.0 * s;
        let creative = self.game.creative;
        let left_w = slot * 9.0 + 12.0 * s;
        let armor_w = slot + 12.0 * s;
        let right_w = (170.0 * s).min(w - left_w - armor_w - 36.0 * s);
        let total_w = armor_w + 6.0 * s + left_w + 8.0 * s + right_w;
        let panel_h = (slot * 9.5).min(h - 20.0 * s).max(slot * 7.0);
        let x0 = (w - total_w) / 2.0 + armor_w + 6.0 * s;
        let y0 = (h - panel_h) / 2.0;
        draw_rectangle(x0, y0, left_w, panel_h, ui::PANEL);
        draw_rectangle_lines(x0, y0, left_w, panel_h, s, WHITE);
        let mut tooltip: Option<String> = None;
        let shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);

        // Armour: four slots, each taking only its own kind.
        let ax = x0 - armor_w - 6.0 * s;
        let armor_h = 18.0 * s + slot * 4.0 + 24.0 * s;
        draw_rectangle(ax, y0, armor_w, armor_h, ui::PANEL);
        draw_rectangle_lines(ax, y0, armor_w, armor_h, s, WHITE);
        self.ui.text("Armour", ax + 4.0 * s, y0 + 12.0 * s, 8.0, WHITE);
        for i in 0..4 {
            let (sx, sy) = (ax + 6.0 * s, y0 + 18.0 * s + i as f32 * slot);
            let worn = self.game.inv.armor[i];
            let (l, r, hov) = self.ui.slot_worn(worn, self.game.inv.armor_wear[i], sx, sy, slot, false);
            if worn.is_none() {
                // A faint outline of what goes here.
                let pad = slot * 0.12;
                self.ui.tile(texture::T_ARMOR_ITEMS + 4 + i as u16, sx + pad, sy + pad, slot - 2.0 * pad, Color::new(0.0, 0.0, 0.0, 0.35));
            }
            if hov {
                tooltip = Some(match worn {
                    Some((id, _)) => format!("{} (+{} armour)", label(worn, self.game.inv.armor_wear[i]).unwrap_or_default(), armor_points(id)),
                    None => ["Helmet", "Chestplate", "Leggings", "Boots"][i].to_string(),
                });
            }
            if l || r {
                self.game.inv.click_armor(i);
            }
        }
        let points = self.game.inv.armor_points();
        let cut = (points as f32 * 4.0).min(80.0);
        self.ui.text(&format!("{points} pts"), ax + 4.0 * s, y0 + 18.0 * s + slot * 4.0 + 9.0 * s, 7.0, GRAY);
        self.ui.text(&format!("-{cut:.0}% dmg"), ax + 4.0 * s, y0 + 18.0 * s + slot * 4.0 + 18.0 * s, 7.0, GRAY);

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
                let (l, r, hov) = self.ui.slot_worn(self.game.inv.slots[i], self.game.inv.wear[i], cx, cy, slot, false);
                if hov {
                    tooltip = label(self.game.inv.slots[i], self.game.inv.wear[i]);
                }
                if l && !(shift && self.game.inv.equip(i)) {
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
            let (l, r, hov) = self.ui.slot_worn(self.game.inv.slots[i], self.game.inv.wear[i], cx, hot_y, slot, i == self.game.inv.selected);
            if hov {
                tooltip = label(self.game.inv.slots[i], self.game.inv.wear[i]);
            }
            if l {
                if creative && self.game.inv.cursor.is_none() {
                    self.game.inv.slots[i] = None;
                } else if !(shift && self.game.inv.equip(i)) {
                    self.game.inv.click(i);
                }
            }
            if r {
                self.game.inv.right_click(i);
            }
        }
        if self.game.inv.cursor.is_some() && self.ui.clicked {
            let panels = [Rect::new(x0, y0, left_w, panel_h), Rect::new(ax, y0, armor_w, armor_h), Rect::new(x0 + left_w + 8.0 * s, y0, right_w, panel_h)];
            if !panels.iter().any(|r| self.ui.hovered(*r)) {
                // Clicked outside: creative forgets it, survival throws it on the floor.
                if creative {
                    self.game.inv.cursor = None;
                } else if let Some((item, n)) = self.game.inv.cursor.take() {
                    let wear = std::mem::take(&mut self.game.inv.cursor_wear);
                    self.game.throw_stack(item, n, wear);
                }
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
                        let mut made = 0u8;
                        for _ in 0..times {
                            if !self.game.inv.craft(r) {
                                break;
                            }
                            made += 1;
                        }
                        // Joined players: the host checks the ingredients against its ledger.
                        if self.game.is_client() && !self.game.creative && made > 0 {
                            self.game.net_send_msg(net::Msg::Craft { recipe: ri as u16, times: made });
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
            self.ui.stack_worn(Some(c), self.game.inv.cursor_wear, mx - slot / 2.0, my - slot / 2.0, slot, true);
        } else if let Some(t) = tooltip {
            self.ui.tooltip(&t);
        }
    }
}

/// Where to stand to show off a structure, a ravine or snow near spawn: (eye, yaw, pitch).
fn scenic_view(g: &Game, mode: &str) -> Option<(Vec3, f32, f32)> {
    use structures::Kind;
    let generator = &g.world.generator;
    let (cx0, cz0) = ((g.spawn.x / 16.0).floor() as i32, (g.spawn.z / 16.0).floor() as i32);
    let ring = |r: i32| (-r..=r).flat_map(move |dz| (-r..=r).map(move |dx| (dx, dz))).filter(move |(dx, dz)| dx.abs().max(dz.abs()) == r);
    let look = |from: Vec3, to: Vec3| {
        let d = to - from;
        (from, d.x.atan2(-d.z), d.y.atan2(Vec2::new(d.x, d.z).length()))
    };
    let kind = match mode {
        "hut" => Kind::Hut,
        "tower" => Kind::Tower,
        "well" => Kind::Well,
        "dungeon" => Kind::Dungeon,
        "village" => Kind::Village,
        "swamp" | "jungle" | "badlands" | "taiga" => {
            let want = match mode {
                "swamp" => world::Biome::Swamp,
                "jungle" => world::Biome::Jungle,
                "badlands" => world::Biome::Badlands,
                _ => world::Biome::Taiga,
            };
            // Somewhere well inside the biome (all nine columns around agree), looking across it.
            for r in 0..160 {
                for (dx, dz) in ring(r) {
                    let (x, z) = ((cx0 + dx) * 16 + 8, (cz0 + dz) * 16 + 8);
                    let inside = (-1..=1).all(|i| (-1..=1).all(|j| generator.column(x + i * 24, z + j * 24).1 == want));
                    if inside {
                        let h = generator.column(x, z).0.max(world::SEA);
                        return Some((Vec3::new(x as f32 + 0.5, h as f32 + 12.0, z as f32 + 0.5), 0.8, -0.35));
                    }
                }
            }
            return None;
        }
        "ravine" | "snow" | "rain" | "thunder" => {
            for r in 0..60 {
                for (dx, dz) in ring(r) {
                    let (x, z) = ((cx0 + dx) * 16 + 8, (cz0 + dz) * 16 + 8);
                    let (h, biome) = generator.column(x, z);
                    let open = generator.site(cx0 + dx, cz0 + dz).is_none() && h > world::SEA + 2;
                    if mode == "snow" {
                        if biome == world::Biome::Snowy && open {
                            return Some((Vec3::new(x as f32 + 0.5, h as f32 + 8.0, z as f32 + 0.5), 2.4, -0.35));
                        }
                    } else if mode != "ravine" {
                        let clear = (-3..=3).all(|i| (-3..=3).all(|j| generator.tree_at(x + i, z + j).is_none()));
                        if biome == world::Biome::Plains && open && clear {
                            return Some((Vec3::new(x as f32 + 0.5, h as f32 + 3.0, z as f32 + 0.5), 2.4, -0.1));
                        }
                    } else if generator.ravine_floor(x, z).is_some() && h > world::SEA + 4 {
                        return Some((Vec3::new(x as f32 + 0.5, h as f32 + 14.0, z as f32 + 0.5), 2.4, -1.1));
                    }
                }
            }
            return None;
        }
        _ => return None,
    };
    for r in 0..60 {
        for (dx, dz) in ring(r) {
            let Some(site) = generator.site(cx0 + dx, cz0 + dz).filter(|s| s.kind == kind) else { continue };
            let o = site.origin.as_vec3() + Vec3::new(0.5, 0.0, 0.5);
            return Some(match kind {
                Kind::Dungeon => look(o + Vec3::new(2.6, 2.4, 2.6), o + Vec3::new(-3.0, 0.5, -1.0)),
                Kind::Tower => look(o + Vec3::new(-11.0, 9.0, -11.0), o + Vec3::Y * 4.0),
                Kind::Village => look(o + Vec3::new(-20.0, 18.0, -20.0), o + Vec3::Y * 2.0),
                _ => look(o + Vec3::new(-8.0, 6.0, -8.0), o + Vec3::Y * 1.5),
            });
        }
    }
    None
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
    /// Render distance in chunks (the default setting when absent).
    distance: Option<i32>,
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
        distance: get("--distance").and_then(|f| f.parse().ok()),
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

/// Tooltip text for a slot: its name, enchantments, and uses left for tools.
fn label(stack: Option<(Id, u8)>, wear: inventory::Wear) -> Option<String> {
    let (id, _) = stack?;
    let mut s = item_name(id).to_string();
    if enchant::is_enchanted(wear) {
        s += &format!(" [{}]", enchant::describe(wear));
    }
    if let Some(max) = inventory::max_uses(id, wear) {
        s += &format!(" ({}/{max} uses left)", max.saturating_sub(inventory::uses(wear) as u32));
    }
    Some(s)
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

    let mut shot = parse_args();
    let mut app = App {
        screen: Screen::Title,
        game: Game::new(random_seed(), true, true),
        renderer,
        ui: Ui::new(tex),
        settings: Settings::default(),
        no_settings_file: shot.is_some(),
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
        chat_sent: Vec::new(),
        name_line: String::new(),
        captions: Default::default(),
        chat_pick: None,
        chat_scroll: 0,
        last_view_proj: Mat4::IDENTITY,
        lan_addr: None,
        mp_password: String::new(),
        upnp_job: None,
        upnp_mapping: None,
        internet_status: None,
        map_colors: navigation::block_colors(&base_atlas),
        base_atlas,
        atlas_gen: block::generation(),
        map_tex: None,
        map_timer: 0.0,
        sign_lines: Default::default(),
        sign_line: 0,
        pad: pad::Pad::new(),
        pad_frame: Default::default(),
        rebinding: None,
        rebind_armed: false,
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
        form_keep: false,
        form_focus: 0,
    };
    // Screenshots always use the defaults, whatever the player last picked.
    if shot.is_none() {
        let saved = Settings::load(&settings::path());
        app.audio.volume = saved.volume;
        app.audio.music_on = saved.music_on;
        if !saved.mp_name.is_empty() {
            app.mp_name = saved.mp_name.clone();
        }
        app.mp_addr = saved.mp_addr.clone();
        if saved.fullscreen {
            set_fullscreen(true);
        }
        app.settings = saved;
    }
    if let Some(d) = shot.as_ref().and_then(|s| s.distance) {
        app.settings.render_distance = d.clamp(3, settings::MAX_RENDER_DISTANCE);
    }
    let flag = |f: &str| std::env::args().any(|a| a == f);
    if shot.is_some() {
        app.settings.colour_blind = flag("--colour-blind");
        app.settings.subtitles = flag("--subtitles");
    }
    let broken: Vec<&block::ModInfo> = mod_infos.iter().filter(|m| m.enabled && !m.errors.is_empty()).collect();
    if let Some(m) = broken.first() {
        app.status = Some((format!("Mod \"{}\" has {} problem(s): see the Mods screen.", m.name, m.errors.len()), 8.0));
    }

    if let Some(s) = &mut shot {
        match s.mode.as_str() {
            "survival" | "creative" | "inventory" | "night" | "death" => {
                let mut g = Game::new(424242, s.mode == "creative", false);
                if s.mode == "inventory" {
                    for (item, n) in [(LOG, 12), (COBBLE, 20), (COAL, 5), (IRON, 3), (DIAMOND, 2), (GUNPOWDER, 5), (SAND, 9), (PORKCHOP, 3), (block::stairs(1, 0), 8), (block::slab(0, false), 12), (block::DOOR, 2)] {
                        g.inv.add(item, n);
                    }
                    g.inv.add(block::ARMOR_FIRST + 8, 1);
                    g.inv.armor[1] = Some((block::ARMOR_FIRST + 4 + 1, 1));
                    g.inv.armor[3] = Some((block::ARMOR_FIRST + 3, 1));
                    g.inv.armor_wear[1] = 150;
                    // Some well-used tools.
                    for (item, wear) in [(block::PICK_IRON, 60), (block::SWORD_STONE, 110), (block::BOW, 20)] {
                        g.inv.add(item, 1);
                        let i = g.inv.slots.iter().position(|s| *s == Some((item, 1))).unwrap();
                        g.inv.wear[i] = wear;
                    }
                    g.player.hunger.food = 13.0;
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
            "farm" | "fish" | "kitchen" | "chest" | "furnace" | "building" | "armour" | "anvil" | "rules" | "xp" | "enchant" | "table" | "liquids" | "zappy" | "trade" | "vehicles" | "decor" | "carpentry" | "brewing" | "contraptions" | "machines" => {
                let mut g = Game::new(424242, s.mode == "farm", false);
                g.time = s.time.unwrap_or(0.2);
                if s.mode == "fish" {
                    g.inv.slots[0] = Some((block::ROD, 1));
                }
                app.start_game(g);
                app.show_debug = false;
            }
            "hut" | "tower" | "well" | "dungeon" | "village" | "ravine" | "rain" | "thunder" | "snow" | "swamp" | "jungle" | "badlands" | "taiga" => {
                // Somewhere the generator built something (or the sky is doing something).
                let mut g = Game::new(424242, true, false);
                g.time = s.time.unwrap_or(0.3);
                let kind = match s.mode.as_str() {
                    "rain" | "snow" => Some(weather::Weather::Rain),
                    "thunder" => Some(weather::Weather::Thunder),
                    _ => None,
                };
                if let Some(k) = kind {
                    g.weather.kind = k;
                    g.weather.strength = 1.0;
                    g.rules.weather_cycle = false;
                }
                if let Some((pos, yaw, pitch)) = scenic_view(&g, &s.mode) {
                    (s.pos, s.yaw, s.pitch) = (Some(pos), yaw, pitch);
                }
                app.start_game(g);
                app.show_debug = false;
            }
            "scorch" | "portal" => {
                // Through a portal (built on the spot), or looking at one.
                let mut g = Game::new(424242, s.mode == "scorch", false);
                g.time = s.time.unwrap_or(0.3);
                let spawn = g.spawn;
                let here = IVec3::new(spawn.x.floor() as i32, spawn.y.floor() as i32, spawn.z.floor() as i32);
                if s.mode == "scorch" {
                    let to = g.travel(here);
                    g.player.body.pos = to;
                    if s.pos.is_none() {
                        s.pos = Some(to + Vec3::new(0.0, 0.0, 1.0));
                        s.yaw = std::f32::consts::PI;
                        s.pitch = -0.15;
                    }
                } else {
                    let (cx, cz) = (here.x.div_euclid(16), here.z.div_euclid(16));
                    for dz in -1..=1 {
                        for dx in -1..=1 {
                            g.world.load_now(cx + dx, cz + dz);
                        }
                    }
                    let base = here + IVec3::new(0, 0, -6);
                    let base = IVec3::new(base.x, g.world.surface_y(base.x, base.z) + 1, base.z);
                    scorch::build_portal(&mut g.world, base);
                    if s.pos.is_none() {
                        s.pos = Some(base.as_vec3() + Vec3::new(1.0, 1.2, 6.0));
                        s.yaw = 0.0;
                        s.pitch = -0.05;
                    }
                }
                app.start_game(g);
                app.show_debug = false;
            }
            "hollow" => {
                // On the island's edge, looking in at the pillars (and whatever circles them).
                let mut g = Game::new(424242, true, false);
                let to = g.hollow_destination(IVec3::ZERO);
                g.player.body.pos = to;
                let o = hollow::ORIGIN.as_vec3();
                g.world.load_now(hollow::ORIGIN.x.div_euclid(16), 0);
                g.alloc_mob(entity::MobKind::Wyrm, o + Vec3::new(20.0, 24.0, -10.0));
                if s.pos.is_none() {
                    s.pos = Some(to + Vec3::Y * 2.0);
                    s.yaw = -std::f32::consts::FRAC_PI_2;
                    s.pitch = 0.12;
                }
                app.start_game(g);
                app.show_debug = false;
            }
            "zoo" | "animals" => {
                // Every mob in two rows, in daylight unless --time says otherwise, in creative (so nobody attacks).
                let mut g = Game::new(424242, true, false);
                g.time = s.time.unwrap_or(0.2);
                app.start_game(g);
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
            "controls" => {
                app.game = Game::new(424242, true, true);
                // Show a changed binding and one waiting for a key.
                app.settings.binds.set(keybinds::Action::Sprint, true, keybinds::Bind::parse("F"));
                app.set_screen(Screen::Controls { from_title: true });
                app.rebinding = Some((keybinds::Action::Drop, true));
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
            if matches!(s.mode.as_str(), "zoo" | "animals" | "farm" | "fish" | "kitchen" | "chest" | "furnace" | "building" | "armour" | "anvil" | "rules" | "xp" | "enchant" | "table" | "liquids" | "zappy" | "trade" | "vehicles" | "decor" | "carpentry" | "brewing" | "contraptions" | "machines") && frames == 120 {
                // A flat, clear stone floor in front of the camera.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let y0 = p.y.floor() as i32 - 1;
                for x in p.x as i32 - 26..p.x as i32 + 26 {
                    for z in p.z as i32 - 26..p.z as i32 + 26 {
                        let d = Vec3::new(x as f32 + 0.5, p.y, z as f32 + 0.5) - p;
                        if !(1.0..22.0).contains(&d.dot(fwd)) || d.dot(right).abs() > 15.0 {
                            continue;
                        }
                        app.game.world.set(x, y0, z, block::STONE);
                        for y in y0 + 1..y0 + 14 {
                            app.game.world.set(x, y, z, block::AIR);
                        }
                    }
                }
            }
            if s.mode == "farm" && frames == 125 {
                // A little farm: three crops at every stage, watered down the middle,
                // with a Scarecrow, and the new decorative blocks along the back.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let y0 = p.y.floor() as i32 - 1;
                let at = |f: f32, r: f32| {
                    let v = p + fwd * f + right * r;
                    (v.x.floor() as i32, v.z.floor() as i32)
                };
                let crops = [block::WHEAT_0, block::CARROT_0, block::POTATO_0];
                for (row, base) in crops.iter().enumerate() {
                    for col in 0..8 {
                        let (x, z) = at(4.0 + row as f32, col as f32 - 3.5);
                        let water = col == 4;
                        app.game.world.set(x, y0, z, if water { block::WATER } else { block::FARMLAND_WET });
                        if !water {
                            let stage = (col.min(7) as u16 / 2).min(3);
                            app.game.world.set(x, y0 + 1, z, base + stage);
                        }
                    }
                }
                let (x, z) = at(3.0, 5.5);
                app.game.world.set(x, y0 + 1, z, block::SCARECROW);
                let showcase = [block::SANDSTONE, block::STONE_BRICKS, block::MOSSY_COBBLE, block::HAY, block::BOOKSHELF, block::LANTERN, block::MUSHROOM, block::WEEDS];
                for (i, id) in showcase.into_iter().enumerate() {
                    let (x, z) = at(9.0, i as f32 * 1.2 - 4.2);
                    if matches!(id, block::MUSHROOM | block::WEEDS) {
                        app.game.world.set(x, y0, z, block::GRASS);
                    }
                    app.game.world.set(x, y0 + 1, z, id);
                }
            }
            if s.mode == "fish" && frames == 125 {
                // A pond, and a big one on the line.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let y0 = p.y.floor() as i32 - 1;
                for x in p.x as i32 - 14..p.x as i32 + 14 {
                    for z in p.z as i32 - 14..p.z as i32 + 14 {
                        let d = Vec3::new(x as f32 + 0.5, p.y, z as f32 + 0.5) - p;
                        if (3.0..12.0).contains(&d.dot(fwd)) && d.dot(right).abs() < 4.5 {
                            for y in y0 - 2..=y0 {
                                app.game.world.set(x, y, z, block::WATER);
                            }
                        }
                    }
                }
            }
            if s.mode == "fish" && frames >= 160 {
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let spot = Vec3::new(p.x, p.y.floor() - 0.1, p.z) + fwd * 8.0;
                app.game.bobber = Some(fishing::Bobber {
                    pos: spot,
                    vel: Vec3::ZERO,
                    state: fishing::BobberState::Biting { window: 1.0 },
                    fight: Some(fishing::Fight::new(0.55, 0.8)),
                    since_nibble: 9.0,
                    bait: false,
                });
            }
            if s.mode == "zoo" && frames == 150 {
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let mut rng = noise::Rng::new(9);
                for (i, kind) in entity::MobKind::ALL.into_iter().enumerate() {
                    // The six newest in front, the originals behind (offset so they show between).
                    let (row, col) = (1 - (i / 6).min(1), i % 6);
                    let off = if row == 1 { 0.5 } else { 0.0 };
                    let at = p + fwd * (6.0 + row as f32 * 5.0) + right * ((col as f32 - 2.5 + off) * 2.6) + Vec3::Y * 2.0;
                    let mut m = entity::Mob::new(kind, at, &mut rng).with_size(if kind == entity::MobKind::Bloop { 2 } else { 1 });
                    m.yaw = s.yaw + std::f32::consts::PI;
                    m.id = 1000 + i as u32;
                    app.game.mobs.push(m);
                }
            }
            if matches!(s.mode.as_str(), "kitchen" | "chest" | "furnace") && frames == 125 {
                // A chest, a table and a row of furnaces (the middle one busy), a few blocks ahead.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let y = p.y.floor() as i32;
                let at = |f: f32, r: f32| {
                    let v = p + fwd * f + right * r;
                    IVec3::new(v.x.floor() as i32, y, v.z.floor() as i32)
                };
                let row = [block::CHEST, block::TABLE, block::FURNACE, block::FURNACE, block::FURNACE, block::CHEST];
                for (i, &id) in row.iter().enumerate() {
                    app.game.world.set_v(at(4.0, i as f32 - 2.5), id);
                }
                let busy = at(4.0, 3.0 - 2.5);
                if let Some(c) = app.game.world.containers.get_mut(&busy) {
                    c.slots[containers::INPUT] = Some((block::PORKCHOP, 5));
                    c.slots[containers::FUEL] = Some((block::COAL, 3));
                    c.slots[containers::OUTPUT] = Some((block::COOKED_CHOP, 2));
                }
                let chest = at(4.0, -2.5);
                if let Some(c) = app.game.world.containers.get_mut(&chest) {
                    let loot = [(block::DIAMOND, 7), (block::COBBLE, 64), (block::LOG, 23), (block::BREAD, 4), (block::BOOT, 1), (block::COAL, 18), (block::GOLD_INGOT, 3), (block::WHEAT_SEEDS, 12)];
                    for (i, (item, n)) in loot.into_iter().enumerate() {
                        c.slots[i * 3 + i / 3] = Some((item, n));
                    }
                }
                for (item, n) in [(block::PORKCHOP, 3), (block::SAND, 16), (block::PLANKS, 20), (block::MUTTON, 2)] {
                    app.game.give(item, n);
                }
                match s.mode.as_str() {
                    "chest" => app.game.open_container(chest),
                    "furnace" => app.game.open_container(busy),
                    _ => {}
                }
            }
            if matches!(s.mode.as_str(), "building" | "armour") && frames == 125 {
                // A little staircase, slabs, a pair of doors and some things on the floor.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let y = p.y.floor() as i32;
                let at = |f: f32, r: f32, up: i32| {
                    let v = p + fwd * f + right * r;
                    IVec3::new(v.x.floor() as i32, y + up, v.z.floor() as i32)
                };
                // Which way is "away" from the camera, as a facing.
                let away = if fwd.x.abs() > fwd.z.abs() { if fwd.x > 0.0 { 1 } else { 3 } } else if fwd.z > 0.0 { 2 } else { 0 };
                let side = (away + 1) % 4;
                for (i, m) in [0usize, 1, 2].into_iter().enumerate() {
                    let r = i as f32 * 1.0 - 5.0;
                    app.game.world.set_v(at(5.0, r, 0), block::stairs(m, away));
                    app.game.world.set_v(at(6.0, r, 0), block::MATERIALS[m].0);
                    app.game.world.set_v(at(6.0, r, 1), block::stairs(m, away));
                    app.game.world.set_v(at(7.0, r, 0), block::MATERIALS[m].0);
                    app.game.world.set_v(at(7.0, r, 1), block::MATERIALS[m].0);
                    app.game.world.set_v(at(4.0, r + 4.0, 0), block::slab(m, false));
                    app.game.world.set_v(at(5.0, r + 4.0, 1), block::slab(m, true));
                    app.game.world.set_v(at(5.0, r + 4.0, 0), block::STONE);
                }
                for (r, open) in [(3.0, false), (4.0, true)] {
                    app.game.world.set_v(at(6.0, r, 0), block::door(away, open, false));
                    app.game.world.set_v(at(6.0, r, 1), block::door(away, open, true));
                }
                for r in [2.0, 5.0] {
                    for up in 0..3 {
                        app.game.world.set_v(at(6.0, r, up), block::PLANKS);
                    }
                }
                app.game.world.set_v(at(6.0, 3.0, 2), block::PLANKS);
                app.game.world.set_v(at(6.0, 4.0, 2), block::PLANKS);
                let _ = side;
                let loot = [(block::DIAMOND, 3), (block::COBBLE, 12), (block::PORKCHOP, 1), (block::stairs(0, 0), 4), (block::ARMOR_FIRST + 4, 1), (block::DOOR, 1), (block::TORCH, 5)];
                for (i, (item, n)) in loot.into_iter().enumerate() {
                    let v = p + fwd * (2.5 + (i % 3) as f32 * 0.7) + right * ((i as f32 - 3.0) * 0.6);
                    app.game.spawn_drop(Vec3::new(v.x, y as f32, v.z), item, n, 0, Vec3::ZERO, 60.0);
                }
                if s.mode == "armour" {
                    for slot in 0..4 {
                        let tier = [3, 1, 2, 0][slot];
                        app.game.inv.armor[slot] = Some((block::ARMOR_FIRST + tier * 4 + slot as u16, 1));
                    }
                    app.game.third_person = true;
                    app.game.player.health = 15.0;
                }
            }
            if s.mode == "contraptions" && frames == 125 {
                // Lever -> dust -> repeater -> lamp; a torch inverter; pistons out; a dispenser.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let f = if fwd.x.abs() > fwd.z.abs() { IVec3::new(fwd.x.signum() as i32, 0, 0) } else { IVec3::new(0, 0, fwd.z.signum() as i32) };
                let r = IVec3::new(-f.z, 0, f.x);
                let base = IVec3::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
                let at = |fo: i32, ro: i32, up: i32| base + f * fo + r * ro + IVec3::Y * up;
                app.game.world.set_v(at(3, -3, 0), block::LEVER_ON);
                for k in -2..0 {
                    app.game.world.set_v(at(3, k, 0), block::WIRE);
                }
                app.game.world.set_v(at(3, 0, 0), contraptions::repeater(contraptions::facing_of(r), false));
                app.game.world.set_v(at(3, 1, 0), block::LAMP);
                app.game.world.set_v(at(5, -2, 0), block::STONE);
                app.game.world.set_v(at(5, -2, 1), block::ZTORCH_ON);
                app.game.world.set_v(at(5, -1, 1), block::LAMP);
                for k in 0..3 {
                    app.game.world.set_v(at(6, k, 0), contraptions::piston(contraptions::facing_of(f), false, k == 1));
                    app.game.world.set_v(at(7, k, 0), block::PLANKS);
                    app.game.world.set_v(at(6, k, 1), block::ZAP_BLOCK);
                }
                app.game.world.set_v(at(4, 3, 0), block::DISPENSER_FIRST + contraptions::facing_of(-f) as block::Id);
                // A chest feeding a furnace through a hopper.
                app.game.world.set_v(at(3, -5, 0), block::FURNACE);
                app.game.world.set_v(at(3, -5, 1), block::HOPPER_FIRST);
                app.game.world.set_v(at(3, -5, 2), block::CHEST);
                app.game.world.set_v(at(4, -5, 1), block::HOPPER_FIRST + 1 + contraptions::facing_of(-f) as block::Id);
            }
            if s.mode == "machines" && frames == 125 {
                // A beacon on its pyramid; a detector rail under a cart lighting a lamp;
                // chest and hopper carts; a comparator reading a chest.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let f = if fwd.x.abs() > fwd.z.abs() { IVec3::new(fwd.x.signum() as i32, 0, 0) } else { IVec3::new(0, 0, fwd.z.signum() as i32) };
                let r = IVec3::new(-f.z, 0, f.x);
                let base = IVec3::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
                let at = |fo: i32, ro: i32, up: i32| base + f * fo + r * ro + IVec3::Y * up;
                let top = at(12, 0, 2);
                for k in 1..=2 {
                    for dx in -k..=k {
                        for dz in -k..=k {
                            app.game.world.set_v(top + IVec3::new(dx, -k, dz), block::OBSIDIAN);
                        }
                    }
                }
                app.game.world.set_v(top, block::BEACON_FIRST + 1);
                // Track across the front, a detector in it, a lamp beside that.
                for k in -5..=2 {
                    app.game.world.set_v(at(5, k, 0), block::RAIL_FIRST);
                }
                app.game.world.set_v(at(5, -1, 0), block::DETECTOR_RAIL);
                app.game.world.set_v(at(6, -1, 0), block::LAMP);
                let spot = |fo: i32, ro: i32| at(fo, ro, 0).as_vec3() + Vec3::new(0.5, 0.06, 0.5);
                let yaw = (r.x as f32).atan2(-(r.z as f32));
                app.game.spawn_vehicle(vehicles::CART_KIND, spot(5, -1), yaw);
                let chest = app.game.spawn_vehicle(vehicles::CHEST_CART_KIND, spot(5, 1), yaw);
                app.game.spawn_vehicle(vehicles::HOPPER_CART_KIND, spot(5, -4), yaw);
                if let Some(c) = containers::store(&mut app.game.world, &mut app.game.vehicles, vehicles::cart_key(chest)) {
                    c.slots[0] = Some((block::DIAMOND, 3));
                }
                // Chest -> comparator -> lamp.
                app.game.world.set_v(at(3, -3, 0), block::CHEST);
                if let Some(c) = app.game.world.containers.get_mut(&at(3, -3, 0)) {
                    c.slots[0] = Some((block::COBBLE, 5));
                }
                app.game.world.set_v(at(3, -2, 0), contraptions::comparator(contraptions::facing_of(r), false, false));
                app.game.world.set_v(at(3, -1, 0), block::LAMP);
            }
            if s.mode == "brewing" && frames == 125 {
                // A brewing stand mid-brew, potions in hand, and a couple of effects on.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let at = (p + fwd * 2.5).floor().as_ivec3();
                app.game.world.set_v(at, block::BREWING_STAND);
                if let Some(c) = app.game.world.containers.get_mut(&at) {
                    c.slots[containers::INPUT] = Some((block::EMBER_SHROOM, 3));
                    c.slots[containers::FUEL] = Some((block::WATER_BOTTLE, 1));
                }
                app.game.world.set_v(at + IVec3::X, block::EMBER_SHROOM);
                for (i, p) in potions::ALL.iter().enumerate() {
                    app.game.inv.slots[i] = Some((potions::potion_item(*p, i % 2 == 1), 1));
                }
                app.game.inv.slots[5] = Some((block::GLASS_BOTTLE, 3));
                app.game.inv.slots[6] = Some((block::WATER_BOTTLE, 2));
                app.game.inv.slots[7] = Some((block::GRUMBLER_TUSK, 1));
                app.game.apply_potion(potions::Potion::Speed);
                app.game.apply_potion(potions::Potion::NightVision);
            }
            if s.mode == "carpentry" && frames == 125 {
                // A fenced pen with a gate, a wall with a ladder, panes and colours.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let f = if fwd.x.abs() > fwd.z.abs() { IVec3::new(fwd.x.signum() as i32, 0, 0) } else { IVec3::new(0, 0, fwd.z.signum() as i32) };
                let r = IVec3::new(-f.z, 0, f.x);
                let base = IVec3::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
                let at = |fo: i32, ro: i32, up: i32| base + f * fo + r * ro + IVec3::Y * up;
                for k in -3..=3 {
                    app.game.world.set_v(at(3, k, 0), if k == 0 { carpentry::gate(f.x != 0, false) } else { block::FENCE_FIRST });
                    app.game.world.set_v(at(7, k, 0), block::FENCE_FIRST);
                }
                for fo in 4..7 {
                    app.game.world.set_v(at(fo, -3, 0), block::FENCE_FIRST);
                    app.game.world.set_v(at(fo, 3, 0), block::FENCE_FIRST);
                }
                app.game.alloc_mob(entity::MobKind::Oinker, at(5, 0, 0).as_vec3() + Vec3::new(0.5, 0.0, 0.5));
                // A wall of coloured wool and stained glass panes, with a ladder.
                for k in 4..12 {
                    for up in 0..4 {
                        let c = (k + up) as u16 % 8;
                        let id = if up == 2 && k % 2 == 0 { block::PANE_FIRST } else if up == 3 { block::STAINED_GLASS + c } else if c == 0 { block::WOOL } else { block::DYED_WOOL + c - 1 };
                        app.game.world.set_v(at(9, k - 8, up), id);
                    }
                }
                for up in 0..4 {
                    app.game.world.set_v(at(8, -4, up), block::LADDER_FIRST + building_facing(-f));
                }
                app.game.world.set_v(at(2, 3, 0), carpentry::trapdoor(0, true));
                app.game.world.set_v(at(2, 4, 0), carpentry::trapdoor(0, false));
                // A campfire of sorts: logs, alight.
                app.game.world.set_v(at(2, -3, 0), block::LOG);
                app.game.world.set_v(at(2, -3, 1), block::FIRE);
                app.game.world.set_v(at(2, -2, 0), block::FIRE);
            }
            if s.mode == "decor" && frames == 125 {
                // A signpost, a framed sword on a wall, and a map in hand.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let f = if fwd.x.abs() > fwd.z.abs() { IVec3::new(fwd.x.signum() as i32, 0, 0) } else { IVec3::new(0, 0, fwd.z.signum() as i32) };
                let r = IVec3::new(-f.z, 0, f.x);
                let base = IVec3::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
                let facing = if f.z < 0 { 0u16 } else if f.x > 0 { 1 } else if f.z > 0 { 2 } else { 3 };
                let sign = base + f * 3 - r;
                app.game.world.set_v(sign, block::SIGN_FIRST + facing);
                app.game.world.signs.insert(sign, ["Welcome to".into(), "STOVEVILLE".into(), "pop. 1".into(), "(it's you)".into()]);
                for k in -1..=2 {
                    for up in 0..3 {
                        app.game.world.set_v(base + f * 6 + r * k + IVec3::Y * up, block::STONE_BRICKS);
                    }
                }
                // Frames hang on the wall's near face: their facing is the far side of their cell.
                let frame_facing = (facing + 0) as block::Id;
                for (k, item) in [(0, block::SWORD_DIAMOND), (1, block::DIAMOND)] {
                    let at = base + f * 5 + r * k + IVec3::Y;
                    app.game.world.set_v(at, block::FRAME_FIRST + frame_facing);
                    app.game.world.frames.insert(at, (item, 0));
                }
                app.game.inv.slots[app.game.inv.selected] = Some((block::MAP, 1));
                app.game.inv.add(block::COMPASS, 1);
            }
            if s.mode == "vehicles" && frames == 125 {
                // A loop of rails with a cart, and a boat on a pond.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let f = if fwd.x.abs() > fwd.z.abs() { IVec3::new(fwd.x.signum() as i32, 0, 0) } else { IVec3::new(0, 0, fwd.z.signum() as i32) };
                let r = IVec3::new(-f.z, 0, f.x);
                let base = IVec3::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
                let at = |fo: i32, ro: i32| base + f * fo + r * ro;
                for k in 0..7 {
                    for (a, b) in [(3 + k, -4), (3 + k, 1), (3, -4 + k.min(5)), (9, -4 + k.min(5))] {
                        app.game.world.set_v(at(a, b), block::RAIL_FIRST);
                    }
                }
                app.game.world.set_v(at(3, -1), block::POWERED_RAIL);
                app.game.world.set_v(at(3, -1) - r, block::LEVER_ON);
                for a in 4..9 {
                    for b in 3..7 {
                        app.game.world.set_v(at(a, b) - IVec3::Y, block::WATER);
                    }
                }
                let cart_at = at(6, -4).as_vec3() + Vec3::new(0.5, 1.0 / 16.0, 0.5);
                app.game.spawn_vehicle(vehicles::CART_KIND, cart_at, 0.0);
                let boat_at = at(6, 5).as_vec3() + Vec3::new(0.5, -0.15, 0.5);
                app.game.spawn_vehicle(vehicles::BOAT_KIND, boat_at, 0.7);
                app.game.inv.add(block::MINECART, 1);
                app.game.inv.add(block::BOAT, 1);
                app.game.inv.add(block::RAIL_FIRST, 32);
            }
            if s.mode == "trade" && frames == 125 {
                // A Librarian and a purse of gold.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let seed = (0..64u32).find(|&s| villagers::Job::of(s) == villagers::Job::Librarian).unwrap_or(1);
                app.game.world.new_huts.push((p + fwd * 3.0, seed));
                app.game.house_hmmers();
                let id = app.game.mobs.last().map(|m| m.id).unwrap_or(0);
                if let Some(m) = app.game.mobs.last_mut() {
                    m.yaw = s.yaw + std::f32::consts::PI;
                }
                app.game.inv.add(block::GOLD_INGOT, 23);
                app.game.inv.add(block::FEATHER, 30);
                app.game.inv.add(block::BOOK, 3);
                app.game.open_trade(id);
            }
            if s.mode == "zappy" && frames == 125 {
                // A lever wired to a row of lamps and a door; a button and a plate beside.
                // Laid out on the grid (dust only links to its neighbours).
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let f = if fwd.x.abs() > fwd.z.abs() { IVec3::new(fwd.x.signum() as i32, 0, 0) } else { IVec3::new(0, 0, fwd.z.signum() as i32) };
                let r = IVec3::new(-f.z, 0, f.x);
                let base = IVec3::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
                let at = |fo: f32, ro: f32, up: i32| base + f * fo.round() as i32 + r * ro.round() as i32 + IVec3::Y * up;
                let w = &mut app.game.world;
                w.set_v(at(4.0, -4.0, 0), block::LEVER_ON);
                for k in 0..9 {
                    w.set_v(at(4.0, -3.0 + k as f32, 0), block::WIRE);
                }
                for k in 0..4 {
                    w.set_v(at(5.0 + k as f32, 5.0, 0), block::WIRE);
                }
                for k in 0..3 {
                    w.set_v(at(5.0, -2.0 + k as f32 * 3.0, 0), block::LAMP);
                    w.set_v(at(5.0, -2.0 + k as f32 * 3.0, 1), block::LAMP);
                }
                let d = at(9.0, 5.0, 0);
                w.set_v(d, block::door(0, false, false));
                w.set_v(d + IVec3::Y, block::door(0, false, true));
                w.set_v(at(3.0, -1.5, 0), block::BUTTON);
                w.set_v(at(2.5, 1.0, 0), block::PLATE);
                w.set_v(at(8.0, -4.0, 0), block::ZAP_BLOCK);
                w.set_v(at(8.0, -3.0, 0), block::LAMP);
                app.game.inv.add(block::ZAP_DUST, 32);
                app.game.inv.add(block::LEVER, 2);
                app.game.inv.add(block::LAMP, 4);
            }
            if s.mode == "liquids" && frames == 125 {
                // A waterfall off a pillar, a lava pool, and where they met.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let at = |f: f32, r: f32, up: i32| {
                    let v = p + fwd * f + right * r;
                    IVec3::new(v.x.floor() as i32, p.y.floor() as i32 + up, v.z.floor() as i32)
                };
                for up in 0..4 {
                    app.game.world.set_v(at(9.0, -3.0, up), block::STONE_BRICKS);
                }
                app.game.world.set_v(at(9.0, -3.0, 4), block::WATER);
                for (f, r) in [(6.0, 3.0), (6.0, 4.0), (7.0, 3.0), (7.0, 4.0)] {
                    app.game.world.set_v(at(f, r, -1), block::LAVA);
                }
                app.game.world.set_v(at(8.0, 3.5, 0), block::OBSIDIAN);
                app.game.world.set_v(at(8.0, 3.5, 1), block::OBSIDIAN);
                app.game.inv.add(block::WATER_BUCKET, 1);
                app.game.inv.add(block::LAVA_BUCKET, 1);
                app.game.inv.add(block::BUCKET, 3);
            }
            if matches!(s.mode.as_str(), "enchant" | "table") && frames == 125 {
                // An enchanting table in a ring of bookshelves, and something to enchant.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let v = p + fwd * 4.5;
                let t = IVec3::new(v.x.floor() as i32, p.y.floor() as i32, v.z.floor() as i32);
                app.game.world.set_v(t, block::ENCHANTING_TABLE);
                for dx in -2..=2i32 {
                    for dz in -2..=2i32 {
                        // Leave a gap on the near side to walk in.
                        let near = IVec3::new(dx, 0, dz).as_vec3().dot(fwd) < -1.0;
                        if dx.abs().max(dz.abs()) == 2 && !near {
                            app.game.world.set_v(t + IVec3::new(dx, 0, dz), block::BOOKSHELF);
                            app.game.world.set_v(t + IVec3::new(dx, 1, dz), block::BOOKSHELF);
                        }
                    }
                }
                app.game.xp = xp::points_for_level(30) + 40;
                for (item, n, wear) in [(block::PICK_DIAMOND, 1, 12), (block::GOLD_INGOT, 9, 0), (block::SWORD_IRON, 1, enchant::with_level(enchant::with_level(40, enchant::Enchant::Sharpness, 3), enchant::Enchant::Unbreaking, 1)), (block::BREAD, 6, 0)] {
                    app.game.inv.add(item, n);
                    let i = app.game.inv.slots.iter().position(|s| *s == Some((item, n))).unwrap();
                    app.game.inv.wear[i] = wear;
                }
                if s.mode == "enchant" {
                    app.game.open_enchanting(t);
                    if let Some(ui) = &mut app.game.enchanting {
                        ui.item = Some((block::PICK_DIAMOND, 1));
                        ui.wear = 12;
                        ui.gold = Some((block::GOLD_INGOT, 3));
                    }
                    app.game.inv.remove(block::PICK_DIAMOND, 1);
                    app.game.inv.remove(block::GOLD_INGOT, 3);
                } else {
                    app.game.inv.selected = app.game.inv.slots.iter().position(|s| s.is_some_and(|s| s.0 == block::SWORD_IRON)).unwrap_or(0);
                }
            }
            if matches!(s.mode.as_str(), "anvil" | "rules" | "xp") && frames == 125 {
                // An anvil (and a chipped one) ahead, a worn pickaxe and some iron, and a few levels.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let at = |f: f32, r: f32| {
                    let v = p + fwd * f + right * r;
                    IVec3::new(v.x.floor() as i32, p.y.floor() as i32, v.z.floor() as i32)
                };
                let (a, b) = (at(3.5, -0.8), at(3.5, 1.2));
                app.game.world.set_v(a, block::ANVIL);
                app.game.world.set_v(b, block::ANVIL_DAMAGED);
                app.game.xp = xp::points_for_level(7) + 20;
                for (item, n, wear) in [(block::PICK_IRON, 1, 190), (block::IRON, 5, 0), (block::SWORD_DIAMOND, 1, 800), (block::BREAD, 6, 0)] {
                    app.game.inv.add(item, n);
                    let i = app.game.inv.slots.iter().position(|s| *s == Some((item, n))).unwrap();
                    app.game.inv.wear[i] = wear;
                }
                match s.mode.as_str() {
                    "anvil" => {
                        app.game.open_anvil(a);
                        if let Some(ui) = &mut app.game.anvil {
                            ui.slots = [Some((block::PICK_IRON, 1)), Some((block::IRON, 3))];
                            ui.wear = [190, 0];
                        }
                        app.game.inv.remove(block::PICK_IRON, 1);
                        app.game.inv.remove(block::IRON, 3);
                    }
                    "rules" => app.set_screen(Screen::WorldSettings),
                    _ => {
                        for i in 0..6 {
                            let v = p + fwd * (2.0 + i as f32 * 0.6) + right * ((i as f32 - 2.5) * 0.7) + Vec3::Y * 0.8;
                            app.game.spawn_orbs(v, [3, 7, 17, 1, 37, 7][i]);
                        }
                        for o in app.game.orbs.iter_mut() {
                            o.body.vel = Vec3::ZERO;
                        }
                    }
                }
            }
            if s.mode == "xp" && frames > 125 {
                // Hold the orbs still for the photo.
                for o in app.game.orbs.iter_mut() {
                    o.body.vel = Vec3::ZERO;
                    o.age = 0.0;
                }
            }
            if s.mode == "thunder" && frames + 2 == s.frames {
                // A bolt in the distance for the photo.
                let p = app.game.player.body.pos;
                let (x, z) = ((p.x + s.yaw.sin() * 14.0).floor() as i32, (p.z - s.yaw.cos() * 14.0).floor() as i32);
                let y = app.game.world.surface_y(x, z) + 1;
                app.game.lightning_effects(Vec3::new(x as f32 + 0.5, y as f32, z as f32 + 0.5));
            }
            if s.mode == "death" && frames == 150 {
                app.game.inv.add(block::DIAMOND, 3);
                app.game.player.hurt = 0.0;
                app.game.hurt_player(100.0, "was defeated by a screenshot");
            }
            if s.mode == "animals" && frames == 150 {
                // A family of Mooers, a shorn Fluffer and a lamb, a pet Woofer sitting nicely.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let mut rng = noise::Rng::new(4);
                let owner = players::record_key(&app.game.player_name);
                let herd: [(entity::MobKind, f32, f32, bool); 7] = [
                    (entity::MobKind::Mooer, 7.0, -3.0, false),
                    (entity::MobKind::Mooer, 7.5, -1.2, false),
                    (entity::MobKind::Mooer, 5.5, -2.0, true),
                    (entity::MobKind::Fluffer, 6.0, 1.5, false),
                    (entity::MobKind::Fluffer, 4.8, 2.6, true),
                    (entity::MobKind::Woofer, 3.2, 0.0, false),
                    (entity::MobKind::Oinker, 8.5, 3.2, false),
                ];
                for (i, (kind, f, r, baby)) in herd.into_iter().enumerate() {
                    let at = p + fwd * f + right * r + Vec3::Y * 1.0;
                    let mut m = entity::Mob::new(kind, at, &mut rng);
                    m.yaw = s.yaw + std::f32::consts::PI + (i as f32 - 3.0) * 0.3;
                    m.id = 2000 + i as u32;
                    if baby {
                        m.set_baby(100.0);
                    }
                    if kind == entity::MobKind::Fluffer && !baby {
                        m.sheared = true;
                    }
                    if kind == entity::MobKind::Woofer {
                        m.owner = Some(owner.clone());
                        m.sitting = true;
                    }
                    if i == 6 {
                        m.love = 20.0;
                    }
                    m.persistent = true;
                    app.game.mobs.push(m);
                }
                app.game.inv.add(block::SHEARS, 1);
                app.game.inv.add(block::WHEAT, 12);
                app.game.inv.add(block::BONE, 5);
            }
            if s.mode == "animals" && frames > 150 {
                for m in app.game.mobs.iter_mut() {
                    m.goal = None;
                    m.body.vel.x = 0.0;
                    m.body.vel.z = 0.0;
                }
            }
            if s.mode == "zoo" && frames > 150 {
                // Hold still for the photo.
                for m in app.game.mobs.iter_mut() {
                    m.yaw = s.yaw + std::f32::consts::PI;
                    m.body.vel.x = 0.0;
                    m.body.vel.z = 0.0;
                }
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
            app.save_settings();
            app.game.disconnect();
            break;
        }
        next_frame().await;
    }
}

/// The ladder/frame facing for a wall on side `d` of the cell.
fn building_facing(d: IVec3) -> block::Id {
    crate::decor::frame_facing(-d).unwrap_or(0) as block::Id
}
