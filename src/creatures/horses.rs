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
/// A `Msg::MountMob` id carries the seat in its top two bits: 0 the driver,
/// 1 a Camel's back seat (or a Floaty's second), 2 and 3 a Floaty's others.
pub const SEAT_SHIFT: u32 = 30;
pub const SEAT_MASK: u32 = 3 << SEAT_SHIFT;
/// A Rotsteed: not quite as quick as a living Galloper.
pub const ROTSTEED_SPEED: f32 = 8.0;
/// A Camel: slower than a Galloper, but it dashes (Space) every few seconds.
pub const CAMEL_SPEED: f32 = 6.5;
pub const CAMEL_DASH: f32 = 14.0;
pub const DASH_REST: f32 = 2.75;

/// Where seat `n` is on `kind`: (to the right, up, ahead).
pub fn seat(kind: MobKind, n: u8) -> Vec3 {
    match (kind, n) {
        (MobKind::Camel, 0) => Vec3::new(0.0, 1.85, 0.35),
        (MobKind::Camel, _) => Vec3::new(0.0, 1.85, -0.5),
        // Four on a Floaty's harness, the driver front left.
        (MobKind::Floaty, n) => Vec3::new(if n % 2 == 0 { -0.8 } else { 0.8 }, 3.95, if n < 2 { 0.8 } else { -0.8 }),
        _ => Vec3::new(0.0, 1.15, 0.0),
    }
}

/// Where seat `n` of a mob at `pos` facing `yaw` is, in the world.
pub fn seat_at(kind: MobKind, n: u8, pos: Vec3, yaw: f32) -> Vec3 {
    let s = seat(kind, n);
    let (fwd, right) = (Vec3::new(yaw.sin(), 0.0, -yaw.cos()), Vec3::new(yaw.cos(), 0.0, yaw.sin()));
    pos + right * s.x + Vec3::Y * s.y + fwd * s.z
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
                self.tell(who, &format!("The {} is yours. It could use a Saddle.", self.mobs[i].kind.name()));
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
            return Interaction::Mounted(m.id | 1 << SEAT_SHIFT);
        }
        Interaction::Nothing
    }

    /// How fast the mount we're driving is going (0 on foot or in a back seat).
    pub fn mount_speed(&self) -> f32 {
        let Some(id) = self.mounted.filter(|_| self.seat_no == 0) else { return 0.0 };
        self.mobs.iter().find(|m| m.id == id).map(|m| Vec3::new(m.body.vel.x, 0.0, m.body.vel.z).length()).unwrap_or(0.0)
    }

    /// Get on the Galloper `id` (the local player; the host has said yes).
    pub fn mount_mob(&mut self, id: u32) {
        self.seat_no = (id >> SEAT_SHIFT) as u8;
        self.mounted = Some(id & !SEAT_MASK);
        self.sfx(Sfx::Place(crate::sound::Mat::Wood), None);
    }

    /// Get off, beside it.
    pub fn dismount_mob(&mut self) {
        let Some(id) = self.mounted.take() else { return };
        let client = self.is_client();
        let seat = std::mem::take(&mut self.seat_no);
        let Some(m) = self.mobs.iter_mut().find(|m| m.id == id) else { return };
        match seat {
            0 => m.rider = 0,
            1 => m.passenger = 0,
            n => m.crew[(n as usize - 2).min(1)] = 0,
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
        if self.seat_no != 0 {
            // Along for the ride: sit in our seat wherever it goes.
            let m = &self.mobs[i];
            let at = seat_at(m.kind, self.seat_no, m.body.pos, m.yaw);
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
        let floaty = self.mobs[i].kind == MobKind::Floaty;
        let speed = if camel {
            CAMEL_SPEED
        } else if floaty {
            crate::floaty::FLY_SPEED
        } else if self.mobs[i].kind == MobKind::Rotsteed {
            ROTSTEED_SPEED
        } else if !strutter {
            RIDE_SPEED
        } else if self.inv.held() == SHROOM_STICK {
            5.5
        } else {
            0.0
        };
        let wish = (fwd * forward + right * strafe * 0.6).clamp_length_max(1.0) * speed;
        let pitch = self.player.pitch;
        let m = &mut self.mobs[i];
        let k = (dt * 4.0).min(1.0);
        m.body.vel.x += (wish.x - m.body.vel.x) * k;
        m.body.vel.z += (wish.z - m.body.vel.z) * k;
        if floaty {
            // No gravity: up with Jump, and along the way we look.
            let climb = if jump { crate::floaty::CLIMB } else { 0.0 } + pitch.sin() * forward * speed * 0.8;
            m.body.vel.y += (climb.clamp(-speed, speed) - m.body.vel.y) * k;
        } else {
            m.body.vel.y = (m.body.vel.y - crate::entity::GRAVITY * dt).max(-40.0);
        }
        if floaty {
        } else if camel {
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
        let seat = seat_at(m.kind, 0, m.body.pos, m.yaw);
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
        if m.passenger == from + 1 || m.crew.contains(&(from + 1)) {
            // The other seats only get off.
            if off {
                if m.passenger == from + 1 {
                    m.passenger = 0;
                }
                for c in m.crew.iter_mut().filter(|c| **c == from + 1) {
                    *c = 0;
                }
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
        let at = seat_at(m.kind, 0, pos, yaw);
        if let Some(p) = self.peers.get_mut(&from) {
            p.target = at;
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
            for k in 0..2 {
                if m.crew[k] != 0 && gone(m.crew[k], m.id) {
                    m.crew[k] = 0;
                }
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
    fn rotsteeds_tame_and_a_spear_from_the_saddle_hits_harder() {
        let mut g = crate::game::tests::arena(94);
        let mut m = Mob::new(MobKind::Rotsteed, Vec3::new(3.5, 50.0, 0.5), &mut g.rng);
        m.id = 779;
        g.mobs.push(m);
        let me = crate::players::record_key(&g.player_name);
        let at = g.player.body.pos;
        for _ in 0..20 {
            g.interact_mob(&me, at, 779, APPLE);
        }
        assert_eq!(g.mobs[0].owner.as_deref(), Some(me.as_str()), "tamed like a Galloper");
        assert_eq!(g.interact_mob(&me, at, 779, SADDLE), Interaction::Ate);
        assert_eq!(g.interact_mob(&me, at, 779, AIR), Interaction::Mounted(779));
        g.mount_mob(779);
        g.player.yaw = 0.0;
        for _ in 0..90 {
            g.ride_tick(1.0 / 60.0, 1.0, 0.0, false, false);
        }
        let speed = g.mount_speed();
        assert!(speed > 6.0 && speed <= ROTSTEED_SPEED + 0.1, "a gallop: {speed}");
        assert!(crate::combat::lunge_bonus(speed) > 6.0);
        assert!(crate::combat::lunge_bonus(100.0) <= crate::combat::LUNGE_MAX);
        // Spears are spears.
        assert!(is_spear(SPEAR) && is_spear(SPEAR_FIRST + 3) && !is_spear(STICK));
        assert_eq!(crate::anvil::repair_material(SPEAR_FIRST + 2), Some(IRON));
    }

    #[test]
    fn a_thrown_spear_lands_as_itself() {
        let mut g = crate::game::tests::arena(95);
        g.throw_spear_from(Vec3::new(0.5, 52.0, 0.5), Vec3::new(0.0, -0.3, -1.0).normalize(), 0, SPEAR_FIRST + 1, 0);
        for _ in 0..200 {
            g.update_entities(0.05);
        }
        assert!(g.drops.iter().any(|d| d.item == SPEAR_FIRST + 1), "the stone spear is back on the ground");
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
        assert_eq!(g.interact_mob(&me, at, 778, AIR), Interaction::Mounted(778 | 1 << SEAT_SHIFT));
        g.mount_mob(778 | 1 << SEAT_SHIFT);
        assert!(g.seat_no == 1 && g.mounted == Some(778));
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
