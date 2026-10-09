//! The Ocean Monument: a great prismarine building at the bottom of a deep,
//! flooded basin in the sea floor (the seas are shallow; it dug its own).
//!
//! - It's full of water: a pillared hall on the ground floor, a gallery
//!   above, a penthouse on the roof, and two wings. Sea lanterns light it.
//!   In the middle, a dark prismarine core hides eight blocks of gold; in
//!   the penthouse, a chest. Sponges sit in a room upstairs.
//! - **Guardians** keep turning up in its water while anyone is near.
//!   They swim about, and when they see you they lock on: a beam grows
//!   between you for two seconds, then zaps you. Their spikes prick anyone
//!   who hits them in melee (while they're not busy aiming).
//! - Three **Elder Guardians** live in it (the penthouse and the wings).
//!   Every minute they curse everyone nearby with **Mining Fatigue**, which
//!   makes digging four times as slow, so you can't just tunnel to the gold.
//!   They drop sponges.
//! - A **Conduit** (a Heart of the Sea, from buried treasure, and eight
//!   Nautilus Shells, from Soggy Groaners) placed in water inside a frame of
//!   prismarine gives **Conduit Power** to everyone in the water around it:
//!   no drowning. The more prismarine round it (count the blocks in the
//!   5x5x5 around it; 16 at least), the further it reaches. With 42 or more
//!   it also zaps monsters in the water near it.

use crate::block::*;
use crate::entity::MobKind;
use crate::game::Game;
use crate::potions::Potion;
use crate::structures::{Kind, Site};
use macroquad::math::{ivec3, IVec3, Mat4, Quat, Vec3};

/// How far below the sea's surface (at least) a monument's floor is.
pub const DEPTH: i32 = 19;
/// How deep a dip in the sea floor its basin fills with sand.
pub const FILL: i32 = 5;
/// How far out from its middle a monument's basin reaches.
pub const BASIN: i32 = 16;

/// Seconds a Guardian takes to charge its laser.
pub const LASER_SECS: f32 = 2.0;
/// Guardians about a monument at once, and how near someone must be for more to turn up.
const GUARDIANS: usize = 6;
const NEAR: f32 = 48.0;
/// How far Elders' curse reaches, how often, and for how long.
const CURSE_RANGE: f32 = 50.0;
const CURSE_EVERY: f32 = 60.0;
const CURSE_SECS: f32 = 300.0;
/// Prismarine round a Conduit it needs, and the most it can use.
pub const CONDUIT_MIN: usize = 16;
pub const CONDUIT_FULL: usize = 42;

pub fn is_guardian(kind: MobKind) -> bool {
    matches!(kind, MobKind::Guardian | MobKind::ElderGuardian)
}

/// Part of a Conduit's frame.
pub fn is_prismarine(id: Id) -> bool {
    matches!(id, PRISMARINE | PRISMARINE_BRICKS | DARK_PRISMARINE | SEA_LANTERN)
}

/// A monument (see the top of this file), its entrance on the south side.
pub fn monument_blocks(site: &Site) -> Vec<(IVec3, Id)> {
    let o = site.origin;
    let s = site.seed;
    let mut out = Vec::new();
    let mut put = |x: i32, y: i32, z: i32, id: Id| out.push((o + ivec3(x, y, z), id));
    // The basin: flooded up to the sea, with a sandy floor (filling any dips).
    for x in -BASIN..=BASIN {
        for z in -BASIN..=BASIN {
            for y in -FILL..0 {
                put(x, y, z, SAND);
            }
            for y in 0..=crate::world::SEA - o.y {
                put(x, y, z, WATER);
            }
        }
    }
    // A footing, so it doesn't overhang dips in the sea floor, and the base.
    for x in -13..=13 {
        for z in -13..=13 {
            for y in -4..0 {
                put(x, y, z, PRISMARINE);
            }
            put(x, 0, z, PRISMARINE_BRICKS);
        }
    }
    // The body: brick walls with lantern windows, a gallery floor halfway up, water inside.
    for x in -12..=12i32 {
        for z in -12..=12i32 {
            for y in 1..=9 {
                let wall = x.abs() == 12 || z.abs() == 12;
                let id = if wall {
                    if y == 9 {
                        DARK_PRISMARINE
                    } else if y % 4 == 3 && (x + z).rem_euclid(6) == 0 {
                        SEA_LANTERN
                    } else {
                        PRISMARINE_BRICKS
                    }
                } else if y == 5 {
                    PRISMARINE
                } else {
                    WATER
                };
                put(x, y, z, id);
            }
            // The roof, with a dark band round its edge.
            put(x, 10, z, if x.abs() == 12 || z.abs() == 12 { DARK_PRISMARINE } else { PRISMARINE });
        }
    }
    // Ways up through the gallery floor, in the four corners.
    for (cx, cz) in [(-8, -8), (8, -8), (-8, 8), (8, 8)] {
        for x in cx - 1..=cx + 1 {
            for z in cz - 1..=cz + 1 {
                put(x, 5, z, WATER);
            }
        }
    }
    // The wings: walls across the ground floor, with doorways.
    for side in [-6, 6] {
        for z in -11..=11i32 {
            for y in 1..=4 {
                put(side, y, z, if z.abs() <= 1 && y <= 3 { WATER } else { PRISMARINE_BRICKS });
            }
        }
    }
    // Pillars in the hall, topped with lanterns.
    for (x, z) in [(-3, -8), (3, -8), (-3, 8), (3, 8), (-9, -4), (-9, 4), (9, -4), (9, 4)] {
        for y in 1..=3 {
            put(x, y, z, PRISMARINE);
        }
        put(x, 4, z, SEA_LANTERN);
    }
    // The entrance, on the south side.
    for x in -2..=2 {
        for y in 1..=4 {
            put(x, y, 12, WATER);
        }
    }
    // The core: dark prismarine round eight blocks of gold.
    for x in -2..=2i32 {
        for z in -2..=2i32 {
            for y in 1..=4 {
                let shell = x.abs() == 2 || z.abs() == 2 || y == 1 || y == 4;
                let gold = (-1..=0).contains(&x) && (-1..=0).contains(&z) && (2..=3).contains(&y);
                put(x, y, z, if shell { DARK_PRISMARINE } else if gold { GOLD_BLOCK } else { WATER });
            }
        }
    }
    // The penthouse on the roof, open to the gallery below.
    for x in -4..=4i32 {
        for z in -4..=4i32 {
            for y in 11..=15 {
                let wall = x.abs() == 4 || z.abs() == 4;
                let id = if y == 15 {
                    DARK_PRISMARINE
                } else if wall {
                    if y == 13 && (x == 0 || z == 0) { SEA_LANTERN } else { PRISMARINE_BRICKS }
                } else {
                    WATER
                };
                put(x, y, z, id);
            }
        }
    }
    for x in -1..=1 {
        for z in -1..=1 {
            put(x, 10, z, WATER);
        }
    }
    put(0, 11, 3, CHEST);
    // Sponges upstairs in the west wing.
    for (x, y, z) in [(-9, 6, -9), (-10, 6, -9), (-9, 7, -10), (-10, 6, -10), (-9, 6, -10), (-10, 7, -9)] {
        if crate::noise::hash3(s, x, y, z) < 0.8 {
            put(x, y, z, SPONGE);
        }
    }
    out
}

/// Where a monument's three Elder Guardians live (from its chest): the
/// penthouse and the two wings.
pub fn elder_spots(chest: IVec3) -> [Vec3; 3] {
    let o = (chest - ivec3(0, 11, 3)).as_vec3() + Vec3::new(0.5, 0.0, 0.5);
    [o + Vec3::new(0.0, 11.5, 0.0), o + Vec3::new(9.0, 1.5, 0.0), o + Vec3::new(-9.0, 1.5, 0.0)]
}

/// How much prismarine frames a Conduit at `p` (the 5x5x5 round it).
pub fn frame_count(world: &crate::world::World, p: IVec3) -> usize {
    let mut n = 0;
    for dx in -2..=2 {
        for dy in -2..=2 {
            for dz in -2..=2 {
                if is_prismarine(world.get_v(p + ivec3(dx, dy, dz))) {
                    n += 1;
                }
            }
        }
    }
    n
}

/// Is a Conduit at `p` under water (most of the 3x3x3 round it is water)?
fn conduit_wet(world: &crate::world::World, p: IVec3) -> bool {
    let wet = (-1..=1).flat_map(|dx| (-1..=1).flat_map(move |dy| (-1..=1).map(move |dz| ivec3(dx, dy, dz)))).filter(|d| is_water(world.get_v(p + *d))).count();
    wet >= 12
}

/// How far a Conduit with this much frame reaches (0: not working).
pub fn conduit_range(frame: usize) -> f32 {
    if frame < CONDUIT_MIN { 0.0 } else { (16.0 * frame.min(CONDUIT_FULL) as f32 / 7.0).min(96.0) }
}

impl crate::world::Generator {
    /// The middle of the monument near `p`, if there is one.
    pub fn monument_near(&self, p: Vec3, r: f32) -> Option<IVec3> {
        let (cx, cz) = ((p.x as i32).div_euclid(16), (p.z as i32).div_euclid(16));
        (-3..=3).flat_map(|dz| (-3..=3).map(move |dx| (dx, dz))).filter_map(|(dx, dz)| self.site(cx + dx, cz + dz)).filter(|s| s.kind == Kind::Monument).map(|s| s.origin).find(|o| o.as_vec3().distance(p) < r)
    }
}

impl Game {
    /// Guardians turn up round monuments, Elders curse, Conduits work.
    pub fn monument_tick(&mut self, dt: f32) {
        if self.is_client() {
            return;
        }
        self.monument_acc += dt;
        if self.monument_acc < 1.0 {
            return;
        }
        self.monument_acc -= 1.0;
        let mut people: Vec<(u32, Vec3)> = self.peers.iter().filter(|(_, p)| p.alive()).map(|(&id, p)| (id, p.target)).collect();
        if !self.away() && self.dead.is_none() {
            people.push((self.my_id, self.player.body.pos));
        }
        // More Guardians in a monument's water while someone's about.
        for &(_, at) in &people {
            let Some(o) = self.world.generator.monument_near(at, NEAR) else { continue };
            let here = self.mobs.iter().filter(|m| m.kind == MobKind::Guardian && m.body.pos.distance(o.as_vec3()) < 30.0).count();
            if here >= GUARDIANS || !self.rules.difficulty.monsters() || !self.rng.chance(0.3) {
                continue;
            }
            let spot = o + ivec3(self.rng.int(-11, 11), self.rng.int(1, 9), self.rng.int(-11, 11));
            if self.world.is_loaded(spot.x, spot.z) && is_water(self.world.get_v(spot)) && is_water(self.world.get_v(spot + IVec3::Y)) {
                let id = self.alloc_mob(MobKind::Guardian, spot.as_vec3() + Vec3::new(0.5, 0.0, 0.5));
                if let Some(m) = self.mobs.iter_mut().find(|m| m.id == id) {
                    m.home = Some(o.as_vec3() + Vec3::Y * 5.0);
                }
            }
        }
        // Elders curse everyone near them, once a minute.
        self.curse_acc += 1.0;
        if self.curse_acc >= CURSE_EVERY {
            self.curse_acc = 0.0;
            let elders: Vec<Vec3> = self.mobs.iter().filter(|m| m.kind == MobKind::ElderGuardian && m.health > 0.0).map(|m| m.body.pos).collect();
            for &(id, at) in &people {
                if !elders.iter().any(|e| e.distance(at) < CURSE_RANGE) {
                    continue;
                }
                if id == self.my_id {
                    if !self.creative {
                        self.timed_effect_amplified(Potion::MiningFatigue, CURSE_SECS, 2);
                        self.msg("An Elder Guardian stares through you. Your arms feel very heavy (Mining Fatigue).");
                        self.advance("cursed");
                    }
                } else {
                    self.send_timed_effect(id, Potion::MiningFatigue, CURSE_SECS, 2);
                }
                self.sfx(crate::sound::Sfx::Shriek, Some(at));
            }
        }
        // Conduits: power for everyone in the water in reach; full ones zap monsters.
        let conduits: Vec<IVec3> = self.world.conduits.iter().copied().filter(|p| self.world.is_loaded(p.x, p.z)).collect();
        self.zap_acc += 1.0;
        let zap = self.zap_acc >= 2.0;
        if zap {
            self.zap_acc = 0.0;
        }
        for c in conduits {
            if self.world.get_v(c) != CONDUIT || !conduit_wet(&self.world, c) {
                continue;
            }
            let frame = frame_count(&self.world, c);
            let range = conduit_range(frame);
            if range <= 0.0 {
                continue;
            }
            let centre = c.as_vec3() + Vec3::splat(0.5);
            for &(id, at) in &people {
                let wet = is_water(self.world.get_v((at + Vec3::Y * 0.5).floor().as_ivec3())) || is_water(self.world.get_v((at + Vec3::Y * 1.6).floor().as_ivec3()));
                if !wet || at.distance(centre) > range {
                    continue;
                }
                if id == self.my_id {
                    self.timed_effect(Potion::ConduitPower, 12.0);
                    self.advance("conduit_power");
                } else {
                    self.send_timed_effect(id, Potion::ConduitPower, 12.0, 0);
                }
            }
            if zap && frame >= CONDUIT_FULL {
                for m in self.mobs.iter_mut().filter(|m| m.kind.hostile() && m.body.in_water && m.body.pos.distance(centre) < 8.0) {
                    m.damage(4.0, centre);
                }
            }
        }
    }

    /// Hitting a Guardian in melee pricks you on its spikes (unless it's busy aiming).
    pub fn guardian_spikes(&mut self, i: usize) {
        let m = &self.mobs[i];
        if is_guardian(m.kind) && m.fuse <= 0.0 && !self.creative {
            self.player.hurt = 0.0;
            self.hurt_player(2.0, "was pricked by a Guardian's spikes. Hug Life, the sequel");
        }
    }

    /// Guardians' lasers: a beam from the eye to whoever it's aiming at,
    /// thickening as it charges.
    pub fn draw_lasers(&self, g: &mut crate::render::DynGeo) {
        let mut people: Vec<Vec3> = self.peers.values().filter(|p| p.alive()).map(|p| p.target).collect();
        if !self.away() && self.dead.is_none() {
            people.push(self.player.body.pos);
        }
        for m in self.mobs.iter().filter(|m| is_guardian(m.kind) && m.fuse > 0.0) {
            let eye = m.body.pos + Vec3::Y * m.body.height * 0.5;
            let Some(target) = people.iter().copied().min_by(|a, b| a.distance_squared(eye).total_cmp(&b.distance_squared(eye))) else { continue };
            let to = target + Vec3::Y * 0.9 - eye;
            let len = to.length();
            if !(0.5..=20.0).contains(&len) {
                continue;
            }
            let thick = 0.03 + 0.07 * (m.fuse / LASER_SECS).min(1.0);
            let rot = Quat::from_rotation_arc(Vec3::Y, to / len);
            let root = Mat4::from_translation(eye) * Mat4::from_quat(rot) * Mat4::from_translation(Vec3::new(-thick / 2.0, 0.0, -thick / 2.0)) * Mat4::from_scale(Vec3::new(thick, len, thick));
            g.cube(&root, [crate::texture::T_GUARDIAN_LASER; 6], 1.0, [0.0, 0.0, 1.0, 1.0]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site() -> Site {
        Site { kind: Kind::Monument, origin: ivec3(0, 20, 0), facing: 0, seed: 99 }
    }

    #[test]
    fn worlds_have_monuments() {
        let g = crate::world::Generator::with(424242, crate::world::GenOptions::DEFAULT);
        let n = (-60..60).flat_map(|cz| (-60..60).map(move |cx| (cx, cz))).filter(|&(cx, cz)| g.site(cx, cz).is_some_and(|s| s.kind == Kind::Monument)).count();
        assert!(n >= 3, "monuments {n}");
    }

    #[test]
    fn the_monument_is_wet_and_golden() {
        let b = monument_blocks(&site());
        let count = |id: Id| b.iter().filter(|x| x.1 == id).count();
        assert_eq!(count(GOLD_BLOCK), 8);
        assert_eq!(count(CHEST), 1);
        assert!(count(SEA_LANTERN) > 10 && count(SPONGE) >= 3);
        // The insides are water (not air), so it stays flooded.
        assert_eq!(count(AIR), 0);
        assert!(count(WATER) > 1000);
        let spots = elder_spots(ivec3(0, 31, 3));
        let at = |p: Vec3| b.iter().rev().find(|(q, _)| *q == p.floor().as_ivec3()).map(|x| x.1);
        assert!(spots.iter().all(|&s| at(s) == Some(WATER)), "Elders start in water");
    }

    #[test]
    fn conduits_need_a_frame_and_water() {
        let mut g = crate::game::tests::arena(95);
        let c = ivec3(0, 60, 0);
        // A pool, a conduit in the middle, and a ring of prismarine round it.
        for dx in -3..=3 {
            for dy in -3..=3 {
                for dz in -3..=3 {
                    g.world.set_v(c + ivec3(dx, dy, dz), WATER);
                }
            }
        }
        g.world.set_v(c, CONDUIT);
        assert!(g.world.conduits.contains(&c));
        assert_eq!(conduit_range(frame_count(&g.world, c)), 0.0, "no frame, no power");
        let mut n = 0;
        'frame: for dx in -2..=2i32 {
            for dz in -2..=2i32 {
                for dy in [-2, 2] {
                    if n >= 20 {
                        break 'frame;
                    }
                    g.world.set_v(c + ivec3(dx, dy, dz), PRISMARINE);
                    n += 1;
                }
            }
        }
        assert!(conduit_range(frame_count(&g.world, c)) > 30.0);
        g.player.body.pos = c.as_vec3() + Vec3::new(1.5, -0.5, 0.5);
        for _ in 0..3 {
            g.monument_tick(1.0);
        }
        assert!(g.has_effect(Potion::ConduitPower), "breathing easy");
    }

    #[test]
    fn guardians_laser_and_elders_curse() {
        let mut g = crate::game::tests::arena(96);
        // A tank of water with a Guardian and the player in it.
        for x in -6..=6 {
            for y in 50..56 {
                for z in -6..=6 {
                    g.world.set(x, y, z, WATER);
                }
            }
        }
        g.player.body.pos = Vec3::new(4.5, 51.0, 0.5);
        let id = g.alloc_mob(MobKind::Guardian, Vec3::new(-4.5, 51.0, 0.5));
        let before = g.player.health;
        for _ in 0..80 {
            g.update_entities(0.05);
        }
        assert!(g.mobs.iter().any(|m| m.id == id));
        assert!(g.player.health < before, "zapped");
        // An Elder curses after a minute.
        g.alloc_mob(MobKind::ElderGuardian, Vec3::new(0.5, 52.0, 3.5));
        for _ in 0..65 {
            g.monument_tick(1.0);
        }
        assert!(g.has_effect(Potion::MiningFatigue));
    }
}

