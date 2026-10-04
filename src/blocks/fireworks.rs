//! Fireworks: the glider's Rockets do more than push.
//!
//! - Use a Rocket on the ground (not gliding) and it shoots up and bursts in
//!   a random colour.
//! - Load a Crossbow with them: with Rockets in your inventory a Crossbow
//!   fires those (they burst on whatever they hit and hurt everything
//!   nearby); otherwise it fires Pointy Sticks, harder than a bow.
//! - Dispensers launch Rockets straight out of their face.
//!
//! Rockets fly where the world lives; the bursts are sent to everyone as
//! `Msg::Firework` so they can see the colours.

use crate::block::*;
use crate::entity::{Arrow, Particle};
use crate::game::Game;
use crate::net::Msg;
use crate::sound::Sfx;
use crate::texture::T_SPARK_FIRST;
use macroquad::math::Vec3;

/// Spark colours (see the T_SPARK tiles).
pub const COLOURS: u8 = 8;
/// A crossbow-fired rocket's burst: how far it reaches and how much it hurts at the middle.
pub const BLAST_RADIUS: f32 = 2.5;
pub const BLAST_DAMAGE: f32 = 6.0;

impl Game {
    /// Set a rocket flying: straight up from the ground, or out of a crossbow
    /// (`armed`: bursts on what it hits, and hurts). Host side.
    pub fn launch_firework(&mut self, from: Vec3, vel: Vec3, colour: u8, shooter: Option<u32>, armed: bool) {
        let mut a = Arrow::new(from, vel, shooter, if armed { BLAST_DAMAGE } else { 0.0 });
        a.firework = colour % COLOURS + 1;
        a.life = if armed { 3.0 } else { self.rng.range(1.0, 1.4) };
        a.appearance = ProjectileAppearance { model: ProjectileModel::Billboard, tile: Some(T_SPARK_FIRST + (colour % COLOURS) as u16), scale: 0.5 };
        self.arrows.push(a);
        self.sfx(Sfx::Twang, Some(from));
    }

    /// A rocket from the ground in front of us (the host launches joined players').
    pub fn set_off_firework(&mut self) {
        self.player.swing = 1.0;
        self.advance("light_show");
        if self.is_client() {
            // (The host heard about it with the rest of our item use.)
            if !self.creative {
                self.use_up_held();
            }
            return;
        }
        let dir = self.player.look_dir();
        let from = self.player.body.pos + Vec3::new(dir.x, 0.0, dir.z).normalize_or_zero() * 1.2 + Vec3::Y * 0.3;
        let colour = self.rng.int(0, COLOURS as i32 - 1) as u8;
        let me = self.my_id;
        let up = Vec3::new(self.rng.range(-1.0, 1.0), 16.0, self.rng.range(-1.0, 1.0));
        self.launch_firework(from, up, colour, Some(me), false);
        if !self.creative {
            self.use_up_held();
        }
    }

    /// A joined player used a Rocket: off the ground, or a boost if they're gliding.
    pub fn host_rocket(&mut self, from: u32) {
        let Some(p) = self.peers.get(&from) else { return };
        if p.flags & crate::net::FLAG_GLIDE != 0 {
            return;
        }
        let dir = Vec3::new(p.yaw.sin(), 0.0, -p.yaw.cos());
        let at = p.target + dir * 1.2 + Vec3::Y * 0.3;
        if self.peer_take(from, ROCKET, 1) {
            let colour = self.rng.int(0, COLOURS as i32 - 1) as u8;
            let up = Vec3::new(self.rng.range(-1.0, 1.0), 16.0, self.rng.range(-1.0, 1.0));
            self.launch_firework(at, up, colour, Some(from), false);
        }
    }

    /// Fire the held Crossbow: Rockets first, else Pointy Sticks.
    pub fn shoot_crossbow(&mut self) {
        let rocket = self.inv.count(ROCKET) > 0;
        if !self.creative && !rocket && self.inv.count(ARROW) == 0 {
            self.msg("Nothing to load. A Crossbow takes Rockets or Pointy Sticks.");
            return;
        }
        self.use_cd = 1.1;
        self.player.swing = 1.0;
        if self.is_client() {
            // The host fires it for us (see `host_crossbow`).
            if !self.creative {
                self.inv.remove(if rocket { ROCKET } else { ARROW }, 1);
            }
            self.sfx(Sfx::Twang, None);
            return;
        }
        if !self.creative {
            self.inv.remove(if rocket { ROCKET } else { ARROW }, 1);
        }
        self.use_tool(1);
        let dir = self.player.look_dir();
        let me = self.my_id;
        self.crossbow_from(self.player.eye() + dir * 0.5, dir, me, rocket);
    }

    /// A joined player's crossbow (they've told us they used it).
    pub fn host_crossbow(&mut self, from: u32) {
        let Some(p) = self.peers.get(&from) else { return };
        let dir = Vec3::new(p.yaw.sin() * p.pitch.cos(), p.pitch.sin(), -p.yaw.cos() * p.pitch.cos());
        let eye = p.target + Vec3::Y * crate::player::EYE;
        let rocket = self.peer_take(from, ROCKET, 1);
        if rocket || self.peer_take(from, ARROW, 1) {
            self.crossbow_from(eye + dir * 0.5, dir, from, rocket);
            self.host_wear(from, CROSSBOW, 1);
        }
    }

    fn crossbow_from(&mut self, from: Vec3, dir: Vec3, shooter: u32, rocket: bool) {
        if rocket {
            let colour = self.rng.int(0, COLOURS as i32 - 1) as u8;
            self.launch_firework(from, dir * 22.0, colour, Some(shooter), true);
        } else {
            let mut a = Arrow::new(from, dir * Arrow::SPEED * 1.4, Some(shooter), 7.0);
            a.life = 8.0;
            self.arrows.push(a);
            self.sfx(Sfx::Twang, Some(from));
        }
    }

    /// A rocket bursts (host side): sparks for everyone, and a crossbow's
    /// rocket hurts whatever's near (not whoever fired it).
    pub fn firework_burst(&mut self, at: Vec3, colour: u8, damage: f32, shooter: Option<u32>) {
        self.sfx(Sfx::Firework, Some(at));
        self.firework_sparks(at, colour);
        self.net_broadcast(Msg::Firework { at, colour });
        if damage <= 0.0 {
            return;
        }
        let share = |d: f32| (1.0 - d / BLAST_RADIUS).max(0.0);
        let me = self.player.body.pos + Vec3::Y * 0.9;
        if !self.dedicated && !self.spectator && self.dead.is_none() && shooter != Some(self.my_id) && me.distance(at) < BLAST_RADIUS {
            self.player.hurt = 0.0;
            self.hurt_player_from(damage * share(me.distance(at)), "went out with a bang", Some(at), true);
        }
        let hit: Vec<(u32, f32, Vec3)> = self.peers.iter().filter(|(id, p)| p.alive() && Some(**id) != shooter).map(|(&id, p)| (id, (p.target + Vec3::Y * 0.9).distance(at), p.target)).filter(|h| h.1 < BLAST_RADIUS).collect();
        for (id, d, pos) in hit {
            self.hurt_peer(id, damage * share(d), "went out with a bang", (pos - at).normalize_or_zero() * 4.0);
        }
        for m in self.mobs.iter_mut() {
            let d = (m.body.pos + Vec3::Y * m.body.height * 0.5).distance(at);
            if d < BLAST_RADIUS + m.body.half {
                m.hurt = 0.0;
                m.damage(damage * share(d).max(0.3), at);
                if let Some(who) = shooter {
                    m.last_attacker = who;
                }
            }
        }
    }

    /// The burst itself: a sphere of coloured sparks that drift down.
    pub fn firework_sparks(&mut self, at: Vec3, colour: u8) {
        if self.dedicated {
            return;
        }
        let tile = T_SPARK_FIRST + (colour % COLOURS) as u16;
        let second = T_SPARK_FIRST + ((colour + 3) % COLOURS) as u16;
        for k in 0..90 {
            let v = Vec3::new(self.rng.range(-1.0, 1.0), self.rng.range(-1.0, 1.0), self.rng.range(-1.0, 1.0)).normalize_or(Vec3::Y) * self.rng.range(5.0, 8.0);
            let t = if k % 4 == 0 { second } else { tile };
            self.particles.push(Particle { pos: at, vel: v, life: self.rng.range(1.2, 1.8), tile: t, uv: [0.25, 0.25], size: 0.28, gravity: 2.0 });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::tests::arena;
    use crate::entity::MobKind;

    #[test]
    fn rockets_from_the_ground_go_up_and_burst() {
        let mut g = arena(41);
        g.inv.slots[g.inv.selected] = Some((ROCKET, 2));
        g.player.gliding = false;
        g.set_off_firework();
        assert_eq!(g.arrows.len(), 1);
        assert!(g.arrows[0].firework > 0);
        let start = g.arrows[0].pos.y;
        let mut peak = start;
        for _ in 0..40 {
            if let Some(a) = g.arrows.first() {
                peak = peak.max(a.pos.y);
            }
            g.particles.clear();
            g.update_arrows(0.05);
        }
        assert!(g.arrows.is_empty(), "burst");
        assert!(peak > start + 10.0, "went up a long way: {start} -> {peak}");
        assert_eq!(g.inv.count(ROCKET), 1);
    }

    #[test]
    fn crossbow_rockets_hurt_what_they_hit() {
        let mut g = arena(42);
        let eye = g.player.eye();
        let dir = g.player.look_dir();
        let target = eye + dir * 6.0 - Vec3::Y * 0.9;
        let id = g.alloc_mob(MobKind::Groaner, target);
        let hp = g.mobs.iter().find(|m| m.id == id).unwrap().health;
        g.inv.slots[g.inv.selected] = Some((CROSSBOW, 1));
        g.inv.slots[(g.inv.selected + 1) % 9] = Some((ROCKET, 1));
        let player_hp = g.player.health;
        g.shoot_crossbow();
        assert_eq!(g.inv.count(ROCKET), 0, "the rocket is loaded first");
        for _ in 0..20 {
            g.update_arrows(0.05);
        }
        let m = g.mobs.iter().find(|m| m.id == id).unwrap();
        assert!(m.health < hp, "it went off on the Groaner");
        assert_eq!(g.player.health, player_hp, "not on whoever fired it");
    }
}
