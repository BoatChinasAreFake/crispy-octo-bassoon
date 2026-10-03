//! Shared AABB physics, mobs, particles and primed TNT.

use crate::block::*;
use crate::noise::Rng;
use crate::render::{DynGeo, Pass};
use crate::texture::*;
use crate::world::World;
use macroquad::math::{Mat4, Vec3};

pub const GRAVITY: f32 = 28.0;

#[derive(Clone)]
pub struct Body {
    pub pos: Vec3,
    pub vel: Vec3,
    pub half: f32,
    pub height: f32,
    pub on_ground: bool,
    pub hit_wall: bool,
    /// Swimming (in water or lava; `in_lava` says which).
    pub in_water: bool,
    pub in_lava: bool,
}

impl Body {
    pub fn new(pos: Vec3, half: f32, height: f32) -> Self {
        Body { pos, vel: Vec3::ZERO, half, height, on_ground: false, hit_wall: false, in_water: false, in_lava: false }
    }
    pub fn min(&self) -> Vec3 {
        self.pos - Vec3::new(self.half, 0.0, self.half)
    }
    pub fn max(&self) -> Vec3 {
        self.pos + Vec3::new(self.half, self.height, self.half)
    }
    pub fn intersects_block(&self, x: i32, y: i32, z: i32) -> bool {
        let (a, b) = (self.min(), self.max());
        a.x < x as f32 + 1.0 && b.x > x as f32 && a.y < y as f32 + 1.0 && b.y > y as f32 && a.z < z as f32 + 1.0 && b.z > z as f32
    }
}

/// The first block box overlapping the box `min..max`, in world coordinates.
fn collides(world: &World, min: Vec3, max: Vec3) -> Option<(Vec3, Vec3)> {
    const E: f32 = 1e-4;
    let low = (min.y + E).floor() as i32;
    // One row lower too: fences and shut gates reach above their cell.
    for y in low - 1..=(max.y - E).floor() as i32 {
        for z in (min.z + E).floor() as i32..=(max.z - E).floor() as i32 {
            for x in (min.x + E).floor() as i32..=(max.x - E).floor() as i32 {
                let id = world.get(x, y, z);
                if !is_solid(id) {
                    continue;
                }
                let tall = crate::carpentry::is_tall(id);
                if y < low && !tall {
                    continue;
                }
                let cell = Vec3::new(x as f32, y as f32, z as f32);
                let (mut boxes, n) = block_boxes(id);
                if tall {
                    for b in boxes.iter_mut() {
                        b.1[1] = crate::carpentry::TALL;
                    }
                }
                for &(a, b) in &boxes[..n] {
                    let (bmin, bmax) = (cell + Vec3::from_array(a), cell + Vec3::from_array(b));
                    if min.x < bmax.x - E && max.x > bmin.x + E && min.y < bmax.y - E && max.y > bmin.y + E && min.z < bmax.z - E && max.z > bmin.z + E {
                        return Some((bmin, bmax));
                    }
                }
            }
        }
    }
    None
}

/// Move one axis, resolving block collisions. Returns true if blocked.
fn move_axis(world: &World, b: &mut Body, axis: usize, d: f32) -> bool {
    if d == 0.0 {
        return false;
    }
    b.pos[axis] += d;
    let mut blocked = false;
    // A few iterations handle standing on block corners.
    for _ in 0..3 {
        let Some((bmin, bmax)) = collides(world, b.min(), b.max()) else { break };
        blocked = true;
        let (neg, pos_ext) = if axis == 1 { (0.0, b.height) } else { (b.half, b.half) };
        if d > 0.0 {
            b.pos[axis] = bmin[axis] - pos_ext - 1e-3;
        } else {
            b.pos[axis] = bmax[axis] + neg + 1e-3;
        }
    }
    if blocked {
        b.vel[axis] = 0.0;
    }
    blocked
}

/// Walking into something at most this tall (a slab, a stair) climbs it.
pub const STEP_UP: f32 = 0.55;

/// Try a horizontal move lifted by up to `STEP_UP`; keeps it only if that got further.
fn step_up(world: &World, b: &mut Body, axis: usize, d: f32) -> bool {
    let before = b.clone();
    b.pos.y += STEP_UP;
    if collides(world, b.min(), b.max()).is_some() {
        *b = before;
        return false;
    }
    let blocked = move_axis(world, b, axis, d);
    if blocked || (b.pos[axis] - before.pos[axis]).abs() < 1e-3 {
        *b = before;
        return false;
    }
    // Settle back down onto the step.
    move_axis(world, b, 1, -STEP_UP);
    true
}

pub fn has_support(world: &World, pos: Vec3, half: f32) -> bool {
    let min = Vec3::new(pos.x - half, pos.y - 0.1, pos.z - half);
    let max = Vec3::new(pos.x + half, pos.y - 0.01, pos.z + half);
    collides(world, min, max).is_some()
}

/// Integrate velocity with collisions. `edge_guard` keeps the body from walking off ledges.
pub fn move_body(world: &World, b: &mut Body, dt: f32, edge_guard: bool) {
    let d = b.vel * dt;
    let steps = ((d.abs().max_element() / 0.4).ceil() as i32).clamp(1, 20);
    let sd = d / steps as f32;
    b.on_ground = false;
    b.hit_wall = false;
    for _ in 0..steps {
        if move_axis(world, b, 1, sd.y) && sd.y < 0.0 {
            b.on_ground = true;
        }
        let grounded = b.on_ground || has_support(world, b.pos, b.half);
        for axis in [0, 2] {
            let before = b.pos;
            let snapshot = b.clone();
            if move_axis(world, b, axis, sd[axis]) {
                let mut lifted = snapshot;
                if grounded && step_up(world, &mut lifted, axis, sd[axis]) {
                    *b = lifted;
                } else {
                    b.hit_wall = true;
                }
            }
            if edge_guard && !has_support(world, b.pos, b.half * 0.9) && has_support(world, before, b.half * 0.9) {
                b.pos = before;
                b.vel[axis] = 0.0;
            }
        }
    }
    if !b.on_ground && b.vel.y <= 0.0 && has_support(world, b.pos, b.half) {
        b.on_ground = true;
    }
    let feet = world.get(b.pos.x.floor() as i32, (b.pos.y + 0.3).floor() as i32, b.pos.z.floor() as i32);
    b.in_water = is_liquid(feet);
    b.in_lava = is_lava(feet);
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MobKind {
    Oinker,
    Hisser,
    Groaner,
    Fluffer,
    Starer,
    Cluckster,
    Mooer,
    Rattler,
    Webber,
    Bloop,
    /// Wolf-ish: neutral in the wild, tameable with bones (see animals.rs).
    Woofer,
    /// Villager-ish: lives in a hut and trades (see villagers.rs).
    Hmmer,
    /// Pig-person-ish: roams the Scorchlands, minds its own business until you hit one of them.
    Grumbler,
    /// Horse-ish: wanders the plains; tame it, saddle it, ride it (see horses.rs).
    Galloper,
    /// Dragon-ish: the Hollow's boss (see hollow.rs).
    Wyrm,
    /// Parrot-ish: a loud, bright bird of the jungle. Flutters like a Cluckster.
    Squawker,
    /// Iron-golem-ish: guards a village from monsters (see golems.rs).
    Clanker,
    /// Fish-ish: swims in schools in the sea; flops about on land.
    Fishy,
    /// Drowned-ish: a waterlogged Groaner that swims after you, some with a Soggy Spear.
    Soggy,
    /// A bee out of its hive because someone upset it (see bees.rs).
    Bee,
    /// Fox-ish: naps by day, hunts Clucksters by night, steals your stuff (see critters.rs).
    Sneaker,
    /// Frog-ish: hops about swamps and eats small Bloops (see critters.rs).
    Ribbit,
    /// Armadillo-ish: rolls into a ball when startled; brush it for scutes (see critters.rs).
    Rollo,
    /// Warden-ish: blind, enormous, and drawn to any sound (see deepdark.rs).
    Hush,
    /// Blaze-ish: hovers about Scorchlands fortresses throwing fireballs (see fortress.rs).
    Sizzler,
    /// Ghast-ish: a huge, sad, floating thing that cries fireballs (see fortress.rs).
    Weeper,
    /// Strider-ish: walks on lava, shivers off it; saddle it and steer with a shroom (see fortress.rs).
    Strutter,
    /// Piglin-ish: trades for gold, and doesn't like you touching its stuff (see fortress.rs).
    Snout,
    /// Pillager-ish: a crossbow and a bad attitude (see raids.rs).
    Pilferer,
    /// Vindicator-ish: an axe and a worse attitude (see raids.rs).
    Hackler,
    /// Evoker-ish: summons Fees and sends Late Fees up out of the ground (see raids.rs).
    Invoicer,
    /// Vex-ish: a little flying charge that goes through walls (see raids.rs).
    Fee,
    /// Ravager-ish: a big angry beast the raiders bring along (see raids.rs).
    Rampager,
    /// A whirl of wind from the Trial Chambers: hops about and fires Wind Charges (see trial.rs).
    Breeze,
    /// Mountain goats: leap high, and now and then ram you (see critters.rs).
    Goat,
    /// Little pink salamanders: hunt Soggy Groaners with you underwater (see critters.rs).
    Axolotl,
    /// Tall desert beasts: saddle one and ride it, with room for a friend (see horses.rs).
    Camel,
    /// Woken by a Creaking Heart in the Pale Garden: moves only while nobody's
    /// looking, and can't be hurt; break its heart instead (see creaking.rs).
    Creaking,
    /// Huge, gentle and ancient: sniffs out old seeds (see sniffers.rs).
    Sniffer,
    /// Built from a pumpkin on copper: carries things from Copper Chests to
    /// the chests that already hold them (see golems.rs).
    CopperGolem,
    /// A big, gentle cloud of a creature: harness one and fly it, with up to
    /// three friends aboard (see floaty.rs).
    Floaty,
    /// An undead horse that wanders at night; tame and ride it like a Galloper.
    Rotsteed,
    /// A mob type defined by a mod (`[mob]` in mod.txt); indexes `reg().mobs`.
    /// Its wire/save index is `BASE_MOBS + i` (see `index`/`from_index`).
    Modded(u16),
}

/// How many base-game mob kinds there are (the length of `MobKind::ALL`).
/// Modded kinds take the indices after these, in registry order.
pub const BASE_MOBS: u8 = MobKind::ALL.len() as u8;

/// At most this many mod mobs, so a kind index always fits the one-byte wire
/// and save encoding alongside the base kinds.
pub const MAX_MOD_MOBS: usize = (u8::MAX as usize) - MobKind::ALL.len();

impl MobKind {
    /// Every base-game kind, in wire/script index order (append only).
    pub const ALL: [MobKind; 42] = [
        MobKind::Oinker,
        MobKind::Hisser,
        MobKind::Groaner,
        MobKind::Fluffer,
        MobKind::Starer,
        MobKind::Cluckster,
        MobKind::Mooer,
        MobKind::Rattler,
        MobKind::Webber,
        MobKind::Bloop,
        MobKind::Woofer,
        MobKind::Hmmer,
        MobKind::Grumbler,
        MobKind::Galloper,
        MobKind::Wyrm,
        MobKind::Squawker,
        MobKind::Clanker,
        MobKind::Fishy,
        MobKind::Soggy,
        MobKind::Bee,
        MobKind::Sneaker,
        MobKind::Ribbit,
        MobKind::Rollo,
        MobKind::Hush,
        MobKind::Sizzler,
        MobKind::Weeper,
        MobKind::Strutter,
        MobKind::Snout,
        MobKind::Pilferer,
        MobKind::Hackler,
        MobKind::Invoicer,
        MobKind::Fee,
        MobKind::Rampager,
        // Appended only: the index is the save and wire encoding.
        MobKind::Breeze,
        MobKind::Goat,
        MobKind::Axolotl,
        MobKind::Camel,
        MobKind::Creaking,
        MobKind::Sniffer,
        MobKind::CopperGolem,
        MobKind::Floaty,
        MobKind::Rotsteed,
    ];

    pub fn index(self) -> u8 {
        match self {
            MobKind::Modded(i) => BASE_MOBS.saturating_add(i.min(u8::MAX as u16) as u8),
            k => MobKind::ALL.iter().position(|x| *x == k).unwrap_or(0) as u8,
        }
    }
    /// The kind for a wire/save index, or `None` if it names no loaded mob
    /// (an unknown or removed modded kind degrades gracefully to `None`).
    pub fn from_index(i: u8) -> Option<MobKind> {
        if i < BASE_MOBS {
            MobKind::ALL.get(i as usize).copied()
        } else {
            let m = (i - BASE_MOBS) as usize;
            (m < crate::block::reg().mobs.len()).then_some(MobKind::Modded(m as u16))
        }
    }
    /// The mod-defined mob definition for a `Modded` kind, if it is still loaded.
    pub fn mod_def(self) -> Option<&'static crate::block::ModMob> {
        match self {
            MobKind::Modded(i) => crate::block::reg().mobs.get(i as usize),
            _ => None,
        }
    }
    /// Names accepted by mods and scripts (the parody name or the one it parodies).
    pub fn from_name(s: &str) -> Option<MobKind> {
        let lower = s.to_ascii_lowercase();
        if let Some(k) = MobKind::from_base_name(&lower) {
            return Some(k);
        }
        // Fall back to a mod-defined mob, by its "modid:name" key or bare name.
        let mobs = &crate::block::reg().mobs;
        mobs.iter()
            .position(|m| m.key == lower || m.key.rsplit(':').next() == Some(lower.as_str()))
            .map(|i| MobKind::Modded(i as u16))
    }
    /// Resolve only the built-in kinds by name (no mod lookup). Used while a
    /// mod registry is still being built, before it's installed.
    pub(crate) fn from_base_name_public(s: &str) -> Option<MobKind> {
        Self::from_base_name(s)
    }
    fn from_base_name(s: &str) -> Option<MobKind> {
        match s {
            "oinker" | "pig" => Some(MobKind::Oinker),
            "hisser" | "creeper" => Some(MobKind::Hisser),
            "groaner" | "zombie" => Some(MobKind::Groaner),
            "fluffer" | "sheep" => Some(MobKind::Fluffer),
            "starer" | "enderman" => Some(MobKind::Starer),
            "cluckster" | "chicken" => Some(MobKind::Cluckster),
            "mooer" | "cow" => Some(MobKind::Mooer),
            "rattler" | "skeleton" => Some(MobKind::Rattler),
            "webber" | "spider" => Some(MobKind::Webber),
            "bloop" | "slime" => Some(MobKind::Bloop),
            "woofer" | "wolf" | "dog" => Some(MobKind::Woofer),
            "hmmer" | "villager" => Some(MobKind::Hmmer),
            "grumbler" | "zombified_piglin" | "zombie_pigman" => Some(MobKind::Grumbler),
            "galloper" | "horse" => Some(MobKind::Galloper),
            "wyrm" | "hollow wyrm" | "hollow_wyrm" | "ender_dragon" | "dragon" => Some(MobKind::Wyrm),
            "squawker" | "parrot" => Some(MobKind::Squawker),
            "clanker" | "iron_golem" | "golem" => Some(MobKind::Clanker),
            "fishy" | "fish" | "cod" => Some(MobKind::Fishy),
            "soggy" | "soggy groaner" | "soggy_groaner" | "drowned" => Some(MobKind::Soggy),
            "bee" => Some(MobKind::Bee),
            "sneaker" | "fox" => Some(MobKind::Sneaker),
            "ribbit" | "frog" => Some(MobKind::Ribbit),
            "rollo" | "armadillo" => Some(MobKind::Rollo),
            "hush" | "the hush" | "the_hush" | "warden" => Some(MobKind::Hush),
            "sizzler" | "blaze" => Some(MobKind::Sizzler),
            "weeper" | "ghast" => Some(MobKind::Weeper),
            "strutter" | "strider" => Some(MobKind::Strutter),
            "snout" | "piglin" => Some(MobKind::Snout),
            "pilferer" | "pillager" => Some(MobKind::Pilferer),
            "hackler" | "vindicator" => Some(MobKind::Hackler),
            "invoicer" | "evoker" => Some(MobKind::Invoicer),
            "fee" | "vex" => Some(MobKind::Fee),
            "rampager" | "ravager" => Some(MobKind::Rampager),
            "breeze" => Some(MobKind::Breeze),
            "goat" => Some(MobKind::Goat),
            "axolotl" => Some(MobKind::Axolotl),
            "camel" => Some(MobKind::Camel),
            "creaking" => Some(MobKind::Creaking),
            "sniffer" => Some(MobKind::Sniffer),
            "copper golem" | "copper_golem" | "coppergolem" => Some(MobKind::CopperGolem),
            "floaty" | "happy_ghast" => Some(MobKind::Floaty),
            "rotsteed" | "zombie_horse" => Some(MobKind::Rotsteed),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        if let MobKind::Modded(_) = self {
            return self.mod_def().map(|d| d.name.as_str()).unwrap_or("Creature");
        }
        match self {
            MobKind::Oinker => "Oinker",
            MobKind::Hisser => "Hisser",
            MobKind::Groaner => "Groaner",
            MobKind::Fluffer => "Fluffer",
            MobKind::Starer => "Starer",
            MobKind::Cluckster => "Cluckster",
            MobKind::Mooer => "Mooer",
            MobKind::Rattler => "Rattler",
            MobKind::Webber => "Webber",
            MobKind::Bloop => "Bloop",
            MobKind::Woofer => "Woofer",
            MobKind::Hmmer => "Hmmer",
            MobKind::Grumbler => "Grumbler",
            MobKind::Galloper => "Galloper",
            MobKind::Wyrm => "Hollow Wyrm",
            MobKind::Squawker => "Squawker",
            MobKind::Clanker => "Clanker",
            MobKind::Fishy => "Fishy",
            MobKind::Soggy => "Soggy Groaner",
            MobKind::Bee => "Bee",
            MobKind::Sneaker => "Sneaker",
            MobKind::Ribbit => "Ribbit",
            MobKind::Rollo => "Rollo",
            MobKind::Hush => "The Hush",
            MobKind::Sizzler => "Sizzler",
            MobKind::Weeper => "Weeper",
            MobKind::Strutter => "Strutter",
            MobKind::Snout => "Snout",
            MobKind::Pilferer => "Pilferer",
            MobKind::Hackler => "Hackler",
            MobKind::Invoicer => "Invoicer",
            MobKind::Fee => "Fee",
            MobKind::Rampager => "Rampager",
            MobKind::Breeze => "Breeze",
            MobKind::Goat => "Goat",
            MobKind::Axolotl => "Axolotl",
            MobKind::Camel => "Camel",
            MobKind::Creaking => "Creaking",
            MobKind::Sniffer => "Sniffer",
            MobKind::CopperGolem => "Copper Golem",
            MobKind::Floaty => "Floaty",
            MobKind::Rotsteed => "Rotsteed",
            MobKind::Modded(_) => "Creature",
        }
    }
    /// Half-width and height at size 1.
    fn dims(self) -> (f32, f32) {
        if let MobKind::Modded(_) = self {
            return self.mod_def().map(|d| (d.half_width, d.height)).unwrap_or((0.4, 0.9));
        }
        match self {
            MobKind::Oinker => (0.45, 0.9),
            MobKind::Hisser => (0.3, 1.65),
            MobKind::Groaner => (0.3, 1.95),
            MobKind::Fluffer => (0.45, 1.25),
            MobKind::Starer => (0.3, 2.9),
            MobKind::Cluckster => (0.2, 0.7),
            MobKind::Mooer => (0.45, 1.4),
            MobKind::Rattler => (0.3, 1.95),
            MobKind::Webber => (0.7, 0.9),
            MobKind::Bloop => (0.26, 0.52),
            MobKind::Woofer => (0.3, 0.85),
            MobKind::Hmmer => (0.3, 1.95),
            MobKind::Grumbler => (0.3, 1.95),
            MobKind::Galloper => (0.6, 1.6),
            MobKind::Wyrm => (2.5, 2.0),
            MobKind::Squawker => (0.2, 0.8),
            MobKind::Clanker => (0.7, 2.7),
            MobKind::Fishy => (0.25, 0.45),
            MobKind::Soggy => (0.3, 1.95),
            MobKind::Bee => (0.2, 0.35),
            MobKind::Sneaker => (0.3, 0.7),
            MobKind::Ribbit => (0.25, 0.5),
            MobKind::Rollo => (0.32, 0.55),
            MobKind::Hush => (0.45, 2.9),
            MobKind::Sizzler => (0.3, 1.8),
            MobKind::Weeper => (2.0, 4.0),
            MobKind::Strutter => (0.45, 1.7),
            MobKind::Snout => (0.3, 1.95),
            MobKind::Pilferer | MobKind::Hackler | MobKind::Invoicer => (0.3, 1.95),
            MobKind::Fee => (0.2, 0.8),
            MobKind::Rampager => (0.95, 2.2),
            MobKind::Breeze => (0.3, 1.6),
            MobKind::Goat => (0.35, 1.2),
            MobKind::Axolotl => (0.3, 0.42),
            MobKind::Camel => (0.7, 2.35),
            MobKind::Creaking => (0.4, 2.7),
            MobKind::Sniffer => (0.9, 1.75),
            MobKind::CopperGolem => (0.3, 1.2),
            MobKind::Floaty => (1.9, 3.8),
            MobKind::Rotsteed => (0.6, 1.6),
            MobKind::Modded(_) => (0.4, 0.9),
        }
    }
    pub fn max_health(self) -> f32 {
        if let MobKind::Modded(_) = self {
            return self.mod_def().map(|d| d.max_health).unwrap_or(10.0);
        }
        match self {
            MobKind::Oinker => 10.0,
            MobKind::Hisser => 20.0,
            MobKind::Groaner => 20.0,
            MobKind::Fluffer => 8.0,
            MobKind::Starer => 40.0,
            MobKind::Cluckster => 4.0,
            MobKind::Mooer => 10.0,
            MobKind::Rattler => 20.0,
            MobKind::Webber => 16.0,
            MobKind::Bloop => 1.0, // times size squared
            MobKind::Woofer => 8.0,
            MobKind::Hmmer => 20.0,
            MobKind::Grumbler => 20.0,
            MobKind::Galloper => 22.0,
            MobKind::Wyrm => 200.0,
            MobKind::Squawker => 6.0,
            MobKind::Clanker => 100.0,
            MobKind::Fishy => 3.0,
            MobKind::Soggy => 20.0,
            MobKind::Bee => 4.0,
            MobKind::Sneaker => 10.0,
            MobKind::Ribbit => 10.0,
            MobKind::Rollo => 12.0,
            MobKind::Hush => 250.0,
            MobKind::Sizzler => 20.0,
            MobKind::Weeper => 10.0,
            MobKind::Strutter => 20.0,
            MobKind::Snout => 16.0,
            MobKind::Pilferer => 24.0,
            MobKind::Hackler => 24.0,
            MobKind::Invoicer => 24.0,
            MobKind::Fee => 14.0,
            MobKind::Rampager => 100.0,
            MobKind::Breeze => 30.0,
            MobKind::Goat => 10.0,
            MobKind::Axolotl => 14.0,
            MobKind::Camel => 32.0,
            MobKind::Creaking => 1.0,
            MobKind::Sniffer => 28.0,
            MobKind::CopperGolem => 12.0,
            MobKind::Floaty => 20.0,
            MobKind::Rotsteed => 18.0,
            MobKind::Modded(_) => 10.0,
        }
    }
    /// Experience for defeating one (`size`: a Bloop's size).
    pub fn xp_value(self, size: f32, rng: &mut Rng) -> u32 {
        if let Some(xp) = self.mod_def().and_then(|d| d.xp) {
            return xp;
        }
        match self {
            MobKind::Bloop => size as u32,
            k if !k.hostile() => rng.int(1, 3) as u32,
            _ => 5,
        }
    }
    /// Spawns at night / in caves and counts toward the hostile cap.
    /// (Starers and daytime Webbers are only hostile once provoked, but they keep monster hours.)
    pub fn hostile(self) -> bool {
        if let MobKind::Modded(_) = self {
            // `hostile` decides whether a modded mob spawns at night and counts
            // toward the monster cap. Whether it also attacks additionally
            // depends on attack_damage > 0 (see Mob::update's Modded arm).
            return self.mod_def().is_some_and(|d| d.hostile);
        }
        !self.passive() && !matches!(self, MobKind::Woofer | MobKind::Hmmer | MobKind::Grumbler | MobKind::Clanker | MobKind::Fishy | MobKind::Bee | MobKind::Hush | MobKind::Snout | MobKind::Fee | MobKind::CopperGolem | MobKind::Floaty | MobKind::Rotsteed)
    }
    /// Farm animals: wander, flee when hit, spawn in daylight on grass.
    pub fn passive(self) -> bool {
        matches!(self, MobKind::Oinker | MobKind::Fluffer | MobKind::Cluckster | MobKind::Mooer | MobKind::Galloper | MobKind::Squawker | MobKind::Sneaker | MobKind::Ribbit | MobKind::Rollo | MobKind::Strutter | MobKind::Goat | MobKind::Axolotl | MobKind::Camel | MobKind::Sniffer)
    }
    /// Flies (no gravity; steers up and down itself).
    pub fn flies(self) -> bool {
        matches!(self, MobKind::Bee | MobKind::Sizzler | MobKind::Weeper | MobKind::Fee | MobKind::Floaty) || self.mod_def().is_some_and(|d| d.flying)
    }
    /// Lava and fire don't bother it.
    pub fn fireproof(self) -> bool {
        matches!(self, MobKind::Grumbler | MobKind::Sizzler | MobKind::Weeper | MobKind::Strutter | MobKind::Wyrm)
    }
    /// Part of a raid (see raids.rs).
    pub fn raider(self) -> bool {
        matches!(self, MobKind::Pilferer | MobKind::Hackler | MobKind::Invoicer | MobKind::Rampager)
    }
    /// What it eats to fall in love (see animals.rs); Woofers only once tamed.
    pub fn breed_food(self) -> &'static [Id] {
        match self {
            MobKind::Oinker => &[CARROT, POTATO],
            MobKind::Fluffer | MobKind::Mooer => &[WHEAT],
            MobKind::Cluckster | MobKind::Squawker => &[WHEAT_SEEDS],
            MobKind::Sneaker => &[CLUCKETS, COOKED_CLUCKETS],
            MobKind::Ribbit => &[GOO],
            MobKind::Rollo => &[DEAD_BUSH],
            MobKind::Strutter => &[EMBER_SHROOM],
            MobKind::Galloper => &[APPLE],
            MobKind::Goat => &[WHEAT],
            MobKind::Camel => &[CACTUS],
            MobKind::Sniffer => &[TORCHFLOWER_SEEDS],
            MobKind::Axolotl => &[TROPICAL],
            MobKind::Woofer => &[PORKCHOP, COOKED_CHOP, MUTTON, COOKED_MUTTON, MOO_STEAK, STEAK, CLUCKETS, COOKED_CLUCKETS, GOO],
            MobKind::Modded(_) => self.mod_def().and_then(|d| d.breed_item.as_ref()).map(std::slice::from_ref).unwrap_or(&[]),
            _ => &[],
        }
    }
    /// Undead: burn in sunlight.
    fn burns(self) -> bool {
        matches!(self, MobKind::Groaner | MobKind::Rattler)
    }
    /// The lowercase name scripts see (e.g. in `on_mob_death`). For a modded
    /// mob this is the bare section name from its key, so `spawn_mob` and
    /// `from_name` round-trip; for base kinds it's the lowercase display name.
    pub fn script_name(self) -> String {
        match self.mod_def() {
            Some(d) => d.key.rsplit(':').next().unwrap_or(&d.key).to_string(),
            None => self.name().to_ascii_lowercase(),
        }
    }
}

pub struct Mob {
    /// Stable id for multiplayer sync.
    pub id: u32,
    /// Latest position from the host (clients interpolate toward it).
    pub net_pos: Vec3,
    /// Player id that last hit this mob (0 = host / single player), for loot.
    pub last_attacker: u32,
    pub kind: MobKind,
    pub body: Body,
    pub yaw: f32,
    pub health: f32,
    pub hurt: f32,
    pub fuse: f32,
    pub attack_cd: f32,
    pub flee: f32,
    wander_t: f32,
    wander_dir: Option<f32>,
    pub anim: f32,
    /// Wingbeats while a bird drops through the air.
    pub flap: f32,
    knock: Vec3,
    pub burning: bool,
    /// Seconds left on fire (from lava; water puts it out).
    pub on_fire: f32,
    /// Starer: provoked (looked at or hit). Synced as `fuse > 0` for clients.
    pub angry: bool,
    /// Starer: seconds until it may teleport again.
    pub warp_cd: f32,
    /// Bloops come in sizes 1, 2 and 4 (it scales the body, health and damage).
    pub size: f32,
    /// Bloop: seconds until the next hop.
    hop_cd: f32,
    // ---- animals (see animals.rs)
    /// Seconds left "in love" (fed its favourite food: looking for a partner).
    pub love: f32,
    /// Seconds before it can breed again.
    pub breed_cd: f32,
    /// Seconds until a baby grows up (0: grown).
    pub baby: f32,
    /// Fluffers: sheared (the wool grows back after some grass).
    pub sheared: bool,
    /// Tamed Woofers: their player (by `players::record_key`), and whether they're sitting.
    pub owner: Option<String>,
    pub sitting: bool,
    /// Tamed Woofers: the mob they're after.
    pub prey: Option<u32>,
    /// Where the game wants it to go (a partner, its owner, prey, someone holding food).
    pub goal: Option<Vec3>,
    /// Kept when players are far away (tamed, bred or fed; saved with the world).
    pub persistent: bool,
    // ---- Hmmers (see villagers.rs)
    /// Decides its job and trades.
    pub seed: u32,
    /// Its hut.
    pub home: Option<Vec3>,
    /// Times each trade was made since the last restock, and seconds to the next.
    pub trades_used: [u8; 8],
    pub restock: f32,
    // ---- Gallopers (see horses.rs)
    pub saddled: bool,
    /// How used to people it is (tamed at `horses::TAME_AT`).
    pub temper: u8,
    /// Who's riding it: 0 nobody, else player id + 1.
    pub rider: u32,
    /// A Camel's second seat, behind the driver: 0 nobody, else player id + 1.
    pub passenger: u32,
    /// A Floaty's other two seats (see floaty.rs), the same way.
    pub crew: [u32; 2],
    /// Its way round obstacles to where it's going (see pathing.rs).
    pub path: Option<Box<crate::pathing::Path>>,
    /// Which colouring it has (Gallopers, Woofers, Mooers, Axolotls; see `variant_tint`).
    pub variant: u8,
}

/// How many colourings a kind comes in (1: just the one).
pub fn variants(kind: MobKind) -> u8 {
    match kind {
        MobKind::Galloper => 5,
        MobKind::Woofer => 4,
        MobKind::Mooer => 3,
        MobKind::Axolotl => 5,
        _ => 1,
    }
}

/// The rare blue Axolotl.
pub const BLUE_AXOLOTL: u8 = 4;

/// A colouring, as a multiplier on the kind's own skin.
pub fn variant_tint(kind: MobKind, v: u8) -> [f32; 3] {
    match (kind, v) {
        // Chestnut (as drawn), white, black, grey, cream.
        (MobKind::Galloper, 1) => [1.7, 1.7, 1.75],
        (MobKind::Galloper, 2) => [0.35, 0.33, 0.33],
        (MobKind::Galloper, 3) => [1.05, 1.15, 1.25],
        (MobKind::Galloper, 4) => [1.45, 1.35, 1.1],
        // Grey (as drawn), black, rusty, snowy.
        (MobKind::Woofer, 1) => [0.4, 0.4, 0.42],
        (MobKind::Woofer, 2) => [1.25, 0.85, 0.6],
        (MobKind::Woofer, 3) => [1.3, 1.3, 1.35],
        // Brown (as drawn), black, red.
        (MobKind::Mooer, 1) => [0.45, 0.42, 0.42],
        (MobKind::Mooer, 2) => [1.35, 0.7, 0.6],
        // Pink (as drawn), gold, cyan, brown, and blue.
        (MobKind::Axolotl, 1) => [1.15, 1.05, 0.45],
        (MobKind::Axolotl, 2) => [0.65, 1.15, 1.25],
        (MobKind::Axolotl, 3) => [0.75, 0.55, 0.45],
        (MobKind::Axolotl, BLUE_AXOLOTL) => [0.4, 0.6, 1.6],
        _ => [1.0; 3],
    }
}

/// A fresh colouring (the blue Axolotl is very rare).
pub fn roll_variant(kind: MobKind, rng: &mut Rng) -> u8 {
    let n = variants(kind);
    if n <= 1 {
        return 0;
    }
    if kind == MobKind::Axolotl {
        return if rng.chance(0.02) { BLUE_AXOLOTL } else { rng.int(0, 3) as u8 };
    }
    rng.int(0, n as i32 - 1) as u8
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModProjectileSpec {
    pub damage: f32,
    pub appearance: ProjectileAppearance,
    pub effect: Option<ProjectileEffect>,
    pub count: u8,
    pub spread: f32,
    /// Turn rate toward the nearest player, degrees per second (0 = none).
    pub homing: f32,
    /// Blast radius on landing (0 = a plain hit).
    pub blast: f32,
}

pub enum MobEvent {
    HurtPlayer(f32, &'static str),
    Explode(Vec3, f32, &'static str),
    Smoke(Vec3),
    /// A Starer blinked from one place to another.
    Warp(Vec3, Vec3),
    /// A Rattler loosed a Pointy Stick: (from, velocity).
    Shoot(Vec3, Vec3),
    /// A modded mob fired one projectile volley: (from, center velocity, specification).
    ShootMod(Vec3, Vec3, ModProjectileSpec),
    /// A fireball thrown: (from, velocity, big (a Weeper's, which explodes)).
    Fireball(Vec3, Vec3, bool),
    /// An Invoicer's spell: Late Fees up out of the ground from here toward there, or Fees summoned.
    Fangs(Vec3, Vec3),
    Summon(Vec3),
    /// A modded boss calls for help: (where, mob kind index, how many).
    SummonMod(Vec3, u8, u8),
    /// The Hush's shush: a blast of sound from `from` at the player, through walls.
    Shush(Vec3),
    /// A Breeze's wind charge: (from, velocity).
    WindCharge(Vec3, Vec3),
    /// Something knocked loose (a Goat's horn, on a wall): (where, item).
    DropItem(Vec3, Id),
    /// A Goat lowers its head to charge.
    Bleat(Vec3),
}

/// A spot a Starer can teleport to near `around`: standing room on solid ground.
pub fn warp_spot(world: &World, around: Vec3, radius: f32, rng: &mut Rng) -> Option<Vec3> {
    for _ in 0..16 {
        let x = (around.x + rng.range(-radius, radius)).floor() as i32;
        let z = (around.z + rng.range(-radius, radius)).floor() as i32;
        if !world.is_loaded(x, z) {
            continue;
        }
        let base = around.y.floor() as i32;
        for dy in [0, 1, -1, 2, -2, 3, -3, 4, -4] {
            let y = base + dy;
            if is_solid(world.get(x, y - 1, z)) && (0..3).all(|h| !is_solid(world.get(x, y + h, z)) && !is_liquid(world.get(x, y + h, z))) {
                return Some(Vec3::new(x as f32 + 0.5, y as f32, z as f32 + 0.5));
            }
        }
    }
    None
}

impl Mob {
    /// Birds beat their wings while `falling`, and fold them once down.
    pub fn flutter(&mut self, falling: bool, dt: f32) {
        let bird = matches!(self.kind, MobKind::Cluckster | MobKind::Squawker);
        // A modded flier's wings beat all the time it's in the air.
        let flier = matches!(self.kind, MobKind::Modded(_)) && self.kind.flies() && !self.sitting && !self.body.on_ground;
        step_anim(&mut self.flap, if (bird && falling) || flier { 5.0 } else { 0.0 }, dt, 5.0);
    }

    pub fn new(kind: MobKind, pos: Vec3, rng: &mut Rng) -> Self {
        let (half, h) = kind.dims();
        Mob {
            id: 0,
            net_pos: pos,
            last_attacker: 0,
            kind,
            body: Body::new(pos, half, h),
            yaw: rng.range(0.0, std::f32::consts::TAU),
            health: kind.max_health(),
            hurt: 0.0,
            fuse: 0.0,
            attack_cd: 0.0,
            flee: 0.0,
            wander_t: rng.range(0.0, 3.0),
            wander_dir: None,
            anim: 0.0,
            flap: 0.0,
            knock: Vec3::ZERO,
            burning: false,
            on_fire: 0.0,
            angry: false,
            // A Goat waits a while before its first charge, a Sniffer before its first dig.
            warp_cd: if matches!(kind, MobKind::Goat | MobKind::Sniffer) { rng.range(10.0, 30.0) } else { 0.0 },
            size: 1.0,
            hop_cd: if matches!(kind, MobKind::Goat | MobKind::Sniffer) { 0.0 } else { rng.range(0.5, 2.0) },
            love: 0.0,
            breed_cd: 0.0,
            baby: 0.0,
            sheared: false,
            owner: None,
            sitting: false,
            prey: None,
            goal: None,
            // A boss stays until it's beaten.
            persistent: kind.mod_def().is_some_and(|d| d.boss),
            seed: 0,
            home: None,
            trades_used: [0; 8],
            restock: crate::villagers::RESTOCK_SECS,
            saddled: false,
            temper: 0,
            rider: 0,
            passenger: 0,
            crew: [0; 2],
            path: None,
            variant: if variants(kind) > 1 { roll_variant(kind, rng) } else { 0 },
        }
    }

    /// Make it a baby for `secs` (half size until it grows up).
    pub fn set_baby(&mut self, secs: f32) {
        let (half, h) = self.kind.dims();
        self.baby = secs;
        let k = if secs > 0.0 { 0.5 } else { 1.0 };
        self.body.half = half * k * self.size;
        self.body.height = h * k * self.size;
    }

    /// Can it fall in love right now?
    /// A monster that's actually a threat: hostile, and not somebody's pet
    /// (a tamed modded monster is left alone by beds, golems and Peaceful).
    /// Has a health bar across the top of the screen (the Wyrm, or a modded boss).
    pub fn is_boss(&self) -> bool {
        self.kind == MobKind::Wyrm || self.kind.mod_def().is_some_and(|d| d.boss)
    }

    pub fn menacing(&self) -> bool {
        self.kind.hostile() && self.owner.is_none()
    }

    pub fn ready_to_breed(&self) -> bool {
        // Woofers (and hostile modded mobs) only once tamed.
        let wild_ok = match self.kind {
            MobKind::Woofer => false,
            MobKind::Modded(_) => !self.kind.mod_def().is_some_and(|d| d.hostile),
            _ => true,
        };
        self.baby <= 0.0 && self.breed_cd <= 0.0 && self.love <= 0.0 && (wild_ok || self.owner.is_some())
    }

    /// Resize (Bloops): scales the body, and health with the square of the size.
    pub fn with_size(mut self, size: u8) -> Self {
        let size = size.clamp(1, 4) as f32;
        let (half, h) = self.kind.dims();
        self.size = size;
        self.body.half = half * size;
        self.body.height = h * size;
        self.health = self.kind.max_health() * size * size;
        self
    }

    pub fn eye(&self) -> Vec3 {
        self.body.pos + Vec3::Y * self.body.height * 0.85
    }

    pub fn damage(&mut self, amount: f32, from: Vec3) {
        if self.hurt > 0.25 {
            return;
        }
        // A Creaking shrugs everything off (only breaking its heart works).
        if self.kind == MobKind::Creaking {
            self.hurt = 0.5;
            return;
        }
        // A curled-up Rollo is mostly shell.
        let amount = if self.kind == MobKind::Rollo && self.fuse > 0.0 { amount * 0.25 } else { amount };
        // Woofer armour soaks up hits until it wears through (see critters.rs).
        let amount = if self.kind == MobKind::Woofer && self.saddled { crate::critters::armour_soak(self, amount) } else { amount };
        self.health -= amount;
        self.hurt = 0.5;
        let mut dir = self.body.pos - from;
        dir.y = 0.0;
        let dir = dir.normalize_or_zero();
        self.knock = dir * 7.0;
        self.body.vel.y = 6.0;
        match self.kind {
            MobKind::Webber => self.angry = true,
            MobKind::Woofer if self.owner.is_none() => self.angry = true,
            MobKind::Rollo => {
                self.fuse = 6.0;
                self.knock *= 0.3;
            }
            k if k.passive() => self.flee = 4.0,
            MobKind::Modded(_) => {
                self.flee = 4.0;
                // Big things (bosses especially) barely budge.
                let budge = 1.0 - self.kind.mod_def().map(|d| d.knockback_resist).unwrap_or(0.0);
                self.knock *= budge;
                self.body.vel.y *= budge;
            }
            MobKind::Hmmer | MobKind::Fishy => self.flee = 4.0,
            MobKind::Grumbler | MobKind::Bee => self.angry = true,
            MobKind::Hush => {
                // Hard to shift; and now it knows exactly where you are.
                self.knock *= 0.1;
                self.body.vel.y = 0.5;
                self.goal = Some(from);
                self.fuse = 0.0;
            }
            MobKind::Clanker => {
                // Hard to shift, and it remembers who did that.
                self.angry = true;
                self.knock *= 0.15;
                self.body.vel.y = 1.0;
            }
            MobKind::Starer => {
                self.angry = true;
                // Takes the hit, then blinks away to think about it (flee = "wants to warp").
                self.flee = 1.0;
            }
            _ => {}
        }
    }

    pub fn update(&mut self, dt: f32, world: &World, player: Vec3, player_visible: bool, daylight: f32, rng: &mut Rng) -> Vec<MobEvent> {
        let mut ev = Vec::new();
        self.hurt = (self.hurt - dt).max(0.0);
        self.attack_cd = (self.attack_cd - dt).max(0.0);
        self.flee = (self.flee - dt).max(0.0);
        self.love = (self.love - dt).max(0.0);
        self.breed_cd = (self.breed_cd - dt).max(0.0);
        if self.baby > 0.0 {
            self.baby -= dt;
            if self.baby <= 0.0 {
                self.set_baby(0.0);
            }
        }
        if self.kind == MobKind::Wyrm {
            crate::hollow::wyrm_update(self, dt, player, player_visible, &mut ev);
            return ev;
        }
        let to_player = player - self.body.pos;
        let dist = to_player.length();
        let flat = Vec3::new(to_player.x, 0.0, to_player.z);

        let mut want: Option<(f32, f32)> = None; // (yaw, speed)
        // Swimmers choose how fast to rise or sink instead of bobbing up (and fliers how fast to climb).
        let mut swim_vy: Option<f32> = None;
        let mut fly_vy: Option<f32> = None;
        // Wander when there's nothing better to do (Bloops only ever hop).
        let mut may_wander = true;
        let face = flat.x.atan2(-flat.z);
        match self.kind {
            // (The Wyrm flies on its own, see hollow.rs.)
            MobKind::Oinker | MobKind::Fluffer | MobKind::Cluckster | MobKind::Mooer | MobKind::Hmmer | MobKind::Galloper | MobKind::Wyrm | MobKind::Squawker | MobKind::Strutter | MobKind::Camel | MobKind::Rotsteed => {
                if self.flee > 0.0 {
                    want = Some(((-flat.x).atan2(flat.z), 3.5));
                } else if let Some(g) = self.goal {
                    // A partner, or someone holding something tasty.
                    let d = g - self.body.pos;
                    let fd = Vec3::new(d.x, 0.0, d.z).length();
                    if fd > 1.1 {
                        want = Some((d.x.atan2(-d.z), 1.8));
                    } else {
                        self.yaw += angle_diff(d.x.atan2(-d.z), self.yaw).clamp(-6.0 * dt, 6.0 * dt);
                        may_wander = false;
                    }
                }
            }
            MobKind::Clanker => {
                if !player_visible || dist > 24.0 {
                    self.angry = false;
                }
                if self.angry && player_visible {
                    want = Some((face, 2.6));
                    if flat.length() < 1.9 && to_player.y.abs() < 2.2 && self.attack_cd <= 0.0 {
                        ev.push(MobEvent::HurtPlayer(7.0, "was flattened by a Clanker"));
                        self.attack_cd = 1.4;
                    }
                } else if let Some(g) = self.goal {
                    // After a monster (see golems.rs), else back to its square.
                    let d = g - self.body.pos;
                    let fd = Vec3::new(d.x, 0.0, d.z).length();
                    if fd > 1.6 {
                        want = Some((d.x.atan2(-d.z), if self.prey.is_some() { 2.6 } else { 1.4 }));
                    } else {
                        may_wander = false;
                    }
                }
            }
            MobKind::Grumbler => {
                if !player_visible || dist > 40.0 {
                    self.angry = false;
                }
                if self.angry && player_visible {
                    want = Some((face, 3.2));
                    if flat.length() < 1.3 && to_player.y.abs() < 1.6 && self.attack_cd <= 0.0 {
                        ev.push(MobEvent::HurtPlayer(5.0, "annoyed a Grumbler (and all its friends)"));
                        self.attack_cd = 1.1;
                    }
                }
            }
            MobKind::Woofer => {
                if self.sitting {
                    may_wander = false;
                } else if self.owner.is_some() {
                    // Tamed: after its prey, else at its owner's heel.
                    if let Some(g) = self.goal {
                        let d = g - self.body.pos;
                        let fd = Vec3::new(d.x, 0.0, d.z).length();
                        let (near, speed) = if self.prey.is_some() { (0.9, 4.6) } else { (2.5, 4.3) };
                        if fd > near {
                            want = Some((d.x.atan2(-d.z), if fd > 6.0 { speed } else { speed * 0.7 }));
                        } else {
                            may_wander = false;
                        }
                    }
                } else if self.angry && player_visible && dist < 32.0 {
                    // Wild and provoked: bite back.
                    want = Some((face, 4.0));
                    if flat.length() < 1.3 && to_player.y.abs() < 1.5 && self.attack_cd <= 0.0 {
                        ev.push(MobEvent::HurtPlayer(3.0, "was bitten by a Woofer. Should have brought a bone"));
                        self.attack_cd = 1.0;
                    }
                } else {
                    self.angry = false;
                    if let Some(g) = self.goal {
                        let d = g - self.body.pos;
                        if Vec3::new(d.x, 0.0, d.z).length() > 1.1 {
                            want = Some((d.x.atan2(-d.z), 1.8));
                        }
                    }
                }
            }
            MobKind::Hisser => {
                if player_visible && dist < 16.0 {
                    if dist > 1.6 {
                        want = Some((flat.x.atan2(-flat.z), 2.1));
                    } else {
                        self.yaw = flat.x.atan2(-flat.z);
                    }
                    if dist < 3.0 {
                        self.fuse += dt;
                    } else {
                        self.fuse = (self.fuse - dt).max(0.0);
                    }
                } else {
                    self.fuse = (self.fuse - dt).max(0.0);
                }
                if self.fuse >= 1.5 {
                    ev.push(MobEvent::Explode(self.body.pos + Vec3::Y * 0.8, 3.0, "was blown up by a Hisser"));
                    self.health = -100.0;
                }
            }
            MobKind::Fishy => {
                if self.body.in_water {
                    // Drift about, never out of the water.
                    let ahead = self.body.pos + Vec3::new(self.yaw.sin(), 0.2, -self.yaw.cos()) * 0.8;
                    if !is_water(world.get(ahead.x.floor() as i32, ahead.y.floor() as i32, ahead.z.floor() as i32)) {
                        self.wander_dir = Some(self.yaw + std::f32::consts::PI + rng.range(-0.8, 0.8));
                        self.wander_t = rng.range(1.0, 3.0);
                    }
                    if self.flee > 0.0 {
                        want = Some(((-flat.x).atan2(flat.z), 4.5));
                    }
                    let above = world.get(self.body.pos.x.floor() as i32, (self.body.pos.y + 0.8).floor() as i32, self.body.pos.z.floor() as i32);
                    let drift = (self.wander_t * 1.7 + self.id as f32).sin() * 0.8;
                    swim_vy = Some(if is_water(above) { drift } else { drift.min(0.0) - 0.3 });
                } else {
                    // Out of the water: flop, and slowly dry out.
                    may_wander = false;
                    self.health -= dt * 0.5;
                    if self.body.on_ground && rng.chance(dt * 2.0) {
                        self.body.vel.y = 4.5;
                        self.yaw = rng.range(0.0, std::f32::consts::TAU);
                        self.knock = Vec3::new(self.yaw.sin(), 0.0, -self.yaw.cos()) * 2.0;
                    }
                }
            }
            MobKind::Soggy => {
                if player_visible && dist < 28.0 {
                    let armed = self.seed == 1;
                    // Armed ones keep a throwing distance and hurl their spear now and then.
                    if armed && (5.0..14.0).contains(&flat.length()) && self.attack_cd <= 0.0 {
                        let eye = self.eye();
                        let aim = player + Vec3::Y * 0.9 - eye;
                        if world.raycast(eye, aim.normalize_or_zero(), aim.length()).is_none() {
                            let vel = aim.normalize_or_zero() * Arrow::SPEED + Vec3::Y * aim.length() * 0.3;
                            ev.push(MobEvent::Shoot(eye + aim.normalize_or_zero() * 0.5, vel));
                            self.attack_cd = rng.range(2.5, 4.0);
                        }
                    }
                    want = Some((face, if self.body.in_water { 2.8 } else { 2.2 }));
                    if self.body.in_water {
                        swim_vy = Some((to_player.y * 2.0).clamp(-3.0, 3.0));
                    }
                    if flat.length() < 1.3 && to_player.y.abs() < 1.6 && self.attack_cd <= 0.0 {
                        ev.push(MobEvent::HurtPlayer(if armed { 6.0 } else { 3.0 }, "was dragged under by a Soggy Groaner"));
                        self.attack_cd = 1.0;
                    }
                } else if self.body.in_water {
                    swim_vy = Some(-0.4);
                }
            }
            MobKind::Bee => {
                // Out of the hive to sting someone, then back home (see bees.rs).
                may_wander = false;
                self.fuse += dt;
                let home = self.home.unwrap_or(self.body.pos) + Vec3::new(0.0, 0.6, 0.0);
                let chase = self.angry && player_visible && dist < 24.0 && self.fuse < 40.0;
                let goal = if chase { player + Vec3::Y * 1.2 } else { home };
                let d = goal - self.body.pos;
                let fd = Vec3::new(d.x, 0.0, d.z);
                if fd.length() > 0.2 {
                    want = Some((fd.x.atan2(-fd.z), if chase { 4.4 } else { 2.5 }));
                }
                fly_vy = Some((d.y * 2.5).clamp(-4.0, 4.0) + (self.fuse * 9.0 + self.id as f32).sin() * 0.5);
                if chase && (player + Vec3::Y * 0.9).distance(self.body.pos) < 1.1 && self.attack_cd <= 0.0 {
                    ev.push(MobEvent::HurtPlayer(2.0, "was stung by a Bee. The smoker was right there"));
                    // A bee only stings once.
                    self.health = -100.0;
                } else if (!chase && d.length() < 0.8) || self.fuse > 60.0 {
                    self.health = -100.0;
                }
            }
            MobKind::Sneaker => {
                // Naps through the day unless something's going on (see critters.rs for the hunting).
                let napping = daylight > 0.7 && self.flee <= 0.0 && self.goal.is_none() && self.hurt <= 0.0 && self.love <= 0.0;
                self.sitting = napping;
                if self.flee > 0.0 {
                    want = Some(((-flat.x).atan2(flat.z), 4.6));
                } else if napping {
                    may_wander = false;
                } else if let Some(g) = self.goal {
                    let d = g - self.body.pos;
                    let fd = Vec3::new(d.x, 0.0, d.z).length();
                    if fd > 1.0 {
                        want = Some((d.x.atan2(-d.z), if self.prey.is_some() { 4.2 } else { 2.8 }));
                    } else {
                        may_wander = false;
                    }
                } else if self.owner.is_none() && player_visible && dist < 5.0 {
                    // Wild ones keep their distance.
                    want = Some(((-flat.x).atan2(flat.z), 3.0));
                }
            }
            MobKind::Ribbit => {
                // Hop, hop. Swims well, too.
                may_wander = false;
                self.hop_cd = (self.hop_cd - dt).max(0.0);
                if self.body.in_water {
                    let drift = (self.wander_t * 1.3 + self.id as f32).sin() * 0.6;
                    swim_vy = Some(drift);
                    may_wander = true;
                } else if self.body.on_ground {
                    if self.hop_cd <= 0.0 {
                        self.yaw = match self.goal {
                            Some(g) => {
                                let d = g - self.body.pos;
                                d.x.atan2(-d.z)
                            }
                            None if self.flee > 0.0 => (-flat.x).atan2(flat.z),
                            None => self.yaw + rng.range(-1.2, 1.2),
                        };
                        self.body.vel.y = 5.5;
                        self.body.on_ground = false;
                        self.hop_cd = if self.goal.is_some() || self.flee > 0.0 { rng.range(0.4, 0.8) } else { rng.range(1.5, 4.0) };
                        self.anim += 1.0;
                    }
                } else {
                    want = Some((self.yaw, 2.4));
                }
            }
            MobKind::Rollo => {
                // Curled up (fuse > 0): a ball until it feels safe again.
                if self.fuse > 0.0 {
                    may_wander = false;
                    if !(player_visible && dist < 3.0) {
                        self.fuse = (self.fuse - dt).max(0.0);
                    }
                } else if player_visible && dist < 2.2 && self.owner.is_none() && rng.chance(dt * 2.0) {
                    self.fuse = 4.0;
                    ev.push(MobEvent::Smoke(self.body.pos + Vec3::Y * 0.2));
                } else if let Some(g) = self.goal {
                    let d = g - self.body.pos;
                    if Vec3::new(d.x, 0.0, d.z).length() > 1.1 {
                        want = Some((d.x.atan2(-d.z), 1.2));
                    }
                } else if self.flee > 0.0 {
                    want = Some(((-flat.x).atan2(flat.z), 2.4));
                }
            }
            MobKind::Hush => {
                // Blind: it goes where it last heard something (see deepdark.rs), or
                // straight for anyone close enough to smell.
                self.fuse += dt;
                self.warp_cd = (self.warp_cd - dt).max(0.0);
                let smelled = dist < 3.5;
                let target = if smelled { Some(player) } else { self.goal };
                if smelled {
                    self.fuse = 0.0;
                    self.goal = Some(player);
                }
                match target {
                    Some(g) => {
                        let d = g - self.body.pos;
                        let fd = Vec3::new(d.x, 0.0, d.z).length();
                        if fd > 1.2 {
                            want = Some((d.x.atan2(-d.z), if smelled { 3.2 } else { 2.0 }));
                        } else {
                            may_wander = false;
                            if !smelled {
                                // Nothing here after all.
                                self.goal = None;
                            }
                        }
                    }
                    None => may_wander = true,
                }
                if smelled && flat.length() < 1.8 && to_player.y.abs() < 2.5 && self.attack_cd <= 0.0 {
                    ev.push(MobEvent::HurtPlayer(15.0, "was flattened by The Hush. It heard that"));
                    self.attack_cd = 1.6;
                } else if self.goal.is_some_and(|g| g.distance(player) < 2.0) && (4.0..16.0).contains(&dist) && self.warp_cd <= 0.0 {
                    // Can't reach you? It shushes you instead, through anything.
                    ev.push(MobEvent::Shush(self.eye()));
                    self.warp_cd = 7.0;
                }
                if self.fuse > 60.0 {
                    // Bored: it digs back down where it came from.
                    ev.push(MobEvent::Smoke(self.body.pos + Vec3::Y));
                    self.health = -100.0;
                }
            }
            MobKind::Sizzler => {
                // Hovers a little above whoever it's after, throwing fireballs three at a time.
                let after = player_visible && dist < 32.0;
                may_wander = !after;
                let bob = (self.wander_t * 2.0 + self.id as f32).sin() * 0.4;
                let hover = if after { player.y + 2.5 + bob } else { self.body.pos.y + bob };
                fly_vy = Some(((hover - self.body.pos.y) * 1.5).clamp(-2.0, 2.0));
                if after {
                    let fd = flat.length();
                    if fd > 10.0 {
                        want = Some((face, 2.6));
                    } else if fd < 4.0 {
                        want = Some((face + std::f32::consts::PI, 2.0));
                    } else {
                        self.yaw += angle_diff(face, self.yaw).clamp(-8.0 * dt, 8.0 * dt);
                    }
                    let eye = self.eye();
                    let aim = player + Vec3::Y * 0.9 - eye;
                    let clear = world.raycast(eye, aim.normalize_or_zero(), aim.length()).is_none();
                    if self.attack_cd <= 0.0 && clear && dist < 24.0 {
                        let wobble = Vec3::new(rng.range(-0.08, 0.08), rng.range(-0.05, 0.05), rng.range(-0.08, 0.08));
                        ev.push(MobEvent::Fireball(eye + aim.normalize_or_zero() * 0.6, (aim.normalize_or_zero() + wobble).normalize_or_zero() * 14.0, false));
                        self.seed += 1;
                        if self.seed >= 3 {
                            self.seed = 0;
                            self.attack_cd = rng.range(3.5, 5.0);
                        } else {
                            self.attack_cd = 0.35;
                        }
                    }
                }
                self.wander_t += dt;
                if rng.chance(dt * 5.0) {
                    ev.push(MobEvent::Smoke(self.eye()));
                }
            }
            MobKind::Weeper => {
                // Drifts about high and slow; opens its eyes, and cries a fireball.
                may_wander = false;
                self.fuse += dt;
                let eye = self.eye();
                let aim = player + Vec3::Y * 0.9 - eye;
                let sees = player_visible && dist < 48.0 && world.raycast(eye, aim.normalize_or_zero(), aim.length()).is_none();
                self.angry = sees && self.attack_cd < 1.0;
                if self.goal.is_none() || self.fuse > 9.0 || self.body.hit_wall || self.goal.is_some_and(|g| g.distance(self.body.pos) < 2.0) {
                    self.fuse = 0.0;
                    self.goal = Some(self.body.pos + Vec3::new(rng.range(-16.0, 16.0), rng.range(-3.0, 3.0), rng.range(-16.0, 16.0)));
                }
                if let Some(g) = self.goal {
                    let d = g - self.body.pos;
                    want = Some((d.x.atan2(-d.z), 1.4));
                    fly_vy = Some((d.y * 0.5).clamp(-1.0, 1.0));
                }
                if sees {
                    want = want.map(|(_, s)| (face, s * 0.5));
                    if self.attack_cd <= 0.0 {
                        ev.push(MobEvent::Fireball(eye + aim.normalize_or_zero() * 2.4, aim.normalize_or_zero() * 10.0, true));
                        self.attack_cd = rng.range(3.0, 5.0);
                    }
                }
            }
            MobKind::Snout => {
                // Minds its own business (and its gold) unless provoked (see fortress.rs).
                if !player_visible || dist > 32.0 {
                    self.angry = false;
                }
                if self.seed == 1 {
                    // Admiring some gold: busy.
                    may_wander = false;
                } else if self.angry && player_visible {
                    want = Some((face, 3.4));
                    if flat.length() < 1.3 && to_player.y.abs() < 1.6 && self.attack_cd <= 0.0 {
                        ev.push(MobEvent::HurtPlayer(6.0, "was clobbered by a Snout. Should've brought gold"));
                        self.attack_cd = 1.0;
                    }
                } else if let Some(g) = self.goal {
                    let d = g - self.body.pos;
                    if Vec3::new(d.x, 0.0, d.z).length() > 0.8 {
                        want = Some((d.x.atan2(-d.z), 2.4));
                    } else {
                        may_wander = false;
                    }
                }
            }
            MobKind::Pilferer | MobKind::Hackler | MobKind::Invoicer | MobKind::Rampager => {
                // Raiders: after anyone they can see, else on to the village (`home`; see raids.rs).
                let fd = flat.length();
                let after = player_visible && dist < if self.kind == MobKind::Pilferer { 28.0 } else { 24.0 };
                if after {
                    match self.kind {
                        MobKind::Pilferer => {
                            if fd > 12.0 {
                                want = Some((face, 2.6));
                            } else if fd < 5.0 {
                                want = Some((face + std::f32::consts::PI, 2.2));
                            } else {
                                self.yaw += angle_diff(face, self.yaw).clamp(-8.0 * dt, 8.0 * dt);
                                may_wander = false;
                            }
                            let eye = self.eye();
                            let aim = player + Vec3::Y * 0.9 - eye;
                            let clear = world.raycast(eye, aim.normalize_or_zero(), aim.length()).is_none();
                            self.fuse = if self.attack_cd < 0.8 { 1.0 } else { 0.0 };
                            if self.attack_cd <= 0.0 && fd < 20.0 && clear {
                                let vel = aim.normalize_or_zero() * Arrow::SPEED * 1.25 + Vec3::Y * aim.length() * 0.3;
                                ev.push(MobEvent::Shoot(eye + aim.normalize_or_zero() * 0.5, vel));
                                self.attack_cd = rng.range(2.0, 3.0);
                            }
                        }
                        MobKind::Invoicer => {
                            // Keeps its distance and casts.
                            if fd < 6.0 {
                                want = Some((face + std::f32::consts::PI, 2.6));
                            } else if fd > 12.0 {
                                want = Some((face, 2.2));
                            } else {
                                self.yaw += angle_diff(face, self.yaw).clamp(-8.0 * dt, 8.0 * dt);
                                may_wander = false;
                            }
                            self.fuse = if self.attack_cd < 1.0 { 1.0 } else { 0.0 };
                            if self.attack_cd <= 0.0 {
                                if rng.chance(0.4) {
                                    ev.push(MobEvent::Summon(self.body.pos + Vec3::Y * 1.5));
                                } else {
                                    ev.push(MobEvent::Fangs(self.body.pos, player));
                                }
                                self.attack_cd = rng.range(5.0, 7.0);
                            }
                        }
                        _ => {
                            let (speed, reach, dmg, cause, cd) = if self.kind == MobKind::Rampager { (2.9, 2.2, 12.0, "was trampled by a Rampager", 1.6) } else { (3.5, 1.3, 8.0, "was hackled. Rude", 1.0) };
                            want = Some((face, speed));
                            if fd < reach && to_player.y.abs() < 2.0 && self.attack_cd <= 0.0 {
                                ev.push(MobEvent::HurtPlayer(dmg, cause));
                                self.attack_cd = cd;
                            }
                        }
                    }
                } else if let Some(h) = self.home {
                    let d = h - self.body.pos;
                    if Vec3::new(d.x, 0.0, d.z).length() > 5.0 {
                        want = Some((d.x.atan2(-d.z), 2.4));
                    }
                }
            }
            MobKind::Fee => {
                // Straight at you, through whatever's in the way; it fades after a while.
                may_wander = false;
                self.fuse += dt;
                let d = player + Vec3::Y * 1.0 - self.body.pos;
                if player_visible {
                    want = Some((d.x.atan2(-d.z), 4.5));
                    fly_vy = Some((d.y * 2.5).clamp(-4.0, 4.0));
                }
                if player_visible && d.length() < 1.0 && self.attack_cd <= 0.0 {
                    ev.push(MobEvent::HurtPlayer(3.0, "was charged a Fee. Non-refundable"));
                    self.attack_cd = 1.0;
                }
                if self.fuse > 30.0 {
                    self.health -= dt;
                }
            }
            MobKind::Groaner => {
                if player_visible && dist < 24.0 {
                    want = Some((flat.x.atan2(-flat.z), 2.3));
                    if flat.length() < 1.3 && to_player.y.abs() < 1.6 && self.attack_cd <= 0.0 {
                        ev.push(MobEvent::HurtPlayer(3.0, "was groaned to death"));
                        self.attack_cd = 1.0;
                    }
                }
            }
            MobKind::Rattler => {
                let fd = flat.length();
                if player_visible && dist < 22.0 {
                    // Keep a polite shooting distance.
                    if fd > 11.0 {
                        want = Some((face, 2.2));
                    } else if fd < 5.0 {
                        want = Some((face + std::f32::consts::PI, 2.0));
                    } else {
                        self.yaw += angle_diff(face, self.yaw).clamp(-8.0 * dt, 8.0 * dt);
                        may_wander = false;
                    }
                    let eye = self.eye();
                    let aim = player - eye;
                    let clear = world.raycast(eye, aim.normalize_or_zero(), aim.length()).is_none();
                    if self.attack_cd <= 0.0 && fd < 16.0 && clear {
                        // Lob it a little higher the further away you are (arrows drop).
                        let vel = aim.normalize_or_zero() * Arrow::SPEED + Vec3::Y * aim.length() * 0.42;
                        ev.push(MobEvent::Shoot(eye + aim.normalize_or_zero() * 0.5, vel));
                        self.attack_cd = rng.range(1.6, 2.6);
                    }
                }
            }
            MobKind::Webber => {
                if !player_visible || dist > 32.0 {
                    self.angry = false;
                }
                // Neutral in bright daylight unless provoked.
                if player_visible && dist < 18.0 && (self.angry || daylight < 0.5) {
                    want = Some((face, 3.4));
                    if flat.length() < self.body.half + 0.9 && to_player.y.abs() < 1.5 && self.attack_cd <= 0.0 {
                        ev.push(MobEvent::HurtPlayer(2.0, "was nibbled by a Webber"));
                        self.attack_cd = 0.9;
                    }
                }
            }
            MobKind::Bloop => {
                may_wander = false;
                self.hop_cd = (self.hop_cd - dt).max(0.0);
                if self.body.on_ground {
                    if self.hop_cd <= 0.0 {
                        let chase = player_visible && dist < 16.0;
                        self.yaw = if chase { face } else { rng.range(0.0, std::f32::consts::TAU) };
                        self.body.vel.y = 6.0 + self.size * 0.6;
                        self.body.on_ground = false;
                        self.hop_cd = if chase { rng.range(0.6, 1.2) } else { rng.range(1.5, 3.5) };
                        self.anim += 1.0;
                    }
                } else {
                    want = Some((self.yaw, 2.6 + self.size * 0.3));
                }
                // Small ones are harmless; bigger ones hurt on contact.
                if self.size >= 2.0 && flat.length() < self.body.half + 0.6 && to_player.y.abs() < self.body.height && self.attack_cd <= 0.0 {
                    ev.push(MobEvent::HurtPlayer(self.size, "was bloop'd"));
                    self.attack_cd = 1.0;
                }
            }
            MobKind::Goat => {
                self.warp_cd = (self.warp_cd - dt).max(0.0);
                if self.hop_cd > 0.0 {
                    // Head down, charging in a straight line.
                    self.hop_cd -= dt;
                    may_wander = false;
                    if self.hop_cd < 1.2 {
                        want = Some((self.wander_dir.unwrap_or(self.yaw), 7.5));
                    } else {
                        self.yaw += angle_diff(face, self.yaw).clamp(-6.0 * dt, 6.0 * dt);
                        self.wander_dir = Some(face);
                    }
                    if flat.length() < self.body.half + 0.9 && to_player.y.abs() < 1.6 && self.attack_cd <= 0.0 {
                        ev.push(MobEvent::HurtPlayer(2.0, "was rammed by a Goat"));
                        self.attack_cd = 1.0;
                        self.hop_cd = 0.0;
                    }
                } else if self.flee > 0.0 {
                    want = Some(((-flat.x).atan2(flat.z), 3.5));
                } else if let Some(g) = self.goal {
                    let d = g - self.body.pos;
                    if Vec3::new(d.x, 0.0, d.z).length() > 1.1 {
                        want = Some((d.x.atan2(-d.z), 1.8));
                    } else {
                        may_wander = false;
                    }
                } else if self.baby <= 0.0 && player_visible && (4.0..12.0).contains(&flat.length()) && to_player.y.abs() < 2.0 && self.warp_cd <= 0.0 {
                    // Takes a dislike to you every so often.
                    self.warp_cd = rng.range(20.0, 50.0);
                    self.hop_cd = 2.0;
                    self.wander_dir = Some(face);
                    ev.push(MobEvent::Bleat(self.body.pos));
                }
            }
            MobKind::Axolotl => {
                if self.body.in_water {
                    let ahead = self.body.pos + Vec3::new(self.yaw.sin(), 0.2, -self.yaw.cos()) * 0.8;
                    if !is_water(world.get(ahead.x.floor() as i32, ahead.y.floor() as i32, ahead.z.floor() as i32)) {
                        self.wander_dir = Some(self.yaw + std::f32::consts::PI + rng.range(-0.8, 0.8));
                        self.wander_t = rng.range(1.0, 3.0);
                    }
                    if self.flee > 0.0 {
                        want = Some(((-flat.x).atan2(flat.z), 4.0));
                    } else if let Some(g) = self.goal {
                        // After something to eat (see beasts.rs), or someone holding fish.
                        let d = g - self.body.pos;
                        if Vec3::new(d.x, 0.0, d.z).length() > 0.8 {
                            want = Some((d.x.atan2(-d.z), 3.4));
                        }
                        swim_vy = Some((d.y * 2.0).clamp(-2.5, 2.5));
                    }
                    if swim_vy.is_none() {
                        let above = world.get(self.body.pos.x.floor() as i32, (self.body.pos.y + 0.6).floor() as i32, self.body.pos.z.floor() as i32);
                        let drift = (self.wander_t * 1.3 + self.id as f32).sin() * 0.6;
                        swim_vy = Some(if is_water(above) { drift } else { drift.min(0.0) - 0.3 });
                    }
                } else if let Some(g) = self.goal {
                    // A slow waddle on land.
                    let d = g - self.body.pos;
                    if Vec3::new(d.x, 0.0, d.z).length() > 1.0 {
                        want = Some((d.x.atan2(-d.z), 1.2));
                    }
                }
            }
            MobKind::Creaking => {
                // Frozen while anyone watches it (see creaking.rs).
                may_wander = false;
                if self.sitting {
                    self.body.vel.x = 0.0;
                    self.body.vel.z = 0.0;
                    self.knock = Vec3::ZERO;
                } else if let Some(g) = self.goal.filter(|g| g.distance(self.body.pos) > crate::creaking::LEASH) {
                    // Too far from its heart: back it goes.
                    let d = g - self.body.pos;
                    want = Some((d.x.atan2(-d.z), 3.0));
                } else if player_visible && dist < 32.0 {
                    want = Some((face, 3.8));
                    if flat.length() < 1.4 && to_player.y.abs() < 2.2 && self.attack_cd <= 0.0 {
                        ev.push(MobEvent::HurtPlayer(3.0, "was caught by a Creaking. Should have kept looking"));
                        self.attack_cd = 1.2;
                    }
                }
            }
            MobKind::CopperGolem => {
                // Off to a chest (golems.rs picks which), or pottering about.
                if self.flee > 0.0 {
                    want = Some(((-flat.x).atan2(flat.z), 2.4));
                } else if let Some(g) = self.goal {
                    let d = g - self.body.pos;
                    if Vec3::new(d.x, 0.0, d.z).length() > 1.3 {
                        want = Some((d.x.atan2(-d.z), 1.7));
                    } else {
                        self.yaw += angle_diff(d.x.atan2(-d.z), self.yaw).clamp(-6.0 * dt, 6.0 * dt);
                        may_wander = false;
                    }
                }
            }
            MobKind::Floaty => {
                // Drifts about a few blocks up; harnessed and riderless, it waits where it is.
                let ground = ground_below(world, self.body.pos, 24);
                let cruise = ground + if self.baby > 0.0 { 2.5 } else { 4.5 };
                if self.saddled && self.rider == 0 {
                    may_wander = false;
                    fly_vy = Some(0.0);
                    self.body.vel.x *= 1.0 - (dt * 3.0).min(1.0);
                    self.body.vel.z *= 1.0 - (dt * 3.0).min(1.0);
                } else if let Some(g) = self.goal {
                    // Someone holding a Harness or a snowball: come and see.
                    let d = g - self.body.pos;
                    if Vec3::new(d.x, 0.0, d.z).length() > 4.0 {
                        want = Some((d.x.atan2(-d.z), 2.2));
                    } else {
                        may_wander = false;
                    }
                    fly_vy = Some(((g.y + 1.5 - self.body.pos.y) * 0.8).clamp(-2.0, 2.0));
                } else {
                    fly_vy = Some(((cruise - self.body.pos.y) * 0.6).clamp(-1.5, 1.5));
                }
            }
            MobKind::Sniffer => {
                // Now and then it stops, sniffs, and digs something up (see sniffers.rs).
                self.warp_cd = (self.warp_cd - dt).max(0.0);
                if self.flee > 0.0 {
                    want = Some(((-flat.x).atan2(flat.z), 2.6));
                } else if self.hop_cd > 0.0 {
                    may_wander = false;
                    self.hop_cd -= dt;
                    if self.hop_cd <= 0.0 && self.baby <= 0.0 {
                        let under = (self.body.pos - Vec3::Y * 0.5).floor().as_ivec3();
                        if crate::sniffers::diggable(world.get_v(under)) {
                            ev.push(MobEvent::DropItem(self.body.pos + Vec3::Y * 0.4, crate::sniffers::dig_up(rng)));
                        }
                        self.warp_cd = rng.range(40.0, 90.0);
                    }
                } else if let Some(g) = self.goal {
                    let d = g - self.body.pos;
                    if Vec3::new(d.x, 0.0, d.z).length() > 1.6 {
                        want = Some((d.x.atan2(-d.z), 1.4));
                    } else {
                        may_wander = false;
                    }
                } else if self.warp_cd <= 0.0 && self.body.on_ground && rng.chance(dt * 0.2) {
                    self.hop_cd = 4.0;
                }
            }
            MobKind::Breeze => {
                self.hop_cd = (self.hop_cd - dt).max(0.0);
                let fd = flat.length();
                if player_visible && dist < 24.0 {
                    may_wander = false;
                    self.yaw += angle_diff(face, self.yaw).clamp(-8.0 * dt, 8.0 * dt);
                    // Bounce about, keeping a good throwing distance.
                    if self.body.on_ground && self.hop_cd <= 0.0 {
                        let dir = if fd > 10.0 {
                            face + rng.range(-0.5, 0.5)
                        } else if fd < 5.0 {
                            face + std::f32::consts::PI + rng.range(-0.6, 0.6)
                        } else {
                            face + std::f32::consts::FRAC_PI_2 * if rng.chance(0.5) { 1.0 } else { -1.0 }
                        };
                        self.body.vel.y = rng.range(7.0, 10.0);
                        self.body.on_ground = false;
                        self.knock = Vec3::new(dir.sin(), 0.0, -dir.cos()) * 4.5;
                        self.hop_cd = rng.range(0.9, 2.0);
                    }
                    let eye = self.eye();
                    let aim = player + Vec3::Y * 0.9 - eye;
                    let clear = world.raycast(eye, aim.normalize_or_zero(), aim.length()).is_none();
                    if self.attack_cd <= 0.0 && fd < 18.0 && clear {
                        let n = aim.normalize_or_zero();
                        ev.push(MobEvent::WindCharge(eye + n * 0.6, n * 15.0));
                        self.attack_cd = rng.range(2.0, 3.0);
                    }
                }
            }
            MobKind::Starer => {
                self.warp_cd = (self.warp_cd - dt).max(0.0);
                if !player_visible || dist > 48.0 {
                    self.angry = false;
                }
                // Starers can't stand water (or getting hit): blink somewhere else.
                if self.body.in_water {
                    self.health -= dt;
                    self.flee = self.flee.max(0.5);
                }
                let mut warp_to = None;
                if self.flee > 0.0 && self.warp_cd <= 0.0 {
                    warp_to = warp_spot(world, self.body.pos, 10.0, rng);
                    self.flee = 0.0;
                } else if self.angry {
                    if flat.length() > 1.2 {
                        want = Some((flat.x.atan2(-flat.z), 3.6));
                    } else {
                        self.yaw = flat.x.atan2(-flat.z);
                    }
                    if flat.length() < 1.5 && to_player.y.abs() < 2.0 && self.attack_cd <= 0.0 {
                        ev.push(MobEvent::HurtPlayer(5.0, "lost a staring contest with a Starer"));
                        self.attack_cd = 1.2;
                    }
                    // Too far to walk? Blink closer.
                    if dist > 10.0 && self.warp_cd <= 0.0 {
                        warp_to = warp_spot(world, player, 3.5, rng);
                    }
                }
                if let Some(to) = warp_to {
                    ev.push(MobEvent::Warp(self.body.pos, to));
                    self.body.pos = to;
                    self.body.vel = Vec3::ZERO;
                    self.knock = Vec3::ZERO;
                    self.warp_cd = rng.range(2.5, 4.5);
                }
            }
            MobKind::Modded(_) => {
                // A hostile modded mob with attack_damage > 0 pursues and melee-
                // attacks the player, mirroring the base Groaner; one with
                // ranged_damage > 0 or a configured effect also fires
                // projectiles at range, mirroring the Rattler. A mob that is
                // not hostile, or has no melee damage, ranged damage or effect
                // (the backward-compatible default), stays a passive wanderer that flees when
                // hit. All of this runs host-side so it stays deterministic and
                // host-authoritative; the events are applied in game.rs.
                // A tamed one never turns on players; a sitting one stays put.
                let def = self.kind.mod_def();
                let tamed = self.owner.is_some();
                let attacking = def
                    .filter(|d| !tamed && d.hostile && (d.attack_damage > 0.0 || d.ranged_damage > 0.0 || d.projectile_effect.is_some()));
                let flier = def.filter(|d| d.flying);
                // How high a flier wants to be: cruising height unless it has
                // somewhere (or someone) to get to.
                let mut fly_goal: Option<f32> = None;
                let mut chasing = false;
                if let Some(d) = flier {
                    // Untamed perchers land for a rest now and then.
                    if d.perches && !tamed {
                        self.hop_cd -= dt;
                        if self.flee > 0.0 || (attacking.is_some() && player_visible && dist < d.aggro_range) {
                            self.sitting = false;
                        } else if self.hop_cd <= 0.0 {
                            self.sitting = !self.sitting;
                            self.hop_cd = if self.sitting { rng.range(5.0, 15.0) } else { rng.range(10.0, 25.0) };
                        }
                    }
                }
                // A boss enrages below its threshold: faster, quicker to strike,
                // and calling for help every so often (see `summon`).
                let (mut spd_mul, mut cd_mul) = (1.0, 1.0);
                if let Some(d) = def.filter(|d| d.boss && d.enrage_at > 0.0) {
                    if !self.angry && self.health <= d.max_health * d.enrage_at {
                        self.angry = true;
                        self.warp_cd = 0.0;
                        ev.push(MobEvent::Smoke(self.eye()));
                    }
                    if self.angry {
                        spd_mul = d.enrage_speed;
                        cd_mul = d.enrage_cooldown;
                        self.warp_cd = (self.warp_cd - dt).max(0.0);
                        if let Some((kind, n)) = d.summon
                            && self.warp_cd <= 0.0
                            && player_visible
                            && dist < d.aggro_range.max(d.ranged_range)
                        {
                            ev.push(MobEvent::SummonMod(self.body.pos, kind, n));
                            self.warp_cd = d.summon_every;
                        }
                    }
                }
                if self.sitting {
                    may_wander = false;
                } else if let Some(def) = attacking {
                    let melee = def.attack_damage > 0.0;
                    let ranged = def.ranged_damage > 0.0 || def.projectile_effect.is_some();
                    // A ranged mob still approaches/tracks within its firing
                    // range even when that reaches past aggro_range.
                    let pursue_range = if ranged { def.aggro_range.max(def.ranged_range) } else { def.aggro_range };
                    if player_visible && dist < pursue_range {
                        // Pursue at the base chase speed scaled by the def's speed
                        // (a flier at its own speed, swooping to the player's height
                        // to bite, or holding its altitude to shoot).
                        chasing = true;
                        want = Some((face, spd_mul * if def.flying { def.fly_speed } else { 2.3 * def.speed }));
                        if def.flying && melee {
                            fly_goal = Some(player.y + 0.6);
                        }
                        let fd = flat.length();
                        // Melee takes priority up close; otherwise fire a ranged
                        // shot at distance. Both share the self.attack_cd timer
                        // (decremented by dt at the top of update), so the mob
                        // can't melee and shoot in the same cooldown window.
                        if melee && fd < def.attack_reach && to_player.y.abs() < 1.6 && self.attack_cd <= 0.0 {
                            // Fixed &'static cause (HurtPlayer takes a &'static str,
                            // so we cannot use the mob's dynamic name here).
                            ev.push(MobEvent::HurtPlayer(def.attack_damage, "was mauled by a monster"));
                            self.attack_cd = def.attack_cooldown * cd_mul;
                        } else if ranged && fd < def.ranged_range && self.attack_cd <= 0.0 {
                            // Raycast a clear line of sight before loosing, like
                            // the Rattler. Lob it higher the further away (arrows
                            // drop). Cadence is the data-driven ranged_cooldown,
                            // with no added jitter, so it stays reproducible.
                            let eye = self.eye();
                            let aim = player - eye;
                            let clear = world.raycast(eye, aim.normalize_or_zero(), aim.length()).is_none();
                            if clear {
                                // A homing shot flies straight (no drop), so it needs no lob.
                                let lob = if def.projectile_homing > 0.0 { 0.0 } else { 0.42 };
                                let vel = aim.normalize_or_zero() * def.projectile_speed + Vec3::Y * aim.length() * lob;
                                let spec = ModProjectileSpec {
                                    damage: def.ranged_damage,
                                    appearance: def.projectile_appearance,
                                    effect: def.projectile_effect,
                                    count: def.projectile_count,
                                    spread: def.projectile_spread,
                                    homing: def.projectile_homing,
                                    blast: def.projectile_blast,
                                };
                                ev.push(MobEvent::ShootMod(eye + aim.normalize_or_zero() * 0.5, vel, spec));
                                self.attack_cd = def.ranged_cooldown * cd_mul;
                            }
                        }
                    }
                } else if self.flee > 0.0 {
                    // Passive / non-attacking: flee when hit, else amble.
                    want = Some(((-flat.x).atan2(flat.z), flier.map(|d| d.fly_speed).unwrap_or(3.5)));
                }
                // An owner, a partner, or someone holding its favourite food (see animals.rs).
                if !self.sitting && !chasing && self.flee <= 0.0
                    && let Some(g) = self.goal {
                        let d = g - self.body.pos;
                        let fd = Vec3::new(d.x, 0.0, d.z).length();
                        let speed = flier.map(|f| f.fly_speed).unwrap_or(2.0 * def.map(|d| d.speed).unwrap_or(1.0));
                        if fd > 1.6 {
                            want = Some((d.x.atan2(-d.z), speed));
                        } else {
                            may_wander = false;
                        }
                        if flier.is_some() {
                            fly_goal = Some(g.y + 1.2);
                        }
                    }
                if let Some(d) = flier
                    && !self.sitting {
                        let ground = ground_below(world, self.body.pos, 24);
                        let target = fly_goal.unwrap_or(ground + d.fly_height).max(ground + 0.6);
                        let bob = (self.wander_t * 2.0 + self.id as f32).sin() * 0.3;
                        fly_vy = Some(((target - self.body.pos.y) * 2.0 + bob).clamp(-d.fly_speed, d.fly_speed));
                    }
            }
        }
        if self.kind.burns() {
            let head = self.eye();
            self.burning = daylight > 0.65 && world.sky_light(head.x.floor() as i32, head.y.floor() as i32, head.z.floor() as i32) >= 1.0 && !self.body.in_water;
            if self.burning {
                self.health -= dt * 1.5;
                if rng.chance(dt * 8.0) {
                    ev.push(MobEvent::Smoke(head));
                }
            }
        }
        // Lava hurts and sets things alight; water puts them out.
        if self.kind.fireproof() {
            self.on_fire = 0.0;
        } else if self.body.in_lava {
            self.on_fire = 6.0;
            self.health -= dt * 6.0;
            self.hurt = self.hurt.max(0.2);
        } else if self.body.in_water {
            self.on_fire = 0.0;
        } else if crate::fire::touches_fire(world, self.body.min(), self.body.max()) {
            self.on_fire = self.on_fire.max(4.0);
        }
        if self.on_fire > 0.0 {
            self.on_fire -= dt;
            self.health -= dt;
            self.burning = true;
            if rng.chance(dt * 8.0) {
                ev.push(MobEvent::Smoke(self.eye()));
            }
        }
        if want.is_none() && may_wander {
            self.wander_t -= dt;
            if self.wander_t <= 0.0 {
                self.wander_t = rng.range(2.0, 6.0);
                self.wander_dir = if rng.chance(0.55) { Some(rng.range(0.0, std::f32::consts::TAU)) } else { None };
            }
            want = self.wander_dir.map(|y| (y, 1.2));
        }

        // Heading somewhere (a goal, or the player it's after) on foot: round
        // whatever's in the way (see pathing.rs). Fleeing and wandering don't bother.
        if let Some((yaw, speed)) = want
            && !self.kind.flies()
            && self.kind != MobKind::Fee
            && !self.body.in_water
            && !self.sitting
        {
            let toward = |t: Vec3| (t.x - self.body.pos.x).atan2(-(t.z - self.body.pos.z));
            let target = match self.goal {
                Some(g) if angle_diff(yaw, toward(g)).abs() < 0.35 => Some(g),
                _ if dist < 40.0 && speed > 1.5 && angle_diff(yaw, face).abs() < 0.35 => Some(player),
                _ => None,
            };
            match target {
                Some(t) => {
                    let stuck = self.body.hit_wall && self.body.on_ground;
                    if let Some(y) = crate::pathing::steer(world, &mut self.path, self.body.pos, t, self.body.height, stuck, dt) {
                        want = Some((y, speed));
                    }
                }
                None => self.path = None,
            }
        }
        let (target_vel, moving) = match want {
            Some((yaw, speed)) => {
                let d = angle_diff(yaw, self.yaw);
                self.yaw += d.clamp(-8.0 * dt, 8.0 * dt);
                (Vec3::new(yaw.sin(), 0.0, -yaw.cos()) * speed, true)
            }
            None => (Vec3::ZERO, false),
        };
        let k = (dt * 10.0).min(1.0);
        self.body.vel.x += (target_vel.x + self.knock.x - self.body.vel.x) * k;
        self.body.vel.z += (target_vel.z + self.knock.z - self.body.vel.z) * k;
        self.knock *= (1.0 - dt * 6.0).max(0.0);
        // A modded flier that has landed (perched or told to sit) falls like anything else.
        let grounded_flier = self.sitting && matches!(self.kind, MobKind::Modded(_));
        if self.kind.flies() && !grounded_flier {
            self.body.vel.y += (fly_vy.unwrap_or(0.0) - self.body.vel.y) * k;
        } else if let (true, Some(vy)) = (self.body.in_water, swim_vy) {
            self.body.vel.y += (vy - self.body.vel.y) * k;
        } else if self.body.in_water {
            self.body.vel.y = (self.body.vel.y + 14.0 * dt).min(2.5);
        } else {
            self.body.vel.y = (self.body.vel.y - GRAVITY * dt).max(-50.0);
            if matches!(self.kind, MobKind::Cluckster | MobKind::Squawker) {
                // Flap flap: Clucksters flutter down instead of falling.
                self.body.vel.y = self.body.vel.y.max(-2.5);
            }
        }
        let prev_ground = self.body.on_ground;
        if self.kind == MobKind::Fee {
            // Through anything.
            self.body.pos += self.body.vel * dt;
        } else {
            move_body(world, &mut self.body, dt, false);
        }
        if self.kind == MobKind::Strutter && self.body.in_lava {
            // Lava is a floor to a Strutter.
            self.body.vel.y = self.body.vel.y.max(3.5);
        }
        if self.kind == MobKind::Goat && self.hop_cd > 0.0 && self.hop_cd < 1.2 && self.body.hit_wall {
            // Straight into a wall: that's the end of the charge, and maybe a horn.
            self.hop_cd = 0.0;
            if !self.sheared && self.baby <= 0.0 && rng.chance(0.5) {
                self.sheared = true;
                ev.push(MobEvent::DropItem(self.body.pos + Vec3::Y * 0.9, GOAT_HORN));
            }
        }
        if moving && self.body.hit_wall {
            match self.kind {
                // Goats are great jumpers.
                MobKind::Goat if self.body.on_ground || prev_ground => self.body.vel.y = 10.5,
                // Webbers walk straight up walls.
                MobKind::Webber => self.body.vel.y = 3.2,
                MobKind::Bee => self.body.vel.y = 3.0,
                MobKind::Bloop | MobKind::Ribbit => {}
                _ if self.body.on_ground || prev_ground => self.body.vel.y = 8.8,
                _ => {}
            }
        }
        let spd = Vec3::new(self.body.vel.x, 0.0, self.body.vel.z).length();
        if self.kind == MobKind::Bee {
            // Wings never stop.
            self.anim += dt * 30.0;
        } else {
            step_anim(&mut self.anim, spd, dt, 5.0);
        }
        self.flutter(!self.body.on_ground && !self.body.in_water && self.body.vel.y < -0.5, dt);
        if self.body.pos.y < -20.0 {
            self.health = -100.0;
        }
        ev
    }

    /// Items dropped on death.
    pub fn loot(&self, rng: &mut Rng) -> Option<(Id, u8)> {
        let n = rng.int(0, 2) as u8;
        match self.kind {
            MobKind::Oinker => Some((PORKCHOP, n.max(1))),
            MobKind::Hisser if n > 0 && self.health > -50.0 => Some((GUNPOWDER, n)),
            MobKind::Groaner if n > 0 => Some((GOO, n)),
            MobKind::Fluffer if !self.sheared => Some((WOOL, n.max(1))),
            MobKind::Starer if n > 0 => Some((PEARL, 1)),
            MobKind::Cluckster | MobKind::Squawker if n > 0 => Some((FEATHER, n)),
            MobKind::Mooer => Some((MOO_STEAK, n + 1)),
            MobKind::Rattler if n > 0 => Some((BONE, n)),
            MobKind::Webber if n > 0 => Some((STRING, n)),
            // Only the smallest Bloops leave anything; bigger ones split instead.
            MobKind::Bloop if n > 0 && self.size <= 1.0 => Some((GOO, n)),
            MobKind::Grumbler if n > 0 => Some((GOO, n)),
            MobKind::Clanker => Some((IRON, 3 + n)),
            MobKind::Fishy => Some(([COD, COD, SALMON, TROPICAL][rng.int(0, 3) as usize], 1)),
            // An armed one drops its spear now and then; otherwise it's soggy goo.
            MobKind::Soggy if self.seed == 1 && rng.chance(0.25) => Some((SPEAR, 1)),
            MobKind::Soggy if n > 0 => Some((GOO, n)),
            // A Sneaker drops whatever it was carrying.
            MobKind::Sneaker if self.seed != 0 => Some((self.seed as Id, 1)),
            MobKind::Rollo if rng.chance(0.3) => Some((SCUTE, 1)),
            MobKind::Hush => Some((SCULK_CATALYST, 1)),
            MobKind::Sizzler if rng.chance(0.5) => Some((SIZZLE_ROD, 1)),
            MobKind::Weeper if rng.chance(0.6) => Some((WEEPER_TEAR, 1)),
            MobKind::Strutter => Some((STRING, rng.int(2, 5) as u8)),
            MobKind::Snout if rng.chance(0.25) => Some((GOLD_INGOT, 1)),
            MobKind::Pilferer if rng.chance(0.08) => Some((CROSSBOW, 1)),
            MobKind::Pilferer if n > 0 => Some((ARROW, n)),
            MobKind::Hackler if rng.chance(0.085) => Some((AXE_FIRST + 3, 1)),
            MobKind::Invoicer => Some((TOTEM, 1)),
            MobKind::Rampager => Some((SADDLE, 1)),
            MobKind::Breeze => Some((BREEZE_ROD, rng.int(1, 2) as u8)),
            // A Copper Golem drops what it was carrying, and some copper.
            MobKind::CopperGolem if self.seed != 0 => Some(((self.seed & 0xFFFF) as Id, ((self.seed >> 16) & 0xFF).max(1) as u8)),
            MobKind::CopperGolem => Some((COPPER_INGOT, 2 + n)),
            MobKind::Rotsteed if n > 0 => Some((GOO, n)),
            MobKind::Floaty => Some((STRING, 1 + n)),
            MobKind::Modded(_) => self.kind.mod_def().and_then(|d| d.drop).map(|(id, max)| (id, rng.int(1, max.max(1) as i32) as u8)),
            _ => None,
        }
        .filter(|_| self.baby <= 0.0)
    }

    /// Anything dropped besides `loot` (Baa-con, Cluckets, spare Pointy Sticks).
    pub fn extra_loot(&self, rng: &mut Rng) -> Option<(Id, u8)> {
        match self.kind {
            MobKind::Fluffer if rng.chance(0.7) => Some((MUTTON, 1)),
            MobKind::Cluckster => Some((CLUCKETS, 1)),
            MobKind::Rattler if rng.chance(0.6) => Some((ARROW, rng.int(1, 2) as u8)),
            MobKind::Grumbler if rng.chance(0.4) => Some((GOLD_INGOT, 1)),
            MobKind::Grumbler if rng.chance(0.5) => Some((GRUMBLER_TUSK, 1)),
            MobKind::Soggy if rng.chance(0.11) => Some((COPPER_INGOT, 1)),
            MobKind::Weeper => Some((GUNPOWDER, rng.int(0, 2) as u8)).filter(|l| l.1 > 0),
            // A patrol's captain carries the banner (see raids.rs).
            MobKind::Pilferer if self.seed == 1 => Some((OMINOUS_BANNER, 1)),
            MobKind::Invoicer if rng.chance(0.5) => Some((GOLD_INGOT, rng.int(1, 3) as u8)),
            _ => None,
        }
        .filter(|_| self.baby <= 0.0)
    }

    /// Is someone at `eye` looking at `dir` staring this mob in the face?
    pub fn stared_at(&self, eye: Vec3, dir: Vec3) -> bool {
        let head = self.body.pos + Vec3::Y * (self.body.height - 0.25);
        let to = head - eye;
        let d = to.length();
        // Within about a head's width of the crosshair.
        d > 0.5 && d < 48.0 && to.dot(dir) / d > 1.0 - 0.5 * (0.4 / d).powi(2)
    }

    /// Drawn again a little bigger and glowing (a Bell rang; see raids.rs).
    pub fn draw_glow(&self, geo: &mut DynGeo, world: &World) {
        let _ = world;
        geo.begin(Pass::Opaque, [3.0, 2.6, 1.2, 1.0], false);
        let root = Mat4::from_translation(self.body.pos - Vec3::Y * 0.05) * Mat4::from_rotation_y(-self.yaw) * Mat4::from_scale(Vec3::splat(self.size * 1.07));
        draw_posed(geo, &root, model(self.kind), self.anim, self.flap, 1.0);
    }

    pub fn draw(&self, geo: &mut DynGeo, world: &World) {
        let flash = self.kind == MobKind::Hisser && self.fuse > 0.0 && (self.fuse * 14.0).sin() > 0.0;
        let tint = if self.hurt > 0.3 {
            [1.0, 0.45, 0.45, 1.0]
        } else if flash {
            [3.0, 3.0, 3.0, 1.0]
        } else if self.burning {
            [1.0, 0.8, 0.6, 1.0]
        } else if self.angry && self.kind == MobKind::Starer {
            [1.6, 0.9, 1.8, 1.0]
        } else {
            [1.0; 4]
        };
        let mut p = self.body.pos;
        if self.angry && self.kind == MobKind::Starer {
            // Angry Starers vibrate with rage.
            p.x += (self.anim * 37.0 + self.id as f32).sin() * 0.03;
            p.z += (self.anim * 29.0).cos() * 0.03;
        }
        let sky = world.sky_shade(p.x.floor() as i32, (p.y + 0.5).floor() as i32, p.z.floor() as i32);
        let v = variant_tint(self.kind, self.variant);
        let tint = [tint[0] * v[0], tint[1] * v[1], tint[2] * v[2], tint[3]];
        geo.begin(Pass::Opaque, tint, false);
        let swell = if self.kind == MobKind::Hisser { 1.0 + self.fuse * 0.08 } else { 1.0 };
        let mut scale = Vec3::splat(swell * self.size * if self.baby > 0.0 { 0.55 } else { 1.0 });
        if self.kind == MobKind::Bloop && !self.body.on_ground {
            // Stretch a little mid-hop.
            scale *= Vec3::new(0.9, 1.2, 0.9);
        }
        if self.sitting {
            p.y -= 0.15 * scale.y;
        }
        let root = Mat4::from_translation(p) * Mat4::from_rotation_y(-self.yaw) * Mat4::from_scale(scale);
        // Modded mobs carry their own texture, so their parts are built fresh.
        if let MobKind::Modded(_) = self.kind {
            if let Some(def) = self.kind.mod_def() {
                let parts = modded_parts(def);
                // A template body is scaled (keeping its proportions) to the
                // mob's own height (custom parts are already measured in blocks).
                let root = if def.parts.is_empty() {
                    use crate::block::MobTemplate::*;
                    let base = match def.template {
                        Quadruped => MobKind::Oinker,
                        Biped => MobKind::Groaner,
                        Blob => MobKind::Bloop,
                        Bird => MobKind::Cluckster,
                    };
                    let (_, h) = base.dims();
                    root * Mat4::from_scale(Vec3::splat(def.height / h))
                } else {
                    root
                };
                draw_posed(geo, &root, &parts, self.anim, self.flap, sky);
            }
            return;
        }
        let parts = match self.kind {
            MobKind::Fluffer if self.sheared => &FLUFFER_SHEARED[..],
            MobKind::Rollo if self.fuse > 0.0 => &ROLLO_BALL[..],
            MobKind::Soggy if self.seed == 1 => &SOGGY_ARMED[..],
            MobKind::Weeper if self.angry => &WEEPER_ANGRY[..],
            MobKind::Creaking if self.sitting => &CREAKING_STILL[..],
            MobKind::Invoicer if self.fuse > 0.0 => &INVOICER_CASTING[..],
            MobKind::Strutter if !is_lava(world.get(p.x.floor() as i32, (p.y - 0.3).floor() as i32, p.z.floor() as i32)) && !self.body.in_lava => &STRUTTER_COLD[..],
            k => model(k),
        };
        draw_posed(geo, &root, parts, if self.sitting { 0.0 } else { self.anim }, self.flap, sky);
        if self.owner.is_some() && self.kind == MobKind::Woofer {
            draw_model(geo, &root, &WOOFER_COLLAR, 0.0, sky, false);
        }
        if self.saddled && self.kind == MobKind::Woofer {
            draw_model(geo, &root, &WOOFER_ARMOUR, 0.0, sky, false);
        } else if self.saddled && self.kind == MobKind::Camel {
            draw_model(geo, &root, &CAMEL_SADDLE, 0.0, sky, false);
        } else if self.saddled && self.kind == MobKind::Floaty {
            draw_model(geo, &root, &FLOATY_HARNESS, 0.0, sky, false);
        } else if self.saddled {
            draw_model(geo, &root, &SADDLE_PART, 0.0, sky, false);
        }
        if self.kind == MobKind::Pilferer && self.seed == 1 {
            draw_model(geo, &root, &BANNER_BACK, 0.0, sky, false);
        }
        // What raiders and Snouts carry: a crossbow, an axe, some gold being admired.
        let carried = match self.kind {
            MobKind::Pilferer => Some(if self.fuse > 0.0 { T_CROSSBOW_LOADED } else { T_CROSSBOW }),
            MobKind::Hackler => Some(item_tile(AXE_FIRST + 3)),
            MobKind::Snout if self.seed == 1 => Some(item_tile(GOLD_INGOT)),
            MobKind::CopperGolem if self.seed != 0 => Some(item_tile((self.seed & 0xFFFF) as Id)),
            _ => None,
        };
        if let Some(tile) = carried {
            let held = [part([-0.55, 0.55, -0.55], [0.06, 0.45, 0.45], [0.0; 3], Limb::Fixed, [tile; 6])];
            draw_model(geo, &root, &held, 0.0, sky, false);
        }
        // A Sneaker with something in its mouth.
        if self.kind == MobKind::Sneaker && self.seed != 0 {
            let tile = item_tile(self.seed as Id);
            let held = [part([-0.08, 0.3, -0.76], [0.16, 0.16, 0.06], [0.0; 3], Limb::Fixed, [tile; 6])];
            draw_model(geo, &root, &held, 0.0, sky, false);
        }
    }
}

pub fn angle_diff(a: f32, b: f32) -> f32 {
    let mut d = (a - b) % std::f32::consts::TAU;
    if d > std::f32::consts::PI {
        d -= std::f32::consts::TAU;
    }
    if d < -std::f32::consts::PI {
        d += std::f32::consts::TAU;
    }
    d
}

#[derive(Clone, Copy)]
pub enum Limb {
    Fixed,
    Swing(f32),
    /// Arms held forward, groaner style.
    Forward,
    /// Sweeps side to side around the vertical axis (Webber legs).
    SwingY(f32),
    /// Flaps up and down around the body's length (the Wyrm's wings).
    Wing(f32),
    /// Held at a fixed lean (radians about x; negative leans forward).
    Tilt(f32),
    /// A bird's wing: out and back while it falls, a little sway as it walks.
    /// The sign says which side (-1 left, 1 right).
    Flap(f32),
    /// Bobs up and down with the walk (custom modded parts).
    Bob(f32),
    /// Turns round about the vertical axis as it moves (custom modded parts).
    Spin(f32),
}

#[derive(Clone, Copy)]
pub struct Part {
    pub min: [f32; 3],
    pub size: [f32; 3],
    pub pivot: [f32; 3],
    pub limb: Limb,
    /// +x, -x, +y, -y, +z, -z
    pub tiles: [u16; 6],
}

const fn part(min: [f32; 3], size: [f32; 3], pivot: [f32; 3], limb: Limb, tiles: [u16; 6]) -> Part {
    Part { min, size, pivot, limb, tiles }
}

const PS: u16 = T_PIG_SKIN;
const HS: u16 = T_HISSER_SKIN;
const GS: u16 = T_GROAN_SKIN;

static OINKER: [Part; 6] = [
    part([-0.3, 0.35, -0.45], [0.6, 0.5, 0.9], [0.0; 3], Limb::Fixed, [PS; 6]),
    part([-0.25, 0.5, -0.85], [0.5, 0.5, 0.45], [0.0; 3], Limb::Fixed, [PS, PS, PS, PS, PS, T_PIG_FACE]),
    part([-0.28, 0.0, -0.4], [0.2, 0.36, 0.2], [0.0, 0.36, -0.3], Limb::Swing(1.0), [PS; 6]),
    part([0.08, 0.0, -0.4], [0.2, 0.36, 0.2], [0.0, 0.36, -0.3], Limb::Swing(-1.0), [PS; 6]),
    part([-0.28, 0.0, 0.2], [0.2, 0.36, 0.2], [0.0, 0.36, 0.3], Limb::Swing(-1.0), [PS; 6]),
    part([0.08, 0.0, 0.2], [0.2, 0.36, 0.2], [0.0, 0.36, 0.3], Limb::Swing(1.0), [PS; 6]),
];

static HISSER: [Part; 6] = [
    part([-0.25, 0.4, -0.15], [0.5, 0.75, 0.3], [0.0; 3], Limb::Fixed, [HS; 6]),
    part([-0.25, 1.15, -0.25], [0.5, 0.5, 0.5], [0.0; 3], Limb::Fixed, [HS, HS, HS, HS, HS, T_HISSER_FACE]),
    part([-0.25, 0.0, -0.42], [0.24, 0.4, 0.26], [0.0, 0.4, -0.3], Limb::Swing(1.0), [HS; 6]),
    part([0.01, 0.0, -0.42], [0.24, 0.4, 0.26], [0.0, 0.4, -0.3], Limb::Swing(-1.0), [HS; 6]),
    part([-0.25, 0.0, 0.16], [0.24, 0.4, 0.26], [0.0, 0.4, 0.3], Limb::Swing(-1.0), [HS; 6]),
    part([0.01, 0.0, 0.16], [0.24, 0.4, 0.26], [0.0, 0.4, 0.3], Limb::Swing(1.0), [HS; 6]),
];

pub const fn humanoid(skin: u16, face: u16, shirt: u16, pants: u16, arms: Limb, arms2: Limb) -> [Part; 6] {
    [
        part([-0.25, 0.0, -0.125], [0.25, 0.75, 0.25], [0.0, 0.75, 0.0], Limb::Swing(1.0), [pants; 6]),
        part([0.0, 0.0, -0.125], [0.25, 0.75, 0.25], [0.0, 0.75, 0.0], Limb::Swing(-1.0), [pants; 6]),
        part([-0.25, 0.75, -0.125], [0.5, 0.75, 0.25], [0.0; 3], Limb::Fixed, [shirt; 6]),
        part([-0.5, 0.75, -0.125], [0.25, 0.75, 0.25], [0.0, 1.4, 0.0], arms, [skin, skin, shirt, skin, skin, skin]),
        part([0.25, 0.75, -0.125], [0.25, 0.75, 0.25], [0.0, 1.4, 0.0], arms2, [skin, skin, shirt, skin, skin, skin]),
        part([-0.25, 1.5, -0.25], [0.5, 0.5, 0.5], [0.0; 3], Limb::Fixed, [skin, skin, skin, skin, skin, face]),
    ]
}

static GROANER: [Part; 6] = humanoid(GS, T_GROAN_FACE, T_GROAN_SHIRT, T_GROAN_PANTS, Limb::Forward, Limb::Forward);

const WL: u16 = T_WOOL;
const FS: u16 = T_FLUFF_SKIN;
static FLUFFER: [Part; 6] = [
    part([-0.35, 0.45, -0.5], [0.7, 0.6, 1.0], [0.0; 3], Limb::Fixed, [WL; 6]),
    part([-0.22, 0.75, -0.85], [0.44, 0.45, 0.4], [0.0; 3], Limb::Fixed, [FS, FS, WL, FS, FS, T_FLUFF_FACE]),
    part([-0.3, 0.0, -0.4], [0.18, 0.5, 0.18], [0.0, 0.5, -0.3], Limb::Swing(1.0), [FS; 6]),
    part([0.12, 0.0, -0.4], [0.18, 0.5, 0.18], [0.0, 0.5, -0.3], Limb::Swing(-1.0), [FS; 6]),
    part([-0.3, 0.0, 0.22], [0.18, 0.5, 0.18], [0.0, 0.5, 0.3], Limb::Swing(-1.0), [FS; 6]),
    part([0.12, 0.0, 0.22], [0.18, 0.5, 0.18], [0.0, 0.5, 0.3], Limb::Swing(1.0), [FS; 6]),
];

const SS: u16 = T_STARER_SKIN;
static STARER: [Part; 6] = [
    part([-0.2, 0.0, -0.08], [0.16, 1.5, 0.16], [0.0, 1.5, 0.0], Limb::Swing(0.6), [SS; 6]),
    part([0.04, 0.0, -0.08], [0.16, 1.5, 0.16], [0.0, 1.5, 0.0], Limb::Swing(-0.6), [SS; 6]),
    part([-0.24, 1.5, -0.12], [0.48, 0.9, 0.24], [0.0; 3], Limb::Fixed, [SS; 6]),
    part([-0.4, 0.9, -0.08], [0.16, 1.45, 0.16], [0.0, 2.35, 0.0], Limb::Swing(-0.4), [SS; 6]),
    part([0.24, 0.9, -0.08], [0.16, 1.45, 0.16], [0.0, 2.35, 0.0], Limb::Swing(0.4), [SS; 6]),
    part([-0.24, 2.4, -0.24], [0.48, 0.48, 0.48], [0.0; 3], Limb::Fixed, [SS, SS, SS, SS, SS, T_STARER_FACE]),
];

const CB: u16 = T_CLUCK_BODY;
const CL: u16 = T_CLUCK_LEG;
static CLUCKSTER: [Part; 7] = [
    part([-0.19, 0.25, -0.25], [0.38, 0.35, 0.5], [0.0; 3], Limb::Fixed, [CB; 6]),
    part([-0.12, 0.45, -0.42], [0.24, 0.3, 0.2], [0.0; 3], Limb::Fixed, [CB, CB, CB, CB, CB, T_CLUCK_FACE]),
    part([-0.1, 0.0, -0.05], [0.06, 0.26, 0.06], [0.0, 0.26, 0.0], Limb::Swing(1.0), [CL; 6]),
    part([0.04, 0.0, -0.05], [0.06, 0.26, 0.06], [0.0, 0.26, 0.0], Limb::Swing(-1.0), [CL; 6]),
    // Wings (they flap when it walks, and it only really walks when fleeing).
    part([-0.24, 0.3, -0.2], [0.05, 0.22, 0.4], [-0.2, 0.52, 0.0], Limb::Flap(-1.0), [CB; 6]),
    part([0.19, 0.3, -0.2], [0.05, 0.22, 0.4], [0.2, 0.52, 0.0], Limb::Flap(1.0), [CB; 6]),
    part([-0.08, 0.5, 0.2], [0.16, 0.15, 0.1], [0.0; 3], Limb::Fixed, [CB; 6]),
];

// Upright, with a long tail and a big beak.
const SQ: u16 = T_SQUAWK;
static SQUAWKER: [Part; 7] = [
    part([-0.16, 0.25, -0.18], [0.32, 0.45, 0.36], [0.0; 3], Limb::Fixed, [SQ; 6]),
    part([-0.13, 0.66, -0.3], [0.26, 0.26, 0.26], [0.0; 3], Limb::Fixed, [SQ, SQ, SQ, SQ, SQ, T_SQUAWK_FACE]),
    part([-0.09, 0.0, -0.02], [0.06, 0.25, 0.06], [0.0, 0.25, 0.0], Limb::Swing(1.0), [CL; 6]),
    part([0.03, 0.0, -0.02], [0.06, 0.25, 0.06], [0.0, 0.25, 0.0], Limb::Swing(-1.0), [CL; 6]),
    part([-0.21, 0.3, -0.14], [0.05, 0.36, 0.3], [-0.18, 0.66, 0.0], Limb::Flap(-1.0), [T_SQUAWK_WING; 6]),
    part([0.16, 0.3, -0.14], [0.05, 0.36, 0.3], [0.18, 0.66, 0.0], Limb::Flap(1.0), [T_SQUAWK_WING; 6]),
    part([-0.07, 0.12, 0.16], [0.14, 0.2, 0.3], [0.0; 3], Limb::Fixed, [T_SQUAWK_WING; 6]),
];

// Broad shoulders, long arms, a small head and a big nose.
const CK: u16 = T_CLANK;
static CLANKER: [Part; 7] = [
    part([-0.42, 0.0, -0.2], [0.34, 1.0, 0.4], [0.0, 1.0, 0.0], Limb::Swing(0.6), [CK; 6]),
    part([0.08, 0.0, -0.2], [0.34, 1.0, 0.4], [0.0, 1.0, 0.0], Limb::Swing(-0.6), [CK; 6]),
    part([-0.66, 1.0, -0.32], [1.32, 1.2, 0.64], [0.0; 3], Limb::Fixed, [CK; 6]),
    part([-0.96, 0.55, -0.2], [0.3, 1.6, 0.4], [0.0, 2.15, 0.0], Limb::Swing(-0.5), [CK; 6]),
    part([0.66, 0.55, -0.2], [0.3, 1.6, 0.4], [0.0, 2.15, 0.0], Limb::Swing(0.5), [CK; 6]),
    part([-0.28, 2.2, -0.36], [0.56, 0.5, 0.56], [0.0; 3], Limb::Fixed, [CK, CK, CK, CK, CK, T_CLANK_FACE]),
    part([-0.07, 2.2, -0.5], [0.14, 0.3, 0.14], [0.0; 3], Limb::Fixed, [T_CLANK_FACE; 6]),
];

const MS: u16 = T_MOO_SKIN;
static MOOER: [Part; 8] = [
    part([-0.38, 0.62, -0.62], [0.76, 0.66, 1.24], [0.0; 3], Limb::Fixed, [MS; 6]),
    part([-0.26, 0.86, -1.0], [0.52, 0.5, 0.4], [0.0; 3], Limb::Fixed, [MS, MS, MS, MS, MS, T_MOO_FACE]),
    part([-0.34, 1.3, -0.86], [0.1, 0.12, 0.08], [0.0; 3], Limb::Fixed, [T_BONE; 6]),
    part([0.24, 1.3, -0.86], [0.1, 0.12, 0.08], [0.0; 3], Limb::Fixed, [T_BONE; 6]),
    part([-0.36, 0.0, -0.5], [0.22, 0.62, 0.22], [0.0, 0.62, -0.4], Limb::Swing(1.0), [MS; 6]),
    part([0.14, 0.0, -0.5], [0.22, 0.62, 0.22], [0.0, 0.62, -0.4], Limb::Swing(-1.0), [MS; 6]),
    part([-0.36, 0.0, 0.28], [0.22, 0.62, 0.22], [0.0, 0.62, 0.4], Limb::Swing(-1.0), [MS; 6]),
    part([0.14, 0.0, 0.28], [0.22, 0.62, 0.22], [0.0, 0.62, 0.4], Limb::Swing(1.0), [MS; 6]),
];

// A fish: a long body, a tail that waggles, a fin on top.
static FISHY: [Part; 3] = [
    part([-0.12, 0.05, -0.3], [0.24, 0.3, 0.55], [0.0; 3], Limb::Fixed, [T_FISHY, T_FISHY, T_FISHY, T_FISHY, T_FISHY, T_FISHY_FACE]),
    part([-0.02, 0.06, 0.25], [0.04, 0.28, 0.22], [0.0, 0.2, 0.25], Limb::SwingY(2.0), [T_FISHY_FIN; 6]),
    part([-0.02, 0.35, -0.12], [0.04, 0.1, 0.24], [0.0; 3], Limb::Fixed, [T_FISHY_FIN; 6]),
];

static SOGGY: [Part; 6] = humanoid(T_SOGGY_SKIN, T_SOGGY_FACE, T_SOGGY_SHIRT, T_SOGGY_PANTS, Limb::Forward, Limb::Forward);
/// With a spear in its right hand.
static SOGGY_ARMED: [Part; 7] = {
    let h = SOGGY;
    [h[0], h[1], h[2], h[3], h[4], h[5], part([0.34, -0.2, -0.03], [0.06, 1.7, 0.06], [0.0, 1.4, 0.0], Limb::Forward, [T_COPPER + 3; 6])]
};

static RATTLER: [Part; 6] = humanoid(T_BONE, T_RATTLER_FACE, T_BONE, T_BONE, Limb::Forward, Limb::Forward);

static GRUMBLER: [Part; 6] = humanoid(T_GRUMBLE_SKIN, T_GRUMBLE_FACE, T_GRUMBLE_SKIN, T_GOLD, Limb::Swing(-0.8), Limb::Swing(0.8));

/// A robe, arms folded, and a nose that means business.
static HMMER: [Part; 7] = {
    let h = humanoid(T_SKIN, T_HMM_FACE, T_HMM_ROBE, T_HMM_ROBE, Limb::Fixed, Limb::Fixed);
    [
        h[0],
        h[1],
        h[2],
        part([-0.3, 1.0, -0.3], [0.6, 0.22, 0.25], [0.0; 3], Limb::Fixed, [T_HMM_ROBE; 6]),
        part([-0.25, 0.3, -0.13], [0.5, 0.5, 0.26], [0.0; 3], Limb::Fixed, [T_HMM_ROBE; 6]),
        h[5],
        part([-0.06, 1.55, -0.38], [0.12, 0.22, 0.14], [0.0; 3], Limb::Fixed, [T_SKIN; 6]),
    ]
};

const WS: u16 = T_WEB_SKIN;
const fn web_leg(side: f32, z: f32, phase: f32) -> Part {
    let x = if side < 0.0 { -1.05 } else { 0.35 };
    part([x, 0.34, z], [0.7, 0.09, 0.09], [side * 0.35, 0.38, z], Limb::SwingY(phase), [WS; 6])
}
static WEBBER: [Part; 10] = [
    part([-0.42, 0.25, -0.05], [0.84, 0.5, 0.85], [0.0; 3], Limb::Fixed, [WS; 6]),
    part([-0.3, 0.22, -0.6], [0.6, 0.42, 0.56], [0.0; 3], Limb::Fixed, [WS, WS, WS, WS, WS, T_WEB_FACE]),
    web_leg(-1.0, -0.45, 1.0),
    web_leg(-1.0, -0.2, -1.0),
    web_leg(-1.0, 0.05, 1.0),
    web_leg(-1.0, 0.3, -1.0),
    web_leg(1.0, -0.45, -1.0),
    web_leg(1.0, -0.2, 1.0),
    web_leg(1.0, 0.05, -1.0),
    web_leg(1.0, 0.3, 1.0),
];

const BL: u16 = T_BLOOP;
static BLOOP: [Part; 1] = [part([-0.26, 0.0, -0.26], [0.52, 0.52, 0.52], [0.0; 3], Limb::Fixed, [BL, BL, BL, BL, BL, T_BLOOP_FACE])];

const WF: u16 = T_WOOF_SKIN;
static WOOFER: [Part; 8] = [
    part([-0.2, 0.42, -0.4], [0.4, 0.32, 0.8], [0.0; 3], Limb::Fixed, [WF; 6]),
    part([-0.2, 0.55, -0.72], [0.4, 0.36, 0.34], [0.0; 3], Limb::Fixed, [WF, WF, WF, WF, WF, T_WOOF_FACE]),
    part([-0.08, 0.55, -0.9], [0.16, 0.14, 0.2], [0.0; 3], Limb::Fixed, [WF, WF, WF, WF, WF, T_WOOF_NOSE]),
    part([-0.18, 0.0, -0.36], [0.12, 0.44, 0.12], [0.0, 0.44, -0.3], Limb::Swing(1.0), [WF; 6]),
    part([0.06, 0.0, -0.36], [0.12, 0.44, 0.12], [0.0, 0.44, -0.3], Limb::Swing(-1.0), [WF; 6]),
    part([-0.18, 0.0, 0.24], [0.12, 0.44, 0.12], [0.0, 0.44, 0.3], Limb::Swing(-1.0), [WF; 6]),
    part([0.06, 0.0, 0.24], [0.12, 0.44, 0.12], [0.0, 0.44, 0.3], Limb::Swing(1.0), [WF; 6]),
    part([-0.05, 0.55, 0.38], [0.1, 0.1, 0.4], [0.0, 0.6, 0.38], Limb::SwingY(1.5), [WF; 6]),
];
const WY: u16 = T_WYRM_SKIN;
static WYRM: [Part; 8] = [
    // A long body, neck and head, two great wings, and a tail.
    part([-0.8, 1.0, -2.0], [1.6, 1.0, 4.0], [0.0; 3], Limb::Fixed, [WY; 6]),
    part([-0.35, 1.3, -3.6], [0.7, 0.6, 1.7], [0.0; 3], Limb::Fixed, [WY; 6]),
    part([-0.55, 1.2, -4.8], [1.1, 0.8, 1.3], [0.0; 3], Limb::Fixed, [WY, WY, WY, WY, WY, T_WYRM_FACE]),
    part([-4.8, 1.8, -1.2], [4.0, 0.15, 2.4], [-0.8, 1.9, 0.0], Limb::Wing(1.0), [T_WYRM_WING; 6]),
    part([0.8, 1.8, -1.2], [4.0, 0.15, 2.4], [0.8, 1.9, 0.0], Limb::Wing(-1.0), [T_WYRM_WING; 6]),
    part([-0.3, 1.2, 2.0], [0.6, 0.5, 2.5], [0.0, 1.45, 2.0], Limb::SwingY(0.6), [WY; 6]),
    part([-0.8, 0.2, -1.2], [0.4, 0.8, 0.4], [0.0; 3], Limb::Fixed, [WY; 6]),
    part([0.4, 0.2, -1.2], [0.4, 0.8, 0.4], [0.0; 3], Limb::Fixed, [WY; 6]),
];
const GL: u16 = T_GALLOPER;
/// Where a Galloper's neck and head bend from, and by how much.
const NECK: [f32; 3] = [0.0, 1.25, -0.77];
const HEAD: [f32; 3] = [0.0, 1.71, -1.05];
const GALLOPER_PARTS: [Part; 13] = [
    // A long body on four long legs, and a tail.
    part([-0.3, 0.85, -0.75], [0.6, 0.6, 1.5], [0.0; 3], Limb::Fixed, [GL; 6]),
    part([-0.28, 0.0, -0.72], [0.16, 0.85, 0.16], [0.0, 0.85, -0.64], Limb::Swing(1.0), [GL; 6]),
    part([0.12, 0.0, -0.72], [0.16, 0.85, 0.16], [0.0, 0.85, -0.64], Limb::Swing(-1.0), [GL; 6]),
    part([-0.28, 0.0, 0.5], [0.16, 0.85, 0.16], [0.0, 0.85, 0.58], Limb::Swing(-1.0), [GL; 6]),
    part([0.12, 0.0, 0.5], [0.16, 0.85, 0.16], [0.0, 0.85, 0.58], Limb::Swing(1.0), [GL; 6]),
    part([-0.06, 0.8, 0.72], [0.12, 0.6, 0.12], [0.0, 1.4, 0.75], Limb::SwingY(1.0), [T_GALLOP_MANE; 6]),
    // The neck leans forward, with the mane down its back.
    part([-0.14, 1.1, -0.98], [0.28, 0.8, 0.42], NECK, Limb::Tilt(-0.45), [GL; 6]),
    part([-0.05, 1.3, -0.6], [0.1, 0.7, 0.1], NECK, Limb::Tilt(-0.45), [T_GALLOP_MANE; 6]),
    // A long head, nose down: eyes on the sides, a narrower muzzle, two ears.
    part([-0.15, 1.55, -1.4], [0.3, 0.32, 0.36], HEAD, Limb::Tilt(-0.5), [T_GALLOP_EYE, T_GALLOP_EYE, GL, GL, GL, GL]),
    part([-0.12, 1.55, -1.75], [0.24, 0.26, 0.36], HEAD, Limb::Tilt(-0.5), [GL, GL, GL, GL, GL, T_GALLOP_FACE]),
    part([-0.13, 1.86, -1.12], [0.07, 0.13, 0.06], HEAD, Limb::Tilt(-0.5), [GL; 6]),
    part([0.06, 1.86, -1.12], [0.07, 0.13, 0.06], HEAD, Limb::Tilt(-0.5), [GL; 6]),
    part([-0.16, 1.62, -1.3], [0.32, 0.06, 0.06], HEAD, Limb::Tilt(-0.5), [T_GALLOP_MANE; 6]),
];
/// A saddle on a Galloper's back.
static GALLOPER: [Part; 13] = GALLOPER_PARTS;
/// The same shape in other tiles (`map`: (from, to) pairs).
const fn retile<const N: usize>(mut parts: [Part; N], map: [(u16, u16); 4]) -> [Part; N] {
    let mut i = 0;
    while i < N {
        let mut f = 0;
        while f < 6 {
            let mut k = 0;
            while k < 4 {
                if parts[i].tiles[f] == map[k].0 {
                    parts[i].tiles[f] = map[k].1;
                    k = 4;
                } else {
                    k += 1;
                }
            }
            f += 1;
        }
        i += 1;
    }
    parts
}
static ROTSTEED: [Part; 13] = retile(GALLOPER_PARTS, [(GL, T_ROTSTEED), (T_GALLOP_MANE, T_ROTSTEED_FACE), (T_GALLOP_EYE, T_ROTSTEED_FACE), (T_GALLOP_FACE, T_ROTSTEED_FACE)]);
const CG: u16 = T_COPPER_GOLEM;
static COPPER_GOLEM: [Part; 9] = [
    part([-0.2, 0.0, -0.08], [0.15, 0.3, 0.16], [0.0, 0.3, 0.0], Limb::Swing(1.0), [CG; 6]),
    part([0.05, 0.0, -0.08], [0.15, 0.3, 0.16], [0.0, 0.3, 0.0], Limb::Swing(-1.0), [CG; 6]),
    part([-0.25, 0.3, -0.15], [0.5, 0.36, 0.3], [0.0; 3], Limb::Fixed, [CG; 6]),
    part([-0.37, 0.3, -0.07], [0.12, 0.34, 0.14], [0.0, 0.64, 0.0], Limb::Swing(-0.7), [CG; 6]),
    part([0.25, 0.3, -0.07], [0.12, 0.34, 0.14], [0.0, 0.64, 0.0], Limb::Swing(0.7), [CG; 6]),
    part([-0.28, 0.66, -0.25], [0.56, 0.32, 0.5], [0.0; 3], Limb::Fixed, [CG, CG, CG, CG, CG, T_COPPER_GOLEM_FACE]),
    part([-0.05, 0.7, -0.33], [0.1, 0.16, 0.08], [0.0; 3], Limb::Fixed, [CG; 6]),
    // A lightning rod on its head.
    part([-0.03, 0.98, -0.03], [0.06, 0.14, 0.06], [0.0; 3], Limb::Fixed, [CG; 6]),
    part([-0.06, 1.1, -0.06], [0.12, 0.12, 0.12], [0.0, 1.0, 0.0], Limb::Bob(1.0), [CG; 6]),
];
const FL: u16 = T_FLOATY;
static FLOATY: [Part; 6] = [
    part([-1.9, 0.6, -1.9], [3.8, 3.2, 3.8], [0.0; 3], Limb::Fixed, [FL, FL, FL, FL, FL, T_FLOATY_FACE]),
    // Short tentacles, swaying.
    part([-1.2, 0.0, -1.2], [0.3, 0.7, 0.3], [0.0, 0.6, -1.05], Limb::Swing(0.4), [FL; 6]),
    part([0.9, 0.0, -1.2], [0.3, 0.7, 0.3], [0.0, 0.6, -1.05], Limb::Swing(-0.4), [FL; 6]),
    part([-1.2, 0.0, 0.9], [0.3, 0.7, 0.3], [0.0, 0.6, 1.05], Limb::Swing(-0.4), [FL; 6]),
    part([0.9, 0.0, 0.9], [0.3, 0.7, 0.3], [0.0, 0.6, 1.05], Limb::Swing(0.4), [FL; 6]),
    part([-0.15, 0.1, -0.15], [0.3, 0.6, 0.3], [0.0, 0.6, 0.0], Limb::Swing(0.6), [FL; 6]),
];
/// A harness on top, with room for four.
static FLOATY_HARNESS: [Part; 3] = [
    part([-1.95, 3.75, -1.95], [3.9, 0.12, 3.9], [0.0; 3], Limb::Fixed, [T_HARNESS_WORN; 6]),
    part([-1.95, 2.2, -1.95], [3.9, 0.25, 0.06], [0.0; 3], Limb::Fixed, [T_HARNESS_WORN; 6]),
    part([-1.95, 2.2, 1.89], [3.9, 0.25, 0.06], [0.0; 3], Limb::Fixed, [T_HARNESS_WORN; 6]),
];
pub static SADDLE_PART: [Part; 1] = [part([-0.32, 1.44, -0.3], [0.64, 0.1, 0.5], [0.0; 3], Limb::Fixed, [T_SADDLE_LEATHER; 6])];
static WOOFER_COLLAR: [Part; 1] = [part([-0.21, 0.52, -0.46], [0.42, 0.1, 0.08], [0.0; 3], Limb::Fixed, [T_COLLAR; 6])];
static FLUFFER_SHEARED: [Part; 6] = [
    part([-0.3, 0.5, -0.45], [0.6, 0.5, 0.9], [0.0; 3], Limb::Fixed, [FS; 6]),
    part([-0.22, 0.75, -0.85], [0.44, 0.45, 0.4], [0.0; 3], Limb::Fixed, [FS, FS, FS, FS, FS, T_FLUFF_FACE]),
    part([-0.3, 0.0, -0.4], [0.18, 0.5, 0.18], [0.0, 0.5, -0.3], Limb::Swing(1.0), [FS; 6]),
    part([0.12, 0.0, -0.4], [0.18, 0.5, 0.18], [0.0, 0.5, -0.3], Limb::Swing(-1.0), [FS; 6]),
    part([-0.3, 0.0, 0.22], [0.18, 0.5, 0.18], [0.0, 0.5, 0.3], Limb::Swing(-1.0), [FS; 6]),
    part([0.12, 0.0, 0.22], [0.18, 0.5, 0.18], [0.0, 0.5, 0.3], Limb::Swing(1.0), [FS; 6]),
];


static BEE: [Part; 4] = [
    part([-0.12, 0.08, -0.16], [0.24, 0.22, 0.32], [0.0; 3], Limb::Fixed, [T_BEE, T_BEE, T_BEE, T_BEE, T_BEE, T_BEE_FACE]),
    part([-0.02, 0.13, 0.16], [0.04, 0.04, 0.06], [0.0; 3], Limb::Fixed, [T_HUSH; 6]),
    part([-0.22, 0.3, -0.06], [0.2, 0.01, 0.14], [-0.02, 0.3, 0.0], Limb::Wing(1.0), [T_BEE_WING; 6]),
    part([0.02, 0.3, -0.06], [0.2, 0.01, 0.14], [0.02, 0.3, 0.0], Limb::Wing(-1.0), [T_BEE_WING; 6]),
];

/// For drawing the bees that buzz about for show (see bees.rs).
pub fn draw_bee(geo: &mut DynGeo, pos: Vec3, yaw: f32, phase: f32, sky: f32) {
    let root = Mat4::from_translation(pos - Vec3::Y * 0.2) * Mat4::from_rotation_y(-yaw) * Mat4::from_scale(Vec3::splat(0.6));
    draw_posed(geo, &root, &BEE, phase * 3.0, 0.0, sky);
}

const FX: u16 = T_FOX;
static SNEAKER: [Part; 10] = [
    part([-0.16, 0.24, -0.3], [0.32, 0.28, 0.6], [0.0; 3], Limb::Fixed, [FX; 6]),
    part([-0.16, 0.36, -0.56], [0.32, 0.26, 0.26], [0.0; 3], Limb::Fixed, [FX, FX, FX, FX, FX, T_FOX_FACE]),
    part([-0.07, 0.37, -0.68], [0.14, 0.1, 0.12], [0.0; 3], Limb::Fixed, [T_FOX_TAIL; 6]),
    part([-0.15, 0.62, -0.5], [0.08, 0.12, 0.05], [0.0; 3], Limb::Fixed, [T_FOX_DARK; 6]),
    part([0.07, 0.62, -0.5], [0.08, 0.12, 0.05], [0.0; 3], Limb::Fixed, [T_FOX_DARK; 6]),
    part([-0.14, 0.0, -0.26], [0.08, 0.25, 0.08], [0.0, 0.25, -0.22], Limb::Swing(1.0), [T_FOX_DARK; 6]),
    part([0.06, 0.0, -0.26], [0.08, 0.25, 0.08], [0.0, 0.25, -0.22], Limb::Swing(-1.0), [T_FOX_DARK; 6]),
    part([-0.14, 0.0, 0.18], [0.08, 0.25, 0.08], [0.0, 0.25, 0.22], Limb::Swing(-1.0), [T_FOX_DARK; 6]),
    part([0.06, 0.0, 0.18], [0.08, 0.25, 0.08], [0.0, 0.25, 0.22], Limb::Swing(1.0), [T_FOX_DARK; 6]),
    part([-0.09, 0.26, 0.28], [0.18, 0.18, 0.42], [0.0, 0.36, 0.3], Limb::Tilt(-0.35), [T_FOX_TAIL; 6]),
];

const FR: u16 = T_FROG;
static RIBBIT: [Part; 7] = [
    part([-0.22, 0.08, -0.24], [0.44, 0.26, 0.46], [0.0; 3], Limb::Fixed, [FR, FR, FR, FR, FR, T_FROG_FACE]),
    part([-0.22, 0.34, -0.24], [0.44, 0.08, 0.28], [0.0; 3], Limb::Fixed, [FR; 6]),
    part([-0.21, 0.38, -0.22], [0.13, 0.1, 0.12], [0.0; 3], Limb::Fixed, [T_FROG_EYE; 6]),
    part([0.08, 0.38, -0.22], [0.13, 0.1, 0.12], [0.0; 3], Limb::Fixed, [T_FROG_EYE; 6]),
    part([-0.26, 0.0, 0.0], [0.12, 0.12, 0.26], [0.0, 0.1, 0.0], Limb::Swing(0.6), [FR; 6]),
    part([0.14, 0.0, 0.0], [0.12, 0.12, 0.26], [0.0, 0.1, 0.0], Limb::Swing(0.6), [FR; 6]),
    part([-0.18, 0.0, -0.22], [0.36, 0.08, 0.1], [0.0; 3], Limb::Fixed, [FR; 6]),
];

static ROLLO: [Part; 9] = [
    part([-0.25, 0.14, -0.3], [0.5, 0.34, 0.6], [0.0; 3], Limb::Fixed, [T_SHELL; 6]),
    part([-0.08, 0.14, -0.48], [0.16, 0.18, 0.2], [0.0; 3], Limb::Fixed, [T_ROLLO_SKIN, T_ROLLO_SKIN, T_ROLLO_SKIN, T_ROLLO_SKIN, T_ROLLO_SKIN, T_ROLLO_FACE]),
    part([-0.09, 0.3, -0.42], [0.05, 0.12, 0.04], [0.0; 3], Limb::Fixed, [T_ROLLO_SKIN; 6]),
    part([0.04, 0.3, -0.42], [0.05, 0.12, 0.04], [0.0; 3], Limb::Fixed, [T_ROLLO_SKIN; 6]),
    part([-0.2, 0.0, -0.24], [0.1, 0.15, 0.1], [0.0, 0.15, -0.2], Limb::Swing(1.0), [T_ROLLO_SKIN; 6]),
    part([0.1, 0.0, -0.24], [0.1, 0.15, 0.1], [0.0, 0.15, -0.2], Limb::Swing(-1.0), [T_ROLLO_SKIN; 6]),
    part([-0.2, 0.0, 0.16], [0.1, 0.15, 0.1], [0.0, 0.15, 0.2], Limb::Swing(-1.0), [T_ROLLO_SKIN; 6]),
    part([0.1, 0.0, 0.16], [0.1, 0.15, 0.1], [0.0, 0.15, 0.2], Limb::Swing(1.0), [T_ROLLO_SKIN; 6]),
    part([-0.04, 0.14, 0.28], [0.08, 0.08, 0.22], [0.0; 3], Limb::Fixed, [T_SHELL; 6]),
];
/// A Rollo rolled up: all shell.
static ROLLO_BALL: [Part; 1] = [part([-0.24, 0.0, -0.26], [0.48, 0.46, 0.52], [0.0; 3], Limb::Fixed, [T_SHELL; 6])];

const HU: u16 = T_HUSH;
static HUSH: [Part; 9] = [
    part([-0.48, 1.15, -0.3], [0.96, 1.1, 0.6], [0.0; 3], Limb::Fixed, [HU; 6]),
    // Souls in its chest, glowing.
    part([-0.3, 1.45, -0.32], [0.6, 0.55, 0.04], [0.0; 3], Limb::Fixed, [T_HUSH_GLOW; 6]),
    part([-0.4, 2.25, -0.32], [0.8, 0.62, 0.62], [0.0; 3], Limb::Fixed, [HU, HU, HU, HU, HU, T_HUSH_FACE]),
    // Antler-ish feelers it "hears" with.
    part([-0.62, 2.55, -0.04], [0.24, 0.34, 0.06], [-0.4, 2.55, 0.0], Limb::Wing(0.3), [T_HUSH_GLOW; 6]),
    part([0.38, 2.55, -0.04], [0.24, 0.34, 0.06], [0.4, 2.55, 0.0], Limb::Wing(-0.3), [T_HUSH_GLOW; 6]),
    part([-0.8, 0.95, -0.17], [0.32, 1.35, 0.34], [-0.64, 2.2, 0.0], Limb::Swing(-0.8), [HU; 6]),
    part([0.48, 0.95, -0.17], [0.32, 1.35, 0.34], [0.64, 2.2, 0.0], Limb::Swing(0.8), [HU; 6]),
    part([-0.42, 0.0, -0.17], [0.34, 1.15, 0.34], [0.0, 1.15, 0.0], Limb::Swing(1.0), [HU; 6]),
    part([0.08, 0.0, -0.17], [0.34, 1.15, 0.34], [0.0, 1.15, 0.0], Limb::Swing(-1.0), [HU; 6]),
];
static SIZZLER: [Part; 9] = [
    part([-0.25, 1.3, -0.25], [0.5, 0.5, 0.5], [0.0; 3], Limb::Fixed, [T_SIZZLER, T_SIZZLER, T_SIZZLER, T_SIZZLER, T_SIZZLER, T_SIZZLER_FACE]),
    part([-0.5, 0.75, -0.06], [0.12, 0.5, 0.12], [0.0, 1.0, 0.0], Limb::SwingY(0.6), [T_SIZZLER_ROD; 6]),
    part([0.38, 0.75, -0.06], [0.12, 0.5, 0.12], [0.0, 1.0, 0.0], Limb::SwingY(0.6), [T_SIZZLER_ROD; 6]),
    part([-0.06, 0.75, -0.5], [0.12, 0.5, 0.12], [0.0, 1.0, 0.0], Limb::SwingY(0.6), [T_SIZZLER_ROD; 6]),
    part([-0.06, 0.75, 0.38], [0.12, 0.5, 0.12], [0.0, 1.0, 0.0], Limb::SwingY(0.6), [T_SIZZLER_ROD; 6]),
    part([-0.32, 0.2, -0.32], [0.12, 0.5, 0.12], [0.0, 0.45, 0.0], Limb::SwingY(-0.6), [T_SIZZLER_ROD; 6]),
    part([0.2, 0.2, -0.32], [0.12, 0.5, 0.12], [0.0, 0.45, 0.0], Limb::SwingY(-0.6), [T_SIZZLER_ROD; 6]),
    part([-0.32, 0.2, 0.2], [0.12, 0.5, 0.12], [0.0, 0.45, 0.0], Limb::SwingY(-0.6), [T_SIZZLER_ROD; 6]),
    part([0.2, 0.2, 0.2], [0.12, 0.5, 0.12], [0.0, 0.45, 0.0], Limb::SwingY(-0.6), [T_SIZZLER_ROD; 6]),
];

const WP: u16 = T_WEEPER;
const fn weeper(face: u16) -> [Part; 10] {
    [
        part([-2.0, 0.0, -2.0], [4.0, 4.0, 4.0], [0.0; 3], Limb::Fixed, [WP, WP, WP, WP, WP, face]),
        part([-1.6, -1.8, -1.2], [0.3, 1.8, 0.3], [0.0, 0.0, 0.0], Limb::Swing(0.25), [WP; 6]),
        part([-0.6, -2.4, -1.4], [0.3, 2.4, 0.3], [0.0, 0.0, 0.0], Limb::Swing(-0.25), [WP; 6]),
        part([0.4, -1.6, -1.3], [0.3, 1.6, 0.3], [0.0, 0.0, 0.0], Limb::Swing(0.25), [WP; 6]),
        part([1.3, -2.2, -1.2], [0.3, 2.2, 0.3], [0.0, 0.0, 0.0], Limb::Swing(-0.25), [WP; 6]),
        part([-1.2, -2.0, 0.0], [0.3, 2.0, 0.3], [0.0, 0.0, 0.0], Limb::Swing(-0.25), [WP; 6]),
        part([0.0, -2.6, 0.2], [0.3, 2.6, 0.3], [0.0, 0.0, 0.0], Limb::Swing(0.25), [WP; 6]),
        part([1.1, -1.8, 0.4], [0.3, 1.8, 0.3], [0.0, 0.0, 0.0], Limb::Swing(-0.25), [WP; 6]),
        part([-0.8, -1.6, 1.3], [0.3, 1.6, 0.3], [0.0, 0.0, 0.0], Limb::Swing(0.25), [WP; 6]),
        part([0.6, -2.2, 1.3], [0.3, 2.2, 0.3], [0.0, 0.0, 0.0], Limb::Swing(-0.25), [WP; 6]),
    ]
}
static WEEPER: [Part; 10] = weeper(T_WEEPER_FACE);
static WEEPER_ANGRY: [Part; 10] = weeper(T_WEEPER_ANGRY);

const fn strutter(skin: u16) -> [Part; 6] {
    [
        part([-0.42, 0.85, -0.42], [0.84, 0.75, 0.84], [0.0; 3], Limb::Fixed, [skin, skin, skin, skin, skin, T_STRUTTER_FACE]),
        part([-0.35, 1.6, -0.1], [0.08, 0.3, 0.08], [0.0, 1.6, 0.0], Limb::Wing(0.4), [skin; 6]),
        part([-0.05, 1.6, 0.05], [0.08, 0.38, 0.08], [0.0, 1.6, 0.0], Limb::Wing(-0.4), [skin; 6]),
        part([0.25, 1.6, -0.2], [0.08, 0.26, 0.08], [0.0, 1.6, 0.0], Limb::Wing(0.4), [skin; 6]),
        part([-0.3, 0.0, -0.09], [0.18, 0.9, 0.18], [0.0, 0.9, 0.0], Limb::Swing(1.0), [skin; 6]),
        part([0.12, 0.0, -0.09], [0.18, 0.9, 0.18], [0.0, 0.9, 0.0], Limb::Swing(-1.0), [skin; 6]),
    ]
}
static STRUTTER: [Part; 6] = strutter(T_STRUTTER);
static STRUTTER_COLD: [Part; 6] = strutter(T_STRUTTER_COLD);

/// A snout of its own, and gold-trimmed clothes.
static SNOUT: [Part; 7] = {
    let h = humanoid(T_SNOUT, T_SNOUT_FACE, T_SNOUT_TUNIC, T_SNOUT_TUNIC, Limb::Swing(-0.8), Limb::Swing(0.8));
    [h[0], h[1], h[2], h[3], h[4], h[5], part([-0.12, 1.55, -0.34], [0.24, 0.16, 0.1], [0.0; 3], Limb::Fixed, [T_SNOUT; 6])]
};

/// Illagers: grey skin, a long nose and a scowl, in a coat to suit the job.
const fn illager(coat: u16, arms: Limb) -> [Part; 7] {
    let h = humanoid(T_ILLAGER, T_ILLAGER_FACE, coat, coat, arms, arms);
    [h[0], h[1], h[2], h[3], h[4], h[5], part([-0.06, 1.55, -0.38], [0.12, 0.22, 0.14], [0.0; 3], Limb::Fixed, [T_ILLAGER; 6])]
}
static PILFERER: [Part; 7] = illager(T_PILFERER_COAT, Limb::Forward);
static HACKLER: [Part; 7] = illager(T_HACKLER_COAT, Limb::Swing(-1.0));
static INVOICER: [Part; 7] = illager(T_INVOICER_ROBE, Limb::Fixed);
/// The Invoicer casting: arms up.
static INVOICER_CASTING: [Part; 7] = illager(T_INVOICER_ROBE, Limb::Tilt(-2.8));
static FEE: [Part; 4] = [
    part([-0.12, 0.3, -0.1], [0.24, 0.3, 0.2], [0.0; 3], Limb::Fixed, [T_FEE; 6]),
    part([-0.14, 0.58, -0.14], [0.28, 0.24, 0.28], [0.0; 3], Limb::Fixed, [T_FEE; 6]),
    part([-0.45, 0.4, 0.05], [0.33, 0.25, 0.03], [-0.12, 0.5, 0.06], Limb::Wing(0.8), [T_FEE; 6]),
    part([0.12, 0.4, 0.05], [0.33, 0.25, 0.03], [0.12, 0.5, 0.06], Limb::Wing(-0.8), [T_FEE; 6]),
];
const RG: u16 = T_RAMPAGER;
static RAMPAGER: [Part; 7] = [
    part([-0.7, 0.9, -0.9], [1.4, 1.1, 1.9], [0.0; 3], Limb::Fixed, [RG; 6]),
    part([-0.45, 1.1, -1.5], [0.9, 0.9, 0.7], [0.0; 3], Limb::Fixed, [RG, RG, RG, RG, RG, T_RAMPAGER_FACE]),
    part([-0.6, 1.8, -1.35], [0.15, 0.4, 0.15], [0.0; 3], Limb::Fixed, [T_BONE; 6]),
    part([0.45, 1.8, -1.35], [0.15, 0.4, 0.15], [0.0; 3], Limb::Fixed, [T_BONE; 6]),
    part([-0.6, 0.0, -0.75], [0.4, 0.95, 0.4], [0.0, 0.95, -0.6], Limb::Swing(1.0), [RG; 6]),
    part([0.2, 0.0, -0.75], [0.4, 0.95, 0.4], [0.0, 0.95, -0.6], Limb::Swing(-1.0), [RG; 6]),
    part([-0.2, 0.0, 0.4], [0.4, 0.95, 0.4], [0.0, 0.95, 0.6], Limb::Swing(-1.0), [RG; 6]),
];
/// A captain's banner, up on its back.
static BANNER_BACK: [Part; 2] = [
    part([-0.03, 1.2, 0.15], [0.06, 1.4, 0.06], [0.0; 3], Limb::Fixed, [T_PLANKS; 6]),
    part([-0.3, 1.6, 0.2], [0.6, 0.9, 0.03], [0.0; 3], Limb::Fixed, [T_BANNER_WORN; 6]),
];
/// Plates of scute over a Woofer (see critters.rs).
static WOOFER_ARMOUR: [Part; 2] = [
    part([-0.24, 0.42, -0.42], [0.48, 0.3, 0.84], [0.0; 3], Limb::Fixed, [T_WOLF_ARMOR_WORN; 6]),
    part([-0.2, 0.55, -0.62], [0.4, 0.28, 0.24], [0.0; 3], Limb::Fixed, [T_WOLF_ARMOR_WORN; 6]),
];

const BZ: u16 = T_BREEZE;
// A Breeze: a head over a whirl of wind, three rods spinning round it.
static BREEZE: [Part; 6] = [
    part([-0.25, 1.05, -0.25], [0.5, 0.5, 0.5], [0.0; 3], Limb::Fixed, [BZ, BZ, BZ, BZ, BZ, T_BREEZE_FACE]),
    part([-0.5, 0.75, -0.05], [0.1, 0.6, 0.1], [0.0; 3], Limb::Spin(3.0), [T_BREEZE_ROD; 6]),
    part([0.4, 0.55, -0.05], [0.1, 0.6, 0.1], [0.0; 3], Limb::Spin(3.0), [T_BREEZE_ROD; 6]),
    part([-0.05, 0.65, 0.4], [0.1, 0.6, 0.1], [0.0; 3], Limb::Spin(3.0), [T_BREEZE_ROD; 6]),
    part([-0.28, 0.45, -0.28], [0.56, 0.45, 0.56], [0.0; 3], Limb::Spin(-5.0), [BZ; 6]),
    part([-0.16, 0.0, -0.16], [0.32, 0.45, 0.32], [0.0; 3], Limb::Spin(7.0), [BZ; 6]),
];
const GT: u16 = T_GOAT;
static GOAT: [Part; 10] = [
    part([-0.25, 0.6, -0.45], [0.5, 0.5, 0.9], [0.0; 3], Limb::Fixed, [GT; 6]),
    part([-0.17, 0.85, -0.82], [0.34, 0.36, 0.42], [0.0; 3], Limb::Fixed, [GT, GT, GT, GT, GT, T_GOAT_FACE]),
    // Beard and horns.
    part([-0.05, 0.68, -0.8], [0.1, 0.2, 0.1], [0.0; 3], Limb::Fixed, [GT; 6]),
    part([-0.15, 1.2, -0.62], [0.08, 0.28, 0.08], [0.0; 3], Limb::Fixed, [T_BONE; 6]),
    part([0.07, 1.2, -0.62], [0.08, 0.28, 0.08], [0.0; 3], Limb::Fixed, [T_BONE; 6]),
    part([-0.24, 0.0, -0.4], [0.14, 0.6, 0.14], [0.0, 0.6, -0.33], Limb::Swing(1.0), [GT; 6]),
    part([0.1, 0.0, -0.4], [0.14, 0.6, 0.14], [0.0, 0.6, -0.33], Limb::Swing(-1.0), [GT; 6]),
    part([-0.24, 0.0, 0.26], [0.14, 0.6, 0.14], [0.0, 0.6, 0.33], Limb::Swing(-1.0), [GT; 6]),
    part([0.1, 0.0, 0.26], [0.14, 0.6, 0.14], [0.0, 0.6, 0.33], Limb::Swing(1.0), [GT; 6]),
    part([-0.05, 0.95, 0.42], [0.1, 0.12, 0.12], [0.0; 3], Limb::Fixed, [GT; 6]),
];
const AX: u16 = T_AXOLOTL;
static AXOLOTL: [Part; 9] = [
    part([-0.18, 0.08, -0.4], [0.36, 0.24, 0.62], [0.0; 3], Limb::Fixed, [AX; 6]),
    part([-0.22, 0.06, -0.68], [0.44, 0.28, 0.3], [0.0; 3], Limb::Fixed, [AX, AX, AX, AX, AX, T_AXOLOTL_FACE]),
    // Frilly gills either side of the head.
    part([-0.32, 0.12, -0.6], [0.1, 0.3, 0.06], [0.0; 3], Limb::Fixed, [T_AXOLOTL_GILL; 6]),
    part([0.22, 0.12, -0.6], [0.1, 0.3, 0.06], [0.0; 3], Limb::Fixed, [T_AXOLOTL_GILL; 6]),
    part([-0.04, 0.06, 0.2], [0.08, 0.26, 0.4], [0.0, 0.2, 0.2], Limb::SwingY(1.6), [AX; 6]),
    part([-0.26, 0.0, -0.32], [0.1, 0.1, 0.1], [0.0, 0.1, -0.3], Limb::Swing(1.0), [AX; 6]),
    part([0.16, 0.0, -0.32], [0.1, 0.1, 0.1], [0.0, 0.1, -0.3], Limb::Swing(-1.0), [AX; 6]),
    part([-0.26, 0.0, 0.08], [0.1, 0.1, 0.1], [0.0, 0.1, 0.1], Limb::Swing(-1.0), [AX; 6]),
    part([0.16, 0.0, 0.08], [0.1, 0.1, 0.1], [0.0, 0.1, 0.1], Limb::Swing(1.0), [AX; 6]),
];
const CRK: u16 = T_CREAKING;
/// A Creaking: tall and spindly, bark for skin; its eyes glow while it moves.
const fn creaking(face: u16) -> [Part; 6] {
    [
        part([-0.2, 0.0, -0.1], [0.18, 1.25, 0.2], [0.0, 1.25, 0.0], Limb::Swing(1.0), [CRK; 6]),
        part([0.02, 0.0, -0.1], [0.18, 1.25, 0.2], [0.0, 1.25, 0.0], Limb::Swing(-1.0), [CRK; 6]),
        part([-0.24, 1.25, -0.14], [0.48, 0.9, 0.28], [0.0; 3], Limb::Fixed, [CRK; 6]),
        part([-0.42, 0.85, -0.08], [0.16, 1.25, 0.16], [0.0, 2.05, 0.0], Limb::Forward, [CRK; 6]),
        part([0.26, 0.85, -0.08], [0.16, 1.25, 0.16], [0.0, 2.05, 0.0], Limb::Forward, [CRK; 6]),
        part([-0.24, 2.15, -0.24], [0.48, 0.55, 0.48], [0.0; 3], Limb::Fixed, [CRK, CRK, CRK, CRK, CRK, face]),
    ]
}
static CREAKING: [Part; 6] = creaking(T_CREAKING_FACE_ON);
static CREAKING_STILL: [Part; 6] = creaking(T_CREAKING_FACE);
const SN: u16 = T_SNIFFER;
static SNIFFER: [Part; 9] = [
    part([-0.55, 0.55, -0.85], [1.1, 1.0, 1.7], [0.0; 3], Limb::Fixed, [SN; 6]),
    part([-0.35, 0.7, -1.35], [0.7, 0.6, 0.55], [0.0; 3], Limb::Fixed, [SN, SN, SN, SN, SN, T_SNIFFER_FACE]),
    // A big nose, and moss on its back.
    part([-0.25, 0.55, -1.6], [0.5, 0.3, 0.3], [0.0, 0.7, -1.35], Limb::Bob(1.0), [T_SNIFFER_FACE; 6]),
    part([-0.5, 1.5, -0.7], [1.0, 0.1, 1.4], [0.0; 3], Limb::Fixed, [T_PALE_MOSS; 6]),
    part([-0.5, 0.0, -0.6], [0.3, 0.6, 0.3], [0.0, 0.6, -0.45], Limb::Swing(0.8), [SN; 6]),
    part([0.2, 0.0, -0.6], [0.3, 0.6, 0.3], [0.0, 0.6, -0.45], Limb::Swing(-0.8), [SN; 6]),
    part([-0.5, 0.0, 0.0], [0.3, 0.6, 0.3], [0.0, 0.6, 0.15], Limb::Swing(-0.8), [SN; 6]),
    part([0.2, 0.0, 0.0], [0.3, 0.6, 0.3], [0.0, 0.6, 0.15], Limb::Swing(0.8), [SN; 6]),
    part([-0.5, 0.0, 0.5], [1.0, 0.6, 0.3], [0.0, 0.6, 0.6], Limb::Swing(0.4), [SN; 6]),
];
const CM: u16 = T_CAMEL;
/// Where a Camel's neck leans from.
const CAMEL_NECK: [f32; 3] = [0.0, 1.5, -0.8];
static CAMEL: [Part; 10] = [
    part([-0.42, 1.15, -0.85], [0.84, 0.7, 1.7], [0.0; 3], Limb::Fixed, [CM; 6]),
    part([-0.3, 1.85, -0.3], [0.6, 0.35, 0.6], [0.0; 3], Limb::Fixed, [T_CAMEL_HUMP; 6]),
    part([-0.15, 1.4, -1.15], [0.3, 0.75, 0.32], CAMEL_NECK, Limb::Tilt(0.35), [CM; 6]),
    part([-0.18, 2.0, -1.55], [0.36, 0.34, 0.6], CAMEL_NECK, Limb::Tilt(0.35), [CM, CM, CM, CM, CM, T_CAMEL_FACE]),
    part([-0.38, 0.0, -0.75], [0.2, 1.15, 0.2], [0.0, 1.15, -0.65], Limb::Swing(0.8), [CM; 6]),
    part([0.18, 0.0, -0.75], [0.2, 1.15, 0.2], [0.0, 1.15, -0.65], Limb::Swing(-0.8), [CM; 6]),
    part([-0.38, 0.0, 0.55], [0.2, 1.15, 0.2], [0.0, 1.15, 0.65], Limb::Swing(-0.8), [CM; 6]),
    part([0.18, 0.0, 0.55], [0.2, 1.15, 0.2], [0.0, 1.15, 0.65], Limb::Swing(0.8), [CM; 6]),
    part([-0.05, 1.2, 0.85], [0.1, 0.5, 0.1], [0.0, 1.7, 0.88], Limb::SwingY(1.0), [CM; 6]),
    part([-0.12, 2.32, -1.28], [0.24, 0.08, 0.06], CAMEL_NECK, Limb::Tilt(0.35), [CM; 6]),
];
/// A saddle on a Camel, in front of the hump and behind it (two seats).
pub static CAMEL_SADDLE: [Part; 2] = [
    part([-0.44, 1.84, -0.75], [0.88, 0.1, 0.45], [0.0; 3], Limb::Fixed, [T_SADDLE_LEATHER; 6]),
    part([-0.44, 1.84, 0.3], [0.88, 0.1, 0.45], [0.0; 3], Limb::Fixed, [T_SADDLE_LEATHER; 6]),
];

fn model(kind: MobKind) -> &'static [Part] {
    match kind {
        MobKind::Oinker => &OINKER,
        MobKind::Hisser => &HISSER,
        MobKind::Groaner => &GROANER,
        MobKind::Fluffer => &FLUFFER,
        MobKind::Starer => &STARER,
        MobKind::Cluckster => &CLUCKSTER,
        MobKind::Mooer => &MOOER,
        MobKind::Rattler => &RATTLER,
        MobKind::Webber => &WEBBER,
        MobKind::Bloop => &BLOOP,
        MobKind::Woofer => &WOOFER,
        MobKind::Hmmer => &HMMER,
        MobKind::Grumbler => &GRUMBLER,
        MobKind::Galloper => &GALLOPER,
        MobKind::Wyrm => &WYRM,
        MobKind::Squawker => &SQUAWKER,
        MobKind::Clanker => &CLANKER,
        MobKind::Fishy => &FISHY,
        MobKind::Soggy => &SOGGY,
        MobKind::Bee => &BEE,
        MobKind::Sneaker => &SNEAKER,
        MobKind::Ribbit => &RIBBIT,
        MobKind::Rollo => &ROLLO,
        MobKind::Hush => &HUSH,
        MobKind::Sizzler => &SIZZLER,
        MobKind::Weeper => &WEEPER,
        MobKind::Strutter => &STRUTTER,
        MobKind::Snout => &SNOUT,
        MobKind::Pilferer => &PILFERER,
        MobKind::Hackler => &HACKLER,
        MobKind::Invoicer => &INVOICER,
        MobKind::Fee => &FEE,
        MobKind::Rampager => &RAMPAGER,
        MobKind::Breeze => &BREEZE,
        MobKind::Goat => &GOAT,
        MobKind::Axolotl => &AXOLOTL,
        MobKind::Camel => &CAMEL,
        MobKind::Creaking => &CREAKING,
        MobKind::Sniffer => &SNIFFER,
        MobKind::CopperGolem => &COPPER_GOLEM,
        MobKind::Floaty => &FLOATY,
        MobKind::Rotsteed => &ROTSTEED,
        // Modded mobs are drawn from a runtime-built, textured copy of a base
        // template (see `modded_parts`); this static fallback keeps `model`
        // total and is used only where the texture doesn't matter (e.g. the
        // Bell-glow pass), falling back to the plain quadruped shape.
        MobKind::Modded(_) => &OINKER,
    }
}

/// Height of the first solid block below `at` (looking at most `max` blocks
/// down); if there's none, as far down as it looked.
pub fn ground_below(world: &World, at: Vec3, max: i32) -> f32 {
    let (x, y, z) = (at.x.floor() as i32, at.y.floor() as i32, at.z.floor() as i32);
    (0..=max).map(|d| y - d).find(|&yy| is_solid(world.get(x, yy, z))).map(|yy| yy as f32 + 1.0).unwrap_or((y - max) as f32)
}

/// The body boxes for a modded mob: a base template re-textured with the mod's
/// own tile. Built fresh each frame (modded mobs are rare), so it needs no
/// `'static` storage and can carry a per-mob texture.
pub fn modded_parts(def: &crate::block::ModMob) -> Vec<Part> {
    use crate::block::MobTemplate::*;
    if !def.parts.is_empty() {
        use crate::block::PartAnim;
        return def
            .parts
            .iter()
            .map(|p| Part {
                min: p.min,
                size: p.size,
                pivot: p.pivot,
                tiles: p.tiles,
                limb: match p.anim {
                    PartAnim::Still => Limb::Fixed,
                    PartAnim::Walk => Limb::Swing(p.amount),
                    PartAnim::Sway => Limb::SwingY(p.amount),
                    PartAnim::Wing => Limb::Wing(p.amount),
                    PartAnim::Flap => Limb::Flap(p.amount),
                    PartAnim::Tilt => Limb::Tilt(p.amount.to_radians()),
                    PartAnim::Bob => Limb::Bob(p.amount),
                    PartAnim::Spin => Limb::Spin(p.amount),
                },
            })
            .collect();
    }
    let t = def.tile;
    let faces = [t; 6];
    match def.template {
        Quadruped => OINKER.iter().map(|p| Part { tiles: faces, ..*p }).collect(),
        Biped => GROANER.iter().map(|p| Part { tiles: faces, ..*p }).collect(),
        Blob => BLOOP.iter().map(|p| Part { tiles: faces, ..*p }).collect(),
        Bird => CLUCKSTER.iter().map(|p| Part { tiles: faces, ..*p }).collect(),
    }
}

/// Advance a walk cycle by `speed` (blocks a second); once nearly still, ease
/// it back to a rest pose (a multiple of pi, where the limbs hang straight)
/// instead of freezing mid-stride.
pub fn step_anim(anim: &mut f32, speed: f32, dt: f32, rate: f32) {
    if speed > 0.25 {
        *anim += speed * dt * rate;
        return;
    }
    let rest = (*anim / std::f32::consts::PI).round() * std::f32::consts::PI;
    let k = (dt * 10.0).min(1.0);
    *anim += (rest - *anim) * k;
    if (rest - *anim).abs() < 0.01 {
        *anim = rest;
    }
}

pub fn draw_model(geo: &mut DynGeo, root: &Mat4, parts: &[Part], anim: f32, sky: f32, _player: bool) {
    draw_posed(geo, root, parts, anim, 0.0, sky);
}

/// `draw_model` with birds' wings beating to `flap`.
fn draw_posed(geo: &mut DynGeo, root: &Mat4, parts: &[Part], anim: f32, flap: f32, sky: f32) {
    let swing = anim.sin() * 0.7;
    for p in parts {
        let rot = match p.limb {
            Limb::Fixed => Mat4::IDENTITY,
            Limb::Swing(s) => Mat4::from_rotation_x(swing * s),
            Limb::Forward => Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2 + swing * 0.1),
            Limb::SwingY(s) => Mat4::from_rotation_y(swing * s * 0.5),
            Limb::Wing(s) => Mat4::from_rotation_z(anim.sin() * 0.6 * s),
            Limb::Tilt(t) => Mat4::from_rotation_x(t),
            Limb::Flap(s) => Mat4::from_rotation_z(flap.sin().abs() * 1.3 * s) * Mat4::from_rotation_x(swing * 0.3 * s),
            Limb::Bob(s) => Mat4::from_translation(Vec3::Y * (anim * 2.0).sin() * 0.06 * s),
            Limb::Spin(s) => Mat4::from_rotation_y(anim * s),
        };
        let pivot = Vec3::from_array(p.pivot);
        let m = *root
            * Mat4::from_translation(pivot)
            * rot
            * Mat4::from_translation(-pivot)
            * Mat4::from_translation(Vec3::from_array(p.min))
            * Mat4::from_scale(Vec3::from_array(p.size));
        geo.cube(&m, p.tiles, sky, [0.0, 0.0, 1.0, 1.0]);
    }
}

/// One box of worn armour: min, size, pivot, and the limb it follows.
type ArmorPiece = ([f32; 3], [f32; 3], [f32; 3], Limb);

/// Worn armour over the player model (`look` from `Inventory::armor_look`):
/// slightly larger boxes that follow the same limbs.
pub fn draw_armor(geo: &mut DynGeo, root: &Mat4, look: u16, trims: u32, anim: f32, sky: f32, gliding: bool) {
    if look == 0 {
        return;
    }
    let tier = |slot: usize| (look >> (slot * 4)) & 0xF;
    let mut parts = Vec::new();
    let mut add = |slot: usize, list: &[ArmorPiece]| {
        let t = tier(slot);
        let tile = match t as usize {
            1..=4 => T_ARMOR_WORN + t - 1,
            t if t == COPPER_TIER + 1 => T_COPPER_ARMOR_WORN,
            // Nothing, or a Glider (drawn below).
            _ => return,
        };
        for &(min, size, pivot, limb) in list {
            parts.push(part(min, size, pivot, limb, [tile; 6]));
        }
        // Its trim, picked out over the top (see trims.rs).
        let t = (trims >> (slot * 5)) & 0x1F;
        if t > 0 {
            let (pattern, material) = ((t as usize - 1) / 4, (t as usize - 1) % 4);
            let tile = crate::trims::MATERIAL_TILES[material];
            for &(min, size, pivot, limb) in list {
                let e = 0.015;
                let band = |y: f32, h: f32| part([min[0] - e, min[1] + y, min[2] - e], [size[0] + 2.0 * e, h, size[2] + 2.0 * e], pivot, limb, [tile; 6]);
                let h = (size[1] * 0.18).max(0.05);
                match pattern {
                    0 => parts.push(band(size[1] * 0.45, h)),
                    1 => parts.push(band(0.0, h)),
                    2 => {
                        parts.push(band(0.0, h * 0.7));
                        parts.push(band(size[1] - h * 0.7, h * 0.7));
                    }
                    _ => parts.push(part([min[0] + size[0] * 0.4, min[1], min[2] - 2.0 * e], [size[0] * 0.2, size[1], 2.0 * e], pivot, limb, [tile; 6])),
                }
            }
        }
    };
    let arm = [0.0, 1.4, 0.0];
    let hip = [0.0, 0.75, 0.0];
    add(HELMET, &[([-0.3, 1.45, -0.3], [0.6, 0.6, 0.6], [0.0; 3], Limb::Fixed)]);
    add(
        CHESTPLATE,
        &[
            ([-0.29, 0.73, -0.165], [0.58, 0.79, 0.33], [0.0; 3], Limb::Fixed),
            ([-0.54, 1.1, -0.165], [0.33, 0.42, 0.33], arm, Limb::Swing(-1.0)),
            ([0.21, 1.1, -0.165], [0.33, 0.42, 0.33], arm, Limb::Swing(1.0)),
        ],
    );
    add(
        LEGGINGS,
        &[
            ([-0.28, 0.66, -0.155], [0.56, 0.12, 0.31], [0.0; 3], Limb::Fixed),
            ([-0.28, 0.3, -0.155], [0.31, 0.48, 0.31], hip, Limb::Swing(1.0)),
            ([-0.03, 0.3, -0.155], [0.31, 0.48, 0.31], hip, Limb::Swing(-1.0)),
        ],
    );
    add(BOOTS, &[([-0.29, -0.01, -0.165], [0.33, 0.3, 0.33], hip, Limb::Swing(1.0)), ([-0.04, -0.01, -0.165], [0.33, 0.3, 0.33], hip, Limb::Swing(-1.0))]);
    draw_model(geo, root, &parts, anim, sky, true);
    // A Glider: folded down the back, or spread wide while gliding.
    if tier(CHESTPLATE) as usize == GLIDER_TIER + 1 {
        let wings = [
            part([-0.3, 0.45, 0.13], [0.28, 1.0, 0.04], [-0.05, 1.45, 0.15], Limb::Flap(-1.0), [T_GLIDER_WING; 6]),
            part([0.02, 0.45, 0.13], [0.28, 1.0, 0.04], [0.05, 1.45, 0.15], Limb::Flap(1.0), [T_GLIDER_WING; 6]),
        ];
        let spread = if gliding { std::f32::consts::FRAC_PI_2 } else { 0.12 };
        draw_posed(geo, root, &wings, 0.0, spread, sky);
    }
}

pub struct Particle {
    pub pos: Vec3,
    pub vel: Vec3,
    pub life: f32,
    pub tile: u16,
    pub uv: [f32; 2],
    pub size: f32,
    pub gravity: f32,
}

impl Particle {
    pub fn update(&mut self, dt: f32, world: &World) {
        self.life -= dt;
        self.vel.y -= self.gravity * dt;
        let next = self.pos + self.vel * dt;
        if is_solid(world.get(next.x.floor() as i32, next.y.floor() as i32, next.z.floor() as i32)) {
            self.vel = Vec3::ZERO;
        } else {
            self.pos = next;
        }
    }
    pub fn draw(&self, geo: &mut DynGeo, world: &World) {
        let sky = world.shade_near(self.pos);
        let s = self.size;
        let m = Mat4::from_translation(self.pos - Vec3::splat(s * 0.5)) * Mat4::from_scale(Vec3::splat(s));
        let r = [self.uv[0], self.uv[1], self.uv[0] + 0.25, self.uv[1] + 0.25];
        geo.cube(&m, [self.tile; 6], sky, r);
    }

    /// Drawn at full brightness whatever the light (firework sparks).
    pub fn draw_lit(&self, geo: &mut DynGeo) {
        let s = self.size;
        let m = Mat4::from_translation(self.pos - Vec3::splat(s * 0.5)) * Mat4::from_scale(Vec3::splat(s));
        let r = [self.uv[0], self.uv[1], self.uv[0] + 0.25, self.uv[1] + 0.25];
        geo.cube(&m, [self.tile; 6], 1.0, r);
    }
}

/// A Pointy Stick in flight (or stuck in something).
pub struct Arrow {
    pub pos: Vec3,
    pub vel: Vec3,
    /// Player id that fired it, or None for a Rattler.
    pub shooter: Option<u32>,
    pub damage: f32,
    /// Seconds left before it disappears.
    pub life: f32,
    pub stuck: bool,
    /// Which way it points (kept once it stops moving).
    pub dir: Vec3,
    /// A thrown Soggy Spear (with its wear) instead of a Pointy Stick: it
    /// drops where it lands, to be picked up again.
    pub spear: Option<(Id, u32)>,
    /// Fired by a modded mob rather than a base Rattler. Host-side only (never
    /// sent over the wire); only changes the death-message attribution so a
    /// modded slinger's kill isn't blamed on a Rattler.
    pub modded: bool,
    /// Transient appearance mirrored to joined clients.
    pub appearance: ProjectileAppearance,
    /// Host-only timed effect payload; never included in snapshots.
    pub effect: Option<ProjectileEffect>,
    /// Host-only: degrees per second it turns toward the nearest player. A
    /// homing projectile isn't pulled down by gravity.
    pub homing: f32,
    /// Host-only: blast radius when it lands (0 = a plain hit).
    pub blast: f32,
    /// Host-only: a wind charge (flies straight; bursts on whatever it meets, see trial.rs).
    pub wind: bool,
    /// Host-only: a firework rocket, by colour + 1 (0: not one; see fireworks.rs).
    pub firework: u8,
}

impl Arrow {
    pub const SPEED: f32 = 24.0;
    const GRAVITY: f32 = 20.0;

    pub fn new(pos: Vec3, vel: Vec3, shooter: Option<u32>, damage: f32) -> Arrow {
        Arrow {
            pos,
            vel,
            shooter,
            damage,
            life: 8.0,
            stuck: false,
            dir: vel.normalize_or(Vec3::Z),
            spear: None,
            modded: false,
            appearance: ProjectileAppearance::default(),
            effect: None,
            homing: 0.0,
            blast: 0.0,
            wind: false,
            firework: 0,
        }
    }

    /// Fly for `dt`. Returns true if it just hit a block (and stuck there).
    pub fn fly(&mut self, dt: f32, world: &World) -> bool {
        self.life -= dt;
        if self.stuck {
            return false;
        }
        if self.homing <= 0.0 && !self.wind && self.firework == 0 {
            self.vel.y -= Self::GRAVITY * dt;
        }
        self.dir = self.vel.normalize_or(self.dir);
        let d = self.vel * dt;
        let steps = (d.length() / 0.2).ceil().max(1.0) as i32;
        for _ in 0..steps {
            let next = self.pos + d / steps as f32;
            if is_solid(world.get(next.x.floor() as i32, next.y.floor() as i32, next.z.floor() as i32)) {
                self.stuck = true;
                self.vel = Vec3::ZERO;
                self.life = self.life.min(5.0);
                return true;
            }
            self.pos = next;
        }
        false
    }

    /// For the network: velocity while flying; a tiny vector along `dir` once stuck.
    pub fn wire_vel(&self) -> Vec3 {
        if self.stuck { self.dir * 1e-3 } else { self.vel }
    }

    /// A client's copy, from the host's snapshot.
    pub fn from_wire(pos: Vec3, v: Vec3, appearance: ProjectileAppearance) -> Arrow {
        let stuck = v.length() < 0.01;
        Arrow {
            pos,
            vel: if stuck { Vec3::ZERO } else { v },
            shooter: None,
            damage: 0.0,
            life: 1.0,
            stuck,
            dir: v.normalize_or(Vec3::Z),
            spear: None,
            modded: false,
            appearance: appearance.normalized(),
            effect: None,
            homing: 0.0,
            blast: 0.0,
            wind: false,
            firework: 0,
        }
    }

    pub fn draw(&self, geo: &mut DynGeo, world: &World, camera: Vec3) {
        let sky = world.shade_near(self.pos);
        let appearance = self.appearance.normalized();
        if appearance == ProjectileAppearance::default() {
            let rot = macroquad::math::Quat::from_rotation_arc(Vec3::Z, self.dir);
            let m = Mat4::from_translation(self.pos) * Mat4::from_quat(rot) * Mat4::from_translation(Vec3::new(-0.03, -0.03, -0.55)) * Mat4::from_scale(Vec3::new(0.06, 0.06, 0.6));
            geo.cube(&m, [T_PLANKS; 6], sky, [0.0, 0.0, 0.25, 0.25]);
            let tip = Mat4::from_translation(self.pos) * Mat4::from_quat(rot) * Mat4::from_translation(Vec3::new(-0.04, -0.04, 0.0)) * Mat4::from_scale(Vec3::new(0.08, 0.08, 0.1));
            geo.cube(&tip, [T_STONE; 6], sky, [0.0, 0.0, 0.25, 0.25]);
            return;
        }

        let scale = appearance.scale;
        match appearance.model {
            ProjectileModel::Arrow => {
                let tile = appearance.tile.unwrap_or(T_PLANKS);
                let rot = macroquad::math::Quat::from_rotation_arc(Vec3::Z, self.dir);
                let m = Mat4::from_translation(self.pos)
                    * Mat4::from_quat(rot)
                    * Mat4::from_translation(Vec3::new(-0.03, -0.03, -0.55) * scale)
                    * Mat4::from_scale(Vec3::new(0.06, 0.06, 0.65) * scale);
                geo.cube(&m, [tile; 6], sky, [0.0, 0.0, 1.0, 1.0]);
            }
            ProjectileModel::Billboard => {
                let tile = appearance.tile.unwrap_or(T_WHITE);
                let half = 0.35 * scale;
                let right = Vec3::Y.cross(camera - self.pos).normalize_or(Vec3::X) * half;
                let up = Vec3::Y * half;
                let corners = [self.pos - right - up, self.pos + right - up, self.pos + right + up, self.pos - right + up];
                geo.quad(corners, tile, [0.0, 0.0, 1.0, 1.0], [1.0, sky]);
                geo.quad([corners[1], corners[0], corners[3], corners[2]], tile, [0.0, 0.0, 1.0, 1.0], [1.0, sky]);
            }
            ProjectileModel::Cube => {
                let tile = appearance.tile.unwrap_or(T_WHITE);
                let size = 0.4 * scale;
                let m = Mat4::from_translation(self.pos - Vec3::splat(size * 0.5)) * Mat4::from_scale(Vec3::splat(size));
                geo.cube(&m, [tile; 6], sky, [0.0, 0.0, 1.0, 1.0]);
            }
        }
    }
}

pub struct PrimedTnt {
    pub pos: Vec3,
    pub fuse: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::noise::Rng;
    use macroquad::math::IVec3;

    #[test]
    fn base_mob_indices_round_trip() {
        // Hold the registry lock so no concurrent `with_mods` test has mod mobs
        // installed in the global registry while we assert its base shape.
        let _guard = crate::mods::registry_test_lock();
        // Every base kind keeps its stable index, and resolves back.
        for (i, k) in MobKind::ALL.iter().enumerate() {
            assert_eq!(k.index() as usize, i);
            assert_eq!(MobKind::from_index(i as u8), Some(*k));
        }
        // Just past the base set is unknown when no mods are loaded.
        assert_eq!(MobKind::from_index(BASE_MOBS), None);
        assert_eq!(MobKind::from_index(u8::MAX), None);
    }

    #[test]
    fn projectile_appearance_render_shapes_preserve_classic_arrow() {
        let world = World::new(1);
        let pos = Vec3::new(2.0, 80.0, 3.0);
        let arrow = Arrow::new(pos, Vec3::Z, None, 3.0);
        let mut classic = DynGeo::default();
        arrow.draw(&mut classic, &world, pos + Vec3::X * 4.0);

        // Pin the exact pre-customization shaft and stone-tip geometry.
        let sky = world.shade_near(pos);
        let rot = macroquad::math::Quat::from_rotation_arc(Vec3::Z, Vec3::Z);
        let mut expected = DynGeo::default();
        let shaft = Mat4::from_translation(pos) * Mat4::from_quat(rot) * Mat4::from_translation(Vec3::new(-0.03, -0.03, -0.55)) * Mat4::from_scale(Vec3::new(0.06, 0.06, 0.6));
        expected.cube(&shaft, [T_PLANKS; 6], sky, [0.0, 0.0, 0.25, 0.25]);
        let tip = Mat4::from_translation(pos) * Mat4::from_quat(rot) * Mat4::from_translation(Vec3::new(-0.04, -0.04, 0.0)) * Mat4::from_scale(Vec3::new(0.08, 0.08, 0.1));
        expected.cube(&tip, [T_STONE; 6], sky, [0.0, 0.0, 0.25, 0.25]);
        assert_eq!(classic.mesh.idx, expected.mesh.idx);
        assert_eq!(classic.mesh.verts.len(), expected.mesh.verts.len());
        for (actual, expected) in classic.mesh.verts.iter().zip(&expected.mesh.verts) {
            assert_eq!((actual.pos, actual.uv, actual.light, actual.tile), (expected.pos, expected.uv, expected.light, expected.tile));
        }

        let render = |appearance| {
            let mut arrow = Arrow::new(pos, Vec3::Z, None, 3.0);
            arrow.appearance = appearance;
            let mut geo = DynGeo::default();
            arrow.draw(&mut geo, &world, pos + Vec3::X * 4.0);
            geo
        };
        let assert_render = |geo: &DynGeo, counts: (usize, usize), expected_extent: Vec3| {
            assert_eq!((geo.mesh.verts.len(), geo.mesh.idx.len()), counts);
            let (u, v, _) = tile_uv(T_STONE);
            let expected_tile = [-u - 2.0, -v - 2.0];
            assert!(geo.mesh.verts.iter().all(|vertex| vertex.tile == expected_tile));
            let mut min = Vec3::splat(f32::INFINITY);
            let mut max = Vec3::splat(f32::NEG_INFINITY);
            for vertex in &geo.mesh.verts {
                let point = Vec3::from_array(vertex.pos);
                min = min.min(point);
                max = max.max(point);
            }
            let extent = max - min;
            assert!((extent.x - expected_extent.x).abs() < 1e-5, "{extent:?}");
            assert!((extent.y - expected_extent.y).abs() < 1e-5, "{extent:?}");
            assert!((extent.z - expected_extent.z).abs() < 1e-5, "{extent:?}");
        };
        assert_render(
            &render(ProjectileAppearance { model: ProjectileModel::Arrow, tile: Some(T_STONE), scale: 2.0 }),
            (24, 36),
            Vec3::new(0.12, 0.12, 1.3),
        );
        assert_render(
            &render(ProjectileAppearance { model: ProjectileModel::Billboard, tile: Some(T_STONE), scale: 1.5 }),
            (8, 12),
            Vec3::new(0.0, 1.05, 1.05),
        );
        assert_render(
            &render(ProjectileAppearance { model: ProjectileModel::Cube, tile: Some(T_STONE), scale: 0.5 }),
            (24, 36),
            Vec3::splat(0.2),
        );
    }

    #[test]
    fn modded_mob_dispatch_is_total_and_wanders_without_panicking() {
        let src = "[mob critter]\nname = Little Critter\ntexture = stone\ntemplate = quadruped\nsize = 0.5\nhealth = 7\ndrops = stick 3\n";
        crate::mods::with_mods(&[("zoo", src)], |reg| {
            let k = MobKind::from_name("zoo:critter").expect("resolves");
            // The def's data flows through the dispatch.
            assert_eq!(k.name(), "Little Critter");
            assert_eq!(k.max_health(), 7.0);
            // A passive wanderer: not hostile, not flying, doesn't burn in the sun.
            assert!(!k.hostile() && !k.flies() && !k.fireproof());
            // The render template exists and has boxes.
            assert!(!modded_parts(&reg.mobs[0]).is_empty());

            let mut rng = Rng::new(1);
            let world = World::new(1);
            let mut m = Mob::new(k, Vec3::new(0.0, 80.0, 0.0), &mut rng);
            assert_eq!(m.health, 7.0);
            assert_eq!((m.body.half, m.body.height), k.dims());
            // Many ticks of AI must never panic for a modded mob.
            for _ in 0..200 {
                m.update(0.05, &world, Vec3::new(2.0, 80.0, 0.0), true, 1.0, &mut rng);
            }
            // Hitting it makes it flee, and it drops its defined item.
            m.damage(2.0, Vec3::ZERO);
            assert!(m.flee > 0.0);
            let drop = m.loot(&mut Rng::new(2));
            assert!(matches!(drop, Some((crate::block::STICK, n)) if (1..=3).contains(&n)));
        });
    }

    #[test]
    fn modded_index_never_reaches_the_save_sentinel() {
        // The save/wire code (animals::encode_mobs) uses 255 as the "modded mob,
        // read a name" sentinel, so a modded kind's one-byte index must never be
        // able to reach it. The only thing guaranteeing that is MAX_MOD_MOBS, so
        // pin the boundary: the highest allotable modded index stays strictly
        // below 255. A future bump to BASE_MOBS or MAX_MOD_MOBS that erodes this
        // headroom fails here loudly.
        const SAVE_SENTINEL: usize = 255;
        let highest_modded_index = BASE_MOBS as usize + (MAX_MOD_MOBS - 1);
        assert!(
            highest_modded_index < SAVE_SENTINEL,
            "modded index {highest_modded_index} must stay below the {SAVE_SENTINEL} save sentinel",
        );

        // The index arithmetic round-trips at the top of the modded range. We
        // test the pure math (index -> byte) without installing a registry, then
        // confirm the byte is still below the sentinel and fits a u8.
        let top = MobKind::Modded(MAX_MOD_MOBS as u16 - 1);
        let byte = top.index();
        assert_eq!(byte as usize, highest_modded_index);
        assert!((byte as usize) < SAVE_SENTINEL);
    }

    /// Count the HurtPlayer events in a tick, returning (count, total_damage).
    fn hurt_hits(ev: &[MobEvent]) -> (usize, f32) {
        let mut n = 0;
        let mut dmg = 0.0;
        for e in ev {
            if let MobEvent::HurtPlayer(d, _) = e {
                n += 1;
                dmg += *d;
            }
        }
        (n, dmg)
    }

    /// Count the ShootMod events in a tick, returning (count, total_damage).
    fn shoot_hits(ev: &[MobEvent]) -> (usize, f32) {
        let mut n = 0;
        let mut dmg = 0.0;
        for e in ev {
            if let MobEvent::ShootMod(_, _, spec) = e {
                n += 1;
                dmg += spec.damage;
            }
        }
        (n, dmg)
    }

    #[test]
    fn goats_charge_and_lose_horns_on_walls() {
        let mut rng = Rng::new(5);
        let mut world = World::new(1);
        world.load_now(0, 0);
        let floor = world.surface_y(8, 8);
        let at = Vec3::new(8.5, floor as f32 + 1.0, 8.5);
        let mut g = Mob::new(MobKind::Goat, at, &mut rng);
        g.warp_cd = 0.0;
        g.yaw = 0.0;
        // Someone 6 blocks away: it lowers its head, then charges.
        let player = at + Vec3::new(0.0, 0.0, -6.0);
        let mut bleat = false;
        let mut rammed = false;
        for _ in 0..120 {
            for e in g.update(0.05, &world, player, true, 1.0, &mut rng) {
                match e {
                    MobEvent::Bleat(_) => bleat = true,
                    MobEvent::HurtPlayer(_, cause) => rammed |= cause.contains("Goat"),
                    _ => {}
                }
            }
        }
        assert!(bleat && rammed, "bleat {bleat}, rammed {rammed}");
        assert!(g.warp_cd > 0.0, "and then it rests");
        // A charge into a wall knocks a horn loose (sometimes).
        let mut horns = 0;
        for k in 0..40 {
            let mut g = Mob::new(MobKind::Goat, at, &mut Rng::new(k));
            g.hop_cd = 1.0;
            g.wander_dir = Some(0.0);
            g.body.hit_wall = true;
            g.body.vel = Vec3::ZERO;
            let wall = at.floor().as_ivec3() + IVec3::new(0, 0, -1);
            world.set_v(wall, STONE);
            world.set_v(wall + IVec3::Y, STONE);
            for _ in 0..10 {
                horns += g.update(0.05, &world, at + Vec3::new(30.0, 0.0, 0.0), false, 1.0, &mut rng).iter().filter(|e| matches!(e, MobEvent::DropItem(_, GOAT_HORN))).count();
            }
        }
        assert!(horns > 5, "{horns} horns from 40 charges");
    }

    #[test]
    fn hostile_modded_mob_attacks_in_reach_then_respects_cooldown() {
        let src = "[mob biter]\nname = Biter\ntexture = stone\ntemplate = biped\nsize = 0.9\nhealth = 20\nhostile = true\nattack_damage = 4\nattack_reach = 1.5\naggro_range = 20\nattack_cooldown = 1.5\n";
        crate::mods::with_mods(&[("zoo", src)], |_reg| {
            let k = MobKind::from_name("zoo:biter").expect("resolves");
            assert!(k.hostile());
            let def = k.mod_def().expect("def");
            assert_eq!(def.attack_damage, 4.0);
            assert_eq!(def.attack_cooldown, 1.5);

            let mut rng = Rng::new(7);
            let world = World::new(1);
            let pos = Vec3::new(0.0, 80.0, 0.0);
            let mut m = Mob::new(k, pos, &mut rng);
            // Place the player right next to the mob (within attack_reach).
            let player = Vec3::new(1.0, 80.0, 0.0);
            // Cooldown starts at 0, so the first tick lands exactly one hit.
            let ev = m.update(0.05, &world, player, true, 1.0, &mut rng);
            let (n, dmg) = hurt_hits(&ev);
            assert_eq!(n, 1, "exactly one HurtPlayer on the first in-reach tick");
            assert_eq!(dmg, 4.0, "deals the configured attack_damage");
            // The cooldown is now set to the configured value, so the very next
            // tick (minus the small dt decrement) must not attack again.
            assert!(m.attack_cd > 1.0);
            let ev2 = m.update(0.05, &world, player, true, 1.0, &mut rng);
            assert_eq!(hurt_hits(&ev2).0, 0, "respects the cooldown on the next tick");
        });
    }

    #[test]
    fn hostile_modded_mob_in_aggro_but_out_of_reach_does_not_attack() {
        let src = "[mob biter]\nname = Biter\ntexture = stone\ntemplate = biped\nhostile = true\nattack_damage = 4\nattack_reach = 1.5\naggro_range = 20\nattack_cooldown = 1.0\n";
        crate::mods::with_mods(&[("zoo", src)], |_reg| {
            let k = MobKind::from_name("zoo:biter").expect("resolves");
            let mut rng = Rng::new(11);
            let world = World::new(1);
            let mut m = Mob::new(k, Vec3::new(0.0, 80.0, 0.0), &mut rng);
            // Within aggro_range (20) but well beyond attack_reach (1.5).
            let player = Vec3::new(8.0, 80.0, 0.0);
            let ev = m.update(0.05, &world, player, true, 1.0, &mut rng);
            assert_eq!(hurt_hits(&ev).0, 0, "no hit beyond attack_reach");
            // It should still be pursuing (its yaw turns toward the player: +x).
            assert!(m.body.vel.x > 0.0 || m.yaw.abs() < std::f32::consts::FRAC_PI_2);
        });
    }

    #[test]
    fn hostile_mob_without_damage_and_nonhostile_mob_never_attack() {
        // hostile = true but attack_damage left at the default 0.0.
        let passive_hostile = "[mob lurker]\nname = Lurker\ntexture = stone\ntemplate = biped\nhostile = true\n";
        crate::mods::with_mods(&[("zoo", passive_hostile)], |_reg| {
            let k = MobKind::from_name("zoo:lurker").expect("resolves");
            assert!(k.hostile());
            assert_eq!(k.mod_def().unwrap().attack_damage, 0.0);
            let mut rng = Rng::new(3);
            let world = World::new(1);
            let mut m = Mob::new(k, Vec3::new(0.0, 80.0, 0.0), &mut rng);
            let player = Vec3::new(0.5, 80.0, 0.0); // adjacent
            for _ in 0..20 {
                let ev = m.update(0.05, &world, player, true, 1.0, &mut rng);
                assert_eq!(hurt_hits(&ev).0, 0, "attack_damage == 0 never attacks");
            }
        });

        // A plain non-hostile mob never attacks either, and still flees when hit.
        let passive = "[mob mouse]\nname = Mouse\ntexture = stone\ntemplate = quadruped\n";
        crate::mods::with_mods(&[("zoo", passive)], |_reg| {
            let k = MobKind::from_name("zoo:mouse").expect("resolves");
            assert!(!k.hostile());
            let mut rng = Rng::new(5);
            let world = World::new(1);
            let mut m = Mob::new(k, Vec3::new(0.0, 80.0, 0.0), &mut rng);
            let player = Vec3::new(0.5, 80.0, 0.0);
            for _ in 0..20 {
                let ev = m.update(0.05, &world, player, true, 1.0, &mut rng);
                assert_eq!(hurt_hits(&ev).0, 0, "non-hostile never attacks");
            }
            m.damage(2.0, Vec3::ZERO);
            assert!(m.flee > 0.0, "passive flee-when-hit behavior is preserved");
        });
    }

    #[test]
    fn projectile_multishot_emits_one_volley_and_one_cooldown_with_melee_priority() {
        let src = "[mob slinger]\nname = Slinger\ntexture = stone\ntemplate = biped\nhostile = true\nattack_damage = 4\nattack_reach = 1.5\naggro_range = 10\nattack_cooldown = 1.0\nranged_damage = 6\nranged_range = 20\nprojectile_speed = 24\nprojectile_count = 3\nprojectile_spread = 18\nranged_cooldown = 2\n";
        crate::mods::with_mods(&[("zoo", src)], |_reg| {
            let k = MobKind::from_name("zoo:slinger").expect("resolves");
            let def = k.mod_def().expect("def");
            assert_eq!(def.ranged_damage, 6.0);
            assert_eq!(def.ranged_cooldown, 2.0);

            let mut rng = Rng::new(7);
            let world = World::new(1);
            let mut m = Mob::new(k, Vec3::new(0.0, 80.0, 0.0), &mut rng);

            // Beyond attack_reach (1.5) but within ranged_range (20): one
            // ShootMod volley event, not one event per projectile.
            let far = Vec3::new(8.0, 80.0, 0.0);
            let ev = m.update(0.05, &world, far, true, 1.0, &mut rng);
            assert_eq!(hurt_hits(&ev).0, 0, "no melee hit beyond attack_reach");
            let (n, dmg) = shoot_hits(&ev);
            assert_eq!(n, 1, "exactly one ShootMod for the volley");
            assert_eq!(dmg, 6.0, "fires the configured ranged_damage");
            let spec = ev.iter().find_map(|event| match event {
                MobEvent::ShootMod(_, _, spec) => Some(*spec),
                _ => None,
            }).expect("volley specification");
            assert_eq!((spec.count, spec.spread), (3, 18.0));
            // The whole volley consumes the configured cooldown exactly once.
            assert_eq!(m.attack_cd, 2.0);
            let ev2 = m.update(0.05, &world, far, true, 1.0, &mut rng);
            assert_eq!(shoot_hits(&ev2).0, 0, "respects ranged cooldown");

            // Within attack_reach: melee takes priority over the whole volley.
            let mut m2 = Mob::new(k, Vec3::new(0.0, 80.0, 0.0), &mut rng);
            let near = Vec3::new(1.0, 80.0, 0.0);
            let ev3 = m2.update(0.05, &world, near, true, 1.0, &mut rng);
            assert_eq!(hurt_hits(&ev3).0, 1, "melees up close");
            assert_eq!(shoot_hits(&ev3).0, 0, "no ranged volley inside attack_reach");
        });
    }

    #[test]
    fn ranged_off_by_default_modded_mob_never_shoots() {
        // A melee-only hostile mob (no ranged fields) never emits ShootMod.
        let src = "[mob biter]\nname = Biter\ntexture = stone\ntemplate = biped\nhostile = true\nattack_damage = 4\nattack_reach = 1.5\naggro_range = 20\nattack_cooldown = 1.0\n";
        crate::mods::with_mods(&[("zoo", src)], |_reg| {
            let k = MobKind::from_name("zoo:biter").expect("resolves");
            assert_eq!(k.mod_def().unwrap().ranged_damage, 0.0);
            let mut rng = Rng::new(13);
            let world = World::new(1);
            let mut m = Mob::new(k, Vec3::new(0.0, 80.0, 0.0), &mut rng);
            let player = Vec3::new(8.0, 80.0, 0.0); // in aggro range, out of reach
            for _ in 0..40 {
                let ev = m.update(0.05, &world, player, true, 1.0, &mut rng);
                assert_eq!(shoot_hits(&ev).0, 0, "ranged_damage == 0 never shoots");
            }
        });
    }

    #[test]
    fn projectile_multishot_effect_only_volley_uses_one_event() {
        let src = "[mob hexer]\ntexture = stone\nhostile = true\nranged_damage = 0\nranged_range = 20\nprojectile_effect = speed\nprojectile_effect_duration = 12\nprojectile_effect_amplifier = 2\nprojectile_count = 4\nprojectile_spread = 12\n\n[mob idle]\ntexture = stone\nhostile = true\nranged_damage = 0\n";
        crate::mods::with_mods(&[("zoo", src)], |_reg| {
            let mut rng = Rng::new(17);
            let world = World::new(1);
            let player = Vec3::new(8.0, 80.0, 0.0);
            let mut hexer = Mob::new(MobKind::from_name("zoo:hexer").unwrap(), Vec3::new(0.0, 80.0, 0.0), &mut rng);
            let events = hexer.update(0.05, &world, player, true, 1.0, &mut rng);
            assert_eq!(shoot_hits(&events), (1, 0.0));
            let spec = events.iter().find_map(|event| match event {
                MobEvent::ShootMod(_, _, spec) => Some(*spec),
                _ => None,
            }).expect("effect-only volley");
            assert_eq!((spec.count, spec.spread), (4, 12.0));
            assert_eq!(spec.effect, Some(ProjectileEffect { kind: crate::potions::Potion::Speed, duration: 12.0, amplifier: 2 }));

            let mut idle = Mob::new(MobKind::from_name("zoo:idle").unwrap(), Vec3::new(0.0, 80.0, 0.0), &mut rng);
            for _ in 0..20 {
                assert_eq!(shoot_hits(&idle.update(0.05, &world, player, true, 1.0, &mut rng)).0, 0);
            }
        });
    }
}
