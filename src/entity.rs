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
}

impl MobKind {
    /// Every kind, in wire/script index order (append only).
    pub const ALL: [MobKind; 13] = [
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
    ];

    pub fn index(self) -> u8 {
        MobKind::ALL.iter().position(|k| *k == self).unwrap_or(0) as u8
    }
    pub fn from_index(i: u8) -> Option<MobKind> {
        MobKind::ALL.get(i as usize).copied()
    }
    /// Names accepted by mods and scripts (the parody name or the one it parodies).
    pub fn from_name(s: &str) -> Option<MobKind> {
        match s.to_ascii_lowercase().as_str() {
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
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
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
        }
    }
    /// Half-width and height at size 1.
    fn dims(self) -> (f32, f32) {
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
        }
    }
    fn max_health(self) -> f32 {
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
        }
    }
    /// Experience for defeating one (`size`: a Bloop's size).
    pub fn xp_value(self, size: f32, rng: &mut Rng) -> u32 {
        match self {
            MobKind::Bloop => size as u32,
            k if !k.hostile() => rng.int(1, 3) as u32,
            _ => 5,
        }
    }
    /// Spawns at night / in caves and counts toward the hostile cap.
    /// (Starers and daytime Webbers are only hostile once provoked, but they keep monster hours.)
    pub fn hostile(self) -> bool {
        !self.passive() && !matches!(self, MobKind::Woofer | MobKind::Hmmer | MobKind::Grumbler)
    }
    /// Farm animals: wander, flee when hit, spawn in daylight on grass.
    pub fn passive(self) -> bool {
        matches!(self, MobKind::Oinker | MobKind::Fluffer | MobKind::Cluckster | MobKind::Mooer)
    }
    /// What it eats to fall in love (see animals.rs); Woofers only once tamed.
    pub fn breed_food(self) -> &'static [Id] {
        match self {
            MobKind::Oinker => &[CARROT, POTATO],
            MobKind::Fluffer | MobKind::Mooer => &[WHEAT],
            MobKind::Cluckster => &[WHEAT_SEEDS],
            MobKind::Woofer => &[PORKCHOP, COOKED_CHOP, MUTTON, COOKED_MUTTON, MOO_STEAK, STEAK, CLUCKETS, COOKED_CLUCKETS, GOO],
            _ => &[],
        }
    }
    /// Undead: burn in sunlight.
    fn burns(self) -> bool {
        matches!(self, MobKind::Groaner | MobKind::Rattler)
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
}

pub enum MobEvent {
    HurtPlayer(f32, &'static str),
    Explode(Vec3, f32, &'static str),
    Smoke(Vec3),
    /// A Starer blinked from one place to another.
    Warp(Vec3, Vec3),
    /// A Rattler loosed a Pointy Stick: (from, velocity).
    Shoot(Vec3, Vec3),
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
            knock: Vec3::ZERO,
            burning: false,
            on_fire: 0.0,
            angry: false,
            warp_cd: 0.0,
            size: 1.0,
            hop_cd: rng.range(0.5, 2.0),
            love: 0.0,
            breed_cd: 0.0,
            baby: 0.0,
            sheared: false,
            owner: None,
            sitting: false,
            prey: None,
            goal: None,
            persistent: false,
            seed: 0,
            home: None,
            trades_used: [0; 8],
            restock: crate::villagers::RESTOCK_SECS,
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
    pub fn ready_to_breed(&self) -> bool {
        self.baby <= 0.0 && self.breed_cd <= 0.0 && self.love <= 0.0 && (self.kind != MobKind::Woofer || self.owner.is_some())
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
        self.health -= amount;
        self.hurt = 0.5;
        let mut dir = self.body.pos - from;
        dir.y = 0.0;
        let dir = dir.normalize_or_zero();
        self.knock = dir * 7.0;
        self.body.vel.y = 6.0;
        match self.kind {
            k if k.passive() => self.flee = 4.0,
            MobKind::Webber => self.angry = true,
            MobKind::Woofer if self.owner.is_none() => self.angry = true,
            MobKind::Hmmer => self.flee = 4.0,
            MobKind::Grumbler => self.angry = true,
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
        let to_player = player - self.body.pos;
        let dist = to_player.length();
        let flat = Vec3::new(to_player.x, 0.0, to_player.z);

        let mut want: Option<(f32, f32)> = None; // (yaw, speed)
        // Wander when there's nothing better to do (Bloops only ever hop).
        let mut may_wander = true;
        let face = flat.x.atan2(-flat.z);
        match self.kind {
            MobKind::Oinker | MobKind::Fluffer | MobKind::Cluckster | MobKind::Mooer | MobKind::Hmmer => {
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
        if self.body.in_lava {
            self.on_fire = 6.0;
            self.health -= dt * 6.0;
            self.hurt = self.hurt.max(0.2);
        } else if self.body.in_water {
            self.on_fire = 0.0;
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
        if self.body.in_water {
            self.body.vel.y = (self.body.vel.y + 14.0 * dt).min(2.5);
        } else {
            self.body.vel.y = (self.body.vel.y - GRAVITY * dt).max(-50.0);
            if self.kind == MobKind::Cluckster {
                // Flap flap: Clucksters flutter down instead of falling.
                self.body.vel.y = self.body.vel.y.max(-2.5);
            }
        }
        let prev_ground = self.body.on_ground;
        move_body(world, &mut self.body, dt, false);
        if moving && self.body.hit_wall {
            match self.kind {
                // Webbers walk straight up walls.
                MobKind::Webber => self.body.vel.y = 3.2,
                MobKind::Bloop => {}
                _ if self.body.on_ground || prev_ground => self.body.vel.y = 8.8,
                _ => {}
            }
        }
        let spd = Vec3::new(self.body.vel.x, 0.0, self.body.vel.z).length();
        self.anim += spd * dt * 5.0;
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
            MobKind::Cluckster if n > 0 => Some((FEATHER, n)),
            MobKind::Mooer => Some((MOO_STEAK, n + 1)),
            MobKind::Rattler if n > 0 => Some((BONE, n)),
            MobKind::Webber if n > 0 => Some((STRING, n)),
            // Only the smallest Bloops leave anything; bigger ones split instead.
            MobKind::Bloop if n > 0 && self.size <= 1.0 => Some((GOO, n)),
            MobKind::Grumbler if n > 0 => Some((GOO, n)),
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
        let sky = world.sky_light(p.x.floor() as i32, (p.y + 0.5).floor() as i32, p.z.floor() as i32);
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
        let parts = match self.kind {
            MobKind::Fluffer if self.sheared => &FLUFFER_SHEARED[..],
            k => model(k),
        };
        draw_model(geo, &root, parts, if self.sitting { 0.0 } else { self.anim }, sky, false);
        if self.owner.is_some() {
            draw_model(geo, &root, &WOOFER_COLLAR, 0.0, sky, false);
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

const fn humanoid(skin: u16, face: u16, shirt: u16, pants: u16, arms: Limb, arms2: Limb) -> [Part; 6] {
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
pub static STOVE: [Part; 6] = humanoid(T_SKIN, T_STOVE_FACE, T_STOVE_SHIRT, T_STOVE_PANTS, Limb::Swing(-1.0), Limb::Swing(1.0));

const CB: u16 = T_CLUCK_BODY;
const CL: u16 = T_CLUCK_LEG;
static CLUCKSTER: [Part; 7] = [
    part([-0.19, 0.25, -0.25], [0.38, 0.35, 0.5], [0.0; 3], Limb::Fixed, [CB; 6]),
    part([-0.12, 0.45, -0.42], [0.24, 0.3, 0.2], [0.0; 3], Limb::Fixed, [CB, CB, CB, CB, CB, T_CLUCK_FACE]),
    part([-0.1, 0.0, -0.05], [0.06, 0.26, 0.06], [0.0, 0.26, 0.0], Limb::Swing(1.0), [CL; 6]),
    part([0.04, 0.0, -0.05], [0.06, 0.26, 0.06], [0.0, 0.26, 0.0], Limb::Swing(-1.0), [CL; 6]),
    // Wings (they flap when it walks, and it only really walks when fleeing).
    part([-0.24, 0.3, -0.2], [0.05, 0.22, 0.4], [-0.2, 0.52, 0.0], Limb::Swing(0.3), [CB; 6]),
    part([0.19, 0.3, -0.2], [0.05, 0.22, 0.4], [0.2, 0.52, 0.0], Limb::Swing(-0.3), [CB; 6]),
    part([-0.08, 0.5, 0.2], [0.16, 0.15, 0.1], [0.0; 3], Limb::Fixed, [CB; 6]),
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
    part([-0.08, 0.55, -0.9], [0.16, 0.14, 0.2], [0.0; 3], Limb::Fixed, [WF, WF, WF, WF, WF, T_WOOF_FACE]),
    part([-0.18, 0.0, -0.36], [0.12, 0.44, 0.12], [0.0, 0.44, -0.3], Limb::Swing(1.0), [WF; 6]),
    part([0.06, 0.0, -0.36], [0.12, 0.44, 0.12], [0.0, 0.44, -0.3], Limb::Swing(-1.0), [WF; 6]),
    part([-0.18, 0.0, 0.24], [0.12, 0.44, 0.12], [0.0, 0.44, 0.3], Limb::Swing(-1.0), [WF; 6]),
    part([0.06, 0.0, 0.24], [0.12, 0.44, 0.12], [0.0, 0.44, 0.3], Limb::Swing(1.0), [WF; 6]),
    part([-0.05, 0.55, 0.38], [0.1, 0.1, 0.4], [0.0, 0.6, 0.38], Limb::SwingY(1.5), [WF; 6]),
];
static WOOFER_COLLAR: [Part; 1] = [part([-0.21, 0.52, -0.46], [0.42, 0.1, 0.08], [0.0; 3], Limb::Fixed, [T_COLLAR; 6])];
static FLUFFER_SHEARED: [Part; 6] = [
    part([-0.3, 0.5, -0.45], [0.6, 0.5, 0.9], [0.0; 3], Limb::Fixed, [FS; 6]),
    part([-0.22, 0.75, -0.85], [0.44, 0.45, 0.4], [0.0; 3], Limb::Fixed, [FS, FS, FS, FS, FS, T_FLUFF_FACE]),
    part([-0.3, 0.0, -0.4], [0.18, 0.5, 0.18], [0.0, 0.5, -0.3], Limb::Swing(1.0), [FS; 6]),
    part([0.12, 0.0, -0.4], [0.18, 0.5, 0.18], [0.0, 0.5, -0.3], Limb::Swing(-1.0), [FS; 6]),
    part([-0.3, 0.0, 0.22], [0.18, 0.5, 0.18], [0.0, 0.5, 0.3], Limb::Swing(-1.0), [FS; 6]),
    part([0.12, 0.0, 0.22], [0.18, 0.5, 0.18], [0.0, 0.5, 0.3], Limb::Swing(1.0), [FS; 6]),
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
    }
}

pub fn draw_model(geo: &mut DynGeo, root: &Mat4, parts: &[Part], anim: f32, sky: f32, _player: bool) {
    let swing = anim.sin() * 0.7;
    for p in parts {
        let rot = match p.limb {
            Limb::Fixed => Mat4::IDENTITY,
            Limb::Swing(s) => Mat4::from_rotation_x(swing * s),
            Limb::Forward => Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2 + swing * 0.1),
            Limb::SwingY(s) => Mat4::from_rotation_y(swing * s * 0.5),
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
pub fn draw_armor(geo: &mut DynGeo, root: &Mat4, look: u16, anim: f32, sky: f32) {
    if look == 0 {
        return;
    }
    let tier = |slot: usize| (look >> (slot * 4)) & 0xF;
    let mut parts = Vec::new();
    let mut add = |slot: usize, list: &[ArmorPiece]| {
        let t = tier(slot);
        if t == 0 || t > 4 {
            return;
        }
        for &(min, size, pivot, limb) in list {
            parts.push(part(min, size, pivot, limb, [T_ARMOR_WORN + t - 1; 6]));
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
        let sky = world.sky_light(self.pos.x.floor() as i32, self.pos.y.floor() as i32, self.pos.z.floor() as i32);
        let s = self.size;
        let m = Mat4::from_translation(self.pos - Vec3::splat(s * 0.5)) * Mat4::from_scale(Vec3::splat(s));
        let r = [self.uv[0], self.uv[1], self.uv[0] + 0.25, self.uv[1] + 0.25];
        geo.cube(&m, [self.tile; 6], sky, r);
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
}

impl Arrow {
    pub const SPEED: f32 = 24.0;
    const GRAVITY: f32 = 20.0;

    pub fn new(pos: Vec3, vel: Vec3, shooter: Option<u32>, damage: f32) -> Arrow {
        Arrow { pos, vel, shooter, damage, life: 8.0, stuck: false, dir: vel.normalize_or(Vec3::Z) }
    }

    /// Fly for `dt`. Returns true if it just hit a block (and stuck there).
    pub fn fly(&mut self, dt: f32, world: &World) -> bool {
        self.life -= dt;
        if self.stuck {
            return false;
        }
        self.vel.y -= Self::GRAVITY * dt;
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
    pub fn from_wire(pos: Vec3, v: Vec3) -> Arrow {
        let stuck = v.length() < 0.01;
        Arrow { pos, vel: if stuck { Vec3::ZERO } else { v }, shooter: None, damage: 0.0, life: 1.0, stuck, dir: v.normalize_or(Vec3::Z) }
    }

    pub fn draw(&self, geo: &mut DynGeo, world: &World) {
        let sky = world.sky_light(self.pos.x.floor() as i32, self.pos.y.floor() as i32, self.pos.z.floor() as i32);
        let rot = macroquad::math::Quat::from_rotation_arc(Vec3::Z, self.dir);
        let m = Mat4::from_translation(self.pos) * Mat4::from_quat(rot) * Mat4::from_translation(Vec3::new(-0.03, -0.03, -0.55)) * Mat4::from_scale(Vec3::new(0.06, 0.06, 0.6));
        geo.cube(&m, [T_PLANKS; 6], sky, [0.0, 0.0, 0.25, 0.25]);
        let tip = Mat4::from_translation(self.pos) * Mat4::from_quat(rot) * Mat4::from_translation(Vec3::new(-0.04, -0.04, 0.0)) * Mat4::from_scale(Vec3::new(0.08, 0.08, 0.1));
        geo.cube(&tip, [T_STONE; 6], sky, [0.0, 0.0, 0.25, 0.25]);
    }
}

pub struct PrimedTnt {
    pub pos: Vec3,
    pub fuse: f32,
}
