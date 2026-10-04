//! Gameplay: the world plus everything living in it, and scene assembly.

use crate::advancements::{Advancement, Progress};
use crate::block::*;
use crate::enchant::{level, Enchant};
use crate::entity::*;
use crate::inventory::Inventory;
use crate::mesher::mesh_chunk;
use crate::multiplayer::{Net, Peer};
use crate::net::Msg;
use crate::noise::{hash2, Perlin, Rng};
use crate::player::{Input, Player, EYE, MAX_HEALTH};
use crate::render::{DynGeo, FrameParams, Pass, Renderer};
use crate::save::SaveData;
use crate::scripting::{Cmd, ScriptHost};
use rhai::{Dynamic, INT};
use crate::sound::{material, Sfx};
use crate::texture::*;
use crate::world::{Hit, World, CH};
use macroquad::math::{ivec3, IVec3, Mat4, Vec3, Vec4};
use macroquad::miniquad::RenderingBackend;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::f32::consts::{PI, TAU};

pub const DAY_SECONDS: f32 = 1200.0;
/// Lines of chat kept to scroll back through.
pub const CHAT_KEEP: usize = 500;

fn projectile_fan(center: Vec3, count: u8, spread: f32) -> Vec<Vec3> {
    let count = count.clamp(1, 5) as usize;
    let spread = if spread.is_finite() { spread.clamp(0.0, 45.0) } else { 0.0 };
    (0..count).map(|i| {
        let angle = if count == 1 { 0.0 } else { -spread * 0.5 + i as f32 * spread / (count - 1) as f32 };
        let (sin, cos) = angle.to_radians().sin_cos();
        Vec3::new(center.x * cos + center.z * sin, center.y, -center.x * sin + center.z * cos)
    }).collect()
}

/// Seconds a homing projectile lasts before it fizzles out.
pub const HOMING_LIFE: f32 = 5.0;
/// How far away a homing projectile can notice a player.
const HOMING_RANGE: f32 = 32.0;

/// Turn a homing projectile toward the nearest target ahead of it, by at most
/// its turn rate, keeping its speed. Targets behind it are ignored, so a shot
/// that overshoots doesn't loop back round.
fn steer(a: &mut Arrow, targets: &[Vec3], dt: f32) {
    let speed = a.vel.length();
    let dir = a.vel.normalize_or_zero();
    if speed <= 0.0 || dir == Vec3::ZERO {
        return;
    }
    let best = targets
        .iter()
        .map(|t| *t - a.pos)
        .filter(|d| d.length() < HOMING_RANGE && d.normalize_or_zero().dot(dir) > 0.0)
        .min_by(|x, y| x.length().total_cmp(&y.length()));
    let Some(want) = best.map(|d| d.normalize_or_zero()) else { return };
    let angle = dir.angle_between(want);
    let max = a.homing.to_radians() * dt;
    let new = if angle <= max {
        want
    } else {
        let axis = dir.cross(want).normalize_or_zero();
        if axis == Vec3::ZERO {
            return;
        }
        macroquad::math::Quat::from_axis_angle(axis, max) * dir
    };
    a.vel = new * speed;
}

#[derive(Default, Clone)]
pub struct Controls {
    pub input: Input,
    pub attack_held: bool,
    pub attack_pressed: bool,
    pub use_held: bool,
    pub use_pressed: bool,
    pub pick: bool,
    pub drop: bool,
    /// With `drop`: the whole stack (Ctrl+Q).
    pub drop_all: bool,
}

pub enum Target {
    Block(Hit),
    Mob(usize),
    /// A boat or minecart (see vehicles.rs).
    Vehicle(usize),
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
    /// Sand, gravel and anvils on their way down (see falling.rs).
    pub falling: Vec<crate::falling::FallingBlock>,
    /// Note blocks with Zappy power on them (they play once per pulse).
    pub powered_notes: HashSet<IVec3>,
    /// Fireballs in flight (see fortress.rs), and how often cages look around.
    pub fireballs: Vec<crate::fortress::Fireball>,
    pub cage_timer: f32,
    /// Trial Spawners' fights, and how often they look around (see trial.rs).
    pub trials: HashMap<IVec3, crate::trial::Trial>,
    pub trial_timer: f32,
    /// How often Creaking Hearts look around (see creaking.rs).
    pub creak_timer: f32,
    /// Regeneration's heartbeat.
    pub regen_clock: f32,
    /// Raids (see raids.rs): the one on now, who has Bad Omen or is a Hero (by player id, seconds left),
    /// the raid bar as shown (state, wave, waves, left, seconds to keep showing it), and timers.
    pub raid: Option<crate::raids::Raid>,
    pub omens: HashMap<u32, f32>,
    pub heroes: HashMap<u32, f32>,
    /// Joined players the host knows have Strength on (seconds left, amplifier).
    pub strong: HashMap<u32, (f32, u8)>,
    pub raid_hud: Option<(u8, u8, u8, u16, f32)>,
    pub raid_clock: f32,
    pub patrol_timer: f32,
    /// Seconds raiders still glow after a Bell rang.
    pub bell_glow: f32,
    /// Pointy Sticks in flight or stuck in things (owned by the host; clients mirror them).
    pub arrows: Vec<Arrow>,
    pub inv: Inventory,
    /// The local player gets things for free and can't be hurt (spectators too).
    pub creative: bool,
    /// The local player is spectating (see modes.rs).
    pub spectator: bool,
    /// What new players start in (the world's own mode).
    pub default_creative: bool,
    /// Fraction of a day, 0 = sunrise.
    pub time: f32,
    pub clock: f32,
    pub messages: Vec<(String, f32)>,
    pub breaking: Option<(IVec3, f32)>,
    pub rng: Rng,
    attack_cd: f32,
    pub(crate) use_cd: f32,
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
    /// What the player has done here (see stats.rs).
    pub stats: crate::stats::Stats,
    /// Seconds of gliding not yet charged to the Glider (see glider.rs).
    pub glide_wear: f32,
    /// Time owed to random block ticks (see copper.rs).
    pub random_tick_acc: f32,
    /// Seconds towards the next look for loose Pointy Rocks (see caves.rs).
    pub shake_acc: f32,
    /// What's packed inside broken Hollow Boxes, by number (see boxes.rs).
    pub boxes: HashMap<u16, crate::containers::Container>,
    /// Sound effects requested this frame (effect, world position if positional).
    pub sounds: Vec<(Sfx, Option<Vec3>)>,
    dig_tick: f32,
    step_dist: f32,
    was_in_water: bool,
    /// Seconds since we were last in water (bobbing at the surface doesn't splash).
    dry_for: f32,
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
    pub advancements: Progress,
    /// "Advancement Made!" toasts on screen: (advancement, seconds left).
    pub toasts: Vec<(&'static Advancement, f32)>,
    /// The Fishing Stick's bobber, when cast (see fishing.rs).
    pub bobber: Option<crate::fishing::Bobber>,
    pub fish_log: crate::fishing::FishLog,
    /// Seconds since the last farm tick (see farming.rs).
    pub farm_timer: f32,
    /// Joined players: keeping our inventory in step with the host's ledger (see ledger.rs).
    pub inv_sync: crate::ledger::InvSync,
    /// The chest or furnace whose screen is open (see containers.rs).
    pub open: Option<IVec3>,
    /// Where the world lives: which joined players have which container open.
    pub viewers: HashMap<IVec3, HashSet<u32>>,
    /// Containers whose contents changed since viewers were last told.
    pub dirty_containers: HashSet<IVec3>,
    /// Detector rails with a cart on (or just off) them: seconds until they switch off.
    pub detectors: HashMap<IVec3, f32>,
    pub container_sync_timer: f32,
    /// Items on the ground (see drops.rs).
    pub drops: Vec<crate::drops::ItemDrop>,
    pub next_drop_id: u32,
    pub drop_timer: f32,
    pub drop_sync: f32,
    pub drops_sent_empty: bool,
    /// Keep inventory, difficulty, daylight cycle (see rules.rs).
    pub rules: crate::rules::WorldRules,
    /// Experience points (see xp.rs), and the orbs floating around.
    pub xp: u32,
    pub orbs: Vec<crate::xp::XpOrb>,
    pub next_orb_id: u32,
    pub orb_sync: f32,
    pub orbs_sent_empty: bool,
    /// The anvil screen's inputs while it's open (see anvil.rs).
    pub anvil: Option<crate::anvil::AnvilUi>,
    /// The enchanting table screen's inputs while it's open (see enchant.rs).
    pub enchanting: Option<crate::enchant::EnchantUi>,
    /// How many times we've enchanted something (seeds the table's offers; the host's word when joined).
    pub enchant_count: u32,
    /// Joined players we remember, by name (see players.rs).
    pub saved_players: std::collections::BTreeMap<String, crate::players::PlayerRecord>,
    /// Allow-list and operators (see admin.rs).
    pub admin: crate::admin::Admin,
    /// Leaves withering away (seconds left), and the sapling growth clock (see trees.rs).
    pub decaying: Vec<(IVec3, f32)>,
    pub sapling_timer: f32,
    pub beacon_timer: f32,
    pub explore_timer: f32,
    /// Graphics options (from settings.txt; see render.rs).
    pub waving_leaves: bool,
    /// Clouds as thick blocks (Options: Clouds: Fancy) rather than a flat layer.
    pub fancy_clouds: bool,
    /// Video options (see settings.rs): clouds at all, distance fog, camera bob, particle level.
    pub clouds_on: bool,
    pub fog_on: bool,
    pub view_bobbing: bool,
    pub particle_level: u8,
    pub water_reflections: bool,
    /// Sun shadows, and deep water with caustics (Video Settings; see render.rs).
    pub shadows: bool,
    pub fancy_water: bool,
    /// Fire update clock (see fire.rs).
    pub fire_timer: f32,
    /// Potion effects on the local player (see potions.rs).
    pub effects: Vec<crate::potions::ActiveEffect>,
    /// Dispensers that were powered last time we looked (they fire on the change).
    pub dispensers_on: std::collections::HashSet<IVec3>,
    /// What each observer last saw in front of it, and pulses still going.
    pub observed: HashMap<IVec3, Id>,
    pub observer_pulses: HashMap<IVec3, f32>,
    /// Hopper clock (see hoppers.rs).
    pub hopper_timer: f32,
    /// The Galloper we're riding, and when we last told the host (see horses.rs).
    pub mounted: Option<u32>,
    /// Sitting behind a Camel's driver (just along for the ride).
    /// Which seat we're in (0 drives; see horses.rs).
    pub seat_no: u8,
    /// When fireflies were last let out (see nature.rs).
    pub firefly_acc: f32,
    /// Seconds since campfire smoke was last puffed (see home.rs).
    pub smoke_acc: f32,
    /// Seconds until a Wanderer might turn up (see villagers.rs).
    pub wanderer_timer: f32,
    /// Frost Walker: seconds since boots last looked for water, and frozen water melting back (see enchant.rs).
    pub frost_acc: f32,
    pub frosted: Vec<(IVec3, f32)>,
    /// The sky (see skies.rs): shooting stars, seconds of rainbow left, and whether it was raining.
    pub shooting: Vec<crate::skies::ShootingStar>,
    pub rainbow: f32,
    pub was_wet: bool,
    /// Seconds until the world border speaks up again (see qol.rs).
    pub border_note: f32,
    /// Distant terrain past the render distance (Video Settings; see lod.rs).
    pub distant_terrain: bool,
    /// Distant terrain as smooth rolling land rather than blocky terraces.
    pub smooth_far: bool,
    pub lod: crate::lod::Lod,
    /// Trades made with each Hmmer, by each player (see villagers.rs).
    pub regulars: HashMap<(u32, String), u16>,
    /// Days gone by (for the moon's phase; see skies.rs).
    pub day: u32,
    pub ride_sync: f32,
    /// The last hundred messages (see `msg`).
    pub chat_log: std::collections::VecDeque<String>,
    /// Names given to mobs with Name Tags, by mob id (see nametags.rs).
    pub mob_names: HashMap<u32, String>,
    /// How the local player looks (see nametags.rs).
    pub skin: u8,
    /// The mob a Name Tag is being written for (the name screen is open).
    pub naming: Option<u32>,
    /// Draw the world through the colour-blind filter (a setting; see access.rs).
    pub colour_blind: bool,
    pub report_timer: f32,
    /// Rain, snow, storms (see weather.rs).
    pub weather: crate::weather::WeatherState,
    /// Seconds since the last water and lava steps, and lava cells waiting for theirs (see liquids.rs).
    pub liquid_timers: [f32; 2],
    pub lava_waiting: HashSet<IVec3>,
    /// Seconds the local player stays on fire (lava).
    pub on_fire: f32,
    /// Zappy Dust (see wiring.rs): time since the last update, pressed
    /// buttons and plates with their time left, doors held open by power.
    pub zap_timer: f32,
    pub buttons: HashMap<IVec3, f32>,
    pub plates: HashMap<IVec3, f32>,
    pub powered_doors: HashSet<IVec3>,
    /// The Hmmer whose trades are on screen (see villagers.rs).
    pub trading: Option<u32>,
    /// Seconds since the last swing (weapons charge back up; see combat.rs), and a raised shield.
    pub since_attack: f32,
    pub blocking: bool,
    /// Portals (see scorch.rs): seconds spent standing in one, seconds before
    /// another trip, and whether we've stepped out since the last.
    pub portal_time: f32,
    pub portal_cooldown: f32,
    pub left_portal: bool,
    /// Which portal leads to which (by `scorch::portal_key`), both ways.
    pub portal_links: HashMap<IVec3, IVec3>,
    /// The sign we've just put up and are writing on (see decor.rs).
    pub editing_sign: Option<IVec3>,
    /// Boats and minecarts (see vehicles.rs), the one we're in, and time to the next sync.
    pub vehicles: Vec<crate::vehicles::Vehicle>,
    pub next_vehicle_id: u32,
    pub riding: Option<u32>,
    pub vehicle_sync: f32,
    /// Everything the player has held, for the recipe book (see crafting.rs),
    /// how many recipes that adds up to, and the "new recipes" note's time left.
    pub known: std::collections::BTreeSet<Id>,
    pub recipes_seen: usize,
    pub recipe_news: f32,
    /// The recipe pinned to the screen (index into `recipes()`).
    pub pinned: Option<usize>,
    learn_timer: f32,
    /// Right-clicked a crafting table: the app opens the inventory.
    pub at_table: bool,
    /// Beekeeping (see bees.rs): every colony by its nest or hive, the log,
    /// the hive clock, and the bees buzzing about for show.
    pub hives: HashMap<IVec3, crate::bees::Colony>,
    pub bee_log: crate::bees::BeeLog,
    pub hive_timer: f32,
    pub buzz: Vec<crate::bees::Buzz>,
    pub buzz_scan: f32,
    /// Archaeology (see archaeology.rs): the brushing in progress, the Field
    /// Journal, and a request to show it.
    pub dig: Option<crate::archaeology::Dig>,
    pub journal: crate::archaeology::Journal,
    pub open_journal: bool,
    /// The Deep Dark (see deepdark.rs): seconds of darkness on screen, sensors
    /// switched on (seconds left), the shriekers' cooldown, each player's
    /// warnings (count, seconds until forgotten), and the footstep clock.
    pub darkness: f32,
    pub sensors_on: HashMap<IVec3, f32>,
    pub shriek_cd: f32,
    pub warnings: HashMap<u32, (u8, f32)>,
    pub step_timer: f32,
    /// The grindstone or smithing table screen's inputs while it's open (see smithing.rs).
    pub bench: Option<crate::smithing::BenchUi>,
    /// Where the local player last died (for the Recovery Compass).
    pub last_death: Option<Vec3>,
    /// Places you've named (see qol.rs).
    pub waypoints: Vec<crate::qol::Waypoint>,
    /// Chest lids swinging open or shut (see chests.rs).
    pub lids: std::collections::HashMap<IVec3, crate::chests::Lid>,
    /// Asleep in a bed (see beds.rs).
    pub sleeping: Option<crate::beds::Sleep>,
    /// The Lodestone our compass points to (see gadgets.rs), and whether we're looking through a Spyglass.
    pub lodestone: Option<IVec3>,
    /// A joined player's view of their Bundles (the host's word), and the one just used.
    pub bundle_mirror: HashMap<u16, Vec<(Id, u8)>>,
    pub bundle_pending: Option<usize>,
    /// Written books' words and lecterns' books (see books.rs); the book open on screen,
    /// one we're waiting on the host for (tag, our writable slot, lectern), and one just written.
    pub books: HashMap<u16, crate::books::Book>,
    pub lecterns: HashMap<IVec3, (Id, u16)>,
    /// Banners that are up (design, facing), the Loom we're at, and our shield's banner (see banners.rs).
    pub banners: HashMap<IVec3, (u16, u8)>,
    pub loom: Option<IVec3>,
    pub shield_banner: u16,
    /// The held map's zoom (see `navigation::ZOOMS`), and the map colours of blocks (for maps in frames).
    pub map_zoom: u8,
    pub map_colors: Vec<[u8; 3]>,
    pub reading: Option<crate::books::BookView>,
    pub book_waiting: Option<(u16, Option<usize>, Option<IVec3>)>,
    pub book_pending: Option<usize>,
    pub spyglass: bool,
}

impl Game {
    pub fn new(seed: u32, creative: bool, menu: bool) -> Self {
        Game::new_with(seed, creative, menu, crate::world::GenOptions::LEGACY)
    }

    /// A game in a world made with these generation options.
    pub fn new_with(seed: u32, creative: bool, menu: bool, worldgen: crate::world::GenOptions) -> Self {
        let world = World::with_options(seed, worldgen);
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
            falling: Vec::new(),
            powered_notes: HashSet::new(),
            fireballs: Vec::new(),
            cage_timer: 0.0,
            trials: HashMap::new(),
            trial_timer: 0.0,
            creak_timer: 0.0,
            regen_clock: 0.0,
            raid: None,
            omens: HashMap::new(),
            heroes: HashMap::new(),
            strong: HashMap::new(),
            raid_hud: None,
            raid_clock: 0.0,
            patrol_timer: 900.0,
            bell_glow: 0.0,
            arrows: Vec::new(),
            inv,
            creative,
            spectator: false,
            default_creative: creative,
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
            stats: Default::default(),
            glide_wear: 0.0,
            boxes: HashMap::new(),
            random_tick_acc: 0.0,
            shake_acc: 0.0,
            sounds: Vec::new(),
            dig_tick: 0.0,
            step_dist: 0.0,
            was_in_water: false,
            dry_for: 10.0,
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
            advancements: Progress::default(),
            toasts: Vec::new(),
            bobber: None,
            fish_log: Default::default(),
            farm_timer: 0.0,
            inv_sync: Default::default(),
            open: None,
            viewers: HashMap::new(),
            dirty_containers: HashSet::new(),
            detectors: HashMap::new(),
            container_sync_timer: 0.0,
            drops: Vec::new(),
            next_drop_id: 0,
            drop_timer: 0.0,
            drop_sync: 0.0,
            drops_sent_empty: true,
            rules: Default::default(),
            xp: 0,
            orbs: Vec::new(),
            next_orb_id: 0,
            orb_sync: 0.0,
            orbs_sent_empty: true,
            anvil: None,
            enchanting: None,
            enchant_count: 0,
            saved_players: Default::default(),
            admin: Default::default(),
            decaying: Vec::new(),
            sapling_timer: 0.0,
            beacon_timer: 0.0,
            explore_timer: 0.0,
            waving_leaves: true,
            fancy_clouds: true,
            clouds_on: true,
            fog_on: true,
            view_bobbing: true,
            particle_level: 0,
            water_reflections: true,
            shadows: true,
            fancy_water: true,
            fire_timer: 0.0,
            effects: Vec::new(),
            dispensers_on: Default::default(),
            observed: HashMap::new(),
            observer_pulses: HashMap::new(),
            hopper_timer: 0.0,
            mounted: None,
            seat_no: 0,
            firefly_acc: 0.0,
            smoke_acc: 0.0,
            wanderer_timer: crate::villagers::WANDER_SECS / 4.0,
            frost_acc: 0.0,
            frosted: Vec::new(),
            shooting: Vec::new(),
            rainbow: 0.0,
            was_wet: false,
            border_note: 0.0,
            distant_terrain: true,
            smooth_far: false,
            lod: crate::lod::Lod::default(),
            regulars: HashMap::new(),
            day: 0,
            ride_sync: 0.0,
            chat_log: Default::default(),
            mob_names: HashMap::new(),
            skin: 0,
            naming: None,
            colour_blind: false,
            report_timer: 0.0,
            weather: Default::default(),
            liquid_timers: [0.0; 2],
            lava_waiting: HashSet::new(),
            on_fire: 0.0,
            zap_timer: 0.0,
            buttons: HashMap::new(),
            plates: HashMap::new(),
            powered_doors: HashSet::new(),
            trading: None,
            since_attack: 10.0,
            blocking: false,
            portal_time: 0.0,
            portal_cooldown: 0.0,
            left_portal: true,
            portal_links: HashMap::new(),
            editing_sign: None,
            vehicles: Vec::new(),
            next_vehicle_id: 0,
            riding: None,
            vehicle_sync: 0.0,
            known: Default::default(),
            recipes_seen: 0,
            recipe_news: 0.0,
            pinned: None,
            learn_timer: 0.0,
            at_table: false,
            hives: HashMap::new(),
            bee_log: Default::default(),
            hive_timer: 0.0,
            buzz: Vec::new(),
            buzz_scan: 0.0,
            dig: None,
            journal: Default::default(),
            open_journal: false,
            darkness: 0.0,
            sensors_on: HashMap::new(),
            shriek_cd: 0.0,
            warnings: HashMap::new(),
            step_timer: 0.0,
            bench: None,
            last_death: None,
            waypoints: Vec::new(),
            lids: Default::default(),
            sleeping: None,
            lodestone: None,
            bundle_mirror: HashMap::new(),
            bundle_pending: None,
            books: HashMap::new(),
            lecterns: HashMap::new(),
            banners: HashMap::new(),
            loom: None,
            shield_banner: 0,
            map_zoom: 0,
            map_colors: Vec::new(),
            reading: None,
            book_waiting: None,
            book_pending: None,
            spyglass: false,
        }
    }

    pub fn from_save(d: SaveData) -> Self {
        let extras = d.extras.clone();
        // How the world was generated (worlds from before there were options: as they always were).
        let worldgen = extras.iter().find(|(k, _)| k == "gen").and_then(|(_, b)| b.get(..4)).map(|b| crate::world::GenOptions::unpack(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))).unwrap_or(crate::world::GenOptions::LEGACY);
        let mut g = Game::new_with(d.seed, d.creative, false, worldgen);
        g.saved_script_vars = d.script_vars.clone();
        g.advancements = Progress::from_keys(&d.advancements);
        g.world.farm = crate::farming::decode(&d.farm);
        g.fish_log = crate::fishing::FishLog::decode(&d.fish_log);
        let remap = palette_remap(reg(), &d.palette);
        g.world.mods = d.mods;
        if let Some(remap) = &remap {
            for m in g.world.mods.values_mut() {
                for id in m.values_mut() {
                    *id = remap[*id as usize];
                }
            }
        }
        // After the edits: replaying them makes empty containers, these fill them.
        let wear_bytes = match d.version {
            0..=8 => 0,
            9 | 10 => 2,
            _ => 4,
        };
        let containers = crate::containers::decode(&d.containers, wear_bytes);
        for (p, mut c) in containers {
            if let Some(r) = &remap {
                for s in c.slots.iter_mut() {
                    *s = s.map(|(id, n)| (r[id as usize], n)).filter(|(id, _)| *id != AIR);
                }
            }
            g.world.containers.insert(p, c);
        }
        for (pos, item, n, age, wear) in crate::drops::decode(&d.drops, wear_bytes) {
            let item = remap.as_ref().map(|r| r[item as usize]).unwrap_or(item);
            g.spawn_drop(pos, item, n, wear, Vec3::ZERO, 0.0);
            if let Some(last) = g.drops.last_mut() {
                last.age = age;
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
        for (i, s) in d.slots.into_iter().take(41).enumerate() {
            let s = match (s, &remap) {
                (Some((id, n)), Some(r)) => Some((r[id as usize], n)).filter(|(id, _)| *id != AIR),
                (s, _) => s,
            };
            let wear = d.wear.get(i).copied().unwrap_or(0);
            let wear = s.map(|(id, _)| crate::inventory::sanitize_wear(id, wear)).unwrap_or(0);
            if i < 36 {
                g.inv.slots[i] = s;
                g.inv.wear[i] = wear;
            } else if i == 40 {
                // (The other hand comes last.)
                g.inv.offhand = s;
                g.inv.offhand_wear = wear;
            } else if s.is_some_and(|(id, _)| armor_of(id).map(|(slot, _)| slot) == Some(i - 36)) {
                g.inv.armor[i - 36] = s;
                g.inv.armor_wear[i - 36] = wear;
            }
        }
        g.player.hunger = crate::hunger::Hunger::new(d.food, d.saturation);
        g.xp = d.xp;
        g.saved_players = crate::players::decode(&d.players);
        g.weather.kind = crate::weather::Weather::from_index(d.weather);
        g.weather.timer = d.weather_timer;
        g.weather.strength = if g.weather.kind.wet() { 1.0 } else { 0.0 };
        if let Some(r) = &remap {
            for rec in g.saved_players.values_mut() {
                for s in rec.report.slots.iter_mut() {
                    s.0 = r[s.0 as usize];
                }
                for b in rec.bag.iter_mut() {
                    b.0 = r[b.0 as usize];
                }
                rec.bag.retain(|b| b.0 != AIR);
                for e in rec.enchanted.iter_mut() {
                    e.0 = r[e.0 as usize];
                }
                rec.enchanted.retain(|e| e.0 != AIR);
            }
        }
        g.rules = crate::rules::WorldRules { keep_inventory: d.keep_inventory, difficulty: crate::rules::Difficulty::from_index(d.difficulty), daylight_cycle: d.daylight_cycle, weather_cycle: d.weather_cycle, hardcore: d.hardcore, seasons: false, border: 0 };
        g.enchant_count = d.enchant_count;
        g.rules.hardcore = d.hardcore;
        g.stats = crate::stats::Stats::decode(&d.stats);
        g.boxes = crate::boxes::decode(&d.boxes);
        g.load_extras(&extras);
        g.set_mode(crate::modes::GameMode::from_index(d.mode));
        g.portal_links = crate::scorch::decode_links(&d.portals);
        let (signs, frames) = crate::decor::decode(&d.decor);
        g.world.signs = signs;
        for (p, (item, wear)) in frames {
            let item = remap.as_ref().map(|r| r[item as usize]).unwrap_or(item);
            if item != AIR {
                g.world.frames.insert(p, (item, wear));
            }
        }
        for (kind, pos, yaw, cargo) in crate::vehicles::decode(&d.vehicles) {
            let id = g.spawn_vehicle(kind, pos, yaw);
            if let (Some(mut c), Some(v)) = (cargo, g.vehicles.iter_mut().find(|v| v.id == id)) {
                if let Some(r) = &remap {
                    for s in c.slots.iter_mut() {
                        *s = s.map(|(id, n)| (r[id as usize], n)).filter(|(id, _)| *id != AIR);
                    }
                }
                if v.contents.as_ref().is_some_and(|have| have.slots.len() == c.slots.len()) {
                    v.contents = Some(c);
                }
            }
        }
        for (mut m, name) in crate::animals::decode_mobs_named(&d.mobs, &mut g.rng) {
            m.id = g.next_mob_id;
            g.next_mob_id += 1;
            if let Some(name) = name {
                g.mob_names.insert(m.id, name);
            }
            g.mobs.push(m);
        }
        // Llamas get their packs back (saved by seed; see wildlife.rs).
        if let Some((_, b)) = extras.iter().find(|(k, _)| k == "packs") {
            g.decode_packs(b);
        }
        g.msg("Welcome back. The world missed you (it's a HashMap, it can't feel).");
        g
    }

    // Takes &mut self because it must return the held cursor item to the inventory before snapshotting.
    #[allow(clippy::wrong_self_convention)]
    pub fn to_save(&mut self) -> SaveData {
        self.inv.return_cursor();
        SaveData {
            seed: self.world.seed(),
            creative: self.default_creative,
            time: self.time,
            pos: self.player.body.pos.to_array(),
            yaw: self.player.yaw,
            pitch: self.player.pitch,
            health: self.player.health,
            spawn: self.spawn.to_array(),
            // The four worn armour slots go after the 36 inventory slots.
            slots: self.inv.slots.iter().chain(self.inv.armor.iter()).chain(std::iter::once(&self.inv.offhand)).copied().collect(),
            // With region files, edits, soil, containers and decor are written there instead (`flush_regions`).
            mods: if self.world.regions.is_some() { HashMap::new() } else { self.world.mods.clone() },
            palette: mod_palette(reg()),
            script_vars: self.export_script_vars(),
            advancements: self.advancements.earned.clone(),
            farm: if self.world.regions.is_some() { Vec::new() } else { crate::farming::encode(&self.world.farm) },
            fish_log: self.fish_log.encode(),
            containers: if self.world.regions.is_some() { Vec::new() } else { crate::containers::encode(&self.world.containers) },
            drops: crate::drops::encode(&self.drops),
            wear: self.inv.wear.iter().chain(self.inv.armor_wear.iter()).chain(std::iter::once(&self.inv.offhand_wear)).copied().collect(),
            food: self.player.hunger.food,
            saturation: self.player.hunger.saturation,
            keep_inventory: self.rules.keep_inventory,
            difficulty: self.rules.difficulty.index(),
            daylight_cycle: self.rules.daylight_cycle,
            weather_cycle: self.rules.weather_cycle,
            xp: self.xp,
            players: crate::players::encode(&self.all_player_records()),
            weather: self.weather.kind.index(),
            weather_timer: self.weather.timer,
            enchant_count: self.enchant_count,
            mobs: crate::animals::encode_mobs(&self.mobs, &self.mob_names),
            portals: crate::scorch::encode_links(&self.portal_links),
            vehicles: crate::vehicles::encode(&self.vehicles),
            decor: if self.world.regions.is_some() { Vec::new() } else { crate::decor::encode(&self.world.signs, &self.world.frames) },
            mode: self.mode().index(),
            hardcore: self.rules.hardcore,
            stats: self.stats.encode(),
            boxes: crate::boxes::encode(&self.boxes),
            extras: self.save_extras(),
            version: crate::save::VERSION,
        }
    }

    /// The save's named sections (see save.rs): each module packs its own.
    fn save_extras(&self) -> Vec<(String, Vec<u8>)> {
        let mut v = vec![
            ("known".to_string(), crate::crafting::encode_known(&self.known).into_bytes()),
            ("hives".to_string(), crate::bees::encode(&self.hives)),
            ("books".to_string(), crate::books::encode(&self.books, &self.lecterns)),
            ("banners".to_string(), crate::banners::encode(&self.banners)),
            ("stashes".to_string(), crate::stash::encode(&self.world.stashes)),
            ("backpacks".to_string(), crate::backpacks::encode(&self.world.backpacks)),
            ("bee_log".to_string(), self.bee_log.encode()),
            ("journal".to_string(), self.journal.encode()),
            ("regulars".to_string(), crate::villagers::encode_regulars(&self.regulars)),
            ("day".to_string(), self.day.to_le_bytes().to_vec()),
            ("packs".to_string(), self.encode_packs()),
            ("sign_styles".to_string(), crate::qol::encode_styles(&self.world.sign_styles)),
            ("rules2".to_string(), [&[self.rules.seasons as u8][..], &self.rules.border.to_le_bytes()].concat()),
        ];
        if let Some(p) = self.pinned {
            v.push(("pinned".into(), (p as u32).to_le_bytes().to_vec()));
        }
        if self.world.generator.opts != crate::world::GenOptions::LEGACY {
            v.push(("gen".into(), self.world.generator.opts.pack().to_le_bytes().to_vec()));
        }
        if !self.waypoints.is_empty() {
            v.push(("waypoints".into(), crate::qol::encode(&self.waypoints)));
        }
        if let Some(d) = self.last_death {
            v.push(("last_death".into(), d.to_array().iter().flat_map(|f| f.to_le_bytes()).collect()));
        }
        if let Some(p) = self.lodestone {
            v.push(("lodestone".into(), p.to_array().iter().flat_map(|c| c.to_le_bytes()).collect()));
        }
        v
    }

    fn load_extras(&mut self, extras: &[(String, Vec<u8>)]) {
        let extra = |key: &str| extras.iter().find(|(k, _)| k == key).map(|(_, b)| b.as_slice());
        if let Some(b) = extra("known") {
            self.known = crate::crafting::decode_known(&String::from_utf8_lossy(b));
            self.recipes_seen = recipes().iter().filter(|r| crate::crafting::discovered(r, &self.known)).count();
        }
        if let Some(b) = extra("pinned").and_then(|b| b.get(..4)) {
            let p = u32::from_le_bytes(b.try_into().unwrap()) as usize;
            self.pinned = (p < recipes().len()).then_some(p);
        }
        if let Some(b) = extra("backpacks") {
            self.world.backpacks = crate::backpacks::decode(b);
        }
        if let Some(b) = extra("stashes") {
            self.world.stashes = crate::stash::decode(b);
        }
        if let Some(b) = extra("sign_styles") {
            self.world.sign_styles = crate::qol::decode_styles(b);
        }
        if let Some(b) = extra("rules2").filter(|b| b.len() >= 5) {
            self.rules.seasons = b[0] != 0;
            self.rules.border = u32::from_le_bytes([b[1], b[2], b[3], b[4]]);
        }
        if let Some(b) = extra("banners") {
            self.banners = crate::banners::decode(b);
        }
        if let Some(b) = extra("books") {
            (self.books, self.lecterns) = crate::books::decode(b);
        }
        if let Some(b) = extra("hives") {
            self.hives = crate::bees::decode(b);
        }
        if let Some(b) = extra("bee_log") {
            self.bee_log = crate::bees::BeeLog::decode(b);
        }
        if let Some(b) = extra("day").and_then(|b| b.get(..4)) {
            self.day = u32::from_le_bytes(b.try_into().unwrap());
        }
        if let Some(b) = extra("regulars") {
            self.regulars = crate::villagers::decode_regulars(b);
        }
        if let Some(b) = extra("journal") {
            self.journal = crate::archaeology::Journal::decode(b);
        }
        if let Some(b) = extra("waypoints") {
            self.waypoints = crate::qol::decode(b);
        }
        if let Some(b) = extra("last_death").filter(|b| b.len() >= 12) {
            let f = |o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());
            let p = Vec3::new(f(0), f(4), f(8));
            self.last_death = p.is_finite().then_some(p);
        }
        if let Some(b) = extra("lodestone").filter(|b| b.len() >= 12) {
            let c = |o: usize| i32::from_le_bytes(b[o..o + 4].try_into().unwrap());
            self.lodestone = Some(IVec3::new(c(0), c(4), c(8)));
        }
    }

    /// Earn an advancement (once per world): toast, fanfare, chat line.
    pub fn advance(&mut self, key: &str) {
        if self.menu || self.dedicated {
            return;
        }
        if let Some(a) = self.advancements.grant(key) {
            self.toasts.push((a, 5.0));
            self.sfx(Sfx::Fanfare, None);
            self.msg(format!("Advancement made: [{}]", a.title));
            if !self.is_client() {
                let me = self.player_name.clone();
                self.fire("on_advancement", vec![me.into(), key.to_string().into()]);
            }
        }
    }

    /// Advancements for going places: the new biomes, and villages.
    fn explore_advancements(&mut self, dt: f32) {
        self.explore_timer += dt;
        if self.explore_timer < 0.5 || self.menu || self.dedicated || !self.ready {
            return;
        }
        self.explore_timer = 0.0;
        use crate::world::Biome;
        let p = self.player.body.pos;
        let (x, z) = (p.x.floor() as i32, p.z.floor() as i32);
        let key = match self.world.generator.column(x, z).1 {
            Biome::Jungle => Some("welcome_to_the_jungle"),
            Biome::Swamp => Some("swamp_thing"),
            Biome::Badlands => Some("stripy"),
            Biome::Taiga => Some("needles"),
            _ => None,
        };
        if let Some(k) = key {
            self.advance(k);
        }
        let near_village = self.world.generator.villages_near(x.div_euclid(16), z.div_euclid(16)).iter().any(|v| v.origin.as_vec3().distance(p) < 24.0);
        if near_village {
            self.advance("village_people");
        }
        // Cave biomes, well under the ground (see caves.rs).
        if p.y < (self.world.generator.column(x, z).0 - 8) as f32 && p.y > 4.0 {
            match self.world.generator.cave_biome(x, z) {
                crate::caves::CaveBiome::Dripstone => self.advance("stalac_tight"),
                crate::caves::CaveBiome::Lush => self.advance("lush_life"),
                crate::caves::CaveBiome::Plain => {}
            }
        }
        // Temples, mineshafts and igloos (see temples.rs).
        use crate::structures::Kind;
        if let Some(kind) = self.world.generator.site_near(p, 14.0) {
            match kind {
                Kind::DesertPyramid => self.advance("pyramid_scheme"),
                Kind::JungleTemple => self.advance("temple_run"),
                Kind::Mineshaft => self.advance("off_the_rails"),
                Kind::Igloo => self.advance("cold_feet"),
                _ => {}
            }
        }
    }

    /// Advancements for getting hold of an item.
    pub fn item_advancements(&mut self, item: Id) {
        let key = match item {
            LOG | SPRUCE_LOG | JUNGLE_LOG | CHERRY_LOG | MANGROVE_LOG | PALE_OAK_LOG => "getting_wood",
            COBBLE => "stone_age",
            IRON => "iron_will",
            DIAMOND => "dimonds",
            GOLD_INGOT => "fools_gold",
            WOOL => "fluffed",
            FEATHER => "why_cross",
            MOO_STEAK => "udderly",
            PICK_WOOD | PICK_STONE | PICK_IRON | PICK_DIAMOND | PICK_COPPER => "tool_time",
            BAMBOO => "bamboozled",
            AMETHYST_SHARD => "crystal_clear",
            _ if (CORAL_FIRST..=DEAD_CORAL).contains(&item) => "reef_madness",
            _ => return,
        };
        self.advance(key);
    }

    /// Called by the crafting screen after a successful craft.
    pub fn on_crafted(&mut self, output: Id, via_gold: bool) {
        if via_gold {
            self.msg("Gold is too soft for tools, so you got a wooden one. Economics!");
        } else {
            self.msg(format!("Crafted {}. Nobody knows how.", item_name(output)));
        }
        self.sfx(Sfx::Craft, None);
        self.item_advancements(output);
    }

    pub fn msg(&mut self, s: impl Into<String>) {
        let s = s.into();
        if self.dedicated {
            println!("[{}] {s}", crate::server::timestamp());
        }
        // Everything said is kept a while, to scroll back through with chat open.
        self.chat_log.push_back(s.clone());
        if self.chat_log.len() > CHAT_KEEP {
            self.chat_log.pop_front();
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
        let clear = 0.18 + 0.82 * t * t * (3.0 - 2.0 * t);
        (clear * crate::weather::dimming(self.weather.kind, self.weather.strength)).max(0.18)
    }

    pub fn is_night(&self) -> bool {
        self.sun_angle().sin() < -0.05
    }

    /// Somewhere other than the ordinary world (no weather, its own sky)?
    pub fn elsewhere(&self) -> bool {
        self.in_scorch() || self.in_hollow()
    }

    /// Is the camera (or the local player) down in the Scorchlands?
    pub fn in_scorch(&self) -> bool {
        !self.menu && crate::scorch::in_scorch(self.player.body.pos.x)
    }

    pub fn sky_color(&self) -> [f32; 3] {
        if self.in_scorch() {
            return [0.24, 0.06, 0.03];
        }
        if self.in_hollow() {
            return [0.05, 0.02, 0.08];
        }
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
        // Rain clouds grey it all out.
        let grey = (c[0] + c[1] + c[2]) / 3.0;
        let k = self.weather.strength * 0.7;
        for v in c.iter_mut() {
            *v += (grey - *v) * k;
        }
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
        // The far-off land, kept up with where we are.
        if self.distant_terrain && !self.in_scorch() && !self.in_hollow() {
            let generator = self.world.generator.clone();
            if let Some(land) = self.lod.tick(&generator, &self.map_colors, center, radius, self.smooth_far) {
                renderer.set_far(ctx, Some(&land));
            }
        } else if renderer.has_far() {
            renderer.set_far(ctx, None);
            self.lod = crate::lod::Lod::default();
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
        } else if let Some(view) = self.sleep_view().filter(|_| !self.third_person) {
            // In bed: on the pillow, looking up.
            view
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
                let b = if self.view_bobbing { self.player.bob } else { 0.0 };
                let bob = Vec3::new(0.0, (b * 2.0).sin().abs() * 0.06, 0.0);
                (eye + bob, dir)
            }
        };
        let fov = if self.spyglassing() {
            crate::gadgets::SPYGLASS_FOV
        } else if !self.menu && self.player.sprinting {
            fov_deg + 8.0
        } else {
            fov_deg
        };
        let proj = Mat4::perspective_rh_gl(fov.to_radians(), aspect, 0.05, 1000.0);
        let view = Mat4::look_at_rh(pos, pos + dir, Vec3::Y);
        Camera { pos, dir, view_proj: proj * view }
    }

    pub fn update(&mut self, dt: f32, c: &Controls) {
        // Spectators move and look, and touch nothing.
        let hands_off;
        let c = if self.spectator {
            hands_off = Controls { input: c.input.clone(), ..Default::default() };
            &hands_off
        } else {
            c
        };
        self.net_receive(dt);
        self.update_local(dt, c);
        self.net_send(dt);
    }

    fn update_local(&mut self, dt: f32, c: &Controls) {
        self.clock += dt;
        if self.rules.daylight_cycle {
            let before = self.time;
            self.time = (self.time + dt / DAY_SECONDS) % 1.0;
            self.count_days(before);
        }
        self.weather_tick(dt);
        self.liquid_tick(dt);
        self.zap_tick(dt);
        self.deep_dark_tick(dt);
        self.buzz_tick(dt);
        self.shake = (self.shake - dt * 1.5).max(0.0);
        self.effects_tick(dt);
        self.explore_advancements(dt);
        self.held_name = (self.held_name - dt).max(0.0);
        for m in self.messages.iter_mut() {
            m.1 -= dt;
        }
        self.messages.retain(|m| m.1 > 0.0);
        if let Some(t) = self.toasts.first_mut() {
            // One at a time, like a polite queue.
            t.1 -= dt;
        }
        self.toasts.retain(|t| t.1 > 0.0);
        if self.menu || !self.ready || self.dead.is_some() {
            return;
        }
        let p = self.player.body.pos;
        if !self.world.is_loaded(p.x.floor() as i32, p.z.floor() as i32) {
            return;
        }

        if self.inv.armor_points() > 0 {
            self.advance("suit_up");
            if self.inv.armor.iter().all(|s| s.and_then(|(id, _)| armor_of(id)).is_some_and(|(_, tier)| tier == 3)) {
                self.advance("cover_me");
            }
        }
        // In a boat or cart, the vehicle moves us (see vehicles.rs).
        let before = self.player.body.pos;
        self.player.jumped = false;
        self.player.glider_on = self.wearing_glider();
        let mut fall = if self.sleeping.is_some() {
            // In bed: no walking about (jumping or sneaking gets you up).
            self.sleep_tick(dt, c.input.jump_pressed || c.input.sneak);
            0.0
        } else if self.riding.is_none() && self.mounted.is_none() {
            self.player.update(dt, &c.input, &self.world, self.creative)
        } else {
            0.0
        };
        if self.player.jumped {
            self.stats.jumps += 1;
        }
        self.vehicles_tick(dt, c.input.forward, c.input.strafe, c.input.sneak);
        self.ride_tick(dt, c.input.forward, c.input.strafe, c.input.jump, c.input.sneak);
        self.track_travel(before, dt);
        self.glider_tick(dt);
        // Flowing water carries you along.
        if self.player.body.in_water && !self.player.flying {
            let push = crate::liquids::current(&self.world, self.player.body.pos + Vec3::Y * 0.3);
            self.player.body.vel += push * dt * if self.player.body.in_lava { 2.0 } else { 9.0 };
        }
        self.hunger_tick(dt);
        self.breath_tick(dt);
        self.lids_tick(dt);
        let landed = self.player.landed.take();
        let feet = self.player.body.pos - Vec3::Y * 0.05;
        let under = ivec3(feet.x.floor() as i32, feet.y.floor() as i32, feet.z.floor() as i32);
        if let Some(height) = landed {
            if self.world.get_v(under) == HAY {
                // Hay bales: nature's crash mat.
                fall = (fall * 0.2).floor();
                if height > 5.0 {
                    self.advance("hay_there");
                }
            }
            // Landing on an upright Pointy Rock: twice the fall damage (and then some).
            let spike = [under, under + IVec3::Y].into_iter().any(|p| self.world.get_v(p) == POINTY_ROCK && !crate::caves::hangs(|y| self.world.get(p.x, y, p.z), p.y));
            if spike && height >= 2.0 {
                fall = fall * 2.0 + 2.0;
            }
            self.trample(under, height);
        }
        if fall > 0.0 {
            self.sfx(Sfx::Thud, None);
            self.hurt_player(fall, "hit the ground too hard (the ground is fine)");
        }
        self.footsteps(dt);
        self.block_effects();
        self.lava_tick(dt);
        if !self.hollow_portal_tick() {
            self.portal_tick(dt);
        }
        if self.player.body.pos.y < -30.0 {
            self.hurt_player(100.0, "fell out of the world. Classic.");
        }

        self.update_target();
        self.attack_cd = (self.attack_cd - dt).max(0.0);
        self.since_attack += dt;
        // Shields go up while right-click is held.
        // A shield blocks from either hand (from the other one, while this one holds a weapon or tool, or nothing).
        self.blocking = c.use_held && (self.inv.held() == SHIELD || (self.inv.offhand.map(|s| s.0) == Some(SHIELD) && offhand_first(self.inv.held())));
        let looking = c.use_held && self.inv.held() == SPYGLASS;
        if looking && !self.spyglass {
            self.advance("bird_plane");
        }
        self.spyglass = looking;
        self.player.blocking = self.blocking;
        self.use_cd = (self.use_cd - dt).max(0.0);
        self.handle_actions(dt, c);
        self.update_fishing(dt, c.use_held);
        self.update_brushing(dt, c.use_held);
        self.inventory_sync_tick(dt);
        self.learn_timer -= dt;
        if self.learn_timer <= 0.0 {
            self.learn_timer = 0.5;
            self.learn_inventory();
        }
        self.recipe_news = (self.recipe_news - dt).max(0.0);
        self.report_tick(dt);
        self.campfire_smoke(dt);
        self.skies_tick(dt);
        self.border_tick(dt);
        self.frost_walk(dt);
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

    pub(crate) fn apply_cmds(&mut self, cmds: Vec<Cmd>) {
        let effect = |heal: f32, teleport: Option<Vec3>, launch: Option<f32>, take: Option<(Id, u8)>| Msg::Effect { heal, teleport, launch, take };
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
                        self.give_peer(id, item, n);
                    }
                }
                Cmd::Take(p, item, n) => {
                    if self.is_local_player(&p) {
                        self.inv.remove(item, n as u32);
                    } else if let Some(id) = self.peer_by_name(&p) {
                        self.take_peer(id, item, n);
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
                    if let Some(kind) = MobKind::from_index(k) {
                        self.alloc_mob(kind, at);
                    }
                }
                Cmd::SetTime(t) => {
                    self.time = t;
                    self.net_broadcast(self.time_msg());
                }
                Cmd::Sound(s, at) => {
                    self.sfx(s, Some(at));
                    self.net_broadcast(Msg::Sound { sfx: s.to_wire(), at });
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
        // A splash for jumping or falling in, not for every bob at the surface.
        if in_water && !self.was_in_water && self.dry_for > 1.0 {
            self.sfx(Sfx::Splash, None);
        }
        self.dry_for = if in_water { 0.0 } else { self.dry_for + dt };
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

    /// What the blocks around the player do to them: bouncing, pokey plants, ice.
    fn block_effects(&mut self) {
        if self.player.bounced {
            self.player.bounced = false;
            self.sfx(Sfx::Boing, None);
            self.advance("boing");
        }
        let b = &self.player.body;
        let (min, max) = (b.min() - Vec3::new(0.06, 0.0, 0.06), b.max() + Vec3::new(0.06, 0.0, 0.06));
        let mut pokey = false;
        for y in min.y.floor() as i32..=(max.y - 0.01).floor() as i32 {
            for z in min.z.floor() as i32..=max.z.floor() as i32 {
                for x in min.x.floor() as i32..=max.x.floor() as i32 {
                    pokey |= self.world.get(x, y, z) == CACTUS;
                }
            }
        }
        // Standing on top of one counts too.
        let feet = b.pos - Vec3::Y * 0.05;
        let under = self.world.get(feet.x.floor() as i32, feet.y.floor() as i32, feet.z.floor() as i32);
        pokey |= under == CACTUS && b.on_ground;
        let zoom = under == ICE && b.on_ground && self.player.sprinting;
        if pokey && self.player.hurt <= 0.0 && !self.creative {
            self.hurt_player(1.0, "hugged a Pokey Plant. We said not to.");
            self.advance("ouch");
        }
        if zoom {
            self.advance("zoomies");
        }
    }

    /// Right-clicking a bed.
    fn sleep(&mut self, bed: IVec3) {
        if self.elsewhere() {
            // Beds don't like it out here.
            self.world.set_v(bed, AIR);
            self.msg(if self.in_scorch() { "The bed exploded. Beds are not rated for the Scorchlands." } else { "The bed exploded. There's no night to skip out here." });
            if !self.is_client() {
                self.explode(bed.as_vec3() + Vec3::splat(0.5), 3.0, "tried to sleep in the Scorchlands");
            }
            return;
        }
        self.spawn = bed.as_vec3() + Vec3::new(0.5, 1.0, 0.5);
        if !self.is_night() {
            self.msg("You can only sleep at night. Naps are a premium feature. (Spawn point set, though.)");
            return;
        }
        let me = self.player.body.pos;
        if self.mobs.iter().any(|m| m.menacing() && m.body.pos.distance(me) < 10.0) {
            self.msg("You may not rest now, there are monsters nearby. They're very loud sleepers.");
            return;
        }
        self.lie_down(bed);
    }

    /// The night is over (the sleeper stayed in bed long enough; see beds.rs).
    pub fn finish_night(&mut self) {
        let me = self.player.body.pos;
        self.time = 0.01;
        // Sleeping through the night clears the weather too.
        if self.weather.kind.wet() {
            self.set_weather(crate::weather::Weather::Clear);
        }
        self.net_broadcast(self.time_msg());
        self.mobs.retain(|m| !m.menacing() || m.body.pos.distance(me) > 64.0);
        self.msg("You slept like a log (a Tree Chunk). Good morning! Spawn point set. Your back hurts.");
        self.advance("sweet_dreams");
    }

    /// Throw a Stare Pearl: blink to wherever you're looking.
    fn throw_pearl(&mut self) {
        let eye = self.player.eye();
        let dir = self.player.look_dir();
        let to = match self.world.raycast(eye, dir, 48.0) {
            Some(h) => {
                let cell = h.pos + h.normal;
                // Find standing room at (or just above) the spot we hit.
                let spot = (0..3).map(|dy| cell + IVec3::Y * dy).find(|c| !is_solid(self.world.get_v(*c)) && !is_solid(self.world.get_v(*c + IVec3::Y)));
                match spot {
                    Some(c) => c.as_vec3() + Vec3::new(0.5, 0.0, 0.5),
                    None => {
                        self.msg("The pearl hit a wall and had a think about it.");
                        return;
                    }
                }
            }
            None => eye + dir * 48.0 - Vec3::Y * EYE,
        };
        let from = self.player.body.pos;
        self.smoke(from + Vec3::Y, 12, 0.4);
        self.player.body.pos = to;
        self.player.body.vel = Vec3::ZERO;
        self.player.fall_start = to.y;
        self.smoke(to + Vec3::Y, 12, 0.4);
        self.sfx(Sfx::Warp, None);
        self.player.hurt = 0.0;
        self.hurt_player(2.0, "teleported directly into a bad decision");
        self.advance("rude_teleport");
        if !self.creative {
            self.use_up_held();
        }
    }

    /// Loose a Pointy Stick from the bow (the host owns arrows, so clients ask it to).
    fn shoot_bow(&mut self) {
        if !self.creative && self.inv.count(ARROW) == 0 {
            self.msg("No Pointy Sticks. The bow twangs sadly at nothing.");
            return;
        }
        if !self.creative {
            self.inv.remove(ARROW, 1);
        }
        self.use_tool(1);
        let dir = self.player.look_dir();
        let pos = self.player.eye() + dir * 0.5;
        self.player.swing = 1.0;
        self.use_cd = 0.6;
        if self.is_client() {
            self.net_send_msg(Msg::Shoot { pos, dir });
            self.sfx(Sfx::Twang, None);
        } else {
            let me = self.my_id;
            self.spawn_arrow(pos, dir * Arrow::SPEED * 1.2, Some(me));
        }
    }

    /// Throw the held spear (the host owns thrown things, so clients ask it to).
    fn throw_spear(&mut self, item: Id) {
        let wear = self.inv.wear[self.inv.selected];
        // Riptide: in water or rain, the spear takes you with it instead.
        if self.riptide(wear) {
            return;
        }
        self.player.swing = 1.0;
        self.use_cd = 0.8;
        self.advance("spear_it");
        if self.is_client() {
            // The host takes it from its ledger and throws it for us.
            self.net_send_msg(Msg::UseItem { item });
            self.inv.consume_held();
            return;
        }
        if !self.creative {
            self.inv.consume_held();
        }
        let dir = self.player.look_dir();
        let me = self.my_id;
        self.throw_spear_from(self.player.eye() + dir * 0.5, dir, me, item, crate::inventory::with_uses(wear, crate::inventory::uses(wear).saturating_add(1)));
    }

    /// Launch a spear (host side).
    pub fn throw_spear_from(&mut self, pos: Vec3, dir: Vec3, shooter: u32, item: Id, wear: crate::inventory::Wear) {
        // Thrown, it hits about as hard as it stabs.
        let damage = if item == SPEAR { 8.0 } else { attack_damage_with(item, 0) + 1.0 };
        let mut a = Arrow::new(pos, dir * Arrow::SPEED * 1.1, Some(shooter), damage);
        a.spear = Some((item, wear));
        self.arrows.push(a);
        self.sfx(Sfx::Twang, Some(pos));
    }

    /// Fire an arrow: from a player's bow (`shooter` = their id) or a Rattler (None).
    pub fn spawn_arrow(&mut self, pos: Vec3, vel: Vec3, shooter: Option<u32>) {
        let damage = if shooter.is_some() { 5.0 } else { 3.0 };
        self.arrows.push(Arrow::new(pos, vel, shooter, damage));
        self.sfx(Sfx::Twang, Some(pos));
        if self.arrows.len() > 200 {
            self.arrows.remove(0);
        }
    }

    /// Fire one deterministic, data-defined projectile volley for a modded mob.
    /// Every projectile shares the center shot's vertical lob and payload.
    pub fn spawn_mod_projectiles(&mut self, pos: Vec3, center_vel: Vec3, spec: ModProjectileSpec) {
        for vel in projectile_fan(center_vel, spec.count, spec.spread) {
            let mut arrow = Arrow::new(pos, vel, None, spec.damage);
            arrow.modded = true;
            arrow.appearance = spec.appearance;
            arrow.effect = spec.effect;
            arrow.homing = spec.homing.clamp(0.0, 180.0);
            arrow.blast = spec.blast.clamp(0.0, 4.0);
            if arrow.homing > 0.0 {
                // A seeker that misses fizzles out rather than circling forever.
                arrow.life = arrow.life.min(HOMING_LIFE);
            }
            self.arrows.push(arrow);
        }
        self.sfx(Sfx::Twang, Some(pos));
        let excess = self.arrows.len().saturating_sub(200);
        self.arrows.drain(..excess);
    }

    /// Where a homing projectile may aim: the chests of every player it could hurt.
    fn homing_targets(&self) -> Vec<Vec3> {
        let mut v = Vec::new();
        if !self.dedicated && !self.spectator && self.dead.is_none() {
            v.push(self.player.body.pos + Vec3::Y * 1.0);
        }
        v.extend(self.peers.values().filter(|p| p.alive() && p.mode != crate::modes::GameMode::Spectator).map(|p| p.target + Vec3::Y * 1.0));
        v
    }

    /// A modded blast projectile goes off: everyone within `r` takes up to
    /// `damage` (half at the edge) and the shot's effect, and is pushed away.
    /// Mobs and blocks are left alone. Host side.
    fn projectile_blast(&mut self, at: Vec3, r: f32, damage: f32, effect: Option<crate::block::ProjectileEffect>) {
        let r = r.clamp(0.5, 4.0);
        let cause = "was blown up by a monster";
        self.sfx(Sfx::Explode, Some(at));
        self.explosion_effects(at, r);
        self.net_broadcast(Msg::Explosion { at, r });
        let share = |d: f32| 1.0 - 0.5 * (d / r).min(1.0);
        let me = self.player.body.pos + Vec3::Y * 0.9;
        if !self.dedicated && !self.spectator && self.dead.is_none() && me.distance(at) <= r {
            let d = me.distance(at);
            if damage > 0.0 {
                self.player.hurt = 0.0;
                let dmg = self.rules.difficulty.mob_damage(damage * share(d));
                self.hurt_player_from(dmg, cause, Some(at), false);
                let knock = self.steadied((me - at).normalize_or(Vec3::Y) * 5.0);
                self.player.body.vel += knock;
            }
            if let Some(e) = effect {
                self.timed_effect_amplified(e.kind, e.duration, e.amplifier);
            }
        }
        let hit: Vec<(u32, f32, Vec3)> = self
            .peers
            .iter()
            .filter(|(_, p)| p.alive() && p.mode != crate::modes::GameMode::Spectator)
            .map(|(&id, p)| (id, (p.target + Vec3::Y * 0.9).distance(at), p.target + Vec3::Y * 0.9))
            .filter(|&(_, d, _)| d <= r)
            .collect();
        for (id, d, pos) in hit {
            if damage > 0.0 {
                let dmg = self.rules.difficulty.mob_damage(damage * share(d));
                self.hurt_peer(id, dmg, cause, (pos - at).normalize_or(Vec3::Y) * 5.0);
            }
            if let Some(e) = effect {
                self.send_timed_effect(id, e.kind, e.duration, e.amplifier);
            }
        }
    }

    /// Move arrows and see what they hit (host / single player only).
    pub(crate) fn update_arrows(&mut self, dt: f32) {
        let mut arrows = std::mem::take(&mut self.arrows);
        let mut landed = Vec::new();
        let mut blasts: Vec<(Vec3, f32, f32, Option<crate::block::ProjectileEffect>)> = Vec::new();
        let mut winds: Vec<(Vec3, Option<u32>)> = Vec::new();
        let mut bursts: Vec<(Vec3, u8, f32, Option<u32>)> = Vec::new();
        let targets = if arrows.iter().any(|a| a.homing > 0.0) { self.homing_targets() } else { Vec::new() };
        arrows.retain_mut(|a| {
            if a.homing > 0.0 && !a.stuck {
                steer(a, &targets, dt);
            }
            if a.firework > 0 && a.damage <= 0.0 {
                // Off the ground: it speeds up as it climbs.
                a.vel.y += 12.0 * dt;
            }
            let thunk = a.fly(dt, &self.world);
            if a.firework > 0 {
                // A crossbow's rocket goes off on the first thing it touches.
                let near = |c: Vec3, r: f32| a.pos.distance(c) < r;
                let touched = a.damage > 0.0
                    && (self.mobs.iter().any(|m| near(m.body.pos + Vec3::Y * m.body.height * 0.5, m.body.half + m.body.height * 0.5 + 0.2))
                        || (Some(self.my_id) != a.shooter && !self.dedicated && !self.spectator && self.dead.is_none() && near(self.player.body.pos + Vec3::Y * 0.9, 1.1))
                        || self.peers.iter().any(|(id, p)| Some(*id) != a.shooter && p.alive() && near(p.target + Vec3::Y * 0.9, 1.1)));
                if thunk || touched || a.life <= 0.0 {
                    bursts.push((a.pos - a.dir * 0.3, a.firework - 1, a.damage, a.shooter));
                    return false;
                }
                return true;
            }
            if a.wind {
                // A wind charge bursts on the first thing it meets: a wall, or
                // a mob (a player's) or a player (a Breeze's).
                let near = |c: Vec3, r: f32| a.pos.distance(c) < r;
                let touched = match a.shooter {
                    Some(_) => self.mobs.iter().any(|m| m.kind != MobKind::Breeze && near(m.body.pos + Vec3::Y * m.body.height * 0.5, m.body.half + m.body.height * 0.5 + 0.2)),
                    None => (!self.dedicated && !self.spectator && self.dead.is_none() && near(self.player.body.pos + Vec3::Y * 0.9, 1.1)) || self.peers.values().any(|p| p.alive() && near(p.target + Vec3::Y * 0.9, 1.1)),
                };
                if thunk || touched || a.life <= 0.0 {
                    winds.push((a.pos - a.dir * 0.3, a.shooter));
                    return false;
                }
                return true;
            }
            // A blast shot goes off where it lands (or fizzles at the end of its life).
            if a.blast > 0.0 && (thunk || a.life <= 0.0) {
                blasts.push((a.pos - a.dir * 0.3, a.blast, a.damage, a.effect));
                return false;
            }
            if thunk {
                self.sfx(Sfx::Thunk, Some(a.pos));
            }
            // A spear that hits the ground drops, ready to be picked up.
            if let Some(spear) = a.spear
                && (a.stuck || a.life <= 0.0)
            {
                landed.push((a.pos - a.dir * 0.3, spear));
                return false;
            }
            if a.life <= 0.0 {
                return false;
            }
            if a.stuck {
                return true;
            }
            let hit_box = |min: Vec3, max: Vec3| {
                let e = Vec3::splat(0.15);
                let p = a.pos;
                p.cmpge(min - e).all() && p.cmple(max + e).all()
            };
            match a.shooter {
                Some(pid) => {
                    // Players' arrows hit mobs.
                    let Some(m) = self.mobs.iter_mut().find(|m| hit_box(m.body.min(), m.body.max())) else { return true };
                    m.hurt = 0.0;
                    m.damage(a.damage, a.pos - a.vel);
                    m.last_attacker = pid;
                    let (kind, at) = (m.kind, m.body.pos);
                    self.sfx(Sfx::hurt_of(kind), Some(at));
                    if let Some(spear) = a.spear {
                        landed.push((a.pos - a.dir * 0.5, spear));
                    } else if pid == self.my_id && !self.dedicated {
                        self.advance("robin_hood");
                    }
                    false
                }
                None => {
                    // Rattlers' (and modded mobs') arrows hit players. A modded
                    // mob's shot gets a generic cause so its kill isn't blamed
                    // on a Rattler.
                    let cause = if a.modded { "was shot down by a monster" } else { "was shot by a Rattler (with a Pointy Stick)" };
                    let me = self.player.body.clone();
                    if a.blast > 0.0 {
                        // A blast shot goes off on whoever it touches.
                        let touched = (!self.dedicated && !self.spectator && self.dead.is_none() && hit_box(me.min(), me.max()))
                            || self.peers.values().any(|p| p.alive() && hit_box(p.target - Vec3::new(0.3, 0.0, 0.3), p.target + Vec3::new(0.3, 1.8, 0.3)));
                        if touched {
                            blasts.push((a.pos, a.blast, a.damage, a.effect));
                            return false;
                        }
                        return true;
                    }
                    if !self.dedicated && !self.spectator && self.dead.is_none() && hit_box(me.min(), me.max()) {
                        if a.damage > 0.0 {
                            self.player.hurt = 0.0;
                            let d = self.rules.difficulty.mob_damage(a.damage);
                            self.hurt_player_from(d, cause, Some(a.pos - a.vel.normalize_or_zero() * 2.0), false);
                            let knock = self.steadied(a.vel.normalize_or_zero() * 4.0);
                            self.player.body.vel += knock;
                        }
                        if let Some(effect) = a.effect {
                            self.timed_effect_amplified(effect.kind, effect.duration, effect.amplifier);
                        }
                        return false;
                    }
                    let hit = self.peers.iter().find(|(_, p)| p.alive() && hit_box(p.target - Vec3::new(0.3, 0.0, 0.3), p.target + Vec3::new(0.3, 1.8, 0.3))).map(|(&id, _)| id);
                    if let Some(id) = hit {
                        if a.damage > 0.0 {
                            let d = self.rules.difficulty.mob_damage(a.damage);
                            self.hurt_peer(id, d, cause, a.vel.normalize_or_zero() * 4.0);
                        }
                        if let Some(effect) = a.effect {
                            self.send_timed_effect(id, effect.kind, effect.duration, effect.amplifier);
                        }
                        return false;
                    }
                    true
                }
            }
        });
        // Anything fired while we were busy (none today, but keep them).
        arrows.append(&mut self.arrows);
        self.arrows = arrows;
        for (at, r, damage, effect) in blasts {
            self.projectile_blast(at, r, damage, effect);
        }
        for (at, shooter) in winds {
            self.wind_burst(at, shooter);
        }
        for (at, colour, damage, shooter) in bursts {
            self.firework_burst(at, colour, damage, shooter);
        }
        for (at, (item, wear)) in landed {
            self.spawn_drop(at, item, 1, wear, Vec3::ZERO, 0.5);
        }
    }

    /// Placing a sponge soaks up water around it.
    fn soak(&mut self, at: IVec3) {
        let mut n = 0;
        for dy in -3..=3 {
            for dz in -3..=3 {
                for dx in -3..=3 {
                    let p = at + ivec3(dx, dy, dz);
                    if is_water(self.world.get_v(p)) {
                        self.world.set_v(p, AIR);
                        n += 1;
                    }
                }
            }
        }
        if n > 0 {
            self.sfx(Sfx::Splash, Some(at.as_vec3()));
            self.msg(format!("Glug. The sponge drank {n} blocks of water. It's still thirsty."));
            self.advance("thirsty");
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
            if let Some(t) = ray_aabb(eye, dir, m.body.min(), m.body.max())
                && t < reach.min(block_dist) && best.map(|b| t < b.1).unwrap_or(true) {
                    best = Some((i, t));
                }
        }
        let mut ride: Option<(usize, f32)> = None;
        for (i, v) in self.vehicles.iter().enumerate() {
            let (min, max) = v.bounds();
            if let Some(t) = ray_aabb(eye, dir, min, max)
                && t < reach.min(block_dist)
                && best.is_none_or(|b| t < b.1)
                && ride.is_none_or(|r| t < r.1)
                && Some(v.id) != self.riding
            {
                ride = Some((i, t));
            }
        }
        self.target = match (ride, best, hit) {
            (Some((i, _)), _, _) => Some(Target::Vehicle(i)),
            (None, Some((i, _)), _) => Some(Target::Mob(i)),
            (None, None, Some(h)) => Some(Target::Block(h)),
            _ => None,
        };
        if self.spectator {
            self.target = None;
        }
    }

    fn handle_actions(&mut self, dt: f32, c: &Controls) {
        let held = self.inv.held();
        if c.attack_pressed {
            self.player.swing = 1.0;
        }
        if c.attack_held && self.player.swing < 0.3 {
            self.player.swing = 1.0;
        }

        // A fireball in the way gets swatted back first (see fortress.rs).
        if c.attack_pressed && self.attack_cd <= 0.0 && !self.fireballs.is_empty() {
            let (eye, dir, me) = (self.player.eye(), self.player.look_dir(), self.my_id);
            if self.deflect_fireball(eye, dir, me) {
                self.attack_cd = 0.3;
                self.player.swing = 1.0;
                return;
            }
        }
        match &self.target {
            Some(Target::Vehicle(i)) => {
                let i = *i;
                self.breaking = None;
                if c.attack_pressed && self.attack_cd <= 0.0 {
                    let id = self.vehicles[i].id;
                    self.attack_cd = 0.3;
                    if self.is_client() {
                        self.net_send_msg(Msg::VehicleUse { id, action: 2 });
                        self.vehicles[i].hurt = 0.4;
                    } else {
                        let me = self.my_id + 1;
                        self.hit_vehicle(id, me);
                    }
                }
            }
            Some(Target::Mob(i)) => {
                self.breaking = None;
                if c.attack_pressed && self.attack_cd <= 0.0 {
                    let i = *i;
                    // Early swings are weak; only a full one can crit (see combat.rs).
                    let charge = self.attack_charge();
                    let crit = self.player.body.vel.y < -1.0 && charge > 0.9;
                    let strength = self.effect_amplifier(crate::potions::Potion::Strength).map_or(0.0, crate::potions::strength_bonus);
                    let mut dmg = (attack_damage_with(held, self.held_level(Enchant::Sharpness)) + strength) * crate::combat::charge_scale(charge) * if crit { 1.5 } else { 1.0 };
                    // A Mace swung on the way down: the fall goes into the blow (and not into you).
                    let fall = self.player.fall_start - self.player.body.pos.y;
                    let smash = held == MACE && fall > crate::combat::SMASH_MIN && !self.player.body.on_ground;
                    if smash {
                        dmg += crate::combat::smash_bonus(fall);
                        self.player.fall_start = self.player.body.pos.y;
                        self.player.body.vel.y = self.player.body.vel.y.max(4.0);
                        self.sfx(Sfx::Thud, None);
                        self.advance("smash");
                    }
                    // A spear from a moving mount: the charge goes into the blow.
                    let charge_speed = self.mount_speed();
                    let lunge = is_spear(held) && charge_speed > 2.0;
                    if lunge {
                        dmg += crate::combat::lunge_bonus(charge_speed);
                        if charge_speed > 6.0 {
                            self.advance("jousting");
                        }
                    }
                    self.stats.damage_dealt += dmg.min(self.mobs[i].health.max(0.0)) as f64;
                    self.since_attack = 0.0;
                    self.use_tool(hit_wear(held));
                    if !self.creative {
                        self.player.hunger.exhaust(crate::hunger::ATTACK);
                    }
                    let from = self.player.body.pos;
                    if !self.is_client() {
                        // Pets join in.
                        let (me, id) = (crate::players::record_key(&self.player_name), self.mobs[i].id);
                        self.sic_pets(&me, id);
                    }
                    if self.is_client() {
                        // The host owns mobs: ask it to apply the hit (it echoes the sound back).
                        let mob = self.mobs[i].id;
                        self.net_send_msg(Msg::Attack { mob, dmg, from });
                        self.mobs[i].hurt = 0.5;
                    } else {
                        if smash {
                            let at = self.mobs[i].body.pos;
                            let me = self.my_id;
                            self.smash_around(at, i, me);
                        }
                        self.creaking_hit(i);
                        self.mobs[i].damage(dmg, from);
                        self.mobs[i].last_attacker = 0;
                        let (kind, at) = (self.mobs[i].kind, self.mobs[i].body.pos);
                        self.sfx(Sfx::hurt_of(kind), Some(at));
                        if lunge {
                            let push = (at - from).normalize_or_zero() * (3.0 + charge_speed * 0.6);
                            self.mobs[i].body.vel += push + Vec3::Y * 3.0;
                        } else if charge > 0.9 {
                            if self.player.sprinting {
                                // A running start sends them flying.
                                let push = (at - from).normalize_or_zero() * 5.0;
                                self.mobs[i].body.vel += push;
                            } else if is_sword(held) && self.player.body.on_ground {
                                self.sweep(i, from);
                            }
                        }
                    }
                    self.attack_cd = 0.1;
                }
            }
            Some(Target::Block(h)) => {
                let pos = h.pos;
                // A frame with something in it gives that up before breaking.
                if c.attack_pressed && self.attack_cd <= 0.0 && self.hit_frame(pos) {
                    self.attack_cd = 0.25;
                    self.breaking = None;
                    return;
                }
                if c.attack_pressed && crate::music::is_note_block(self.world.get_v(pos)) {
                    // A tap plays it (as well as starting to break it).
                    let pitch = crate::music::pitch_of(self.world.get_v(pos));
                    self.sound_note(pos, pitch);
                }
                if c.attack_held {
                    let id = self.world.get_v(pos);
                    if self.creative {
                        if self.attack_cd <= 0.0 {
                            self.break_block(pos, false);
                            self.attack_cd = 0.22;
                        }
                    } else {
                        let efficiency = self.held_level(Enchant::Efficiency);
                        let (t, _) = break_time_with(id, held, efficiency);
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
                            let (_, drops) = break_time_with(id, held, efficiency);
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

        if (c.use_pressed || c.use_held) && self.use_cd <= 0.0 {
            self.use_cd = 0.25;
            // With a weapon or tool in hand, a block (torches, say) in the other hand gets placed.
            let other = self.inv.offhand.map(|s| s.0).unwrap_or(AIR);
            if other != AIR && other != SHIELD && is_block_item(other) && offhand_first(self.inv.held()) && matches!(self.target, Some(Target::Block(_))) {
                self.inv.swap_hands();
                self.use_item();
                self.inv.swap_hands();
            } else {
                self.use_item();
            }
        }
        if !c.use_held {
            self.use_cd = 0.0;
        }

        if c.pick
            && let Some(Target::Block(h)) = &self.target {
                let id = self.world.get_v(h.pos);
                if self.creative && is_block_item(id) {
                    self.inv.slots[self.inv.selected] = Some((id, 64));
                    self.held_name = 2.0;
                }
            }
        if c.drop {
            self.throw_held(c.drop_all);
        }
    }

    fn use_item(&mut self) {
        let held = self.inv.held();
        // Goo (or honeycomb) waxes copper so it stops ageing.
        if matches!(held, GOO | HONEYCOMB)
            && let Some(Target::Block(h)) = &self.target
        {
            let pos = h.pos;
            if self.wax_copper(pos) {
                return;
            }
        }
        // Feeding, shearing and taming animals.
        if let Some(Target::Mob(i)) = self.target
            && self.use_on_mob(i)
        {
            return;
        }
        // Getting into a boat or cart; putting one down.
        if let Some(Target::Vehicle(i)) = self.target {
            self.mount(i);
            return;
        }
        if matches!(held, BOAT | MINECART | CHEST_MINECART | HOPPER_MINECART) && self.place_vehicle(held) {
            return;
        }
        // Sneak-right-clicking a chest with iron, gold or diamonds upgrades it.
        if let Some(Target::Block(h)) = &self.target
            && self.player.sneaking
            && crate::chests::is_chest(self.world.get_v(h.pos))
            && self.upgrade_chest(h.pos)
        {
            return;
        }
        // Chests, furnaces and anvils open (sneak to place against them instead).
        if let Some(Target::Block(h)) = &self.target
            && !self.player.sneaking
        {
            let (pos, id) = (h.pos, self.world.get_v(h.pos));
            if crate::containers::is_container(id) {
                self.open_container(pos);
                return;
            }
            if crate::anvil::is_anvil(id) {
                self.open_anvil(pos);
                return;
            }
            if id == ENCHANTING_TABLE {
                self.open_enchanting(pos);
                return;
            }
            if let Some(b) = crate::smithing::Bench::of_block(id) {
                self.open_bench(pos, b);
                return;
            }
            if crate::music::is_note_block(id) {
                self.tune_note_block(pos);
                return;
            }
            if id == BELL {
                self.ring_bell(pos);
                return;
            }
            if id == VAULT || id == VAULT_OMINOUS {
                self.use_vault(pos);
                return;
            }
            if id == LOOM {
                self.use_loom(pos);
                return;
            }
            if id == PERSONAL_CHEST {
                self.open_stash(pos);
                return;
            }
            if crate::books::is_lectern(id) {
                self.use_lectern(pos, held);
                return;
            }
            if id == LODESTONE && held == COMPASS {
                self.link_lodestone(pos);
                return;
            }
            if crate::music::is_jukebox(id) && self.use_jukebox(pos, held) {
                return;
            }
            if crate::bees::is_hive(id) && matches!(held, GLASS_BOTTLE | SHEARS | BEE_SMOKER | WOOD_ASH | HIVE_TOOL | QUEEN_BEE) {
                self.use_on_hive(pos, held);
                return;
            }
            if id == RESTORATION_BENCH && held == ENCRUSTED_RELIC {
                self.use_restoration(pos);
                return;
            }
            // Glow berries: pick them off a vine; plant them under a ceiling.
            if id == CAVE_VINES_LIT && self.pick_berries(pos) {
                return;
            }
            if held == GLOW_BERRIES
                && let Some(Target::Block(h)) = &self.target
            {
                let (normal, hit) = (h.normal, h.pos);
                if self.plant_berries(hit, normal, id) {
                    return;
                }
            }
            if let Some(sprout) = crate::sniffers::sprout_of(held)
                && matches!(id, GRASS | DIRT | FARMLAND | FARMLAND_WET | MUD | PALE_MOSS)
                && self.world.get_v(pos + IVec3::Y) == AIR
            {
                self.world.set_v(pos + IVec3::Y, sprout);
                self.advance("ancient_seeds");
                self.sfx(Sfx::Place(crate::sound::Mat::Grass), Some(pos.as_vec3() + Vec3::splat(0.5)));
                self.player.swing = 1.0;
                if !self.creative {
                    self.inv.consume_held();
                }
                return;
            }
        }
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
        // Armour goes on (swapping with whatever was worn).
        if armor_of(held).is_some() {
            let slot = self.inv.selected;
            self.inv.equip(slot);
            self.sfx(Sfx::Place(crate::sound::Mat::Glass), None);
            self.msg(format!("You put on the {}. Dashing.", item_name(held)));
            return;
        }
        if matches!(held, BUCKET | WATER_BUCKET | LAVA_BUCKET) && self.use_bucket(held) {
            return;
        }
        if held == STARING_EYE && self.use_eye() {
            return;
        }
        if self.use_potion(held) {
            return;
        }
        // Planting, tilling and soil science come before eating (carrots are both).
        if let Some(Target::Block(h)) = &self.target {
            let pos = h.pos;
            if self.farm_use(held, pos) {
                return;
            }
        }
        if held == PUFFER {
            self.use_up_held();
            self.sfx(Sfx::Eat, None);
            self.player.hurt = 0.0;
            self.hurt_player(4.0, "ate a Pufferfish. It said 'Do Not Eat' right on it");
            self.player.hunger.eat(1.0, 0.1);
            self.msg("You ate a Pufferfish. Bold. Very bold. Ow.");
            return;
        }
        if held == COOKED_PUFFER {
            self.use_up_held();
            self.sfx(Sfx::Eat, None);
            self.player.hurt = 0.0;
            self.hurt_player(2.0, "ate a Cooked Pufferfish. Cooking it did not help");
            self.player.hunger.eat(2.0, 0.3);
            self.msg("Cooking it only halved the problem. Still ow.");
            return;
        }
        if held == STEW {
            // Suspicious for a reason.
            let heal = self.rng.range(-3.0, 8.0).round();
            self.use_up_held();
            self.sfx(Sfx::Eat, None);
            self.player.hunger.eat(6.0, 0.6);
            if heal < 0.0 {
                self.player.hurt = 0.0;
                self.hurt_player(-heal, "ate a Suspicious Stew. It was, in fact, suspicious");
                self.msg("The stew was... suspicious. Your stomach files a complaint.");
            } else {
                self.player.health = (self.player.health + heal).min(MAX_HEALTH);
                self.msg(format!("The stew was surprisingly fine. +{heal} health."));
            }
            return;
        }
        if self.open_backpack() {
            return;
        }
        if held == ROCKET {
            self.use_rocket();
            return;
        }
        if self.use_find(held) {
            return;
        }
        if held == RECOVERY_COMPASS {
            match self.last_death {
                Some(d) => {
                    let v = d - self.player.body.pos;
                    let dist = Vec3::new(v.x, 0.0, v.z).length();
                    self.msg(format!("The needle points {} : your last mistake is {:.0} blocks away.", crate::archaeology::compass_word(v), dist));
                }
                None => self.msg("The needle spins. You haven't died here yet. Congratulations?"),
            }
            return;
        }
        if self.eat_honey(held) {
            return;
        }
        if is_spear(held) {
            self.throw_spear(held);
            return;
        }
        if held == WIND_CHARGE {
            self.throw_wind_charge();
            return;
        }
        if held == GOAT_HORN {
            self.blow_horn();
            return;
        }
        if held == OMINOUS_BOTTLE {
            // A Bad Omen: raids in villages, ominous trials in Trial Chambers.
            if !self.is_client() {
                let me = self.my_id;
                self.give_effect_to(me, crate::potions::Potion::BadOmen, crate::raids::OMEN_SECS);
            }
            self.sfx(Sfx::Eat, None);
            self.msg("You feel a Bad Omen. Something's going to go wrong (on purpose).");
            if !self.creative {
                self.use_up_held();
            }
            return;
        }
        if held == SPYGLASS {
            // Held down to look through (see gadgets.rs).
            return;
        }
        if held == BUNDLE {
            self.tip_bundle();
            return;
        }
        if crate::books::is_book(held) {
            self.open_held_book();
            return;
        }
        if held == MAP {
            // Zoom out (and back in).
            self.map_zoom = (self.map_zoom + 1) % crate::navigation::ZOOMS.len() as u8;
            self.msg(format!("Map: one pixel for every {} block(s).", crate::navigation::ZOOMS[self.map_zoom as usize]));
            return;
        }
        if held == BOTTLE {
            let m = crate::fishing::BOTTLE_MESSAGES[self.rng.int(0, crate::fishing::BOTTLE_MESSAGES.len() as i32 - 1) as usize];
            self.msg(format!("The message reads: {m}"));
            if !self.creative {
                self.use_up_held();
            }
            return;
        }
        // Food fills the hunger bar (a full bar is what heals you). Legendary
        // food can be eaten any time, and heals you outright.
        if let Some(points) = food_value(held) {
            let legendary = matches!(held, GOLDEN_CHOP | BIG_BOB);
            if !self.player.hunger.full() || legendary || self.creative {
                self.player.hunger.eat(points, food_quality(held));
                self.stats.eaten += 1;
                if legendary {
                    self.player.health = MAX_HEALTH;
                }
                if !self.creative {
                    self.use_up_held();
                }
                self.sfx(Sfx::Eat, None);
                self.msg(match held {
                    GOO => "You ate Groaner Goo. You feel... gooey.",
                    MUTTON => "Raw Baa-con. Crunchy? No. Wool-adjacent? Yes.",
                    GOLDEN_CHOP => "You feel golden. Also slightly metallic. Fully healed!",
                    BREAD => "Bread! Civilisation has arrived.",
                    CARROT => "Crunch. You can see slightly better. (You can't.)",
                    POTATO => "A raw potato. Nobody asked for this. Least of all you.",
                    FISH_CHIPS => "Fish n' Chips. Legally distinct, emotionally identical.",
                    BIG_BOB => "You ate Big Bob. You monster. Fully healed, though.",
                    COD | SALMON | TROPICAL => "Raw fish. Sushi, technically.",
                    CLUCKETS | MOO_STEAK => "Raw meat. There's a furnace for that now, you know.",
                    COOKED_CHOP | STEAK | COOKED_MUTTON | COOKED_CLUCKETS => "Cooked to perfection. By a cube. Of cobblestone.",
                    COOKED_COD | COOKED_SALMON => "Grilled fish. The Fish Log would be proud.",
                    BAKED_POTATO => "A baked potato. The potato's redemption arc is complete.",
                    COOKED_BOOT => "You ate a boot. It was chewy. It's always chewy.",
                    _ => "*nom* Oinkchop acquired (internally).",
                });
                if held == GOLDEN_CHOP {
                    self.advance("golden_boy");
                }
                if held == MELON_SLICE {
                    self.advance("melon_baller");
                }
                return;
            }
        }
        if let Some((actions, consume)) = use_actions(held) {
            let at = self.player.eye() + self.player.look_dir() * 2.0;
            self.run_actions(actions, at);
            self.player.swing = 1.0;
            if consume && !self.creative {
                self.use_up_held();
            }
            return;
        }
        if held == PEARL {
            self.player.swing = 1.0;
            self.throw_pearl();
            return;
        }
        if held == BOW {
            self.shoot_bow();
            return;
        }
        if held == CROSSBOW {
            self.shoot_crossbow();
            return;
        }
        if held == ROD {
            self.use_rod();
            return;
        }
        let Some(Target::Block(h)) = &self.target else { return };
        let (hit_pos, normal) = (h.pos, h.normal);
        let hit_y = self.player.eye().y + self.player.look_dir().y * h.dist - hit_pos.y as f32;
        let hit_id = self.world.get_v(hit_pos);
        // Levers and buttons (sneak to place against one instead).
        if !self.player.sneaking && self.use_switch(hit_pos, hit_id) {
            return;
        }
        // Item frames take what you're holding.
        if !self.player.sneaking && crate::decor::is_frame(hit_id) && self.use_frame(hit_pos) {
            return;
        }
        // Campfires take raw food to cook.
        if hit_id == CAMPFIRE && self.use_campfire(hit_pos) {
            return;
        }
        // Signs take a dye's colour, or a Glowshroom's glow.
        if crate::decor::is_sign(hit_id) && !self.player.sneaking && self.style_sign(hit_pos) {
            return;
        }
        // Candles light and go out.
        if matches!(hit_id, CANDLE | CANDLE_LIT) && self.use_candle(hit_pos) {
            return;
        }
        // Gates and trapdoors swing (sneak to place against them instead).
        if !self.player.sneaking && self.toggle_hinged(hit_pos) {
            return;
        }
        // Doors open and close (sneak to place against one instead).
        if is_door(hit_id) && !self.player.sneaking {
            self.toggle_door(hit_pos);
            return;
        }
        // Sneak to place blocks against beds and cakes instead of using them.
        if crate::beds::is_bed(hit_id) && !self.player.sneaking {
            self.sleep(hit_pos);
            return;
        }
        if hit_id == CAKE && !self.player.sneaking {
            if self.player.hunger.full() && !self.creative {
                self.msg("You're full. Even for cake. Impressive restraint.");
                return;
            }
            self.player.hunger.eat(14.0, 0.1);
            self.world.set_v(hit_pos, AIR);
            self.block_particles_tile(hit_pos, T_CAKE_SIDE, 12);
            self.sfx(Sfx::Eat, None);
            self.msg("You ate the whole cake in one bite. The cake was not a lie.");
            self.advance("cake");
            return;
        }
        if held == SPARKER && self.use_sparker(hit_pos, normal) {
            return;
        }
        if hit_id == TNT && (held == TORCH || held == AIR || held == SPARKER) {
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
        if hit_id == TABLE && !self.player.sneaking {
            self.at_table = true;
            return;
        }
        if held == DOOR {
            self.place_door(hit_pos, normal, hit_id);
            return;
        }
        if held == ZAP_DUST {
            self.place_dust(hit_pos, normal, hit_id);
            return;
        }
        // String on a floor is tripwire, running the way you're facing (see tripwire.rs).
        if held == STRING && normal == IVec3::Y && is_solid(hit_id) {
            let place = hit_pos + IVec3::Y;
            if place.y < CH && replaceable(self.world.get_v(place)) {
                self.world.set_v(place, crate::tripwire::tripwire(self.facing() % 2, false));
                self.sfx(Sfx::Place(crate::sound::Mat::Grass), Some(place.as_vec3() + Vec3::splat(0.5)));
                self.player.swing = 1.0;
                if !self.creative {
                    self.inv.consume_held();
                }
            }
            return;
        }
        if !is_block_item(held) {
            return;
        }
        if self.try_merge_slab(held, hit_pos, hit_id, normal) {
            return;
        }
        let place = if replaceable(hit_id) { hit_pos } else { hit_pos + normal };
        if place.y < 0 || place.y >= CH || !replaceable(self.world.get_v(place)) {
            return;
        }
        let below = self.world.get_v(place - IVec3::Y);
        match held {
            FLOWER | TALL_GRASS | SAPLING if !matches!(below, GRASS | DIRT | SNOW_GRASS) => return,
            // Torches go on walls too.
            TORCH if !(is_solid(below) || normal.y == 0 && is_solid(hit_id)) => return,
            LEVER | BUTTON | PLATE | RAIL_FIRST | POWERED_RAIL | DETECTOR_RAIL | SIGN_FIRST if !is_solid(below) => return,
            // Frames and ladders go on walls.
            FRAME_FIRST | LADDER_FIRST | PAINTING_FIRST | TRIPWIRE_HOOK_FIRST if crate::decor::frame_facing(normal).is_none() || !is_solid(hit_id) => return,
            _ => {}
        }
        if is_solid(held) && self.cell_occupied(place) {
            return;
        }
        if !self.is_client() {
            let args = vec![self.player_name.clone().into(), (place.x as INT).into(), (place.y as INT).into(), (place.z as INT).into(), reg().key_of(held).into()];
            if !self.fire("on_block_place", args) {
                return;
            }
        }
        let mut oriented = self.oriented(held, normal, if replaceable(hit_id) { 0.0 } else { hit_y });
        if held == TORCH
            && is_solid(hit_id)
            && !replaceable(hit_id)
            && let Some(f) = crate::decor::frame_facing(normal)
        {
            oriented = WALL_TORCH_FIRST + f as Id;
        }
        self.world.set_v(place, oriented);
        if held == HOLLOW_BOX && !self.is_client() {
            let wear = self.inv.wear[self.inv.selected];
            self.unpack_box(place, wear);
        }
        if held == BANNER && !self.is_client() {
            let design = crate::enchant::enchants(self.inv.wear[self.inv.selected]);
            let facing = crate::banners::facing_from_yaw(self.player.yaw);
            self.put_up_banner(place, design, facing);
        }
        if held == SIGN_FIRST {
            // Something to write on it.
            self.editing_sign = Some(place);
        }
        self.sfx(Sfx::Place(material(held)), Some(place.as_vec3() + Vec3::splat(0.5)));
        self.stats.placed += 1;
        self.player.swing = 1.0;
        if !self.creative {
            self.inv.consume_held();
        }
        match held {
            SPONGE => self.soak(place),
            JACK => self.advance("spooky"),
            p if crate::masonry::is_powder(p) && !self.is_client() => self.harden_powder(place),
            _ => {}
        }
        if matches!(held, PUMPKIN | JACK) && !self.is_client() {
            let me = crate::players::record_key(&self.player_name);
            self.try_build_copper_golem(place, &me);
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
        if !self.creative && drops {
            self.use_tool(dig_wear(self.inv.held(), id));
            self.player.hunger.exhaust(crate::hunger::DIG);
        }
        if is_door(id) {
            self.remove_door_partner(pos, id);
        }
        // Chests and furnaces hand over what was inside (joined players get it from the host).
        if !self.is_client() {
            self.spill_container(pos);
            self.spill_frame(pos);
            self.spill_lectern(pos);
            self.spill_banner(pos);
            self.block_gone(pos, id);
        }
        self.world.set_v(pos, AIR);
        self.sfx(Sfx::Break(material(id)), Some(pos.as_vec3() + Vec3::splat(0.5)));
        let on_break: &'static [Action] = &block(id).on_break;
        if !on_break.is_empty() {
            self.run_actions(on_break, pos.as_vec3() + Vec3::splat(0.5));
        }
        self.stats.mined += 1;
        if self.stats.mined >= 100 {
            self.advance("centurion");
        }
        if id == WYRM_CRYSTAL {
            self.crystal_broken(pos);
        }
        if id == ICE && !self.creative {
            self.world.set_v(pos, WATER);
            self.msg("The ice melted. Science!");
        }
        if crate::archaeology::is_suspicious(id) {
            self.msg("You dug it up instead of brushing it. Whatever was inside crumbled. Archaeologists everywhere wince.");
        }
        if drops && !self.creative {
            // Drops land where the world lives; joined players' come from the host
            // (see ledger.rs), and they pick them up like anything else.
            if !self.is_client() {
                let center = pos.as_vec3() + Vec3::splat(0.5);
                let d = block(id).drop;
                let silk = self.held_level(Enchant::SilkTouch) > 0 && (crate::enchant::Enchant::SilkTouch).fits(self.inv.held());
                if let Some(whole) = crate::enchant::silk_drop(id).filter(|_| silk) {
                    self.pop_drop(center, whole, 1);
                } else if d != AIR {
                    let roll = self.rng.range(0.0, 1.0);
                    let n = fortune_count(id, self.held_level(Enchant::Fortune), roll);
                    self.pop_drop(center, d, n);
                }
                for (item, n) in crate::farming::random_drops(id, &mut self.rng).into_iter().filter(|_| !silk) {
                    match item {
                        COAL => self.msg("Found coal in the gravel. Don't ask."),
                        BAIT => self.msg("You found a Wiggly Worm. The fish will love it."),
                        _ => {}
                    }
                    self.pop_drop(center, item, n);
                }
                let points = crate::xp::ore_xp(id, &mut self.rng);
                self.spawn_orbs(center, points);
            }
            self.farm_break_effects(pos, id);
        }
        // Vines and buds hanging from it come down.
        self.drop_hangers(pos);
        // Plants and torches pop off with their support.
        let above = pos + IVec3::Y;
        let a = self.world.get_v(above);
        if (block(a).model == Model::Cross && !is_wall_torch(a)) || door_state(a).is_some_and(|(_, _, top)| !top) || crate::wiring::needs_floor(a) {
            self.world.set_v(above, AIR);
            if is_door(a) {
                self.world.set_v(above + IVec3::Y, AIR);
            }
            if !self.creative && !self.is_client() && block(a).drop != AIR {
                self.pop_drop(above.as_vec3() + Vec3::splat(0.5), block(a).drop, 1);
            }
        }
        // Torches on its sides fall off too.
        for f in 0..4u8 {
            let side = pos + crate::decor::outward(f).as_ivec3();
            if self.world.get_v(side) == WALL_TORCH_FIRST + f as Id {
                self.world.set_v(side, AIR);
                if !self.creative && !self.is_client() {
                    self.pop_drop(side.as_vec3() + Vec3::splat(0.5), TORCH, 1);
                }
            }
        }
        // Bamboo: the whole stalk above comes down.
        let mut q = above + IVec3::Y;
        while a == BAMBOO && self.world.get_v(q) == BAMBOO {
            self.world.set_v(q, AIR);
            if !self.creative && !self.is_client() {
                self.pop_drop(q.as_vec3() + Vec3::splat(0.5), BAMBOO, 1);
            }
            q += IVec3::Y;
        }
    }

    /// Tell everyone how the local player died.
    fn announce_death(&mut self, cause: &str) {
        let line = format!("{} {cause}", self.player_name);
        if self.is_client() {
            self.net_send_msg(Msg::Died { cause: cause.to_string() });
        } else {
            self.system_message(None, &line);
            self.chat_log.push_back(line);
            while self.chat_log.len() > CHAT_KEEP {
                self.chat_log.pop_front();
            }
        }
    }

    /// A block broke where the world lives (by anyone): hives, pots and sculk react, and it's heard.
    pub fn block_gone(&mut self, pos: IVec3, old: Id) {
        self.jukebox_broken(pos, old);
        self.gold_taken(pos, old);
        if crate::bees::is_hive(old) {
            self.hive_broken(pos, old);
        }
        if crate::archaeology::is_pot(old) {
            self.pot_broken(pos, old);
        }
        self.vibrate(pos.as_vec3() + Vec3::splat(0.5), None);
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
                // Joined players' items come from the host, which runs the same action.
                Action::Give(item, n) if !self.is_client() => self.give(*item, *n),
                Action::Give(..) => {}
                Action::SetTime(t) => {
                    if self.is_client() {
                        self.msg("Only the host can change the time.");
                    } else {
                        self.time = *t;
                        self.net_broadcast(self.time_msg());
                    }
                }
                Action::Spawn(k) => {
                    if self.is_client() {
                        self.msg("Only the host can spawn mobs.");
                    } else if let Some(kind) = MobKind::from_index(*k) {
                        self.alloc_mob(kind, at + Vec3::Y * 0.5);
                    }
                }
            }
        }
    }

    pub fn give(&mut self, item: Id, n: u8) {
        self.give_worn(item, n, 0);
    }

    /// Give items, a used tool keeping its wear.
    pub fn give_worn(&mut self, item: Id, n: u8, wear: crate::inventory::Wear) {
        self.sfx(Sfx::Pop, None);
        let left = self.inv.add_worn(item, n, wear);
        if left > 0 {
            self.msg("Inventory full. It's on the floor now.");
            if self.is_client() {
                // The host already counted it as ours: it puts it on the ground for us.
                self.net_send_msg(Msg::DropItem { item, n: left, wear, scatter: false });
            } else {
                let at = self.player.body.pos + Vec3::Y * 0.5;
                self.spawn_drop(at, item, left, wear, Vec3::ZERO, crate::drops::THROW_DELAY);
            }
        }
        self.item_advancements(item);
    }

    pub fn block_particles(&mut self, pos: IVec3, n: usize) {
        let id = self.world.get_v(pos);
        self.block_particles_tile(pos, block(id).tex[1], n);
    }

    pub(crate) fn block_particles_tile(&mut self, pos: IVec3, tile: u16, n: usize) {
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

    pub(crate) fn smoke(&mut self, at: Vec3, n: usize, spread: f32) {
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

    /// How long we can hold our breath (longer in a Turtle Shell).
    pub fn max_air(&self) -> f32 {
        let shell = self.inv.armor[0].is_some_and(|(id, _)| id == TURTLE_SHELL);
        crate::player::MAX_AIR + if shell { crate::player::SHELL_AIR } else { 0.0 }
    }

    /// Breath underwater: it runs down with your head under, then you drown
    /// (a heart a second, with bubbles), and it comes back fast in the air.
    pub fn breath_tick(&mut self, dt: f32) {
        let max = self.max_air();
        let under = self.player.head_in_water(&self.world) && !self.creative && !self.spectator && self.dead.is_none();
        if !under {
            self.player.air = (self.player.air + dt * max / 2.0).min(max);
            return;
        }
        let before = self.player.air;
        self.player.air -= dt;
        if self.player.air > 0.0 {
            // A warning as the last bit runs out.
            if before > 3.0 && self.player.air <= 3.0 {
                self.sfx(Sfx::Bubbles, None);
            }
            return;
        }
        // Out of air: a heart's worth each second.
        self.player.air = (self.player.air + 1.0).max(0.01);
        self.sfx(Sfx::Bubbles, None);
        self.player.hurt = 0.0;
        self.hurt_player(2.0, "drowned. Turns out you do need air");
    }

    /// Hunger over time: healing when well fed, starving when empty.
    fn hunger_tick(&mut self, dt: f32) {
        if self.creative || self.dead.is_some() {
            return;
        }
        let difficulty = self.rules.difficulty;
        if !difficulty.monsters() {
            // Peaceful: never hungry, so always healing.
            self.player.hunger.food = crate::hunger::MAX_FOOD;
            self.player.hunger.saturation = self.player.hunger.saturation.max(5.0);
        }
        let change = self.player.hunger.tick(dt, self.player.health, difficulty.starve_floor());
        if change > 0.0 {
            self.player.health = (self.player.health + change).min(MAX_HEALTH);
        } else if change < 0.0 {
            self.player.hurt = 0.0;
            self.hurt_player(-change, "starved. Should have packed a lunch");
        }
    }

    /// Damage that armour softens (mobs, arrows, explosions): 4% less per armour
    /// point. Each worn piece takes a quarter of the hit (at least 1) as wear.
    pub fn hurt_player_armored(&mut self, amount: f32, cause: &str) {
        let blocked = self.creative || self.dead.is_some() || self.player.hurt > 0.0;
        // Protection: another 4% per level on each piece.
        let protection: u32 = self.inv.armor_wear.iter().map(|&w| level(w, Enchant::Protection) as u32).sum();
        let cut = ((self.inv.armor_points() + protection) as f32 * 0.04).min(0.8);
        self.hurt_player(amount * (1.0 - cut), cause);
        if !blocked && amount > 0.0 {
            for id in self.inv.wear_armor((amount / 4.0).max(1.0) as u16) {
                self.sfx(Sfx::Break(crate::sound::Mat::Glass), None);
                self.msg(format!("Your {} fell apart. It died doing what it loved.", item_name(id)));
            }
        }
    }

    /// Use the held tool, weapon, bow or rod (survival only); it may break.
    pub fn use_tool(&mut self, amount: u16) {
        if self.creative || amount == 0 {
            return;
        }
        if let Some(id) = self.inv.wear_held(amount) {
            self.sfx(Sfx::Break(crate::sound::Mat::Glass), None);
            self.msg(format!("Your {} broke. It had a good life.", item_name(id)));
            self.held_name = 0.0;
        }
    }

    pub fn hurt_player(&mut self, amount: f32, cause: &str) {
        if self.creative || self.dead.is_some() || self.player.hurt > 0.0 {
            return;
        }
        self.player.hunger.exhaust(crate::hunger::HURT);
        self.stats.damage_taken += amount.min(self.player.health).max(0.0) as f64;
        self.player.health -= amount;
        self.player.hurt = 0.5;
        self.sfx(Sfx::Hurt, None);
        if self.player.health <= 0.0 && self.use_totem() {
            return;
        }
        if self.player.health <= 0.0 {
            self.player.health = 0.0;
            self.stats.deaths += 1;
            self.last_death = Some(self.player.body.pos);
            self.dead = Some(format!("{} {cause}", self.player_name));
            self.announce_death(cause);
            // Everything falls out of your pockets, unless the world says otherwise.
            // (Joined players' experience is spilled by the host when it sees them die.)
            if !self.rules.keep_inventory {
                self.drop_everything();
                let at = self.player.body.pos + Vec3::Y * 0.5;
                let points = crate::xp::death_drop(std::mem::take(&mut self.xp));
                self.spawn_orbs(at, points);
            }
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
                        AIR | BEDROCK => {}
                        _ if is_liquid(id) => {}
                        TNT => {
                            self.world.set_v(p, AIR);
                            let fuse = self.rng.range(0.3, 0.9);
                            self.tnts.push(PrimedTnt { pos: p.as_vec3(), fuse });
                        }
                        _ => {
                            self.world.set_v(p, AIR);
                            // No half doors left standing.
                            if is_door(id) {
                                self.remove_door_partner(p, id);
                            }
                            if !self.creative && self.rng.chance(0.25) && block(id).drop != AIR && block(id).pick_tier <= 1 {
                                self.pop_drop(p.as_vec3() + Vec3::splat(0.5), block(id).drop, 1);
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
            self.hurt_player_from(dmg, cause, Some(at), true);
            let push = (self.player.body.pos - at).normalize_or_zero() * (1.0 - pd / (r * 2.0)) * 14.0;
            self.player.body.vel += self.steadied(push + Vec3::Y * 4.0);
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
        if self.dead.is_none() && !self.dedicated && !self.spectator {
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
    pub fn block_change_feedback(&mut self, pos: IVec3, old: Id, new: Id) {
        let center = pos.as_vec3() + Vec3::splat(0.5);
        if new == AIR || is_liquid(new) {
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
        if self.rules.daylight_cycle {
            let before = self.time;
            self.time = (self.time + dt / DAY_SECONDS) % 1.0;
            self.count_days(before);
        }
        self.weather_tick(dt);
        self.liquid_tick(dt);
        self.zap_tick(dt);
        self.vehicles_tick(dt, 0.0, 0.0, false);
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

    pub(crate) fn update_entities(&mut self, dt: f32) {
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
        // Who is looking where, for Starers: (is the local player, eye, look direction).
        let mut gazes: Vec<(bool, Vec3, Vec3)> = Vec::new();
        if visible && self.dead.is_none() && !self.dedicated {
            gazes.push((true, self.player.eye(), self.player.look_dir()));
        }
        for p in self.peers.values().filter(|p| p.alive()) {
            let dir = Vec3::new(p.yaw.sin() * p.pitch.cos(), p.pitch.sin(), -p.yaw.cos() * p.pitch.cos());
            gazes.push((false, p.target + Vec3::Y * EYE, dir));
        }
        let mut eye_contact = false;
        for m in self.mobs.iter_mut().filter(|m| m.kind == MobKind::Starer && !m.angry) {
            for &(local, eye, dir) in &gazes {
                if !m.stared_at(eye, dir) {
                    continue;
                }
                let dist = (m.body.pos + Vec3::Y * (m.body.height - 0.25)).distance(eye);
                // Walls block eye contact.
                if self.world.raycast(eye, dir, dist).is_none_or(|h| h.dist > dist - 0.5) {
                    m.angry = true;
                    noises.push((Sfx::Warp, m.body.pos));
                    eye_contact |= local;
                }
            }
        }
        if eye_contact {
            self.msg("You made eye contact with a Starer. Bold. Also a mistake.");
            self.advance("dont_blink");
        }
        self.house_hmmers();
        self.move_in_temples();
        self.house_clankers();
        self.clankers_tick(dt);
        self.beacons_tick(dt);
        self.animals_tick(dt);
        // (After animals_tick, which sets every mob's goal.)
        self.wildlife_tick(dt);
        self.copper_golems_tick(dt);
        self.floaties_tick();
        self.fireflies_tick(dt);
        self.campfires_tick(dt);
        self.frost_tick(dt);
        self.critters_tick(dt);
        self.cages_tick(dt);
        self.trials_tick(dt);
        self.creaking_tick(dt);
        self.snouts_tick(dt);
        self.raids_tick(dt);
        self.hmmers_tick(dt);
        self.zombie_hmmers_tick(dt);
        self.free_riderless();
        self.tidy_mob_names();
        for m in self.mobs.iter_mut() {
            let p = m.body.pos;
            // Ridden Gallopers go where their rider steers (see horses.rs).
            if !self.world.is_loaded(p.x.floor() as i32, p.z.floor() as i32) || m.rider != 0 || m.passenger != 0 || m.crew != [0, 0] {
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
            let id = m.id;
            events.extend(evs.into_iter().map(|e| (target_id, target, id, e)));
            if fuse_before == 0.0 && m.fuse > 0.0 {
                noises.push((Sfx::Hiss, m.body.pos));
            }
            // Idle chatter.
            if self.rng.chance(dt * 0.1) && targets.iter().any(|t| m.body.pos.distance(t.1) < 20.0) {
                match m.kind {
                    MobKind::Oinker => noises.push((Sfx::Oink, m.body.pos)),
                    MobKind::Groaner => noises.push((Sfx::Groan, m.body.pos)),
                    MobKind::Fluffer => noises.push((Sfx::Baa, m.body.pos)),
                    MobKind::Cluckster => noises.push((Sfx::Cluck, m.body.pos)),
                    MobKind::Mooer => noises.push((Sfx::Moo, m.body.pos)),
                    MobKind::Rattler => noises.push((Sfx::Rattle, m.body.pos)),
                    MobKind::Webber => noises.push((Sfx::Skitter, m.body.pos)),
                    MobKind::Bloop => noises.push((Sfx::Bloop, m.body.pos)),
                    MobKind::Woofer => noises.push((Sfx::Woof, m.body.pos)),
                    MobKind::Hmmer => noises.push((Sfx::Hmm, m.body.pos)),
                    MobKind::Grumbler => noises.push((Sfx::Oink, m.body.pos)),
                    MobKind::Soggy => noises.push((Sfx::Groan, m.body.pos)),
                    MobKind::Squawker => noises.push((Sfx::Squawk, m.body.pos)),
                    MobKind::Bee => noises.push((Sfx::Buzz, m.body.pos)),
                    MobKind::Sneaker if !m.sitting => noises.push((Sfx::Yip, m.body.pos)),
                    MobKind::Ribbit => noises.push((Sfx::Croak, m.body.pos)),
                    MobKind::Hush => noises.push((Sfx::Roar, m.body.pos)),
                    MobKind::Weeper => noises.push((Sfx::Groan, m.body.pos)),
                    MobKind::Snout | MobKind::Strutter => noises.push((Sfx::Oink, m.body.pos)),
                    MobKind::Pilferer | MobKind::Hackler | MobKind::Invoicer => noises.push((Sfx::Hmm, m.body.pos)),
                    MobKind::Rampager => noises.push((Sfx::Roar, m.body.pos)),
                    MobKind::Goat => noises.push((Sfx::Bleat, m.body.pos)),
                    MobKind::Sizzler | MobKind::Fee | MobKind::Breeze | MobKind::Axolotl | MobKind::Camel | MobKind::Creaking => {}
                    MobKind::Sniffer => noises.push((Sfx::Moo, m.body.pos)),
                    MobKind::Rotsteed => noises.push((Sfx::Groan, m.body.pos)),
                    MobKind::CopperGolem | MobKind::Floaty => {}
                    MobKind::ZombieHmmer => noises.push((Sfx::Groan, m.body.pos)),
                    MobKind::Wanderer => noises.push((Sfx::Hmm, m.body.pos)),
                    MobKind::Turtle | MobKind::Dolphin | MobKind::Panda | MobKind::PolarBear | MobKind::Llama => {}
                    MobKind::Hisser | MobKind::Starer | MobKind::Galloper | MobKind::Wyrm | MobKind::Clanker | MobKind::Fishy | MobKind::Sneaker | MobKind::Rollo => {}
                    MobKind::Modded(_) => {}
                }
            }
        }
        for (s, at) in noises {
            self.sfx(s, Some(at));
        }
        // Hit one Grumbler and the rest nearby join in.
        let riled: Vec<Vec3> = self.mobs.iter().filter(|m| m.kind == MobKind::Grumbler && m.angry && m.hurt > 0.3).map(|m| m.body.pos).collect();
        for at in riled {
            for m in self.mobs.iter_mut().filter(|m| m.kind == MobKind::Grumbler && m.body.pos.distance(at) < 16.0) {
                m.angry = true;
            }
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
        for (target_id, target, mob_id, e) in events {
            match e {
                MobEvent::HurtPlayer(d, cause) if target_id != self.my_id => {
                    let d = self.rules.difficulty.mob_damage(d);
                    self.hurt_peer(target_id, d, cause, Vec3::Y * 3.0);
                    let _ = target;
                    if let Some(name) = self.peers.get(&target_id).map(|p| crate::players::record_key(&p.name)) {
                        self.sic_pets(&name, mob_id);
                    }
                }
                MobEvent::HurtPlayer(d, cause) => {
                    if cause.starts_with("was stung") {
                        self.bee_log.stings += 1;
                        self.advance("bee_careful");
                    }
                    let me = crate::players::record_key(&self.player_name);
                    self.sic_pets(&me, mob_id);
                    let d = self.rules.difficulty.mob_damage(d);
                    let from = self.mobs.iter().find(|m| m.id == mob_id).map(|m| m.body.pos + Vec3::Y * 0.9);
                    self.hurt_player_from(d, cause, from, false);
                    let knock = (self.player.body.pos - ppos).normalize_or_zero();
                    self.player.body.vel += self.steadied(knock * 3.0 + Vec3::Y * 3.0);
                }
                MobEvent::Explode(at, r, cause) => self.explode(at, r, cause),
                MobEvent::Smoke(at) => self.smoke(at, 1, 0.2),
                MobEvent::Warp(from, to) => {
                    self.smoke(from + Vec3::Y * 1.4, 10, 0.4);
                    self.smoke(to + Vec3::Y * 1.4, 10, 0.4);
                    self.sfx(Sfx::Warp, Some(from));
                    self.sfx(Sfx::Warp, Some(to));
                }
                MobEvent::Shoot(from, vel) => self.spawn_arrow(from, vel, None),
                MobEvent::ShootMod(from, vel, spec) => self.spawn_mod_projectiles(from, vel, spec),
                MobEvent::Fireball(from, vel, big) => self.spawn_fireball(from, vel, big, mob_id),
                MobEvent::Fangs(from, to) => self.late_fees(from, to),
                MobEvent::Summon(at) => self.summon_fees(at),
                MobEvent::SummonMod(at, kind, n) => self.boss_summon(at, kind, n),
                MobEvent::Shush(from) => self.shush(from, target_id, target),
                MobEvent::WindCharge(from, vel) => self.spawn_wind_charge(from, vel, None),
                MobEvent::DropItem(at, item) => self.pop_drop(at, item, 1),
                MobEvent::Bleat(at) => self.sfx(Sfx::Bleat, Some(at)),
            }
        }
        self.update_arrows(dt);
        let mut i = 0;
        while i < self.mobs.len() {
            let m = &self.mobs[i];
            let far = !m.persistent && (self.dedicated || m.body.pos.distance(self.player.body.pos) > 110.0) && self.peers.values().all(|p| m.body.pos.distance(p.target) > 110.0);
            if m.health <= 0.0 || far {
                let m = self.mobs.swap_remove(i);
                if m.kind == MobKind::Llama && m.health <= 0.0 {
                    self.spill_pack(m.id, m.body.pos + Vec3::Y * 0.8);
                }
                if m.health <= 0.0 && m.kind.raider() {
                    self.raider_died(&m);
                }
                if m.health <= 0.0 && m.health > -50.0 {
                    let at = m.body.pos + Vec3::Y * 0.5;
                    self.smoke(at, 10, 0.3);
                    let killer = if m.last_attacker == self.my_id && !self.dedicated { self.player_name.clone() } else { self.peers.get(&m.last_attacker).map(|p| p.name.clone()).unwrap_or_default() };
                    let args = vec![m.kind.script_name().into(), (at.x as rhai::FLOAT).into(), (at.y as rhai::FLOAT).into(), (at.z as rhai::FLOAT).into(), killer.into()];
                    self.fire("on_mob_death", args);
                    if m.kind == MobKind::Wyrm {
                        self.wyrm_defeated(at);
                    }
                    let remote = m.last_attacker != self.my_id && self.peers.contains_key(&m.last_attacker);
                    if m.last_attacker == self.my_id && !self.dedicated {
                        self.stats.kills += 1;
                    }
                    if !remote && !self.dedicated && at.distance(self.player.body.pos) < 32.0 {
                        match m.kind {
                            MobKind::Oinker => self.advance("bacon"),
                            MobKind::Hisser => self.advance("hiss_tory"),
                            MobKind::Groaner => self.advance("groan_up"),
                            MobKind::Starer => self.advance("staring_champ"),
                            MobKind::Rattler => self.advance("bone_zone"),
                            MobKind::Webber => self.advance("arachno"),
                            MobKind::Bloop => self.advance("split_decision"),
                            MobKind::Soggy => self.advance("soggy"),
                            MobKind::Fishy => self.advance("fishy_business"),
                            MobKind::Hush => self.advance("silence"),
                            MobKind::Sizzler => self.advance("too_hot"),
                            MobKind::Weeper => self.advance("dry_your_eyes"),
                            MobKind::Rampager => self.advance("rampage_over"),
                            MobKind::Breeze => self.advance("breeze_through"),
                            MobKind::Creaking => self.advance("heartbreak"),
                            MobKind::Goat | MobKind::Axolotl | MobKind::Camel | MobKind::Sniffer | MobKind::CopperGolem | MobKind::Floaty | MobKind::Rotsteed => {}
                            MobKind::Turtle | MobKind::Dolphin | MobKind::Panda | MobKind::PolarBear | MobKind::Llama | MobKind::ZombieHmmer | MobKind::Wanderer => {}
                            MobKind::Strutter | MobKind::Snout | MobKind::Pilferer | MobKind::Hackler | MobKind::Invoicer | MobKind::Fee => {}
                            MobKind::Fluffer | MobKind::Cluckster | MobKind::Mooer | MobKind::Woofer | MobKind::Hmmer | MobKind::Grumbler | MobKind::Galloper | MobKind::Wyrm | MobKind::Squawker | MobKind::Clanker | MobKind::Bee | MobKind::Sneaker | MobKind::Ribbit | MobKind::Rollo => {}
                            MobKind::Modded(_) => {}
                        }
                    }
                    // Big Bloops split into smaller ones.
                    if m.kind == MobKind::Bloop && m.size > 1.0 {
                        let size = (m.size / 2.0) as u8;
                        for _ in 0..self.rng.int(2, 4) {
                            let off = Vec3::new(self.rng.range(-0.4, 0.4), 0.3, self.rng.range(-0.4, 0.4)) * m.size;
                            self.alloc_mob_sized(MobKind::Bloop, m.body.pos + off, size);
                        }
                        self.sfx(Sfx::Bloop, Some(at));
                    }
                    let points = m.kind.xp_value(m.size, &mut self.rng);
                    self.spawn_orbs(m.body.pos + Vec3::Y * 0.5, points);
                    self.sculk_spread(at, points);
                    self.vibrate(at, None);
                    if let Some(d) = m.kind.mod_def().filter(|d| d.boss) {
                        let t = format!("The {} has been defeated!", d.name);
                        self.msg(t.clone());
                        self.system_message(None, &t);
                        self.sfx(Sfx::Fanfare, Some(at));
                    }
                    // Looting on whatever killed it: a little more of each.
                    let looting = if m.last_attacker == self.my_id && !self.dedicated {
                        if Enchant::Looting.fits(self.inv.held()) { self.held_level(Enchant::Looting) } else { 0 }
                    } else if self.peers.contains_key(&m.last_attacker) && Enchant::Looting.fits(self.verified_held(m.last_attacker)) {
                        crate::enchant::level((self.verified_ench(m.last_attacker) as u32) << 16, Enchant::Looting)
                    } else {
                        0
                    };
                    let extra = if looting > 0 { self.rng.int(0, looting as i32) as u8 } else { 0 };
                    let drops = [m.loot(&mut self.rng).map(|(i, n)| (i, n.saturating_add(extra))), m.extra_loot(&mut self.rng)];
                    for (item, n) in drops.into_iter().flatten() {
                        if !self.creative {
                            self.pop_drop(m.body.pos + Vec3::Y * 0.5, item, n);
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
        // Fewer (or hardly any) particles, as the video options ask.
        let most = [usize::MAX, 160, 24][self.particle_level.min(2) as usize];
        if self.particles.len() > most {
            let extra = self.particles.len() - most;
            self.particles.drain(..extra);
        }
        if self.particles.len() > 1500 {
            let n = self.particles.len() - 1500;
            self.particles.drain(0..n);
        }

        for t in self.tnts.iter_mut() {
            t.fuse -= dt;
        }
        self.falling_tick(dt);
        self.stalactite_tick(dt);
        self.fireballs_tick(dt);
        let boom: Vec<Vec3> = self.tnts.iter().filter(|t| t.fuse <= 0.0).map(|t| t.pos + Vec3::splat(0.5)).collect();
        self.tnts.retain(|t| t.fuse > 0.0);
        for at in boom {
            self.explode(at, 4.0, "went out with a bang (TNT)");
            if at.distance(self.player.body.pos) < 48.0 {
                self.advance("kaboom");
            }
        }

        self.spawn_timer -= dt;
        if self.spawn_timer <= 0.0 {
            self.spawn_timer = 1.0;
            self.try_spawn();
        }
        self.farm_tick(dt);
        self.hive_tick(dt);
        self.trees_tick(dt);
        self.random_ticks(dt);
        self.hollow_tick(dt);
        self.fire_tick(dt);
        self.container_tick(dt);
        self.hoppers_tick(dt);
        self.drops_tick(dt);
        self.orbs_tick(dt);
        if !self.rules.difficulty.monsters() {
            self.mobs.retain(|m| !m.menacing());
        }
    }

    /// A new mob; returns its id.
    /// A modded boss calls for help: up to `n` of `kind` around it, but never
    /// more than eight of them within 64 blocks.
    fn boss_summon(&mut self, at: Vec3, kind: u8, n: u8) {
        let Some(kind) = MobKind::from_index(kind) else { return };
        if kind == MobKind::Wyrm {
            return;
        }
        // (Counted well beyond the fight, so ones that wandered off still count.)
        let near = self.mobs.iter().filter(|m| m.kind == kind && m.body.pos.distance(at) < 64.0).count();
        let n = (n.min(4) as usize).min(8usize.saturating_sub(near));
        for _ in 0..n {
            let off = Vec3::new(self.rng.range(-2.5, 2.5), 0.5, self.rng.range(-2.5, 2.5));
            let p = at + off;
            if is_solid(self.world.get_v(p.floor().as_ivec3())) {
                continue;
            }
            self.alloc_mob(kind, p);
            self.smoke(p + Vec3::Y, 8, 0.4);
        }
        self.sfx(Sfx::Warp, Some(at));
    }

    pub(crate) fn alloc_mob(&mut self, kind: MobKind, pos: Vec3) -> u32 {
        self.alloc_mob_sized(kind, pos, 1)
    }

    fn alloc_mob_sized(&mut self, kind: MobKind, pos: Vec3, size: u8) -> u32 {
        let mut m = Mob::new(kind, pos, &mut self.rng).with_size(size);
        m.id = self.next_mob_id;
        self.next_mob_id += 1;
        self.mobs.push(m);
        self.next_mob_id - 1
    }

    fn try_spawn(&mut self) {
        // Spawn around a random player so everyone gets company.
        let mut centers = if self.dedicated { vec![] } else { vec![self.player.body.pos] };
        centers.extend(self.peers.values().map(|p| p.target));
        if centers.is_empty() {
            return;
        }
        let p = centers[self.rng.int(0, centers.len() as i32 - 1) as usize];
        if crate::scorch::in_scorch(p.x) {
            self.scorch_spawn(p);
            return;
        }
        if crate::hollow::in_hollow(p.x) {
            // Only Starers (and the Wyrm, see hollow.rs) out here.
            let here = self.mobs.iter().filter(|m| m.kind == MobKind::Starer && crate::hollow::in_hollow(m.body.pos.x)).count();
            let (x, z) = (p.x as i32 + self.rng.int(-30, 30), p.z as i32 + self.rng.int(-30, 30));
            if here < 6 && self.world.is_loaded(x, z) && self.world.get(x, crate::hollow::ORIGIN.y, z) == HOLLOW_STONE && self.rules.difficulty.monsters() {
                self.alloc_mob(MobKind::Starer, Vec3::new(x as f32 + 0.5, (crate::hollow::ORIGIN.y + 1) as f32, z as f32 + 0.5));
            }
            return;
        }
        // Kept animals (bred, tamed) don't count against new ones turning up.
        let passive = self.mobs.iter().filter(|m| !m.kind.hostile() && !m.persistent && m.kind != MobKind::Fishy).count();
        let hostile = self.mobs.len() - passive;
        let a = self.rng.range(0.0, TAU);
        let d = self.rng.range(24.0, 56.0);
        let (x, z) = ((p.x + a.cos() * d).floor() as i32, (p.z + a.sin() * d).floor() as i32);
        if !self.world.is_loaded(x, z) {
            return;
        }
        let y = self.world.surface_y(x, z);
        let top = self.world.get(x, y, z);
        let clear = |w: &World, y: i32| !is_solid(w.get(x, y, z)) && !is_solid(w.get(x, y + 1, z)) && !is_liquid(w.get(x, y, z));
        let (_, biome) = self.world.generator.column(x, z);
        use crate::world::Biome;
        let woofy = matches!(biome, Biome::Forest | Biome::Snowy | Biome::Taiga);
        // The sea: schools of Fishies any time, Soggy Groaners in the dark.
        if is_water(top) {
            let depth = (0..y).take_while(|d| is_water(self.world.get(x, y - d, z))).count() as i32;
            let fish = self.mobs.iter().filter(|m| m.kind == MobKind::Fishy).count();
            // Axolotls in warm, weedy water.
            let axolotls = self.mobs.iter().filter(|m| m.kind == MobKind::Axolotl).count();
            if axolotls < 6 && depth >= 2 && matches!(biome, Biome::Jungle | Biome::Mangrove | Biome::Swamp) && self.rng.chance(0.35) {
                for i in 0..self.rng.int(1, 2) {
                    self.alloc_mob(MobKind::Axolotl, Vec3::new(x as f32 + 0.5 + i as f32 * 0.6, (y - depth / 2) as f32, z as f32 + 0.5));
                }
                return;
            }
            // Dolphins in the open sea, in little pods.
            let dolphins = self.mobs.iter().filter(|m| m.kind == MobKind::Dolphin).count();
            if dolphins < 6 && depth >= 5 && biome == Biome::Ocean && !self.is_night() && self.rng.chance(0.15) {
                for i in 0..self.rng.int(1, 3) {
                    self.alloc_mob(MobKind::Dolphin, Vec3::new(x as f32 + 0.5 + i as f32 * 1.2, (y - 2) as f32, z as f32 + 0.5));
                }
                return;
            }
            if fish < 12 && depth >= 2 && self.rng.chance(0.6) {
                let n = self.rng.int(2, 4);
                for i in 0..n {
                    let pos = Vec3::new(x as f32 + 0.5 + i as f32 * 0.6, (y - depth / 2) as f32, z as f32 + 0.5 - i as f32 * 0.4);
                    self.alloc_mob(MobKind::Fishy, pos);
                }
                return;
            }
            let dark = self.is_night() || self.weather.kind == crate::weather::Weather::Thunder;
            if dark && depth >= 3 && hostile < 12 + 4 * self.peers.len() && self.rules.difficulty.monsters() && self.rng.chance(0.5) {
                self.alloc_mob(MobKind::Soggy, Vec3::new(x as f32 + 0.5, (y - depth + 1) as f32, z as f32 + 0.5));
                if self.rng.chance(0.3)
                    && let Some(m) = self.mobs.last_mut()
                {
                    m.seed = 1;
                }
            }
            return;
        }
        // Turtles on beaches, Pandas in jungles, Polar Bears on the snow, Llamas on the plains (see wildlife.rs).
        if !self.is_night() && passive < 8 && matches!(top, SAND | GRASS | SNOW_GRASS) && clear(&self.world, y + 1) && (top != SAND || (crate::world::SEA - 1..=crate::world::SEA + 2).contains(&y))
            && let Some(kind) = crate::wildlife::spawn_kind(biome, top, &mut self.rng)
        {
            for i in 0..self.rng.int(1, 2) {
                self.alloc_mob(kind, Vec3::new(x as f32 + 0.5 + i as f32 * 0.9, y as f32 + 1.0, z as f32 + 0.5));
            }
            return;
        }
        // Rollos in the dry lands.
        if !self.is_night() && passive < 8 && matches!(top, SAND | RED_SAND) && matches!(biome, Biome::Desert | Biome::Badlands) && clear(&self.world, y + 1) && self.rng.chance(0.3) {
            // (Camels, now and then, in the desert proper.)
            let kind = if biome == Biome::Desert && self.rng.chance(0.3) { MobKind::Camel } else { MobKind::Rollo };
            self.alloc_mob(kind, Vec3::new(x as f32 + 0.5, y as f32 + 1.0, z as f32 + 0.5));
            return;
        }
        if !self.is_night() && passive < 8 && matches!(top, GRASS | SNOW_GRASS | MUD) && clear(&self.world, y + 1) {
            let kind = if woofy && self.rng.chance(if biome == Biome::Taiga { 0.4 } else { 0.25 }) {
                MobKind::Woofer
            } else if matches!(biome, Biome::Taiga | Biome::Snowy) && (y > crate::world::SEA + 22 || self.rng.chance(0.12)) {
                // Goats like it high up and cold.
                MobKind::Goat
            } else if matches!(biome, Biome::Taiga | Biome::Snowy) && self.rng.chance(0.35) {
                MobKind::Sneaker
            } else if biome == Biome::Swamp && self.rng.chance(0.5) {
                MobKind::Ribbit
            } else if top == SNOW_GRASS || top == MUD {
                return;
            } else if biome == Biome::Plains && self.rng.chance(0.15) {
                MobKind::Galloper
            } else if biome == Biome::Jungle && self.rng.chance(0.5) {
                MobKind::Squawker
            } else {
                [MobKind::Oinker, MobKind::Fluffer, MobKind::Cluckster, MobKind::Mooer][self.rng.int(0, 3) as usize]
            };
            for i in 0..self.rng.int(1, 3) {
                let pos = Vec3::new(x as f32 + 0.5 + i as f32 * 0.7, y as f32 + 1.0, z as f32 + 0.5);
                self.alloc_mob(kind, pos);
            }
            return;
        }
        let cap = ((12 + 4 * self.peers.len()) as f32 * crate::skies::monster_scale(self.moon_phase())) as usize;
        if hostile >= cap || !self.rules.difficulty.monsters() {
            return;
        }
        let roll = self.rng.f32();
        let kind = match roll {
            // Rotsteeds wander the open plains at night.
            r if matches!(biome, Biome::Plains) && r < 0.05 => MobKind::Rotsteed,
            // Swamps are Bloop country; the badlands rattle.
            r if biome == Biome::Swamp && r < 0.35 => MobKind::Bloop,
            r if biome == Biome::Badlands && r < 0.3 => MobKind::Rattler,
            r if r < 0.28 => MobKind::Hisser,
            r if r < 0.56 => MobKind::Groaner,
            r if r < 0.74 => MobKind::Rattler,
            r if r < 0.86 => MobKind::Webber,
            r if r < 0.94 => MobKind::Bloop,
            _ => MobKind::Starer,
        };
        let size = if kind == MobKind::Bloop { [1, 2, 2, 4][self.rng.int(0, 3) as usize] } else { 1 };
        // Starers (and big Bloops) are tall: they need an extra block of headroom.
        let tall = kind == MobKind::Starer || size == 4;
        let clear = |w: &World, y: i32| clear(w, y) && (!tall || !is_solid(w.get(x, y + 2, z)));
        // Night, or a storm dark enough for monsters.
        let dark = self.is_night() || self.weather.kind == crate::weather::Weather::Thunder;
        // Torchlight keeps monsters away (block light 8 or more).
        let torchlit = |w: &World, y: i32| w.block_level(x, y, z) >= 8;
        if dark && is_solid(top) && clear(&self.world, y + 1) && !torchlit(&self.world, y + 1) {
            let pos = Vec3::new(x as f32 + 0.5, y as f32 + 1.0, z as f32 + 0.5);
            self.alloc_mob_sized(kind, pos, size);
            return;
        }
        // Caves are always spooky (but no horses down there).
        if kind == MobKind::Rotsteed {
            return;
        }
        let cy = self.rng.int(4, y.max(5));
        if cy + 1 < y && is_solid(self.world.get(x, cy - 1, z)) && clear(&self.world, cy) && self.world.sky_light(x, cy, z) < 0.15 && !torchlit(&self.world, cy) {
            let pos = Vec3::new(x as f32 + 0.5, cy as f32, z as f32 + 0.5);
            self.alloc_mob_sized(kind, pos, size);
        }
    }

    /// The Scorchlands have their own residents, light or dark.
    fn scorch_spawn(&mut self, p: Vec3) {
        let here = self.mobs.iter().filter(|m| crate::scorch::in_scorch(m.body.pos.x)).count();
        if here >= 10 + 3 * self.peers.len() || !self.rules.difficulty.monsters() {
            return;
        }
        let a = self.rng.range(0.0, TAU);
        let d = self.rng.range(16.0, 40.0);
        let (x, z) = ((p.x + a.cos() * d).floor() as i32, (p.z + a.sin() * d).floor() as i32);
        if !self.world.is_loaded(x, z) || crate::scorch::in_wall(x) || x < crate::scorch::SCORCH_X + 16 {
            return;
        }
        let sea = crate::scorch::LAVA_SEA;
        let kind_count = |g: &Game, k: MobKind| g.mobs.iter().filter(|m| m.kind == k).count();
        // Strutters stroll on the lava sea.
        if self.rng.chance(0.2) && is_lava(self.world.get(x, sea, z)) && self.world.get(x, sea + 1, z) == AIR && kind_count(self, MobKind::Strutter) < 6 {
            for i in 0..self.rng.int(1, 2) {
                self.alloc_mob(MobKind::Strutter, Vec3::new(x as f32 + 0.5 + i as f32, (sea + 1) as f32, z as f32 + 0.5));
            }
            return;
        }
        // Weepers drift in the big open spaces.
        if self.rng.chance(0.15) && kind_count(self, MobKind::Weeper) < 3 {
            let y = self.rng.int(sea + 8, sea + 24);
            let open = [(-3, -3), (3, -3), (-3, 3), (3, 3), (0, 0)].iter().all(|&(dx, dz)| (-1..5).all(|dy| self.world.get(x + dx, y + dy, z + dz) == AIR));
            if open {
                self.alloc_mob(MobKind::Weeper, Vec3::new(x as f32 + 0.5, y as f32, z as f32 + 0.5));
                return;
            }
        }
        let y0 = self.rng.int(sea + 1, CH - 10);
        for y in y0..y0 + 12 {
            let floor = self.world.get(x, y - 1, z);
            let clear = (0..3).all(|h| self.world.get(x, y + h, z) == AIR);
            if matches!(floor, SCORCHROCK | EMBERSAND | GILDED_SCORCHROCK | SCORCH_BRICKS) && clear {
                let at = Vec3::new(x as f32 + 0.5, y as f32, z as f32 + 0.5);
                // Snouts keep near their camps.
                let camp = self.world.generator.nearest_site(crate::structures::Kind::SnoutCamp, at, 2).is_some_and(|o| o.as_vec3().distance(at) < 24.0);
                let roll = self.rng.f32();
                if camp && roll < 0.7 {
                    for i in 0..self.rng.int(1, 3) {
                        self.alloc_mob(MobKind::Snout, at + Vec3::new(i as f32 * 0.7, 0.0, 0.0));
                    }
                } else if roll < 0.45 {
                    for i in 0..self.rng.int(1, 3) {
                        self.alloc_mob(MobKind::Grumbler, at + Vec3::new(i as f32 * 0.7, 0.0, 0.0));
                    }
                } else if roll < 0.65 {
                    self.alloc_mob(MobKind::Snout, at);
                } else if roll < 0.78 {
                    self.alloc_mob_sized(MobKind::Bloop, at, 2);
                } else if roll < 0.92 {
                    self.alloc_mob(MobKind::Rattler, at);
                } else {
                    self.alloc_mob(MobKind::Sizzler, at + Vec3::Y);
                }
                return;
            }
        }
    }

    pub fn respawn(&mut self) {
        self.dead = None;
        self.player = Player::new(self.spawn);
        if self.net.is_none() {
            self.mobs.retain(|m| !m.menacing());
        }
        self.ready = false;
        self.msg(if self.rules.keep_inventory {
            "Respawned. Inventory kept, because this world is nice."
        } else {
            "Respawned. Your stuff is where you fell. It'll wait five minutes. Probably."
        });
    }

    /// Where the player died, if they did (for the death screen).
    pub fn death_spot(&self) -> Option<IVec3> {
        self.dead.as_ref().map(|_| ivec3(self.player.body.pos.x.floor() as i32, self.player.body.pos.y.floor() as i32, self.player.body.pos.z.floor() as i32))
    }

    /// Build all per-frame geometry: sky, clouds, entities, highlights, hand.
    pub fn build_geo(&self, cam: &Camera, render_distance: i32) -> DynGeo {
        let mut g = DynGeo::default();
        let eye = cam.pos;
        let a = self.sun_angle();
        let sun_dir = Vec3::new(a.cos(), a.sin(), 0.25).normalize();

        // Stars fade in at night (no sky at all under the Scorchlands' bedrock).
        let scorch = self.in_scorch();
        // The Hollow's sky is always starry.
        let night = if self.in_hollow() { 1.0 } else { 1.0 - ((self.daylight() - 0.18) / 0.5).clamp(0.0, 1.0) };
        if night > 0.01 && !scorch {
            g.begin(Pass::Sky, [1.0, 1.0, 1.0, night], true);
            let rot = Mat4::from_rotation_z(a);
            for (i, s) in self.stars.iter().enumerate() {
                let d = rot.transform_vector3(*s);
                let size = 0.25 + (i % 3) as f32 * 0.12;
                sky_quad(&mut g, eye + d * 150.0, d, size, T_WHITE);
            }
        }
        g.begin(Pass::Sky, [1.0; 4], true);
        // No sun, moon or clouds anywhere but the ordinary world.
        let scorch = self.elsewhere();
        if !scorch {
            sky_quad(&mut g, eye + sun_dir * 150.0, sun_dir, 16.0, T_SUN);
            sky_quad(&mut g, eye - sun_dir * 150.0, -sun_dir, 11.0, crate::texture::T_MOON_PHASES + self.moon_phase() as u16);
        }
        self.draw_skies(&mut g, eye, sun_dir);
        self.draw_border(&mut g, eye);
        self.draw_lids(&mut g);

        // Clouds: a scrolling blocky layer.
        let cloud_y = 112.0;
        let cell = 12.0;
        let scroll = self.clock * 1.2 + self.time * DAY_SECONDS;
        let (ox, oz) = ((eye.x + scroll) / cell, eye.z / cell);
        let reach = if scorch || !self.clouds_on { -1 } else { ((render_distance * 16) as f32 / cell) as i32 + 4 };
        // Each row's runs of cloudy cells become one strip (Fast) or one box
        // (Fancy). Edges come from whole cells plus one shared fraction, so
        // neighbours meet exactly, and the plain white tile is sampled at its
        // middle so distant (mipmapped) clouds never pick up the tiles around it.
        let shift = (scroll / cell).floor();
        let frac = scroll - shift * cell;
        let edge = |ci: i32| (ci - shift as i32) as f32 * cell - frac;
        let cloudy = |ci: i32, cj: i32| self.clouds.noise2(ci as f32 * 0.17, cj as f32 * 0.17) + hash2(7, ci, cj) * 0.12 >= 0.12;
        let (fi, fj) = (ox.floor() as i32, oz.floor() as i32);
        let fancy = self.fancy_clouds;
        // Boxes cost more; they stop a little sooner (and fade into the fog anyway).
        let reach = if fancy { reach.min(32) } else { reach };
        let thick = 4.0;
        if fancy {
            g.begin(Pass::Opaque, [1.0; 4], false);
            g.no_shadow();
        } else {
            g.begin(Pass::Blend, [1.0, 1.0, 1.0, 0.82], false);
        }
        for j in -reach..=reach {
            let cj = fj + j;
            let (z0, z1) = (cj as f32 * cell, (cj + 1) as f32 * cell);
            let mut i = -reach;
            while i <= reach {
                if !cloudy(fi + i, cj) {
                    i += 1;
                    continue;
                }
                let start = fi + i;
                while i <= reach && cloudy(fi + i, cj) {
                    i += 1;
                }
                let end = fi + i;
                let (x0, x1) = (edge(start), edge(end));
                if !fancy {
                    let c = [Vec3::new(x0, cloud_y, z1), Vec3::new(x1, cloud_y, z1), Vec3::new(x1, cloud_y, z0), Vec3::new(x0, cloud_y, z0)];
                    g.quad(c, T_CLOUD, [0.5, 0.5, 0.5, 0.5], [1.0, 1.0]);
                    continue;
                }
                let (lo, hi) = (Vec3::new(x0, cloud_y, z0), Vec3::new(x1, cloud_y + thick, z1));
                // Top, bottom and the two ends of the run always face open sky.
                for f in [0, 1, 2, 3] {
                    cloud_face(&mut g, f, lo, hi);
                }
                // The long sides only where the next row over has no cloud, in stretches.
                for (f, dj, z) in [(4usize, 1, z1), (5, -1, z0)] {
                    let mut k = start;
                    while k < end {
                        if cloudy(k, cj + dj) {
                            k += 1;
                            continue;
                        }
                        let from = k;
                        while k < end && !cloudy(k, cj + dj) {
                            k += 1;
                        }
                        let (a, b) = (Vec3::new(edge(from), cloud_y, z), Vec3::new(edge(k), cloud_y + thick, z));
                        cloud_face(&mut g, f, a, b);
                    }
                }
            }
        }

        // Mobs
        for m in &self.mobs {
            if m.body.pos.distance(eye) < (render_distance * 16) as f32 {
                m.draw(&mut g, &self.world);
                // A Bell's ring makes raiders glow (see raids.rs).
                if self.bell_glow > 0.0 && m.kind.raider() {
                    m.draw_glow(&mut g, &self.world);
                }
            }
        }
        // Bees about their business (see bees.rs).
        if !self.buzz.is_empty() {
            g.begin(Pass::Opaque, [1.0; 4], false);
            for b in self.buzz.iter().filter(|b| b.pos.distance(eye) < 40.0) {
                let sky = self.world.shade_near(b.pos);
                crate::entity::draw_bee(&mut g, b.pos, b.yaw, b.phase, sky);
            }
        }
        // Pointy Sticks
        if !self.arrows.is_empty() {
            g.begin(Pass::Opaque, [1.0; 4], false);
            for a in self.arrows.iter().filter(|a| a.pos.distance(eye) < (render_distance * 16) as f32) {
                a.draw(&mut g, &self.world, eye);
            }
        }
        // The fishing line and bobber
        if let Some(b) = &self.bobber {
            let at = b.draw_pos(self.clock);
            let sky = self.world.shade_near(at);
            g.begin(Pass::Opaque, [1.0; 4], false);
            let m = Mat4::from_translation(at - Vec3::new(0.08, 0.0, 0.08)) * Mat4::from_scale(Vec3::new(0.16, 0.16, 0.16));
            g.cube(&m, [T_BOBBER; 6], sky, [0.0, 0.0, 1.0, 1.0]);
            // A sagging line from the rod tip (roughly where the held item is).
            let dir = self.player.look_dir();
            let right = dir.cross(Vec3::Y).normalize_or_zero();
            let tip = self.player.eye() + dir * 0.8 + right * 0.3 - Vec3::Y * 0.12;
            let span = at + Vec3::Y * 0.16 - tip;
            let slack = if b.fight.is_some() { 0.0 } else { (span.length() * 0.08).min(1.5) };
            let point = |t: f32| tip + span * t - Vec3::Y * slack * 4.0 * t * (1.0 - t);
            g.begin(Pass::Opaque, [0.9, 0.9, 0.9, 1.0], false);
            let segments = 16;
            for i in 0..segments {
                let (a, b) = (point(i as f32 / segments as f32), point((i + 1) as f32 / segments as f32));
                let d = b - a;
                let rot = macroquad::math::Quat::from_rotation_arc(Vec3::Z, d.normalize_or(Vec3::Z));
                let m = Mat4::from_translation(a) * Mat4::from_quat(rot) * Mat4::from_translation(Vec3::new(-0.01, -0.01, 0.0)) * Mat4::from_scale(Vec3::new(0.02, 0.02, d.length()));
                g.cube(&m, [T_WHITE; 6], sky, [0.0, 0.0, 1.0, 1.0]);
            }
        }
        // Other players
        for p in self.peers.values().filter(|p| p.alive()) {
            let sky = self.world.sky_shade(p.pos.x.floor() as i32, (p.pos.y + 1.0).floor() as i32, p.pos.z.floor() as i32);
            let tint = if p.flags & crate::net::FLAG_HURT != 0 { [1.0, 0.5, 0.5, 1.0] } else { [1.0; 4] };
            g.begin(Pass::Opaque, tint, false);
            let sneak = if p.flags & crate::net::FLAG_SNEAK != 0 { 0.12 } else { 0.0 };
            let gliding = p.flags & crate::net::FLAG_GLIDE != 0;
            // Asleep: lying in the bed they're on.
            let bed = p.pos.floor().as_ivec3();
            let asleep = p.flags & crate::net::FLAG_SLEEP != 0 && crate::beds::is_bed(self.world.get_v(bed));
            let root = if asleep {
                crate::beds::pose(bed, crate::beds::facing(self.world.get_v(bed)))
            } else {
                Mat4::from_translation(p.pos - Vec3::Y * sneak) * Mat4::from_rotation_y(-p.yaw) * glide_pose(gliding)
            };
            draw_model(&mut g, &root, &crate::nametags::SKIN_MODELS[p.skin as usize % 6], p.anim, sky, true);
            crate::entity::draw_armor(&mut g, &root, p.armor, p.trims, p.anim, sky, gliding);
        }
        // The player, in third person
        if self.third_person && !self.menu && !self.spectator {
            let p = &self.player;
            let sky = self.world.sky_shade(p.body.pos.x.floor() as i32, (p.body.pos.y + 1.0).floor() as i32, p.body.pos.z.floor() as i32);
            let tint = if p.hurt > 0.3 { [1.0, 0.5, 0.5, 1.0] } else { [1.0; 4] };
            g.begin(Pass::Opaque, tint, false);
            let root = match self.sleeping {
                Some(s) => crate::beds::pose(s.bed, crate::beds::facing(self.world.get_v(s.bed))),
                None => Mat4::from_translation(p.body.pos) * Mat4::from_rotation_y(-p.yaw) * glide_pose(p.gliding),
            };
            draw_model(&mut g, &root, &crate::nametags::SKIN_MODELS[self.skin as usize % 6], p.bob * 2.0, sky, true);
            crate::entity::draw_armor(&mut g, &root, self.inv.armor_look(), crate::trims::look(&self.inv.armor, &self.inv.armor_wear), p.bob * 2.0, sky, p.gliding);
        }
        // Primed TNT
        for t in &self.tnts {
            let flash = (t.fuse * 8.0).sin() > 0.0;
            g.begin(Pass::Opaque, if flash { [3.0, 3.0, 3.0, 1.0] } else { [1.0; 4] }, false);
            let s = 1.0 + (1.0 - t.fuse.min(1.0)) * 0.15;
            let m = Mat4::from_translation(t.pos + Vec3::splat(0.5)) * Mat4::from_scale(Vec3::splat(s)) * Mat4::from_translation(Vec3::splat(-0.5));
            let sky = self.world.sky_shade(t.pos.x as i32, t.pos.y as i32 + 1, t.pos.z as i32);
            g.cube(&m, [T_TNT_SIDE, T_TNT_SIDE, T_TNT_TOP, T_TNT_BOTTOM, T_TNT_SIDE, T_TNT_SIDE], sky, [0.0, 0.0, 1.0, 1.0]);
        }
        self.draw_falling(&mut g);
        self.draw_fireballs(&mut g);
        // Rain, snow and lightning
        if !scorch {
            self.draw_weather(&mut g, eye);
        }
        self.draw_vehicles(&mut g);
        self.draw_beacons(&mut g, eye, (render_distance * 16) as f32);
        self.draw_frames(&mut g, eye, (render_distance * 16) as f32);
        self.draw_stands(&mut g, eye, 48.0);
        self.draw_banners(&mut g, eye, (render_distance * 16) as f32);
        // Items on the ground, and experience
        self.draw_drops(&mut g, eye, 48.0);
        self.draw_orbs(&mut g, eye, 48.0);
        // Particles
        g.begin(Pass::Opaque, [1.0; 4], false);
        let spark = |p: &&crate::entity::Particle| (crate::texture::T_SPARK_FIRST..crate::texture::T_SPARK_FIRST + crate::fireworks::COLOURS as u16).contains(&p.tile);
        for p in self.particles.iter().filter(|p| !spark(p)) {
            p.draw(&mut g, &self.world);
        }
        // Firework sparks glow, day or night.
        g.begin(Pass::Opaque, [2.2, 2.2, 2.2, 1.0], false);
        for p in self.particles.iter().filter(spark) {
            p.draw_lit(&mut g);
        }

        if self.menu || self.dead.is_some() {
            return g;
        }

        // Block highlight and cracks
        if let Some(Target::Block(h)) = &self.target {
            let id = self.world.get_v(h.pos);
            let (min, max) = if block(id).model == Model::Cross {
                (h.pos.as_vec3() + Vec3::new(0.2, 0.0, 0.2), h.pos.as_vec3() + Vec3::new(0.8, 0.8, 0.8))
            } else if block(id).model == Model::Shaped {
                // Around all of its boxes.
                let (boxes, n) = block_boxes(id);
                let lo = boxes[..n].iter().fold(Vec3::ONE, |m, b| m.min(Vec3::from_array(b.0)));
                let hi = boxes[..n].iter().fold(Vec3::ZERO, |m, b| m.max(Vec3::from_array(b.1)));
                (h.pos.as_vec3() + lo, h.pos.as_vec3() + hi)
            } else {
                (h.pos.as_vec3(), h.pos.as_vec3() + Vec3::ONE)
            };
            g.begin(Pass::Blend, [0.0, 0.0, 0.0, 0.55], true);
            // The edges straddle the block's surface: their outer faces sit just
            // outside it (so they never z-fight) and their inner faces just inside
            // it (so there's no gap between outline and block).
            outline(&mut g, min - Vec3::splat(0.009), max + Vec3::splat(0.009), 0.012);
            if let Some((bp, prog)) = self.breaking
                && bp == h.pos {
                    let stage = ((prog * 5.0) as u16).min(4);
                    g.begin(Pass::Blend, [1.0; 4], false);
                    let m = Mat4::from_translation(min - Vec3::splat(0.006)) * Mat4::from_scale(max - min + Vec3::splat(0.012));
                    g.cube(&m, [T_CRACK0 + stage; 6], 1.0, [0.0, 0.0, 1.0, 1.0]);
                }
        }

        if !self.third_person && !self.spectator {
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
        let sky = self.world.sky_shade(e.x.floor() as i32, e.y.floor() as i32, e.z.floor() as i32);
        let s = self.player.swing;
        let swing = (s * PI).sin();
        let bob = self.player.bob;
        let local = Mat4::from_translation(Vec3::new(0.42 - swing * 0.15, -0.42 + (bob * 2.0).sin().abs() * 0.03 + swing * 0.12, -0.72 - swing * 0.1))
            * Mat4::from_rotation_x(-swing * 0.9);
        let held = self.inv.held();
        let tint = if self.player.hurt > 0.3 { [1.0, 0.6, 0.6, 1.0] } else { [1.0; 4] };
        g.begin(Pass::Overlay, tint, false);
        if held == AIR {
            // The forearm rises from below the bottom-right corner toward the
            // crosshair, like a raised fist; the sleeve covers its near end.
            // The overlay draws without depth, so the far part (the hand) goes
            // first and the nearer sleeve over it.
            let bobv = (bob * 2.0).sin().abs() * 0.02;
            let base = Vec3::new(0.56 - swing * 0.18, -0.62 + bobv + swing * 0.1, -0.5 - swing * 0.12);
            let tip = Vec3::new(0.34 - swing * 0.12, -0.3 + bobv + swing * 0.22, -0.98 - swing * 0.2);
            let len = base.distance(tip);
            let z = (base - tip) / len;
            let x = Vec3::Y.cross(z).normalize();
            let y = z.cross(x);
            let frame = basis * Mat4::from_cols(x.extend(0.0), y.extend(0.0), z.extend(0.0), base.extend(1.0)) * Mat4::from_rotation_z(-0.3);
            let w = 0.15;
            let hand = frame * Mat4::from_translation(Vec3::new(-w / 2.0, -w / 2.0, -len)) * Mat4::from_scale(Vec3::new(w, w, len));
            let sw = w + 0.016;
            let sleeve = frame * Mat4::from_translation(Vec3::new(-sw / 2.0, -sw / 2.0, -len * 0.45)) * Mat4::from_scale(Vec3::new(sw, sw, len * 0.45 + 0.3));
            let light = sky;
            let [tone, _, shirt, _] = crate::nametags::skin_tiles(self.skin as u16 % 6);
            g.cube(&hand, [tone; 6], light, [0.0, 0.0, 1.0, 1.0]);
            g.cube(&sleeve, [shirt; 6], light, [0.0, 0.0, 1.0, 1.0]);
        } else if is_block_item(held) && matches!(block(held).model, Model::Cube | Model::Shaped) && !matches!(block(held).shape, Shape::Dust) {
            let tiles = {
                let t = block(held).tex;
                [t[1], t[1], t[0], t[2], t[1], t[1]]
            };
            let (boxes, n) = block_boxes(held);
            for &(a, b) in &boxes[..n] {
                let (a, b) = (Vec3::from_array(a), Vec3::from_array(b));
                let m = basis * local * Mat4::from_rotation_y(0.75) * Mat4::from_translation(Vec3::splat(-0.12)) * Mat4::from_scale(Vec3::splat(0.24)) * Mat4::from_translation(a) * Mat4::from_scale(b - a);
                g.cube(&m, tiles, sky, [a.x, 1.0 - b.y, b.x, 1.0 - a.y]);
            }
        } else {
            let tile = if is_block_item(held) { block(held).tex[1] } else { item_tile(held) };
            // Tools and weapons (anything that wears out) are gripped by the handle
            // and tilted up and in; other things are held up flat to look at.
            let tool = crate::block::durability(held).is_some();
            let m = if tool {
                // Seen from behind, so the handle sits in the hand at the bottom right.
                basis * local * Mat4::from_translation(Vec3::new(-0.02, 0.1, 0.0)) * Mat4::from_rotation_y(PI - 0.9) * Mat4::from_rotation_z(-0.1)
            } else {
                basis * local * Mat4::from_translation(Vec3::new(0.0, 0.08, 0.0)) * Mat4::from_rotation_y(-0.5) * Mat4::from_rotation_z(0.2)
            };
            extruded_sprite(g, &m, tile, if tool { 0.46 } else { 0.3 }, sky);
            // Our banner, painted on the shield (see banners.rs).
            if held == SHIELD && self.shield_banner != 0 {
                let cloth = m * Mat4::from_translation(Vec3::new(-0.12, -0.17, 0.03)) * Mat4::from_scale(Vec3::new(0.24, 0.34, 1.0));
                crate::banners::draw_cloth(g, &cloth, crate::banners::Design::from_bits(self.shield_banner), sky, Pass::Overlay);
            }
        }
    }

    pub fn frame_params(&self, cam: &Camera, renderer: &Renderer, render_distance: i32) -> FrameParams {
        let underwater = !self.menu && self.player.head_in_water(&self.world);
        let sky = self.sky_color();
        let far = (render_distance * 16) as f32;
        let far_land = self.distant_terrain && renderer.has_far() && !self.in_scorch() && !self.in_hollow();
        let (fog_color, fog_start, fog_end) = if underwater {
            // A Turtle Shell lets you see a good way further.
            let shell = self.inv.armor[0].is_some_and(|(id, _)| id == TURTLE_SHELL);
            ([0.05, 0.12, 0.35], 0.0, if shell { 48.0 } else { 22.0 })
        } else if self.in_scorch() {
            // Hazy, hot air.
            (sky, far * 0.2, far * 0.8)
        } else if self.in_hollow() {
            ([0.1, 0.05, 0.14], far * 0.4, far)
        } else if !self.fog_on {
            (sky, 0.0, 0.0)
        } else if far_land {
            // The haze starts later and reaches the far-off land's edge.
            let lod = crate::lod::far_for(render_distance) as f32;
            (sky, far * 0.7, lod - 8.0)
        } else {
            (sky, far * 0.55, far - 4.0)
        };
        // Held torches glow too.
        let mut extra = Vec::new();
        let held = self.inv.held();
        if !self.menu && is_block_item(held) && block(held).light > 0.0 {
            let e = self.player.eye();
            extra.push([e.x, e.y, e.z, -block(held).light * 0.8]);
        }
        let lights: [Vec4; 16] = renderer.nearby_lights(cam.pos, &extra);
        FrameParams { view_proj: cam.view_proj, cam_pos: cam.pos, fog_color, fog_start, fog_end, far_land: far_land && !underwater, daylight: self.daylight(), ambient: if self.has_effect(crate::potions::Potion::NightVision) { 0.7 } else if self.in_scorch() { 0.32 } else if self.in_hollow() { 0.45 } else { 0.0 }, lights, colour_blind: self.colour_blind, waving_leaves: self.waving_leaves, water_reflections: self.water_reflections, time: self.clock, shadows: self.shadows && !self.in_scorch() && !self.in_hollow(), sun_dir: { let a = sun_step(self.sun_angle()); Vec3::new(a.cos(), a.sin(), 0.25).normalize() }, fancy_water: self.fancy_water }
    }
}

/// One face of a cloud box between `lo` and `hi` (face order as `mesher::FACES`:
/// +x, -x, +y, -y, +z, -z), shaded like a block face so the shape reads.
fn cloud_face(g: &mut DynGeo, f: usize, lo: Vec3, hi: Vec3) {
    let (_, corners, shade) = crate::mesher::FACES[f];
    let size = hi - lo;
    // A side given as a flat slice (no depth) still spans its full height and length.
    let c = corners.map(|p| lo + Vec3::from_array(p) * size);
    g.quad(c, T_CLOUD, [0.5, 0.5, 0.5, 0.5], [0.72 + 0.28 * shade, 1.0]);
}

/// A sprite given one pixel of thickness, like Minecraft's held items: its
/// face front and back, and a thin side wherever a solid pixel meets a clear
/// one. `m` places the sprite (centred, `size` across, facing +z).
/// While this is in hand, the other hand gets used instead (a shield blocks, a block is placed).
pub fn offhand_first(held: Id) -> bool {
    held == AIR || is_sword(held) || held == MACE || pick_tier(held) > 0 || crate::tools::tool_uses(held).is_some()
}

fn extruded_sprite(g: &mut DynGeo, m: &Mat4, tile: u16, size: f32, sky: f32) {
    use crate::texture::{solid, TILE};
    let px = size / TILE as f32;
    let (z0, z1) = (-px / 2.0, px / 2.0);
    let at = |x: f32, y: f32, z: f32| m.transform_point3(Vec3::new(x * px - size / 2.0, size / 2.0 - y * px, z));
    let n = TILE as i32;
    let filled = |x: i32, y: i32| (0..n).contains(&x) && (0..n).contains(&y) && solid(tile, x as usize, y as usize);
    // The sides first: the overlay pass has no depth test, so the faces go on top.
    for y in 0..n {
        for x in 0..n {
            if !filled(x, y) {
                continue;
            }
            // Just this pixel's colour, from the middle of it.
            let t = TILE as f32;
            let uv = [(x as f32 + 0.25) / t, (y as f32 + 0.25) / t, (x as f32 + 0.75) / t, (y as f32 + 0.75) / t];
            let (fx, fy) = (x as f32, y as f32);
            for (dx, dy, a, b) in [(-1, 0, (fx, fy), (fx, fy + 1.0)), (1, 0, (fx + 1.0, fy), (fx + 1.0, fy + 1.0)), (0, -1, (fx, fy), (fx + 1.0, fy)), (0, 1, (fx, fy + 1.0), (fx + 1.0, fy + 1.0))] {
                if filled(x + dx, y + dy) {
                    continue;
                }
                let q = [at(a.0, a.1, z0), at(b.0, b.1, z0), at(b.0, b.1, z1), at(a.0, a.1, z1)];
                // Both windings: culling keeps whichever faces the camera.
                g.quad(q, tile, uv, [0.7, sky]);
                g.quad([q[1], q[0], q[3], q[2]], tile, uv, [0.7, sky]);
            }
        }
    }
    let t = TILE as f32;
    let front = [at(0.0, t, z1), at(t, t, z1), at(t, 0.0, z1), at(0.0, 0.0, z1)];
    let back = [at(t, t, z0), at(0.0, t, z0), at(0.0, 0.0, z0), at(t, 0.0, z0)];
    g.quad(back, tile, [1.0, 0.0, 0.0, 1.0], [0.85, sky]);
    g.quad(front, tile, [0.0, 0.0, 1.0, 1.0], [1.0, sky]);
}

/// Names of every mod-added block and item, so saves survive mods being added or removed.
pub(crate) fn mod_palette(r: &Registry) -> Vec<(Id, String)> {
    let blocks = (NUM_BLOCKS..r.blocks.len() as Id).map(|id| (id, r.blocks[id as usize].key.to_string()));
    let items = (FIRST_MOD_ITEM as usize..FIRST_ITEM as usize + r.items.len()).map(|id| (id as Id, r.key_of(id as Id).to_string()));
    blocks.chain(items).collect()
}

/// Map ids in a save to ids in the current registry. Mod things that no longer
/// exist become air (blocks) or vanish (items). None when nothing needs changing.
pub(crate) fn palette_remap(r: &Registry, palette: &[(Id, String)]) -> Option<Vec<Id>> {
    let mut map: Vec<Id> = (0..=Id::MAX).collect();
    // Any mod-range id the save doesn't mention is unknown.
    for id in NUM_BLOCKS..FIRST_ITEM {
        map[id as usize] = AIR;
    }
    for id in FIRST_MOD_ITEM..=Id::MAX {
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

/// The sun's angle in small steps (a second or so of daytime apart): if the
/// sun's camera turned a little every frame, every shadow edge would shimmer
/// as the shadow map's texels slid across the world.
pub fn sun_step(a: f32) -> f32 {
    const STEP: f32 = TAU / 1200.0;
    (a / STEP).round() * STEP
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

/// Gliding players lie flat, head first (turned about the middle of the body).
fn glide_pose(gliding: bool) -> Mat4 {
    if !gliding {
        return Mat4::IDENTITY;
    }
    let mid = Vec3::Y * 0.9;
    Mat4::from_translation(mid) * Mat4::from_rotation_x(-std::f32::consts::FRAC_PI_2) * Mat4::from_translation(-mid)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::inventory::Inventory;

    /// A world with the 3x3 chunks around the origin generated (x and z in -16..32).
    fn loaded_world(seed: u32) -> World {
        // Exactly these nine chunks, made here in a fixed order: waiting on the
        // generator threads let a varying number of extra chunks arrive, and
        // anything done per loaded chunk (random ticks) then played out
        // differently from run to run.
        let mut w = World::new(seed);
        for cz in -1..=1 {
            for cx in -1..=1 {
                w.load_now(cx, cz);
            }
        }
        w
    }

    #[test]
    fn water_under_a_block_keeps_its_surface() {
        let mut w = loaded_world(7);
        let (x, y, z) = (5, crate::world::CH - 12, 5);
        // Shallow water boxed in on every side, with a ceiling a little above it.
        for (dx, dy, dz) in [(1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 0, -1), (0, -1, 0), (0, 1, 0)] {
            w.set(x + dx, y + dy, z + dz, crate::block::STONE);
        }
        w.set(x, y, z, crate::block::liquid_at(false, 3));
        let surface = |w: &World| crate::mesher::mesh_chunk(w, 0, 0).water.verts.chunks(4).filter(|q| q.iter().all(|v| v.pos[1] > y as f32 + 0.3 && v.pos[1] < y as f32 + 1.0)).count();
        assert_eq!(surface(&w), 1, "the surface shows below the ceiling");
        // Water filling its cell right up to the block above has nothing to show.
        w.set(x, y, z, crate::block::WATER);
        w.set(x, y + 1, z, crate::block::WATER);
        w.set(x, y + 2, z, crate::block::STONE);
        assert_eq!(surface(&w), 0);
    }

    #[test]
    fn greedy_meshing_shrinks_real_terrain() {
        let w = loaded_world(7);
        let mesh = crate::mesher::mesh_chunk(&w, 0, 0);
        let merged = mesh.opaque.verts.iter().filter(|v| v.tile[0] >= 0.0).count() / 4;
        let quads = mesh.opaque.verts.len() / 4;
        // Count what one quad per visible cube face would have cost.
        let mut faces = 0;
        for y in 0..crate::world::CH {
            for z in 0..16 {
                for x in 0..16 {
                    let id = w.get(x, y, z);
                    if block(id).model != crate::block::Model::Cube {
                        continue;
                    }
                    for (n, _, _) in crate::mesher::FACES {
                        if !(is_opaque(w.get(x + n[0], y + n[1], z + n[2])) || w.get(x + n[0], y + n[1], z + n[2]) == id && block(id).see_through) {
                            faces += 1;
                        }
                    }
                }
            }
        }
        // This chunk is a stepped hill (little to merge), yet its cube faces still
        // come out as fewer quads (plants and other shapes aren't counted in `faces`).
        assert!(merged > 0 && merged < faces, "{quads} quads ({merged} cube) for {faces} faces");
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
    fn old_saves_keep_their_mod_things_after_new_base_blocks() {
        use crate::mods::{build, ModSource};
        let mut files = std::collections::BTreeMap::new();
        files.insert("mod.txt".to_string(), b"[block one]\n[item gem]\n".to_vec());
        let r = build(&[ModSource { id: "aaa".into(), files }], &[]);
        // Before the parody update, the first mod block was id 24 and the first mod
        // item one-byte id 118 (widened on load like everything in an old save).
        let gem_old = crate::save::legacy_id(118);
        let old_palette = vec![(24, "aaa:one".to_string()), (gem_old, "aaa:gem".to_string())];
        let map = palette_remap(&r, &old_palette).expect("ids moved");
        assert_eq!(map[24], r.lookup("aaa:one").unwrap());
        assert_eq!(map[24], NUM_BLOCKS);
        assert_eq!(map[gem_old as usize], r.lookup("aaa:gem").unwrap());
        assert_eq!(map[gem_old as usize], FIRST_MOD_ITEM);
        assert_eq!(map[PEARL as usize], PEARL, "base items the palette doesn't mention stay put");
        assert_eq!(map[FIRST_MOD_ITEM as usize + 5], AIR, "unknown mod ids become nothing");
        assert_eq!(map[DIAMOND_ORE as usize], DIAMOND_ORE);
    }

    #[test]
    fn mob_kinds_round_trip() {
        for k in MobKind::ALL {
            assert_eq!(MobKind::from_index(k.index()), Some(k));
            assert_eq!(MobKind::from_name(k.name()), Some(k));
        }
        assert_eq!(MobKind::from_name("enderman"), Some(MobKind::Starer));
        assert_eq!(MobKind::from_name("sheep"), Some(MobKind::Fluffer));
        assert!(MobKind::from_index(99).is_none());
        let mut rng = Rng::new(1);
        let s = Mob::new(MobKind::Starer, Vec3::new(0.0, 0.0, -10.0), &mut rng);
        let eye = Vec3::new(0.0, 2.65, 0.0);
        assert!(s.stared_at(eye, -Vec3::Z), "looking straight at its face");
        assert!(!s.stared_at(eye, Vec3::new(0.3, 0.0, -1.0).normalize()), "looking past it");
        assert!(!s.stared_at(eye, Vec3::Z), "looking away");
    }

    #[test]
    fn idle_mobs_settle_into_a_standing_pose() {
        let mut a = 0.0;
        crate::entity::step_anim(&mut a, 3.0, 0.3, 5.0);
        assert!(a.sin().abs() > 0.5, "mid-stride while walking");
        for _ in 0..60 {
            crate::entity::step_anim(&mut a, 0.0, 1.0 / 60.0, 5.0);
        }
        assert!(a.sin().abs() < 1e-3, "limbs straight once stopped, not frozen mid-stride");
        let settled = a;
        crate::entity::step_anim(&mut a, 0.1, 1.0 / 60.0, 5.0);
        assert_eq!(a, settled, "drifting slowly doesn't shuffle the legs");
    }

    #[test]
    fn light_levels_look_steadily_brighter() {
        use crate::light::{brightness, darkest, shade, DEFAULT_BRIGHTNESS};
        assert_eq!(brightness(), DEFAULT_BRIGHTNESS);
        assert!((shade(0) - darkest(DEFAULT_BRIGHTNESS)).abs() < 1e-6);
        assert!((shade(15) - 1.0).abs() < 1e-6);
        for l in 0..15 {
            assert!(shade(l + 1) > shade(l), "level {l}");
        }
    }

    #[test]
    fn parody_blocks_and_advancements() {
        let mut g = Game::new(11, false, false);
        g.world = loaded_world(11);
        g.ready = true;
        let top = g.world.surface_y(2, 2);
        let base = IVec3::new(2, top + 1, 2);
        let clear = |g: &mut Game| {
            for y in 0..6 {
                for z in -4..=4 {
                    for x in -4..=4 {
                        g.world.set(2 + x, top + 1 + y, 2 + z, AIR);
                    }
                }
            }
        };
        clear(&mut g);

        // Items grant advancements when you get them.
        g.give(LOG, 1);
        assert!(g.advancements.has("getting_wood"));
        assert_eq!(g.toasts.len(), 1);
        g.give(LOG, 1);
        assert_eq!(g.toasts.len(), 1, "only once");

        // Cake fills you up and disappears.
        g.world.set_v(base, CAKE);
        g.player.hunger.food = 4.0;
        g.target = Some(Target::Block(crate::world::Hit { pos: base, normal: IVec3::Y, dist: 1.0 }));
        g.use_item();
        assert_eq!(g.world.get_v(base), AIR);
        assert_eq!(g.player.hunger.food, 18.0);
        assert!(g.advancements.has("cake"));

        // Beds skip the night (and not the day).
        g.world.set_v(base, BED);
        g.time = 0.25;
        g.sleep(base);
        assert_eq!(g.time, 0.25);
        g.time = 0.7;
        g.sleep(base);
        // (It takes a few seconds in bed.)
        for _ in 0..100 {
            g.sleep_tick(0.05, false);
        }
        assert!(g.time < 0.05 && !g.is_night());
        assert!(g.advancements.has("sweet_dreams"));
        assert_eq!(g.spawn, base.as_vec3() + Vec3::new(0.5, 1.0, 0.5));

        // Sponges drink water.
        clear(&mut g);
        for x in 0..3 {
            g.world.set(1 + x, top + 1, 4, WATER);
        }
        g.soak(base);
        assert!((0..3).all(|x| g.world.get(1 + x, top + 1, 4) == AIR));
        assert!(g.advancements.has("thirsty"));

        // Broken ice leaves water behind.
        g.world.set_v(base, ICE);
        g.break_block(base, true);
        assert_eq!(g.world.get_v(base), WATER);
        g.world.set_v(base, AIR);

        // Stare Pearls teleport you to where you look (and sting a bit).
        g.player.body.pos = Vec3::new(2.5, top as f32 + 1.0, 2.5);
        g.player.health = 20.0;
        g.player.yaw = 0.0; // -Z
        g.player.pitch = -0.6;
        g.inv.slots[g.inv.selected] = Some((PEARL, 2));
        g.throw_pearl();
        let moved = g.player.body.pos.distance(Vec3::new(2.5, top as f32 + 1.0, 2.5));
        assert!(moved > 1.0, "moved {moved}");
        assert_eq!(g.player.health, 18.0);
        assert_eq!(g.inv.count(PEARL), 1);
        assert!(g.advancements.has("rude_teleport"));

        // Pokey plants poke.
        g.player.hurt = 0.0;
        let p = g.player.body.pos;
        g.world.set(p.x.floor() as i32 + 1, p.y.floor() as i32, p.z.floor() as i32, CACTUS);
        g.player.body.pos.x = p.x.floor() + 0.65;
        let before = g.player.health;
        g.block_effects();
        assert_eq!(g.player.health, before - 1.0);
        assert!(g.advancements.has("ouch"));

        // And it all goes in the save.
        let d = g.to_save();
        let back = Game::from_save(d);
        assert_eq!(back.advancements.count(), g.advancements.count());
    }

    #[test]
    fn chests_hold_things_and_furnaces_cook() {
        use crate::containers::{FUEL, INPUT, OUTPUT};
        let mut g = arena(31);
        let chest = IVec3::new(2, 50, 0);
        g.world.set_v(chest, CHEST);
        assert_eq!(g.world.containers[&chest].slots.len(), 27);
        // Right-clicking it opens it.
        g.player.body.pos = Vec3::new(0.5, 50.0, 0.5);
        g.target = Some(Target::Block(crate::world::Hit { pos: chest, normal: IVec3::NEG_X, dist: 1.5 }));
        g.use_item();
        assert_eq!(g.open, Some(chest));
        // Click a stack in; shift-click another straight in.
        g.inv.slots[0] = Some((DIAMOND, 5));
        g.inv.click(0);
        g.container_click(0, false, false);
        assert_eq!(g.world.containers[&chest].slots[0], Some((DIAMOND, 5)));
        g.inv.slots[1] = Some((STICK, 10));
        g.container_quick_put(1);
        assert_eq!(g.world.containers[&chest].slots[1], Some((STICK, 10)));
        assert_eq!(g.inv.count(STICK), 0);
        // Shift-click back out.
        g.container_click(1, false, true);
        assert_eq!(g.inv.count(STICK), 10);
        g.close_container();

        // It survives saving.
        let back = Game::from_save(g.to_save());
        assert_eq!(back.world.containers[&chest].slots[0], Some((DIAMOND, 5)));

        // Breaking it spills the contents (and the chest) on the ground.
        g.break_block(chest, true);
        assert!(!g.world.containers.contains_key(&chest));
        assert_eq!(g.inv.count(DIAMOND), 0);
        collect(&mut g);
        assert_eq!(g.inv.count(DIAMOND), 5);
        assert_eq!(g.inv.count(CHEST), 1);

        // A furnace: chops on top, coal below, and it lights up while it works.
        let furnace = IVec3::new(-2, 50, 0);
        g.world.set_v(furnace, FURNACE);
        let f = g.world.containers.get_mut(&furnace).unwrap();
        f.slots[INPUT] = Some((PORKCHOP, 2));
        f.slots[FUEL] = Some((COAL, 1));
        for _ in 0..90 {
            g.container_tick(0.1);
        }
        assert_eq!(g.world.get_v(furnace), FURNACE_LIT);
        assert_eq!(g.world.containers[&furnace].slots[OUTPUT], Some((COOKED_CHOP, 1)));
        // The output is take-only.
        g.open = Some(furnace);
        g.inv.cursor = Some((DIRT, 1));
        g.container_click(OUTPUT, false, false);
        assert_eq!(g.inv.cursor, Some((DIRT, 1)));
        g.inv.cursor = None;
        g.container_click(OUTPUT, false, false);
        assert_eq!(g.inv.cursor, Some((COOKED_CHOP, 1)));
        g.close_container();
        // Out of things to cook: it goes out once the coal's spent.
        for _ in 0..900 {
            g.container_tick(0.1);
        }
        assert_eq!(g.world.get_v(furnace), FURNACE);
        assert_eq!(g.world.containers[&furnace].slots[OUTPUT], Some((COOKED_CHOP, 1)));
        assert_eq!(g.world.containers[&furnace].slots[INPUT], None);
    }

    pub(crate) fn aim(g: &mut Game, pos: IVec3, normal: IVec3) {
        let dist = (pos.as_vec3() + Vec3::splat(0.5)).distance(g.player.eye());
        g.target = Some(Target::Block(crate::world::Hit { pos, normal, dist }));
    }

    /// Turn the player to look at a point (for things that raycast themselves).
    fn look_at(g: &mut Game, at: Vec3) {
        let d = at - g.player.eye();
        g.player.yaw = d.x.atan2(-d.z);
        g.player.pitch = d.y.atan2(Vec3::new(d.x, 0.0, d.z).length());
    }

    #[test]
    fn slabs_stairs_and_doors() {
        let mut g = arena(41);
        g.player.yaw = 0.0; // looking north (-z)
        g.player.pitch = 0.0;
        // Stairs placed on the floor rise away from you.
        g.inv.slots[g.inv.selected] = Some((stairs(0, 0), 4));
        aim(&mut g, IVec3::new(0, 49, -3), IVec3::Y);
        g.use_item();
        assert_eq!(g.world.get(0, 50, -3), stairs(0, 0));
        g.player.yaw = std::f32::consts::FRAC_PI_2; // east
        aim(&mut g, IVec3::new(0, 49, -4), IVec3::Y);
        g.use_item();
        assert_eq!(g.world.get(0, 50, -4), stairs(0, 1));
        g.player.yaw = 0.0;
        assert_eq!(g.inv.count(stairs(0, 0)), 2);

        // A slab on the floor sits low; a second one on top makes cobblestone.
        g.inv.slots[g.inv.selected] = Some((slab(1, false), 8));
        aim(&mut g, IVec3::new(1, 49, -3), IVec3::Y);
        g.use_item();
        assert_eq!(g.world.get(1, 50, -3), slab(1, false));
        aim(&mut g, IVec3::new(1, 50, -3), IVec3::Y);
        g.use_item();
        assert_eq!(g.world.get(1, 50, -3), COBBLE);
        // Against the upper half of a wall, it goes on top.
        g.world.set(2, 51, -4, STONE);
        aim(&mut g, IVec3::new(2, 51, -4), IVec3::Z);
        g.use_item();
        assert_eq!(g.world.get(2, 51, -3), slab(1, true));
        assert_eq!(g.inv.count(slab(1, false)), 5);

        // Only the slab's half is there to hit or bump into.
        g.world.set(4, 50, 0, slab(1, false));
        let hit = g.world.raycast(Vec3::new(4.5, 55.0, 0.5), Vec3::NEG_Y, 10.0).unwrap();
        assert_eq!((hit.pos, hit.normal), (IVec3::new(4, 50, 0), IVec3::Y));
        assert!((hit.dist - 4.5).abs() < 1e-3);
        // Walking into it steps up onto it; a full block is a wall.
        g.world.set(5, 50, 0, slab(1, false));
        g.world.set(8, 50, 0, STONE);
        g.world.set(8, 51, 0, STONE);
        let mut b = crate::entity::Body::new(Vec3::new(2.5, 50.0, 0.5), 0.3, 1.8);
        let mut on_slabs = false;
        for _ in 0..60 {
            b.vel.x = 4.0;
            b.vel.y -= crate::entity::GRAVITY * 0.05;
            crate::entity::move_body(&g.world, &mut b, 0.05, false);
            on_slabs |= (4.5..5.5).contains(&b.pos.x) && (b.pos.y - 50.5).abs() < 0.01;
        }
        assert!(on_slabs, "stepped up onto the slabs");
        assert!(b.pos.x < 7.71 && b.pos.x > 7.5, "stopped by the wall: {}", b.pos);

        // Doors: two blocks tall, facing away from you.
        g.inv.slots[g.inv.selected] = Some((DOOR, 2));
        aim(&mut g, IVec3::new(-2, 49, -3), IVec3::Y);
        g.use_item();
        let (bottom, top) = (IVec3::new(-2, 50, -3), IVec3::new(-2, 51, -3));
        assert_eq!((g.world.get_v(bottom), g.world.get_v(top)), (door(0, false, false), door(0, false, true)));
        assert_eq!(g.inv.count(DOOR), 1);
        // Right-click opens both halves; the panel moves to the side.
        aim(&mut g, top, IVec3::Z);
        g.use_item();
        assert_eq!((g.world.get_v(bottom), g.world.get_v(top)), (door(0, true, false), door(0, true, true)));
        assert!(g.advancements.has("open_door_policy"));
        assert_ne!(block_boxes(door(0, true, false)).0[0], block_boxes(door(0, false, false)).0[0]);
        // Breaking either half takes the whole door, and drops one.
        g.break_block(top, true);
        assert_eq!((g.world.get_v(bottom), g.world.get_v(top)), (AIR, AIR));
        collect(&mut g);
        assert_eq!(g.inv.count(DOOR), 2);
        // No door without room for it.
        g.world.set(-3, 51, -3, STONE);
        aim(&mut g, IVec3::new(-3, 49, -3), IVec3::Y);
        g.use_item();
        assert_eq!(g.world.get(-3, 50, -3), AIR);
    }

    #[test]
    fn armour_softens_blows() {
        let mut g = arena(43);
        let chest = ARMOR_FIRST + 3 * 4 + CHESTPLATE as Id; // dimond
        g.inv.slots[g.inv.selected] = Some((chest, 1));
        g.use_item();
        assert_eq!(g.inv.armor[CHESTPLATE], Some((chest, 1)));
        assert_eq!(g.inv.held(), AIR);
        assert_eq!(g.inv.armor_points(), 8);
        g.hurt_player_armored(10.0, "tested armour");
        assert!((g.player.health - (MAX_HEALTH - 6.8)).abs() < 1e-3, "{}", g.player.health);
        // Only the right slot takes it; worn armour still counts as owned.
        g.inv.cursor = Some((chest, 1));
        g.inv.click_armor(HELMET);
        assert_eq!(g.inv.armor[HELMET], None);
        g.inv.cursor = None;
        assert_eq!(g.inv.counts().get(&chest), Some(&1));
        // Shift-click equips from the inventory, swapping out what was worn.
        let iron = ARMOR_FIRST + 4 + CHESTPLATE as Id;
        g.inv.slots[5] = Some((iron, 1));
        assert!(g.inv.equip(5));
        assert_eq!((g.inv.armor[CHESTPLATE], g.inv.slots[5]), (Some((iron, 1)), Some((chest, 1))));
        for slot in [HELMET, LEGGINGS, BOOTS] {
            g.inv.armor[slot] = Some((ARMOR_FIRST + 3 * 4 + slot as Id, 1));
        }
        assert_eq!(g.inv.armor_points(), 3 + 6 + 6 + 3);
        assert_eq!(g.inv.armor_look(), 0x4424);
        // Saved with the world.
        let back = Game::from_save(g.to_save());
        assert_eq!(back.inv.armor, g.inv.armor);
        assert_eq!(back.inv.slots[5], Some((chest, 1)));
        // Falling isn't softened.
        g.player.health = MAX_HEALTH;
        g.player.hurt = 0.0;
        g.hurt_player(4.0, "fell");
        assert_eq!(g.player.health, MAX_HEALTH - 4.0);
    }

    #[test]
    fn items_on_the_ground() {
        let mut g = arena(47);
        // Q throws one; it can't be caught again straight away.
        g.inv.slots[g.inv.selected] = Some((DIRT, 3));
        g.throw_held(false);
        assert_eq!((g.inv.count(DIRT), g.drops.len()), (2, 1));
        assert!(g.advancements.has("butterfingers"));
        g.player.body.pos = g.drops[0].body.pos;
        g.drops_tick(0.1);
        assert_eq!(g.inv.count(DIRT), 2, "too soon");
        for _ in 0..20 {
            g.drops_tick(0.1);
            g.player.body.pos = g.drops.first().map(|d| d.body.pos).unwrap_or(g.player.body.pos);
        }
        assert_eq!((g.inv.count(DIRT), g.drops.len()), (3, 0));
        // Ctrl+Q throws the stack, and things fall and stop on the floor.
        g.player.body.pos = Vec3::new(0.5, 50.0, 0.5);
        g.throw_held(true);
        for _ in 0..30 {
            g.drops[0].age = 0.0; // keep it out of reach of pickup for now
            g.drops_tick(0.1);
        }
        let d = &g.drops[0];
        assert_eq!((d.item, d.n), (DIRT, 3));
        assert!((d.body.pos.y - 50.0).abs() < 0.01 && d.body.on_ground, "{}", d.body.pos);
        // A full inventory leaves things where they are.
        g.drops.clear();
        g.inv.slots = [Some((STONE, 64)); 36];
        g.give(DIRT, 5);
        assert_eq!(g.drops.len(), 1, "what didn't fit is on the floor");
        for _ in 0..30 {
            g.drops_tick(0.1);
            g.player.body.pos = g.drops[0].body.pos;
        }
        assert_eq!(g.drops[0].n, 5);
        // Mobs and explosions drop things too, and it's all saved.
        g.inv.slots = [None; 36];
        g.player.body.pos = Vec3::new(0.5, 50.0, 0.5);
        g.world.set(3, 50, 3, COBBLE);
        g.break_block(IVec3::new(3, 50, 3), true);
        assert!(g.drops.iter().any(|d| d.item == COBBLE));
        let back = Game::from_save(g.to_save());
        assert_eq!(back.drops.len(), g.drops.len());
        // And they don't last forever.
        for d in g.drops.iter_mut() {
            d.age = crate::drops::DESPAWN_SECS;
        }
        g.drops_tick(0.1);
        assert!(g.drops.is_empty());
    }

    #[test]
    fn tools_and_armour_wear_out() {
        let mut g = arena(51);
        // A wooden pickaxe lasts 59 blocks; this one is on its last two.
        g.inv.slots[g.inv.selected] = Some((PICK_WOOD, 1));
        g.inv.wear[g.inv.selected] = 57;
        g.world.set(2, 50, 2, STONE);
        g.break_block(IVec3::new(2, 50, 2), true);
        assert_eq!(g.inv.wear[g.inv.selected], 58);
        // Things that break instantly don't wear it.
        g.world.set(3, 50, 2, TORCH);
        g.break_block(IVec3::new(3, 50, 2), true);
        assert_eq!(g.inv.wear[g.inv.selected], 58);
        g.world.set(2, 50, 2, STONE);
        g.break_block(IVec3::new(2, 50, 2), true);
        assert_eq!(g.inv.held(), AIR, "worn out");
        assert!(g.messages.iter().any(|m| m.0.contains("broke")));
        // Swords are bad shovels.
        g.inv.slots[g.inv.selected] = Some((SWORD_IRON, 1));
        g.world.set(2, 50, 2, DIRT);
        g.break_block(IVec3::new(2, 50, 2), true);
        assert_eq!(g.inv.wear[g.inv.selected], 2);

        // Wear follows the item: onto the cursor, into another slot, into a chest and back.
        g.inv.click(g.inv.selected);
        assert_eq!((g.inv.cursor, g.inv.cursor_wear), (Some((SWORD_IRON, 1)), 2));
        g.inv.click(20);
        assert_eq!((g.inv.slots[20], g.inv.wear[20]), (Some((SWORD_IRON, 1)), 2));
        let chest = IVec3::new(-2, 50, 0);
        g.world.set_v(chest, CHEST);
        g.open = Some(chest);
        g.container_quick_put(20);
        assert_eq!(g.world.containers[&chest].wear[0], 2);
        g.container_click(0, false, true);
        let slot = g.inv.slots.iter().position(|s| *s == Some((SWORD_IRON, 1))).unwrap();
        assert_eq!(g.inv.wear[slot], 2);
        g.close_container();
        // ...and onto the ground and back, and through a save.
        g.inv.selected = slot;
        g.throw_held(false);
        assert_eq!(g.drops.iter().find(|d| d.item == SWORD_IRON).map(|d| d.wear), Some(2));
        collect(&mut g);
        let slot = g.inv.slots.iter().position(|s| *s == Some((SWORD_IRON, 1))).unwrap();
        assert_eq!(g.inv.wear[slot], 2);
        let back = Game::from_save(g.to_save());
        assert_eq!(back.inv.wear[slot], 2);

        // Armour wears a little with every hit it softens.
        let boots = ARMOR_FIRST + BOOTS as Id; // Woolly Socks: 65 uses
        g.inv.armor[BOOTS] = Some((boots, 1));
        g.inv.armor_wear[BOOTS] = 63;
        g.hurt_player_armored(4.0, "tested");
        assert_eq!(g.inv.armor_wear[BOOTS], 64);
        g.player.hurt = 0.0;
        g.hurt_player_armored(4.0, "tested");
        assert_eq!(g.inv.armor[BOOTS], None, "the socks gave up");
        // Creative never wears anything.
        g.creative = true;
        g.inv.slots[0] = Some((PICK_STONE, 1));
        g.inv.selected = 0;
        g.inv.wear[0] = 0;
        g.use_tool(5);
        assert_eq!(g.inv.wear[0], 0);
    }

    #[test]
    fn mod_tools_wear_out_and_an_anvil_mends_them() {
        // A mod tool with its own durability and repair material. Mining wears
        // it through the normal path, and an anvil mends it with that material.
        let mod_txt = "\
[item ruby]

[item ruby_pick]
durability = 20
repair = ruby

[item ruby_blade]
damage = 7
durability = 900
repair = ruby
";
        crate::mods::with_mods(&[("gems", mod_txt)], |reg| {
            let ruby = reg.lookup("gems:ruby").unwrap();
            let pick = reg.lookup("gems:ruby_pick").unwrap();
            let blade = reg.lookup("gems:ruby_blade").unwrap();
            // The documented rules: durability is honoured, and a damaging,
            // non-pickaxe, non-armour mod tool counts as a weapon ("sword").
            assert_eq!(durability(pick), Some(20));
            assert_eq!(durability(blade), Some(900));
            assert!(is_sword(blade) && !is_sword(pick));
            assert_eq!(crate::anvil::repair_material(pick), Some(ruby));

            let mut g = arena(61);
            // Mining with the mod tool wears it one use per block, like any tool.
            g.inv.slots[g.inv.selected] = Some((pick, 1));
            g.inv.wear[g.inv.selected] = 10;
            g.world.set(2, 50, 2, STONE);
            g.break_block(IVec3::new(2, 50, 2), true);
            assert_eq!(g.inv.wear[g.inv.selected], 11, "the mod tool wore a use");

            // Mend it at an anvil with its repair material (ruby). Each unit
            // restores a quarter of the max (20 / 4 = 5 uses).
            let worn = g.inv.wear[g.inv.selected];
            g.give(ruby, 4);
            g.xp = crate::xp::points_for_level(30);
            let anvil_pos = IVec3::new(-2, 50, 0);
            g.world.set_v(anvil_pos, ANVIL);
            g.open_anvil(anvil_pos);
            let ui = g.anvil.as_mut().unwrap();
            ui.slots[0] = Some((pick, 1));
            ui.wear[0] = worn;
            ui.slots[1] = Some((ruby, 4));
            let (item, plan) = g.anvil_plan().expect("the anvil knows how to mend it");
            assert_eq!(item, pick);
            assert_eq!(plan.wear, worn.saturating_sub(5 * plan.used as u32), "a quarter per ruby");
            assert!(plan.wear < worn && plan.used >= 1);
            g.anvil_take();
            assert_eq!(g.inv.cursor, Some((pick, 1)));
            assert_eq!(g.inv.cursor_wear, plan.wear, "the mended tool is less worn");
        });
    }

    #[test]
    fn mod_armour_is_worn_and_softens_blows() {
        // A mod chestplate with its own armour points. It equips into the
        // right slot, counts towards protection, and cuts incoming damage by
        // the documented 4% per point.
        let mod_txt = "\
[item ruby_chestplate]
armor = chestplate
armor_points = 5
looks_like = diamond
";
        crate::mods::with_mods(&[("gems", mod_txt)], |reg| {
            let chest = reg.lookup("gems:ruby_chestplate").unwrap();
            // armor_of yields (slot, looks_like); armor_points yields K.
            assert_eq!(armor_of(chest), Some((CHESTPLATE, 3))); // diamond tier = 3
            assert_eq!(armor_points(chest), 5);

            let mut g = arena(62);
            // Only the chest slot accepts it: a helmet slot rejects it.
            g.inv.cursor = Some((chest, 1));
            g.inv.click_armor(HELMET);
            assert_eq!(g.inv.armor[HELMET], None, "wrong slot refuses it");
            g.inv.click_armor(CHESTPLATE);
            assert_eq!(g.inv.armor[CHESTPLATE], Some((chest, 1)));
            g.inv.cursor = None;
            assert_eq!(g.inv.armor_points(), 5);

            // 5 points cut 20% off a hit (4% each): a 10-damage blow does 8.
            g.player.health = MAX_HEALTH;
            g.player.hurt = 0.0;
            g.hurt_player_armored(10.0, "tested mod armour");
            assert!((g.player.health - (MAX_HEALTH - 8.0)).abs() < 1e-3, "{}", g.player.health);
        });
    }

    #[test]
    fn hunger_and_food() {
        let mut g = arena(53);
        // Eating fills the bar, not your health.
        g.player.hunger.food = 10.0;
        g.player.health = 10.0;
        g.inv.slots[g.inv.selected] = Some((COOKED_CHOP, 2));
        g.use_item();
        assert_eq!((g.player.hunger.food, g.player.health), (18.0, 10.0));
        // Full means full (except for legendary snacks, which also heal).
        g.player.hunger.food = 20.0;
        g.use_item();
        assert_eq!(g.inv.count(COOKED_CHOP), 1, "not hungry");
        g.inv.slots[g.inv.selected] = Some((GOLDEN_CHOP, 1));
        g.use_item();
        assert_eq!(g.player.health, MAX_HEALTH);
        // A full stomach heals...
        g.player.health = 10.0;
        g.player.hunger.saturation = 5.0;
        for _ in 0..50 {
            g.hunger_tick(0.1);
        }
        assert!(g.player.health > 12.0, "{}", g.player.health);
        // ...an empty one hurts, down to half a heart.
        g.player.hunger = crate::hunger::Hunger::new(0.0, 0.0);
        g.player.health = 3.0;
        for _ in 0..200 {
            g.hunger_tick(0.1);
            g.player.hurt = 0.0;
        }
        assert_eq!(g.player.health, 1.0);
        assert!(g.dead.is_none());
        // Hunger is saved.
        g.player.hunger = crate::hunger::Hunger::new(7.0, 2.0);
        let back = Game::from_save(g.to_save());
        assert_eq!((back.player.hunger.food, back.player.hunger.saturation), (7.0, 2.0));
        // Too hungry to sprint.
        let run = Input { forward: 1.0, strafe: 0.0, jump: false, jump_pressed: false, sneak: false, sprint: true };
        g.player.hunger.food = 4.0;
        g.player.update(0.05, &run, &g.world, false);
        assert!(!g.player.sprinting);
        g.player.hunger.food = 20.0;
        g.player.update(0.05, &run, &g.world, false);
        assert!(g.player.sprinting);
    }

    #[test]
    fn dying_drops_everything() {
        let mut g = arena(57);
        g.inv.slots[0] = Some((DIAMOND, 5));
        g.inv.slots[3] = Some((PICK_IRON, 1));
        g.inv.wear[3] = 40;
        g.inv.armor[HELMET] = Some((ARMOR_FIRST + 4 + HELMET as Id, 1));
        g.player.hurt = 0.0;
        g.hurt_player(100.0, "was tested to destruction");
        assert!(g.dead.is_some());
        assert!(g.inv.counts().is_empty(), "pockets emptied");
        let mut on_floor: Vec<(Id, u8, crate::inventory::Wear)> = g.drops.iter().map(|d| (d.item, d.n, d.wear)).collect();
        on_floor.sort();
        assert_eq!(on_floor, vec![(DIAMOND, 5, 0), (PICK_IRON, 1, 40), (ARMOR_FIRST + 4, 1, 0)]);
        assert!(g.death_spot().is_some());
        g.respawn();
        assert_eq!(g.player.hunger.food, crate::hunger::MAX_FOOD, "respawn with a full stomach");
        // Keep-inventory worlds are kinder.
        let mut g = arena(59);
        g.rules.keep_inventory = true;
        g.inv.slots[0] = Some((DIAMOND, 5));
        g.hurt_player(100.0, "was tested gently");
        assert_eq!(g.inv.count(DIAMOND), 5);
        assert!(g.drops.is_empty());
        assert!(Game::from_save(g.to_save()).rules.keep_inventory);
    }

    #[test]
    fn experience_from_everything() {
        use crate::xp::{death_drop, level_of, points_for_level};
        let mut g = arena(61);
        // Orbs drift to you and count up.
        let at = g.player.body.pos + Vec3::new(3.0, 1.0, 0.0);
        g.spawn_orbs(at, 30);
        for _ in 0..100 {
            g.orbs_tick(0.05);
        }
        assert!(g.orbs.is_empty());
        assert_eq!(g.xp, 30);
        assert_eq!(g.level().0, 3);
        // Dimond ore pays 3 to 7; the furnace pays for what you take out.
        g.world.set(2, 50, 0, DIAMOND_ORE);
        g.break_block(IVec3::new(2, 50, 0), true);
        let ore: u32 = g.orbs.iter().map(|o| o.value as u32).sum();
        assert!((3..=7).contains(&ore), "{ore}");
        g.orbs.clear();
        let furnace = IVec3::new(-2, 50, 0);
        g.world.set_v(furnace, FURNACE);
        g.world.containers.get_mut(&furnace).unwrap().slots[crate::containers::OUTPUT] = Some((COOKED_CHOP, 10));
        g.open = Some(furnace);
        g.container_click(crate::containers::OUTPUT, false, false);
        assert!((33..=34).contains(&g.xp), "3.5 points for ten chops: {}", g.xp);
        g.close_container();
        // Saved with the world.
        assert_eq!(Game::from_save(g.to_save()).xp, g.xp);
        // Dying spills some of it (7 a level) and loses the rest.
        g.xp = points_for_level(10);
        g.hurt_player(100.0, "was tested");
        assert_eq!(g.xp, 0);
        assert_eq!(g.orbs.iter().map(|o| o.value as u32).sum::<u32>(), death_drop(points_for_level(10)));
        // ...unless the world keeps inventories.
        let mut g = arena(63);
        g.rules.keep_inventory = true;
        g.xp = points_for_level(10);
        g.hurt_player(100.0, "was tested");
        assert_eq!(level_of(g.xp).0, 10);
    }

    #[test]
    fn anvils_repair_for_levels() {
        use crate::xp::{level_of, points_for_level};
        let mut g = arena(65);
        let pos = IVec3::new(2, 50, 0);
        g.world.set_v(pos, ANVIL);
        aim(&mut g, pos, IVec3::NEG_X);
        g.use_item();
        assert!(g.anvil.is_some(), "right-click opens it");
        // A badly worn iron pickaxe and some iron.
        g.inv.cursor = Some((PICK_IRON, 1));
        g.inv.cursor_wear = 200;
        g.anvil_click(0, false);
        g.inv.cursor = Some((IRON, 10));
        g.anvil_click(1, false);
        let (item, r) = g.anvil_plan().unwrap();
        assert_eq!((item, r.used, r.cost, r.wear), (PICK_IRON, 4, 4, 0));
        // Not enough levels: nothing happens.
        g.xp = points_for_level(3);
        g.anvil_take();
        assert!(g.inv.cursor.is_none());
        // Enough: good as new, iron and levels spent.
        g.xp = points_for_level(5);
        g.rng = crate::noise::Rng::new(3);
        g.anvil_take();
        assert_eq!((g.inv.cursor, g.inv.cursor_wear), (Some((PICK_IRON, 1)), 0));
        assert_eq!(g.anvil.as_ref().unwrap().slots[1], Some((IRON, 6)));
        assert_eq!(level_of(g.xp).0, 1);
        assert!(g.advancements.has("good_as_new"));
        // Walking away returns what's left on it.
        g.inv.cursor = None;
        g.close_anvil();
        assert_eq!(g.inv.count(IRON), 6);
        // Every use might chip it; eventually it crumbles.
        let mut stages = vec![g.world.get_v(pos)];
        for _ in 0..500 {
            g.anvil_wear_down(pos);
            let now = g.world.get_v(pos);
            if *stages.last().unwrap() != now {
                stages.push(now);
            }
            if now == AIR {
                break;
            }
        }
        assert!(stages.starts_with(&[ANVIL]) && stages.ends_with(&[ANVIL_CHIPPED, ANVIL_DAMAGED, AIR]), "{stages:?}");
        assert!(g.advancements.has("ominous"));
    }

    #[test]
    fn world_rules_change_the_world() {
        use crate::rules::{Difficulty, WorldRules};
        let mut g = arena(67);
        // Peaceful: no monsters, never hungry.
        g.alloc_mob(MobKind::Groaner, Vec3::new(3.5, 50.0, 3.5));
        g.alloc_mob(MobKind::Oinker, Vec3::new(-3.5, 50.0, 3.5));
        g.rules.difficulty = Difficulty::Peaceful;
        g.update_entities(0.05);
        assert_eq!(g.mobs.len(), 1);
        assert!(!g.mobs[0].kind.hostile());
        g.player.hunger.food = 3.0;
        g.hunger_tick(0.1);
        assert_eq!(g.player.hunger.food, crate::hunger::MAX_FOOD);
        // Hard: starving can finish you off.
        g.rules.difficulty = Difficulty::Hard;
        g.player.hunger = crate::hunger::Hunger::new(0.0, 0.0);
        g.player.health = 2.0;
        for _ in 0..200 {
            g.hunger_tick(0.1);
            g.player.hurt = 0.0;
        }
        assert!(g.dead.is_some());
        // A frozen sun, and all of it saved.
        let mut g = arena(69);
        g.rules = WorldRules { keep_inventory: true, difficulty: Difficulty::Easy, daylight_cycle: false, weather_cycle: false, hardcore: false, seasons: false, border: 0 };
        let t = g.time;
        let idle = Controls { input: Input { forward: 0.0, strafe: 0.0, jump: false, jump_pressed: false, sneak: false, sprint: false }, attack_held: false, attack_pressed: false, use_held: false, use_pressed: false, pick: false, drop: false, drop_all: false };
        for _ in 0..20 {
            g.update(0.5, &idle);
        }
        assert_eq!(g.time, t);
        assert_eq!(Game::from_save(g.to_save()).rules, g.rules);
    }

    #[test]
    fn weather_waters_strikes_and_clears() {
        use crate::weather::Weather;
        let mut g = arena(71);
        g.rules.weather_cycle = false;
        // Rain on open farmland counts as water.
        let soil = IVec3::new(3, 49, 3);
        g.world.set_v(soil, FARMLAND);
        g.farm_tick(1.0);
        assert!(!g.world.farm[&soil].wet);
        g.set_weather(Weather::Rain);
        g.farm_tick(1.0);
        assert!(g.world.farm[&soil].wet, "rained on");
        // Storms are darker than rain, which is darker than clear.
        g.time = 0.25;
        g.weather.strength = 1.0;
        let rainy = g.daylight();
        g.weather.kind = Weather::Thunder;
        assert!(g.daylight() < rainy);
        g.weather.kind = Weather::Clear;
        assert!(g.daylight() > rainy);
        // Lightning hurts whoever's under it, and sets off TNT.
        g.world.set(0, 50, 5, TNT);
        g.player.health = 20.0;
        g.lightning_strike(Vec3::new(0.5, 51.0, 5.5));
        assert!(g.weather.bolt.is_some());
        assert_eq!(g.tnts.len(), 1);
        assert_eq!(g.world.get(0, 50, 5), AIR);
        g.lightning_strike(g.player.body.pos);
        assert_eq!(g.player.health, 20.0 - crate::weather::LIGHTNING_DAMAGE);
        // With the cycle off, it stays as it is; saved with the world.
        g.set_weather(Weather::Thunder);
        g.weather.timer = 0.0;
        g.weather_tick(1.0);
        assert_eq!(g.weather.kind, Weather::Thunder);
        let back = Game::from_save(g.to_save());
        assert_eq!((back.weather.kind, back.rules.weather_cycle), (Weather::Thunder, false));
        // The cycle moves it along.
        g.rules.weather_cycle = true;
        g.weather.timer = 0.0;
        g.weather_tick(1.0);
        assert_eq!(g.weather.kind, Weather::Clear);
        assert!(g.weather.timer >= 300.0);
    }

    /// Walk over every item on the ground (then back).
    fn collect(g: &mut Game) {
        let home = g.player.body.pos;
        for _ in 0..100 {
            let Some(p) = g.drops.first().map(|d| d.body.pos) else { break };
            g.player.body.pos = p;
            g.drops_tick(0.1);
        }
        g.player.body.pos = home;
    }

    #[test]
    fn enchanting_tables_enchant_for_levels_and_gold() {
        use crate::enchant::{enchants, is_enchanted, level, offers, offer_seed, Enchant};
        use crate::xp::{level_of, points_for_level};
        let mut g = arena(66);
        let pos = IVec3::new(2, 50, 0);
        g.world.set_v(pos, ENCHANTING_TABLE);
        aim(&mut g, pos, IVec3::NEG_X);
        g.use_item();
        assert!(g.enchanting.is_some(), "right-click opens it");
        assert_eq!(g.bookshelves(pos), 0);
        // Bookshelves in the ring around it power it up (at most 15 count).
        for dx in -2..=2i32 {
            for dz in -2..=2i32 {
                if dx.abs().max(dz.abs()) == 2 {
                    g.world.set_v(pos + IVec3::new(dx, 0, dz), BOOKSHELF);
                    g.world.set_v(pos + IVec3::new(dx, 1, dz), BOOKSHELF);
                }
            }
        }
        assert_eq!(g.bookshelves(pos), 15);
        // Only one thing at a time, only gold in the gold slot.
        g.inv.cursor = Some((PICK_IRON, 1));
        g.inv.cursor_wear = 30;
        g.enchant_click(0, false);
        g.inv.cursor = Some((DIRT, 5));
        g.enchant_click(1, false);
        assert_eq!(g.enchanting.as_ref().unwrap().gold, None);
        g.inv.cursor = Some((GOLD_INGOT, 5));
        g.enchant_click(1, false);
        let o = g.enchant_offers().expect("pickaxes can be enchanted");
        assert!(o[0].0 <= o[1].0 && o[1].0 <= o[2].0 && o[2].0 >= 30, "{o:?}");
        assert!(o.iter().all(|x| is_enchanted(x.1)));
        // The same offers every time, until you enchant something.
        assert_eq!(Some(o), offers(PICK_IRON, 30, 15, offer_seed(PICK_IRON, 0)));
        // Not enough levels: nothing happens.
        g.xp = points_for_level(2);
        g.enchant_pick(2);
        assert!(!is_enchanted(g.enchanting.as_ref().unwrap().wear));
        // Enough: it glows, and levels and gold are spent.
        g.xp = points_for_level(30);
        g.enchant_pick(2);
        let ui = g.enchanting.as_ref().unwrap();
        assert_eq!(enchants(ui.wear), enchants(o[2].1));
        assert_eq!(ui.wear & 0xFFFF, 30, "wear is kept");
        assert_eq!(ui.gold, Some((GOLD_INGOT, 2)));
        assert_eq!((level_of(g.xp).0, g.enchant_count), (27, 1));
        assert!(g.advancements.has("enchanter"));
        // Enchanted things can't be enchanted again; new offers for the next thing.
        assert!(g.enchant_offers().is_none());
        assert_ne!(offers(PICK_IRON, 0, 15, offer_seed(PICK_IRON, 1)), Some(o));
        // Walking away returns both.
        g.close_enchanting();
        assert_eq!(g.inv.count(GOLD_INGOT), 2);
        let slot = g.inv.slots.iter().position(|s| *s == Some((PICK_IRON, 1))).unwrap();
        assert_eq!(g.inv.wear[slot], o[2].1 | 30);

        // What each enchantment does.
        assert!(break_time_with(STONE, PICK_IRON, 3).0 < break_time_with(STONE, PICK_IRON, 0).0 / 2.0);
        assert_eq!(attack_damage_with(SWORD_IRON, 2), attack_damage(SWORD_IRON) + 2.5);
        assert_eq!(fortune_count(DIAMOND_ORE, 0, 0.99), 1);
        assert_eq!(fortune_count(DIAMOND_ORE, 3, 0.99), 4);
        assert_eq!(fortune_count(STONE, 3, 0.99), 1, "only ores");
        let w = crate::enchant::with_level(0, Enchant::Unbreaking, 2);
        assert_eq!(crate::inventory::max_uses(PICK_IRON, w), Some(3 * durability(PICK_IRON).unwrap() as u32));
        assert_eq!(level(w, Enchant::Unbreaking), 2);
        // Protection softens hits on top of the armour itself.
        let hit = |prot: u8| {
            let mut g = arena(67);
            g.inv.armor[1] = Some((ARMOR_FIRST + 4 + 1, 1));
            g.inv.armor_wear[1] = crate::enchant::with_level(0, Enchant::Protection, prot);
            g.hurt_player_armored(10.0, "was tested");
            20.0 - g.player.health
        };
        assert!(hit(4) < hit(0) - 1.0, "{} vs {}", hit(4), hit(0));
    }

    #[test]
    fn buckets_and_flowing_liquids() {
        let mut g = arena(68);
        let step = |g: &mut Game, secs: f32| {
            let mut t = 0.0;
            while t < secs {
                g.liquid_tick(0.05);
                t += 0.05;
            }
        };
        // A pool of water two blocks deep to scoop from.
        let pool = IVec3::new(3, 49, 0);
        g.world.set_v(pool, WATER);
        g.inv.slots = [None; 36];
        g.inv.slots[0] = Some((BUCKET, 2));
        g.inv.selected = 0;
        look_at(&mut g, pool.as_vec3() + Vec3::new(0.5, 0.9, 0.5));
        g.use_item();
        assert_eq!(g.world.get_v(pool), AIR, "scooped up");
        assert_eq!((g.inv.count(BUCKET), g.inv.count(WATER_BUCKET)), (1, 1));
        // Pour it out on the floor: it spreads seven blocks and no further.
        let slot = g.inv.slots.iter().position(|s| *s == Some((WATER_BUCKET, 1))).unwrap();
        g.inv.selected = slot;
        let spot = IVec3::new(0, 50, 3);
        look_at(&mut g, spot.as_vec3() + Vec3::new(0.5, 0.02, 0.5));
        g.use_item();
        assert_eq!(g.world.get_v(spot), WATER);
        assert_eq!(g.inv.slots[slot], Some((BUCKET, 1)));
        step(&mut g, 4.0);
        assert_eq!(g.world.get_v(spot + IVec3::new(0, 0, 1)), liquid_at(false, 1));
        assert_eq!(g.world.get_v(spot + IVec3::new(0, 0, 7)), liquid_at(false, 7));
        assert_eq!(g.world.get_v(pool), liquid_at(false, 1), "it pours into the hole left behind");
        assert_eq!(g.world.get_v(spot + IVec3::new(-7, 0, 0)), liquid_at(false, 7));
        assert_eq!(g.world.get_v(spot + IVec3::new(-8, 0, 0)), AIR);
        // It carries things downstream.
        assert!(crate::liquids::current(&g.world, (spot + IVec3::new(-3, 0, 0)).as_vec3() + Vec3::splat(0.5)).x < 0.0);
        // Scoop the source back up and it all drains away.
        g.inv.selected = slot;
        look_at(&mut g, spot.as_vec3() + Vec3::new(0.5, 0.5, 0.5));
        g.use_item();
        assert_eq!(g.inv.slots[slot], Some((WATER_BUCKET, 1)));
        step(&mut g, 5.0);
        assert!((-8..=8).all(|d| g.world.get_v(spot + IVec3::new(d, 0, 0)) == AIR));

        // Lava next to water turns to obsidian; standing in lava hurts and sets you alight.
        let lava = IVec3::new(-4, 50, -4);
        g.world.set_v(lava, LAVA);
        g.world.set_v(lava + IVec3::X, WATER);
        step(&mut g, 2.0);
        assert_eq!(g.world.get_v(lava), OBSIDIAN);
        g.world.set_v(IVec3::new(5, 50, 5), LAVA);
        g.player.body.pos = Vec3::new(5.5, 50.0, 5.5);
        g.player.hurt = 0.0;
        let before = g.player.health;
        g.lava_tick(0.05);
        assert!(g.player.health < before && g.on_fire > 0.0);
        assert!(g.advancements.has("hot_stuff"));
        // Out of the lava it keeps burning until water (or rain) puts it out.
        g.world.set_v(IVec3::new(5, 50, 5), AIR);
        g.player.hurt = 0.0;
        let before = g.player.health;
        g.lava_tick(1.0);
        assert!(g.player.health < before);
        g.player.body.in_water = true;
        g.lava_tick(0.05);
        assert_eq!(g.on_fire, 0.0);
    }

    #[test]
    fn feeding_breeding_shearing_and_taming() {
        use crate::animals::{Interaction, GROW_SECS};
        use crate::players::record_key;
        let mut g = arena(5150);
        for x in -12..12 {
            for z in -12..12 {
                g.world.set(x, 49, z, GRASS);
            }
        }
        g.mobs.clear();
        let me = record_key(&g.player_name);
        let at = g.player.body.pos;
        let spawn = |g: &mut Game, kind: MobKind, x: f32| {
            let mut m = Mob::new(kind, Vec3::new(x, 50.0, 2.5), &mut g.rng);
            m.id = g.next_mob_id;
            g.next_mob_id += 1;
            g.mobs.push(m);
            m_id(g)
        };
        fn m_id(g: &Game) -> u32 {
            g.mobs.last().unwrap().id
        }
        // Wheat for Mooers; not carrots.
        let a = spawn(&mut g, MobKind::Mooer, 1.5);
        let b = spawn(&mut g, MobKind::Mooer, 3.0);
        assert_eq!(g.interact_mob(&me, at, a, CARROT), Interaction::Nothing);
        assert_eq!(g.interact_mob(&me, at, a, WHEAT), Interaction::Ate);
        assert_eq!(g.interact_mob(&me, at, a, WHEAT), Interaction::Nothing, "already in love");
        assert_eq!(g.interact_mob(&me, at, b, WHEAT), Interaction::Ate);
        // They find each other and a calf appears.
        for _ in 0..400 {
            g.animals_tick(0.05);
            let (daylight, world) = (1.0, &g.world);
            for m in g.mobs.iter_mut() {
                m.update(0.05, world, Vec3::new(0.0, 50.0, -20.0), false, daylight, &mut g.rng);
            }
            if g.mobs.len() == 3 {
                break;
            }
        }
        assert_eq!(g.mobs.len(), 3, "a baby");
        let calf = g.mobs.last().unwrap();
        assert!(calf.baby > 0.0 && calf.persistent && calf.body.height < 1.0);
        assert!(g.mobs.iter().take(2).all(|m| m.breed_cd > 0.0 && m.love == 0.0));
        assert_eq!(g.interact_mob(&me, at, a, WHEAT), Interaction::Nothing, "resting");
        assert!(g.advancements.has("the_birds_and_the_bees"));
        // Babies grow up.
        let mut calf = g.mobs.pop().unwrap();
        calf.update(GROW_SECS + 1.0, &g.world, Vec3::ZERO, false, 1.0, &mut g.rng);
        assert!(calf.baby <= 0.0 && calf.body.height > 1.0);

        // Shearing.
        let f = spawn(&mut g, MobKind::Fluffer, -2.0);
        let drops = g.drops.len();
        assert_eq!(g.interact_mob(&me, at, f, SHEARS), Interaction::Sheared);
        assert!(g.drops.len() > drops && g.drops.last().unwrap().item == WOOL);
        assert_eq!(g.interact_mob(&me, at, f, SHEARS), Interaction::Nothing, "nothing left to shear");
        // It grows back after some grass.
        let fi = g.mobs.len() - 1;
        g.mobs[fi].body.on_ground = true;
        for _ in 0..2000 {
            g.animals_tick(0.1);
            if !g.mobs[fi].sheared {
                break;
            }
        }
        assert!(!g.mobs[fi].sheared);

        // Taming takes a few bones; then it follows, sits and fights.
        let w = spawn(&mut g, MobKind::Woofer, -4.0);
        g.rng = crate::noise::Rng::new(2);
        let mut bones = 0;
        while g.mobs.iter().find(|m| m.id == w).unwrap().owner.is_none() {
            assert_eq!(g.interact_mob(&me, at, w, BONE), Interaction::Ate);
            bones += 1;
            assert!(bones < 50);
        }
        let wi = g.mobs.iter().position(|m| m.id == w).unwrap();
        assert!(g.mobs[wi].sitting && g.mobs[wi].persistent && g.advancements.has("good_boy"));
        assert_eq!(g.interact_mob(&me, at, w, AIR), Interaction::Toggled);
        assert!(!g.mobs[wi].sitting);
        g.animals_tick(0.05);
        assert!(g.mobs[wi].goal.is_some(), "heads for its owner");
        // Someone else's Woofer ignores you.
        assert_eq!(g.interact_mob("somebody_else", at, w, AIR), Interaction::Nothing);
        // Hit a mob and the Woofer goes for it.
        let target = spawn(&mut g, MobKind::Oinker, -4.5);
        g.sic_pets(&me, target);
        assert_eq!(g.mobs[wi].prey, Some(target));
        let hp = g.mobs.last().unwrap().health;
        for _ in 0..10 {
            g.mobs[wi].attack_cd = 0.0;
            g.animals_tick(0.05);
        }
        assert!(g.mobs.last().unwrap().health < hp);
        // Tamed, bred and fed animals are saved with the world (wild ones aren't).
        let wild = spawn(&mut g, MobKind::Oinker, 5.0);
        let back = Game::from_save(g.to_save());
        let pet = back.mobs.iter().find(|m| m.kind == MobKind::Woofer).expect("the Woofer came back");
        assert_eq!(pet.owner.as_deref(), Some(me.as_str()));
        assert!(back.mobs.iter().any(|m| m.kind == MobKind::Fluffer));
        assert_eq!(back.mobs.iter().filter(|m| m.kind == MobKind::Mooer).count(), 2);
        assert!(back.mobs.len() < g.mobs.len(), "not the wild Oinker {wild}");
    }

    #[test]
    fn zappy_dust_carries_power() {
        let mut g = arena(69);
        let run = |g: &mut Game, secs: f32| {
            let mut t = 0.0;
            while t < secs {
                g.zap_tick(0.05);
                t += 0.05;
            }
        };
        // Lever, 15 blocks of dust with a step up in the middle, a lamp at the end.
        let lever = IVec3::new(-8, 50, 0);
        g.world.set_v(lever, LEVER);
        g.world.set_v(IVec3::new(0, 50, 0), STONE);
        for x in -7..=7 {
            let y = if x == 0 { 51 } else { 50 };
            g.world.set_v(IVec3::new(x, y, 0), WIRE);
        }
        let lamp = IVec3::new(8, 50, 0);
        g.world.set_v(lamp, LAMP);
        run(&mut g, 0.5);
        assert_eq!(g.world.get_v(lamp), LAMP);
        // Flip it (as the player would).
        g.inv.slots = [None; 36];
        aim(&mut g, lever, IVec3::Y);
        g.use_item();
        assert_eq!(g.world.get_v(lever), LEVER_ON);
        run(&mut g, 0.5);
        assert!((-7..=7).all(|x| g.world.get_v(IVec3::new(x, if x == 0 { 51 } else { 50 }, 0)) == WIRE_ON), "the whole line lights up");
        assert_eq!(g.world.get_v(lamp), LAMP_ON);
        assert!(g.advancements.has("its_alive"));
        // Dust only carries so far: a 16th block stays dark.
        g.world.set_v(lamp, WIRE);
        g.world.set_v(lamp + IVec3::X, LAMP);
        run(&mut g, 0.5);
        assert_eq!(g.world.get_v(lamp), WIRE);
        assert_eq!(g.world.get_v(lamp + IVec3::X), LAMP);
        // Flip it back and it all goes dark.
        g.world.set_v(lamp, AIR);
        g.world.set_v(lamp + IVec3::X, AIR);
        g.world.set_v(lamp, LAMP);
        aim(&mut g, lever, IVec3::Y);
        g.use_item();
        run(&mut g, 0.5);
        assert_eq!(g.world.get_v(IVec3::new(3, 50, 0)), WIRE);
        assert_eq!(g.world.get_v(lamp), LAMP);

        // A button: a second of power, then it pops out.
        let button = IVec3::new(4, 50, 4);
        let door_at = IVec3::new(5, 50, 4);
        g.world.set_v(button, BUTTON);
        g.world.set_v(door_at, door(0, false, false));
        g.world.set_v(door_at + IVec3::Y, door(0, false, true));
        aim(&mut g, button, IVec3::Y);
        g.use_item();
        run(&mut g, 0.3);
        assert_eq!(door_state(g.world.get_v(door_at)), Some((0, true, false)), "the door opens");
        run(&mut g, 1.5);
        assert_eq!(g.world.get_v(button), BUTTON);
        assert_eq!(door_state(g.world.get_v(door_at)), Some((0, false, false)), "and closes again");

        // A pressure plate under a mob, next to TNT.
        let plate = IVec3::new(-4, 50, 6);
        g.world.set_v(plate, PLATE);
        g.world.set_v(plate + IVec3::X, TNT);
        g.mobs.clear();
        g.alloc_mob(MobKind::Oinker, plate.as_vec3() + Vec3::new(0.5, 0.0, 0.5));
        run(&mut g, 0.3);
        assert_eq!(g.world.get_v(plate), PLATE_ON);
        assert_eq!(g.world.get_v(plate + IVec3::X), AIR);
        assert_eq!(g.tnts.len(), 1, "primed");
        g.mobs.clear();
        run(&mut g, 1.0);
        assert_eq!(g.world.get_v(plate), PLATE);

        // Dust is laid from the item, on a floor; it falls off with it.
        g.inv.slots[0] = Some((ZAP_DUST, 3));
        g.inv.selected = 0;
        let floor = IVec3::new(-6, 49, -6);
        aim(&mut g, floor, IVec3::Y);
        g.use_item();
        assert_eq!(g.world.get_v(floor + IVec3::Y), WIRE);
        assert_eq!(g.inv.count(ZAP_DUST), 2);
        g.break_block(floor, false);
        assert_eq!(g.world.get_v(floor + IVec3::Y), AIR);
    }

        #[test]
    fn enchanted_books() {
        use crate::anvil::plan;
        use crate::enchant::{level, merge, offers, offer_seed, with_level, Enchant};
        // Books take any enchantment at the table, and come out enchanted.
        let o = offers(BOOK, 0, 15, offer_seed(BOOK, 0)).expect("books are enchantable");
        assert!(o.iter().all(|x| x.1 != 0));
        let mut g = arena(70);
        let pos = IVec3::new(2, 50, 0);
        g.world.set_v(pos, ENCHANTING_TABLE);
        g.open_enchanting(pos);
        g.inv.slots = [None; 36];
        g.inv.slots[0] = Some((BOOK, 5));
        g.inv.slots[1] = Some((GOLD_INGOT, 5));
        g.enchant_quick_put(0);
        g.enchant_quick_put(1);
        assert_eq!(g.inv.slots[0], Some((BOOK, 4)), "one book at a time");
        g.xp = crate::xp::points_for_level(30);
        g.enchant_pick(2);
        let ui = g.enchanting.as_ref().unwrap();
        assert_eq!(ui.item, Some((ENCHANTED_BOOK, 1)));
        let book = ui.wear;
        assert!(book >> 16 != 0);
        g.close_enchanting();
        let slot = g.inv.slots.iter().position(|s| *s == Some((ENCHANTED_BOOK, 1))).unwrap();
        assert_eq!(g.inv.wear[slot], book, "the book keeps its enchantments");

        // At the anvil, a book's enchantments go onto anything they fit.
        let sharp3 = with_level(0, Enchant::Sharpness, 3) | with_level(0, Enchant::Efficiency, 2);
        let r = plan(SWORD_IRON, 40, Some((ENCHANTED_BOOK, 1)), sharp3).unwrap();
        assert!(r.book && r.used == 1);
        assert_eq!(level(r.wear, Enchant::Sharpness), 3);
        assert_eq!(level(r.wear, Enchant::Efficiency), 0, "Efficiency doesn't fit a sword");
        assert_eq!(r.wear & 0xFFFF, 40, "wear kept");
        assert_eq!(r.cost, 3);
        // Two of the same level make one higher; different levels keep the best.
        let sharp3_sword = with_level(0, Enchant::Sharpness, 3);
        assert_eq!(level(merge(SWORD_IRON, sharp3_sword, sharp3_sword), Enchant::Sharpness), 4);
        assert_eq!(level(merge(SWORD_IRON, with_level(0, Enchant::Sharpness, 5), sharp3_sword), Enchant::Sharpness), 5);
        // Nothing new to add: nothing to do.
        assert!(plan(SWORD_IRON, with_level(0, Enchant::Sharpness, 5), Some((ENCHANTED_BOOK, 1)), with_level(0, Enchant::Sharpness, 5)).is_none(), "already as good as it gets");
        assert!(plan(SWORD_IRON, 0, Some((ENCHANTED_BOOK, 1)), with_level(0, Enchant::Protection, 2)).is_none(), "nothing fits");
        // Merging two enchanted tools merges their enchantments too.
        let r = plan(PICK_IRON, with_level(100, Enchant::Efficiency, 2), Some((PICK_IRON, 1)), with_level(50, Enchant::Efficiency, 2) | with_level(0, Enchant::Fortune, 1)).unwrap();
        assert_eq!((level(r.wear, Enchant::Efficiency), level(r.wear, Enchant::Fortune)), (3, 1));
        // Books merge with books.
        let r = plan(ENCHANTED_BOOK, with_level(0, Enchant::Protection, 2), Some((ENCHANTED_BOOK, 1)), with_level(0, Enchant::Protection, 2)).unwrap();
        assert_eq!(level(r.wear, Enchant::Protection), 3);
    }

        #[test]
    fn hmmers_move_in_and_trade() {
        use crate::villagers::{trades, Job, STOCK};
        let mut g = arena(71);
        g.mobs.clear();
        // A hut chest filled for the first time brings a Hmmer (a Smith, for this seed).
        let seed = (0..64u32).find(|&s| Job::of(s) == Job::Smith).unwrap();
        g.world.new_huts.push((Vec3::new(2.5, 50.0, 0.5), seed));
        g.house_hmmers();
        assert_eq!(g.mobs.len(), 1);
        let (id, kind) = (g.mobs[0].id, g.mobs[0].kind);
        assert_eq!(kind, MobKind::Hmmer);
        assert!(g.mobs[0].persistent && g.mobs[0].home.is_some());
        // Right-click to talk.
        g.target = Some(Target::Mob(0));
        g.use_item();
        assert_eq!(g.trading, Some(id));
        let (job, list) = g.trade_list().unwrap();
        assert_eq!((job, list.clone()), (format!("Hmmer: {}", Job::Smith.name()), trades(seed)));
        // Coal for gold.
        let coal = list.iter().position(|t| t.give[0].0 == COAL).unwrap();
        g.inv.slots = [None; 36];
        g.inv.slots[0] = Some((COAL, 64));
        g.inv.slots[1] = Some((COAL, 64));
        g.make_trade(coal);
        assert_eq!((g.inv.count(COAL), g.inv.count(GOLD_INGOT)), (116, 1));
        assert!(g.advancements.has("what_a_deal"));
        // Can't afford: nothing happens.
        let chest = list.iter().position(|t| t.give[0] == (GOLD_INGOT, 10)).unwrap();
        g.make_trade(chest);
        assert_eq!(g.inv.count(GOLD_INGOT), 1);
        // They run out, and restock the next day.
        for _ in 0..STOCK {
            g.make_trade(coal);
        }
        assert_eq!(g.inv.count(GOLD_INGOT), STOCK as u32);
        g.mobs[0].restock = 0.01;
        g.hmmers_tick(0.1);
        g.make_trade(coal);
        assert_eq!(g.inv.count(GOLD_INGOT), STOCK as u32 + 1);
        // Hmmers stay near home.
        g.mobs[0].body.pos = Vec3::new(-10.0, 50.0, 10.0);
        g.mobs[0].goal = None;
        g.hmmers_tick(0.1);
        assert!(g.mobs[0].goal.is_some());
        // And they're kept with the world.
        let back = Game::from_save(g.to_save());
        let h = back.mobs.iter().find(|m| m.kind == MobKind::Hmmer).unwrap();
        assert_eq!((h.seed, h.home), (seed, g.mobs[0].home));
    }

        #[test]
    fn charged_blows_sweeps_and_shields() {
        let mut g = arena(72);
        g.mobs.clear();
        g.inv.slots = [None; 36];
        g.inv.slots[0] = Some((SWORD_IRON, 1));
        g.inv.selected = 0;
        let spawn = |g: &mut Game, x: f32| {
            g.alloc_mob(MobKind::Mooer, Vec3::new(x, 50.0, -1.5));
            g.mobs.len() - 1
        };
        let (a, b) = (spawn(&mut g, 0.5), spawn(&mut g, 1.4));
        let full = g.mobs[a].health;
        let swing = |g: &mut Game, i: usize| {
            g.target = Some(Target::Mob(i));
            let c = Controls { input: Default::default(), attack_held: true, attack_pressed: true, use_held: false, use_pressed: false, pick: false, drop: false, drop_all: false };
            g.attack_cd = 0.0;
            g.handle_actions(0.0, &c);
        };
        // A fully charged sword blow does full damage and sweeps the one beside it.
        g.player.body.on_ground = true;
        swing(&mut g, a);
        assert!((full - g.mobs[a].health - attack_damage(SWORD_IRON)).abs() < 1e-3);
        assert!(g.mobs[b].health < full, "swept");
        // Straight away again: much weaker.
        for m in g.mobs.iter_mut() {
            m.hurt = 0.0;
        }
        let before = g.mobs[a].health;
        swing(&mut g, a);
        assert!(before - g.mobs[a].health < attack_damage(SWORD_IRON) * 0.3);

        // A shield up stops hits from in front, and wears instead.
        g.inv.slots[1] = Some((SHIELD, 1));
        g.inv.selected = 1;
        g.player.yaw = 0.0;
        g.player.pitch = 0.0;
        g.blocking = true;
        g.player.hurt = 0.0;
        let hp = g.player.health;
        let front = g.player.body.pos + Vec3::new(0.0, 0.9, -3.0);
        g.hurt_player_from(5.0, "tested", Some(front), false);
        assert_eq!(g.player.health, hp);
        assert_eq!(g.inv.wear[1], 5);
        assert!(g.advancements.has("not_today"));
        // Not from behind, though.
        g.hurt_player_from(5.0, "tested", Some(g.player.body.pos + Vec3::new(0.0, 0.9, 3.0)), false);
        assert!(g.player.health < hp);
        // Armour steadies you.
        let push = Vec3::new(10.0, 0.0, 0.0);
        assert_eq!(g.steadied(push), push);
        g.inv.armor = [Some((ARMOR_FIRST + 12, 1)), Some((ARMOR_FIRST + 13, 1)), Some((ARMOR_FIRST + 14, 1)), Some((ARMOR_FIRST + 15, 1))];
        assert!(g.steadied(push).x < 6.0);
    }

        #[test]
    fn portals_to_the_scorchlands_and_back() {
        use crate::scorch::{in_scorch, is_portal};
        let mut g = arena(73);
        // A 4x5 obsidian frame (corners too), lit with a Sparker.
        let base = IVec3::new(3, 50, -3);
        for dx in -1..=2 {
            for dy in -1..=3 {
                if dx == -1 || dx == 2 || dy == -1 || dy == 3 {
                    g.world.set_v(base + IVec3::new(dx, dy, 0), OBSIDIAN);
                }
            }
        }
        g.inv.slots = [None; 36];
        g.inv.slots[0] = Some((SPARKER, 1));
        g.inv.selected = 0;
        aim(&mut g, base - IVec3::Y, IVec3::Y);
        g.use_item();
        assert!((0..2).all(|dx| (0..3).all(|dy| g.world.get_v(base + IVec3::new(dx, dy, 0)) == PORTAL_X)), "lit");
        assert!(g.advancements.has("portal_open"));
        assert_eq!(g.inv.wear[0], 1, "the Sparker wears");
        // A frame with a gap won't light.
        let open = IVec3::new(-6, 50, 3);
        for dy in -1..=3 {
            g.world.set_v(open + IVec3::new(-1, dy, 0), OBSIDIAN);
        }
        assert!(crate::scorch::portal_frame(&g.world, open, true).is_none());

        // Standing in it for a couple of seconds takes you to the Scorchlands, into a new portal.
        g.player.body.pos = base.as_vec3() + Vec3::new(1.0, 0.0, 0.5);
        for _ in 0..50 {
            g.portal_tick(0.05);
        }
        let there = g.player.body.pos;
        assert!(in_scorch(there.x), "at {there}");
        let feet = IVec3::new(there.x.floor() as i32, there.y.floor() as i32, there.z.floor() as i32);
        assert!(is_portal(g.world.get_v(feet)), "arrived in a portal");
        assert!(g.advancements.has("hotter"));
        assert_eq!(g.sky_color(), [0.24, 0.06, 0.03]);
        // Straight back through isn't possible without stepping out first.
        for _ in 0..60 {
            g.portal_tick(0.05);
        }
        assert!(in_scorch(g.player.body.pos.x));
        // Step out, step back in: home again, to the portal we came from.
        g.player.body.pos += Vec3::new(0.0, 0.0, 2.0);
        g.portal_tick(0.05);
        g.player.body.pos = there;
        for _ in 0..90 {
            g.portal_tick(0.05);
        }
        let home = g.player.body.pos;
        assert!(!in_scorch(home.x));
        assert!(home.distance(base.as_vec3()) < 6.0, "back at the first portal: {home}");
        // The links are kept with the world.
        let back = Game::from_save(g.to_save());
        assert_eq!(back.portal_links, g.portal_links);
        assert_eq!(g.portal_links.len(), 2);
        // Break the frame and the portal falls apart.
        g.world.set_v(base + IVec3::new(-1, 1, 0), AIR);
        for _ in 0..30 {
            g.zap_tick(0.05);
        }
        assert!((0..2).all(|dx| (0..3).all(|dy| g.world.get_v(base + IVec3::new(dx, dy, 0)) == AIR)));
    }

        #[test]
    fn carts_ride_rails_and_boats_float() {
        use crate::vehicles::{is_rail, rail_dirs};
        let mut g = arena(74);
        let run = |g: &mut Game, secs: f32, forward: f32| {
            let mut t = 0.0;
            while t < secs {
                g.zap_tick(0.05);
                g.vehicles_tick(0.05, forward, 0.0, false);
                t += 0.05;
            }
        };
        // An L of rails: they bend round the corner by themselves.
        for z in 0..6 {
            g.world.set_v(IVec3::new(0, 50, -z), RAIL_FIRST);
        }
        for x in 1..6 {
            g.world.set_v(IVec3::new(x, 50, -5), RAIL_FIRST);
        }
        // A powered rail at the start, with a lever to power it.
        g.world.set_v(IVec3::new(0, 50, 0), POWERED_RAIL);
        g.world.set_v(IVec3::new(-1, 50, 0), LEVER_ON);
        run(&mut g, 0.3, 0.0);
        let corner = g.world.get_v(IVec3::new(0, 50, -5));
        assert_eq!(rail_dirs(corner), Some([IVec3::Z, IVec3::X]), "the corner bent: {corner}");
        assert_eq!(rail_dirs(g.world.get_v(IVec3::new(3, 50, -5))), Some([IVec3::X, IVec3::NEG_X]));
        assert_eq!(g.world.get_v(IVec3::new(0, 50, 0)), POWERED_RAIL + 1, "powered and lit");
        // A cart on the powered rail sets off round the corner and to the end.
        g.inv.slots = [None; 36];
        g.inv.slots[0] = Some((MINECART, 1));
        g.inv.selected = 0;
        g.player.yaw = 0.0;
        g.player.pitch = -0.6;
        g.player.body.pos = Vec3::new(0.5, 50.0, 2.5);
        assert!(g.place_vehicle(MINECART));
        assert_eq!(g.vehicles.len(), 1);
        g.mount(0);
        assert!(g.riding.is_some());
        run(&mut g, 4.0, 0.0);
        let v = &g.vehicles[0];
        assert!(v.pos.x > 4.5 && (v.pos.z + 4.5).abs() < 0.2, "at the end of the line: {}", v.pos);
        assert!(g.player.body.pos.distance(v.seat()) < 0.01, "carrying us");
        // Sneak to get out; hit it three times to pick it up.
        g.vehicles_tick(0.05, 0.0, 0.0, true);
        assert!(g.riding.is_none());
        let id = g.vehicles[0].id;
        for _ in 0..3 {
            g.hit_vehicle(id, 1);
        }
        assert!(g.vehicles.is_empty());
        assert!(g.drops.iter().any(|d| d.item == MINECART));
        assert!(is_rail(RAIL_FIRST + 5));

        // A boat on a pond floats, and rows forward.
        for x in -8..=-2 {
            for z in 2..=8 {
                g.world.set(x, 49, z, WATER);
            }
        }
        let id = g.spawn_vehicle(crate::vehicles::BOAT_KIND, Vec3::new(-5.0, 49.6, 5.0), 0.0);
        let i = g.vehicles.iter().position(|v| v.id == id).unwrap();
        g.mount(i);
        run(&mut g, 1.0, 1.0);
        let b = g.vehicles.iter().find(|v| v.id == id).unwrap();
        assert!(b.pos.z < 4.5, "rowed north: {}", b.pos);
        assert!((b.pos.y - 49.85).abs() < 0.3, "afloat: {}", b.pos.y);
        // Vehicles are kept with the world.
        let back = Game::from_save(g.to_save());
        assert_eq!(back.vehicles.len(), 1);
        assert_eq!(back.vehicles[0].rider, 0);
    }

        #[test]
    fn signs_frames_and_maps() {
        let mut g = arena(75);
        g.inv.slots = [None; 36];
        g.inv.slots[0] = Some((SIGN_FIRST, 3));
        g.inv.selected = 0;
        g.player.yaw = 0.0;
        g.player.pitch = -0.3;
        // Put a sign up: it faces us and asks for words.
        let floor = IVec3::new(0, 49, -3);
        aim(&mut g, floor, IVec3::Y);
        g.use_item();
        let sign = floor + IVec3::Y;
        assert!(crate::decor::is_sign(g.world.get_v(sign)));
        assert_eq!(g.editing_sign, Some(sign));
        g.editing_sign = None;
        g.set_sign(sign, &["Welcome".into(), "to".into(), "Stoveville".into()]);
        assert_eq!(g.world.signs[&sign][2], "Stoveville");
        // Hang a frame on a wall and put a pickaxe in it.
        let wall = IVec3::new(3, 50, 0);
        g.world.set_v(wall, STONE);
        g.inv.slots[1] = Some((FRAME_FIRST, 1));
        g.inv.slots[2] = Some((PICK_IRON, 1));
        g.inv.wear[2] = 17;
        g.inv.selected = 1;
        aim(&mut g, wall, IVec3::NEG_X);
        g.use_item();
        let frame = wall - IVec3::X;
        assert_eq!(g.world.get_v(frame), FRAME_FIRST + 1, "hung on the east wall");
        g.inv.selected = 2;
        aim(&mut g, frame, IVec3::NEG_X);
        g.use_item();
        assert_eq!(g.world.frames.get(&frame), Some(&(PICK_IRON, 17)));
        assert_eq!(g.inv.count(PICK_IRON), 0);
        // Kept with the world.
        let back = Game::from_save(g.to_save());
        assert_eq!(back.world.signs.get(&sign), g.world.signs.get(&sign));
        assert_eq!(back.world.frames.get(&frame), Some(&(PICK_IRON, 17)));
        // Hit it: the pickaxe pops out (the frame stays); breaking the frame drops nothing more.
        assert!(g.hit_frame(frame));
        assert!(g.world.frames.is_empty());
        assert!(g.drops.iter().any(|d| d.item == PICK_IRON && d.wear == 17));
        assert!(!g.hit_frame(frame));
        // Breaking the sign takes its words.
        g.break_block(sign, false);
        assert!(g.world.signs.is_empty());

        // The map shows the floor we're standing on; the compass points home.
        let colors = vec![[10, 20, 30]; reg().blocks.len()];
        let px = crate::navigation::map_pixels(&g.world, g.player.body.pos, &colors, 1);
        assert_eq!(px.len(), crate::navigation::MAP_SIZE * crate::navigation::MAP_SIZE * 4);
        let mid = (crate::navigation::MAP_SIZE / 2 * crate::navigation::MAP_SIZE + crate::navigation::MAP_SIZE / 2) * 4;
        // Our colour, maybe shaded by the lie of the land.
        assert!([8, 10, 11].contains(&px[mid]) && px[mid + 3] == 255, "{:?}", &px[mid..mid + 4]);
    }

        /// A flat, empty arena: stone floor at y = 49, air above, around the origin.
    pub(crate) fn arena(seed: u32) -> Game {
        let mut g = Game::new(seed, false, false);
        g.world = loaded_world(seed);
        g.ready = true;
        for x in -12..12 {
            for z in -12..12 {
                g.world.set(x, 49, z, STONE);
                for y in 50..62 {
                    g.world.set(x, y, z, AIR);
                }
            }
        }
        g.player.body.pos = Vec3::new(0.5, 50.0, 0.5);
        g.spawn_timer = 1e9; // no surprise visitors
        g
    }

    #[test]
    fn projectile_fan_is_repeatable_and_odd_even_symmetric() {
        let center = Vec3::new(0.0, 7.5, -20.0);
        let odd = projectile_fan(center, 3, 20.0);
        assert_eq!(odd, projectile_fan(center, 3, 20.0), "fan generation is repeatable");
        assert_eq!(odd[1], center, "an odd fan includes the center ray");
        assert!((odd[0].x + odd[2].x).abs() < 1e-5);
        assert!((odd[0].z - odd[2].z).abs() < 1e-5);
        assert!((odd[0].x.abs().atan2(-odd[0].z) - 10.0f32.to_radians()).abs() < 1e-6);

        let even = projectile_fan(center, 4, 30.0);
        assert_eq!(even, projectile_fan(center, 4, 30.0), "even fan is repeatable");
        assert!(even[1].x * even[2].x < 0.0, "even fan straddles the center ray");
        for i in 0..2 {
            assert!((even[i].x + even[3 - i].x).abs() < 1e-5);
            assert!((even[i].z - even[3 - i].z).abs() < 1e-5);
        }
        assert!(odd.iter().chain(&even).all(|vel| vel.y == center.y), "rotation preserves the vertical lob");
        assert_eq!(projectile_fan(center, 1, 45.0), vec![center], "one projectile always uses angle zero");
    }

    #[test]
    fn projectile_multishot_expands_one_effect_only_volley_and_caps_arrows() {
        let mut g = arena(80);
        for i in 0..199 {
            g.arrows.push(Arrow::new(Vec3::new(i as f32, 60.0, 0.0), Vec3::Z, None, 3.0));
        }
        g.sounds.clear();
        let appearance = ProjectileAppearance { model: ProjectileModel::Billboard, tile: Some(T_STONE), scale: 0.75 };
        let effect = ProjectileEffect { kind: crate::potions::Potion::Leaping, duration: 4.0, amplifier: 1 };
        let pos = Vec3::new(2.0, 55.0, 3.0);
        let center = Vec3::new(0.0, 8.0, -24.0);
        g.spawn_mod_projectiles(pos, center, ModProjectileSpec { damage: 0.0, appearance, effect: Some(effect), count: 9, spread: 30.0, homing: 0.0, blast: 0.0 });

        assert_eq!(g.arrows.len(), 200, "the existing global arrow cap is preserved");
        let volley: Vec<_> = g.arrows.iter().filter(|arrow| arrow.modded).collect();
        assert_eq!(volley.len(), 5, "one event expands to at most five arrows");
        assert!(volley.iter().all(|arrow| {
            arrow.pos == pos && arrow.vel.y == center.y && arrow.damage == 0.0 && arrow.appearance == appearance && arrow.effect == Some(effect)
        }), "every arrow shares the spawn, vertical lob, and effect-only payload");
        assert_eq!(g.sounds.iter().filter(|(sound, _)| matches!(sound, Sfx::Twang)).count(), 1, "one twang per volley");
    }

    #[test]
    fn projectile_effect_requires_confirmed_hit_and_defaults_remain_inert() {
        let effect = ProjectileEffect { kind: crate::potions::Potion::Speed, duration: 12.0, amplifier: 2 };

        let mut hit = arena(76);
        let health = hit.player.health;
        let mut arrow = Arrow::new(hit.player.body.pos + Vec3::new(-0.1, 0.9, 0.0), Vec3::X, None, 0.0);
        arrow.effect = Some(effect);
        hit.arrows.push(arrow);
        hit.update_arrows(0.01);
        assert_eq!(hit.player.health, health, "effect-only arrows deal no damage");
        assert_eq!(hit.effect_amplifier(crate::potions::Potion::Speed), Some(2));
        assert!(hit.arrows.is_empty());

        let mut wall = arena(77);
        wall.world.set(3, 50, 0, STONE);
        let mut arrow = Arrow::new(Vec3::new(2.5, 50.5, 0.5), Vec3::X * 20.0, None, 0.0);
        arrow.effect = Some(effect);
        wall.arrows.push(arrow);
        wall.update_arrows(0.05);
        assert!(!wall.has_effect(crate::potions::Potion::Speed), "wall impact cannot apply an effect");
        assert!(wall.arrows[0].stuck);

        let mut miss = arena(78);
        let mut arrow = Arrow::new(Vec3::new(5.0, 55.0, 5.0), Vec3::X, None, 0.0);
        arrow.effect = Some(effect);
        miss.arrows.push(arrow);
        miss.update_arrows(0.05);
        assert!(!miss.has_effect(crate::potions::Potion::Speed), "a miss cannot apply an effect");

        let mut default = arena(79);
        default.arrows.push(Arrow::new(default.player.body.pos + Vec3::new(-0.1, 0.9, 0.0), Vec3::X, None, 0.0));
        default.update_arrows(0.01);
        assert!(default.effects.is_empty(), "ordinary arrows carry no effect");
        let mirrored = Arrow::from_wire(Vec3::ZERO, Vec3::X, ProjectileAppearance::default());
        assert_eq!(mirrored.effect, None, "client snapshots cannot carry effects");
    }

    #[test]
    fn homing_projectiles_turn_at_a_bounded_rate_and_fizzle_out() {
        let mut g = arena(91);
        g.player.body.pos = Vec3::new(0.5, 50.0, 6.5);
        // Fired along +X, past the player off to the side (+Z).
        let spec = ModProjectileSpec { damage: 2.0, appearance: ProjectileAppearance::default(), effect: None, count: 1, spread: 0.0, homing: 90.0, blast: 0.0 };
        g.spawn_mod_projectiles(Vec3::new(-6.0, 51.0, 0.5), Vec3::X * 10.0, spec);
        assert!(g.arrows[0].life <= HOMING_LIFE, "homing shots have a short life");
        let before = g.arrows[0].vel.normalize();
        g.update_arrows(0.1);
        let a = &g.arrows[0];
        let turned = before.angle_between(a.vel.normalize()).to_degrees();
        assert!(turned > 1.0 && turned <= 9.0 + 0.01, "turned {turned} degrees in 0.1 s at 90 degrees per second");
        assert!(a.vel.z > 0.0, "it turns toward the player");
        assert!((a.vel.length() - 10.0).abs() < 0.01, "homing keeps its speed (no gravity)");
        // Eventually it reaches the player.
        let health = g.player.health;
        for _ in 0..60 {
            g.update_arrows(0.05);
        }
        assert!(g.player.health < health, "the seeker found its mark");
        // One that never finds anyone fizzles out.
        let mut lonely = arena(92);
        lonely.dead = Some("gone".into());
        lonely.spawn_mod_projectiles(Vec3::new(0.0, 55.0, 0.0), Vec3::X * 0.5, spec);
        for _ in 0..((HOMING_LIFE / 0.1) as usize + 2) {
            lonely.update_arrows(0.1);
        }
        assert!(lonely.arrows.is_empty());
    }

    #[test]
    fn blast_projectiles_hurt_everyone_nearby_but_never_blocks() {
        let effect = ProjectileEffect { kind: crate::potions::Potion::Speed, duration: 5.0, amplifier: 0 };
        let spec = ModProjectileSpec { damage: 6.0, appearance: ProjectileAppearance::default(), effect: Some(effect), count: 1, spread: 0.0, homing: 0.0, blast: 3.0 };
        // Landing on the floor a block and a half from the player still catches them.
        let mut g = arena(93);
        let health = g.player.health;
        g.spawn_mod_projectiles(Vec3::new(2.0, 51.0, 0.5), Vec3::new(0.0, -10.0, 0.0), spec);
        for _ in 0..10 {
            g.update_arrows(0.05);
        }
        assert!(g.arrows.is_empty(), "a blast shot is used up when it lands");
        assert!(g.player.health < health && g.player.health > health - 6.0, "splash damage, less than a direct hit: {}", g.player.health);
        assert!(g.has_effect(crate::potions::Potion::Speed));
        assert_eq!(g.world.get(1, 49, 0), STONE, "blasts never break blocks");
        assert_eq!(g.world.get(2, 49, 0), STONE);
        // Out of range: nothing.
        let mut far = arena(94);
        let health = far.player.health;
        far.spawn_mod_projectiles(Vec3::new(9.0, 51.0, 9.0), Vec3::new(0.0, -10.0, 0.0), spec);
        for _ in 0..10 {
            far.update_arrows(0.05);
        }
        assert_eq!(far.player.health, health);
        assert!(!far.has_effect(crate::potions::Potion::Speed));
    }

    #[test]
    fn projectile_effect_ignores_local_spectator() {
        let mut spectator = arena(81);
        spectator.spectator = true;
        let health = spectator.player.health;
        let pos = spectator.player.body.pos + Vec3::new(-0.1, 0.9, 0.0);

        let effect = ProjectileEffect { kind: crate::potions::Potion::Speed, duration: 12.0, amplifier: 2 };
        let mut effect_only = Arrow::new(pos, Vec3::X, None, 0.0);
        effect_only.effect = Some(effect);
        spectator.arrows.push(effect_only);
        spectator.arrows.push(Arrow::new(pos, Vec3::X, None, 6.0));

        spectator.update_arrows(0.01);

        assert_eq!(spectator.player.health, health, "spectators cannot be damaged by projectiles");
        assert!(!spectator.has_effect(crate::potions::Potion::Speed), "spectators cannot receive projectile effects");
        assert_eq!(spectator.arrows.len(), 2, "overlapping projectiles pass through spectators without being consumed");
    }

    #[test]
    fn bows_shoot_mobs_and_rattlers_shoot_back() {
        let mut g = arena(21);
        // A Mooer straight ahead (-Z), and a bow with one Pointy Stick.
        g.alloc_mob(MobKind::Mooer, Vec3::new(0.5, 50.0, -6.5));
        let before = g.mobs[0].health;
        g.inv.slots[g.inv.selected] = Some((BOW, 1));
        g.inv.slots[1] = Some((ARROW, 1));
        g.player.yaw = 0.0;
        g.player.pitch = -0.05;
        g.shoot_bow();
        assert_eq!(g.inv.count(ARROW), 0, "the arrow is used up");
        assert_eq!(g.arrows.len(), 1);
        for _ in 0..30 {
            g.update_arrows(0.02);
        }
        assert!(g.mobs[0].health < before, "the arrow hit the Mooer");
        assert!(g.arrows.is_empty());
        assert!(g.advancements.has("robin_hood"));
        g.shoot_bow();
        assert!(g.arrows.is_empty(), "no Pointy Sticks, no shot");

        // A Rattler at night, with a clear line of sight, shoots within a few seconds.
        g.mobs.clear();
        g.time = 0.75;
        g.alloc_mob(MobKind::Rattler, Vec3::new(0.5, 50.0, -9.5));
        let health = g.player.health;
        for _ in 0..200 {
            g.update_entities(0.02);
            g.player.hurt = 0.0;
        }
        assert!(g.player.health < health, "the Rattler hit the player ({} left)", g.player.health);
    }

    #[test]
    fn ranged_modded_mob_shoots_and_damages_the_player() {
        // A hostile modded mob with ranged_damage > 0 fires base-game arrows at
        // a visible player within ranged_range, reusing the whole arrow path, and
        // only on its data-driven cooldown cadence.
        let src = "[mob slinger]\nname = Slinger\ntexture = stone\ntemplate = biped\nhostile = true\nranged_damage = 6\nranged_range = 22\nprojectile_speed = 24\nranged_cooldown = 2\naggro_range = 12\n";
        crate::mods::with_mods(&[("zoo", src)], |_reg| {
            let k = MobKind::from_name("zoo:slinger").expect("resolves");
            let mut g = arena(31);
            // Clear line of sight down -Z, well beyond any melee reach.
            g.alloc_mob(k, Vec3::new(0.5, 50.0, -9.5));
            let health = g.player.health;
            // One shot flies and hits within a couple of seconds.
            let mut first_hit = 0;
            for t in 0..200 {
                g.update_entities(0.02);
                g.player.hurt = 0.0;
                if g.player.health < health && first_hit == 0 {
                    first_hit = t;
                }
            }
            assert!(g.player.health < health, "the Slinger shot the player ({} left)", g.player.health);
            assert!(first_hit > 0, "an arrow actually travelled before landing");

            // The kill is attributed to a generic monster, not misblamed on a
            // Rattler: a modded slinger's arrow carries a generic death cause.
            g.player.health = 1.0;
            for _ in 0..400 {
                g.update_entities(0.02);
                if g.dead.is_some() {
                    break;
                }
                g.player.hurt = 0.0; // keep taking hits until one is lethal
            }
            let death = g.dead.as_deref().expect("the Slinger eventually killed the player");
            assert!(!death.contains("Rattler"), "a modded shot must not be blamed on a Rattler: {death:?}");
            assert!(death.contains("was shot down by a monster"), "modded shot uses the generic cause: {death:?}");
        });
    }

    #[test]
    fn the_same_seed_plays_out_the_same_way() {
        // Tests (and bug reports) rely on a world doing the same thing twice.
        let run = || {
            let mut g = arena(57);
            for (i, k) in [MobKind::Oinker, MobKind::Mooer, MobKind::Groaner, MobKind::Rattler].into_iter().enumerate() {
                g.alloc_mob(k, Vec3::new(-6.5 + 4.0 * i as f32, 50.0, 6.5));
            }
            for _ in 0..300 {
                g.update_entities(0.05);
            }
            g.mobs.iter().map(|m| (m.kind, (m.body.pos * 1000.0).round(), m.health)).collect::<Vec<_>>()
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn modded_fliers_cruise_swoop_and_land_when_told_to_sit() {
        let src = "[mob bat]\nname = Bat\ntexture = stone\ntemplate = bird\nflying = true\nfly_height = 5\nfly_speed = 4\ntame_item = bone\ntame_chance = 1\n\n[mob owl]\ntexture = stone\ntemplate = bird\nflying = true\nperches = true\n\n[mob swooper]\ntexture = stone\ntemplate = bird\nflying = true\nfly_height = 6\nhostile = true\nattack_damage = 3\naggro_range = 20\n";
        crate::mods::with_mods(&[("zoo", src)], |_reg| {
            let bat = MobKind::from_name("zoo:bat").expect("resolves");
            assert!(bat.flies());
            let mut g = arena(41);
            // A wandering bat can cross the arena's edge, where it would
            // cruise over the real (lower) terrain: give it the whole of the
            // loaded ground to fly over.
            for x in -16..32 {
                for z in -16..32 {
                    g.world.set(x, 49, z, STONE);
                    for y in 50..62 {
                        g.world.set(x, y, z, AIR);
                    }
                }
            }
            g.alloc_mob(bat, Vec3::new(5.5, 50.0, 5.5));
            for _ in 0..200 {
                g.update_entities(0.05);
            }
            let m = &g.mobs[0];
            assert!((53.5..=56.5).contains(&m.body.pos.y), "cruises about 5 blocks up: {:?}", m.body.pos);
            // Tamed, it sits (and lands); called along, it flies to its owner.
            let me = crate::players::record_key(&g.player_name.clone());
            let (id, at) = (g.mobs[0].id, g.mobs[0].body.pos);
            assert_eq!(g.interact_mob(&me, at, id, BONE), crate::animals::Interaction::Ate);
            assert!(g.mobs[0].sitting && g.mobs[0].owner.is_some());
            for _ in 0..120 {
                g.update_entities(0.05);
            }
            assert!(g.mobs[0].body.pos.y < 51.0, "a sitting flier lands: y = {}", g.mobs[0].body.pos.y);
            let at = g.mobs[0].body.pos;
            assert_eq!(g.interact_mob(&me, at, id, AIR), crate::animals::Interaction::Toggled);
            g.player.body.pos = Vec3::new(-6.5, 50.0, -6.5);
            for _ in 0..200 {
                g.update_entities(0.05);
            }
            let m = &g.mobs[0];
            let flat = Vec3::new(m.body.pos.x - g.player.body.pos.x, 0.0, m.body.pos.z - g.player.body.pos.z).length();
            assert!(flat < 4.0 && m.body.pos.y > 50.5, "follows its owner through the air: {:?}", m.body.pos);

            // A percher lands for a rest now and then, then takes off again.
            let mut g = arena(44);
            g.alloc_mob(MobKind::from_name("zoo:owl").unwrap(), Vec3::new(5.5, 54.0, 5.5));
            let (mut landed, mut flew) = (false, false);
            for _ in 0..1600 {
                g.update_entities(0.05);
                let m = &g.mobs[0];
                landed |= m.sitting && m.body.on_ground;
                flew |= landed && !m.sitting && m.body.pos.y > 52.0;
            }
            assert!(landed && flew, "perched (landed: {landed}) and took off again (flew: {flew})");

            // A hostile flier dives to bite.
            let mut g = arena(42);
            g.alloc_mob(MobKind::from_name("zoo:swooper").unwrap(), Vec3::new(4.5, 56.0, 0.5));
            let health = g.player.health;
            for _ in 0..200 {
                g.update_entities(0.05);
                g.player.hurt = 0.0;
            }
            assert!(g.player.health < health, "the swooper came down to bite");
        });
    }

    #[test]
    fn modded_mobs_tame_breed_and_tamed_monsters_stay_friendly() {
        let src = "[mob pup]\ntexture = stone\nhealth = 12\nhostile = true\nattack_damage = 4\naggro_range = 20\ntame_item = bone\ntame_chance = 1\nbreed_item = wheat\n";
        crate::mods::with_mods(&[("zoo", src)], |_reg| {
            let pup = MobKind::from_name("zoo:pup").expect("resolves");
            let mut g = arena(43);
            g.alloc_mob(pup, Vec3::new(1.5, 50.0, 0.5));
            g.alloc_mob(pup, Vec3::new(2.5, 50.0, 0.5));
            let me = crate::players::record_key(&g.player_name.clone());
            // Wild and hostile: wheat does nothing yet.
            let (id0, at0) = (g.mobs[0].id, g.mobs[0].body.pos);
            assert_eq!(g.interact_mob(&me, at0, id0, WHEAT), crate::animals::Interaction::Nothing);
            for i in 0..2 {
                let (id, at) = (g.mobs[i].id, g.mobs[i].body.pos);
                assert_eq!(g.interact_mob(&me, at, id, BONE), crate::animals::Interaction::Ate);
                assert!(!g.mobs[i].menacing());
            }
            // Tamed monsters right next to the player never bite them.
            let health = g.player.health;
            for i in 0..2 {
                let (id, at) = (g.mobs[i].id, g.mobs[i].body.pos);
                g.interact_mob(&me, at, id, AIR); // stand up
                assert!(!g.mobs[i].sitting);
            }
            for _ in 0..100 {
                g.update_entities(0.05);
            }
            assert_eq!(g.player.health, health, "a tamed monster is friendly");
            // Feed both: a baby, born tame.
            for i in 0..2 {
                let (id, at) = (g.mobs[i].id, g.mobs[i].body.pos);
                assert_eq!(g.interact_mob(&me, at, id, WHEAT), crate::animals::Interaction::Ate);
            }
            for _ in 0..400 {
                g.update_entities(0.05);
                if g.mobs.len() > 2 {
                    break;
                }
            }
            assert_eq!(g.mobs.len(), 3, "the pair had a baby");
            let baby = &g.mobs[2];
            assert!(baby.baby > 0.0 && baby.owner.as_deref() == Some(me.as_str()) && baby.kind == pup);
            // Saved and loaded, pets keep their owner.
            let bytes = crate::animals::encode_mobs(&g.mobs, &HashMap::new());
            let back = crate::animals::decode_mobs(&bytes, &mut crate::noise::Rng::new(1));
            assert!(back.iter().all(|m| m.owner.as_deref() == Some(me.as_str()) && m.kind == pup), "{}", back.len());
        });
    }

    #[test]
    fn modded_traders_trade_through_the_hmmer_screen() {
        let src = "[mob merchant]\nname = Cheese Merchant\ntexture = stone\ntrade = gold 2 -> bread 3\ntrade = bone + string 2 for diamond\n\n[mob crook]\ntexture = stone\nhostile = true\nattack_damage = 2\ntrade = dirt -> diamond 64\n";
        crate::mods::with_mods(&[("shop", src)], |_reg| {
            let mut g = arena(51);
            g.creative = false;
            let id = g.alloc_mob(MobKind::from_name("shop:merchant").unwrap(), Vec3::new(1.5, 50.0, 0.5));
            let i = g.mobs.iter().position(|m| m.id == id).unwrap();
            g.inv.add(GOLD_INGOT, 5);
            assert!(g.use_on_mob(i));
            assert_eq!(g.trading, Some(id));
            let (title, list) = g.trade_list().expect("talking");
            assert_eq!(title, "Cheese Merchant");
            assert_eq!(list.len(), 2);
            assert_eq!(list[1].give, [(BONE, 1), (STRING, 2)]);
            g.make_trade(0);
            assert_eq!((g.inv.count(GOLD_INGOT), g.inv.count(BREAD)), (3, 3));
            // Can't afford the second.
            g.make_trade(1);
            assert_eq!(g.inv.count(DIAMOND), 0);
            // Out of stock after six.
            g.inv.add(GOLD_INGOT, 64);
            for _ in 0..10 {
                g.make_trade(0);
            }
            assert_eq!(g.inv.count(BREAD), 3 * crate::villagers::STOCK as u32);
            // A monster on the loose doesn't trade.
            let crook = g.alloc_mob(MobKind::from_name("shop:crook").unwrap(), Vec3::new(-1.5, 50.0, 0.5));
            let m = g.mobs.iter().find(|m| m.id == crook).unwrap();
            assert!(crate::villagers::trades_of(m).is_none());
        });
    }

    #[test]
    fn modded_bosses_enrage_summon_resist_taming_and_are_announced() {
        let src = "[mob minion]\ntexture = stone\nhealth = 4\n\n[mob king]\nname = Cheese King\ntexture = stone\ntemplate = biped\nhealth = 40\nhostile = true\nattack_damage = 2\nattack_cooldown = 2\naggro_range = 30\nboss = true\nenrage_at = 0.5\nenrage_speed = 2\nenrage_cooldown = 0.5\nsummon = minion 3\nsummon_every = 5\nxp = 300\ntame_item = bone\ntame_chance = 1\n";
        crate::mods::with_mods(&[("court", src)], |_reg| {
            let king = MobKind::from_name("court:king").unwrap();
            let minion = MobKind::from_name("court:minion").unwrap();
            let def = king.mod_def().unwrap();
            assert!(def.boss && def.knockback_resist > 0.5, "bosses shrug off knockback by default");
            assert_eq!(king.xp_value(1.0, &mut crate::noise::Rng::new(1)), 300);
            let mut g = arena(52);
            let id = g.alloc_mob(king, Vec3::new(6.5, 50.0, 0.5));
            let i = g.mobs.iter().position(|m| m.id == id).unwrap();
            assert!(g.mobs[i].persistent, "a boss never despawns");
            // It can't be tamed.
            let me = crate::players::record_key(&g.player_name.clone());
            let at = g.mobs[i].body.pos;
            g.interact_mob(&me, at, id, BONE);
            assert!(g.mobs[i].owner.is_none());
            // Knockback barely moves it.
            g.mobs[i].damage(1.0, g.player.body.pos);
            assert!(g.mobs[i].body.vel.y < 3.0, "barely knocked up");
            // Calm at full health: no help called.
            for _ in 0..100 {
                g.update_entities(0.05);
                g.player.health = 20.0;
            }
            assert_eq!(g.mobs.iter().filter(|m| m.kind == minion).count(), 0);
            // Hurt below half, it enrages and calls minions, never more than eight around.
            let i = g.mobs.iter().position(|m| m.id == id).unwrap();
            g.mobs[i].health = 15.0;
            for _ in 0..1200 {
                g.update_entities(0.05);
                g.player.health = 20.0;
                g.player.hurt = 0.0;
            }
            let b = g.mobs.iter().find(|m| m.id == id).expect("still there");
            assert!(b.angry, "enraged");
            let minions = g.mobs.iter().filter(|m| m.kind == minion).count();
            assert!((3..=8).contains(&minions), "{minions} minions");
            // Beaten: announced to everyone.
            let i = g.mobs.iter().position(|m| m.id == id).unwrap();
            g.mobs[i].health = -1.0;
            g.update_entities(0.05);
            assert!(g.chat_log.iter().any(|l| l == "The Cheese King has been defeated!"), "{:?}", g.chat_log);
        });
    }

    #[test]
    fn observers_pulse_bulbs_toggle_and_crafters_craft() {
        use crate::contraptions::{observer, observer_state};
        let zap = |g: &mut Game, secs: f32| {
            for _ in 0..(secs / 0.05) as usize {
                g.zap_tick(0.05);
            }
        };
        // An observer looking east at (2, 50, 0), a lamp behind it.
        let mut g = arena(61);
        let (obs, front, lamp) = (IVec3::new(1, 50, 0), IVec3::new(2, 50, 0), IVec3::new(0, 50, 0));
        g.world.set_v(lamp, LAMP);
        g.world.set_v(obs, observer(crate::contraptions::facing_of(IVec3::X), false));
        zap(&mut g, 0.3);
        assert_eq!(g.world.get_v(lamp), LAMP, "nothing changed yet");
        g.world.set_v(front, STONE);
        let mut lit = false;
        for _ in 0..10 {
            g.zap_tick(0.05);
            lit |= g.world.get_v(lamp) == LAMP_ON && observer_state(g.world.get_v(obs)).1;
        }
        assert!(lit, "a change in front sends a pulse out of the back");
        zap(&mut g, 0.6);
        assert_eq!(g.world.get_v(lamp), LAMP, "and the pulse ends");
        assert!(!observer_state(g.world.get_v(obs)).1);

        // A copper bulb flips each time power arrives.
        let mut g = arena(62);
        let (bulb, lever) = (IVec3::new(0, 50, 0), IVec3::new(1, 50, 0));
        g.world.set_v(bulb, COPPER_BULB);
        g.world.set_v(lever, LEVER);
        zap(&mut g, 0.2);
        let mut states = Vec::new();
        for on in [true, false, true, false] {
            g.world.set_v(lever, if on { LEVER_ON } else { LEVER });
            zap(&mut g, 0.3);
            states.push(g.world.get_v(bulb) == COPPER_BULB_ON);
        }
        assert_eq!(states, [true, true, false, false], "on, stays on, off, stays off");
        assert!(block(COPPER_BULB_ON).light > 10.0);

        // A crafter makes the recipe its grid adds up to when powered.
        let mut g = arena(63);
        let (crafter, lever) = (IVec3::new(0, 50, 0), IVec3::new(0, 50, 1));
        g.world.set_v(crafter, CRAFTER_FIRST + crate::contraptions::facing_of(IVec3::X) as Id);
        g.world.set_v(lever, LEVER);
        if let Some(c) = g.world.containers.get_mut(&crafter) {
            c.slots[0] = Some((PLANKS, 1));
            c.slots[4] = Some((PLANKS, 1));
        }
        zap(&mut g, 0.2);
        g.world.set_v(lever, LEVER_ON);
        zap(&mut g, 0.3);
        assert!(g.drops.iter().any(|d| d.item == STICK && d.n == 4), "planks into sticks");
        assert!(g.world.containers[&crafter].slots.iter().all(|s| s.is_none()), "the grid is used up");
        // Spare ingredients: no recipe, nothing made.
        assert!(crate::contraptions::crafter_recipe(&[Some((PLANKS, 2)), Some((DIRT, 1))]).is_none());
        assert!(crate::contraptions::crafter_recipe(&[None; 9]).is_none());
    }

    #[test]
    fn ranged_off_by_default_modded_mob_never_fires() {
        // A plain hostile modded mob with no ranged fields never spawns an arrow
        // and never ranged-damages the player; a melee-only one likewise never
        // fires (it only melees).
        let passive = "[mob lurker]\nname = Lurker\ntexture = stone\ntemplate = biped\nhostile = true\naggro_range = 20\n";
        crate::mods::with_mods(&[("zoo", passive)], |_reg| {
            let k = MobKind::from_name("zoo:lurker").expect("resolves");
            let mut g = arena(32);
            g.alloc_mob(k, Vec3::new(0.5, 50.0, -6.5));
            for _ in 0..200 {
                g.update_entities(0.02);
                assert!(g.arrows.is_empty(), "ranged off by default: no arrow ever spawns");
            }
        });

        // Melee-only: it closes in and bites, but still never fires an arrow.
        let melee = "[mob biter]\nname = Biter\ntexture = stone\ntemplate = biped\nhostile = true\nattack_damage = 4\nattack_reach = 1.5\naggro_range = 20\nattack_cooldown = 1.0\n";
        crate::mods::with_mods(&[("zoo", melee)], |_reg| {
            let k = MobKind::from_name("zoo:biter").expect("resolves");
            let mut g = arena(33);
            g.alloc_mob(k, Vec3::new(0.5, 50.0, -3.5));
            for _ in 0..200 {
                g.update_entities(0.02);
                g.player.hurt = 0.0;
                assert!(g.arrows.is_empty(), "a melee-only mob never fires an arrow");
            }
        });
    }

    #[test]
    fn bloops_split_and_clucksters_flutter() {
        let mut g = arena(22);
        g.creative = true; // nobody gets hurt
        g.alloc_mob_sized(MobKind::Bloop, Vec3::new(4.5, 50.0, 4.5), 4);
        assert_eq!(g.mobs[0].body.height, 0.52 * 4.0);
        g.mobs[0].health = 0.0;
        g.update_entities(0.02);
        let kids: Vec<f32> = g.mobs.iter().filter(|m| m.kind == MobKind::Bloop).map(|m| m.size).collect();
        assert!(kids.len() >= 2 && kids.iter().all(|&s| s == 2.0), "{kids:?}");

        g.mobs.clear();
        g.alloc_mob(MobKind::Cluckster, Vec3::new(-4.5, 58.0, -4.5));
        for _ in 0..20 {
            g.update_entities(0.02);
        }
        assert!(g.mobs[0].body.vel.y >= -2.5, "Clucksters flutter down ({})", g.mobs[0].body.vel.y);
    }

    #[test]
    fn webbers_climb_walls() {
        let mut g = arena(23);
        g.time = 0.75; // night: Webbers hunt
        // A wall between the Webber and the player, who stands on top of it.
        for x in -12..12 {
            for y in 50..54 {
                g.world.set(x, y, -3, STONE);
            }
        }
        g.player.body.pos = Vec3::new(0.5, 54.0, -2.5);
        g.alloc_mob(MobKind::Webber, Vec3::new(0.5, 50.0, -7.5));
        let mut top = 0.0f32;
        for _ in 0..200 {
            g.update_entities(0.02);
            top = top.max(g.mobs[0].body.pos.y);
        }
        assert!(top > 53.0, "climbed to {top}");
    }

    #[test]
    fn farming_from_seed_to_bread() {
        use crate::farming::Crop;
        let mut g = arena(31);
        g.time = 0.25; // noon
        let (soil, water) = (IVec3::new(0, 49, -3), IVec3::new(2, 49, -3));
        g.world.set_v(soil, DIRT);
        g.world.set_v(water, WATER);
        g.player.body.pos = Vec3::new(0.5, 50.0, 0.5);
        g.inv.slots[g.inv.selected] = Some((HOE, 1));
        assert!(g.farm_use(HOE, soil));
        assert_eq!(g.world.get_v(soil), FARMLAND);
        assert!(g.world.farm.contains_key(&soil), "tilling makes a soil record");
        g.inv.slots[g.inv.selected] = Some((WHEAT_SEEDS, 2));
        assert!(g.farm_use(WHEAT_SEEDS, soil));
        let above = soil + IVec3::Y;
        assert_eq!(g.world.get_v(above), Crop::Wheat.block(0));
        assert_eq!(g.inv.count(WHEAT_SEEDS), 1);

        // Water nearby hydrates it; sun, water, food and company grow it in a few minutes.
        for _ in 0..200 {
            g.farm_tick(1.0);
        }
        assert_eq!(g.world.get_v(soil), FARMLAND_WET);
        assert_eq!(g.world.get_v(above), Crop::Wheat.block(3), "{}", g.world.farm[&soil].report(1.0, true));
        assert!(g.world.farm[&soil].nutrients[0] < 60.0, "wheat ate some nitrogen");
        g.break_block(above, true);
        collect(&mut g);
        assert!(g.inv.count(WHEAT) >= 1 && g.advancements.has("green_thumb"));
        g.farm_tick(1.0);
        assert_eq!(g.world.farm[&soil].last, Some(Crop::Wheat));

        // A different crop next: rotation bonus.
        g.world.set_v(above, Crop::Carrot.block(0));
        g.farm_tick(1.0);
        assert!(g.world.farm[&soil].rotated());
        let report = g.farm_interact(soil, SOIL_PROBE).unwrap();
        assert!(report.contains("rotation bonus"), "{report}");
        assert!(g.farm_interact(soil, COMPOST).unwrap().contains("Nitrogen"));

        // Clucksters eat seedlings, unless there's a Scarecrow.
        g.alloc_mob(MobKind::Cluckster, above.as_vec3() + Vec3::new(1.0, 0.0, 0.5));
        g.world.set_v(IVec3::new(-4, 50, -6), SCARECROW);
        for _ in 0..60 {
            g.farm_tick(1.0);
            g.mobs[0].body.pos = above.as_vec3() + Vec3::new(1.0, 0.0, 0.5);
        }
        assert!(Crop::of_block(g.world.get_v(above)).is_some(), "the Scarecrow kept watch");
        g.world.set_v(IVec3::new(-4, 50, -6), AIR);
        g.world.set_v(above, Crop::Carrot.block(0));
        for _ in 0..200 {
            if g.world.get_v(above) == AIR {
                break;
            }
            g.world.set_v(above, Crop::Carrot.block(0)); // keep it a seedling
            g.world.farm.get_mut(&soil).unwrap().progress = 0.0;
            g.farm_tick(1.0);
            g.mobs[0].body.pos = above.as_vec3() + Vec3::new(1.0, 0.0, 0.5);
        }
        assert_eq!(g.world.get_v(above), AIR, "pecked");

        // Jumping on farmland tramples it; dry, bare farmland gives up on its own.
        g.trample(soil, 2.0);
        assert_eq!(g.world.get_v(soil), DIRT);
        assert!(!g.world.farm.contains_key(&soil));
        let dry = IVec3::new(-6, 49, 6);
        g.world.set_v(dry, FARMLAND);
        for _ in 0..130 {
            g.farm_tick(1.0);
            if g.world.get_v(dry + IVec3::Y) == WEEDS {
                g.world.set_v(dry + IVec3::Y, AIR);
            }
        }
        assert_eq!(g.world.get_v(dry), DIRT);
    }

    #[test]
    fn fishing_catches_things() {
        use crate::fishing::{Bobber, BobberState};
        let mut g = arena(32);
        // A 6x6 pond, 3 deep.
        for x in 2..8 {
            for z in -8..-2 {
                for y in 47..50 {
                    g.world.set(x, y, z, WATER);
                }
            }
        }
        g.inv.slots[g.inv.selected] = Some((ROD, 1));
        let spot = Vec3::new(4.5, 49.9, -5.5);
        // Reeling in on a nibble is too early.
        g.bobber = Some(Bobber { pos: spot, vel: Vec3::ZERO, state: BobberState::Floating { wait: 5.0, nibble: 2.0 }, fight: None, since_nibble: 0.1, bait: false });
        g.use_rod();
        assert!(g.bobber.is_none() && g.messages.iter().any(|m| m.0.contains("Too early")));

        // A real bite: reel in, and win the tug-of-war if it's a big one.
        for _ in 0..5 {
            g.bobber = Some(Bobber { pos: spot, vel: Vec3::ZERO, state: BobberState::Biting { window: 1.0 }, fight: None, since_nibble: 9.0, bait: false });
            g.use_rod();
            for _ in 0..3000 {
                let Some(b) = &g.bobber else { break };
                let easy = b.fight.map(|f| f.tension < 0.45).unwrap_or(false);
                g.update_fishing(0.02, easy);
            }
            assert!(g.bobber.is_none());
        }
        assert!(g.fish_log.xp > 0, "{:?}", g.messages);
        assert!(g.advancements.has("gone_fishin"));
        let caught: u32 = g.fish_log.species.values().map(|v| v.0).sum();
        assert_eq!(caught, 5);

        // Casting for real: the bobber flies, lands in the pond, and floats.
        g.player.body.pos = Vec3::new(0.5, 50.0, -5.5);
        g.player.yaw = std::f32::consts::FRAC_PI_2; // +X, toward the pond
        g.player.pitch = -0.3;
        g.use_rod();
        for _ in 0..200 {
            g.update_fishing(0.02, false);
        }
        assert!(matches!(g.bobber.as_ref().map(|b| b.state), Some(BobberState::Floating { .. } | BobberState::Biting { .. })));
        // Switching away from the rod reels it in.
        g.inv.slots[g.inv.selected] = None;
        g.update_fishing(0.02, false);
        assert!(g.bobber.is_none());
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

    #[test]
    fn you_drown_without_air_and_a_turtle_shell_helps() {
        let mut g = arena(231);
        let p = ivec3(2, 50, 2);
        for y in 0..3 {
            g.world.set_v(p + IVec3::Y * y, WATER);
        }
        g.player.body.pos = p.as_vec3() + Vec3::new(0.5, 0.0, 0.5);
        let full = g.player.health;
        // Fifteen seconds of breath, then a heart a second.
        for _ in 0..(14.0 / 0.05) as i32 {
            g.breath_tick(0.05);
        }
        assert_eq!(g.player.health, full, "still holding it");
        for _ in 0..(3.0 / 0.05) as i32 {
            g.breath_tick(0.05);
        }
        assert!(g.player.health <= full - 2.0 && g.player.health >= full - 6.0, "{}", g.player.health);
        // Back in the air it comes back quickly.
        g.world.set_v(p + IVec3::Y, AIR);
        g.world.set_v(p + IVec3::Y * 2, AIR);
        for _ in 0..(2.0 / 0.05) as i32 {
            g.breath_tick(0.05);
        }
        assert!(g.player.air >= crate::player::MAX_AIR - 0.01);
        g.inv.armor[0] = Some((TURTLE_SHELL, 1));
        assert_eq!(g.max_air(), crate::player::MAX_AIR + crate::player::SHELL_AIR);
        // Creative players don't need air.
        g.creative = true;
        g.world.set_v(p + IVec3::Y, WATER);
        g.player.air = 0.5;
        let h = g.player.health;
        for _ in 0..40 {
            g.breath_tick(0.05);
        }
        assert_eq!(g.player.health, h);
    }

    #[test]
    fn frogs_climb_out_of_ponds_and_lily_pads_hold_you_up() {
        let mut g = arena(232);
        // A pond two deep, its banks a block above the water.
        for x in -2..=2 {
            for z in -2..=2 {
                g.world.set(x, 47, z, STONE);
                g.world.set(x, 48, z, WATER);
                g.world.set(x, 49, z, WATER);
            }
        }
        g.world.set(0, 50, 2, LILY_PAD);
        let frog = g.alloc_mob(MobKind::Ribbit, Vec3::new(0.5, 48.2, 0.5));
        let mut out = false;
        for _ in 0..1200 {
            let world = &g.world;
            for m in g.mobs.iter_mut() {
                m.update(0.05, world, Vec3::new(0.0, 50.0, -30.0), false, 0.3, &mut g.rng);
            }
            let f = g.mobs.iter().find(|m| m.id == frog).unwrap();
            if f.body.pos.y >= 50.0 && !f.body.in_water && f.body.on_ground {
                out = true;
                break;
            }
        }
        assert!(out, "the frog got out");
        // Something dropped onto the lily pad stands on it...
        let mut b = crate::entity::Body::new(Vec3::new(0.5, 51.5, 2.5), 0.3, 1.8);
        for _ in 0..40 {
            b.vel.y -= crate::entity::GRAVITY * 0.05;
            crate::entity::move_body(&g.world, &mut b, 0.05, false);
        }
        assert!(b.on_ground && (b.pos.y - 50.0625).abs() < 0.01, "{}", b.pos.y);
        // ...but a swimmer comes up through it.
        let mut s = crate::entity::Body::new(Vec3::new(0.5, 48.5, 2.5), 0.3, 1.8);
        s.vel.y = 4.0;
        crate::entity::move_body(&g.world, &mut s, 0.3, false);
        assert!(s.pos.y > 49.5, "{}", s.pos.y);
    }

    #[test]
    fn torches_go_on_walls_and_fall_off_with_them() {
        let mut g = arena(233);
        let wall = ivec3(3, 51, 3);
        g.world.set_v(wall, STONE);
        // Facing +x out from the wall's east side.
        let f = crate::decor::frame_facing(IVec3::X).unwrap();
        let torch = wall + IVec3::X;
        g.world.set_v(torch, WALL_TORCH_FIRST + f as Id);
        assert_eq!(crate::decor::outward(f).as_ivec3(), IVec3::X);
        assert_eq!(placing_item(WALL_TORCH_FIRST + f as Id), Some(TORCH));
        assert_eq!(crate::light::emission(WALL_TORCH_FIRST + f as Id), crate::light::emission(TORCH));
        // Breaking the block under it leaves it be; breaking its wall drops it.
        g.break_block(torch - IVec3::Y, false);
        assert_eq!(g.world.get_v(torch), WALL_TORCH_FIRST + f as Id);
        g.break_block(wall, false);
        assert_eq!(g.world.get_v(torch), AIR);
    }

    #[test]
    fn a_worlds_generation_options_are_kept() {
        use crate::world::GenOptions;
        let opts = GenOptions { version: 1, structures: 1, biome_size: 3, terrain: 2 };
        let mut g = Game::new_with(236, false, false, opts);
        let back = Game::from_save(g.to_save());
        assert_eq!(back.world.generator.opts, opts);
        // Old worlds (no options saved) keep the old rules.
        let mut old = Game::new(237, false, false);
        assert_eq!(Game::from_save(old.to_save()).world.generator.opts, GenOptions::LEGACY);
    }
}
