//! Spelunker's Rope: throw it down a shaft and climb it.
//!
//! Put a rope against a wall and it unrolls straight down (up to 32 blocks)
//! until it meets something. Click the top of a ledge and it goes over the
//! edge the way you're looking, down the hole. Climb it like a ladder. It
//! stays put, and maps mark where ropes hang. Break any part and the whole
//! rope comes back as the one rope it was.
//!
//! Joined players place the rope like any block; the host unrolls it (and
//! rolls it back up when it's broken), and the blocks arrive as usual.

use crate::block::*;
use crate::game::Game;
use crate::world::World;
use macroquad::math::{ivec3, IVec3, Vec3};

/// The longest a rope unrolls.
pub const ROPE_LEN: i32 = 32;

/// Where a rope used on `hit` (its `normal` face, looking along `look`) goes:
/// over the edge of a ledge if you clicked its top beside a drop, else the
/// cell on the clicked face.
pub fn spot(world: &World, hit: IVec3, normal: IVec3, look: Vec3) -> Option<IVec3> {
    if normal == IVec3::Y {
        let d = if look.x.abs() > look.z.abs() { ivec3(look.x.signum() as i32, 0, 0) } else { ivec3(0, 0, look.z.signum() as i32) };
        let edge = hit + d;
        if world.get_v(edge) == AIR && world.get_v(edge - IVec3::Y) == AIR {
            return Some(edge);
        }
    }
    let p = hit + normal;
    (p.y > 0 && p.y < crate::world::CH && world.get_v(p) == AIR).then_some(p)
}

/// The whole rope a cell is part of, top to bottom.
pub fn whole(world: &World, p: IVec3) -> Vec<IVec3> {
    let mut top = p;
    while world.get_v(top + IVec3::Y) == ROPE {
        top += IVec3::Y;
    }
    let mut out = Vec::new();
    let mut q = top;
    while world.get_v(q) == ROPE || q == p {
        out.push(q);
        q -= IVec3::Y;
    }
    out
}

/// Is this rope cell the top of its rope (what maps mark)?
pub fn is_top(world: &World, p: IVec3) -> bool {
    world.get_v(p) == ROPE && world.get_v(p + IVec3::Y) != ROPE
}

impl Game {
    /// The local player throws a rope at `hit`. True if it went somewhere.
    pub fn throw_rope(&mut self, hit: IVec3, normal: IVec3) -> bool {
        let Some(at) = spot(&self.world, hit, normal, self.player.look_dir()) else { return false };
        self.world.set_v(at, ROPE);
        self.player.swing = 1.0;
        self.sfx(crate::sound::Sfx::Place(crate::sound::Mat::Grass), Some(at.as_vec3() + Vec3::splat(0.5)));
        if !self.creative {
            self.inv.consume_held();
        }
        if !self.is_client() {
            self.unroll(at);
        }
        let below = self.world.get_v(at - IVec3::Y * 2);
        if below == ROPE {
            self.advance("rope_a_dope");
        }
        true
    }

    /// A rope just went in at `top` (where the world lives): it unrolls down.
    pub fn unroll(&mut self, top: IVec3) {
        for k in 1..ROPE_LEN {
            let p = top - IVec3::Y * k;
            if p.y < 1 || self.world.get_v(p) != AIR {
                break;
            }
            self.world.set_v(p, ROPE);
        }
    }

    /// A piece of rope at `p` was broken: the rest of it goes too (the one
    /// broken drops the rope, as usual).
    pub fn rope_broken(&mut self, p: IVec3) {
        for q in whole(&self.world, p) {
            if q != p {
                self.world.set_v(q, AIR);
            }
        }
    }

    /// Rope tops near `at`, for maps.
    pub fn rope_tops_near(&self, at: Vec3, r: f32) -> Vec<IVec3> {
        let mut tops: Vec<IVec3> = self.world.ropes.iter().copied().filter(|p| is_top(&self.world, *p) && Vec3::new(p.x as f32 - at.x, 0.0, p.z as f32 - at.z).length() < r).collect();
        tops.sort_by_key(|p| (p.x, p.z, p.y));
        tops
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A shaft 1 wide and 10 deep under the arena's floor, at x = 3.
    fn shaft(g: &mut Game) -> IVec3 {
        for y in 30..49 {
            for x in 2..=4 {
                for z in -1..=1 {
                    g.world.set_v(ivec3(x, y, z), STONE);
                }
            }
        }
        for y in 39..50 {
            g.world.set_v(ivec3(3, y, 0), AIR);
        }
        ivec3(3, 39, 0)
    }

    #[test]
    fn a_rope_over_a_ledge_hangs_to_the_bottom_and_climbs() {
        let mut g = crate::game::tests::arena(401);
        let bottom = shaft(&mut g);
        // Standing on the floor west of the hole, looking east at the edge.
        g.player.body.pos = Vec3::new(1.5, 50.0, 0.5);
        g.player.yaw = std::f32::consts::FRAC_PI_2;
        g.player.pitch = -0.6;
        g.inv.slots[0] = Some((ROPE, 1));
        g.inv.selected = 0;
        let look = g.player.look_dir();
        assert!(look.x > 0.5, "looking east: {look:?}");
        assert!(g.throw_rope(ivec3(2, 49, 0), IVec3::Y));
        assert_eq!(g.world.get_v(ivec3(3, 49, 0)), ROPE, "over the edge");
        assert_eq!(g.world.get_v(bottom), ROPE, "all the way down");
        assert_eq!(g.world.get_v(bottom - IVec3::Y), STONE);
        assert_eq!(g.inv.held(), AIR, "one rope used");
        assert!(crate::carpentry::on_ladder(&g.world, Vec3::new(3.2, 44.0, 0.2), Vec3::new(3.8, 45.8, 0.8)), "climbable");
        assert_eq!(g.rope_tops_near(Vec3::new(3.0, 0.0, 0.0), 10.0), vec![ivec3(3, 49, 0)]);
        // Break the middle: it all comes away, one rope dropped.
        g.break_block(ivec3(3, 44, 0), true);
        assert!((39..50).all(|y| g.world.get(3, y, 0) == AIR));
        assert_eq!(g.drops.iter().filter(|d| d.item == ROPE).map(|d| d.n as u32).sum::<u32>(), 1);
        assert!(g.world.ropes.is_empty());
    }

    #[test]
    fn a_rope_on_a_wall_unrolls_down_and_stops_at_the_floor() {
        let mut g = crate::game::tests::arena(402);
        g.world.set_v(ivec3(5, 55, 0), STONE);
        assert_eq!(spot(&g.world, ivec3(5, 55, 0), IVec3::NEG_X, Vec3::X), Some(ivec3(4, 55, 0)));
        g.inv.slots[0] = Some((ROPE, 2));
        g.inv.selected = 0;
        assert!(g.throw_rope(ivec3(5, 55, 0), IVec3::NEG_X));
        assert!((50..=55).all(|y| g.world.get(4, y, 0) == ROPE));
        assert_eq!(g.world.get(4, 49, 0), STONE);
        assert_eq!(whole(&g.world, ivec3(4, 52, 0)).len(), 6);
    }
}
