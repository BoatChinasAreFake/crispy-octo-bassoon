//! MINCERAFT — a native, browser-free block game parody.
//! Rust + raw OpenGL (via miniquad/macroquad). No asset files: everything is
//! generated at startup (the Windows exe's icon, assets/minceraft.ico, is
//! generated too, by --export-icon).

mod game;
mod engine;
mod land;
mod blocks;
mod creatures;
mod survival;
mod online;

pub(crate) use engine::{access, backups, compose, keybinds, light, lod, mesher, noise, pacing, pad, palette, paths, regions, render, save, settings, sound, texture, tint, ui, updates, upnp};
pub(crate) use land::{archaeology, bastion, caveins, caves, copper, deepdark, dims, falling, fire, fortress, hollow, houses, liquids, monument, realms, scorch, seas, seasons, skies, structures, temples, treasure, trial, weather, wilds, world};
pub(crate) use blocks::{anvil, backpacks, banners, beacon, beds, block, books, boxes, carpentry, chests, containers, contraptions, crafting, decor, enchant, fireworks, home, homecraft, hoppers, masonry, models, music, potions, smithing, stash, trims, tripwire, wiring, woods};
pub(crate) use creatures::{animals, beasts, bees, creaking, critters, entity, floaty, horses, leads, nametags, night, pathing, raids, sniffers, villagers, wildlife, wilter};
pub(crate) use survival::{advancements, combat, drops, farming, fishing, gadgets, glider, hunger, inventory, modes, navigation, player, qol, rules, stats, tools, vehicles, xp};
pub(crate) use online::{admin, cheats, ledger, mods, multiplayer, net, players, playtest, scripting, server};
mod screens;
mod shots;
use shots::parse_args;

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
    /// Dragging the palette's scrollbar.
    palette_drag: bool,
    /// Creative inventory: which kind of thing the palette shows, and
    /// whether your own 27 slots are showing instead.
    creative_tab: u8,
    creative_backpack: bool,
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
    /// Which page of the pause menu is showing.
    pause_page: PausePage,
    /// Tab completion: the lines Tab can make of the command being typed, and which one it's on.
    chat_options: Option<(Vec<String>, usize)>,
    chat_scroll: usize,
    /// Where the chat log's scrollbar was last drawn (see screens/hud.rs).
    chat_bar: (f32, f32, f32, f32),
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
    /// World generation options for the next new world.
    form_gen: world::GenOptions,
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
    // Backspace (or Delete: there's no caret to delete after) takes off the last
    // character, and keeps going while it's held, like everywhere else.
    let held = is_key_down(KeyCode::Backspace) || is_key_down(KeyCode::Delete);
    let pressed = is_key_pressed(KeyCode::Backspace) || is_key_pressed(KeyCode::Delete);
    if erase_repeat(pressed, held, get_frame_time()) {
        buf.pop();
    }
}

thread_local! {
    /// How long Backspace has been held, and when it last repeated.
    static ERASE_HELD: std::cell::Cell<(f32, f32)> = const { std::cell::Cell::new((0.0, 0.0)) };
}

/// Should a held erase key erase now? Once on the press, then after a pause, quickly.
fn erase_repeat(pressed: bool, held: bool, dt: f32) -> bool {
    const DELAY: f32 = 0.45;
    const EVERY: f32 = 0.035;
    ERASE_HELD.with(|c| {
        if pressed {
            c.set((0.0, 0.0));
            return true;
        }
        if !held {
            c.set((0.0, 0.0));
            return false;
        }
        let (t, last) = c.get();
        let t = t + dt.min(0.1);
        if t >= DELAY && t - last.max(DELAY - EVERY) >= EVERY {
            c.set((t, t));
            true
        } else {
            c.set((t, last));
            false
        }
    })
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

/// The pause menu's pages.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum PausePage {
    Main,
    Logs,
    Share,
}

impl App {
    fn set_screen(&mut self, s: Screen) {
        if s == Screen::Playing {
            self.pause_page = PausePage::Main;
        }
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
            self.game.use_regions(dir);
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
        let mut g = Game::new_with(seed, creative, false, self.form_gen);
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
        if let Err(e) = self.game.flush_regions() {
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
        // (Not while typing in chat: the pointer is free then.)
        if self.screen == Screen::Playing
            && self.chat.is_none()
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
            // Tab finishes a command (or a player, item, mob... in it): as far as all the
            // choices agree, then once more steps through them.
            if is_key_pressed(KeyCode::Tab) {
                let stepping = self.chat_options.as_ref().filter(|(o, i)| o.len() > 1 && o.get(*i).is_some_and(|l| l == line || l.trim_end() == line.as_str()));
                if let Some((o, i)) = stepping {
                    let next = (i + 1) % o.len();
                    *line = o[next].clone();
                    self.chat_options = Some((o.clone(), next));
                } else {
                    let options = self.game.complete_command(line);
                    match options.len() {
                        0 => self.chat_options = None,
                        1 => {
                            *line = format!("{} ", options[0]);
                            self.chat_options = None;
                        }
                        _ => {
                            let common = options.iter().skip(1).fold(options[0].clone(), |acc, o| acc.chars().zip(o.chars()).take_while(|(a, b)| a.eq_ignore_ascii_case(b)).map(|(a, _)| a).collect());
                            if common.len() > line.len() {
                                *line = common;
                                self.chat_options = Some((options, usize::MAX));
                            } else {
                                *line = options[0].clone();
                                self.chat_options = Some((options, 0));
                            }
                        }
                    }
                }
            } else if self.chat_options.as_ref().is_some_and(|(o, i)| o.get(*i).is_none_or(|l| l != line) && !o.iter().any(|l| l.starts_with(line.as_str()))) {
                // Typed something else: the choices are stale.
                self.chat_options = None;
            }
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
            let most = self.game.chat_log.len().saturating_sub(screens::hud::CHAT_LINES);
            if is_key_pressed(KeyCode::PageUp) {
                self.chat_scroll = (self.chat_scroll + screens::hud::CHAT_LINES - 1).min(most);
            } else if is_key_pressed(KeyCode::PageDown) {
                self.chat_scroll = self.chat_scroll.saturating_sub(screens::hud::CHAT_LINES - 1);
            } else if wheel > 0.01 {
                self.chat_scroll = (self.chat_scroll + 3).min(most);
            } else if wheel < -0.01 {
                self.chat_scroll = self.chat_scroll.saturating_sub(3);
            }
            // Dragging the scrollbar.
            if is_mouse_button_down(MouseButton::Left) && most > 0 {
                let s = self.ui.s;
                let (bx, top, bw, bh) = self.chat_bar;
                let (mx, my) = mouse_position();
                if (bx - 6.0 * s..bx + bw + 6.0 * s).contains(&mx) && (top..top + bh).contains(&my) {
                    let up = 1.0 - (my - top) / bh;
                    self.chat_scroll = ((up * most as f32).round() as usize).min(most);
                }
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
                if (is_key_pressed(KeyCode::Escape) || self.pad_frame.back) && self.pause_page != PausePage::Main {
                    self.pause_page = PausePage::Main;
                } else if is_key_pressed(KeyCode::Escape) || self.pad_frame.pause || self.pad_frame.back {
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
        {
            let size = render::SHADOW_SIZES[self.settings.shadow_quality as usize % render::SHADOW_SIZES.len()].0;
            let gl = unsafe { get_internal_gl() };
            self.renderer.set_shadow_size(gl.quad_context, size);
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
            let Some(net::Msg::Welcome { id, seed, time, creative, spawn, keep_inventory, worldgen }) = msgs.pop() else { return };
            let (conn, _) = self.joining.take().unwrap();
            let mut g = Game::new_client(id, seed, worldgen, time, creative, spawn, conn, &self.mp_name, leftover);
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
            // Dyed words take the dye's colour; glowing ones show from further off (see qol.rs).
            let style = self.game.world.sign_styles.get(pos).copied().unwrap_or(0);
            let glow = style & qol::GLOW != 0;
            let [r, g, b] = qol::sign_colour(style);
            let ink = Color::from_rgba(r, g, b, 255);
            let at = pos.as_vec3() + Vec3::new(0.5, 0.78, 0.5);
            let d = at.distance(eye);
            if d > if glow { 32.0 } else { 16.0 } || lines.iter().all(|l| l.is_empty()) {
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
                let y = top + i as f32 * line_h;
                if glow {
                    // A soft halo of the same colour behind the words.
                    let halo = Color::from_rgba(r, g, b, 90);
                    for (ox, oy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
                        self.ui.text_centered(l, sx + ox * s, y + oy * s, size, halo);
                    }
                }
                self.ui.text_centered(l, sx, y, size, ink);
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
        self.game.smooth_far = self.settings.smooth_far;
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
    if let Some(t) = trims::describe_dye(wear).filter(|_| trims::is_woolly(id)) {
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
        palette_drag: false,
        creative_tab: 0,
        creative_backpack: false,
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
        pause_page: PausePage::Main,
        chat_options: None,
        chat_scroll: 0,
        chat_bar: (0.0, 0.0, 0.0, 0.0),
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
        form_gen: world::GenOptions::DEFAULT,
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
        // (Drivers can ignore what the window asked for; tell the context too.)
        pacing::init();
        pacing::set_vsync(saved.vsync);
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
        app.settings.smooth_far = flag("--smooth-far");
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
        app.shot_setup(s);
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
            app.shot_frame(s, frames);
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
        let now = std::time::Instant::now();
        match pacing::next_due(last_frame, if shot.is_none() { app.settings.max_fps } else { 0 }, now) {
            Some(due) => {
                pacing::wait_until(due);
                last_frame = due;
            }
            None => last_frame = now,
        }
        next_frame().await;
    }
}

/// The ladder/frame facing for a wall on side `d` of the cell.
fn building_facing(d: IVec3) -> block::Id {
    crate::decor::frame_facing(-d).unwrap_or(0) as block::Id
}

#[cfg(test)]
mod typing_tests {
    #[test]
    fn a_held_backspace_repeats_after_a_pause() {
        assert!(super::erase_repeat(true, true, 0.016), "once on the press");
        let mut erased = 0;
        // Held for a second at 60 fps: nothing for the first 0.45 s, then quickly.
        for _ in 0..60 {
            erased += super::erase_repeat(false, true, 1.0 / 60.0) as u32;
        }
        assert!((8..=20).contains(&erased), "{erased}");
        assert!(!super::erase_repeat(false, false, 0.016), "let go");
        assert!(!super::erase_repeat(false, true, 0.016), "held again: waits first");
    }
}
