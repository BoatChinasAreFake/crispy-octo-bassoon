//! Nicer skies: **shooting stars** on clear nights, **auroras** rippling
//! over the snow, and a **rainbow** when the rain stops in daylight.
//!
//! All of it is just a sight, so each player's game makes its own.
//!
//! The moon's phases: full, waning, new and waxing again over eight nights.
//!
//! The world counts its days (`Game::day`, kept with the world and sent to
//! joined players with the time of day). The moon is drawn in its phase, a
//! full moon brings out more monsters, and eggs (Sniffer eggs, turtle eggs)
//! hatch faster under it, as they do in the game this parodies.

use crate::game::Game;
use crate::render::{DynGeo, Pass};
use crate::texture::T_WHITE;
use crate::world::Biome;
use macroquad::math::Vec3;

/// How long a shooting star and a rainbow last (seconds).
const STAR_LIFE: f32 = 0.9;
pub const RAINBOW_SECS: f32 = 90.0;
/// Shooting stars a second, on average, on a clear night.
const STAR_RATE: f32 = 0.12;
/// How far off the sky is drawn.
const SKY: f32 = 140.0;

pub struct ShootingStar {
    pub dir: Vec3,
    pub vel: Vec3,
    pub age: f32,
}

/// Rainbow bands, outside in.
const BANDS: [[u8; 3]; 6] = [[230, 60, 60], [240, 150, 50], [240, 230, 70], [80, 200, 90], [70, 120, 230], [140, 80, 200]];

/// A flat quad facing the eye at `centre` (a point in the sky).
fn spot(g: &mut DynGeo, centre: Vec3, dir: Vec3, size: f32, rgb: [u8; 3]) {
    let u = Vec3::Y.cross(dir).normalize_or(Vec3::X);
    let v = dir.cross(u);
    let c = [centre - u * size - v * size, centre + u * size - v * size, centre + u * size + v * size, centre - u * size + v * size];
    g.quad_tinted(c, T_WHITE, [1.0, 1.0], rgb);
}

impl Game {
    /// Shooting stars come and go; a rainbow follows the rain.
    pub fn skies_tick(&mut self, dt: f32) {
        if self.dedicated || self.elsewhere() {
            return;
        }
        let wet = self.weather.kind.wet();
        if self.was_wet && !wet && self.daylight() > 0.5 {
            self.rainbow = RAINBOW_SECS;
        }
        self.was_wet = wet;
        self.rainbow = (self.rainbow - dt).max(0.0);
        for s in self.shooting.iter_mut() {
            s.age += dt;
            s.dir = (s.dir + s.vel * dt).normalize();
        }
        self.shooting.retain(|s| s.age < STAR_LIFE);
        if self.night_sky() > 0.6 && !wet && self.rng.chance(dt * STAR_RATE) {
            let r = &mut self.rng;
            let dir = Vec3::new(r.range(-1.0, 1.0), r.range(0.35, 0.9), r.range(-1.0, 1.0)).normalize();
            let across = Vec3::new(r.range(-1.0, 1.0), r.range(-0.6, -0.1), r.range(-1.0, 1.0));
            let vel = (across - dir * across.dot(dir)).normalize_or(Vec3::X) * r.range(0.5, 0.9);
            self.shooting.push(ShootingStar { dir, vel, age: 0.0 });
        }
    }

    /// How dark the sky is (0 by day, 1 at night).
    pub fn night_sky(&self) -> f32 {
        1.0 - ((self.daylight() - 0.18) / 0.5).clamp(0.0, 1.0)
    }

    /// Is there an aurora over where we are (a clear night in the snow)?
    pub fn aurora_here(&self) -> bool {
        let p = self.player.body.pos;
        self.night_sky() > 0.5 && !self.weather.kind.wet() && !self.elsewhere() && matches!(self.world.generator.column(p.x.floor() as i32, p.z.floor() as i32).1, Biome::Snowy | Biome::Taiga)
    }

    /// Draw the shooting stars, any aurora and any rainbow.
    pub fn draw_skies(&self, g: &mut DynGeo, eye: Vec3, sun_dir: Vec3) {
        if self.elsewhere() {
            return;
        }
        let night = self.night_sky();
        if !self.shooting.is_empty() {
            g.begin(Pass::Sky, [1.0, 1.0, 1.0, night], true);
            for s in &self.shooting {
                let fade = 1.0 - s.age / STAR_LIFE;
                for k in 0..10 {
                    let back = (s.dir - s.vel * k as f32 * 0.025).normalize();
                    let size = (0.5 - k as f32 * 0.04) * fade;
                    let c = (255.0 * (1.0 - k as f32 * 0.07)) as u8;
                    spot(g, eye + back * SKY, back, size.max(0.05), [c, c, 255]);
                }
            }
        }
        if self.aurora_here() {
            // Curtains across the northern sky, waving slowly.
            let t = self.clock;
            let strength = (night - 0.5) * 2.0;
            g.begin(Pass::Sky, [1.0, 1.0, 1.0, 0.35 * strength], true);
            let strips = 48;
            for i in 0..strips {
                let a0 = -1.1 + 2.2 * i as f32 / strips as f32;
                let a1 = -1.1 + 2.2 * (i + 1) as f32 / strips as f32;
                let wave = |a: f32| (a * 3.0 + t * 0.3).sin() * 0.12 + (a * 7.0 - t * 0.5).sin() * 0.05;
                let at = |a: f32, up: f32| {
                    let d = Vec3::new(a.sin() + wave(a), up, -a.cos()).normalize();
                    eye + d * SKY
                };
                for (lo, hi, rgb) in [(0.25, 0.5, [70, 230, 140]), (0.5, 0.8, [150, 90, 220])] {
                    let pulse = 0.7 + 0.3 * (t * 0.7 + i as f32 * 0.4).sin();
                    let c = [(rgb[0] as f32 * pulse) as u8, (rgb[1] as f32 * pulse) as u8, (rgb[2] as f32 * pulse) as u8];
                    g.quad_tinted([at(a0, lo), at(a1, lo), at(a1, hi), at(a0, hi)], T_WHITE, [1.0, 1.0], c);
                }
            }
        }
        if self.rainbow > 0.0 {
            // An arc round the point opposite the sun.
            let fade = (self.rainbow / 10.0).min(1.0) * (1.0 - night).max(0.0);
            g.begin(Pass::Sky, [1.0, 1.0, 1.0, 0.55 * fade], true);
            let anti = -sun_dir;
            let u = anti.cross(Vec3::Y).normalize_or(Vec3::X);
            let v = u.cross(anti);
            let segments = 40;
            for (b, rgb) in BANDS.iter().enumerate() {
                let (r0, r1) = (0.74 - b as f32 * 0.018, 0.74 - (b + 1) as f32 * 0.018);
                let point = |r: f32, t: f32| (anti * r.cos() + (u * t.cos() + v * t.sin()) * r.sin()).normalize();
                for k in 0..segments {
                    let (t0, t1) = (std::f32::consts::PI * k as f32 / segments as f32, std::f32::consts::PI * (k + 1) as f32 / segments as f32);
                    let quad = [point(r0, t0), point(r0, t1), point(r1, t1), point(r1, t0)];
                    if quad.iter().all(|d| d.y < -0.02) {
                        continue;
                    }
                    g.quad_tinted(quad.map(|d| eye + d * SKY), T_WHITE, [1.0, 1.0], *rgb);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shooting_stars_at_night_and_a_rainbow_after_rain() {
        let mut g = crate::game::tests::arena(201);
        g.time = 0.25;
        for _ in 0..2000 {
            g.skies_tick(0.05);
        }
        assert!(g.shooting.is_empty(), "none by day");
        g.time = 0.75;
        let mut seen = false;
        for _ in 0..4000 {
            g.skies_tick(0.05);
            seen |= !g.shooting.is_empty();
        }
        assert!(seen, "some at night");
        // Rain, then clear skies in the daytime: a rainbow.
        g.time = 0.3;
        g.weather.kind = crate::weather::Weather::Rain;
        g.skies_tick(0.05);
        g.weather.kind = crate::weather::Weather::Clear;
        g.skies_tick(0.05);
        assert!(g.rainbow > RAINBOW_SECS - 1.0);
        let mut geo = DynGeo::default();
        g.draw_skies(&mut geo, Vec3::new(0.0, 60.0, 0.0), Vec3::new(1.0, 0.5, 0.2).normalize());
        assert!(!geo.mesh.idx.is_empty(), "the rainbow is drawn");
    }
}

// ---- moon

/// How many phases, and which is which.
pub const PHASES: u32 = 8;
pub const NAMES: [&str; 8] = ["Full Moon", "Waning Gibbous", "Last Quarter", "Waning Crescent", "New Moon", "Waxing Crescent", "First Quarter", "Waxing Gibbous"];

/// How lit the moon's face is in phase `p` (1 full, 0 new).
pub fn brightness(p: u32) -> f32 {
    let a = (p % PHASES) as f32 / PHASES as f32 * std::f32::consts::TAU;
    (1.0 + a.cos()) * 0.5
}

/// Monsters at once, as a share of the usual: more at full moon, fewer at new.
pub fn monster_scale(p: u32) -> f32 {
    0.75 + 0.5 * brightness(p)
}

/// How much more often eggs hatch.
pub fn hatch_scale(p: u32) -> f32 {
    0.5 + brightness(p)
}

impl Game {
    /// Tonight's phase.
    pub fn moon_phase(&self) -> u32 {
        self.day % PHASES
    }

    /// The clock moved from `before` to `self.time`: past midnight-into-morning
    /// (time wraps to 0 at sunrise), it's a new day.
    pub fn count_days(&mut self, before: f32) {
        if self.time < before {
            self.day = self.day.wrapping_add(1);
        }
    }

    /// The time of day for joined players, with the day count in front
    /// (`day + fraction`, so older games still read the time).
    pub fn time_msg(&self) -> crate::net::Msg {
        crate::net::Msg::Time((self.day % 4096) as f32 + self.time)
    }
}

#[cfg(test)]
mod moon_tests {
    use super::*;

    #[test]
    fn the_moon_goes_round_and_days_are_counted() {
        assert_eq!(brightness(0), 1.0);
        assert!(brightness(4) < 0.001);
        assert!(monster_scale(0) > monster_scale(4));
        assert!(hatch_scale(0) > 1.0 && hatch_scale(4) < 1.0);
        let mut g = crate::game::Game::new(1, false, false);
        g.time = 0.99;
        let before = g.time;
        g.time = 0.01;
        g.count_days(before);
        assert_eq!(g.day, 1);
        assert_eq!(g.moon_phase(), 1);
        let crate::net::Msg::Time(t) = g.time_msg() else { panic!() };
        assert!((t - 1.01).abs() < 1e-4);
    }
}
