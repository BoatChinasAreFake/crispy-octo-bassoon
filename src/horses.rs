//! Gallopers (legally distinct horses): tame one, saddle it, ride it.
//!
//! - Gallopers wander the plains. Right-click one with an empty hand to try
//!   your luck: it bucks you off, but it gets a little more used to you each
//!   time, and Apples or wheat help a lot. Eventually it's yours (hearts).
//! - Put a **Saddle** (three wool, an iron chunk and two string) on your
//!   Galloper, then right-click to ride. It goes where you look: W to go,
//!   Space to jump (high), sneak to get off. Much faster than walking, and it
//!   jumps fences.
//! - Tamed Gallopers breed on Apples, stay put when nobody's around, and are
//!   saved with the world (saddles too).
//!
//! Where the world lives the Galloper's owner is remembered. A joined player
//! riding one drives it on their own screen and tells the host where it is
//! (`Msg::RideMob`); the host keeps everyone else in the picture.

use crate::animals::Interaction;
use crate::block::*;
use crate::entity::{MobKind, move_body};
use crate::game::Game;
use crate::net::Msg;
use crate::sound::Sfx;
use macroquad::math::Vec3;

/// How used to you a Galloper must get before it's yours.
pub const TAME_AT: u8 = 10;
/// Riding: top speed (blocks a second), and jump strength.
pub const RIDE_SPEED: f32 = 9.5;
pub const RIDE_JUMP: f32 = 11.0;
/// How often a joined rider tells the host where they are.
const RIDE_SYNC: f32 = 0.08;
/// Set on a `Msg::MountMob` id: you're in a Camel's back seat.
pub const PASSENGER_SEAT: u32 = 1 << 31;
/// A Camel: slower than a Galloper, but it dashes (Space) every few seconds.
pub const CAMEL_SPEED: f32 = 6.5;
pub const CAMEL_DASH: f32 = 14.0;
pub const DASH_REST: f32 = 2.75;

/// How high a rider sits on `kind`, and how far forward (back seat: behind).
pub fn seat(kind: MobKind, passenger: bool) -> (f32, f32) {
    match kind {
        MobKind::Camel if passenger => (1.85, -0.5),
        MobKind::Camel => (1.85, 0.35),
        _ => (1.15, 0.0),
    }
}

impl Game {
    /// Right-click on a Galloper by `who` holding `item` (where the world lives).
    pub fn galloper_interact(&mut self, who: &str, i: usize, item: Id, rider: u32) -> Interaction {
        let pos = self.mobs[i].body.pos + Vec3::Y * 1.6;
        let mine = self.mobs[i].owner.as_deref() == Some(who);
        let m = &mut self.mobs[i];
        if m.owner.is_none() {
            // Getting to know each other.
            let gain = match item {
                APPLE => 4,
                WHEAT => 2,
                AIR => 1,
                _ => return Interaction::Nothing,
            };
            m.temper = m.temper.saturating_add(gain);
            m.persistent = true;
            if m.temper >= TAME_AT && self.rng.chance(0.5) {
                m.owner = Some(who.to_string());
                self.hearts(pos, 7);
                self.tell(who, "The Galloper is yours. It could use a Saddle.");
                self.advance_for(who, "giddy_up");
            } else if item == AIR {
                self.smoke(pos, 5, 0.2);
                self.tell(who, "It bucks you off. It's getting used to you, though.");
            } else {
                self.hearts(pos, 2);
            }
            return if item == AIR { Interaction::Toggled } else { Interaction::Ate };
        }
        if !mine {
            return Interaction::Nothing;
        }
        let m = &mut self.mobs[i];
        if item == SADDLE && !m.saddled {
            m.saddled = true;
            self.sfx(Sfx::Place(crate::sound::Mat::Wood), Some(pos));
            return Interaction::Ate;
        }
        if m.kind.breed_food().contains(&item) && m.ready_to_breed() {
            m.love = crate::animals::LOVE_SECS;
            self.hearts(pos, 4);
            return Interaction::Ate;
        }
        if m.saddled && m.rider == 0 && m.baby <= 0.0 {
            m.rider = rider;
            return Interaction::Mounted(m.id);
        }
        Interaction::Nothing
    }

    /// Saddle, feed or climb onto a Camel (no taming needed; two seats).
    pub fn camel_interact(&mut self, i: usize, item: Id, rider: u32) -> Interaction {
        let pos = self.mobs[i].body.pos + Vec3::Y * 2.0;
        let m = &mut self.mobs[i];
        if item == SADDLE && !m.saddled && m.baby <= 0.0 {
            m.saddled = true;
            m.persistent = true;
            self.sfx(Sfx::Place(crate::sound::Mat::Wood), Some(pos));
            return Interaction::Ate;
        }
        if m.kind.breed_food().contains(&item) && m.ready_to_breed() {
            m.love = crate::animals::LOVE_SECS;
            m.persistent = true;
            self.hearts(pos, 4);
            return Interaction::Ate;
        }
        if !m.saddled || m.baby > 0.0 || rider == 0 || m.rider == rider || m.passenger == rider {
            return Interaction::Nothing;
        }
        if m.rider == 0 {
            m.rider = rider;
            return Interaction::Mounted(m.id);
        }
        if m.passenger == 0 {
            m.passenger = rider;
            return Interaction::Mounted(m.id | PASSENGER_SEAT);
        }
        Interaction::Nothing
    }

    /// Get on the Galloper `id` (the local player; the host has said yes).
    pub fn mount_mob(&mut self, id: u32) {
        self.passenger_seat = id & PASSENGER_SEAT != 0;
        self.mounted = Some(id & !PASSENGER_SEAT);
        self.sfx(Sfx::Place(crate::sound::Mat::Wood), None);
    }

    /// Get off, beside it.
    pub fn dismount_mob(&mut self) {
        let Some(id) = self.mounted.take() else { return };
        let client = self.is_client();
        let back = std::mem::take(&mut self.passenger_seat);
        let Some(m) = self.mobs.iter_mut().find(|m| m.id == id) else { return };
        if back {
            m.passenger = 0;
        } else {
            m.rider = 0;
        }
        let side = Vec3::new(m.yaw.cos(), 0.0, m.yaw.sin());
        let (pos, yaw) = (m.body.pos, m.yaw);
        self.player.body.pos = pos + side * 1.3 + Vec3::Y * 0.2;
        self.player.body.vel = Vec3::ZERO;
        self.player.fall_start = self.player.body.pos.y;
        if client {
            self.net_send_msg(Msg::RideMob { mob: id, pos, yaw, off: true });
        }
    }

    /// Riding: our controls move the Galloper, and we sit on it.
    pub fn ride_tick(&mut self, dt: f32, forward: f32, strafe: f32, jump: bool, sneak: bool) {
        let Some(id) = self.mounted else { return };
        if sneak || self.dead.is_some() {
            self.dismount_mob();
            return;
        }
        let Some(i) = self.mobs.iter().position(|m| m.id == id) else {
            self.mounted = None;
            return;
        };
        if self.passenger_seat {
            // Along for the ride: sit in the back seat wherever it goes.
            let m = &self.mobs[i];
            let (up, ahead) = seat(m.kind, true);
            let at = m.body.pos + Vec3::new(m.yaw.sin(), 0.0, -m.yaw.cos()) * ahead + Vec3::Y * up;
            self.player.body.pos = at;
            self.player.body.vel = Vec3::ZERO;
            self.player.fall_start = at.y;
            return;
        }
        let yaw = self.player.yaw;
        let fwd = Vec3::new(yaw.sin(), 0.0, -yaw.cos());
        let right = Vec3::new(yaw.cos(), 0.0, yaw.sin());
        // A Strutter only goes where its shroom on a stick says (see fortress.rs), and not as fast.
        let strutter = self.mobs[i].kind == crate::entity::MobKind::Strutter;
        let camel = self.mobs[i].kind == MobKind::Camel;
        let speed = if camel {
            CAMEL_SPEED
        } else if !strutter {
            RIDE_SPEED
        } else if self.inv.held() == SHROOM_STICK {
            5.5
        } else {
            0.0
        };
        let wish = (fwd * forward + right * strafe * 0.6).clamp_length_max(1.0) * speed;
        let m = &mut self.mobs[i];
        let k = (dt * 4.0).min(1.0);
        m.body.vel.x += (wish.x - m.body.vel.x) * k;
        m.body.vel.z += (wish.z - m.body.vel.z) * k;
        m.body.vel.y = (m.body.vel.y - crate::entity::GRAVITY * dt).max(-40.0);
        if camel {
            // No big jumps: a dash forward instead, then a rest.
            m.warp_cd = (m.warp_cd - dt).max(0.0);
            if jump && m.body.on_ground && m.warp_cd <= 0.0 {
                m.body.vel += fwd * CAMEL_DASH + Vec3::Y * 5.0;
                m.warp_cd = DASH_REST;
            }
        } else if jump && m.body.on_ground {
            m.body.vel.y = RIDE_JUMP;
        }
        move_body(&self.world, &mut m.body, dt, false);
        if strutter && m.body.in_lava {
            // Lava's a floor to it.
            m.body.vel.y = m.body.vel.y.max(3.5);
        }
        if speed > 0.0 {
            m.yaw = yaw;
        }
        m.net_pos = m.body.pos;
        m.anim += Vec3::new(m.body.vel.x, 0.0, m.body.vel.z).length() * dt * 2.5;
        let (up, ahead) = seat(m.kind, false);
        let seat = m.body.pos + Vec3::new(m.yaw.sin(), 0.0, -m.yaw.cos()) * ahead + Vec3::Y * up;
        let (pos, myaw) = (m.body.pos, m.yaw);
        self.player.body.pos = seat;
        self.player.body.vel = Vec3::ZERO;
        self.player.fall_start = seat.y;
        if self.is_client() {
            self.ride_sync += dt;
            if self.ride_sync >= RIDE_SYNC {
                self.ride_sync = 0.0;
                self.net_send_msg(Msg::RideMob { mob: id, pos, yaw: myaw, off: false });
            }
        }
    }

    /// A joined player riding a Galloper says where it is (or that they got off).
    pub fn host_ride_mob(&mut self, from: u32, mob: u32, pos: Vec3, yaw: f32, off: bool) {
        let Some(m) = self.mobs.iter_mut().find(|m| m.id == mob) else { return };
        if m.passenger == from + 1 {
            // The back seat only gets off.
            if off {
                m.passenger = 0;
            }
            return;
        }
        if m.rider != from + 1 {
            return;
        }
        if off {
            m.rider = 0;
        }
        // Only believable moves.
        if pos.is_finite() && yaw.is_finite() && pos.distance(m.body.pos) < 6.0 {
            m.body.pos = pos;
            m.body.vel = Vec3::ZERO;
            m.yaw = yaw;
        }
        let up = seat(m.kind, false).0;
        if let Some(p) = self.peers.get_mut(&from) {
            p.target = pos + Vec3::Y * up;
        }
    }

    /// Riders who've left free their Gallopers.
    pub fn free_riderless(&mut self) {
        let me = if self.dedicated { 0 } else { self.my_id + 1 };
        let local = self.mounted;
        let gone = |who: u32, id: u32| if who == me { local != Some(id) } else { !self.peers.contains_key(&(who - 1)) };
        for m in self.mobs.iter_mut() {
            if m.rider != 0 && gone(m.rider, m.id) {
                m.rider = 0;
            }
            if m.passenger != 0 && gone(m.passenger, m.id) {
                m.passenger = 0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{Mob, MobKind};

    #[test]
    fn tame_saddle_ride() {
        let mut g = crate::game::tests::arena(91);
        let mut m = Mob::new(MobKind::Galloper, Vec3::new(3.5, 50.0, 0.5), &mut g.rng);
        m.id = 777;
        g.mobs.push(m);
        let me = crate::players::record_key(&g.player_name);
        let at = g.player.body.pos;
        // Apples until it's ours.
        for _ in 0..20 {
            g.interact_mob(&me, at, 777, APPLE);
        }
        assert_eq!(g.mobs[0].owner.as_deref(), Some(me.as_str()));
        assert_eq!(g.interact_mob(&me, at, 777, AIR), Interaction::Nothing, "no saddle yet");
        assert_eq!(g.interact_mob(&me, at, 777, SADDLE), Interaction::Ate);
        assert_eq!(g.interact_mob(&me, at, 777, AIR), Interaction::Mounted(777));
        g.mount_mob(777);
        g.player.yaw = 0.0;
        let start = g.mobs[0].body.pos;
        for _ in 0..60 {
            g.ride_tick(1.0 / 60.0, 1.0, 0.0, false, false);
        }
        let moved = g.mobs[0].body.pos - start;
        assert!(moved.z < -4.0, "went {moved:?}");
        assert!((g.player.body.pos - g.mobs[0].body.pos).y > 1.0);
        g.ride_tick(1.0 / 60.0, 0.0, 0.0, false, true);
        assert!(g.mounted.is_none() && g.mobs[0].rider == 0);
        // Saddle and owner are saved.
        let back = crate::animals::decode_mobs(&crate::animals::encode_mobs(&g.mobs, &g.mob_names), &mut g.rng);
        assert!(back[0].saddled && back[0].owner.is_some());
    }

    #[test]
    fn camels_carry_two_and_dash() {
        let mut g = crate::game::tests::arena(93);
        let mut m = Mob::new(MobKind::Camel, Vec3::new(3.5, 50.0, 0.5), &mut g.rng);
        m.id = 778;
        g.mobs.push(m);
        let me = crate::players::record_key(&g.player_name);
        let at = g.player.body.pos;
        // No taming: just a saddle.
        assert_eq!(g.interact_mob(&me, at, 778, SADDLE), Interaction::Ate);
        let i = 0;
        // Someone else is driving: we get the back seat.
        g.mobs[i].rider = 99;
        assert_eq!(g.interact_mob(&me, at, 778, AIR), Interaction::Mounted(778 | PASSENGER_SEAT));
        g.mount_mob(778 | PASSENGER_SEAT);
        assert!(g.passenger_seat && g.mounted == Some(778));
        g.mobs[i].body.pos = Vec3::new(3.5, 50.0, 0.5);
        g.ride_tick(1.0 / 60.0, 1.0, 0.0, false, false);
        assert!(g.mobs[i].body.pos.distance(Vec3::new(3.5, 50.0, 0.5)) < 0.01, "the passenger doesn't steer");
        assert!(g.player.body.pos.y > g.mobs[i].body.pos.y + 1.5, "up high");
        g.ride_tick(1.0 / 60.0, 0.0, 0.0, false, true);
        assert!(g.mounted.is_none() && g.mobs[i].passenger == 0 && g.mobs[i].rider == 99, "the driver stays on");
        // Now we drive; a dash goes a long way, then needs a rest.
        g.mobs[i].rider = 0;
        assert_eq!(g.interact_mob(&me, at, 778, AIR), Interaction::Mounted(778));
        g.mount_mob(778);
        g.player.yaw = 0.0;
        for _ in 0..30 {
            g.ride_tick(1.0 / 60.0, 0.0, 0.0, false, false);
        }
        let start = g.mobs[i].body.pos;
        g.ride_tick(1.0 / 60.0, 1.0, 0.0, true, false);
        assert!(g.mobs[i].warp_cd > 2.0, "dashed");
        for _ in 0..20 {
            g.ride_tick(1.0 / 60.0, 1.0, 0.0, true, false);
        }
        assert!(start.z - g.mobs[i].body.pos.z > 2.5, "went {:?}", g.mobs[i].body.pos - start);
    }
}
