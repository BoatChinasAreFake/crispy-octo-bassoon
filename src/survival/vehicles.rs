//! Boats, minecarts and rails.
//!
//! - **Boats** (five planks) float on water. Right-click to get in; W and S
//!   row, A and D steer, sneak to get out. On land they barely move.
//! - **Minecarts** (five iron) run on rails (six iron and a stick make 16).
//!   Rails join up with their neighbours by themselves, curving round
//!   corners. W pushes a cart the way you're looking. Powered rails (rails
//!   with Zappy Dust) speed carts up when powered and brake them when not.
//! - A **Minecart with Chest** carries 27 stacks and a **Minecart with
//!   Hopper** 5; right-click either to look inside. A hopper cart picks up
//!   what it rolls over and takes from containers above the track. A hopper
//!   under the track empties any loaded cart that stops or rolls over it,
//!   and a hopper pointing at a cart fills it.
//! - A **Detector Rail** powers things next to it while a cart is on it.
//! - Hit a vehicle a few times to pick it back up (spilling what's in it).
//!
//! Vehicles live where the world lives. A joined player drives the one
//! they're in themselves (telling the host where it is), and the host tells
//! everyone where all of them are.

use crate::block::*;
use crate::game::Game;
use crate::net::Msg;
use crate::render::{DynGeo, Pass};
use crate::sound::{Mat, Sfx};
use crate::texture::*;
use crate::world::World;
use macroquad::math::{IVec3, Mat4, Vec3};

pub const BOAT_KIND: u8 = 0;
pub const CART_KIND: u8 = 1;
pub const CHEST_CART_KIND: u8 = 2;
pub const HOPPER_CART_KIND: u8 = 3;
/// The new woods' boats (see woods.rs): `+ wood`.
pub const WOOD_BOAT_KIND: u8 = 4;
/// The highest kind there is.
pub const LAST_KIND: u8 = WOOD_BOAT_KIND + crate::woods::WOODS.len() as u8 - 1;

/// A boat of any wood.
pub fn is_boat(kind: u8) -> bool {
    kind == BOAT_KIND || (WOOD_BOAT_KIND..=LAST_KIND).contains(&kind)
}

/// Carts' contents are reached by a key that's no place in the world
/// (below bedrock, far west of even the Hollow; see containers.rs `store`).
const CART_KEY_X: i32 = i32::MIN + 1;
const CART_KEY_Y: i32 = -4096;
pub fn cart_key(id: u32) -> IVec3 {
    IVec3::new(CART_KEY_X, CART_KEY_Y, id as i32)
}
/// The cart a container key belongs to, if it's a cart's.
pub fn cart_of_key(p: IVec3) -> Option<u32> {
    (p.x == CART_KEY_X && p.y == CART_KEY_Y).then_some(p.z as u32)
}
/// The vehicle an item puts down.
pub fn kind_of_item(item: Id) -> Option<u8> {
    match item {
        BOAT => Some(BOAT_KIND),
        b if crate::woods::WOODS.iter().any(|w| w.boat_item == b) => crate::woods::wood_of_item(b).map(|w| WOOD_BOAT_KIND + w as u8),
        MINECART => Some(CART_KIND),
        CHEST_MINECART => Some(CHEST_CART_KIND),
        HOPPER_MINECART => Some(HOPPER_CART_KIND),
        _ => None,
    }
}
/// Top speeds (blocks a second), and how many hits pick one up.
const BOAT_SPEED: f32 = 7.0;
const CART_SPEED: f32 = 9.0;
const HITS: u8 = 3;
/// How often the host tells everyone where vehicles are.
const SYNC_SECS: f32 = 0.1;

#[derive(Clone, Debug, PartialEq)]
pub struct Vehicle {
    pub id: u32,
    pub kind: u8,
    pub pos: Vec3,
    pub vel: Vec3,
    pub yaw: f32,
    /// Who's in it: 0 nobody, else a player id + 1 (the host is 1).
    pub rider: u32,
    /// Carts: the way it's going along the rail (an axis), speed, and whether it's past the rail's middle.
    pub heading: IVec3,
    pub speed: f32,
    pub past_middle: bool,
    pub hits: u8,
    pub hurt: f32,
    /// What's in a chest or hopper cart.
    pub contents: Option<crate::containers::Container>,
}

impl Vehicle {
    pub fn new(id: u32, kind: u8, pos: Vec3, yaw: f32) -> Vehicle {
        let contents = match kind {
            CHEST_CART_KIND => Some(crate::containers::Container::for_block(CHEST)),
            HOPPER_CART_KIND => Some(crate::containers::Container::for_block(HOPPER_FIRST)),
            _ => None,
        };
        Vehicle { id, kind, pos, vel: Vec3::ZERO, yaw, rider: 0, heading: IVec3::Z, speed: 0.0, past_middle: false, hits: 0, hurt: 0.0, contents }
    }

    pub fn item(&self) -> Id {
        match self.kind {
            BOAT_KIND => BOAT,
            k if is_boat(k) => crate::woods::WOODS[(k - WOOD_BOAT_KIND) as usize].boat_item,
            CHEST_CART_KIND => CHEST_MINECART,
            HOPPER_CART_KIND => HOPPER_MINECART,
            _ => MINECART,
        }
    }

    /// Can someone sit in it? (Loaded carts are full of cargo.)
    pub fn rideable(&self) -> bool {
        self.contents.is_none()
    }

    /// The block its contents behave like (for the container screen and hoppers).
    pub fn container_block(&self) -> Id {
        if self.kind == HOPPER_CART_KIND { HOPPER_FIRST } else { CHEST }
    }

    /// The cell it's in.
    pub fn cell(&self) -> IVec3 {
        IVec3::new(self.pos.x.floor() as i32, self.pos.y.floor() as i32, self.pos.z.floor() as i32)
    }

    /// Its box, for pointing at and bumping.
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let (half, h) = if is_boat(self.kind) { (0.65, 0.55) } else { (0.45, 0.7) };
        (self.pos - Vec3::new(half, 0.0, half), self.pos + Vec3::new(half, h, half))
    }

    /// Where a rider sits.
    pub fn seat(&self) -> Vec3 {
        self.pos + Vec3::Y * if is_boat(self.kind) { 0.15 } else { 0.25 }
    }
}

// ------------------------------------------------------------------ rails

pub fn is_rail(id: Id) -> bool {
    (RAIL_FIRST..RAIL_FIRST + 6).contains(&id) || (POWERED_RAIL..POWERED_RAIL + 4).contains(&id) || is_detector(id)
}

pub fn is_detector(id: Id) -> bool {
    (DETECTOR_RAIL..DETECTOR_RAIL + 4).contains(&id)
}

/// A detector rail that has a cart on it (it powers what's around it).
pub fn detector_on(id: Id) -> bool {
    is_detector(id) && (id - DETECTOR_RAIL) % 2 == 1
}

pub fn is_powered_rail(id: Id) -> bool {
    (POWERED_RAIL..POWERED_RAIL + 4).contains(&id)
}

/// The two ways a rail leads: north-south, east-west, or round a corner.
pub fn rail_dirs(id: Id) -> Option<[IVec3; 2]> {
    let shape = if is_powered_rail(id) {
        ((id - POWERED_RAIL) / 2) as u8
    } else if is_detector(id) {
        ((id - DETECTOR_RAIL) / 2) as u8
    } else if is_rail(id) {
        (id - RAIL_FIRST) as u8
    } else {
        return None;
    };
    let (n, s, e, w) = (IVec3::NEG_Z, IVec3::Z, IVec3::X, IVec3::NEG_X);
    Some(match shape {
        0 => [n, s],
        1 => [e, w],
        2 => [n, e],
        3 => [n, w],
        4 => [s, e],
        _ => [s, w],
    })
}

/// The shape a plain rail at `p` should take, joining the rails around it.
pub fn rail_shape(world: &World, p: IVec3) -> Id {
    let has = |d: IVec3| is_rail(world.get_v(p + d));
    let (n, s, e, w) = (has(IVec3::NEG_Z), has(IVec3::Z), has(IVec3::X), has(IVec3::NEG_X));
    let shape = match (n, s, e, w) {
        (true, true, _, _) => 0,
        (_, _, true, true) => 1,
        (true, _, true, _) => 2,
        (true, _, _, true) => 3,
        (_, true, true, _) => 4,
        (_, true, _, true) => 5,
        (false, false, true, false) | (false, false, false, true) => 1,
        _ => 0,
    };
    RAIL_FIRST + shape
}

/// A powered rail at `p`: its axis from the rails around it, lit if powered.
pub fn powered_shape(world: &World, p: IVec3, on: bool) -> Id {
    let has = |d: IVec3| is_rail(world.get_v(p + d));
    let ew = (has(IVec3::X) || has(IVec3::NEG_X)) && !(has(IVec3::Z) || has(IVec3::NEG_Z));
    POWERED_RAIL + ew as Id * 2 + on as Id
}

/// A detector rail at `p`: its axis from the rails around it, on if a cart's there.
pub fn detector_shape(world: &World, p: IVec3, on: bool) -> Id {
    let has = |d: IVec3| is_rail(world.get_v(p + d));
    let ew = (has(IVec3::X) || has(IVec3::NEG_X)) && !(has(IVec3::Z) || has(IVec3::NEG_Z));
    DETECTOR_RAIL + ew as Id * 2 + on as Id
}

/// A cart's step along the rails. Returns false if it ran out of track.
fn ride_rails(world: &World, v: &mut Vehicle, dt: f32) -> bool {
    let mut left = v.speed * dt;
    for _ in 0..8 {
        let cell = IVec3::new(v.pos.x.floor() as i32, v.pos.y.floor() as i32, v.pos.z.floor() as i32);
        let Some(dirs) = rail_dirs(world.get_v(cell)) else { return false };
        if !dirs.contains(&v.heading) {
            // Just got here: head out the other way than we came in.
            let back = dirs.iter().position(|d| *d == -v.heading);
            v.heading = match back {
                Some(i) => dirs[1 - i],
                None => dirs[0],
            };
            v.past_middle = false;
        }
        let center = cell.as_vec3() + Vec3::new(0.5, 1.0 / 16.0, 0.5);
        let target = if v.past_middle { center + v.heading.as_vec3() * 0.5 } else { center };
        let to = target - v.pos;
        let d = to.length();
        if d > left {
            v.pos += to / d.max(1e-6) * left;
            v.yaw = (v.heading.x as f32).atan2(-(v.heading.z as f32));
            return true;
        }
        v.pos = target;
        left -= d;
        if !v.past_middle {
            v.past_middle = true;
            continue;
        }
        // Over the edge into the next cell.
        let next = cell + v.heading;
        match rail_dirs(world.get_v(next)) {
            Some(nd) if nd.contains(&-v.heading) => {
                // In at this edge, out at the rail's other end.
                let came = v.heading;
                v.pos += came.as_vec3() * 0.001;
                v.heading = if nd[0] == -came { nd[1] } else { nd[0] };
                v.past_middle = false;
            }
            _ => {
                v.speed = 0.0;
                return true;
            }
        }
    }
    true
}

impl Game {
    /// Right-click with a boat or minecart: put it down. True if it did.
    pub fn place_vehicle(&mut self, held: Id) -> bool {
        let eye = self.player.eye();
        let dir = self.player.look_dir();
        let reach = if self.creative { 6.5 } else { 5.0 };
        let Some(kind) = kind_of_item(held) else { return false };
        let (kind, at) = if is_boat(kind) {
            let Some(h) = self.world.raycast_liquid(eye, dir, reach).or_else(|| self.world.raycast(eye, dir, reach)) else { return false };
            let top = if is_liquid(self.world.get_v(h.pos)) { h.pos.as_vec3() + Vec3::new(0.5, 0.6, 0.5) } else { (h.pos + h.normal).as_vec3() + Vec3::new(0.5, 0.0, 0.5) };
            (kind, top)
        } else {
            let Some(h) = self.world.raycast(eye, dir, reach) else { return false };
            if !is_rail(self.world.get_v(h.pos)) {
                return false;
            }
            (kind, h.pos.as_vec3() + Vec3::new(0.5, 1.0 / 16.0, 0.5))
        };
        let yaw = self.player.yaw;
        if self.is_client() {
            self.net_send_msg(Msg::PlaceVehicle { kind, pos: at, yaw });
        } else {
            self.spawn_vehicle(kind, at, yaw);
        }
        if !self.creative {
            self.inv.consume_held();
        }
        self.sfx(Sfx::Place(Mat::Wood), Some(at));
        self.player.swing = 1.0;
        true
    }

    pub fn spawn_vehicle(&mut self, kind: u8, at: Vec3, yaw: f32) -> u32 {
        self.next_vehicle_id += 1;
        let id = self.next_vehicle_id;
        let mut v = Vehicle::new(id, kind, at, yaw);
        if !is_boat(kind) {
            // Face along the rail.
            let cell = IVec3::new(at.x.floor() as i32, at.y.floor() as i32, at.z.floor() as i32);
            if let Some(d) = rail_dirs(self.world.get_v(cell)) {
                v.heading = d[0];
            }
        }
        self.vehicles.push(v);
        id
    }

    /// Get in (the local player); a loaded cart opens up instead.
    pub fn mount(&mut self, i: usize) {
        if !self.vehicles[i].rideable() {
            let key = cart_key(self.vehicles[i].id);
            self.open_container(key);
            return;
        }
        let me = self.my_id + 1;
        if self.vehicles[i].rider != 0 && self.vehicles[i].rider != me {
            return;
        }
        let id = self.vehicles[i].id;
        self.vehicles[i].rider = me;
        self.riding = Some(id);
        if self.is_client() {
            self.net_send_msg(Msg::VehicleUse { id, action: 0 });
        }
        self.sfx(Sfx::Place(Mat::Wood), None);
    }

    /// Get out, next to it.
    pub fn dismount(&mut self) {
        let Some(id) = self.riding.take() else { return };
        if let Some(v) = self.vehicles.iter_mut().find(|v| v.id == id) {
            v.rider = 0;
            let side = Vec3::new(v.yaw.cos(), 0.0, v.yaw.sin());
            let spot = v.pos + side * 1.2 + Vec3::Y * 0.6;
            self.player.body.pos = spot;
            self.player.body.vel = Vec3::ZERO;
            self.player.fall_start = spot.y;
        }
        if self.is_client() {
            self.net_send_msg(Msg::VehicleUse { id, action: 1 });
        }
    }

    /// Hit a vehicle: a few hits and it pops off as an item.
    pub fn hit_vehicle(&mut self, id: u32, by_rider: u32) {
        let Some(i) = self.vehicles.iter().position(|v| v.id == id) else { return };
        let creative = self.creative;
        let v = &mut self.vehicles[i];
        v.hits += if creative { HITS } else { 1 };
        v.hurt = 0.4;
        let (pos, hits) = (v.pos, v.hits);
        self.sfx(Sfx::Hit(Mat::Wood), Some(pos));
        if hits >= HITS {
            let v = self.vehicles.remove(i);
            if !self.creative {
                self.pop_drop(v.pos + Vec3::Y * 0.4, v.item(), 1);
            }
            // Whatever it carried spills out.
            if let Some(c) = &v.contents {
                for (item, n, wear) in c.contents() {
                    self.pop_drop_worn(v.pos + Vec3::Y * 0.4, item, n, wear);
                }
            }
            if self.open == Some(cart_key(v.id)) {
                self.open = None;
            }
            if self.riding == Some(v.id) {
                self.riding = None;
            }
            let _ = by_rider;
        }
    }

    /// Move vehicles: the one we ride by our controls (wherever we are), the rest where the world lives.
    pub fn vehicles_tick(&mut self, dt: f32, forward: f32, strafe: f32, sneak: bool) {
        for v in self.vehicles.iter_mut() {
            v.hurt = (v.hurt - dt).max(0.0);
        }
        let me = self.my_id + 1;
        let riding = self.riding;
        if let Some(id) = riding
            && sneak
        {
            let _ = id;
            self.dismount();
        }
        let look = self.player.look_dir();
        let host = !self.is_client();
        // Riders who've left leave their seats empty.
        if host {
            let peers: Vec<u32> = self.peers.keys().copied().collect();
            let local = if self.away() { None } else { self.riding };
            for v in self.vehicles.iter_mut() {
                let gone = v.rider != 0 && v.rider != me && !peers.contains(&(v.rider - 1));
                if gone || (v.rider == me && local != Some(v.id)) {
                    v.rider = 0;
                }
            }
        }
        let mut moved = None;
        for v in self.vehicles.iter_mut() {
            let mine = Some(v.id) == self.riding;
            // Other players drive their own; clients only drive theirs.
            if !mine && (v.rider != 0 || !host) {
                continue;
            }
            let (f, s) = if mine { (forward, strafe) } else { (0.0, 0.0) };
            if is_boat(v.kind) {
                boat_step(&self.world, v, dt, f, s);
            } else {
                cart_step(&self.world, v, dt, if mine { f } else { 0.0 }, look);
            }
            if mine {
                moved = Some((v.id, v.pos, v.yaw, v.seat()));
                v.rider = me;
            }
        }
        if let Some((id, pos, yaw, seat)) = moved {
            self.player.body.pos = seat;
            self.player.body.vel = Vec3::ZERO;
            self.player.fall_start = seat.y;
            if self.is_client() {
                self.vehicle_sync -= dt;
                if self.vehicle_sync <= 0.0 {
                    self.vehicle_sync = SYNC_SECS;
                    self.net_send_msg(Msg::Ride { id, pos, yaw });
                }
            }
        }
        if host {
            self.vehicle_sync -= dt;
            if self.vehicle_sync <= 0.0 && self.net.is_some() {
                self.vehicle_sync = SYNC_SECS;
                let list = self.vehicles.iter().map(|v| (v.id, v.kind, v.pos, v.yaw, v.rider)).collect();
                self.net_broadcast(Msg::Vehicles(list));
            }
        }
    }

    /// Joined players: the host's list of vehicles (keeping our own ride as we have it).
    pub fn apply_vehicles(&mut self, list: Vec<(u32, u8, Vec3, f32, u32)>) {
        let mine = self.riding;
        let old = std::mem::take(&mut self.vehicles);
        for (id, kind, pos, yaw, rider) in list {
            if !pos.is_finite() || kind > LAST_KIND {
                continue;
            }
            if Some(id) == mine
                && let Some(v) = old.iter().find(|v| v.id == id)
            {
                self.vehicles.push(v.clone());
                continue;
            }
            let mut v = old.iter().find(|v| v.id == id && v.kind == kind).cloned().unwrap_or_else(|| Vehicle::new(id, kind, pos, yaw));
            v.pos += (pos - v.pos) * 0.5;
            if v.pos.distance(pos) > 4.0 {
                v.pos = pos;
            }
            v.yaw = yaw;
            v.rider = rider;
            self.vehicles.push(v);
        }
        if mine.is_some_and(|id| !self.vehicles.iter().any(|v| v.id == id)) {
            self.riding = None;
        }
    }

    /// The host: a joined player's vehicle actions.
    pub fn host_vehicle_use(&mut self, from: u32, id: u32, action: u8) {
        let Some(me) = self.peers.get(&from).map(|p| p.target) else { return };
        let Some(i) = self.vehicles.iter().position(|v| v.id == id && v.pos.distance(me) < 7.0) else { return };
        let rider = from + 1;
        match action {
            0 if self.vehicles[i].rider == 0 && self.vehicles[i].rideable() => self.vehicles[i].rider = rider,
            1 if self.vehicles[i].rider == rider => self.vehicles[i].rider = 0,
            2 => self.hit_vehicle(id, rider),
            _ => {}
        }
    }

    /// The host: where a joined player has driven the vehicle they're in.
    pub fn host_ride(&mut self, from: u32, id: u32, pos: Vec3, yaw: f32) {
        if !pos.is_finite() || !yaw.is_finite() {
            return;
        }
        if let Some(v) = self.vehicles.iter_mut().find(|v| v.id == id && v.rider == from + 1)
            && v.pos.distance(pos) < 12.0
        {
            v.pos = pos;
            v.yaw = yaw;
        }
    }

    /// The host: a joined player put down a boat or cart.
    pub fn host_place_vehicle(&mut self, from: u32, kind: u8, pos: Vec3, yaw: f32) {
        let Some(me) = self.peers.get(&from).map(|p| p.target) else { return };
        let item = Vehicle::new(0, kind, Vec3::ZERO, 0.0).item();
        if kind > LAST_KIND || !pos.is_finite() || pos.distance(me) > 8.0 || !self.peer_take(from, item, 1) {
            return;
        }
        self.spawn_vehicle(kind, pos, yaw);
    }

    /// Draw every vehicle.
    pub fn draw_vehicles(&self, g: &mut DynGeo) {
        for v in &self.vehicles {
            let sky = self.world.sky_shade(v.pos.x.floor() as i32, (v.pos.y + 0.5).floor() as i32, v.pos.z.floor() as i32).max(0.25);
            let tint = if v.hurt > 0.0 { [1.0, 0.5, 0.5, 1.0] } else { [1.0; 4] };
            g.begin(Pass::Opaque, tint, false);
            let root = Mat4::from_translation(v.pos) * Mat4::from_rotation_y(-v.yaw);
            let boxes: &[([f32; 3], [f32; 3])] = if is_boat(v.kind) {
                &[([-0.6, 0.0, -0.8], [1.2, 0.12, 1.6]), ([-0.6, 0.12, -0.8], [0.1, 0.35, 1.6]), ([0.5, 0.12, -0.8], [0.1, 0.35, 1.6]), ([-0.5, 0.12, -0.8], [1.0, 0.35, 0.1]), ([-0.5, 0.12, 0.7], [1.0, 0.35, 0.1])]
            } else {
                &[([-0.45, 0.1, -0.55], [0.9, 0.08, 1.1]), ([-0.45, 0.1, -0.55], [0.08, 0.55, 1.1]), ([0.37, 0.1, -0.55], [0.08, 0.55, 1.1]), ([-0.37, 0.1, -0.55], [0.74, 0.55, 0.08]), ([-0.37, 0.1, 0.47], [0.74, 0.55, 0.08])]
            };
            let tile = match v.kind {
                BOAT_KIND => T_PLANKS,
                k if is_boat(k) => crate::woods::WOODS[(k - WOOD_BOAT_KIND) as usize].planks,
                _ => T_CART,
            };
            for &(min, size) in boxes {
                let m = root * Mat4::from_translation(Vec3::from_array(min)) * Mat4::from_scale(Vec3::from_array(size));
                g.cube(&m, [tile; 6], sky, [0.0, 0.0, 1.0, 1.0]);
            }
            // The cargo: a chest, or a hopper's funnel.
            let cargo: Option<([f32; 3], [f32; 3], [u16; 6])> = match v.kind {
                CHEST_CART_KIND => Some(([-0.35, 0.18, -0.4], [0.7, 0.62, 0.8], [T_CHEST_SIDE, T_CHEST_SIDE, T_CHEST_TOP, T_CHEST_TOP, T_CHEST_SIDE, T_CHEST_SIDE])),
                HOPPER_CART_KIND => Some(([-0.37, 0.4, -0.45], [0.74, 0.45, 0.9], [T_HOPPER_SIDE, T_HOPPER_SIDE, T_HOPPER_TOP, T_HOPPER_SIDE, T_HOPPER_SIDE, T_HOPPER_SIDE])),
                _ => None,
            };
            if let Some((min, size, tiles)) = cargo {
                let m = root * Mat4::from_translation(Vec3::from_array(min)) * Mat4::from_scale(Vec3::from_array(size));
                g.cube(&m, tiles, sky, [0.0, 0.0, 1.0, 1.0]);
            }
        }
    }
}

/// How fast a boat can go on this block, if it's ice.
fn ice_speed(id: Id) -> Option<f32> {
    match id {
        ICE => Some(22.0),
        PACKED_ICE => Some(30.0),
        _ => None,
    }
}

/// A boat: floats, rows, steers; crawls on land; flies along ice.
fn boat_step(world: &World, v: &mut Vehicle, dt: f32, forward: f32, strafe: f32) {
    v.yaw += strafe * 2.2 * dt;
    let dir = Vec3::new(v.yaw.sin(), 0.0, -v.yaw.cos());
    let cell = |p: Vec3| IVec3::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
    let under = world.get_v(cell(v.pos + Vec3::Y * 0.2));
    let wet = is_water(under) || is_water(world.get_v(cell(v.pos + Vec3::Y * 0.6)));
    // On ice a boat hardly slows at all, and goes much faster than it rows.
    let ice = if wet { None } else { ice_speed(world.get_v(cell(v.pos - Vec3::Y * 0.1))) };
    let mut cap = BOAT_SPEED;
    if let Some(top) = ice {
        v.vel.y -= crate::entity::GRAVITY * dt;
        v.vel += dir * forward * 14.0 * dt;
        let drag = (1.0 - 0.15 * dt).max(0.0);
        v.vel.x *= drag;
        v.vel.z *= drag;
        // It slides the way it's pointing, mostly (or you'd never get round a bend).
        let flat = Vec3::new(v.vel.x, 0.0, v.vel.z);
        let along = flat.dot(dir);
        let turned = dir * along + (flat - dir * along) * (1.0 - 2.5 * dt).max(0.0);
        v.vel.x = turned.x;
        v.vel.z = turned.z;
        cap = top;
    } else if wet {
        // Float at the surface.
        let mut top = cell(v.pos + Vec3::Y * 0.2);
        while is_water(world.get_v(top + IVec3::Y)) {
            top += IVec3::Y;
        }
        let surface = top.y as f32 + 0.85;
        v.vel.y += (surface - v.pos.y) * 12.0 * dt - v.vel.y * 4.0 * dt;
        v.vel += dir * forward * 6.0 * dt;
        let drag = (1.0 - 0.9 * dt).max(0.0);
        v.vel.x *= drag;
        v.vel.z *= drag;
    } else {
        v.vel.y -= crate::entity::GRAVITY * dt;
        v.vel += dir * forward * 1.0 * dt;
        let drag = (1.0 - 8.0 * dt).max(0.0);
        v.vel.x *= drag;
        v.vel.z *= drag;
    }
    let flat = Vec3::new(v.vel.x, 0.0, v.vel.z);
    if flat.length() > cap {
        let k = cap / flat.length();
        v.vel.x *= k;
        v.vel.z *= k;
    }
    let mut body = crate::entity::Body::new(v.pos, 0.55, 0.5);
    body.vel = v.vel;
    crate::entity::move_body(world, &mut body, dt, false);
    v.pos = body.pos;
    v.vel = body.vel;
    if body.on_ground && v.vel.y < 0.0 {
        v.vel.y = 0.0;
    }
}

/// A cart: along the rails, with pushes, boosts and brakes; otherwise it just sits.
fn cart_step(world: &World, v: &mut Vehicle, dt: f32, forward: f32, look: Vec3) {
    let cell = IVec3::new(v.pos.x.floor() as i32, v.pos.y.floor() as i32, v.pos.z.floor() as i32);
    let rail = world.get_v(cell);
    if rail_dirs(rail).is_none() {
        // Off the rails: fall, and slide to a stop.
        v.vel.y -= crate::entity::GRAVITY * dt;
        let mut body = crate::entity::Body::new(v.pos, 0.45, 0.7);
        body.vel = Vec3::new(v.vel.x * (1.0 - 4.0 * dt).max(0.0), v.vel.y, v.vel.z * (1.0 - 4.0 * dt).max(0.0));
        crate::entity::move_body(world, &mut body, dt, false);
        v.pos = body.pos;
        v.vel = body.vel;
        v.speed = 0.0;
        return;
    }
    v.vel = Vec3::ZERO;
    // The rider pushes the way they're looking: forward along the rail, or turn it round.
    if forward > 0.0 {
        let along = v.heading.as_vec3().dot(Vec3::new(look.x, 0.0, look.z).normalize_or_zero());
        if along < -0.3 && v.speed < 1.0 {
            v.heading = -v.heading;
            v.past_middle = !v.past_middle;
        }
        v.speed += 5.0 * dt;
    }
    if is_powered_rail(rail) {
        if (rail - POWERED_RAIL) % 2 == 1 {
            v.speed = (v.speed + 14.0 * dt).max(if v.speed < 0.1 { 2.0 } else { 0.0 });
        } else {
            v.speed *= (1.0 - 6.0 * dt).max(0.0);
        }
    }
    v.speed = (v.speed * (1.0 - 0.15 * dt)).clamp(0.0, CART_SPEED);
    if v.speed < 0.02 {
        v.speed = 0.0;
        return;
    }
    if !ride_rails(world, v, dt) {
        v.vel = Vec3::new(v.heading.x as f32, 0.0, v.heading.z as f32) * v.speed;
        v.speed = 0.0;
    }
}

/// The count's top bit marks the newer layout, where each vehicle is
/// followed by its cargo (a length, then `containers::encode` of it).
const WITH_CARGO: u32 = 1 << 31;

/// Pack vehicles for the save file (nobody's in them when it loads).
pub fn encode(vehicles: &[Vehicle]) -> Vec<u8> {
    let mut out = (vehicles.len() as u32 | WITH_CARGO).to_le_bytes().to_vec();
    for v in vehicles {
        out.push(v.kind);
        for f in [v.pos.x, v.pos.y, v.pos.z, v.yaw] {
            out.extend_from_slice(&f.to_le_bytes());
        }
        let cargo = v.contents.as_ref().map(|c| crate::containers::encode(&std::collections::HashMap::from([(IVec3::ZERO, c.clone())]))).unwrap_or_default();
        out.extend_from_slice(&(cargo.len() as u32).to_le_bytes());
        out.extend_from_slice(&cargo);
    }
    out
}

/// Unpack `encode` (and the older layout without cargo): kind, place, yaw, cargo.
pub fn decode(b: &[u8]) -> Vec<(u8, Vec3, f32, Option<crate::containers::Container>)> {
    let head = b.get(0..4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]])).unwrap_or(0);
    let (n, cargo) = ((head & !WITH_CARGO) as usize, head & WITH_CARGO != 0);
    let mut v = Vec::new();
    let mut at = 4;
    for _ in 0..n.min(10_000) {
        let Some(s) = b.get(at..at + 17) else { break };
        at += 17;
        let f: Vec<f32> = (0..4).map(|i| f32::from_le_bytes([s[1 + i * 4], s[2 + i * 4], s[3 + i * 4], s[4 + i * 4]])).collect();
        let mut contents = None;
        if cargo {
            let Some(len) = b.get(at..at + 4).map(|l| u32::from_le_bytes([l[0], l[1], l[2], l[3]]) as usize) else { break };
            at += 4;
            let Some(blob) = b.get(at..at + len) else { break };
            at += len;
            contents = crate::containers::decode(blob, 4).remove(&IVec3::ZERO);
        }
        if s[0] <= LAST_KIND && f.iter().all(|x| x.is_finite()) {
            v.push((s[0], Vec3::new(f[0], f[1], f[2]), f[3], contents));
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boats_fly_along_ice_and_wood_boats_go_down() {
        let mut g = crate::game::tests::arena(61);
        // A long strip of ice to row along (north), and the same over plain ground.
        let speed_on = |g: &mut Game, floor: Id| {
            for z in -15..4 {
                for x in -2..=2 {
                    g.world.set_v(IVec3::new(x, 49, z), floor);
                    for y in 50..53 {
                        g.world.set_v(IVec3::new(x, y, z), AIR);
                    }
                }
            }
            let mut v = Vehicle::new(1, BOAT_KIND, Vec3::new(0.5, 50.0, 2.5), 0.0);
            // (A second's rowing: the arena only reaches so far.)
            for _ in 0..60 {
                boat_step(&g.world, &mut v, 1.0 / 60.0, 1.0, 0.0);
            }
            Vec3::new(v.vel.x, 0.0, v.vel.z).length()
        };
        let ground = speed_on(&mut g, STONE);
        let ice = speed_on(&mut g, ICE);
        assert!(ice > BOAT_SPEED && ice > ground * 4.0, "ice {ice} vs ground {ground}");
        assert!(ice_speed(PACKED_ICE) > ice_speed(ICE), "packed ice is quicker still");
        // Every wood's boat goes down like the plain one.
        for w in crate::woods::WOODS.iter() {
            assert!(kind_of_item(w.boat_item).is_some_and(is_boat));
        }
    }

    #[test]
    fn rails_join_up() {
        assert_eq!(rail_dirs(RAIL_FIRST), Some([IVec3::NEG_Z, IVec3::Z]));
        assert_eq!(rail_dirs(RAIL_FIRST + 2), Some([IVec3::NEG_Z, IVec3::X]));
        assert!(rail_dirs(STONE).is_none());
        assert!(is_rail(POWERED_RAIL + 3) && is_powered_rail(POWERED_RAIL + 1) && !is_powered_rail(RAIL_FIRST));
        assert!(is_rail(DETECTOR_RAIL + 2) && detector_on(DETECTOR_RAIL + 3) && !detector_on(DETECTOR_RAIL));
        assert_eq!(rail_dirs(DETECTOR_RAIL + 2), Some([IVec3::X, IVec3::NEG_X]));
    }

    #[test]
    fn carts_keep_their_cargo_in_saves() {
        let mut chest = Vehicle::new(4, CHEST_CART_KIND, Vec3::new(1.5, 50.0, 2.5), 0.3);
        chest.contents.as_mut().unwrap().slots[10] = Some((DIAMOND, 5));
        let plain = Vehicle::new(5, CART_KIND, Vec3::new(3.5, 50.0, 2.5), 0.0);
        let back = decode(&encode(&[chest, plain]));
        assert_eq!(back.len(), 2);
        assert_eq!(back[0].3.as_ref().unwrap().slots[10], Some((DIAMOND, 5)));
        assert!(back[1].3.is_none() && back[1].0 == CART_KIND);
        // The old layout (no cargo) still reads.
        let mut old = 1u32.to_le_bytes().to_vec();
        old.push(BOAT_KIND);
        for f in [1.0f32, 2.0, 3.0, 0.5] {
            old.extend_from_slice(&f.to_le_bytes());
        }
        assert_eq!(decode(&old).len(), 1);
        assert_eq!(cart_of_key(cart_key(77)), Some(77));
        assert_eq!(cart_of_key(IVec3::new(1, 2, 3)), None);
    }
}
