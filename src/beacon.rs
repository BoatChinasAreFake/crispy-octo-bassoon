//! Beacons: what the Wyrm Egg is for.
//!
//! A **Beacon** (the Wyrm Egg, five glass and three obsidian) needs the open
//! sky above it and an obsidian pyramid below: a 3×3 layer under it, then
//! 5×5 under that, then 7×7. With at least the first layer it shines a
//! beam into the sky and, every few seconds, gives everyone within reach
//! (16 blocks, 28 with two layers, 40 with all three) one effect: Speed,
//! Leaping, Night Vision, Fire Resistance, or Healing (a little at a time).
//! Right-click it to choose which. Beacons work where the world lives;
//! joined players get their effect from the host.

use crate::block::*;
use crate::game::Game;
use crate::net::Msg;
use crate::potions::{potion_item, Potion, ALL};
use crate::render::{DynGeo, Pass};
use crate::world::{World, CH};
use macroquad::math::{IVec3, Mat4, Vec3};

/// Seconds between pulses, and how long each one's effect lasts.
pub const PULSE: f32 = 4.0;
pub const LASTS: f32 = 10.0;
/// How far one reaches with 1, 2 and 3 layers.
pub const RANGE: [f32; 3] = [16.0, 28.0, 40.0];

pub fn is_beacon(id: Id) -> bool {
    (BEACON_FIRST..BEACON_FIRST + ALL.len() as Id).contains(&id)
}

pub fn effect_of(id: Id) -> Potion {
    ALL[((id - BEACON_FIRST) as usize).min(ALL.len() - 1)]
}

/// The beacon block giving the next effect along.
pub fn next_effect(id: Id) -> Id {
    BEACON_FIRST + ((id - BEACON_FIRST + 1) % ALL.len() as Id)
}

/// How many complete obsidian layers are under it (0: it doesn't work), if it can see the sky.
pub fn tier(world: &World, p: IVec3) -> usize {
    if (p.y + 1..CH).any(|y| is_opaque(world.get(p.x, y, p.z))) {
        return 0;
    }
    let mut layers = 0;
    for k in 1..=3 {
        let full = (-k..=k).all(|dx| (-k..=k).all(|dz| world.get(p.x + dx, p.y - k, p.z + dz) == OBSIDIAN));
        if !full {
            break;
        }
        layers = k as usize;
    }
    layers
}

impl Game {
    /// A beacon's effect: kept going quietly while you're near one.
    pub fn beacon_effect(&mut self, p: Potion) {
        if p.instant() {
            self.player.health = (self.player.health + 2.0).min(20.0);
            return;
        }
        self.timed_effect(p, LASTS);
    }

    /// Where the world lives: every working beacon pulses now and then.
    pub fn beacons_tick(&mut self, dt: f32) {
        if self.is_client() {
            return;
        }
        self.beacon_timer += dt;
        if self.beacon_timer < PULSE {
            return;
        }
        self.beacon_timer = 0.0;
        let beacons: Vec<IVec3> = self.world.beacons.iter().copied().filter(|p| self.world.is_loaded(p.x, p.z)).collect();
        for p in beacons {
            let t = tier(&self.world, p);
            if t == 0 {
                continue;
            }
            let effect = effect_of(self.world.get_v(p));
            let (at, reach) = (p.as_vec3() + Vec3::splat(0.5), RANGE[t - 1]);
            if !self.dedicated && self.dead.is_none() && self.player.body.pos.distance(at) < reach {
                self.beacon_effect(effect);
                self.advance("beaconator");
            }
            let near: Vec<u32> = self.peers.iter().filter(|(_, q)| q.alive() && q.target.distance(at) < reach).map(|(&id, _)| id).collect();
            for id in near {
                self.net_send_to(id, Msg::BeaconEffect { item: potion_item(effect, false) });
            }
        }
    }

    /// Working beacons' beams of light.
    pub fn draw_beacons(&self, g: &mut DynGeo, eye: Vec3, range: f32) {
        for &p in &self.world.beacons {
            let at = p.as_vec3() + Vec3::new(0.5, 1.0, 0.5);
            if Vec3::new(at.x - eye.x, 0.0, at.z - eye.z).length() > range || tier(&self.world, p) == 0 {
                continue;
            }
            let c = effect_of(self.world.get_v(p)).colour();
            let tint = [c[0] as f32 / 255.0 * 0.6 + 0.4, c[1] as f32 / 255.0 * 0.6 + 0.4, c[2] as f32 / 255.0 * 0.6 + 0.4, 0.55];
            g.begin(Pass::Blend, tint, true);
            let height = (CH + 64) as f32 - at.y;
            let spin = Mat4::from_rotation_y(self.clock * 0.6);
            let m = Mat4::from_translation(at) * spin * Mat4::from_translation(Vec3::new(-0.2, 0.0, -0.2)) * Mat4::from_scale(Vec3::new(0.4, height, 0.4));
            g.cube(&m, [crate::texture::T_BEACON_BEAM; 6], 1.0, [0.0, 0.0, 1.0, 1.0]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use macroquad::math::ivec3;

    #[test]
    fn beacons_need_a_pyramid_and_the_sky() {
        let mut g = crate::game::tests::arena(91);
        let p = ivec3(0, 53, 0);
        g.world.set_v(p, BEACON_FIRST + Potion::Speed.index() as Id);
        assert_eq!(tier(&g.world, p), 0, "nothing under it");
        for k in 1..=2 {
            for dx in -k..=k {
                for dz in -k..=k {
                    g.world.set_v(p + ivec3(dx, -k, dz), OBSIDIAN);
                }
            }
        }
        assert_eq!(tier(&g.world, p), 2);
        // It gives the player its effect.
        g.player.body.pos = Vec3::new(5.5, 50.0, 0.5);
        g.beacons_tick(PULSE + 0.1);
        assert!(g.has_effect(Potion::Speed));
        // Right-click picks the next effect.
        assert!(g.use_switch(p, g.world.get_v(p)));
        assert_eq!(effect_of(g.world.get_v(p)), ALL[(Potion::Speed.index() + 1) % ALL.len()]);
        // Roofed over, it stops.
        g.world.set_v(p + IVec3::Y * 3, STONE);
        assert_eq!(tier(&g.world, p), 0);
    }
}
