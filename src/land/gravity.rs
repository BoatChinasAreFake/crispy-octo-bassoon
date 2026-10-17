//! Gravity islands: small islands out among the Hollow's outer ones (worlds
//! from generator 9) where down is up.
//!
//! Each hangs flat side down, with a glowing **Gravity Core** set in the
//! middle of that flat face. Under it, for a few blocks, gravity points up:
//! go near and you fall *up* onto the underside, and can walk about on it,
//! upside down, with the void overhead. Jump off (or walk off the edge) and
//! ordinary gravity takes over again, which is worth thinking about first.
//!
//! The field comes from the generator, so everyone (host and joined players
//! alike) agrees where it is without being told; each player's own machine
//! runs its physics, and draws other players upside down there too.

use crate::block::*;
use crate::dims::Dim;
use crate::noise::hash2;
use crate::world::{Generator, CH, CW};
use macroquad::math::{ivec3, IVec3, Vec3};

/// Where to draw someone standing upside down at `pos` (their box's bottom), facing `yaw`.
pub fn flipped_root(pos: Vec3, yaw: f32) -> macroquad::math::Mat4 {
    use macroquad::math::Mat4;
    Mat4::from_translation(pos + Vec3::Y * 1.8) * Mat4::from_rotation_y(-yaw) * Mat4::from_rotation_z(std::f32::consts::PI)
}

/// At most one gravity island per cell this big.
const CELL: i32 = 120;
/// How far below the flat face the field reaches, and how far beyond its rim.
pub const FIELD_DEPTH: f32 = 10.0;
const FIELD_RIM: f32 = 3.0;

/// The gravity island in cell (gx, gz) (generator coordinates), if any: the
/// middle of its flat underside, and its radius.
pub fn gravity_island(seed: u32, gx: i32, gz: i32) -> Option<(IVec3, i32)> {
    let s = seed ^ 0x6A17;
    if hash2(s, gx, gz) > 0.35 {
        return None;
    }
    let o = crate::hollow::GEN_ORIGIN;
    let x = o.x + gx * CELL + 20 + (hash2(s ^ 1, gx, gz) * (CELL - 40) as f32) as i32;
    let z = o.z + gz * CELL + 20 + (hash2(s ^ 2, gx, gz) * (CELL - 40) as f32) as i32;
    let d = ((x - o.x) as f32).hypot((z - o.z) as f32);
    if !(140.0..700.0).contains(&d) {
        return None;
    }
    let r = 5 + (hash2(s ^ 3, gx, gz) * 4.0) as i32;
    // Clear of the outer islands.
    let crowded = crate::hollow::outer_islands_near(seed, x, z).iter().any(|(c, cr, _)| ((c.x - x) as f32).hypot((c.z - z) as f32) < (cr + r + 24) as f32);
    if crowded {
        return None;
    }
    Some((ivec3(x, o.y + 14 + (hash2(s ^ 4, gx, gz) * 12.0) as i32, z), r))
}

/// Gravity islands that might reach (x, z).
pub fn gravity_islands_near(seed: u32, x: i32, z: i32) -> Vec<(IVec3, i32)> {
    let o = crate::hollow::GEN_ORIGIN;
    let (gx, gz) = ((x - o.x).div_euclid(CELL), (z - o.z).div_euclid(CELL));
    (-1..=1).flat_map(|dz| (-1..=1).map(move |dx| (gx + dx, gz + dz))).filter_map(|(a, b)| gravity_island(seed, a, b)).collect()
}

/// An island's blocks: a mound rising from a flat face, with the core in that face.
pub fn island_blocks(c: IVec3, r: i32) -> Vec<(IVec3, Id)> {
    let mut v = Vec::new();
    for dz in -r..=r {
        for dx in -r..=r {
            let d = ((dx * dx + dz * dz) as f32).sqrt();
            if d > r as f32 + 0.3 {
                continue;
            }
            let height = ((1.0 - d / (r as f32 + 0.5)).max(0.0).sqrt() * r as f32 * 0.9) as i32 + 1;
            for dy in 0..height {
                v.push((c + ivec3(dx, dy, dz), HOLLOW_STONE));
            }
        }
    }
    v.push((c, GRAVITY_CORE));
    v
}

impl crate::game::Game {
    /// Which way down is for the local player (where they are, every frame).
    pub fn gravity_tick(&mut self) {
        let flip = self.world.generator.gravity_flipped(self.player.body.pos + Vec3::Y * self.player.body.height * 0.5);
        if flip != self.player.upside_down {
            self.player.upside_down = flip;
            self.player.fall_start = self.player.body.pos.y;
        }
        if flip && self.player.body.on_ground {
            self.advance("down_is_up");
        }
    }
}

impl Generator {
    /// Does gravity point up at `p` (in this dimension's own coordinates)?
    pub fn gravity_flipped(&self, p: Vec3) -> bool {
        if self.opts.version < 9 || self.dim != Dim::Hollow {
            return false;
        }
        let (x, z) = (p.x.floor() as i32 + Dim::Hollow.gen_x(), p.z.floor() as i32);
        gravity_islands_near(self.seed, x, z).into_iter().any(|(c, r)| {
            let flat = ((x - c.x) as f32 + 0.5).hypot((z - c.z) as f32 + 0.5);
            flat <= r as f32 + FIELD_RIM && p.y < c.y as f32 && p.y > c.y as f32 - FIELD_DEPTH
        })
    }

    /// Write the gravity islands that reach chunk (cx, cz) (generator coordinates).
    pub fn place_gravity_islands(&self, cx: i32, cz: i32, b: &mut [Id]) {
        let mid = (cx * CW + CW / 2, cz * CW + CW / 2);
        for (c, r) in gravity_islands_near(self.seed, mid.0, mid.1) {
            for (p, id) in island_blocks(c, r) {
                let (lx, lz) = (p.x - cx * CW, p.z - cz * CW);
                if (0..CW).contains(&lx) && (0..CW).contains(&lz) && (1..CH - 1).contains(&p.y) {
                    b[crate::world::idx(lx, p.y, lz)] = id;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Game;
    use crate::world::GenOptions;

    fn an_island(seed: u32) -> (IVec3, i32) {
        (-8..8).flat_map(|gz| (-8..8).map(move |gx| (gx, gz))).find_map(|(gx, gz)| gravity_island(seed, gx, gz)).expect("a gravity island")
    }

    #[test]
    fn gravity_islands_hang_flat_side_down_in_newer_hollows() {
        let g = Generator::with_dim(5, GenOptions::DEFAULT, Dim::Hollow);
        let (c, _r) = an_island(5);
        let chunk = g.generate_hollow(c.x.div_euclid(CW), c.z.div_euclid(CW));
        let at = |p: IVec3| chunk[crate::world::idx(p.x.rem_euclid(CW), p.y, p.z.rem_euclid(CW))];
        assert_eq!(at(c), GRAVITY_CORE);
        assert_eq!(at(c + IVec3::Y), HOLLOW_STONE);
        assert_eq!(at(c - IVec3::Y), AIR);
        // Under it, down is up; above it and far off, it isn't.
        let here = |dy: f32| Vec3::new((c.x - Dim::Hollow.gen_x()) as f32 + 0.5, c.y as f32 + dy, c.z as f32 + 0.5);
        assert!(g.gravity_flipped(here(-3.0)));
        assert!(!g.gravity_flipped(here(5.0)));
        assert!(!g.gravity_flipped(here(-30.0)));
        // Older worlds don't have them.
        let old = Generator::with_dim(5, GenOptions { version: 8, ..GenOptions::DEFAULT }, Dim::Hollow);
        assert!(!old.gravity_flipped(here(-3.0)));
        assert_ne!(old.generate_hollow(c.x.div_euclid(CW), c.z.div_euclid(CW))[crate::world::idx(c.x.rem_euclid(CW), c.y, c.z.rem_euclid(CW))], GRAVITY_CORE);
    }

    #[test]
    fn you_fall_up_and_stand_on_the_underside() {
        let mut g = Game::new_with(5, false, false, GenOptions::DEFAULT);
        g.ready = true;
        let (dim, _) = g.hollow_destination(IVec3::ZERO);
        let (c, _) = an_island(5);
        let c = c - ivec3(Dim::Hollow.gen_x(), 0, 0);
        let start = Vec3::new(c.x as f32 + 1.5, c.y as f32 - 6.0, c.z as f32 + 0.5);
        g.move_local_player(dim, start);
        for dz in -1..=1 {
            for dx in -1..=1 {
                g.world.load_now(c.x.div_euclid(CW) + dx, c.z.div_euclid(CW) + dz);
            }
        }
        let still = crate::player::Input::default();
        for _ in 0..80 {
            g.gravity_tick();
            g.player.update(0.05, &still, &g.world, false);
        }
        assert!(g.player.upside_down);
        assert!(g.player.body.on_ground, "standing on the underside");
        assert!((g.player.body.pos.y + g.player.body.height - c.y as f32).abs() < 0.1, "head down, feet on the face: {}", g.player.body.pos.y);
        assert!(g.player.eye().y < g.player.body.pos.y + 0.5, "the eyes are at the bottom now");
        assert!(g.advancements.has("down_is_up"));
        // Jump: away from the face, and back.
        let jump = crate::player::Input { jump: true, ..Default::default() };
        g.player.update(0.05, &jump, &g.world, false);
        assert!(g.player.body.vel.y < 0.0);
    }
}
