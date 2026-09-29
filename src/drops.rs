//! Items lying on the ground: what breaking blocks, killing mobs, explosions,
//! broken chests and Q leave behind.
//!
//! They live where the world lives (single player, host, server), fall, slide,
//! bob, merge with matching neighbours and vanish after five minutes. Walk into
//! one to pick it up. Joined players see the host's copies and ask for them;
//! the host checks they're close enough and hands the items over through the
//! ledger (see ledger.rs), so pickups can't be faked either.

use crate::block::*;
use crate::entity::{move_body, Body, GRAVITY};
use crate::game::Game;
use crate::net::Msg;
use crate::render::{DynGeo, Pass};
use crate::sound::Sfx;
use crate::world::World;
use macroquad::math::{Mat4, Vec3};

/// Seconds before an item left alone disappears.
pub const DESPAWN_SECS: f32 = 300.0;
/// How close (from the middle of a player) an item has to be to be picked up.
pub const PICKUP_RANGE: f32 = 1.5;
/// Items thrown with Q can't be picked up again straight away.
pub const THROW_DELAY: f32 = 1.5;
/// Anything else (block and mob drops) waits this long, so you see it pop out.
pub const DROP_DELAY: f32 = 0.3;
/// The host never keeps more than this many; the oldest go first.
const MAX_DROPS: usize = 1024;

pub struct ItemDrop {
    pub id: u32,
    pub item: Id,
    pub n: u8,
    pub body: Body,
    pub age: f32,
    /// Seconds (of age) before it can be picked up.
    pub delay: f32,
    /// A used tool or piece of armour keeps its wear on the ground.
    pub wear: crate::inventory::Wear,
    /// Joined players: the host's latest position, and when we last asked for it.
    pub net_pos: Vec3,
    pub asked: f32,
}

impl ItemDrop {
    pub fn new(id: u32, item: Id, n: u8, pos: Vec3, vel: Vec3, delay: f32) -> ItemDrop {
        let mut body = Body::new(pos, 0.125, 0.25);
        body.vel = vel;
        ItemDrop { id, item, n, body, age: 0.0, delay, wear: 0, net_pos: pos, asked: 0.0 }
    }

    fn update(&mut self, dt: f32, world: &World) {
        self.age += dt;
        let p = self.body.pos;
        if !world.is_loaded(p.x.floor() as i32, p.z.floor() as i32) {
            return;
        }
        if self.body.in_water && self.body.vel.y < 1.5 {
            // Bob to the surface (unless it's flying out, like a catch), and drift with the current.
            self.body.vel.y = (self.body.vel.y + 6.0 * dt).min(1.5);
            let k = (1.0 - 3.0 * dt).max(0.0);
            self.body.vel.x *= k;
            self.body.vel.z *= k;
            self.body.vel += crate::liquids::current(world, p + Vec3::Y * 0.1) * 6.0 * dt;
        } else {
            self.body.vel.y -= GRAVITY * dt;
        }
        move_body(world, &mut self.body, dt, false);
        if self.body.on_ground {
            let k = (1.0 - 10.0 * dt).max(0.0);
            self.body.vel.x *= k;
            self.body.vel.z *= k;
        }
    }

    pub fn can_pick_up(&self) -> bool {
        self.age >= self.delay
    }

    /// Where it's drawn: a little above the ground, bobbing.
    fn draw_pos(&self, clock: f32) -> Vec3 {
        self.body.pos + Vec3::Y * (0.12 + ((clock * 2.5 + self.id as f32).sin() * 0.06))
    }

    pub fn draw(&self, g: &mut DynGeo, world: &World, clock: f32) {
        let at = self.draw_pos(clock);
        let sky = world.sky_light(at.x.floor() as i32, at.y.floor() as i32 + 1, at.z.floor() as i32);
        let spin = clock * 1.6 + self.id as f32 * 0.7;
        let copies = if self.n > 16 { 3 } else if self.n > 1 { 2 } else { 1 };
        for c in 0..copies {
            let off = Vec3::new(c as f32 * 0.07, c as f32 * 0.05, -(c as f32) * 0.06);
            let root = Mat4::from_translation(at + off) * Mat4::from_rotation_y(spin);
            draw_item(g, &root, self.item, 0.26, sky);
        }
    }
}

/// An item as a little 3D thing centred on `root`'s origin (bottom at y = 0):
/// blocks as scaled-down blocks, everything else as a two-sided sprite.
pub fn draw_item(g: &mut DynGeo, root: &Mat4, item: Id, size: f32, sky: f32) {
    g.begin(Pass::Opaque, [1.0; 4], false);
    if is_block_item(item) && matches!(block(item).model, Model::Cube | Model::Shaped) {
        let t = block(item).tex;
        let tiles = [t[1], t[1], t[0], t[2], t[1], t[1]];
        let (boxes, n) = block_boxes(item);
        for &(a, b) in &boxes[..n] {
            let (a, b) = (Vec3::from_array(a), Vec3::from_array(b));
            let m = *root * Mat4::from_scale(Vec3::splat(size)) * Mat4::from_translation(a - Vec3::new(0.5, 0.0, 0.5)) * Mat4::from_scale(b - a);
            g.cube(&m, tiles, sky, [a.x, 1.0 - b.y, b.x, 1.0 - a.y]);
        }
    } else {
        let tile = if is_block_item(item) { block(item).tex[1] } else { item_tile(item) };
        let s = size * 1.5;
        let c = [Vec3::new(-s / 2.0, 0.0, 0.0), Vec3::new(s / 2.0, 0.0, 0.0), Vec3::new(s / 2.0, s, 0.0), Vec3::new(-s / 2.0, s, 0.0)].map(|p| root.transform_point3(p));
        g.quad(c, tile, [0.0, 0.0, 1.0, 1.0], [1.0, sky]);
        g.quad([c[1], c[0], c[3], c[2]], tile, [1.0, 0.0, 0.0, 1.0], [1.0, sky]);
    }
}

// ------------------------------------------------------------------ saving

/// Save format: v8 saves had 19 bytes per drop, v9 adds two of wear, v11 four.
pub fn encode(drops: &[ItemDrop]) -> Vec<u8> {
    let mut out = Vec::new();
    for d in drops {
        for v in [d.body.pos.x, d.body.pos.y, d.body.pos.z, d.age] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(&d.item.to_le_bytes());
        out.push(d.n);
        out.extend_from_slice(&d.wear.to_le_bytes());
    }
    out
}

/// A saved drop: position, item, count, age and wear.
pub type SavedDrop = (Vec3, Id, u8, f32, crate::inventory::Wear);

/// Unpack `encode`'s output (`wear_bytes` per drop: 0 before save v9, 2
/// before v11, then 4); stops at anything malformed.
pub fn decode(b: &[u8], wear_bytes: usize) -> Vec<SavedDrop> {
    let mut v = Vec::new();
    for c in b.chunks_exact(19 + wear_bytes) {
        let f = |o: usize| f32::from_le_bytes([c[o], c[o + 1], c[o + 2], c[o + 3]]);
        let (pos, age) = (Vec3::new(f(0), f(4), f(8)), f(12));
        let (item, n) = (u16::from_le_bytes([c[16], c[17]]), c[18]);
        if !pos.is_finite() || !age.is_finite() || n == 0 {
            break;
        }
        let wear = match wear_bytes {
            2 => u16::from_le_bytes([c[19], c[20]]) as u32,
            4 => u32::from_le_bytes([c[19], c[20], c[21], c[22]]),
            _ => 0,
        };
        v.push((pos, item, n.min(64), age.clamp(0.0, DESPAWN_SECS), wear));
    }
    v
}

impl Game {
    /// Put items on the ground (where the world lives; joined players never call this).
    pub fn spawn_drop(&mut self, at: Vec3, item: Id, n: u8, wear: crate::inventory::Wear, vel: Vec3, delay: f32) {
        if n == 0 || !valid_item(item) || self.is_client() {
            return;
        }
        self.next_drop_id = self.next_drop_id.wrapping_add(1).max(1);
        let mut d = ItemDrop::new(self.next_drop_id, item, n, at, vel, delay);
        d.wear = crate::inventory::sanitize_wear(item, wear);
        self.drops.push(d);
        if self.drops.len() > MAX_DROPS {
            self.drops.remove(0);
        }
    }

    /// Items popping out of something at `at` in a random direction.
    pub fn pop_drop(&mut self, at: Vec3, item: Id, n: u8) {
        self.pop_drop_worn(at, item, n, 0);
    }

    pub fn pop_drop_worn(&mut self, at: Vec3, item: Id, n: u8, wear: crate::inventory::Wear) {
        let vel = self.pop_velocity();
        self.spawn_drop(at - Vec3::Y * 0.125, item, n, wear, vel, DROP_DELAY);
    }

    fn pop_velocity(&mut self) -> Vec3 {
        let r = &mut self.rng;
        Vec3::new(r.range(-1.5, 1.5), r.range(3.0, 5.0), r.range(-1.5, 1.5))
    }

    /// Q: throw one of the held item (or the whole stack) the way we're looking.
    pub fn throw_held(&mut self, all: bool) {
        let Some((item, count)) = self.inv.slots[self.inv.selected] else { return };
        let n = if all { count } else { 1 };
        let wear = self.inv.wear[self.inv.selected];
        self.inv.slots[self.inv.selected] = if count > n { Some((item, count - n)) } else { None };
        self.throw_stack(item, n, wear);
    }

    /// Throw items that have already left the inventory (held, or on the cursor).
    pub fn throw_stack(&mut self, item: Id, n: u8, wear: crate::inventory::Wear) {
        self.player.swing = 1.0;
        self.advance("butterfingers");
        if self.is_client() {
            // The host takes them from its ledger and puts them on the ground.
            if !self.creative {
                self.net_send_msg(Msg::DropItem { item, n, wear, scatter: false });
            }
            return;
        }
        let (eye, dir) = (self.player.eye(), self.player.look_dir());
        self.spawn_drop(eye - Vec3::Y * 0.3 + dir * 0.3, item, n, wear, dir * 6.0 + Vec3::Y * 1.5, THROW_DELAY);
    }

    /// Died without keep-inventory: everything (armour and the cursor too) falls out.
    pub fn drop_everything(&mut self) {
        let inv = &mut self.inv;
        let mut stacks: Vec<(Id, u8, crate::inventory::Wear)> = Vec::new();
        for (s, w) in inv.slots.iter_mut().zip(inv.wear.iter_mut()).chain(inv.armor.iter_mut().zip(inv.armor_wear.iter_mut())).chain(std::iter::once((&mut inv.cursor, &mut inv.cursor_wear))) {
            if let Some((id, n)) = s.take() {
                stacks.push((id, n, *w));
            }
            *w = 0;
        }
        let at = self.player.body.pos + Vec3::Y * 0.6;
        for (item, n, wear) in stacks {
            if self.is_client() {
                self.net_send_msg(Msg::DropItem { item, n, wear, scatter: true });
            } else {
                let vel = self.pop_velocity() * 0.8;
                self.spawn_drop(at, item, n, wear, vel, THROW_DELAY);
            }
        }
    }

    /// A joined player threw something (Q, a full inventory), or died (`scatter`).
    pub fn host_throw(&mut self, from: u32, item: Id, n: u8, wear: crate::inventory::Wear, scatter: bool) {
        let Some((pos, yaw, pitch)) = self.peers.get(&from).map(|p| (p.target, p.yaw, p.pitch)) else { return };
        if n == 0 || !valid_item(item) || !self.peer_take(from, item, n as u32) {
            return;
        }
        let wear = self.launder(from, item, wear);
        if scatter {
            let vel = self.pop_velocity() * 0.8;
            self.spawn_drop(pos + Vec3::Y * 0.6, item, n, wear, vel, THROW_DELAY);
            return;
        }
        let dir = Vec3::new(yaw.sin() * pitch.cos(), pitch.sin(), -yaw.cos() * pitch.cos());
        self.spawn_drop(pos + Vec3::Y * 1.3 + dir * 0.3, item, n, wear, dir * 6.0 + Vec3::Y * 1.5, THROW_DELAY);
    }

    /// Where the world lives: move, merge, expire, and let the local player pick up.
    pub fn drops_tick(&mut self, dt: f32) {
        for d in self.drops.iter_mut() {
            d.update(dt, &self.world);
        }
        // Lava eats things.
        let burnt: Vec<Vec3> = self.drops.iter().filter(|d| d.body.in_lava).map(|d| d.body.pos).collect();
        for p in burnt {
            self.sfx(Sfx::Hiss, Some(p));
            self.smoke(p + Vec3::Y * 0.3, 4, 0.2);
        }
        self.drops.retain(|d| d.age < DESPAWN_SECS && d.body.pos.y > -16.0 && !d.body.in_lava);
        self.drop_timer += dt;
        if self.drop_timer >= 0.5 {
            self.drop_timer = 0.0;
            merge(&mut self.drops);
        }
        // The local player walks into things.
        if self.dedicated || self.menu || self.dead.is_some() {
            return;
        }
        let me = self.player.body.pos + Vec3::Y * 0.9;
        let mut got = Vec::new();
        for d in self.drops.iter_mut() {
            if !d.can_pick_up() || d.body.pos.distance(me) > PICKUP_RANGE {
                continue;
            }
            let room = self.inv.room_for(d.item).min(d.n as u32) as u8;
            if room == 0 {
                continue;
            }
            self.inv.add_worn(d.item, room, d.wear);
            d.n -= room;
            got.push(d.item);
        }
        if !got.is_empty() {
            self.drops.retain(|d| d.n > 0);
            self.sfx(Sfx::Pop, None);
            for item in got {
                self.item_advancements(item);
            }
        }
    }

    /// A joined player asks for a drop they walked into.
    pub fn host_pickup(&mut self, from: u32, id: u32, room: u8) {
        let Some(me) = self.peers.get(&from).filter(|p| p.alive()).map(|p| p.target + Vec3::Y * 0.9) else { return };
        let Some(i) = self.drops.iter().position(|d| d.id == id) else { return };
        let d = &mut self.drops[i];
        // A little extra reach for lag.
        if !d.can_pick_up() || d.body.pos.distance(me) > PICKUP_RANGE + 2.0 || room == 0 {
            return;
        }
        let n = room.min(d.n);
        let (item, wear) = (d.item, d.wear);
        d.n -= n;
        if d.n == 0 {
            self.drops.remove(i);
        }
        self.give_peer_worn(from, item, n, wear);
    }

    /// The host tells joined players what's on the ground, a few times a second.
    pub fn send_drops(&mut self, dt: f32) {
        self.drop_sync += dt;
        if self.drop_sync < 0.2 {
            return;
        }
        self.drop_sync = 0.0;
        if self.drops.is_empty() && self.drops_sent_empty {
            return;
        }
        self.drops_sent_empty = self.drops.is_empty();
        let list = self.drops.iter().map(|d| (d.id, d.body.pos, d.item, d.n)).collect();
        self.net_broadcast(Msg::Drops(list));
    }

    /// Joined players: the host's list.
    pub fn apply_drops(&mut self, list: Vec<(u32, Vec3, Id, u8)>) {
        let old = std::mem::take(&mut self.drops);
        for (id, pos, item, n) in list {
            if !valid_item(item) || n == 0 || !pos.is_finite() {
                continue;
            }
            match old.iter().find(|d| d.id == id) {
                Some(d) => {
                    let mut d = ItemDrop { item, n, net_pos: pos, ..ItemDrop::new(id, item, n, d.body.pos, Vec3::ZERO, 0.0) };
                    d.asked = old.iter().find(|o| o.id == id).map(|o| o.asked).unwrap_or(0.0);
                    self.drops.push(d);
                }
                None => self.drops.push(ItemDrop::new(id, item, n, pos, Vec3::ZERO, 0.0)),
            }
        }
    }

    /// Joined players: glide toward the host's positions, and ask for anything we walk into.
    pub fn client_drops(&mut self, dt: f32) {
        let me = self.player.body.pos + Vec3::Y * 0.9;
        let alive = self.dead.is_none();
        let mut asks = Vec::new();
        for d in self.drops.iter_mut() {
            let k = (dt * 12.0).min(1.0);
            d.body.pos += (d.net_pos - d.body.pos) * k;
            d.asked = (d.asked - dt).max(0.0);
            if alive && d.asked <= 0.0 && d.body.pos.distance(me) <= PICKUP_RANGE {
                let room = self.inv.room_for(d.item).min(64) as u8;
                if room > 0 {
                    d.asked = 0.5;
                    asks.push(Msg::Pickup { id: d.id, room });
                }
            }
        }
        for m in asks {
            self.net_send_msg(m);
        }
    }

    pub fn draw_drops(&self, g: &mut DynGeo, eye: Vec3, range: f32) {
        for d in self.drops.iter().filter(|d| d.body.pos.distance(eye) < range) {
            d.draw(g, &self.world, self.clock);
        }
    }
}

/// Matching items close together become one stack.
fn merge(drops: &mut Vec<ItemDrop>) {
    let mut i = 0;
    while i < drops.len() {
        let mut j = i + 1;
        while j < drops.len() {
            let (a, b) = (&drops[i], &drops[j]);
            let max = max_stack(a.item);
            if a.item == b.item && a.body.pos.distance(b.body.pos) < 1.0 && a.n as u16 + b.n as u16 <= max as u16 {
                let (n, delay, age) = (b.n, b.delay.max(a.delay), b.age.min(a.age));
                let a = &mut drops[i];
                a.n += n;
                a.delay = delay;
                a.age = age;
                drops.remove(j);
            } else {
                j += 1;
            }
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_save_and_merge() {
        let mut v = vec![ItemDrop::new(1, DIRT, 3, Vec3::new(1.0, 60.0, -2.5), Vec3::ZERO, 0.0), ItemDrop::new(2, DIAMOND, 1, Vec3::new(9.0, 61.0, 4.0), Vec3::ZERO, 0.0)];
        v[1].age = 12.5;
        v[1].wear = 9;
        let back = decode(&encode(&v), 4);
        assert_eq!(back, vec![(Vec3::new(1.0, 60.0, -2.5), DIRT, 3, 0.0, 0), (Vec3::new(9.0, 61.0, 4.0), DIAMOND, 1, 12.5, 9)]);
        assert!(decode(&[1, 2, 3], 4).is_empty());
        // Neighbouring dirt merges; the far diamond and a full stack don't.
        v.push(ItemDrop::new(3, DIRT, 5, Vec3::new(1.5, 60.0, -2.5), Vec3::ZERO, 0.0));
        v.push(ItemDrop::new(4, DIRT, 64, Vec3::new(1.2, 60.0, -2.5), Vec3::ZERO, 0.0));
        merge(&mut v);
        assert_eq!(v.len(), 3);
        assert_eq!(v[0].n, 8);
    }
}
