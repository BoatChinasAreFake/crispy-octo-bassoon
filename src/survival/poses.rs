//! Shared cosmetic skeleton. Physics, consumption, and inventory remain authoritative elsewhere.
use crate::{
    block::*,
    entity::{Limb, Part},
    game::Game,
    multiplayer::Peer,
    net::*,
    render::{DynGeo, Pass},
    world::World,
};
use macroquad::prelude::*;
use std::{
    collections::BTreeMap,
    f32::consts::{PI, TAU},
};

pub const MEAL_SECONDS: f32 = 1.25;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Meal {
    pub item: Id,
    pub elapsed: f32,
}
pub fn drink(item: Id) -> bool {
    matches!(crate::potions::potion_of(item), Some((_, false))) || crate::bees::Flavour::of_item(item).is_some() || matches!(item, OMINOUS_BOTTLE)
}
pub fn edible(item: Id) -> bool {
    food_value(item).is_some() || drink(item) || crate::cooking::Dish::of_item(item).is_some() || matches!(item, STEW | PUFFER | COOKED_PUFFER | CAKE)
}
impl Meal {
    pub fn valid(self) -> bool {
        edible(self.item) && self.elapsed.is_finite() && (0.0..MEAL_SECONDS).contains(&self.elapsed)
    }
    pub fn weight(self) -> f32 {
        (self.elapsed / 0.14).clamp(0.0, 1.0) * ((MEAL_SECONDS - self.elapsed) / 0.22).clamp(0.0, 1.0)
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Motion {
    pub held: Id,
    pub offhand: Id,
    pub off_swing: f32,
    pub blocking: bool,
    pub bow: f32,
    pub climb: bool,
    pub hurt: f32,
    pub velocity: Vec3,
    pub pitch: f32,
    pub crouch: bool,
    pub swim: bool,
    pub fly: bool,
    pub glide: bool,
    pub grounded: bool,
    pub swing: f32,
}
impl Default for Motion {
    fn default() -> Self {
        Self {
            held: AIR,
            offhand: AIR,
            off_swing: 0.0,
            blocking: false,
            bow: 0.0,
            climb: false,
            hurt: 0.0,
            velocity: Vec3::ZERO,
            pitch: 0.0,
            crouch: false,
            swim: false,
            fly: false,
            glide: false,
            grounded: true,
            swing: 0.0,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pose {
    pub lean: f32,
    pub bank: f32,
    pub lower: f32,
    pub torso: f32,
    pub head: f32,
    pub arms: [Vec3; 2],
    pub elbows: [f32; 2],
    pub thighs: [f32; 2],
    pub knees: [f32; 2],
    pub wings: f32,
}
impl Pose {
    fn blend(&mut self, to: Self, k: f32) {
        for (a, b) in [
            (&mut self.lean, to.lean),
            (&mut self.bank, to.bank),
            (&mut self.lower, to.lower),
            (&mut self.torso, to.torso),
            (&mut self.head, to.head),
            (&mut self.wings, to.wings),
        ] {
            *a += (b - *a) * k;
        }
        for i in 0..2 {
            self.arms[i] = self.arms[i].lerp(to.arms[i], k);
            self.elbows[i] += (to.elbows[i] - self.elbows[i]) * k;
            self.thighs[i] += (to.thighs[i] - self.thighs[i]) * k;
            self.knees[i] += (to.knees[i] - self.knees[i]) * k;
        }
    }
    pub fn root(self) -> Mat4 {
        Mat4::from_translation(Vec3::Y * (-self.lower + 0.9))
            * Mat4::from_rotation_z(self.bank)
            * Mat4::from_rotation_x(self.lean)
            * Mat4::from_translation(-Vec3::Y * 0.9)
    }
    pub(crate) fn torso(self) -> Mat4 {
        around(Vec3::Y * 0.75, Mat4::from_rotation_x(self.torso))
    }
    /// One articulated joint, measured in the original skin's coordinates.
    pub fn bone(self, slot: usize, lower: bool) -> Mat4 {
        match slot {
            0 | 1 => {
                let x = if slot == 0 { -0.125 } else { 0.125 };
                let upper = around(Vec3::new(x, 0.75, 0.0), Mat4::from_rotation_x(self.thighs[slot]));
                upper * if lower { around(Vec3::new(x, 0.375, 0.0), Mat4::from_rotation_x(self.knees[slot])) } else { Mat4::IDENTITY }
            }
            3 | 4 => {
                let i = slot - 3;
                let x = if i == 0 { -0.375 } else { 0.375 };
                let a = self.arms[i];
                let upper =
                    self.torso() * around(Vec3::new(x, 1.45, 0.0), Mat4::from_rotation_y(a.y) * Mat4::from_rotation_z(a.z) * Mat4::from_rotation_x(a.x));
                upper * if lower { around(Vec3::new(x, 1.125, 0.0), Mat4::from_rotation_x(self.elbows[i])) } else { Mat4::IDENTITY }
            }
            5 => self.torso() * around(Vec3::Y * 1.5, Mat4::from_rotation_x(self.head)),
            _ => self.torso(),
        }
    }
}
fn around(p: Vec3, r: Mat4) -> Mat4 {
    Mat4::from_translation(p) * r * Mat4::from_translation(-p)
}
#[derive(Clone, Debug)]
pub struct Animator {
    pub pose: Pose,
    pub phase: f32,
    pub meal: Option<Meal>,
    grounded: bool,
    fall_speed: f32,
    landing: f32,
}
impl Default for Animator {
    fn default() -> Self {
        Self { pose: Pose::default(), phase: 0.0, meal: None, grounded: true, fall_speed: 0.0, landing: 0.0 }
    }
}
impl Animator {
    pub fn start(&mut self, meal: Meal) {
        if meal.valid() {
            self.meal = Some(meal);
        }
    }
    pub fn tick(&mut self, dt: f32, input: Motion) {
        let dt = if dt.is_finite() { dt.clamp(0.0, 0.25) } else { 0.0 };
        if dt == 0.0 {
            return;
        }
        if let Some(m) = &mut self.meal {
            m.elapsed += dt;
            if m.elapsed >= MEAL_SECONDS {
                self.meal = None;
            }
        }
        let v = if input.velocity.is_finite() { input.velocity.clamp_length_max(40.0) } else { Vec3::ZERO };
        let speed = Vec2::new(v.x, v.z).length();
        let aquatic = input.swim && !input.fly && !input.glide;
        let climbing = input.climb && !input.fly && !input.glide && !aquatic;
        let airborne = input.fly || input.glide || aquatic || climbing;
        if input.grounded && !self.grounded && !airborne {
            self.landing = (-self.fall_speed / 15.0).clamp(0.0, 0.8);
        }
        self.grounded = input.grounded;
        self.fall_speed = v.y;
        self.landing *= (-12.0 * dt).exp();
        self.phase = (self.phase
            + dt * if climbing {
                v.y.abs() * 3.0
            } else if aquatic {
                2.6 + speed * 0.5
            } else if airborne {
                2.5
            } else {
                speed * 2.4
            })
        .rem_euclid(TAU);
        let s = self.phase.sin();
        let pitch = if input.pitch.is_finite() { input.pitch.clamp(-1.3, 1.3) } else { 0.0 };
        let mut target = Pose { head: pitch, ..Default::default() };
        let walk = (speed / 3.0).clamp(0.0, 1.0);
        target.thighs = [s * 0.65 * walk, -s * 0.65 * walk];
        target.arms = [Vec3::X * (-s * 0.6 * walk), Vec3::X * (s * 0.6 * walk)];
        if input.crouch && !airborne {
            target.lower = 0.13;
            target.torso = -0.28;
            target.head = pitch + 0.22;
            target.thighs = [0.65 + s * 0.25 * walk, 0.65 - s * 0.25 * walk];
            target.knees = [-1.2, -1.2];
        }
        if !input.grounded && !airborne {
            target.knees = [-0.25, -0.4];
            target.thighs = [0.2, -0.15];
        }
        if aquatic {
            let moving = (speed / 1.7).clamp(0.0, 1.0);
            target.lean = (-PI * 0.5 + pitch * 0.45) * moving;
            target.head = pitch * 0.4;
            target.lower = 0.03 * s;
            target.bank = (-v.x * 0.035).clamp(-0.18, 0.18) * moving;
            for i in 0..2 {
                let sign = if i == 0 { -1.0 } else { 1.0 };
                let stroke = (self.phase + if i == 0 { 0.0 } else { PI }).sin();
                target.arms[i] = Vec3::new(0.35 + moving * (1.15 + stroke), 0.0, sign * (0.42 - 0.24 * moving));
                target.elbows[i] = 0.3 + 0.35 * stroke.abs();
                target.thighs[i] = stroke * (0.12 + 0.25 * moving);
                target.knees[i] = -0.2 - stroke.max(0.0) * 0.3;
            }
        } else if input.glide {
            target.lean = -PI * 0.5 + pitch * 0.6;
            target.bank = (-v.x * 0.025).clamp(-0.3, 0.3);
            target.head = pitch * 0.4;
            target.arms = [Vec3::new(-0.1, 0.0, -0.28), Vec3::new(-0.1, 0.0, 0.28)];
            target.thighs = [s * 0.04, -s * 0.04];
            target.knees = [-0.12, -0.12];
            target.wings = 1.0;
        } else if input.fly {
            target.lean = (v.z * 0.035).clamp(-0.35, 0.35);
            target.bank = (-v.x * 0.035).clamp(-0.28, 0.28);
            target.lower = s * 0.025;
            let spread = 0.35 + (-v.y / 15.0).clamp(-0.1, 0.35);
            target.arms = [Vec3::new(0.1, 0.0, -spread), Vec3::new(0.1, 0.0, spread)];
            target.thighs = [0.13 + s * 0.06, 0.13 - s * 0.06];
            target.knees = [-0.28 - (v.y / 30.0).clamp(-0.15, 0.2); 2];
        }
        if !airborne {
            let bend = self.landing * 0.4;
            target.lower += 0.75 * (1.0 - bend.cos());
            for i in 0..2 {
                target.thighs[i] += bend;
                target.knees[i] -= bend * 2.0;
            }
        }
        if climbing {
            target.torso = -0.08;
            target.head = pitch * 0.5;
            for i in 0..2 {
                let stroke = if i == 0 { s } else { -s };
                target.arms[i] = Vec3::new(2.45 + stroke * 0.35, 0.0, if i == 0 { -0.12 } else { 0.12 });
                target.elbows[i] = 0.45 + stroke * 0.2;
                target.thighs[i] = 0.65 - stroke * 0.35;
                target.knees[i] = -0.8 + stroke * 0.4;
            }
        } else {
            if input.held != AIR {
                target.arms[1].x = target.arms[1].x * 0.35 + 0.22;
                target.elbows[1] = 0.25;
            }
            if input.offhand != AIR {
                target.arms[0].x = target.arms[0].x * 0.35 + 0.22;
                target.elbows[0] = 0.25;
            }
            if input.swing.is_finite() && input.swing > 0.0 {
                let hit = (input.swing.clamp(0.0, 1.0) * PI).sin();
                target.arms[1].x += hit * if is_spear(input.held) { 1.05 } else { 1.65 };
                target.arms[1].y -= hit * 0.35;
                target.elbows[1] += hit * 0.45;
                target.bank += hit * 0.07;
            }
            if input.off_swing.is_finite() && input.off_swing > 0.0 {
                let w = (input.off_swing.clamp(0.0, 1.0) * PI).sin();
                target.arms[0].x += w * 1.3;
                target.elbows[0] += w * 0.4;
            }
            if input.blocking {
                let i = if input.held == SHIELD { 1 } else { 0 };
                target.arms[i] = Vec3::new(1.1, if i == 0 { -0.55 } else { 0.55 }, 0.0);
                target.elbows[i] = 1.0;
            }
            if input.held == BOW && input.bow.is_finite() && input.bow > 0.0 {
                let w = input.bow.clamp(0.0, 1.0);
                target.arms[1] = Vec3::new(1.45 + pitch * 0.7, -0.3, 0.0);
                target.elbows[1] = 0.08;
                target.arms[0] = Vec3::new(1.25 + pitch * 0.7, -0.6 - w * 0.15, -0.35);
                target.elbows[0] = 0.65 + w * 0.95;
                target.torso = -0.08;
            }
        }
        if input.hurt.is_finite() && input.hurt > 0.0 {
            let w = (input.hurt / 0.6).clamp(0.0, 1.0) * 0.16;
            target.bank += w;
            target.torso += w;
        }
        if let Some(meal) = self.meal {
            let w = meal.weight();
            let sip = drink(meal.item);
            let bite = (meal.elapsed * 19.0).sin();
            let arm = Vec3::new(if sip { 1.25 } else { 1.05 + bite * 0.1 }, 0.60, 0.0);
            target.arms[1] = target.arms[1].lerp(arm, w);
            target.elbows[1] += (1.65 - target.elbows[1]) * w;
            target.head += if sip { 0.18 * w } else { 0.07 * w * bite };
        }
        self.pose.blend(target, 1.0 - (-14.0 * dt).exp());
    }
}
fn relative(v: Vec3, yaw: f32) -> Vec3 {
    Mat3::from_rotation_y(yaw) * v
}
fn immersed(world: &World, p: Vec3) -> bool {
    is_water(world.get_v((p + Vec3::Y * 0.9).floor().as_ivec3()))
}
pub fn tick_peers(peers: &mut BTreeMap<u32, Peer>, world: &World, dt: f32) {
    for p in peers.values_mut() {
        let before = p.pos;
        let delta = p.target - before;
        let teleport = delta.length() > 8.0;
        p.pos = if teleport { p.target } else { before + delta * (dt * 12.0).clamp(0.0, 1.0) };
        let velocity = if teleport { Vec3::ZERO } else { (p.pos - before) / dt.max(1e-4) };
        crate::entity::step_anim(&mut p.anim, Vec2::new(velocity.x, velocity.z).length(), dt, 2.4);
        if !p.alive() || p.flags & FLAG_SLEEP != 0 {
            p.animation = Animator::default();
            continue;
        }
        if teleport {
            p.animation = Animator::default();
        }
        p.animation.tick(
            dt,
            Motion {
                velocity: relative(velocity, p.yaw),
                pitch: p.pitch,
                crouch: p.flags & FLAG_SNEAK != 0,
                swim: immersed(world, p.pos),
                fly: p.flags & FLAG_FLY != 0,
                glide: p.flags & FLAG_GLIDE != 0,
                grounded: is_solid(world.get_v((p.pos - Vec3::Y * 0.08).floor().as_ivec3())),
                swing: {
                    if (p.flags & FLAG_SWING != 0 || p.gesture & 2 != 0) && p.swing < 0.3 {
                        p.swing = 1.0;
                    }
                    p.swing = (p.swing - dt * 4.0).max(0.0);
                    if p.flags & FLAG_SWING != 0 { p.swing } else { 0.0 }
                },
                off_swing: if p.gesture & 2 != 0 { p.swing } else { 0.0 },
                held: p.held,
                offhand: p.offhand,
                blocking: p.gesture & 1 != 0,
                bow: p.draw as f32 / 255.0,
                climb: climb_at(world, p.pos),
                hurt: if p.flags & FLAG_HURT != 0 { 0.4 } else { 0.0 },
            },
        );
    }
}
fn climb_at(world: &World, pos: Vec3) -> bool {
    let min = pos - Vec3::new(0.3, 0.0, 0.3);
    let max = pos + Vec3::new(0.3, 1.8, 0.3);
    crate::carpentry::on_ladder(world, min, max) || crate::masonry::in_scaffolding(world, min, max)
}
impl Game {
    pub(crate) fn tick_player_pose(&mut self, dt: f32) {
        if self.menu || self.dead.is_some() || self.sleeping.is_some() || self.spectator || self.rocket_pilot().is_some() {
            self.player.animation = Animator::default();
            return;
        }
        self.off_swing = (self.off_swing - dt.max(0.0) * 4.0).max(0.0);
        let p = &self.player;
        let input = Motion {
            velocity: relative(p.body.vel, p.yaw),
            pitch: p.pitch,
            crouch: p.sneaking,
            swim: immersed(&self.world, p.body.pos),
            fly: p.flying,
            glide: p.gliding,
            grounded: p.body.on_ground,
            swing: p.swing,
            held: self.inv.held(),
            offhand: self.inv.offhand.map_or(AIR, |s| s.0),
            off_swing: self.off_swing,
            blocking: self.blocking,
            bow: self.bow_draw,
            climb: climb_at(&self.world, p.body.pos),
            hurt: p.hurt,
        };
        self.player.animation.tick(dt, input);
    }
    pub(crate) fn animate_consumption(&mut self, item: Id) {
        let meal = Meal { item, elapsed: 0.0 };
        if !meal.valid() {
            return;
        }
        self.player.animation.start(meal);
        self.net_send_msg(Msg::MealAnimation { id: self.my_id, item, elapsed: 0.0 });
    }
}
/// Draw each skin/armour part through the same skeleton, splitting at elbow/knee.
pub fn draw_parts(g: &mut DynGeo, root: &Mat4, parts: &[Part], pose: Pose, sky: f32) {
    for p in parts {
        let min = Vec3::from_array(p.min);
        let size = Vec3::from_array(p.size);
        let mid = min + size * 0.5;
        let slot = match p.limb {
            Limb::Swing(_) if p.pivot[1] > 1.0 => {
                if mid.x < 0.0 {
                    3
                } else {
                    4
                }
            }
            Limb::Swing(_) => {
                if mid.x < 0.0 {
                    0
                } else {
                    1
                }
            }
            _ => {
                if mid.y > 1.5 {
                    5
                } else {
                    2
                }
            }
        };
        let split = match slot {
            0 | 1 => Some(0.375),
            3 | 4 => Some(1.125),
            _ => None,
        };
        let draw = |g: &mut DynGeo, lo: Vec3, hi: Vec3, lower: bool| {
            let frame = *root * pose.bone(slot, lower) * Mat4::from_translation(lo) * Mat4::from_scale(hi - lo);
            let v0 = (min.y + size.y - hi.y) / size.y;
            let v1 = (min.y + size.y - lo.y) / size.y;
            // Side UVs retain their original vertical ranges across each bend.
            for (f, (_, corners, shade)) in crate::mesher::FACES.iter().enumerate() {
                let c = corners.map(|v| frame.transform_point3(Vec3::from_array(v)));
                let uv = if f == 2 || f == 3 { [0.0, 0.0, 1.0, 1.0] } else { [0.0, v0, 1.0, v1] };
                g.quad(c, p.tiles[f], uv, [*shade, sky]);
            }
        };
        if let Some(y) = split {
            if min.y < y && min.y + size.y > y {
                draw(g, min, Vec3::new(min.x + size.x, y, min.z + size.z), true);
                draw(g, Vec3::new(min.x, y, min.z), min + size, false);
            } else {
                draw(g, min, min + size, min.y + size.y <= y);
            }
        } else {
            draw(g, min, min + size, false);
        }
    }
}
pub fn draw_held(g: &mut DynGeo, root: &Mat4, a: &Animator, held: Id, offhand: Id, blocking: bool, sky: f32) {
    for (i, item) in [(0, offhand), (1, held)] {
        if item == AIR || (i == 1 && a.meal.is_some()) {
            continue;
        }
        let x = if i == 0 { -0.375 } else { 0.375 };
        let bone = a.pose.bone(i + 3, true);
        let grip = Vec3::new(x, 0.80, -0.10);
        // A shield and a bow stay upright while their grip follows the wrist.
        let hand = if matches!(item, SHIELD | BOW) {
            *root * Mat4::from_translation(bone.transform_point3(grip))
        } else {
            *root * bone * Mat4::from_translation(grip)
        };
        g.begin(Pass::Opaque, [1.0; 4], false);
        if crate::models::drawn_as_block(item) {
            let frame = hand * Mat4::from_scale(Vec3::splat(0.28)) * Mat4::from_translation(Vec3::splat(-0.5));
            for ((lo, size), tiles, uv) in crate::models::item_parts(item) {
                let m = frame * Mat4::from_translation(Vec3::from_array(lo)) * Mat4::from_scale(Vec3::from_array(size));
                g.cube(&m, tiles, sky, uv);
            }
        } else {
            // Tools sit diagonally across the palm; the handle extends below it.
            let tool = is_sword(item) || pick_tier(item) > 0 || crate::tools::tool_uses(item).is_some();
            let frame = hand
                * Mat4::from_rotation_y(if item == SHIELD && blocking { 0.0 } else { PI * 0.5 })
                * Mat4::from_rotation_z(if tool { -0.65 } else { 0.0 })
                * Mat4::from_translation(Vec3::new(0.0, if tool { 0.15 } else { 0.0 }, 0.0));
            crate::game::extruded_sprite(
                g,
                &frame,
                crate::models::flat_tile(item),
                if item == SHIELD {
                    0.7
                } else if item == BOW {
                    0.65
                } else if is_spear(item) {
                    0.9
                } else if tool {
                    0.65
                } else {
                    0.4
                },
                sky,
            );
        }
    }
}
pub fn draw_meal(g: &mut DynGeo, root: &Mat4, a: &Animator, sky: f32) {
    let Some(meal) = a.meal else { return };
    let w = meal.weight();
    if w <= 0.01 {
        return;
    }
    let at = a.pose.bone(4, true).transform_point3(Vec3::new(0.375, 0.76, -0.05));
    let hand = *root * Mat4::from_translation(at + Vec3::new(0.0, 0.02, -0.09)) * Mat4::from_rotation_x(if drink(meal.item) { 0.65 } else { 0.0 });
    let frame = hand * Mat4::from_translation(Vec3::new(-0.11, -0.05, -0.03)) * Mat4::from_scale(Vec3::new(0.22, 0.22, 0.045));
    g.begin(Pass::Opaque, [1.0; 4], false);
    g.cube(&frame, [crate::models::flat_tile(meal.item); 6], sky, [0.0, 0.0, 1.0, 1.0]);
    if !drink(meal.item) {
        for i in 0..4 {
            let t = (meal.elapsed * 3.0 + i as f32 * 0.25).fract();
            let at = Vec3::new((i as f32 - 1.5) * 0.045, 1.63 - t * 0.25, -0.31 - t * 0.13);
            let frame = *root * Mat4::from_translation(at) * Mat4::from_scale(Vec3::splat(0.025 * (1.0 - t) * w));
            g.cube(&frame, [crate::models::flat_tile(meal.item); 6], sky, [0.0, 0.0, 1.0, 1.0]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn settle(m: Motion) -> Animator {
        let mut a = Animator::default();
        for _ in 0..90 {
            a.tick(1.0 / 60.0, m);
        }
        a
    }
    #[test]
    fn poses_blend_and_distinguish_swimming_flying_and_crouching() {
        let crouch = Motion { crouch: true, ..Default::default() };
        let mut a = Animator::default();
        a.tick(1.0 / 60.0, crouch);
        assert!(a.pose.lower > 0.0 && a.pose.lower < 0.13);
        assert!(a.pose.knees[0] < 0.0 && a.pose.knees[0] > -1.2);
        let crouch = settle(crouch);
        assert!((crouch.pose.lower - 0.13).abs() < 0.001);
        assert!(crouch.pose.torso < -0.25);
        let swim = settle(Motion { swim: true, grounded: false, velocity: Vec3::new(0.0, 0.0, -2.5), ..Default::default() });
        assert!(swim.pose.lean < -1.4);
        assert!((swim.pose.arms[0].x - swim.pose.arms[1].x).abs() > 0.1);
        let tread = settle(Motion { swim: true, grounded: false, ..Default::default() });
        assert!(tread.pose.lean.abs() < 0.05);
        assert!(tread.pose.arms[0].z.abs() > 0.3);
        let fly = |y| settle(Motion { fly: true, grounded: false, velocity: Vec3::new(3.0, y, -6.0), ..Default::default() });
        let (up, down) = (fly(8.0), fly(-8.0));
        assert!(up.pose.lean < -0.1 && up.pose.bank < -0.05);
        assert!(down.pose.arms[1].z > up.pose.arms[1].z + 0.2);
        let glide = settle(Motion { glide: true, grounded: false, velocity: Vec3::new(2.0, 0.0, -10.0), ..Default::default() });
        assert!(glide.pose.wings > 0.99 && glide.pose.lean < -1.4);
        let mut blend = crouch;
        let before = blend.pose.lean;
        blend.tick(1.0 / 60.0, Motion { swim: true, grounded: false, velocity: -Vec3::Z * 3.0, ..Default::default() });
        assert!(blend.pose.lean < before && blend.pose.lean > -1.0);
    }
    #[test]
    fn combat_grips_and_climbing_override_walk_without_detaching_hands() {
        let idle = settle(Motion::default());
        let shield = settle(Motion { held: SWORD_IRON, offhand: SHIELD, blocking: true, ..Default::default() });
        assert!(shield.pose.arms[0].x > 1.0 && shield.pose.elbows[0] > 0.9);
        assert!(shield.pose.arms[1].x > idle.pose.arms[1].x);
        let hurt = settle(Motion { hurt: 0.4, ..Default::default() });
        assert!(hurt.pose.bank > 0.1 && hurt.pose.torso > 0.1);
        let bow = settle(Motion { held: BOW, bow: 1.0, ..Default::default() });
        assert!(bow.pose.arms[1].x > 1.4 && bow.pose.elbows[0] > 1.5);
        let climbing = settle(Motion { climb: true, grounded: false, velocity: Vec3::Y * 3.0, ..Default::default() });
        assert!(climbing.pose.arms.iter().all(|a| a.x > 2.0));
        assert!(climbing.pose.knees[0] < -0.3);
        let mut hang = climbing;
        let phase = hang.phase;
        for _ in 0..30 {
            hang.tick(1.0 / 60.0, Motion { climb: true, grounded: false, ..Default::default() });
        }
        assert_eq!(phase, hang.phase);
        assert!(hang.pose.arms[0].x > 2.0);
        let place = settle(Motion { held: PICK_IRON, offhand: STONE, off_swing: 0.5, ..Default::default() });
        assert!(place.pose.arms[0].x > 1.4 && place.pose.arms[1].x < 0.3);
        let swing = settle(Motion { held: SWORD_IRON, swing: 0.5, ..Default::default() });
        assert!(swing.pose.arms[1].x > 1.5);
        for _ in 0..120 {
            hang.tick(1.0 / 60.0, Motion::default());
        }
        assert!(hang.pose.arms[0].length() < 0.01);
    }
    #[test]
    fn skeleton_joints_keep_limbs_attached_and_crouched_feet_on_the_floor() {
        let p = settle(Motion { crouch: true, ..Default::default() }).pose;
        for slot in [0, 1, 3, 4] {
            let joint = if slot < 2 {
                Vec3::new(if slot == 0 { -0.125 } else { 0.125 }, 0.375, 0.0)
            } else {
                Vec3::new(if slot == 3 { -0.375 } else { 0.375 }, 1.125, 0.0)
            };
            assert!(p.bone(slot, false).transform_point3(joint).distance(p.bone(slot, true).transform_point3(joint)) < 0.0001);
        }
        let foot = (p.root() * p.bone(0, true)).transform_point3(Vec3::new(-0.125, 0.0, 0.0));
        assert!(foot.y.abs() < 0.01, "foot {foot:?}");
        let mut sixty = Animator::default();
        let mut thirty = Animator::default();
        let m = Motion { crouch: true, ..Default::default() };
        for _ in 0..60 {
            sixty.tick(1.0 / 60.0, m);
        }
        for _ in 0..30 {
            thirty.tick(1.0 / 30.0, m);
        }
        assert!((sixty.pose.lower - thirty.pose.lower).abs() < 0.0001);
    }
    #[test]
    fn meals_are_finite_short_lived_and_keep_the_consumed_item() {
        let mut a = Animator::default();
        a.start(Meal { item: BREAD, elapsed: 0.0 });
        for _ in 0..20 {
            a.tick(0.016, Motion::default());
        }
        assert_eq!(a.meal.unwrap().item, BREAD);
        assert!(a.pose.arms[1].x > 0.5 && a.pose.elbows[1] > 1.0);
        assert!(!drink(BREAD));
        assert!(drink(crate::potions::potion_item(crate::potions::Potion::Healing, false)));
        let msg = Msg::MealAnimation { id: 7, item: BREAD, elapsed: 0.3 };
        assert_eq!(Msg::decode(&msg.encode()).unwrap(), msg);
        for elapsed in [f32::NAN, -0.1, MEAL_SECONDS, f32::INFINITY] {
            assert!(Msg::decode(&Msg::MealAnimation { id: 7, item: BREAD, elapsed }.encode()).is_err());
        }
        a.start(Meal { item: DIAMOND, elapsed: 0.0 });
        assert_eq!(a.meal.unwrap().item, BREAD);
        for _ in 0..100 {
            a.tick(0.016, Motion::default());
        }
        assert!(a.meal.is_none());
        a.tick(f32::NAN, Motion::default());
        a.tick(0.016, Motion { velocity: Vec3::splat(f32::NAN), pitch: f32::NAN, swing: f32::INFINITY, ..Default::default() });
        assert!(a.pose.root().is_finite());
    }
    #[test]
    fn jumping_and_landing_settle_without_permanent_crouching() {
        let mut a = Animator::default();
        a.tick(0.1, Motion { grounded: false, velocity: -Vec3::Y * 12.0, ..Default::default() });
        a.tick(0.016, Motion::default());
        assert!(a.pose.lower > 0.0);
        assert!(a.pose.knees[0] < 0.0);
        for _ in 0..180 {
            a.tick(1.0 / 60.0, Motion::default());
        }
        assert!(a.pose.lower.abs() < 0.001 && a.pose.knees[0].abs() < 0.001);
    }
}

#[cfg(test)]
mod render_tests {
    use super::*;
    #[test]
    fn articulated_skins_and_armour_have_finite_geometry_with_preserved_uvs() {
        let mut a = Animator::default();
        a.start(Meal { item: BREAD, elapsed: 0.1 });
        for _ in 0..20 {
            a.tick(0.016, Motion { crouch: true, ..Default::default() });
        }
        for parts in [crate::nametags::SKIN_MODELS[0], crate::skins::model(0)] {
            let mut g = DynGeo::default();
            g.begin(Pass::Opaque, [1.0; 4], false);
            draw_parts(&mut g, &a.pose.root(), &parts, a.pose, 1.0);
            assert_eq!(g.mesh.verts.len(), 10 * 24); // four limbs split, torso and head intact
            assert!(g.mesh.verts.iter().all(|v| v.pos.iter().all(|v| v.is_finite()) && v.uv.iter().all(|v| v.is_finite())));
            let root = a.pose.root();
            crate::entity::draw_player_armor(&mut g, &root, 0x4444, 0x1234, 1.0, a.pose);
            draw_meal(&mut g, &root, &a, 1.0);
            assert!(g.mesh.verts.iter().all(|v| v.pos.iter().all(|v| v.is_finite())));
        }
    }
}

#[cfg(test)]
mod realm_tests {
    use super::*;
    #[test]
    fn remote_swimming_reads_its_own_realms_water_and_teleports_reset_blending() {
        let mut g = Game::new(787, true, false);
        let at = Vec3::new(8.0, 90.0, 8.0);
        g.world.load_now(0, 0);
        for y in 90..=92 {
            g.world.set(8, y, 8, WATER);
            g.world.set(8, y, 7, WATER);
        }
        let mut wet = Peer::new("Wet".into(), at);
        wet.target = at - Vec3::Z * 0.3;
        g.peers.insert(71, wet);
        g.in_realm(crate::dims::Dim::Scorch, |g| {
            g.world.load_now(0, 0);
            for y in 90..=92 {
                g.world.set(8, y, 8, AIR);
                g.world.set(8, y, 7, AIR);
            }
            let mut dry = Peer::new("Dry".into(), at);
            dry.target = at - Vec3::Z * 0.3;
            g.peers.insert(72, dry);
        });
        g.net_receive(0.016);
        assert!(g.peers[&71].animation.pose.lean < -0.1);
        g.in_realm(crate::dims::Dim::Scorch, |g| assert!(g.peers[&72].animation.pose.lean.abs() < 0.001));
        g.peers.get_mut(&71).unwrap().target += Vec3::X * 100.0;
        g.net_receive(0.016);
        assert!(g.peers[&71].animation.pose.lean.abs() < 0.001);
        assert!(g.peers[&71].animation.pose.root().is_finite());
    }
}
