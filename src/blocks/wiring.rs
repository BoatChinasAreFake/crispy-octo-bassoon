//! Zappy Dust: legally distinct redstone, simplified.
//!
//! Power comes from switches: a Lever (flip it), a Button (on for a second),
//! a Pressure Plate (on while anything stands on it) and a Block of Zappy
//! Dust (always on). Zappy Dust laid on the floor carries power up to 15
//! blocks from a switch, along the floor and up or down one step. Anything
//! next to a switch or powered dust is powered: Zappy Lamps light up, doors
//! open (and close when the power goes), and TNT goes off.
//!
//! Like liquids, nothing is checked until something nearby changes (see
//! `World::wake_zappy`), and it all runs where the world lives; joined
//! players flip levers and press buttons as ordinary block edits.

use crate::block::*;
use crate::entity::PrimedTnt;
use crate::game::Game;
use crate::sound::Sfx;
use crate::world::World;
use macroquad::math::{IVec3, Vec3};
use std::collections::{HashMap, HashSet, VecDeque};

/// Seconds between updates, how long a button stays pressed, and how far dust carries power.
pub const STEP: f32 = 0.1;
pub const BUTTON_SECS: f32 = 1.0;
pub const REACH: u32 = 15;
/// Biggest wire network looked at at once.
const MAX_NETWORK: usize = 2048;

const SIDES: [IVec3; 6] = [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z];
const FLAT: [IVec3; 4] = [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z];

pub fn is_wire(id: Id) -> bool {
    id == WIRE || id == WIRE_ON
}

/// A switch that's on (or the block that always is).
pub fn source_on(id: Id) -> bool {
    matches!(id, LEVER_ON | BUTTON_ON | PLATE_ON | ZAP_BLOCK | SCULK_SENSOR_ACTIVE) || crate::vehicles::detector_on(id) || crate::tripwire::tripped(id)
}

/// Things that sit on the floor and fall off when it goes.
pub fn needs_floor(id: Id) -> bool {
    is_wire(id) || matches!(id, LEVER | LEVER_ON | BUTTON | BUTTON_ON | PLATE | PLATE_ON) || crate::vehicles::is_rail(id) || crate::tripwire::is_tripwire(id)
}

/// Dust next to this dust: along the floor, and one step up or down.
fn wire_links(world: &World, p: IVec3) -> Vec<IVec3> {
    let mut v = Vec::new();
    for d in FLAT {
        for dy in [0, 1, -1] {
            let q = p + d + IVec3::Y * dy;
            if is_wire(world.get_v(q)) {
                v.push(q);
            }
        }
    }
    v
}

/// Is something at `p` getting power (from a switch or lit dust next to it)?
pub fn powered(world: &World, p: IVec3) -> bool {
    SIDES.iter().any(|d| {
        let id = world.get_v(p + *d);
        source_on(id) || id == WIRE_ON || crate::contraptions::powers(world, p + *d, p)
    })
}

/// Work out a whole wire network: which of its dust should be lit.
fn settle_network(world: &World, start: IVec3) -> HashMap<IVec3, bool> {
    let mut all = HashSet::new();
    let mut queue = vec![start];
    all.insert(start);
    while let Some(p) = queue.pop() {
        if all.len() >= MAX_NETWORK {
            break;
        }
        for q in wire_links(world, p) {
            if all.insert(q) {
                queue.push(q);
            }
        }
    }
    // Distance from the nearest switch, through the dust.
    let mut dist: HashMap<IVec3, u32> = HashMap::new();
    let mut bfs = VecDeque::new();
    for &p in &all {
        if SIDES.iter().any(|d| source_on(world.get_v(p + *d)) || crate::contraptions::powers(world, p + *d, p)) {
            dist.insert(p, 0);
            bfs.push_back(p);
        }
    }
    while let Some(p) = bfs.pop_front() {
        let d = dist[&p];
        if d + 1 >= REACH {
            continue;
        }
        for q in wire_links(world, p) {
            if all.contains(&q) && !dist.contains_key(&q) {
                dist.insert(q, d + 1);
                bfs.push_back(q);
            }
        }
    }
    all.into_iter().map(|p| (p, dist.contains_key(&p))).collect()
}

impl Game {
    pub fn zap_tick(&mut self, dt: f32) {
        if !self.world.simulate_liquids {
            self.world.zap_dirty.clear();
            return;
        }
        self.zap_timer += dt;
        if self.zap_timer < STEP {
            return;
        }
        let dt = std::mem::take(&mut self.zap_timer);
        // Buttons pop back out.
        let mut released = Vec::new();
        for (p, t) in self.buttons.iter_mut() {
            *t -= dt;
            if *t <= 0.0 {
                released.push(*p);
            }
        }
        for p in released {
            self.buttons.remove(&p);
            if self.world.get_v(p) == BUTTON_ON {
                self.world.set_v(p, BUTTON);
                self.sfx(Sfx::Click, Some(p.as_vec3() + Vec3::splat(0.5)));
            }
        }
        self.pressure_plates(dt);
        self.observers_tick(dt);
        self.detector_rails(dt);
        // Comparators watch their containers (which change without any block changing).
        let comparators: Vec<IVec3> = self.world.comparators.iter().copied().filter(|p| self.world.is_loaded(p.x, p.z)).collect();
        for p in comparators {
            self.contraption_update(p);
        }
        let dirty: Vec<IVec3> = self.world.zap_dirty.drain().collect();
        let mut check: HashSet<IVec3> = HashSet::new();
        let mut done: HashSet<IVec3> = HashSet::new();
        for p in dirty {
            if !self.world.is_loaded(p.x, p.z) {
                continue;
            }
            let id = self.world.get_v(p);
            // Pressed buttons start their timer (whoever pressed them).
            if id == BUTTON_ON && !self.buttons.contains_key(&p) {
                self.buttons.insert(p, BUTTON_SECS);
            }
            check.insert(p);
            // Rails bend to join their neighbours; powered ones follow the power.
            if crate::vehicles::is_rail(id) {
                let want = if crate::vehicles::is_detector(id) {
                    crate::vehicles::detector_shape(&self.world, p, crate::vehicles::detector_on(id))
                } else if crate::vehicles::is_powered_rail(id) {
                    crate::vehicles::powered_shape(&self.world, p, powered(&self.world, p))
                } else {
                    crate::vehicles::rail_shape(&self.world, p)
                };
                if want != id {
                    self.world.set_v(p, want);
                }
                continue;
            }
            // A portal whose frame is broken falls apart.
            if crate::scorch::is_portal(id) {
                let side = if id == PORTAL_X { IVec3::X } else { IVec3::Z };
                let intact = [side, -side, IVec3::Y, IVec3::NEG_Y].iter().all(|d| {
                    let n = self.world.get_v(p + *d);
                    n == id || n == OBSIDIAN
                });
                if !intact {
                    self.world.set_v(p, AIR);
                    self.sfx(Sfx::Warp, Some(p.as_vec3() + Vec3::splat(0.5)));
                }
                continue;
            }
            if is_wire(id) && !done.contains(&p) {
                for (q, lit) in settle_network(&self.world, p) {
                    done.insert(q);
                    let want = if lit { WIRE_ON } else { WIRE };
                    if self.world.get_v(q) != want {
                        self.world.set_v(q, want);
                        for d in SIDES {
                            check.insert(q + d);
                        }
                    }
                }
            }
        }
        for p in check {
            self.zap_consumer(p);
        }
    }

    /// A lamp, door or TNT at `p` reacting to power arriving or going.
    fn zap_consumer(&mut self, p: IVec3) {
        // A Zappy Torch standing on this block watches it.
        if crate::contraptions::is_ztorch(self.world.get_v(p + IVec3::Y)) {
            self.contraption_update(p + IVec3::Y);
        }
        let id = self.world.get_v(p);
        if crate::contraptions::is_contraption(id) {
            self.contraption_update(p);
            return;
        }
        let center = p.as_vec3() + Vec3::splat(0.5);
        match id {
            LAMP | LAMP_ON => {
                let on = powered(&self.world, p);
                let want = if on { LAMP_ON } else { LAMP };
                if want != id {
                    self.world.set_v(p, want);
                    if on && self.player.body.pos.distance(center) < 16.0 {
                        self.advance("its_alive");
                    }
                }
            }
            _ if crate::music::is_note_block(id) => {
                let on = powered(&self.world, p);
                self.note_power(p, on);
            }
            TNT if powered(&self.world, p) => {
                self.world.set_v(p, AIR);
                self.tnts.push(PrimedTnt { pos: p.as_vec3(), fuse: 3.0 });
                self.sfx(Sfx::Hiss, Some(center));
            }
            _ if is_door(id) => {
                let Some((_, open, top)) = door_state(id) else { return };
                let bottom = if top { p - IVec3::Y } else { p };
                let on = powered(&self.world, bottom) || powered(&self.world, bottom + IVec3::Y);
                // Only a change of power moves it (so you can still open and close it by hand).
                let was = self.powered_doors.contains(&bottom);
                if on == was {
                    return;
                }
                if on {
                    self.powered_doors.insert(bottom);
                } else {
                    self.powered_doors.remove(&bottom);
                }
                if open != on {
                    let f = door_state(self.world.get_v(bottom)).map(|d| d.0).unwrap_or(0);
                    for (q, t) in [(bottom, false), (bottom + IVec3::Y, true)] {
                        if self.world.get_v(q) == door(f, open, t) {
                            self.world.set_v(q, door(f, on, t));
                        }
                    }
                    self.sfx(Sfx::Place(crate::sound::Mat::Wood), Some(center));
                }
            }
            _ => {}
        }
    }

    /// Pressure plates go down under anyone (or anything) standing on them.
    fn pressure_plates(&mut self, dt: f32) {
        let mut feet: Vec<Vec3> = Vec::new();
        if !self.away() && self.dead.is_none() {
            feet.push(self.player.body.pos);
        }
        feet.extend(self.peers.values().filter(|p| p.alive()).map(|p| p.target));
        feet.extend(self.mobs.iter().map(|m| m.body.pos));
        feet.extend(self.drops.iter().map(|d| d.body.pos));
        let mut pressed = HashSet::new();
        let mut trips = Vec::new();
        for f in feet {
            let p = IVec3::new(f.x.floor() as i32, (f.y + 0.05).floor() as i32, f.z.floor() as i32);
            let id = self.world.get_v(p);
            if id == PLATE || id == PLATE_ON {
                pressed.insert(p);
            }
            if crate::tripwire::is_tripwire(id) || crate::tripwire::is_hook(id) {
                trips.push(p);
                pressed.insert(p);
            }
        }
        // Tripwire trips its whole line (see tripwire.rs).
        self.trip(&trips);
        for q in trips.iter().flat_map(|&p| crate::tripwire::line(&self.world, p)) {
            pressed.insert(q);
        }
        for &p in &pressed {
            self.plates.insert(p, 0.5);
            if self.world.get_v(p) == PLATE {
                self.world.set_v(p, PLATE_ON);
                self.sfx(Sfx::Click, Some(p.as_vec3() + Vec3::splat(0.5)));
            }
        }
        let mut up = Vec::new();
        for (p, t) in self.plates.iter_mut() {
            if !pressed.contains(p) {
                *t -= dt;
                if *t <= 0.0 {
                    up.push(*p);
                }
            }
        }
        for p in up {
            self.plates.remove(&p);
            let id = self.world.get_v(p);
            if id == PLATE_ON {
                self.world.set_v(p, PLATE);
                self.sfx(Sfx::Click, Some(p.as_vec3() + Vec3::splat(0.5)));
            } else if crate::tripwire::tripped(id) {
                self.world.set_v(p, crate::tripwire::with_tripped(id, false));
            }
        }
    }

    /// Detector rails switch on under a cart, and off a moment after it's gone.
    fn detector_rails(&mut self, dt: f32) {
        use crate::vehicles::{detector_on, detector_shape, is_boat, is_detector};
        let carts: HashSet<IVec3> = self.vehicles.iter().filter(|v| !is_boat(v.kind)).map(|v| v.cell()).filter(|c| is_detector(self.world.get_v(*c))).collect();
        for &p in &carts {
            self.detectors.insert(p, 0.5);
            let id = self.world.get_v(p);
            if !detector_on(id) {
                let mine = self.riding.is_some_and(|r| self.vehicles.iter().any(|v| v.id == r && v.cell() == p));
                if mine {
                    self.advance("tattletale");
                }
                self.world.set_v(p, detector_shape(&self.world, p, true));
                self.sfx(Sfx::Click, Some(p.as_vec3() + Vec3::splat(0.5)));
            }
        }
        let mut off = Vec::new();
        for (p, t) in self.detectors.iter_mut() {
            if !carts.contains(p) {
                *t -= dt;
                if *t <= 0.0 {
                    off.push(*p);
                }
            }
        }
        for p in off {
            self.detectors.remove(&p);
            if detector_on(self.world.get_v(p)) {
                self.world.set_v(p, detector_shape(&self.world, p, false));
            }
        }
    }

    /// Right-click on a lever or button (the local player). True if it was one.
    pub fn use_switch(&mut self, pos: IVec3, id: Id) -> bool {
        let new = match id {
            LEVER => LEVER_ON,
            LEVER_ON => LEVER,
            BUTTON => BUTTON_ON,
            BUTTON_ON => return true,
            b if crate::beacon::is_beacon(b) && self.inv.held() == WILTER_STAR && !crate::beacon::starred(b) => return self.star_beacon(pos),
            b if crate::beacon::is_beacon(b) => {
                let next = crate::beacon::next_effect(b);
                self.msg(format!("The beacon will give: {}.", crate::beacon::effect_of(next).name()));
                next
            }
            c if crate::contraptions::is_comparator(c) => {
                // Flip between "anything in it" and "half full".
                let (f, more, _) = crate::contraptions::comparator_state(c);
                crate::contraptions::comparator(f, !more, false)
            }
            _ => return false,
        };
        self.world.set_v(pos, new);
        self.sfx(Sfx::Click, Some(pos.as_vec3() + Vec3::splat(0.5)));
        self.player.swing = 1.0;
        true
    }

    /// Placing Zappy Dust (an item that lays a wire on the floor).
    pub fn place_dust(&mut self, hit: IVec3, normal: IVec3, hit_id: Id) {
        let at = if replaceable(hit_id) { hit } else { hit + normal };
        if !replaceable(self.world.get_v(at)) || !is_solid(self.world.get_v(at - IVec3::Y)) || is_liquid(self.world.get_v(at)) {
            return;
        }
        self.world.set_v(at, WIRE);
        self.sfx(Sfx::Place(crate::sound::Mat::Stone), Some(at.as_vec3() + Vec3::splat(0.5)));
        self.player.swing = 1.0;
        if !self.creative {
            self.inv.consume_held();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switches_and_links() {
        assert!(source_on(LEVER_ON) && source_on(ZAP_BLOCK) && !source_on(LEVER) && !source_on(WIRE_ON));
        assert!(needs_floor(WIRE) && needs_floor(PLATE_ON) && !needs_floor(LAMP));
        assert!(is_zappy(WIRE) && is_zappy(LAMP_ON) && !is_zappy(ZAP_ORE) && !is_zappy(STONE));
        assert_eq!(placing_item(WIRE), Some(ZAP_DUST));
        assert_eq!(placing_item(WIRE_ON), None);
    }

    #[test]
    fn detector_rails_power_things_while_a_cart_is_on_them() {
        let mut g = crate::game::tests::arena(71);
        let rail = IVec3::new(0, 50, 2);
        let lamp = rail + IVec3::X;
        g.world.set_v(rail, DETECTOR_RAIL);
        g.world.set_v(lamp, LAMP);
        g.world.set_v(rail + IVec3::Z, RAIL_FIRST);
        let id = g.spawn_vehicle(crate::vehicles::CART_KIND, rail.as_vec3() + Vec3::new(0.5, 0.06, 0.5), 0.0);
        for _ in 0..10 {
            g.zap_tick(0.1);
        }
        assert!(crate::vehicles::detector_on(g.world.get_v(rail)), "on under a cart");
        assert_eq!(g.world.get_v(lamp), LAMP_ON);
        g.vehicles.retain(|v| v.id != id);
        for _ in 0..20 {
            g.zap_tick(0.1);
        }
        assert!(!crate::vehicles::detector_on(g.world.get_v(rail)) && crate::vehicles::is_detector(g.world.get_v(rail)));
        assert_eq!(g.world.get_v(lamp), LAMP);
    }
}
