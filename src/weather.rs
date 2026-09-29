//! Weather: clear skies, rain (snow where it's cold, nothing in deserts) and
//! thunderstorms. Rain waters crops and makes the fish bite sooner, storms are
//! dark enough for monsters, and lightning strikes near players now and then.
//!
//! The world's owner decides the weather (unless the Weather Cycle rule is
//! off) and tells everyone; sleeping through the night clears it up.

use crate::block::*;
use crate::game::Game;
use crate::net::Msg;
use crate::noise::hash2;
use crate::render::{DynGeo, Pass};
use crate::sound::Sfx;
use crate::texture::T_WHITE;
use crate::world::Biome;
use macroquad::math::{Mat4, Vec3};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Weather {
    Clear,
    Rain,
    Thunder,
}

impl Weather {
    pub fn index(self) -> u8 {
        self as u8
    }

    pub fn from_index(i: u8) -> Weather {
        match i {
            1 => Weather::Rain,
            2 => Weather::Thunder,
            _ => Weather::Clear,
        }
    }

    pub fn wet(self) -> bool {
        self != Weather::Clear
    }
}

pub struct WeatherState {
    pub kind: Weather,
    /// Seconds until the weather changes (the owner's business).
    pub timer: f32,
    /// 0..1, easing toward 1 while it rains (for dimming and drawing).
    pub strength: f32,
    /// Seconds until the next lightning strike.
    pub lightning: f32,
    /// The latest bolt: where it hit, and how long it still shows.
    pub bolt: Option<(Vec3, f32)>,
}

impl Default for WeatherState {
    fn default() -> Self {
        WeatherState { kind: Weather::Clear, timer: 600.0, strength: 0.0, lightning: 10.0, bolt: None }
    }
}

/// How much a storm darkens the day (multiplies daylight).
pub fn dimming(kind: Weather, strength: f32) -> f32 {
    let full = match kind {
        Weather::Clear => 0.0,
        Weather::Rain => 0.35,
        Weather::Thunder => 0.6,
    };
    1.0 - full * strength
}

/// Damage from being struck.
pub const LIGHTNING_DAMAGE: f32 = 5.0;
const LIGHTNING_RADIUS: f32 = 3.0;

impl Game {
    /// Is it raining (or snowing) right here, on this spot under the open sky?
    pub fn rained_on(&self, x: i32, y: i32, z: i32) -> bool {
        self.weather.kind.wet() && !crate::hollow::in_hollow(x as f32) && self.world.sky_light(x, y, z) >= 1.0 && self.world.generator.column(x, z).1 != Biome::Desert
    }

    /// Every side: ease the look in and out; the owner also decides what's next.
    pub fn weather_tick(&mut self, dt: f32) {
        let target = if self.weather.kind.wet() && !self.in_hollow() { 1.0 } else { 0.0 };
        let s = &mut self.weather.strength;
        *s += (target - *s).clamp(-dt * 0.15, dt * 0.15);
        if let Some((_, life)) = &mut self.weather.bolt {
            *life -= dt;
        }
        self.weather.bolt = self.weather.bolt.filter(|b| b.1 > 0.0);
        if self.is_client() {
            return;
        }
        if self.rules.weather_cycle {
            self.weather.timer -= dt;
            if self.weather.timer <= 0.0 {
                let next = match self.weather.kind {
                    Weather::Clear if self.rng.chance(0.3) => Weather::Thunder,
                    Weather::Clear => Weather::Rain,
                    _ => Weather::Clear,
                };
                self.set_weather(next);
            }
        }
        if self.weather.kind == Weather::Thunder {
            self.weather.lightning -= dt;
            if self.weather.lightning <= 0.0 {
                self.weather.lightning = self.rng.range(4.0, 14.0);
                self.random_lightning();
            }
        }
    }

    /// The owner changes the weather (and says so).
    pub fn set_weather(&mut self, kind: Weather) {
        self.weather.kind = kind;
        self.weather.timer = match kind {
            Weather::Clear => self.rng.range(300.0, 900.0),
            _ => self.rng.range(120.0, 300.0),
        };
        self.net_broadcast(Msg::Weather { kind: kind.index() });
    }

    /// A strike somewhere near a random player.
    fn random_lightning(&mut self) {
        let mut near: Vec<Vec3> = self.peers.values().filter(|p| p.alive()).map(|p| p.target).collect();
        if !self.dedicated && !self.menu {
            near.push(self.player.body.pos);
        }
        if near.is_empty() {
            return;
        }
        let c = near[self.rng.int(0, near.len() as i32 - 1) as usize];
        let (x, z) = ((c.x + self.rng.range(-28.0, 28.0)).floor() as i32, (c.z + self.rng.range(-28.0, 28.0)).floor() as i32);
        if !self.world.is_loaded(x, z) || !self.rained_on(x, crate::world::CH - 1, z) {
            return;
        }
        let y = self.world.surface_y(x, z) + 1;
        self.lightning_strike(Vec3::new(x as f32 + 0.5, y as f32, z as f32 + 0.5));
    }

    /// Lightning hits `at` (where the world lives): everyone close gets hurt, TNT goes off.
    pub fn lightning_strike(&mut self, at: Vec3) {
        self.net_broadcast(Msg::Lightning { at });
        self.lightning_effects(at);
        let under = macroquad::math::ivec3(at.x.floor() as i32, at.y.floor() as i32 - 1, at.z.floor() as i32);
        if self.world.get_v(under) == TNT {
            self.world.set_v(under, AIR);
            self.tnts.push(crate::entity::PrimedTnt { pos: under.as_vec3(), fuse: 1.0 });
        }
        // Lightning starts fires.
        let at_cell = under + macroquad::math::IVec3::Y;
        if !self.is_client() {
            self.ignite(at_cell);
        }
        let cause = "was struck by lightning. Statistically impressive";
        if !self.dedicated && self.player.body.pos.distance(at) < LIGHTNING_RADIUS {
            self.player.hurt = 0.0;
            self.hurt_player(LIGHTNING_DAMAGE, cause);
        }
        let hit: Vec<u32> = self.peers.iter().filter(|(_, p)| p.alive() && p.target.distance(at) < LIGHTNING_RADIUS).map(|(id, _)| *id).collect();
        for id in hit {
            self.hurt_peer(id, LIGHTNING_DAMAGE, cause, Vec3::Y * 4.0);
        }
        for m in self.mobs.iter_mut().filter(|m| m.body.pos.distance(at) < LIGHTNING_RADIUS) {
            m.hurt = 0.0;
            m.damage(LIGHTNING_DAMAGE, at);
        }
    }

    /// Every side: the flash and the bang.
    pub fn lightning_effects(&mut self, at: Vec3) {
        self.weather.bolt = Some((at, 0.35));
        self.sfx(Sfx::Thunder, None);
        self.shake = self.shake.max(0.3);
    }

    /// Rain streaks (or snowflakes) falling around the camera, and any bolt.
    pub fn draw_weather(&self, g: &mut DynGeo, eye: Vec3) {
        if let Some((at, life)) = self.weather.bolt {
            g.begin(Pass::Opaque, [3.0, 3.0, 3.5, 1.0], true);
            let seed = (at.x * 13.0 + at.z * 7.0) as i32;
            let mut p = at;
            for k in 0..24 {
                let jag = Vec3::new(hash2(1, seed, k) - 0.5, 0.0, hash2(2, seed, k) - 0.5) * 1.6;
                let next = at + Vec3::Y * ((k + 1) as f32 * 3.0) + jag * (k as f32 * 0.1).min(1.5);
                let d = next - p;
                let rot = macroquad::math::Quat::from_rotation_arc(Vec3::Z, d.normalize_or(Vec3::Z));
                let w = 0.12 * life / 0.35 + 0.05;
                let m = Mat4::from_translation(p) * Mat4::from_quat(rot) * Mat4::from_translation(Vec3::new(-w / 2.0, -w / 2.0, 0.0)) * Mat4::from_scale(Vec3::new(w, w, d.length()));
                g.cube(&m, [T_WHITE; 6], 1.0, [0.0, 0.0, 1.0, 1.0]);
                p = next;
            }
        }
        let strength = self.weather.strength;
        if strength < 0.02 {
            return;
        }
        let (ex, ez) = (eye.x.floor() as i32, eye.z.floor() as i32);
        let snowy = self.world.generator.column(ex, ez).1 == Biome::Snowy;
        let tint = if snowy { [1.0, 1.0, 1.0, 0.85 * strength] } else { [0.7, 0.75, 0.9, 0.45 * strength] };
        g.begin(Pass::Blend, tint, false);
        let t = self.clock;
        let r = 10;
        for dz in -r..=r {
            for dx in -r..=r {
                let (x, z) = (ex + dx, ez + dz);
                if hash2(91, x, z) > 0.35 + 0.4 * strength {
                    continue;
                }
                let biome = self.world.generator.column(x, z).1;
                if biome == Biome::Desert {
                    continue;
                }
                let snow = biome == Biome::Snowy;
                let speed = if snow { 2.0 } else { 14.0 };
                let span = 18.0;
                let phase = hash2(17, x, z) * span;
                let y = eye.y + 9.0 - ((t * speed + phase) % span);
                if self.world.sky_light(x, y.floor() as i32, z) < 1.0 {
                    continue;
                }
                let fx = x as f32 + 0.2 + hash2(3, x, z) * 0.6 + if snow { (t + phase).sin() * 0.3 } else { 0.0 };
                let fz = z as f32 + 0.2 + hash2(5, x, z) * 0.6;
                // Face the camera sideways.
                let to_eye = Vec3::new(eye.x - fx, 0.0, eye.z - fz).normalize_or(Vec3::Z);
                let side = Vec3::new(-to_eye.z, 0.0, to_eye.x);
                let (w, h) = if snow { (0.07, 0.07) } else { (0.025, 1.1) };
                let base = Vec3::new(fx, y, fz);
                let c = [base - side * w, base + side * w, base + side * w + Vec3::Y * h, base - side * w + Vec3::Y * h];
                g.quad(c, T_WHITE, [0.0, 0.0, 1.0, 1.0], [1.0, 1.0]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storms_darken_the_day() {
        assert_eq!(dimming(Weather::Clear, 1.0), 1.0);
        assert!(dimming(Weather::Thunder, 1.0) < dimming(Weather::Rain, 1.0));
        assert_eq!(dimming(Weather::Rain, 0.0), 1.0);
        for w in [Weather::Clear, Weather::Rain, Weather::Thunder] {
            assert_eq!(Weather::from_index(w.index()), w);
        }
    }
}
