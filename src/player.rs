//! First-person controller: walking, sprinting, sneaking, swimming and flying.

use crate::block::*;
use crate::entity::{move_body, Body, GRAVITY};
use crate::world::World;
use macroquad::math::Vec3;

pub const EYE: f32 = 1.62;
pub const MAX_HEALTH: f32 = 20.0;

pub struct Input {
    pub forward: f32,
    pub strafe: f32,
    pub jump: bool,
    pub jump_pressed: bool,
    pub sneak: bool,
    pub sprint: bool,
}

pub struct Player {
    pub body: Body,
    pub yaw: f32,
    pub pitch: f32,
    pub flying: bool,
    pub health: f32,
    pub hurt: f32,
    pub fall_start: f32,
    pub sprinting: bool,
    pub sneaking: bool,
    pub bob: f32,
    pub last_jump_press: f64,
    /// Food, saturation and exhaustion (see hunger.rs).
    pub hunger: crate::hunger::Hunger,
    pub swing: f32,
    /// Speed multiplier from the block underfoot (mod blocks can change it).
    pub ground_speed: f32,
    /// Set when a bouncy block launched us (the game plays a sound and clears it).
    pub bounced: bool,
    /// Set on landing to how far we fell (for trampling and hay bales).
    pub landed: Option<f32>,
}

impl Player {
    pub fn new(pos: Vec3) -> Self {
        Player {
            body: Body::new(pos, 0.3, 1.8),
            yaw: 0.0,
            pitch: -0.2,
            flying: false,
            health: MAX_HEALTH,
            hurt: 0.0,
            fall_start: pos.y,
            sprinting: false,
            sneaking: false,
            bob: 0.0,
            last_jump_press: -10.0,
            hunger: Default::default(),
            swing: 0.0,
            ground_speed: 1.0,
            bounced: false,
            landed: None,
        }
    }

    pub fn eye(&self) -> Vec3 {
        let sneak = if self.sneaking && !self.flying { 0.12 } else { 0.0 };
        self.body.pos + Vec3::Y * (EYE - sneak)
    }

    pub fn look_dir(&self) -> Vec3 {
        Vec3::new(self.yaw.sin() * self.pitch.cos(), self.pitch.sin(), -self.yaw.cos() * self.pitch.cos())
    }

    pub fn head_in_water(&self, world: &World) -> bool {
        let e = self.eye();
        is_water(world.get(e.x.floor() as i32, (e.y + 0.05).floor() as i32, e.z.floor() as i32))
    }

    /// Advance physics. Returns fall damage taken, if any.
    pub fn update(&mut self, dt: f32, input: &Input, world: &World, creative: bool) -> f32 {
        self.hurt = (self.hurt - dt).max(0.0);
        self.swing = (self.swing - dt * 4.0).max(0.0);
        let fwd = Vec3::new(self.yaw.sin(), 0.0, -self.yaw.cos());
        let right = Vec3::new(self.yaw.cos(), 0.0, self.yaw.sin());
        let mut wish = fwd * input.forward + right * input.strafe;
        if wish.length_squared() > 1.0 {
            wish = wish.normalize();
        }
        self.sprinting = input.sprint && input.forward > 0.0 && !input.sneak && (creative || self.hunger.can_sprint());
        self.sneaking = input.sneak && !self.flying;

        if creative && input.jump_pressed {
            let now = macroquad::time::get_time();
            if now - self.last_jump_press < 0.3 {
                self.flying = !self.flying;
                self.body.vel.y = 0.0;
            }
            self.last_jump_press = now;
        }
        if !creative {
            self.flying = false;
        }

        let b = &mut self.body;
        if self.flying {
            let speed = if self.sprinting { 22.0 } else { 11.0 };
            let target = wish * speed;
            let k = (dt * 10.0).min(1.0);
            b.vel.x += (target.x - b.vel.x) * k;
            b.vel.z += (target.z - b.vel.z) * k;
            let vy = if input.jump { 9.0 } else if input.sneak { -9.0 } else { 0.0 };
            b.vel.y += (vy - b.vel.y) * k;
            move_body(world, b, dt, false);
            if b.on_ground && !input.jump {
                self.flying = false;
            }
            self.fall_start = b.pos.y;
            return 0.0;
        }

        let in_water = b.in_water;
        let speed = if b.in_lava {
            1.2
        } else if in_water {
            2.6
        } else if self.sneaking {
            1.4
        } else if self.sprinting {
            5.8
        } else {
            4.3
        };
        let target = wish * speed * if b.on_ground { self.ground_speed } else { 1.0 };
        let accel = if b.on_ground || in_water { 14.0 } else { 3.0 };
        let k = (dt * accel).min(1.0);
        b.vel.x += (target.x - b.vel.x) * k;
        b.vel.z += (target.z - b.vel.z) * k;

        if in_water {
            b.vel.y -= GRAVITY * 0.25 * dt;
            b.vel.y *= (1.0 - dt * 2.5).max(0.0);
            if input.jump {
                b.vel.y = (b.vel.y + 22.0 * dt).min(3.5);
            }
        } else {
            b.vel.y = (b.vel.y - GRAVITY * dt).max(-60.0);
            if input.jump && b.on_ground {
                b.vel.y = 8.7;
                if self.sprinting {
                    b.vel.x += fwd.x * 1.5;
                    b.vel.z += fwd.z * 1.5;
                }
                if !creative {
                    self.hunger.exhaust(if self.sprinting { crate::hunger::SPRINT_JUMP } else { crate::hunger::JUMP });
                }
            }
        }
        let was_ground = b.on_ground;
        let falling_speed = -b.vel.y;
        let before = b.pos;
        move_body(world, b, dt, self.sneaking && was_ground);
        if !creative {
            let moved = Vec3::new(b.pos.x - before.x, 0.0, b.pos.z - before.z).length();
            if in_water {
                self.hunger.exhaust(moved * crate::hunger::SWIM_PER_BLOCK);
            } else if self.sprinting {
                self.hunger.exhaust(moved * crate::hunger::SPRINT_PER_BLOCK);
            }
        }
        if b.on_ground {
            let under = world.get(b.pos.x.floor() as i32, (b.pos.y - 0.05).floor() as i32, b.pos.z.floor() as i32);
            let def = block(under);
            self.ground_speed = def.speed;
            // Bouncy blocks: sneak to land softly.
            if def.bounce > 0.0 && falling_speed > 3.0 && !self.sneaking {
                b.vel.y = falling_speed * def.bounce;
                b.on_ground = false;
                self.fall_start = b.pos.y;
                self.bounced = true;
            }
        }
        // Hop out of water onto a ledge.
        if in_water && b.hit_wall && input.jump {
            b.vel.y = 6.0;
        }

        let mut dmg = 0.0;
        if b.in_water {
            self.fall_start = b.pos.y;
        } else if b.on_ground {
            let fall = self.fall_start - b.pos.y;
            if fall > 0.6 {
                self.landed = Some(fall);
            }
            if fall > 3.5 && !creative {
                dmg = (fall - 3.0).floor();
            }
            self.fall_start = b.pos.y;
        } else if b.vel.y > 0.0 || b.pos.y > self.fall_start {
            self.fall_start = b.pos.y;
        }

        let moving = Vec3::new(b.vel.x, 0.0, b.vel.z).length();
        if b.on_ground {
            self.bob += moving * dt * 2.2;
        }
        dmg
    }
}
