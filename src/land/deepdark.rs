//! The Deep Dark: wide, quiet caverns of deepslate near the bottom of the
//! world, carpeted in **sculk** that listens.
//!
//! - **Sculk Sensors** click (and give off a little Zappy power) when
//!   something happens near them: footsteps (sneak to walk silently), blocks
//!   broken or placed, things dying, explosions.
//! - A sensor that hears something wakes any **Sculk Shriekers** nearby. Each
//!   shriek is a warning to the closest player, and the darkness closes in
//!   for a moment. The fourth warning summons **The Hush**.
//! - **The Hush** is blind and enormous. It goes wherever it last heard
//!   something, smells anyone within a few blocks, hits for fifteen, and if
//!   it can't reach you it shushes you through the walls. Left in silence
//!   for a minute, it digs back down. It drops a Sculk Catalyst.
//! - **Sculk Catalysts** spread sculk around anything that dies near them.
//! - Deep in the caverns stand the **Hushed City**s: deepslate halls, soul
//!   lanterns, chests of Echo Shards, Scorchite Upgrade Templates and
//!   enchanted books, a great frame of unbreakable stone, and rubble hiding
//!   the Hushed Ones' relics (see archaeology.rs). Tiptoe.

use crate::block::*;
use crate::noise::{hash3, Rng};
use crate::structures::Site;
use crate::world::Generator;
use macroquad::math::{ivec3, IVec3, Vec3};

/// The Deep Dark is below this height, in its regions.
pub const DEEP_TOP: i32 = 24;
/// The Hushed City's floor.
pub const CITY_Y: i32 = 10;
/// How far a sensor hears, and how far a sensor's alarm reaches shriekers.
pub const HEAR: i32 = 8;
/// How far the Hush hears.
pub const HUSH_HEARS: f32 = 18.0;
/// Shrieks it takes to summon the Hush (the last summons it).
pub const WARNINGS: u8 = 4;

impl Generator {
    /// Is (x, z) over a Deep Dark region?
    pub fn deep_dark(&self, x: i32, z: i32) -> bool {
        self.cavern.noise3(x as f32 / 160.0, 300.5, z as f32 / 160.0) > 0.22
    }

    /// The Deep Dark's big open caverns.
    pub fn deep_cavern(&self, x: i32, y: i32, z: i32) -> bool {
        (3..DEEP_TOP - 4).contains(&y) && self.cavern.noise3(x as f32 / 28.0, y as f32 / 11.0 + 40.0, z as f32 / 28.0) > 0.12
    }

    /// Sculk on the floor here?
    pub fn sculk_patch(&self, x: i32, y: i32, z: i32) -> bool {
        self.cavern.noise3(x as f32 / 9.0, y as f32 / 9.0 + 90.0, z as f32 / 9.0) > -0.05
    }
}

/// What sits on a sculk floor, from a 0..1 roll.
pub fn sculk_feature(r: f32) -> Option<Id> {
    if r < 0.025 {
        Some(SCULK_SENSOR)
    } else if r < 0.031 {
        Some(SCULK_SHRIEKER)
    } else if r < 0.034 {
        Some(SCULK_CATALYST)
    } else {
        None
    }
}

/// The Hushed City: a deepslate plaza in a cleared dome, a great frame in the
/// middle, halls round it with chests and soul lanterns, and sculk creeping
/// over everything.
pub fn city_blocks(site: &Site) -> Vec<(IVec3, Id)> {
    let o = site.origin;
    let s = site.seed;
    let mut out = Vec::new();
    const R: i32 = 17;
    for x in -R..=R {
        for z in -R..=R {
            let d2 = x * x + z * z;
            if d2 > R * R {
                continue;
            }
            let r = hash3(s, x, 0, z);
            // The plaza: tiles with sculk creeping over them, on a deepslate base.
            out.push((ivec3(o.x + x, o.y - 2, o.z + z), DEEPSLATE));
            out.push((ivec3(o.x + x, o.y - 1, o.z + z), if r < 0.25 { SCULK } else { DEEPSLATE_TILES }));
            // A dome of open air.
            let roof = 9 - d2 / 48;
            for y in 0..=roof.max(3) {
                out.push((ivec3(o.x + x, o.y + y, o.z + z), AIR));
            }
            // Things on the floor.
            let f = hash3(s ^ 0xF00, x, 0, z);
            if d2 > 25 && !(x.abs() <= 5 && z.abs() <= 2) {
                if f < 0.012 {
                    out.push((ivec3(o.x + x, o.y, o.z + z), SCULK_SENSOR));
                } else if f < 0.02 {
                    out.push((ivec3(o.x + x, o.y, o.z + z), SCULK_SHRIEKER));
                } else if f < 0.04 && r >= 0.25 {
                    // Rubble with relics in it.
                    out.push((ivec3(o.x + x, o.y, o.z + z), if f < 0.03 { SUSPICIOUS_GRAVEL } else { GRAVEL }));
                }
            }
        }
    }
    // The great frame: unbreakable, and nobody knows what it's for.
    for x in -4..=4i32 {
        for y in 0..=6 {
            let edge = x.abs() == 4 || y == 0 || y == 6;
            out.push((ivec3(o.x + x, o.y + y, o.z), if edge { REINFORCED_DEEPSLATE } else { AIR }));
        }
    }
    // Four halls, one on each side.
    for (k, (hx, hz)) in [(11i32, 0i32), (-11, 0), (0, 11), (0, -11)].into_iter().enumerate() {
        for x in -3..=3i32 {
            for z in -3..=3i32 {
                for y in 0..=4 {
                    let wall = x.abs() == 3 || z.abs() == 3;
                    let door = (x == 0 || z == 0) && y <= 2 && (x.abs() == 3 || z.abs() == 3) && (hx.signum() == -x.signum() || hz.signum() == -z.signum());
                    let id = if y == 4 {
                        DEEPSLATE_TILES
                    } else if wall && !door {
                        if hash3(s ^ 0xB, hx + x, y, hz + z) < 0.15 { COBBLED_DEEPSLATE } else { DEEPSLATE_BRICKS }
                    } else {
                        AIR
                    };
                    out.push((ivec3(o.x + hx + x, o.y + y, o.z + hz + z), id));
                }
            }
        }
        // A chest against the back wall, and a lantern.
        let back = ivec3(hx.signum() * 2, 0, hz.signum() * 2);
        let side = if hx == 0 { ivec3(2, 0, 0) } else { ivec3(0, 0, 2) };
        out.push((o + ivec3(hx, 0, hz) + back + side, CHEST));
        out.push((o + ivec3(hx, 3, hz) - side, SOUL_LANTERN));
        if k % 2 == 0 {
            out.push((o + ivec3(hx, 0, hz) + back - side, SUSPICIOUS_GRAVEL));
        }
    }
    // Lanterns on posts round the plaza.
    for k in 0..8 {
        let a = k as f32 * std::f32::consts::FRAC_PI_4 + 0.4;
        let (x, z) = ((a.cos() * 7.0) as i32, (a.sin() * 7.0) as i32);
        out.push((ivec3(o.x + x, o.y, o.z + z), DEEPSLATE_BRICKS));
        out.push((ivec3(o.x + x, o.y + 1, o.z + z), SOUL_LANTERN));
    }
    out
}

// ------------------------------------------------------------------ in the game

use crate::entity::MobKind;
use crate::game::Game;
use crate::sound::Sfx;

impl Game {
    /// Something made a noise at `at` (where the world lives): sensors hear
    /// it, shriekers may shriek, the Hush comes looking. `who`: the player
    /// responsible, if any (their warnings go up).
    pub fn vibrate(&mut self, at: Vec3, who: Option<u32>) {
        if self.is_client() || self.menu {
            return;
        }
        // The Hush hears everything.
        for m in self.mobs.iter_mut().filter(|m| m.kind == MobKind::Hush) {
            if m.body.pos.distance(at) < HUSH_HEARS {
                m.goal = Some(at);
                m.fuse = 0.0;
            }
        }
        if at.y > (DEEP_TOP + 40) as f32 {
            return;
        }
        let c = at.floor().as_ivec3();
        let mut sensors = Vec::new();
        for dy in -HEAR..=HEAR {
            for dz in -HEAR..=HEAR {
                for dx in -HEAR..=HEAR {
                    let p = c + ivec3(dx, dy, dz);
                    if self.world.get_v(p) == SCULK_SENSOR && p.as_vec3().distance(at) <= HEAR as f32 {
                        sensors.push(p);
                    }
                }
            }
        }
        for p in sensors {
            self.world.set_v(p, SCULK_SENSOR_ACTIVE);
            self.sensors_on.insert(p, 1.5);
            self.sfx(Sfx::Sculk, Some(p.as_vec3() + Vec3::splat(0.5)));
            self.wake_shriekers(p, who);
        }
    }

    /// A sensor at `p` heard something: shriekers within earshot of it shriek.
    fn wake_shriekers(&mut self, p: IVec3, who: Option<u32>) {
        if self.shriek_cd > 0.0 {
            return;
        }
        let mut found = None;
        for dy in -HEAR..=HEAR {
            for dz in -HEAR..=HEAR {
                for dx in -HEAR..=HEAR {
                    let q = p + ivec3(dx, dy, dz);
                    if self.world.get_v(q) == SCULK_SHRIEKER {
                        found = Some(q);
                    }
                }
            }
        }
        let Some(q) = found else { return };
        self.shriek_cd = 10.0;
        self.sfx(Sfx::Shriek, Some(q.as_vec3() + Vec3::splat(0.5)));
        // Darkness for everyone nearby.
        let at = q.as_vec3();
        if self.player.body.pos.distance(at) < 40.0 && !self.dedicated {
            self.darkness = self.darkness.max(12.0);
        }
        let near: Vec<u32> = self.peers.iter().filter(|(_, p)| p.target.distance(at) < 40.0).map(|(&id, _)| id).collect();
        for id in near {
            self.net_send_to(id, crate::net::Msg::Darkness { secs: 12.0 });
        }
        // Only the Deep Dark's own shriekers call it up (not ones people put down).
        let wild = q.y < DEEP_TOP && self.world.generator.deep_dark(q.x, q.z);
        let warned = who.unwrap_or(self.my_id);
        let w = self.warnings.entry(warned).or_insert((0, 0.0));
        w.0 += 1;
        w.1 = 600.0;
        let level = w.0;
        let to_me = warned == self.my_id && !self.dedicated;
        let line = match level {
            1 => "A shrieker shrieks. Something, far below, stirs.",
            2 => "Another shriek. Something is listening. Tiptoe.",
            3 => "The shrieking is getting closer. Something is coming up.",
            _ => "IT'S HERE.",
        };
        if to_me {
            self.msg(line);
        } else {
            self.system_message(Some(warned), line);
        }
        let hush_near = self.mobs.iter().any(|m| m.kind == MobKind::Hush && m.body.pos.distance(at) < 48.0);
        if level >= WARNINGS && wild && !hush_near && self.rules.difficulty.monsters() {
            self.warnings.remove(&warned);
            if let Some(spot) = crate::entity::warp_spot(&self.world, at, 6.0, &mut self.rng) {
                self.alloc_mob(MobKind::Hush, spot);
                if let Some(m) = self.mobs.last_mut() {
                    m.goal = Some(at);
                }
                self.sfx(Sfx::Roar, Some(spot));
                self.smoke(spot + Vec3::Y, 30, 1.0);
                if to_me {
                    self.advance("it_listens");
                }
            }
        }
    }

    /// Timers: sensors switch back off, warnings fade, footsteps are heard.
    pub fn deep_dark_tick(&mut self, dt: f32) {
        self.darkness = (self.darkness - dt).max(0.0);
        if self.is_client() || self.menu {
            return;
        }
        self.shriek_cd = (self.shriek_cd - dt).max(0.0);
        let mut off = Vec::new();
        for (p, t) in self.sensors_on.iter_mut() {
            *t -= dt;
            if *t <= 0.0 {
                off.push(*p);
            }
        }
        for p in off {
            self.sensors_on.remove(&p);
            if self.world.get_v(p) == SCULK_SENSOR_ACTIVE {
                self.world.set_v(p, SCULK_SENSOR);
            }
        }
        for w in self.warnings.values_mut() {
            w.1 -= dt;
        }
        self.warnings.retain(|_, w| w.1 > 0.0);
        // Footsteps: walking (not sneaking, not flying) on the ground.
        self.step_timer -= dt;
        if self.step_timer > 0.0 {
            return;
        }
        self.step_timer = 0.6;
        let mut steps: Vec<(Vec3, Option<u32>)> = Vec::new();
        if !self.dedicated && self.dead.is_none() && !self.spectator {
            let b = &self.player.body;
            let moving = Vec3::new(b.vel.x, 0.0, b.vel.z).length() > 1.0;
            if moving && b.on_ground && !self.player.sneaking && !self.player.flying {
                steps.push((b.pos, Some(self.my_id)));
            }
        }
        for (&id, p) in &self.peers {
            let moved = p.target.distance(p.last_step) > 0.5;
            if moved && p.alive() && p.flags & crate::net::FLAG_SNEAK == 0 && p.flags & crate::net::FLAG_GHOST == 0 {
                steps.push((p.target, Some(id)));
            }
        }
        for p in self.peers.values_mut() {
            p.last_step = p.target;
        }
        for (at, who) in steps {
            self.vibrate(at, who);
        }
    }

    /// Something died at `at`: catalysts nearby spread sculk around it.
    pub fn sculk_spread(&mut self, at: Vec3, amount: u32) {
        if self.is_client() || amount == 0 {
            return;
        }
        let c = at.floor().as_ivec3();
        let mut catalyst = false;
        'find: for dy in -8..=8 {
            for dz in -8..=8 {
                for dx in -8..=8 {
                    if self.world.get_v(c + ivec3(dx, dy, dz)) == SCULK_CATALYST {
                        catalyst = true;
                        break 'find;
                    }
                }
            }
        }
        if !catalyst {
            return;
        }
        let mut rng = Rng::new((at.x * 31.0 + at.z * 17.0 + self.clock * 7.0) as u64);
        for _ in 0..amount.min(20) * 2 {
            let p = c + ivec3(rng.int(-3, 3), rng.int(-2, 1), rng.int(-3, 3));
            let id = self.world.get_v(p);
            let open = !is_solid(self.world.get_v(p + IVec3::Y));
            if open && matches!(id, STONE | DEEPSLATE | DIRT | GRASS | COBBLE | COBBLED_DEEPSLATE | GRAVEL | SAND) {
                self.world.set_v(p, SCULK);
                if rng.chance(0.06) && self.world.get_v(p + IVec3::Y) == AIR {
                    self.world.set_v(p + IVec3::Y, SCULK_SENSOR);
                }
            }
        }
        self.sfx(Sfx::Sculk, Some(at));
    }

    /// The Hush shushed someone: a ring of sound particles and a lot of pain.
    pub fn shush(&mut self, from: Vec3, target_id: u32, target: Vec3) {
        self.sfx(Sfx::Shush, Some(from));
        let d = target - from;
        for k in 0..12 {
            let p = from + d * (k as f32 / 12.0);
            self.smoke(p, 2, 0.15);
        }
        if target_id == self.my_id {
            // Straight through armour (and walls).
            self.hurt_player_from(10.0, "was shushed by The Hush. Rude, but effective", Some(from), true);
        } else {
            self.hurt_peer(target_id, 10.0, "was shushed by The Hush. Rude, but effective", d.normalize_or_zero() * 4.0 + Vec3::Y * 3.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structures::Kind;

    #[test]
    fn deep_dark_regions_and_a_city() {
        let g = Generator::new(77);
        let deep = (0..200).flat_map(|z| (0..200).map(move |x| (x * 8, z * 8))).filter(|&(x, z)| g.deep_dark(x, z)).count();
        assert!(deep > 1000 && deep < 30000, "{deep} of 40000 columns are deep dark");
        let city = (-120..120).flat_map(|cz| (-120..120).map(move |cx| (cx, cz))).find_map(|(cx, cz)| g.site(cx, cz).filter(|s| s.kind == Kind::HushedCity));
        let city = city.expect("a city somewhere");
        let blocks = g.site_blocks(&city);
        assert_eq!(blocks.iter().filter(|b| b.1 == CHEST).count(), 4);
        assert!(blocks.iter().any(|b| b.1 == REINFORCED_DEEPSLATE));
        assert!(blocks.iter().any(|b| b.1 == SUSPICIOUS_GRAVEL));
        assert!(blocks.iter().any(|b| b.1 == SCULK_SHRIEKER));
        assert_eq!(sculk_feature(0.01), Some(SCULK_SENSOR));
        assert_eq!(sculk_feature(0.5), None);
    }
}
