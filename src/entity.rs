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
}

impl MobKind {
    pub fn name(self) -> &'static str {
        match self {
            MobKind::Oinker => "Oinker",
            MobKind::Hisser => "Hisser",
            MobKind::Groaner => "Groaner",
        }
    }
    fn dims(self) -> (f32, f32) {
        match self {
            MobKind::Oinker => (0.45, 0.9),
            MobKind::Hisser => (0.3, 1.65),
            MobKind::Groaner => (0.3, 1.95),
        }
    }
    fn max_health(self) -> f32 {
        match self {
            MobKind::Oinker => 10.0,
            MobKind::Hisser => 20.0,
            MobKind::Groaner => 20.0,
        }
    }
    pub fn hostile(self) -> bool {
        self != MobKind::Oinker
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
}

pub enum MobEvent {
    HurtPlayer(f32, &'static str),
    Explode(Vec3, f32, &'static str),
    Smoke(Vec3),
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
        if self.kind == MobKind::Oinker {
            self.flee = 4.0;
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
            MobKind::Oinker => {
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
            _ => None,
        }
    }

    pub fn draw(&self, geo: &mut DynGeo, world: &World) {
        let flash = self.kind == MobKind::Hisser && self.fuse > 0.0 && (self.fuse * 14.0).sin() > 0.0;
        let tint = if self.hurt > 0.3 {
            [1.0, 0.45, 0.45, 1.0]
        } else if flash {
            [3.0, 3.0, 3.0, 1.0]
        } else if self.burning {
            [1.0, 0.8, 0.6, 1.0]
        } else {
            [1.0; 4]
        };
        let p = self.body.pos;
        let sky = world.sky_light(p.x.floor() as i32, (p.y + 0.5).floor() as i32, p.z.floor() as i32);
        geo.begin(Pass::Opaque, tint, false);
        let swell = 1.0 + self.fuse * 0.08;
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
pub static STOVE: [Part; 6] = humanoid(T_SKIN, T_STOVE_FACE, T_STOVE_SHIRT, T_STOVE_PANTS, Limb::Swing(-1.0), Limb::Swing(1.0));

fn model(kind: MobKind) -> &'static [Part] {
    match kind {
        MobKind::Oinker => &OINKER,
        MobKind::Hisser => &HISSER,
        MobKind::Groaner => &GROANER,
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
