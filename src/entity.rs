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
    pub in_water: bool,
}

impl Body {
    pub fn new(pos: Vec3, half: f32, height: f32) -> Self {
        Body { pos, vel: Vec3::ZERO, half, height, on_ground: false, hit_wall: false, in_water: false }
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

fn collides(world: &World, min: Vec3, max: Vec3) -> Option<(i32, i32, i32)> {
    const E: f32 = 1e-4;
    for y in (min.y + E).floor() as i32..=(max.y - E).floor() as i32 {
        for z in (min.z + E).floor() as i32..=(max.z - E).floor() as i32 {
            for x in (min.x + E).floor() as i32..=(max.x - E).floor() as i32 {
                if is_solid(world.get(x, y, z)) {
                    return Some((x, y, z));
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
        let Some((x, y, z)) = collides(world, b.min(), b.max()) else { break };
        blocked = true;
        let cell = [x, y, z][axis] as f32;
        let (neg, pos_ext) = if axis == 1 { (0.0, b.height) } else { (b.half, b.half) };
        if d > 0.0 {
            b.pos[axis] = cell - pos_ext - 1e-3;
        } else {
            b.pos[axis] = cell + 1.0 + neg + 1e-3;
        }
    }
    if blocked {
        b.vel[axis] = 0.0;
    }
    blocked
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
        for axis in [0, 2] {
            let before = b.pos;
            if move_axis(world, b, axis, sd[axis]) {
                b.hit_wall = true;
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
    b.in_water = feet == WATER;
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MobKind {
    Oinker,
    Hisser,
    Groaner,
    Fluffer,
    Starer,
}

impl MobKind {
    /// Every kind, in wire/script index order (append only).
    pub const ALL: [MobKind; 5] = [MobKind::Oinker, MobKind::Hisser, MobKind::Groaner, MobKind::Fluffer, MobKind::Starer];

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
        }
    }
    fn dims(self) -> (f32, f32) {
        match self {
            MobKind::Oinker => (0.45, 0.9),
            MobKind::Hisser => (0.3, 1.65),
            MobKind::Groaner => (0.3, 1.95),
            MobKind::Fluffer => (0.45, 1.25),
            MobKind::Starer => (0.3, 2.9),
        }
    }
    fn max_health(self) -> f32 {
        match self {
            MobKind::Oinker => 10.0,
            MobKind::Hisser => 20.0,
            MobKind::Groaner => 20.0,
            MobKind::Fluffer => 8.0,
            MobKind::Starer => 40.0,
        }
    }
    /// Spawns at night / in caves and counts toward the hostile cap.
    /// (Starers are only hostile once provoked, but they keep monster hours.)
    pub fn hostile(self) -> bool {
        !matches!(self, MobKind::Oinker | MobKind::Fluffer)
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
    /// Starer: provoked (looked at or hit). Synced as `fuse > 0` for clients.
    pub angry: bool,
    /// Starer: seconds until it may teleport again.
    pub warp_cd: f32,
}

pub enum MobEvent {
    HurtPlayer(f32, &'static str),
    Explode(Vec3, f32, &'static str),
    Smoke(Vec3),
    /// A Starer blinked from one place to another.
    Warp(Vec3, Vec3),
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
            if is_solid(world.get(x, y - 1, z)) && (0..3).all(|h| !is_solid(world.get(x, y + h, z)) && world.get(x, y + h, z) != WATER) {
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
            angry: false,
            warp_cd: 0.0,
        }
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
            MobKind::Oinker | MobKind::Fluffer => self.flee = 4.0,
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
        let to_player = player - self.body.pos;
        let dist = to_player.length();
        let flat = Vec3::new(to_player.x, 0.0, to_player.z);

        let mut want: Option<(f32, f32)> = None; // (yaw, speed)
        match self.kind {
            MobKind::Oinker | MobKind::Fluffer => {
                if self.flee > 0.0 {
                    want = Some(((-flat.x).atan2(flat.z), 3.5));
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
                let head = self.eye();
                self.burning = daylight > 0.65 && world.sky_light(head.x.floor() as i32, head.y.floor() as i32, head.z.floor() as i32) >= 1.0 && !self.body.in_water;
                if self.burning {
                    self.health -= dt * 1.5;
                    if rng.chance(dt * 8.0) {
                        ev.push(MobEvent::Smoke(head));
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
        }
        if want.is_none() {
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
        }
        let prev_ground = self.body.on_ground;
        move_body(world, &mut self.body, dt, false);
        if moving && self.body.hit_wall && (self.body.on_ground || prev_ground) {
            self.body.vel.y = 8.8;
        }
        let spd = Vec3::new(self.body.vel.x, 0.0, self.body.vel.z).length();
        self.anim += spd * dt * 5.0;
        if self.body.pos.y < -20.0 {
            self.health = -100.0;
        }
        ev
    }

    /// Items dropped on death.
    pub fn loot(&self, rng: &mut Rng) -> Option<(u8, u8)> {
        let n = rng.int(0, 2) as u8;
        match self.kind {
            MobKind::Oinker => Some((PORKCHOP, n.max(1))),
            MobKind::Hisser if n > 0 && self.health > -50.0 => Some((GUNPOWDER, n)),
            MobKind::Groaner if n > 0 => Some((GOO, n)),
            MobKind::Fluffer => Some((WOOL, n.max(1))),
            MobKind::Starer if n > 0 => Some((PEARL, 1)),
            _ => None,
        }
    }

    /// Anything dropped besides `loot` (Fluffers also give Baa-con).
    pub fn extra_loot(&self, rng: &mut Rng) -> Option<(u8, u8)> {
        match self.kind {
            MobKind::Fluffer if rng.chance(0.7) => Some((MUTTON, 1)),
            _ => None,
        }
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
        } else if self.angry {
            [1.6, 0.9, 1.8, 1.0]
        } else {
            [1.0; 4]
        };
        let mut p = self.body.pos;
        if self.angry {
            // Angry Starers vibrate with rage.
            p.x += (self.anim * 37.0 + self.id as f32).sin() * 0.03;
            p.z += (self.anim * 29.0).cos() * 0.03;
        }
        let sky = world.sky_light(p.x.floor() as i32, (p.y + 0.5).floor() as i32, p.z.floor() as i32);
        geo.begin(Pass::Opaque, tint, false);
        let swell = if self.kind == MobKind::Hisser { 1.0 + self.fuse * 0.08 } else { 1.0 };
        let root = Mat4::from_translation(p) * Mat4::from_rotation_y(-self.yaw) * Mat4::from_scale(Vec3::splat(swell));
        draw_model(geo, &root, model(self.kind), self.anim, sky, false);
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
}

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

fn model(kind: MobKind) -> &'static [Part] {
    match kind {
        MobKind::Oinker => &OINKER,
        MobKind::Hisser => &HISSER,
        MobKind::Groaner => &GROANER,
        MobKind::Fluffer => &FLUFFER,
        MobKind::Starer => &STARER,
    }
}

pub fn draw_model(geo: &mut DynGeo, root: &Mat4, parts: &[Part], anim: f32, sky: f32, _player: bool) {
    let swing = anim.sin() * 0.7;
    for p in parts {
        let rot = match p.limb {
            Limb::Fixed => Mat4::IDENTITY,
            Limb::Swing(s) => Mat4::from_rotation_x(swing * s),
            Limb::Forward => Mat4::from_rotation_x(std::f32::consts::FRAC_PI_2 + swing * 0.1),
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

pub struct PrimedTnt {
    pub pos: Vec3,
    pub fuse: f32,
}
