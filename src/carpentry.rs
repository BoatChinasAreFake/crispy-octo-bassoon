//! Building bits: fences, fence gates, ladders, trapdoors, glass panes, and
//! dyes for coloured wool and stained glass.
//!
//! - **Fences** and **glass panes** join up with their neighbours by
//!   themselves: fences to fences, gates and solid blocks, panes to panes,
//!   glass and solid blocks. Which sides are joined is part of the block, and
//!   it's worked out where the world lives whenever something next door
//!   changes (joined players get it as an ordinary edit).
//! - Fences and shut gates stand a bit over a block and a half tall for anything trying
//!   to jump them, so animals stay in.
//! - **Gates** and **trapdoors** open and close with a right-click.
//! - **Ladders** go on walls. Walk into one to climb, sneak to hold on, and
//!   you slide down slowly otherwise.
//! - **Dyes** come from flowers, coal, Bone Dust, pumpkins, gold (finally!),
//!   Pokey Plants and tropical fish; red and blue make purple.

use crate::block::*;
use crate::game::Game;
use crate::sound::{Mat, Sfx};
use crate::world::World;
use macroquad::math::{IVec3, Vec3};

/// Colours: saved key and display name.
pub const COLOURS: [(&str, &str); 8] = [
    ("white", "White"),
    ("black", "Black"),
    ("red", "Red"),
    ("orange", "Orange"),
    ("yellow", "Yellow"),
    ("green", "Green"),
    ("blue", "Blue"),
    ("purple", "Purple"),
];

pub fn colour_rgb(c: usize) -> [u8; 3] {
    [[235, 235, 235], [40, 40, 45], [190, 40, 40], [230, 130, 40], [235, 210, 50], [80, 150, 50], [50, 80, 200], [130, 60, 170]][c % 8]
}

pub fn is_fence(id: Id) -> bool {
    (FENCE_FIRST..FENCE_FIRST + 16).contains(&id)
}
pub fn is_pane(id: Id) -> bool {
    (PANE_FIRST..PANE_FIRST + 16).contains(&id)
}
pub fn is_gate(id: Id) -> bool {
    (GATE_FIRST..GATE_FIRST + 4).contains(&id)
}
pub fn is_ladder(id: Id) -> bool {
    (LADDER_FIRST..LADDER_FIRST + 4).contains(&id)
}
pub fn is_trapdoor(id: Id) -> bool {
    (TRAPDOOR_FIRST..TRAPDOOR_FIRST + 8).contains(&id)
}
pub fn is_stained_glass(id: Id) -> bool {
    (STAINED_GLASS..STAINED_GLASS + 8).contains(&id)
}

/// The item a family member is placed from (and drops as).
pub fn family(id: Id) -> Option<Id> {
    if is_fence(id) {
        Some(FENCE_FIRST)
    } else if is_pane(id) {
        Some(PANE_FIRST)
    } else if is_gate(id) {
        Some(GATE_FIRST)
    } else if is_ladder(id) {
        Some(LADDER_FIRST)
    } else if is_trapdoor(id) {
        Some(TRAPDOOR_FIRST)
    } else {
        None
    }
}

/// East, west, south, north: the bits of a join mask.
const SIDES: [(IVec3, u8); 4] = [(IVec3::X, 1), (IVec3::NEG_X, 2), (IVec3::Z, 4), (IVec3::NEG_Z, 8)];

/// Does a fence (or pane) reach out to this neighbour?
fn joins(pane: bool, other: Id) -> bool {
    if pane {
        is_pane(other) || other == GLASS || is_stained_glass(other) || is_opaque(other)
    } else {
        is_fence(other) || is_gate(other) || is_opaque(other)
    }
}

/// Which sides a fence or pane at `p` should join.
pub fn join_mask(world: &World, p: IVec3, pane: bool) -> u8 {
    SIDES.iter().filter(|(d, _)| joins(pane, world.get_v(p + *d))).fold(0, |m, (_, b)| m | b)
}

/// How high fences stand for collisions: a little over a jump.
pub const TALL: f32 = 1.6;

/// Things that stand taller than their block for anything trying to jump them.
pub fn is_tall(id: Id) -> bool {
    is_fence(id) || (is_gate(id) && (id - GATE_FIRST).is_multiple_of(2))
}

/// A gate's state: (spans east-west, open).
pub fn gate_state(id: Id) -> (bool, bool) {
    let k = id - GATE_FIRST;
    (k >= 2, k % 2 == 1)
}

pub fn gate(x_axis: bool, open: bool) -> Id {
    GATE_FIRST + x_axis as Id * 2 + open as Id
}

pub fn trapdoor(facing: u8, open: bool) -> Id {
    TRAPDOOR_FIRST + (facing % 4) as Id * 2 + open as Id
}

impl World {
    /// Fences and panes at and beside `p` join up with what's around them now.
    pub fn reshape_joins(&mut self, p: IVec3) {
        for q in [p, p + IVec3::X, p - IVec3::X, p + IVec3::Z, p - IVec3::Z] {
            let id = self.get_v(q);
            let (first, pane) = if is_fence(id) {
                (FENCE_FIRST, false)
            } else if is_pane(id) {
                (PANE_FIRST, true)
            } else {
                continue;
            };
            let want = first + join_mask(self, q, pane) as Id;
            if want != id {
                self.set_v(q, want);
            }
        }
    }
}

impl Game {
    /// Open or shut a gate or trapdoor at `pos`. Returns whether there was one.
    pub fn toggle_hinged(&mut self, pos: IVec3) -> bool {
        let id = self.world.get_v(pos);
        let new = if is_gate(id) {
            let (x_axis, open) = gate_state(id);
            // Gates open away from you, which here just means across your way.
            gate(x_axis, !open)
        } else if is_trapdoor(id) {
            let k = id - TRAPDOOR_FIRST;
            trapdoor((k / 2) as u8, k.is_multiple_of(2))
        } else {
            return false;
        };
        // Don't shut it on someone.
        if is_solid(new) && self.cell_occupied(pos) && is_gate(id) {
            return true;
        }
        self.world.set_v(pos, new);
        self.sfx(Sfx::Place(Mat::Wood), Some(pos.as_vec3() + Vec3::splat(0.5)));
        self.player.swing = 1.0;
        true
    }

    /// The block to place for a gate, ladder or trapdoor held in hand.
    pub fn hinged_facing(&self, held: Id, normal: IVec3) -> Option<Id> {
        match held {
            GATE_FIRST => {
                // Across the way you're facing.
                let f = self.facing();
                Some(gate(f.is_multiple_of(2), false))
            }
            LADDER_FIRST => crate::decor::frame_facing(normal).map(|f| LADDER_FIRST + f as Id),
            TRAPDOOR_FIRST => {
                // Hinged on the side you put it against, or the side nearest you.
                let f = crate::decor::frame_facing(normal).unwrap_or((self.facing() + 2) % 4);
                Some(trapdoor(f, false))
            }
            _ => None,
        }
    }
}

/// Is a body in this cell range on a ladder?
pub fn on_ladder(world: &World, min: Vec3, max: Vec3) -> bool {
    let (x0, x1) = (min.x.floor() as i32, (max.x - 1e-3).floor() as i32);
    let (z0, z1) = (min.z.floor() as i32, (max.z - 1e-3).floor() as i32);
    let (y0, y1) = (min.y.floor() as i32, (min.y + 1.0).floor() as i32);
    (y0..=y1).any(|y| (z0..=z1).any(|z| (x0..=x1).any(|x| is_ladder(world.get(x, y, z)))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use macroquad::math::ivec3;

    #[test]
    fn fences_join_up_and_come_apart() {
        let mut g = crate::game::tests::arena(41);
        let a = ivec3(2, 50, 2);
        g.world.set_v(a, FENCE_FIRST);
        g.world.set_v(a + IVec3::X, FENCE_FIRST);
        g.world.set_v(a + IVec3::Z, FENCE_FIRST);
        // East and south of the first one.
        assert_eq!(g.world.get_v(a), FENCE_FIRST + 1 + 4);
        assert_eq!(g.world.get_v(a + IVec3::X), FENCE_FIRST + 2);
        g.world.set_v(a + IVec3::X, AIR);
        assert_eq!(g.world.get_v(a), FENCE_FIRST + 4);
        // Panes join glass but not fences.
        let p = ivec3(-3, 50, -3);
        g.world.set_v(p, PANE_FIRST);
        g.world.set_v(p + IVec3::X, GLASS);
        g.world.set_v(p - IVec3::X, FENCE_FIRST);
        assert_eq!(g.world.get_v(p), PANE_FIRST + 1);
    }

    #[test]
    fn fences_are_too_tall_to_jump() {
        let mut g = crate::game::tests::arena(42);
        for x in -3..=3 {
            g.world.set_v(ivec3(x, 50, -2), FENCE_FIRST);
        }
        g.player.body.pos = Vec3::new(0.5, 50.0, 0.5);
        let c = crate::game::Controls { input: crate::player::Input { forward: 1.0, jump: true, ..Default::default() }, ..Default::default() };
        g.player.yaw = 0.0;
        for _ in 0..120 {
            g.update(1.0 / 60.0, &c);
        }
        assert!(g.player.body.pos.z > -1.3, "jumped the fence: {:?}", g.player.body.pos);
    }

    #[test]
    fn gates_and_trapdoors_swing() {
        let mut g = crate::game::tests::arena(43);
        let p = ivec3(3, 50, 3);
        g.world.set_v(p, gate(true, false));
        assert!(g.toggle_hinged(p));
        assert_eq!(g.world.get_v(p), gate(true, true));
        assert!(!is_solid(g.world.get_v(p)));
        g.world.set_v(p, trapdoor(2, false));
        g.toggle_hinged(p);
        assert_eq!(g.world.get_v(p), trapdoor(2, true));
    }

    #[test]
    fn ladders_climb() {
        let mut g = crate::game::tests::arena(44);
        for y in 50..58 {
            g.world.set_v(ivec3(0, y, -1), STONE);
            g.world.set_v(ivec3(0, y, 0), LADDER_FIRST);
        }
        g.player.body.pos = Vec3::new(0.5, 50.0, 0.5);
        g.player.yaw = 0.0;
        let c = crate::game::Controls { input: crate::player::Input { forward: 1.0, ..Default::default() }, ..Default::default() };
        for _ in 0..90 {
            g.update(1.0 / 60.0, &c);
        }
        assert!(g.player.body.pos.y > 52.0, "didn't climb: {:?}", g.player.body.pos);
    }
}
