//! MINCERAFT — a native, browser-free block game parody.
//! Rust + raw OpenGL (via miniquad/macroquad). No asset files: everything is
//! generated at startup (the Windows exe's icon, assets/minceraft.ico, is
//! generated too, by --export-icon).

mod admin;
mod access;
mod advancements;
mod animals;
mod anvil;
mod archaeology;
mod bees;
mod backups;
mod banners;
mod beacon;
mod block;
mod books;
mod boxes;
mod building;
mod carpentry;
mod cheats;
mod combat;
mod containers;
mod copper;
mod creaking;
mod crafting;
mod critters;
mod deepdark;
mod contraptions;
mod decor;
mod drops;
mod enchant;
mod entity;
mod falling;
mod farming;
mod fortress;
mod music;
mod fire;
mod fireworks;
mod floaty;
mod gadgets;
mod fishing;
mod game;
mod glider;
mod golems;
mod hollow;
mod home;
mod masonry;
mod treasure;
mod wildlife;
mod hoppers;
mod horses;
mod hunger;
mod inventory;
mod keybinds;
mod ledger;
mod light;
mod lod;
mod liquids;
mod mesher;
mod modes;
mod mods;
mod moon;
mod multiplayer;
mod nametags;
mod nature;
mod navigation;
mod net;
mod noise;
mod pad;
mod palette;
mod pathing;
mod paths;
mod tint;
mod trial;
mod updates;
mod player;
mod playtest;
mod potions;
mod players;
mod regions;
mod render;
mod rules;
mod raids;
mod save;
mod scorch;
mod seasons;
mod skies;
mod screens;
mod scripting;
mod server;
mod structures;
mod trees;
mod settings;
mod smithing;
mod sniffers;
mod stash;
mod upnp;
mod vehicles;
mod villagers;
mod songs;
mod sound;
mod stats;
mod texture;
mod tools;
mod trims;
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
    /// A book open to read or write in (see books.rs).
    Book,
    /// At a Loom, patterning a banner or painting a shield (see banners.rs).
    Loom,
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
    /// A grindstone or smithing table is open (see smithing.rs).
    Bench,
    /// The Field Journal (see archaeology.rs) and the Beekeeping Log (bees.rs).
    Journal,
    BeeLog,
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
    /// Graphics and frame rate (reached from Options).
    Video { from_title: bool },
    Help { from_title: bool },
    Advancements,
    /// What you've done in this world (see stats.rs).
    Stats,
    FishLog,
    Multiplayer,
    Mods,
    Worlds,
    CreateWorld,
    RenameWorld,
    DeleteWorld,
    /// The selected world's backups (see backups.rs).
    Backups,
}

struct App {
    screen: Screen,
    game: Game,
    renderer: Renderer,
    ui: Ui,
    settings: Settings,
    /// VSync and anti-aliasing as the window was opened with (they need a restart).
    video_at_start: (bool, u8),
    /// Screenshot runs neither read nor write settings.txt.
    no_settings_file: bool,
    splash: &'static str,
    last_mouse: Option<Vec2>,
    show_debug: bool,
    recipe_scroll: f32,
    /// Rows scrolled down the creative palette.
    palette_scroll: usize,
    /// The recipe book: search text (and whether it's being typed in), tab, "craftable only".
    book_search: String,
    book_focus: bool,
    book_tab: crafting::Tab,
    book_craftable: bool,
    /// Where the Field Journal goes back to (the pause menu, or the game).
    journal_back: Screen,
    quit: bool,
    /// The Screenshot key was pressed: save this frame once it's drawn.
    shot_pending: bool,
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
    /// The book on screen: its pages, which one is showing, a title being
    /// typed to sign it, and whether it's already been put away.
    book_pages: Vec<String>,
    book_page: usize,
    book_title: Option<String>,
    book_done: bool,
    /// The Loom's choice of pattern and dye colour.
    loom_pattern: u8,
    loom_colour: u8,
    /// A game controller, if one is plugged in, and what it did this frame.
    pad: pad::Pad,
    pad_frame: pad::PadFrame,
    /// The action (and which of its two slots) waiting for a new key.
    rebinding: Option<(keybinds::Action, bool)>,
    /// Skip the click that started rebinding, so it isn't taken as the new binding.
    rebind_armed: bool,
    mods_scroll: usize,
    adv_scroll: usize,
    /// Which advancements tab is showing (see advancements::TABS).
    adv_tab: usize,
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
    form_hardcore: bool,
    form_focus: usize,
    /// The Backups screen's list and choice.
    /// Whether a newer release is out (see updates.rs).
    /// Recent frame times in ms, newest last (the F3 graph).
    frame_times: std::collections::VecDeque<f32>,
    /// The graphics card and OpenGL version, as the driver names them.
    /// Smoothed CPU time per frame stage, ms: tick, meshing, scene, draw calls, UI (F3).
    stage_ms: [f32; 5],
    gpu: (String, String),
    updates: updates::UpdateCheck,
    backup_list: Vec<backups::Backup>,
    backup_sel: Option<usize>,
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
    // The taskbar and title-bar icon, drawn from the game's own textures.
    let atlas = texture::build_atlas(1337);
    let icon = (|| {
        Some(macroquad::miniquad::conf::Icon {
            small: texture::icon_rgba(&atlas, 16).try_into().ok()?,
            medium: texture::icon_rgba(&atlas, 32).try_into().ok()?,
            big: texture::icon_rgba(&atlas, 64).try_into().ok()?,
        })
    })();
    // Anti-aliasing and vsync have to be chosen before the window opens.
    let video = settings::Settings::load(&settings::Settings::early_path());
    Conf {
        window_title: "Minceraft".to_owned(),
        icon,
        window_width: 1280,
        window_height: 720,
        high_dpi: false,
        sample_count: video.msaa.max(1) as i32,
        window_resizable: true,
        platform: macroquad::miniquad::conf::Platform { swap_interval: Some(if video.vsync { 1 } else { 0 }), ..Default::default() },
        ..Default::default()
    }
}

/// How many frames the F3 frame-time graph shows.
const FRAME_GRAPH: usize = 240;

/// The graphics card's name and the OpenGL version, as the driver reports them.
fn gl_strings() -> (String, String) {
    use macroquad::miniquad::gl;
    let get = |name: u32| unsafe {
        let p = gl::glGetString(name);
        if p.is_null() { "unknown".to_string() } else { std::ffi::CStr::from_ptr(p as *const std::ffi::c_char).to_string_lossy().into_owned() }
    };
    // 0x1F01 is GL_RENDERER (miniquad doesn't name it).
    (get(0x1F01), get(gl::GL_VERSION))
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
        if self.screen == Screen::Bench && s != Screen::Bench {
            self.game.close_bench();
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
        if self.screen == Screen::Loom && s != Screen::Loom {
            self.game.loom = None;
        }
        if self.screen == Screen::Book && s != Screen::Book {
            // Closing a Book and Quill keeps what was written.
            if let Some(view) = self.game.reading.take()
                && let Some(slot) = view.writing
                && !self.book_done
            {
                self.game.finish_writing(slot, std::mem::take(&mut self.book_pages), None);
            }
        }
        if s == Screen::Book {
            self.book_pages = self.game.reading.as_ref().map(|r| r.book.pages.clone()).unwrap_or_default();
            if self.book_pages.is_empty() {
                self.book_pages.push(String::new());
            }
            self.book_page = 0;
            self.book_title = None;
            self.book_done = false;
            drain_chars();
        }
        if s == Screen::Sign {
            self.sign_lines = Default::default();
            self.sign_line = 0;
            drain_chars();
        }
        // Leaving a screen where settings change: keep them for next time.
        if matches!(self.screen, Screen::Options { .. } | Screen::Controls { .. } | Screen::Video { .. } | Screen::Multiplayer) && self.screen != s {
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
        self.game.shield_banner = self.settings.shield_banner;
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
        // A copy as it was before this session, in case anything goes wrong.
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        if let Err(e) = backups::back_up(&save::saves_dir(), &backups::backups_dir(), &id, now) {
            eprintln!("Minceraft: couldn't back up \"{}\": {e}", w.name);
            self.status = Some((format!("Couldn't back up this world first: {e}"), 6.0));
        }
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
        if self.form_hardcore {
            self.game.rules.hardcore = true;
            self.game.rules.keep_inventory = false;
            self.game.rules.difficulty = rules::Difficulty::Hard;
            self.game.msg("Hardcore: one life, hard mode. Good luck.");
        }
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
        if self.screen == Screen::Playing
            && let Some(last) = self.last_mouse {
                let d = m - last;
                if d.length() < 400.0 {
                    // (Slower through a Spyglass.)
                    let zoom = if self.game.spyglassing() { 0.25 } else { 1.0 };
                    let s = 0.0026 * self.settings.sensitivity * zoom;
                    let p = &mut self.game.player;
                    p.yaw = (p.yaw + d.x * s).rem_euclid(std::f32::consts::TAU);
                    p.pitch = (p.pitch - d.y * s).clamp(-1.55, 1.55);
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
            Screen::DeleteWorld | Screen::Backups => {
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
                } else if (self.settings.binds.pressed(keybinds::Action::Inventory) || self.pad_frame.inventory) && !self.game.spectator {
                    self.open_inventory();
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
                if self.settings.binds.pressed(keybinds::Action::SwapHands) {
                    self.game.inv.swap_hands();
                    self.game.held_name = 2.0;
                }
            }
            Screen::Inventory if self.book_focus => {
                // Typing in the recipe book's search box.
                if is_key_pressed(KeyCode::Escape) || is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter) {
                    self.book_focus = false;
                } else {
                    let before = self.book_search.clone();
                    type_into(&mut self.book_search, 24);
                    if self.book_search != before {
                        self.recipe_scroll = 0.0;
                    }
                }
            }
            Screen::Inventory | Screen::Container | Screen::Anvil | Screen::Enchant | Screen::Trade | Screen::Bench => {
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
            Screen::Loom => {
                if is_key_pressed(KeyCode::Escape) {
                    self.set_screen(Screen::Playing);
                }
            }
            Screen::Book => {
                let writing = self.game.reading.as_ref().is_some_and(|r| r.writing.is_some());
                if is_key_pressed(KeyCode::Escape) {
                    if self.book_title.is_some() {
                        self.book_title = None;
                    } else {
                        self.set_screen(Screen::Playing);
                    }
                } else if let Some(title) = self.book_title.as_mut() {
                    if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter) {
                        self.sign_book();
                    } else {
                        type_into(title, books::TITLE_LEN);
                    }
                } else if writing {
                    let page = &mut self.book_pages[self.book_page];
                    if (is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter)) && page.chars().count() < books::PAGE_LEN {
                        page.push('\n');
                    }
                    type_into(page, books::PAGE_LEN);
                } else {
                    drain_chars();
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
            Screen::Advancements | Screen::Stats | Screen::FishLog | Screen::WorldSettings | Screen::BeeLog => {
                if is_key_pressed(KeyCode::Escape) {
                    self.set_screen(Screen::Paused);
                }
            }
            Screen::Journal => {
                if is_key_pressed(KeyCode::Escape) || self.settings.binds.pressed(keybinds::Action::Inventory) {
                    self.set_screen(self.journal_back);
                }
            }
            Screen::Video { from_title } => {
                if is_key_pressed(KeyCode::Escape) {
                    self.set_screen(Screen::Options { from_title });
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

    /// Save what's on screen to `screenshots/` (in the data folder) and say where.
    fn save_screenshot(&mut self) {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let stamp = paths::utc_date_time(secs).replace([' ', ':'], "-");
        let dir = std::path::Path::new("screenshots");
        if let Err(e) = std::fs::create_dir_all(dir) {
            self.game.msg(format!("Couldn't save a screenshot: {e}"));
            return;
        }
        let mut path = dir.join(format!("minceraft-{stamp}.png"));
        let mut n = 2;
        while path.exists() {
            path = dir.join(format!("minceraft-{stamp}-{n}.png"));
            n += 1;
        }
        get_screen_data().export_png(&path.to_string_lossy());
        let shown = std::fs::canonicalize(&path).unwrap_or(path);
        self.game.msg(format!("Saved screenshot: {}", shown.display()));
    }

    fn frame(&mut self) {
        if self.settings.binds.pressed(keybinds::Action::Screenshot) && self.chat.is_none() && self.rebinding.is_none() {
            self.shot_pending = true;
        }
        let dt = get_frame_time().min(0.05);
        self.fps = self.fps * 0.95 + (1.0 / get_frame_time().max(1e-4)) * 0.05;
        if self.frame_times.len() == FRAME_GRAPH {
            self.frame_times.pop_front();
        }
        self.frame_times.push_back(get_frame_time() * 1000.0);
        self.ui.begin_frame(self.settings.ui_scale);
        self.pad_frame = self.pad.poll();
        self.handle_keys();
        self.mouse_look();

        let controls = self.controls();
        // Multiplayer worlds never pause: other people are still in them.
        let simulate = matches!(self.screen, Screen::Playing | Screen::Inventory | Screen::Container | Screen::Anvil | Screen::Enchant | Screen::Trade | Screen::Bench | Screen::Journal | Screen::Title | Screen::Dead) || self.game.net.is_some();
        let t_tick = std::time::Instant::now();
        if simulate {
            if self.game.map_colors.len() != self.map_colors.len() {
                self.game.map_colors = self.map_colors.clone();
            }
            self.game.update(dt, &controls);
        }
        let tick_ms = t_tick.elapsed().as_secs_f32() * 1000.0;
        // Right-clicked a chest or furnace: show it. Broken under us: close it.
        if self.game.open.is_some() && self.screen == Screen::Playing {
            self.set_screen(Screen::Container);
        } else if self.screen == Screen::Container && !self.game.container_still_there() {
            self.set_screen(Screen::Playing);
        }
        if std::mem::take(&mut self.game.at_table) && self.screen == Screen::Playing {
            self.open_inventory();
        }
        if self.game.anvil.is_some() && self.screen == Screen::Playing {
            self.set_screen(Screen::Anvil);
        } else if self.screen == Screen::Anvil && !self.game.anvil_still_there() {
            self.set_screen(Screen::Playing);
        }
        if self.game.bench.is_some() && self.screen == Screen::Playing {
            self.set_screen(Screen::Bench);
        } else if self.screen == Screen::Bench && !self.game.bench_still_there() {
            self.set_screen(Screen::Playing);
        }
        if std::mem::take(&mut self.game.open_journal) && self.screen == Screen::Playing {
            self.journal_back = Screen::Playing;
            self.set_screen(Screen::Journal);
        }
        if self.game.enchanting.is_some() && self.screen == Screen::Playing {
            self.set_screen(Screen::Enchant);
        } else if self.screen == Screen::Enchant && !self.game.enchanting_still_there() {
            self.set_screen(Screen::Playing);
        }
        if self.game.editing_sign.is_some() && self.screen == Screen::Playing {
            self.set_screen(Screen::Sign);
        }
        if self.game.reading.is_some() && self.screen == Screen::Playing {
            self.set_screen(Screen::Book);
        }
        if self.game.loom.is_some() && self.screen == Screen::Playing {
            self.set_screen(Screen::Loom);
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
            self.ui.tex.update_from_bytes(texture::UI_ATLAS as u32, texture::UI_ATLAS as u32, &texture::ui_atlas(&atlas));
        }

        // 3D world
        let sky = self.game.sky_color();
        clear_background(Color::new(sky[0], sky[1], sky[2], 1.0));
        {
            let mut gl = unsafe { get_internal_gl() };
            gl.flush();
            let rd = self.settings.render_distance;
            let t = std::time::Instant::now();
            self.game.stream(&mut self.renderer, gl.quad_context, rd);
            let mesh_ms = t.elapsed().as_secs_f32() * 1000.0;
            let t = std::time::Instant::now();
            let aspect = screen_width() / screen_height().max(1.0);
            let cam = self.game.camera(aspect, self.settings.fov);
            self.last_view_proj = cam.view_proj;
            let geo = self.game.build_geo(&cam, rd);
            let fp = self.game.frame_params(&cam, &self.renderer, rd);
            let scene_ms = t.elapsed().as_secs_f32() * 1000.0;
            let t = std::time::Instant::now();
            self.renderer.draw(gl.quad_context, &fp, &geo);
            let draw_ms = t.elapsed().as_secs_f32() * 1000.0;
            self.note_stages([tick_ms, mesh_ms, scene_ms, draw_ms, self.stage_ms[4]]);
        }

        set_default_camera();
        let t = std::time::Instant::now();
        self.draw_ui();
        self.play_sounds(dt);
        self.stage_ms[4] = self.stage_ms[4] * 0.9 + t.elapsed().as_secs_f32() * 1000.0 * 0.1;
    }

    /// Smooth the per-stage CPU times shown in F3 (UI is smoothed where it's measured).
    fn note_stages(&mut self, now: [f32; 5]) {
        for (k, v) in now.iter().enumerate().take(4) {
            self.stage_ms[k] = self.stage_ms[k] * 0.9 + v * 0.1;
        }
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

    /// Start hosting (if not already). Returns the port.
    fn host_now(&mut self) -> Option<u16> {
        if self.game.is_host()
            && let Some(multiplayer::Net::Host(srv)) = &self.game.net {
                return Some(srv.port);
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

    /// Sign the Book and Quill on screen with the title typed in.
    fn sign_book(&mut self) {
        let title = self.book_title.clone().unwrap_or_default();
        if title.trim().is_empty() {
            return;
        }
        if let Some(slot) = self.game.reading.as_ref().and_then(|r| r.writing) {
            let pages = self.book_pages.clone();
            self.game.finish_writing(slot, pages, Some(title));
            self.book_done = true;
        }
        self.set_screen(Screen::Playing);
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

    /// Potion effects and their time left, down the left side.
    fn effects_hud(&self) {
        let s = self.ui.s;
        let h = screen_height();
        for (i, effect) in self.game.effects.iter().enumerate() {
            let y = h * 0.35 + i as f32 * 12.0 * s;
            let c = effect.kind.colour();
            let secs = effect.seconds.max(0.0) as i32;
            let level = effect.amplifier as u16 + 1;
            let name = if level > 1 { format!("{} {level}", effect.kind.name()) } else { effect.kind.name().to_string() };
            draw_rectangle(4.0 * s, y - 7.0 * s, 6.0 * s, 6.0 * s, Color::from_rgba(c[0], c[1], c[2], 255));
            self.ui.text(&format!("{name} {}:{:02}", secs / 60, secs % 60), 13.0 * s, y, 8.0, WHITE);
        }
    }

    fn play_sounds(&mut self, dt: f32) {
        let listener = self.game.player.eye();
        if self.ui.pressed.replace(false) {
            self.audio.play(Sfx::Click, None, listener);
        }
        // Keep the game world quiet while paused or in menus layered over it.
        let world_audible = !matches!(self.screen, Screen::Paused | Screen::Options { .. } | Screen::Controls { .. } | Screen::Video { .. } | Screen::Help { .. } | Screen::Advancements | Screen::Stats | Screen::FishLog | Screen::BeeLog);
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
        self.game.waving_leaves = self.settings.waving_leaves;
        self.game.fancy_clouds = self.settings.fancy_clouds;
        self.game.clouds_on = self.settings.clouds;
        self.game.fog_on = self.settings.fog;
        self.game.view_bobbing = self.settings.view_bobbing;
        self.game.particle_level = self.settings.particles;
        self.game.water_reflections = self.settings.water_reflections;
        self.game.shadows = self.settings.shadows;
        self.game.fancy_water = self.settings.fancy_water;
        self.game.distant_terrain = self.settings.distant_terrain;
        if mesher::smooth() != self.settings.smooth_lighting {
            // Every chunk has to be meshed again with the other kind of lighting.
            mesher::set_smooth(self.settings.smooth_lighting);
            let all: Vec<(i32, i32)> = self.game.world.chunks.keys().copied().collect();
            self.game.world.dirty.extend(all);
        }
        if seasons::shown() != self.game.season() {
            // Grass and leaves change colour: everything is meshed again.
            seasons::set_shown(self.game.season());
            let all: Vec<(i32, i32)> = self.game.world.chunks.keys().copied().collect();
            self.game.world.dirty.extend(all);
        }
        if light::brightness() != self.settings.brightness {
            // Light is baked into the chunk meshes, so they all have to be redone.
            light::set_brightness(self.settings.brightness);
            let all: Vec<(i32, i32)> = self.game.world.chunks.keys().copied().collect();
            self.game.world.dirty.extend(all);
        }
        let in_game = !self.game.menu && self.screen != Screen::Dead;
        self.updates.poll();
        let amb = if in_game { self.game.ambience() } else { sound::Ambience::default() };
        self.audio.update_music(dt, in_game, amb);
        let juke = if in_game { self.game.nearest_jukebox() } else { None };
        self.audio.update_jukebox(dt, juke, self.game.player.eye());
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
            Screen::Video { from_title } => {
                draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.55));
                self.video_screen(from_title);
            }
            Screen::Controls { from_title } => {
                draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.7));
                self.controls_screen(from_title);
            }
            Screen::Mods => {
                draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.7));
                self.mods_screen();
            }
            Screen::Worlds | Screen::CreateWorld | Screen::RenameWorld | Screen::DeleteWorld | Screen::Backups => {
                draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.65));
                match self.screen {
                    Screen::Worlds => self.worlds_screen(),
                    Screen::CreateWorld => self.create_world_screen(),
                    Screen::RenameWorld => self.rename_world_screen(),
                    Screen::Backups => self.backups_screen(),
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
                    if self.screen == Screen::Playing && !self.game.creative {
                        self.pinned_recipe();
                    }
                }
                match self.screen {
                    Screen::Paused => self.pause_screen(),
                    Screen::Advancements => self.advancements_screen(),
                    Screen::Stats => self.stats_screen(),
                    Screen::FishLog => self.fish_log_screen(),
                    Screen::Journal => self.journal_screen(),
                    Screen::BeeLog => self.bee_log_screen(),
                    Screen::Bench => self.bench_screen(),
                    Screen::Inventory => self.inventory_screen(),
                    Screen::Container => self.container_screen(),
                    Screen::Anvil => self.anvil_screen(),
                    Screen::Enchant => self.enchant_screen(),
                    Screen::Trade => self.trade_screen(),
                    Screen::Sign => self.sign_screen(),
                    Screen::Book => self.book_screen(),
                    Screen::Loom => self.loom_screen(),
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

    /// While brushing: how uncovered the find is, and how hard you're pressing.
    /// The raid bar across the top (see raids.rs).
    fn raid_hud(&self) {
        let Some((state, wave, waves, left, _)) = self.game.raid_hud else { return };
        let (w, s) = (screen_width(), self.ui.s);
        let bw = (220.0 * s).min(w * 0.7);
        let (x, y) = (w / 2.0 - bw / 2.0, 22.0 * s);
        let (label, fill, col) = match state {
            2 => ("Raid - Victory!".to_string(), 1.0, Color::new(0.35, 0.85, 0.35, 1.0)),
            3 => ("Raid - Defeat".to_string(), 0.0, Color::new(0.6, 0.15, 0.15, 1.0)),
            _ => (
                format!("Raid - wave {wave} of {waves} - {left} raider{} left", if left == 1 { "" } else { "s" }),
                1.0 - (wave.saturating_sub(1) as f32 + if left == 0 { 1.0 } else { 0.0 }) / waves.max(1) as f32,
                Color::new(0.85, 0.15, 0.15, 1.0),
            ),
        };
        self.ui.text_centered(&label, w / 2.0, y - 4.0 * s, 9.0, WHITE);
        draw_rectangle(x - 2.0 * s, y - 2.0 * s + 2.0 * s, bw + 4.0 * s, 6.0 * s + 4.0 * s, Color::new(0.0, 0.0, 0.0, 0.6));
        draw_rectangle(x, y + 2.0 * s, bw * fill.clamp(0.0, 1.0), 6.0 * s, col);
    }

    fn brushing_hud(&self) {
        let Some(d) = &self.game.dig else { return };
        if d.idle > 1.5 {
            return;
        }
        let (w, h, s) = (screen_width(), screen_height(), self.ui.s);
        let bw = (160.0 * s).min(w * 0.6);
        let x = w / 2.0 - bw / 2.0;
        let y = h * 0.62;
        let bar = |y: f32, v: f32, col: Color, label: &str| {
            draw_rectangle(x - 2.0 * s, y - 2.0 * s, bw + 4.0 * s, 10.0 * s + 4.0 * s, Color::new(0.0, 0.0, 0.0, 0.6));
            draw_rectangle(x, y, bw * v.clamp(0.0, 1.0), 10.0 * s, col);
            self.ui.text(label, x, y - 4.0 * s, 8.0, WHITE);
        };
        let p = d.pressure;
        let pcol = if p > 0.75 { Color::new(0.95, 0.2, 0.15, 1.0) } else if p > 0.5 { Color::new(0.95, 0.7, 0.2, 1.0) } else { Color::new(0.4, 0.85, 0.4, 1.0) };
        let cracks = if d.cracks > 0 { format!(" ({} crack{})", d.cracks, if d.cracks == 1 { "" } else { "s" }) } else { String::new() };
        bar(y, d.progress, Color::new(0.85, 0.7, 0.4, 1.0), &format!("Uncovered{cracks}"));
        bar(y + 26.0 * s, p, pcol, if p > 0.75 { "PRESSURE - EASE OFF!" } else { "Pressure (let go to ease off)" });
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

    /// A "- label +" row for the Options screens; returns -1, 0 or 1.
    fn stepper(&self, label: &str, x: f32, y: f32, bw: f32, bh: f32) -> i32 {
        let s = self.ui.s;
        let small = bh * 1.3;
        let mut d = 0;
        if self.ui.button(Rect::new(x, y, small, bh), "-", true) {
            d = -1;
        }
        let r = Rect::new(x + small + 4.0 * s, y, bw - 2.0 * small - 8.0 * s, bh);
        draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.15, 0.15, 0.18, 0.9));
        self.ui.text_centered(label, r.x + r.w / 2.0, r.y + r.h * 0.68, 10.0, WHITE);
        if self.ui.button(Rect::new(x + bw - small, y, small, bh), "+", true) {
            d = 1;
        }
        d
    }

    fn inventory_slot_click(&mut self, i: usize, l: bool, r: bool, shift: bool) {
        if l && shift && self.game.bench.is_some() {
            self.game.bench_quick_put(i);
        } else if l && shift && self.game.anvil.is_some() {
            self.game.anvil_quick_put(i);
        } else if l && shift && self.game.enchanting.is_some() {
            self.game.enchant_quick_put(i);
        } else if l && shift {
            self.game.container_quick_put(i);
        } else if self.game.bundle_click(i, l, r) {
        } else if l {
            self.game.inv.click(i);
        } else if r {
            self.game.inv.right_click(i);
        }
    }

    fn open_inventory(&mut self) {
        self.recipe_scroll = 0.0;
        self.book_focus = false;
        drain_chars();
        self.set_screen(Screen::Inventory);
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
    // The Scorchlands' structures are a long way east.
    let (cx0, cz0) = if matches!(mode, "fortress" | "camp") { (crate::scorch::SCORCH_ORIGIN / 16, 0) } else { (cx0, cz0) };
    let kind = match mode {
        "outpost" => Kind::Outpost,
        "trials" => Kind::TrialChambers,
        "fortress" => Kind::Fortress,
        "camp" => Kind::SnoutCamp,
        "raid" => Kind::Village,
        "hut" => Kind::Hut,
        "tower" => Kind::Tower,
        "well" => Kind::Well,
        "dungeon" => Kind::Dungeon,
        "village" => Kind::Village,
        "city" => Kind::HushedCity,
        "ruins" => Kind::DesertRuins,
        "trailruins" => Kind::TrailRuins,
        "oceanruins" => Kind::OceanRuins,
        "shipwreck" => Kind::Shipwreck,
        "deepdark" => {
            // Inside a Deep Dark cavern, standing on its floor.
            for r in 0..200 {
                for (dx, dz) in ring(r) {
                    let (x, z) = ((cx0 + dx) * 16 + 8, (cz0 + dz) * 16 + 8);
                    if !generator.deep_dark(x, z) || generator.column(x, z).0 < deepdark::DEEP_TOP + 8 {
                        continue;
                    }
                    for y in 5..deepdark::DEEP_TOP - 6 {
                        let open = (0..4).all(|k| generator.deep_cavern(x, y + k, z)) && !generator.deep_cavern(x, y - 1, z);
                        if open {
                            return Some((Vec3::new(x as f32 + 0.5, y as f32 + 1.6, z as f32 + 0.5), 0.7, -0.15));
                        }
                    }
                }
            }
            return None;
        }
        "beenest" => {
            // An oak with a nest on it (the same rule the generator uses).
            for r in 0..120 {
                for (dx, dz) in ring(r) {
                    for k in 0..256 {
                        let (tx, tz) = ((cx0 + dx) * 16 + k % 16, (cz0 + dz) * 16 + k / 16);
                        let Some((h, trunk, kind)) = generator.tree_at(tx, tz) else { continue };
                        let biome = generator.column(tx, tz).1;
                        if kind == world::TreeKind::Oak && noise::hash2(generator.seed ^ 0xBEE, tx, tz) < 0.07 && matches!(biome, world::Biome::Plains | world::Biome::Forest) {
                            let nest = Vec3::new(tx as f32 + 0.5, (h + (trunk - 3).max(1)) as f32 + 0.5, tz as f32 + 0.5);
                            return Some(look(nest + Vec3::new(4.5, 0.5, 4.5), nest));
                        }
                    }
                }
            }
            return None;
        }
        "swamp" | "jungle" | "badlands" | "taiga" | "cherry" | "mangrove" | "palegarden" | "aurora" | "autumn" | "rainbow" => {
            let want = match mode {
                "aurora" => world::Biome::Snowy,
                "autumn" => world::Biome::Forest,
                "rainbow" => world::Biome::Plains,
                "swamp" => world::Biome::Swamp,
                "jungle" => world::Biome::Jungle,
                "badlands" => world::Biome::Badlands,
                "cherry" => world::Biome::Cherry,
                "palegarden" => world::Biome::PaleGarden,
                "mangrove" => world::Biome::Mangrove,
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
        "ocean" => {
            // Above deep-ish open sea.
            for r in 0..160 {
                for (dx, dz) in ring(r) {
                    let (x, z) = ((cx0 + dx) * 16 + 8, (cz0 + dz) * 16 + 8);
                    let (h, biome) = generator.column(x, z);
                    if biome == world::Biome::Ocean && h < world::SEA - 6 && !generator.cold(x, z) {
                        return Some((Vec3::new(x as f32 + 0.5, world::SEA as f32 - 3.0, z as f32 + 0.5), 0.8, -0.4));
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
                Kind::HushedCity => look(o + Vec3::new(-7.0, 4.5, -9.0), o + Vec3::new(0.0, 3.0, 0.0)),
                Kind::Fortress => look(o + Vec3::new(-22.0, 22.0, -26.0), o + Vec3::new(0.0, 0.0, -4.0)),
                Kind::SnoutCamp => look(o + Vec3::new(-9.0, 6.0, -10.0), o + Vec3::Y * 1.5),
                Kind::Outpost => look(o + Vec3::new(-14.0, 10.0, -14.0), o + Vec3::Y * 7.0),
                Kind::TrialChambers => look(o + Vec3::new(-5.5, 5.5, -5.5), o + Vec3::new(2.0, 1.0, 2.0)),
                Kind::DesertRuins | Kind::TrailRuins | Kind::OceanRuins => look(o + Vec3::new(-7.0, 7.0, -7.0), o + Vec3::new(1.0, 0.0, 0.0)),
                Kind::Shipwreck => look(o + Vec3::new(-8.0, 3.0, -6.0), o + Vec3::Y * 1.0),
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

/// One hook for every panic. A missing or broken sound device panics inside
/// the audio library, on its own thread and possibly before the game starts:
/// that just means playing in silence. Anything else prints as usual and
/// leaves crash.txt behind (in the data folder for the game, see paths.rs).
fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let file = info.location().map(|l| l.file()).unwrap_or("");
        if file.contains("quad-snd") || file.contains("quad_snd") || file.contains("alsa") {
            if !sound::AUDIO_DEAD.swap(true, std::sync::atomic::Ordering::Relaxed) {
                eprintln!("Minceraft: no usable audio device ({info}). Continuing in silence.");
            }
            return;
        }
        default(info);
        if let Some(p) = paths::write_crash_report(info) {
            eprintln!("Minceraft crashed. The details are in {}", p.display());
        }
    }));
}

/// Tooltip text for a slot: its name, enchantments, and uses left for tools.
fn label(stack: Option<(Id, u8)>, wear: inventory::Wear) -> Option<String> {
    let (id, _) = stack?;
    let mut s = item_name(id).to_string();
    if id == BANNER {
        s += &format!(" [{}]", banners::Design::from_bits(enchant::enchants(wear)).describe());
    }
    if id == HOLLOW_BOX || id == BUNDLE {
        if boxes::box_id(wear) != 0 {
            s += " [packed]";
        }
    } else if id == TREASURE_MAP {
        if let Some((x, z)) = treasure::marked(wear) {
            s += &format!(" [X at {x}, {z}]");
        }
    } else if enchant::is_enchanted(wear) {
        s += &format!(" [{}]", enchant::describe_for(id, wear));
    }
    if let Some(t) = trims::describe(wear).filter(|_| armor_of(id).is_some()) {
        s += &format!(" [{t}]");
    }
    if let Some(max) = inventory::max_uses(id, wear) {
        s += &format!(" ({}/{max} uses left)", max.saturating_sub(inventory::uses(wear) as u32));
    }
    if let Some(d) = archaeology::describe(id, wear) {
        s += &format!("\n{d}");
    }
    if id == QUEEN_BEE {
        s += &format!("\nTemperament: {}", bees::Queen::of_wear(wear).name());
    }
    if let Some(f) = bees::Flavour::of_item(id) {
        s += &format!("\n{}", f.taste());
    }
    Some(s)
}

fn main() {
    install_panic_hook();
    let args: Vec<String> = std::env::args().collect();
    // Headless modes run before any window (or GPU) is touched.
    if args.iter().any(|a| a == "--server") {
        std::process::exit(server::run(&args));
    }
    if args.iter().any(|a| a == "--playtest") {
        std::process::exit(playtest::run(&args));
    }
    if let Some(i) = args.iter().position(|a| a == "--export-icon") {
        // Writes the icon the Windows build embeds (assets/minceraft.ico).
        let path = args.get(i + 1).map(String::as_str).unwrap_or("minceraft.ico");
        match std::fs::write(path, texture::icon_ico(&texture::build_atlas(1337))) {
            Ok(()) => println!("Wrote {path}"),
            Err(e) => eprintln!("Couldn't write {path}: {e}"),
        }
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "--export-sounds") {
        let dir = args.get(i + 1).map(String::as_str).unwrap_or("sounds");
        match sound::export_wavs(std::path::Path::new(dir)) {
            Ok(n) => println!("Wrote {n} sounds to {dir}/"),
            Err(e) => eprintln!("Couldn't export sounds: {e}"),
        }
        return;
    }
    // Screenshot runs stay where they're started; the game proper works in the
    // player's data folder (see paths.rs).
    if !args.iter().any(|a| a == "--screenshot") {
        let (_, notes) = paths::enter_data_dir();
        for n in notes {
            eprintln!("Minceraft: {n}");
        }
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
    let tex = Texture2D::from_rgba8(texture::UI_ATLAS as u16, texture::UI_ATLAS as u16, &texture::ui_atlas(&atlas));
    tex.set_filter(FilterMode::Nearest);

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
        palette_scroll: 0,
        book_search: String::new(),
        book_focus: false,
        book_tab: crafting::Tab::All,
        book_craftable: false,
        journal_back: Screen::Playing,
        quit: false,
        shot_pending: false,
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
        book_pages: Vec::new(),
        book_page: 0,
        book_title: None,
        book_done: false,
        loom_pattern: 1,
        loom_colour: 2,
        pad: pad::Pad::new(),
        pad_frame: Default::default(),
        rebinding: None,
        rebind_armed: false,
        mods_scroll: 0,
        adv_scroll: 0,
        adv_tab: 0,
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
        form_hardcore: false,
        form_focus: 0,
        backup_list: Vec::new(),
        backup_sel: None,
        updates: updates::UpdateCheck::start(false),
        frame_times: std::collections::VecDeque::with_capacity(FRAME_GRAPH),
        gpu: gl_strings(),
        stage_ms: [0.0; 5],
        video_at_start: (true, 4),
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
        app.video_at_start = (saved.vsync, saved.msaa);
        app.settings = saved;
        app.updates = updates::UpdateCheck::start(app.settings.check_updates);
    }
    if let Some(d) = shot.as_ref().and_then(|s| s.distance) {
        app.settings.render_distance = d.clamp(3, settings::MAX_RENDER_DISTANCE);
    }
    let flag = |f: &str| std::env::args().any(|a| a == f);
    if shot.is_some() {
        app.settings.colour_blind = flag("--colour-blind");
        app.settings.smooth_lighting = !flag("--flat-lighting");
        let args: Vec<String> = std::env::args().collect();
        let arg = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
        if let Some(v) = arg("--ui-scale").and_then(|v| v.parse::<f32>().ok()) {
            app.settings.ui_scale = v;
        }
        app.settings.fancy_clouds = !flag("--fast-clouds");
        app.settings.shadows = !flag("--no-shadows");
        app.settings.fancy_water = !flag("--simple-water");
        // Show the "new version" button as if one were out (for screenshots).
        if let Some(tag) = arg("--pretend-update") {
            app.updates.newer = Some(tag);
        }
        if let Some(b) = args.iter().position(|a| a == "--brightness").and_then(|i| args.get(i + 1)).and_then(|v| v.parse::<f32>().ok()) {
            app.settings.brightness = b.clamp(0.0, 1.0);
        }
        app.settings.subtitles = flag("--subtitles");
    }
    let broken: Vec<&block::ModInfo> = mod_infos.iter().filter(|m| m.enabled && !m.errors.is_empty()).collect();
    if let Some(m) = broken.first() {
        app.status = Some((format!("Mod \"{}\" has {} problem(s): see the Mods screen.", m.name, m.errors.len()), 8.0));
    }

    if let Some(s) = &mut shot {
        match s.mode.as_str() {
            "survival" | "creative" | "inventory" | "night" | "death" => {
                // --seed N shows someone else's world (to look at a reported bug).
                let seed = std::env::args().collect::<Vec<_>>().windows(2).find(|w| w[0] == "--seed").and_then(|w| w[1].parse().ok()).unwrap_or(424242);
                let mut g = Game::new(seed, s.mode == "creative", false);
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
                    g.inv.offhand = Some((block::TORCH, 16));
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
            "farm" | "fish" | "kitchen" | "chest" | "furnace" | "building" | "armour" | "anvil" | "rules" | "xp" | "enchant" | "table" | "liquids" | "zappy" | "trade" | "vehicles" | "decor" | "carpentry" | "brewing" | "contraptions" | "machines" | "newblocks" | "glider" => {
                let mut g = Game::new(424242, matches!(s.mode.as_str(), "farm" | "newblocks"), false);
                g.time = s.time.unwrap_or(0.2);
                if s.mode == "fish" {
                    g.inv.slots[0] = Some((block::ROD, 1));
                }
                app.start_game(g);
                app.show_debug = false;
            }
            "hut" | "tower" | "well" | "dungeon" | "village" | "ravine" | "rain" | "thunder" | "snow" | "swamp" | "jungle" | "badlands" | "taiga" | "cherry" | "mangrove" | "palegarden" | "city" | "ruins" | "trailruins" | "oceanruins" | "shipwreck" | "deepdark" | "beenest" | "outpost" | "fortress" | "camp" | "raid" | "trials" | "aurora" | "autumn" | "rainbow" => {
                // Somewhere the generator built something (or the sky is doing something).
                let mut g = Game::new(424242, true, false);
                g.time = s.time.unwrap_or(if s.mode == "aurora" { 0.8 } else { 0.3 });
                match s.mode.as_str() {
                    "autumn" => {
                        g.rules.seasons = true;
                        g.day = seasons::SEASON_DAYS * 2 + 3;
                    }
                    "rainbow" => {
                        g.rainbow = skies::RAINBOW_SECS;
                        g.time = s.time.unwrap_or(0.38);
                    }
                    _ => {}
                }
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
                // Look up at the sky: north for an aurora, away from the sun for a rainbow.
                if s.mode == "aurora" {
                    (s.yaw, s.pitch) = (0.0, 0.45);
                } else if s.mode == "rainbow" {
                    let a = g.sun_angle();
                    let anti = -Vec3::new(a.cos(), a.sin(), 0.25);
                    (s.yaw, s.pitch) = (anti.x.atan2(-anti.z), 0.2);
                }
                app.start_game(g);
                app.show_debug = false;
            }
            "cave" | "caverain" => {
                // Standing in a roomy cave pocket near spawn, well below the surface.
                let mut g = Game::new(424242, true, false);
                g.time = s.time.unwrap_or(0.3);
                if s.mode == "caverain" {
                    // A storm overhead must stay overhead.
                    g.weather.kind = weather::Weather::Rain;
                    g.weather.strength = 1.0;
                    g.rules.weather_cycle = false;
                }
                let (cx0, cz0) = ((g.spawn.x / 16.0).floor() as i32, (g.spawn.z / 16.0).floor() as i32);
                for cz in cz0 - 4..=cz0 + 4 {
                    for cx in cx0 - 4..=cx0 + 4 {
                        g.world.load_now(cx, cz);
                    }
                }
                let air = |g: &Game, x: i32, y: i32, z: i32| g.world.get(x, y, z) == AIR;
                let mut found = None;
                'search: for r in 0i32..56 {
                    for dz in -r..=r {
                        for dx in -r..=r {
                            if dx.abs().max(dz.abs()) != r {
                                continue;
                            }
                            let (x, z) = (cx0 * 16 + 8 + dx, cz0 * 16 + 8 + dz);
                            let top = (0..world::CH).rev().find(|&y| !air(&g, x, y, z)).unwrap_or(0);
                            for y in (14..top - 8).rev() {
                                let roomy = (-1..=1).all(|i| (-1..=1).all(|k| (0..3).all(|h| air(&g, x + i, y + h, z + k))));
                                let floor = is_solid(g.world.get(x, y - 1, z)) && (0..=8).all(|i| !is_liquid(g.world.get(x + i, y - 1, z)));
                                let far = (3..9).filter(|d| air(&g, x + d, y + 1, z)).count() >= 4;
                                if roomy && floor && far {
                                    found = Some(Vec3::new(x as f32 + 0.5, y as f32 + 1.6, z as f32 + 0.5));
                                    break 'search;
                                }
                            }
                        }
                    }
                }
                if s.pos.is_none() {
                    s.pos = found;
                    s.yaw = std::f32::consts::FRAC_PI_2;
                    s.pitch = -0.2;
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
            "zoo" | "animals" | "newmobs" | "music" | "modzoo" | "banners" | "golems" | "homestead" => {
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
            "shadowtest" => {
                // A floating platform over flat ground: its shadow should land just beside it.
                let mut g = Game::new(424242, true, false);
                g.time = s.time.unwrap_or(0.1);
                let base = g.spawn.floor().as_ivec3();
                for dz in -1..=1 {
                    for dx in -1..=1 {
                        g.world.load_now(base.x.div_euclid(16) + dx, base.z.div_euclid(16) + dz);
                    }
                }
                let ground = g.world.surface_y(base.x, base.z);
                for z in -12..=12 {
                    for x in -12..=12 {
                        for y in ground + 1..ground + 12 {
                            g.world.set(base.x + x, y, base.z + z, AIR);
                        }
                        g.world.set(base.x + x, ground, base.z + z, STONE);
                    }
                }
                for z in -2..=2 {
                    for x in -2..=2 {
                        g.world.set(base.x + x, ground + 5, base.z + z, GOLD_ORE);
                    }
                }
                let o = Vec3::new(base.x as f32 + 0.5, ground as f32 + 1.0, base.z as f32 + 0.5);
                let from = o + Vec3::new(-9.0, 10.0, -9.0);
                let d = o - from;
                (s.pos, s.yaw, s.pitch) = (Some(from), d.x.atan2(-d.z), d.y.atan2(Vec2::new(d.x, d.z).length()));
                g.player.flying = true;
                app.start_game(g);
                app.show_debug = false;
            }
            "recipes" => {
                let mut g = Game::new(424242, false, false);
                for (item, n) in [(LOG, 12), (PLANKS, 20), (COBBLE, 20), (IRON, 4), (STICK, 6), (COAL, 5), (WHEAT, 6), (HONEYCOMB, 3), (FEATHER, 2), (COPPER_INGOT, 2)] {
                    g.inv.add(item, n);
                }
                g.learn_inventory();
                g.pinned = recipes().iter().position(|r| r.output.0 == BEEHIVE);
                app.start_game(g);
                app.set_screen(Screen::Inventory);
                app.book_search = "pick".into();
            }
            "journal" | "beelog" => {
                let mut g = Game::new(424242, false, false);
                for (i, c) in [(0, 0), (1, 2), (2, 1), (5, 0), (7, 3), (8, 1), (11, 0)] {
                    g.journal.note(SHARD_FIRST + i, c);
                }
                for (i, c) in [(0, 0), (2, 1), (3, 2), (9, 0), (10, 0), (11, 1)] {
                    g.journal.note(RELIC_FIRST + i, c);
                }
                for i in archaeology::Culture::Hushed.shards() {
                    g.journal.note(SHARD_FIRST + *i as Id, 0);
                }
                g.journal.xp = 140;
                g.journal.digs = 31;
                g.journal.cracked = 9;
                g.journal.shattered = 2;
                g.journal.restored = 3;
                g.bee_log = bees::BeeLog { xp: 95, bottles: [14, 6, 3, 8, 1], comb: 21, stings: 4, swarms_caught: 2, swarms_lost: 1, queens: 3, colonies_started: 2 };
                app.start_game(g);
                app.journal_back = Screen::Paused;
                app.set_screen(if s.mode == "journal" { Screen::Journal } else { Screen::BeeLog });
            }
            "bench" | "grindstone" => {
                let mut g = Game::new(424242, false, false);
                for (item, n) in [(UPGRADE_TEMPLATE, 2), (SCORCHITE_INGOT, 3), (PICK_DIAMOND, 1), (SWORD_DIAMOND, 1), (ARMOR_FIRST + 13, 1)] {
                    g.inv.add(item, n);
                }
                let bench = if s.mode == "bench" { smithing::Bench::Smithing } else { smithing::Bench::Grindstone };
                // A real one to stand at (the screen closes if it isn't there).
                let at = g.spawn.floor().as_ivec3() + IVec3::new(2, 0, 0);
                g.world.load_now(at.x.div_euclid(16), at.z.div_euclid(16));
                g.world.set_v(at, bench.block());
                app.start_game(g);
                app.game.open_bench(at, bench);
                if let Some(b) = &mut app.game.bench {
                    if bench == smithing::Bench::Smithing {
                        b.slots = [Some((UPGRADE_TEMPLATE, 1)), Some((PICK_DIAMOND, 1)), Some((SCORCHITE_INGOT, 1))];
                        b.wear[1] = enchant::with_level(120, enchant::Enchant::Efficiency, 4);
                    } else {
                        b.slots[0] = Some((SWORD_IRON, 1));
                        b.wear[0] = enchant::with_level(enchant::with_level(30, enchant::Enchant::Sharpness, 3), enchant::Enchant::Unbreaking, 2);
                    }
                }
                app.screen = Screen::Bench;
            }
            "stats" => {
                let mut g = Game::new(424242, false, false);
                g.stats = stats::Stats { mined: 1843, placed: 1207, crafted: 311, kills: 58, deaths: 3, damage_dealt: 402.0, damage_taken: 131.0, walked: 18_420.0, swum: 960.0, flown: 0.0, ridden: 2_310.0, jumps: 4_107, played: 3.0 * 3600.0 + 1260.0, fish: 12, eaten: 96 };
                app.start_game(g);
                app.set_screen(Screen::Stats);
            }
            "mods" => {
                app.game = Game::new(424242, true, true);
                app.set_screen(Screen::Mods);
            }
            "worlds" => {
                app.game = Game::new(424242, true, true);
                app.open_worlds();
            }
            "backups" => {
                // A saved world with a few backups made over the last days.
                let (root, id) = (save::saves_dir(), "castle-town");
                let _ = save::write_name(&root, id, "Castle Town");
                app.game = Game::new(424242, false, false);
                app.current_world = Some(id.into());
                let _ = app.write_current_world();
                app.game = Game::new(424242, true, true);
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                for hours in [50, 26, 3] {
                    let _ = backups::back_up(&root, &backups::backups_dir(), id, now - hours * 3600);
                }
                app.open_worlds();
                app.world_sel = app.worlds.iter().position(|w| w.id == id);
                app.backup_list = backups::list(&backups::backups_dir(), id);
                app.backup_sel = Some(0);
                app.set_screen(Screen::Backups);
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
            "video" => {
                app.game = Game::new(424242, true, true);
                app.settings.max_fps = 144;
                app.settings.particles = 1;
                app.set_screen(Screen::Video { from_title: true });
            }
            "reef" => {
                // Under a warm sea, somewhere with coral (found once the chunks are in).
                let mut g = Game::new(424242, true, false);
                g.time = s.time.unwrap_or(0.25);
                if let Some((pos, yaw, pitch)) = scenic_view(&g, "ocean") {
                    (s.pos, s.yaw, s.pitch) = (Some(pos), yaw, pitch);
                }
                app.start_game(g);
                app.show_debug = false;
            }
            "spire" => {
                // One of the Hollow's outer islands, from a little way off.
                let mut g = Game::new(424242, true, false);
                g.time = 0.25;
                let seed = g.world.seed();
                let spire = (1..12).flat_map(|r: i32| (-r..=r).flat_map(move |i| [(i, -r), (i, r), (-r, i), (r, i)])).find_map(|(gx, gz)| hollow::outer_island(seed, gx, gz).filter(|i| i.2));
                if let Some((c, _, _)) = spire {
                    let from = c.as_vec3() + Vec3::new(14.0, 9.0, 14.0);
                    let d = c.as_vec3() + Vec3::new(0.5, 4.0, 0.5) - from;
                    (s.pos, s.yaw, s.pitch) = (Some(from), d.x.atan2(-d.z), d.y.atan2(Vec2::new(d.x, d.z).length()));
                }
                g.player.flying = true;
                app.start_game(g);
                app.show_debug = false;
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

    // Something in hand for the screenshot (--hold iron_pickaxe).
    let hold = std::env::args().collect::<Vec<_>>().windows(2).find(|w| w[0] == "--hold").map(|w| w[1].clone());
    if let Some(id) = hold.and_then(|k| block::reg().lookup(&k)) {
        let sel = app.game.inv.selected;
        app.game.inv.slots[sel] = Some((id, 1));
    }
    let mut frames = 0u32;
    let mut last_frame = std::time::Instant::now();
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
            if matches!(s.mode.as_str(), "zoo" | "newmobs" | "modzoo" | "music" | "animals" | "banners" | "golems" | "homestead" | "farm" | "fish" | "kitchen" | "chest" | "furnace" | "building" | "armour" | "anvil" | "rules" | "xp" | "enchant" | "table" | "liquids" | "zappy" | "trade" | "vehicles" | "decor" | "carpentry" | "brewing" | "contraptions" | "machines" | "newblocks") && frames == 120 {
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
            if s.mode == "banners" && frames == 125 {
                // Three banners up in front, a lectern and a loom, and a painted shield in hand.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let y0 = p.y.floor() as i32;
                let at = |f: f32, r: f32| {
                    let v = p + fwd * f + right * r;
                    IVec3::new(v.x.floor() as i32, y0, v.z.floor() as i32)
                };
                let designs = [
                    banners::Design::default().with(0, 2).with(1, 0).with(4, 4),
                    banners::Design::default().with(0, 6).with(3, 0),
                    banners::Design::default().with(0, 1).with(5, 3).with(6, 4),
                ];
                let facing = banners::facing_from_yaw(s.yaw);
                for (k, d) in designs.iter().enumerate() {
                    let q = at(5.0, (k as f32 - 1.0) * 2.0);
                    app.game.world.set_v(q, block::BANNER);
                    app.game.put_up_banner(q, d.bits(), facing);
                }
                app.game.world.set_v(at(3.0, -2.5), block::LECTERN_BOOK);
                app.game.world.set_v(at(3.0, 2.5), block::LOOM);
                app.game.inv.slots[0] = Some((block::SHIELD, 1));
                app.game.inv.selected = 0;
                app.game.shield_banner = designs[1].bits();
                // A wall of framed maps behind them.
                let f = if fwd.x.abs() > fwd.z.abs() { IVec3::new(fwd.x.signum() as i32, 0, 0) } else { IVec3::new(0, 0, fwd.z.signum() as i32) };
                if let Some(facing) = decor::frame_facing(-f) {
                    for (k, up) in [(-1, 1), (0, 1), (-1, 2), (0, 2)] {
                        let wall = at(8.0, k as f32) + IVec3::Y * up;
                        app.game.world.set_v(wall, block::STONE_BRICKS);
                        let frame = wall - f;
                        app.game.world.set_v(frame, block::FRAME_FIRST + facing as u16);
                        app.game.world.frames.insert(frame, (block::MAP, 0));
                    }
                }
            }
            if s.mode == "homestead" && frames == 125 {
                // The v0.1.20 home blocks: a campfire cooking, a smoker and a blast
                // furnace going, a barrel, a dressed armour stand and paintings on a wall.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let y0 = p.y.floor() as i32;
                let at = |f: f32, r: f32| {
                    let v = p + fwd * f + right * r;
                    IVec3::new(v.x.floor() as i32, y0, v.z.floor() as i32)
                };
                let fire = at(4.0, 0.0);
                app.game.world.set_v(fire, block::CAMPFIRE);
                app.game.put_on_campfire(fire, block::PORKCHOP);
                app.game.put_on_campfire(fire, block::COD);
                app.game.world.set_v(at(6.0, -3.0), block::SMOKER_LIT);
                app.game.world.set_v(at(6.0, -2.0), block::BLAST_FURNACE_LIT);
                app.game.world.set_v(at(6.0, 2.0), block::BARREL);
                app.game.world.set_v(at(6.0, 2.0) + IVec3::Y, block::BARREL);
                let facing = (((s.yaw / std::f32::consts::FRAC_PI_2).round() as i32 + 2).rem_euclid(4)) as u16;
                let stand = at(3.5, 2.5);
                app.game.world.set_v(stand, block::ARMOUR_STAND_FIRST + facing);
                let mut c = containers::Container::for_block(block::ARMOUR_STAND_FIRST);
                c.slots[0] = Some((block::TURTLE_SHELL, 1));
                c.slots[1] = Some((block::ARMOR_FIRST + 12 + block::CHESTPLATE as u16, 1));
                c.slots[2] = Some((block::ARMOR_FIRST + 8 + block::LEGGINGS as u16, 1));
                c.slots[3] = Some((block::ARMOR_FIRST + 4 + block::BOOTS as u16, 1));
                app.game.world.containers.insert(stand, c);
                // A wall at the back with three paintings on it.
                let f = if fwd.x.abs() > fwd.z.abs() { IVec3::new(fwd.x.signum() as i32, 0, 0) } else { IVec3::new(0, 0, fwd.z.signum() as i32) };
                if let Some(facing) = decor::frame_facing(-f) {
                    for r in -4..=4 {
                        for up in 0..4 {
                            app.game.world.set_v(at(10.0, r as f32) + IVec3::Y * up, block::PLANKS);
                        }
                    }
                    for r in [-3, 0, 3] {
                        app.game.world.set_v(at(10.0, r as f32) + IVec3::Y * 2 - f, block::PAINTING_FIRST + facing as u16);
                    }
                }
                app.game.inv.slots[0] = Some((block::TREASURE_MAP, 1));
                app.game.inv.wear[0] = treasure::mark(at(40.0, 10.0));
                app.game.inv.selected = 0;
            }
            if s.mode == "homestead" && frames == 150 {
                use entity::MobKind as K;
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let mut rng = noise::Rng::new(12);
                for (i, (kind, f, r)) in [(K::Llama, 7.0, -6.0), (K::Panda, 8.0, -2.0), (K::PolarBear, 8.5, 1.5), (K::Turtle, 4.5, -3.0), (K::Dolphin, 9.0, 0.0), (K::Wanderer, 7.0, 4.0), (K::ZombieHmmer, 6.5, -4.0)].into_iter().enumerate() {
                    let mut m = entity::Mob::new(kind, p + fwd * f + right * r, &mut rng);
                    m.id = 2200 + i as u32;
                    m.saddled = kind == K::Llama;
                    m.persistent = true;
                    app.game.mobs.push(m);
                }
            }
            if s.mode == "golems" && frames == 125 {
                // A Copper Golem between its chests, a harnessed Floaty, a saddled
                // Rotsteed, and the new ground covers, with an iron spear in hand.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let y0 = p.y.floor() as i32;
                let at = |f: f32, r: f32| {
                    let v = p + fwd * f + right * r;
                    IVec3::new(v.x.floor() as i32, y0, v.z.floor() as i32)
                };
                app.game.world.set_v(at(4.0, -3.0), block::COPPER_CHEST);
                app.game.world.set_v(at(4.0, 3.0), block::CHEST);
                app.game.world.set_v(at(3.0, -5.5), block::FIREFLY_BUSH);
                app.game.world.set_v(at(3.0, 5.5), block::EYEBLOSSOM_OPEN);
                app.game.world.set_v(at(9.0, -6.0), block::RESIN_BRICKS);
                app.game.world.set_v(at(9.0, -6.0) + IVec3::Y, block::RESIN_BLOCK);
                for f in 5..8 {
                    for r in -6..-2 {
                        app.game.world.set_v(at(f as f32, r as f32), block::LEAF_LITTER);
                    }
                    for r in 3..7 {
                        app.game.world.set_v(at(f as f32, r as f32), block::WILDFLOWERS);
                    }
                }
                app.game.inv.slots[0] = Some((block::SPEAR_FIRST + 2, 1));
                app.game.inv.selected = 0;
            }
            if s.mode == "golems" && frames == 150 {
                use entity::MobKind as K;
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let mut rng = noise::Rng::new(11);
                for (i, (kind, f, r, up)) in [(K::CopperGolem, 4.0, 0.0, 0.0), (K::Rotsteed, 7.0, -2.5, 0.0), (K::Floaty, 13.0, 2.0, 2.5)].into_iter().enumerate() {
                    let mut m = entity::Mob::new(kind, p + fwd * f + right * r + Vec3::Y * up, &mut rng);
                    m.yaw = s.yaw + std::f32::consts::PI + if kind == K::Rotsteed { 1.2 } else { 0.0 };
                    m.id = 2100 + i as u32;
                    m.saddled = kind != K::CopperGolem;
                    if kind == K::CopperGolem {
                        m.seed = block::RESIN_CLUMP as u32 | 5 << 16;
                    }
                    m.persistent = true;
                    app.game.mobs.push(m);
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
            if s.mode == "newblocks" && frames == 125 {
                // Copper as it ages (and waxed), bamboo things, coral, a Hollow Box.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let y = p.y.floor() as i32;
                let at = |f: f32, r: f32, up: i32| {
                    let v = p + fwd * f + right * r;
                    IVec3::new(v.x.floor() as i32, y + up, v.z.floor() as i32)
                };
                let w = &mut app.game.world;
                for i in 0..4 {
                    w.set_v(at(6.0, i as f32 - 5.0, 0), block::COPPER_FIRST + i);
                    w.set_v(at(6.0, i as f32 - 5.0, 1), block::WAXED_COPPER_FIRST + i);
                    w.set_v(at(8.0, i as f32 - 5.0, 0), block::CORAL_FIRST + i);
                }
                w.set_v(at(8.0, -1.0, 0), block::DEAD_CORAL);
                w.set_v(at(6.0, 0.0, 0), block::COPPER_ORE);
                w.set_v(at(6.0, 1.0, 0), block::BAMBOO_BLOCK);
                w.set_v(at(6.0, 2.0, 0), block::BAMBOO_PLANKS);
                w.set_v(at(6.0, 3.0, 0), block::BAMBOO_MOSAIC);
                w.set_v(at(5.0, 1.0, 0), block::BAMBOO_SLAB);
                w.set_v(at(5.0, 2.0, 0), block::BAMBOO_STAIRS);
                w.set_v(at(5.0, 3.0, 0), block::HOLLOW_BOX);
                for up in 0..6 {
                    w.set_v(at(8.0, 3.0, up), block::BAMBOO);
                    w.set_v(at(9.0, 2.0, up.min(3)), block::BAMBOO);
                }
                app.game.inv.slots[0] = Some((block::GLIDER, 1));
                app.game.inv.slots[1] = Some((block::ROCKET, 16));
                app.game.inv.slots[2] = Some((block::SPEAR, 1));
                app.game.inv.slots[3] = Some((block::COPPER_INGOT, 12));
                app.game.inv.slots[4] = Some((block::HOLLOW_BOX, 1));
                app.game.inv.slots[5] = Some((block::BAMBOO, 32));
                app.game.inv.slots[6] = Some((block::AXE_FIRST + 2, 1));
                app.game.inv.slots[7] = Some((block::SHOVEL_FIRST + 4, 1));
                app.game.inv.slots[8] = Some((block::PICK_COPPER, 1));
                app.game.inv.selected = 6;
            }
            if s.mode == "glider" && frames == 125 {
                // Soaring: worn Glider, seen from behind.
                app.game.inv.armor[block::CHESTPLATE] = Some((block::GLIDER, 1));
                app.game.player.body.pos += Vec3::Y * 40.0;
                app.game.player.body.vel = Vec3::new(s.yaw.sin(), -0.2, -s.yaw.cos()) * 18.0;
                app.game.player.body.on_ground = false;
                app.game.player.gliding = true;
                app.game.third_person = true;
            }
            if s.mode == "reef" && frames == 150 {
                // Some sea life in front of the camera.
                let eye = app.game.player.eye();
                let fwd = Vec3::new(s.yaw.sin() * s.pitch.cos(), s.pitch.sin(), -s.yaw.cos() * s.pitch.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let mut rng = noise::Rng::new(3);
                for i in 0..5 {
                    let at = eye + fwd * (4.0 + (i % 2) as f32) + right * (i as f32 * 0.7 - 1.4) + Vec3::Y * ((i % 3) as f32 * 0.4 - 0.4);
                    let mut m = entity::Mob::new(entity::MobKind::Fishy, at, &mut rng);
                    m.id = 5000 + i;
                    m.yaw = s.yaw + 1.2;
                    app.game.mobs.push(m);
                }
                let mut m = entity::Mob::new(entity::MobKind::Soggy, eye + fwd * 8.0 - right * 3.0 - Vec3::Y * 1.0, &mut rng);
                m.seed = 1;
                m.id = 5100;
                m.yaw = s.yaw + std::f32::consts::PI;
                app.game.mobs.push(m);
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
                        // Each piece trimmed differently (see trims.rs).
                        app.game.inv.armor_wear[slot] = trims::with_trim(0, slot, [1, 2, 3, 0][slot]);
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
                let frame_facing = facing as block::Id;
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
            if s.mode == "newmobs" && frames == 150 {
                // This batch's mobs (a Camel with both seats, an Axolotl in a puddle), under fireworks.
                use entity::MobKind as K;
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let mut rng = noise::Rng::new(9);
                let line = [(K::Axolotl, 4.0, -2.0), (K::Goat, 5.0, -0.3), (K::Breeze, 5.0, 2.0), (K::Camel, 8.5, -1.2)];
                for (x, y, z) in [(-8.0, 14.0, 22.0), (4.0, 17.0, 26.0), (12.0, 12.0, 20.0)] {
                    let colour = (x as i32).rem_euclid(8) as u8;
                    app.game.firework_sparks(p + right * x + Vec3::Y * y + fwd * z, colour);
                }
                for (i, (kind, f, r)) in line.into_iter().enumerate() {
                    let up = 0.0;
                    let mut m = entity::Mob::new(kind, p + fwd * f + right * r + Vec3::Y * (2.0 + up), &mut rng);
                    m.yaw = s.yaw + std::f32::consts::PI;
                    m.id = 2000 + i as u32;
                    // A captain with its banner, a Snout admiring gold, an Invoicer casting.
                    m.saddled = kind == K::Camel;
                    app.game.mobs.push(m);
                }
            }
            if s.mode == "modzoo" && frames == 150 {
                // Every mob the loaded mods define, in a row (fliers up in the air).
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let mut rng = noise::Rng::new(9);
                let n = block::reg().mobs.len();
                for i in 0..n {
                    let kind = entity::MobKind::Modded(i as u16);
                    // Rows of five, further back each row.
                    let (row, col) = (i / 5, i % 5);
                    let in_row = (n - row * 5).min(5);
                    let r = (col as f32 - (in_row as f32 - 1.0) * 0.5) * 2.2;
                    let up = if kind.flies() { 2.5 } else { 0.0 };
                    let mut m = entity::Mob::new(kind, p + fwd * (5.0 + row as f32 * 4.0) + right * r + Vec3::Y * (2.0 + up), &mut rng);
                    m.yaw = s.yaw + std::f32::consts::PI - 0.5;
                    m.id = 3000 + i as u32;
                    m.anim = 0.8;
                    app.game.mobs.push(m);
                }
            }
            if s.mode == "music" && frames == 125 {
                // Note blocks on each kind of block, a jukebox playing, a bell, gold.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let y = p.y.floor() as i32;
                let at = |f: f32, r: f32| {
                    let v = p + fwd * f + right * r;
                    IVec3::new(v.x.floor() as i32, y, v.z.floor() as i32)
                };
                let under = [block::PLANKS, block::STONE, block::SAND, block::GLASS, block::GOLD_BLOCK, block::ICE, block::TERRACOTTA, block::WOOL, block::GRASS];
                for (i, &b) in under.iter().enumerate() {
                    let c = at(5.0, i as f32 * 1.0 - 4.0);
                    app.game.world.set_v(c, b);
                    app.game.world.set_v(c + IVec3::Y, block::NOTE_BLOCK + (i as u16 * 3) % 25);
                }
                app.game.world.set_v(at(3.0, -2.0), block::JUKEBOX_DISC_FIRST + 4);
                app.game.world.set_v(at(3.0, 0.0), block::JUKEBOX);
                app.game.world.set_v(at(3.0, 2.0) + IVec3::Y, block::BELL);
                app.game.world.set_v(at(3.0, 2.0), block::GOLD_BLOCK);
                for i in 0..6 {
                    app.game.give(block::DISC_FIRST + i, 1);
                }
                let note = at(5.0, 0.0) + IVec3::Y * 2;
                for k in 0..6 {
                    app.game.note_particle(note.as_vec3() + Vec3::new(0.5 + k as f32 * 0.2 - 0.5, 0.2 + k as f32 * 0.25, 0.5), (k * 4) as u8);
                }
            }
            if s.mode == "raid" && frames == 150 {
                // Wave two marching on the square.
                use entity::MobKind as K;
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let mut rng = noise::Rng::new(5);
                for (i, kind) in [K::Pilferer, K::Pilferer, K::Hackler, K::Pilferer, K::Invoicer, K::Rampager, K::Hackler].into_iter().enumerate() {
                    let v = p + fwd * (16.0 + (i % 3) as f32 * 2.0) + right * ((i as f32 - 3.0) * 2.2);
                    let (x, z) = (v.x.floor() as i32, v.z.floor() as i32);
                    let ground = app.game.world.surface_y(x, z) + 1;
                    let mut m = entity::Mob::new(kind, Vec3::new(x as f32 + 0.5, ground as f32, z as f32 + 0.5), &mut rng);
                    m.yaw = s.yaw + std::f32::consts::PI;
                    m.id = 3000 + i as u32;
                    m.seed = (i == 0) as u32;
                    app.game.mobs.push(m);
                }
                app.game.raid_hud = Some((1, 2, 5, 7, 999.0));
                app.game.bell_glow = 999.0;
            }
            if matches!(s.mode.as_str(), "zoo" | "newmobs" | "raid" | "golems" | "homestead") && frames > 150 {
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
                for (i, kind) in entity::MobKind::ALL.into_iter().filter(|k| *k != entity::MobKind::Wyrm).enumerate() {
                    let at = p + Vec3::new(2.4f32.sin() * dist + (i as f32 - 2.0) * 2.4, 2.0, -2.4f32.cos() * dist);
                    let m = entity::Mob::new(kind, at, &mut rng);
                    app.game.mobs.push(m);
                }
            }
        }
        app.frame();
        if std::mem::take(&mut app.shot_pending) {
            app.save_screenshot();
        }
        app.audio.poll().await;
        frames += 1;
        if let Some(s) = &shot
            && frames == s.frames {
                get_screen_data().export_png(&s.path);
                break;
            }
        if app.quit || is_quit_requested() {
            if !app.game.menu && !app.game.is_client() {
                app.save();
            }
            app.save_settings();
            app.game.disconnect();
            break;
        }
        // Max FPS: wait out the rest of this frame's share of a second.
        if shot.is_none() && app.settings.max_fps > 0 {
            let share = std::time::Duration::from_secs_f64(1.0 / app.settings.max_fps as f64);
            let due = last_frame + share;
            let now = std::time::Instant::now();
            if due > now {
                std::thread::sleep(due - now);
            }
            last_frame = due.max(now - share);
        } else {
            last_frame = std::time::Instant::now();
        }
        next_frame().await;
    }
}

/// The ladder/frame facing for a wall on side `d` of the cell.
fn building_facing(d: IVec3) -> block::Id {
    crate::decor::frame_facing(-d).unwrap_or(0) as block::Id
}
