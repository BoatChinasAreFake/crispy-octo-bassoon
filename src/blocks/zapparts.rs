//! More Zappy parts (v0.4): things that switch power on by themselves, and
//! two that do something with it.
//!
//! - A **Daylight Sensor** is on while the sun is up and it can see the sky
//!   (right-click it to turn it round: then it's on at night instead).
//! - A **Target** switches on for a second when an arrow, a snowball or a
//!   thrown spear hits it.
//! - A **Dropper** is a dispenser that only drops: each time power reaches
//!   it, one thing tumbles out of its front, or, if there's a chest, hopper
//!   or anything else that holds things in front, goes into that instead.
//! - An **Activator Rail**, while powered, tips the rider out of any cart
//!   rolling over it and stops a hopper cart on it picking things up.
//! - A **Trapped Chest** is a chest that powers everything beside it while
//!   anyone has it open.
//! - A **Calibrated Sculk Sensor** listens for one kind of noise only:
//!   footsteps, blocks placed or broken, music, or creatures dying
//!   (right-click to choose). It hears them anywhere, not just deep down,
//!   and switches on for a moment and a half.
//!
//! Like the rest of the wiring, it all runs where the world lives.

use crate::block::*;
use crate::game::Game;
use crate::sound::{Mat, Sfx};
use macroquad::math::{IVec3, Vec3};

/// Seconds a Target stays on after a hit, and a calibrated sensor after a noise.
pub const TARGET_SECS: f32 = 1.0;
pub const HEARD_SECS: f32 = 1.5;
/// How far a calibrated sensor hears.
pub const CAL_HEAR: f32 = 8.0;
/// How often daylight sensors look at the sky.
const SKY_EVERY: f32 = 1.0;

/// What made a noise (see `Game::vibrate`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vibe {
    Step,
    Block,
    Music,
    Mob,
}

impl Vibe {
    pub const ALL: [Vibe; 4] = [Vibe::Step, Vibe::Block, Vibe::Music, Vibe::Mob];
    pub fn name(self) -> &'static str {
        match self {
            Vibe::Step => "footsteps",
            Vibe::Block => "blocks placed and broken",
            Vibe::Music => "music",
            Vibe::Mob => "creatures dying",
        }
    }
}

pub fn is_daylight(id: Id) -> bool {
    (DAYLIGHT_SENSOR..DAYLIGHT_SENSOR + 4).contains(&id)
}
/// (turned round: on at night, on)
pub fn daylight_state(id: Id) -> (bool, bool) {
    let k = id - DAYLIGHT_SENSOR;
    (k / 2 == 1, k % 2 == 1)
}
pub fn daylight(night: bool, on: bool) -> Id {
    DAYLIGHT_SENSOR + night as Id * 2 + on as Id
}
pub fn is_target(id: Id) -> bool {
    id == TARGET || id == TARGET_ON
}
pub fn is_dropper(id: Id) -> bool {
    (DROPPER_FIRST..DROPPER_FIRST + 6).contains(&id)
}
pub fn is_activator(id: Id) -> bool {
    (ACTIVATOR_RAIL..ACTIVATOR_RAIL + 4).contains(&id)
}
/// A powered activator rail.
pub fn activator_on(id: Id) -> bool {
    is_activator(id) && (id - ACTIVATOR_RAIL) % 2 == 1
}
/// An activator rail at `p`: its axis from the rails around it, and power.
pub fn activator_shape(world: &crate::world::World, p: IVec3, on: bool) -> Id {
    let has = |d: IVec3| crate::vehicles::is_rail(world.get_v(p + d));
    let ew = (has(IVec3::X) || has(IVec3::NEG_X)) && !(has(IVec3::Z) || has(IVec3::NEG_Z));
    ACTIVATOR_RAIL + ew as Id * 2 + on as Id
}
pub fn is_trapped(id: Id) -> bool {
    id == TRAPPED_CHEST || id == TRAPPED_CHEST_ON
}
pub fn is_calibrated(id: Id) -> bool {
    (CALIBRATED_SENSOR..CALIBRATED_SENSOR + 8).contains(&id)
}
/// (what it listens for, on)
pub fn calibrated_state(id: Id) -> (Vibe, bool) {
    let k = id - CALIBRATED_SENSOR;
    (Vibe::ALL[(k / 2) as usize % 4], k % 2 == 1)
}
pub fn calibrated(kind: Vibe, on: bool) -> Id {
    CALIBRATED_SENSOR + Vibe::ALL.iter().position(|&v| v == kind).unwrap_or(0) as Id * 2 + on as Id
}

/// The ones that are switched on (they power everything beside them).
pub fn source(id: Id) -> bool {
    (is_daylight(id) && daylight_state(id).1) || id == TARGET_ON || id == TRAPPED_CHEST_ON || (is_calibrated(id) && calibrated_state(id).1)
}

/// The ones the world keeps a list of (they're looked at every so often).
pub fn watched(id: Id) -> bool {
    is_daylight(id) || is_trapped(id) || is_calibrated(id)
}

/// Any of them (for waking the wiring; see `is_zappy`).
pub fn is_part(id: Id) -> bool {
    (DAYLIGHT_SENSOR..CALIBRATED_SENSOR + 8).contains(&id)
}

/// The blocks, in id order.
pub fn defs() -> Vec<BlockDef> {
    use crate::texture::*;
    let mut v = Vec::new();
    for k in 0..4u16 {
        let night = k / 2 == 1;
        let key = ["daylight_sensor", "daylight_sensor_on", "daylight_sensor_night", "daylight_sensor_night_on"][k as usize];
        let top = if night { T_DAYLIGHT_TOP_NIGHT } else { T_DAYLIGHT_TOP };
        let mut d = def(key, if night { "Daylight Sensor (Night Owl)" } else { "Daylight Sensor (Sun's Out)" }, Model::Shaped, true, false, [top, T_DAYLIGHT_SIDE, T_PLANKS], 0.6, 0, false, DAYLIGHT_SENSOR, 0.0, S_WOOD);
        d.shape = Shape::Slab { top: false };
        d.creative = k == 0;
        v.push(d);
    }
    for on in [false, true] {
        let mut d = def(if on { "target_on" } else { "target" }, "Target (Aim Here)", Model::Cube, true, true, [T_TARGET_TOP, T_TARGET_SIDE, T_TARGET_TOP], 0.5, 0, false, TARGET, 0.0, S_GRASS);
        d.creative = !on;
        v.push(d);
    }
    for facing in 0..6u8 {
        let key = format!("dropper{}", ["", "_east", "_south", "_west", "_up", "_down"][facing as usize]);
        let mut d = def(leak(&key), "Dropper (Just Drops Things)", Model::Cube, true, true, [T_COBBLE; 3], 3.5, 1, true, DROPPER_FIRST, 0.0, S_STONE);
        d.creative = facing == 0;
        v.push(d);
    }
    for (k, key) in ["activator_rail", "activator_rail_on", "activator_rail_ew", "activator_rail_ew_on"].into_iter().enumerate() {
        let mut d = def(key, "Activator Rail (Everybody Out)", Model::Shaped, false, false, [T_ACTIVATOR_RAIL + k as u16; 3], 0.7, 0, false, ACTIVATOR_RAIL, 0.0, S_STONE);
        d.shape = Shape::Dust;
        d.creative = k == 0;
        v.push(d);
    }
    for on in [false, true] {
        let mut d = def(if on { "trapped_chest_on" } else { "trapped_chest" }, "Trapped Chest (Suspiciously Normal)", Model::Shaped, true, false, [T_TRAPPED_CHEST_TOP, T_TRAPPED_CHEST_SIDE, T_TRAPPED_CHEST_TOP], 2.5, 0, false, TRAPPED_CHEST, 0.0, S_WOOD);
        d.shape = Shape::Chest;
        d.creative = !on;
        v.push(d);
    }
    for k in 0..8u16 {
        let (kind, on) = (Vibe::ALL[(k / 2) as usize], k % 2 == 1);
        let key = format!("calibrated_sculk_sensor_{}{}", ["steps", "blocks", "music", "mobs"][(k / 2) as usize], if on { "_on" } else { "" });
        let _ = kind;
        let mut d = def(leak(&key), "Calibrated Sculk Sensor (Picky Listener)", Model::Cube, true, true, [T_CALIBRATED_TOP + k / 2, T_CALIBRATED_SIDE, T_SCULK], 1.5, 0, false, CALIBRATED_SENSOR, if on { 8.0 } else { 1.0 }, S_GRASS);
        d.creative = k == 0;
        v.push(d);
    }
    v
}

/// Their recipes.
pub fn recipes() -> Vec<Recipe> {
    let r = |inputs: &[(Id, u8)], output: (Id, u8)| Recipe { inputs: inputs.to_vec(), output };
    vec![
        r(&[(GLASS, 3), (PLANKS, 3), (ZAP_DUST, 1)], (DAYLIGHT_SENSOR, 1)),
        r(&[(HAY, 1), (ZAP_DUST, 4)], (TARGET, 1)),
        r(&[(COBBLE, 7), (ZAP_DUST, 1)], (DROPPER_FIRST, 1)),
        r(&[(IRON, 6), (STICK, 2), (ZTORCH_ON, 1)], (ACTIVATOR_RAIL, 6)),
        r(&[(CHEST, 1), (TRIPWIRE_HOOK_FIRST, 1)], (TRAPPED_CHEST, 1)),
        r(&[(SCULK_SENSOR, 1), (AMETHYST_SHARD, 3)], (CALIBRATED_SENSOR, 1)),
    ]
}

impl Game {
    /// Daylight sensors look at the sky; trapped chests notice who has them open.
    pub fn zapparts_tick(&mut self, dt: f32) {
        if self.is_client() || !self.world.simulate_liquids {
            return;
        }
        self.sky_timer -= dt;
        let look_up = self.sky_timer <= 0.0;
        if look_up {
            self.sky_timer = SKY_EVERY;
        }
        let day = !self.is_night();
        let parts: Vec<IVec3> = self.world.zap_parts.iter().copied().filter(|p| self.world.is_loaded(p.x, p.z)).collect();
        for p in parts {
            let id = self.world.get_v(p);
            if is_daylight(id) && look_up {
                let (night, on) = daylight_state(id);
                let sky = self.world.sky_light(p.x, p.y + 1, p.z) >= 0.5;
                let want = sky && (day != night) && self.world.dim().open_sky();
                if want != on {
                    self.world.set_v(p, daylight(night, want));
                }
            } else if is_trapped(id) {
                let open = self.open == Some(p) && !self.away() || self.viewers.get(&p).is_some_and(|v| !v.is_empty());
                let want = if open { TRAPPED_CHEST_ON } else { TRAPPED_CHEST };
                if want != id {
                    self.world.set_v(p, want);
                    // Sprung by us, with something wired up to it.
                    let wired = [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z].iter().any(|d| is_zappy(self.world.get_v(p + *d)));
                    if open && wired && self.open == Some(p) {
                        self.advance("its_a_trap");
                    }
                }
            }
        }
    }

    /// Something flying hit the block at `p` (where the world lives).
    pub fn projectile_hit_block(&mut self, p: IVec3, shooter: Option<u32>) {
        if self.is_client() || !is_target(self.world.get_v(p)) {
            return;
        }
        self.world.set_v(p, TARGET_ON);
        self.buttons.insert(p, TARGET_SECS);
        self.sfx(Sfx::Click, Some(p.as_vec3() + Vec3::splat(0.5)));
        if shooter == Some(self.my_id) && !self.away() {
            self.advance("bullseye");
        }
    }

    /// Right-click on a daylight or calibrated sensor (the local player): turn
    /// it round, or have it listen for the next kind of noise. True if it was one.
    pub fn use_zappart(&mut self, pos: IVec3, id: Id) -> bool {
        let new = if is_daylight(id) {
            let (night, on) = daylight_state(id);
            self.msg(if night { "Now it switches on in the daytime." } else { "Now it switches on at night." });
            daylight(!night, on)
        } else if is_calibrated(id) {
            let (kind, _) = calibrated_state(id);
            let next = Vibe::ALL[(Vibe::ALL.iter().position(|&v| v == kind).unwrap_or(0) + 1) % 4];
            self.msg(format!("It listens for {} now.", next.name()));
            self.advance("tuning_in");
            calibrated(next, false)
        } else {
            return false;
        };
        self.world.set_v(pos, new);
        self.sfx(Sfx::Click, Some(pos.as_vec3() + Vec3::splat(0.5)));
        self.player.swing = 1.0;
        true
    }

    /// A noise of kind `kind` at `at`: calibrated sensors that listen for it hear it.
    pub fn hear_calibrated(&mut self, at: Vec3, kind: Vibe) {
        let near: Vec<IVec3> = self.world.zap_parts.iter().copied().filter(|p| (p.as_vec3() + Vec3::splat(0.5)).distance(at) <= CAL_HEAR).collect();
        for p in near {
            let id = self.world.get_v(p);
            if is_calibrated(id) && calibrated_state(id).0 == kind {
                if !calibrated_state(id).1 {
                    self.world.set_v(p, calibrated(kind, true));
                    self.sfx(Sfx::Sculk, Some(p.as_vec3() + Vec3::splat(0.5)));
                }
                self.sensors_on.insert(p, HEARD_SECS);
            }
        }
    }

    /// A dropper got power: one thing out of its front, into whatever holds things there.
    pub fn drop_one(&mut self, p: IVec3, f: u8) {
        let d = crate::contraptions::dir6(f);
        let mouth = p.as_vec3() + Vec3::splat(0.5) + d.as_vec3() * 0.7;
        let Some(slot) = self.world.containers.get(&p).and_then(|c| c.slots.iter().position(|s| s.is_some())) else {
            self.sfx(Sfx::Click, Some(mouth));
            return;
        };
        // Into a container in front, if there is one (and it has room).
        if let Some(to) = self.container_in(p + d) {
            if self.move_one(p, Some(vec![slot]), to, false) {
                self.sfx(Sfx::Click, Some(mouth));
            }
            return;
        }
        let Some(c) = self.world.containers.get_mut(&p) else { return };
        let Some((item, n)) = c.slots[slot] else { return };
        let wear = c.wear[slot];
        c.slots[slot] = if n > 1 { Some((item, n - 1)) } else { None };
        if n <= 1 {
            c.wear[slot] = 0;
        }
        self.dirty_containers.insert(p);
        self.spawn_drop(mouth - Vec3::Y * 0.125, item, 1, wear, d.as_vec3() * 2.5 + Vec3::Y * 0.5, 0.5);
        self.sfx(Sfx::Place(Mat::Stone), Some(mouth));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use macroquad::math::ivec3;

    fn settle(g: &mut Game) {
        for _ in 0..12 {
            g.zap_tick(0.1);
            g.zapparts_tick(0.1);
        }
    }

    #[test]
    fn ids_and_states_round_trip() {
        for night in [false, true] {
            for on in [false, true] {
                assert_eq!(daylight_state(daylight(night, on)), (night, on));
            }
        }
        for kind in Vibe::ALL {
            assert_eq!(calibrated_state(calibrated(kind, true)), (kind, true));
        }
        assert!(source(TARGET_ON) && source(daylight(true, true)) && !source(daylight(false, false)) && !source(TARGET));
        assert!(defs().len() as Id == NUM_BLOCKS - DAYLIGHT_SENSOR, "one def for every id");
        assert!(crate::chests::is_chest(TRAPPED_CHEST) && crate::containers::is_container(DROPPER_FIRST + 2));
        assert!(crate::vehicles::is_rail(ACTIVATOR_RAIL + 3) && activator_on(ACTIVATOR_RAIL + 1));
    }

    #[test]
    fn daylight_sensors_follow_the_sun_and_targets_take_hits() {
        let mut g = crate::game::tests::arena(960);
        let s = ivec3(2, 60, 2);
        let lamp = s + IVec3::X;
        g.world.set_v(s, DAYLIGHT_SENSOR);
        g.world.set_v(lamp, LAMP);
        g.time = 0.3; // morning, sun well up
        settle(&mut g);
        assert_eq!(g.world.get_v(s), daylight(false, true), "on in the day");
        assert_eq!(g.world.get_v(lamp), LAMP_ON);
        g.time = 0.8;
        settle(&mut g);
        assert_eq!(g.world.get_v(lamp), LAMP, "off at night");
        // Turned round, it's on at night.
        g.use_zappart(s, g.world.get_v(s));
        settle(&mut g);
        assert_eq!(g.world.get_v(lamp), LAMP_ON);
        // A target: on for a moment after a hit.
        let t = ivec3(-3, 60, 2);
        g.world.set_v(t, TARGET);
        g.world.set_v(t + IVec3::Z, LAMP);
        g.projectile_hit_block(t, Some(g.my_id));
        settle(&mut g);
        assert!(g.advancements.has("bullseye"));
        assert_eq!(g.world.get_v(t), TARGET);
        // (Settled past its second: off again; look while it's on.)
        g.projectile_hit_block(t, None);
        g.zap_tick(0.1);
        g.zap_tick(0.1);
        assert_eq!(g.world.get_v(t + IVec3::Z), LAMP_ON);
    }

    #[test]
    fn droppers_drop_or_fill_and_trapped_chests_trip() {
        let mut g = crate::game::tests::arena(961);
        let d = ivec3(0, 52, 0);
        g.world.set_v(d, DROPPER_FIRST + 1); // facing east
        g.world.containers.get_mut(&d).unwrap().slots[0] = Some((COBBLE, 3));
        g.drop_one(d, 1);
        assert!(g.drops.iter().any(|x| x.item == COBBLE), "out onto the floor");
        // With a chest in front, into the chest.
        g.world.set_v(d + IVec3::X, CHEST);
        g.drop_one(d, 1);
        assert!(g.world.containers[&(d + IVec3::X)].slots.iter().flatten().any(|s| s.0 == COBBLE));
        assert_eq!(g.world.containers[&d].slots[0], Some((COBBLE, 1)));
        // A trapped chest powers its neighbours while it's open.
        let t = ivec3(4, 52, 0);
        g.world.set_v(t, TRAPPED_CHEST);
        g.world.set_v(t + IVec3::X, LAMP);
        g.open = Some(t);
        settle(&mut g);
        assert_eq!(g.world.get_v(t), TRAPPED_CHEST_ON);
        assert_eq!(g.world.get_v(t + IVec3::X), LAMP_ON);
        assert!(g.world.containers.contains_key(&t), "keeps its contents");
        g.open = None;
        settle(&mut g);
        assert_eq!(g.world.get_v(t + IVec3::X), LAMP);
    }

    #[test]
    fn calibrated_sensors_hear_only_their_kind_anywhere() {
        let mut g = crate::game::tests::arena(962);
        let s = ivec3(1, 60, 1);
        g.world.set_v(s, CALIBRATED_SENSOR); // footsteps
        let at = s.as_vec3() + Vec3::new(3.0, 0.0, 0.5);
        g.vibrate_kind(at, None, Vibe::Music);
        assert_eq!(g.world.get_v(s), calibrated(Vibe::Step, false), "not music");
        g.vibrate_kind(at, None, Vibe::Step);
        assert_eq!(g.world.get_v(s), calibrated(Vibe::Step, true), "footsteps, high up");
        g.use_zappart(s, g.world.get_v(s));
        assert_eq!(calibrated_state(g.world.get_v(s)).0, Vibe::Block);
        assert!(g.advancements.has("tuning_in"));
    }

    #[test]
    fn powered_activator_rails_tip_riders_out() {
        let mut g = crate::game::tests::arena(963);
        let r = ivec3(0, 51, 4);
        g.world.set_v(r, ACTIVATOR_RAIL + 1);
        let id = g.spawn_vehicle(crate::vehicles::CART_KIND, r.as_vec3() + Vec3::new(0.5, 0.06, 0.5), 0.0);
        g.riding = Some(id);
        if let Some(v) = g.vehicles.iter_mut().find(|v| v.id == id) {
            v.rider = g.my_id + 1;
        }
        g.vehicles_tick(0.05, 0.0, 0.0, false);
        assert_eq!(g.riding, None, "out you get");
    }
}
