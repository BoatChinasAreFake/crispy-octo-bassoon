//! More Zappy Dust parts: torches that invert, repeaters, pistons and
//! dispensers (see wiring.rs for the dust itself).
//!
//! - A **Zappy Torch** stands on a block and is lit unless that block is
//!   powered: an inverter. Lit, it powers everything beside it (not the block
//!   it stands on).
//! - A **Repeater** takes power from behind and passes it on to the front
//!   only, a moment later, at full strength again (so a line can run past 15
//!   blocks). It points away from whoever placed it.
//! - A **Piston** pushes up to 12 blocks in front of it when powered and
//!   pulls its head back when not. A **Sticky Piston** also pulls the block
//!   in front back with it. Bedrock, obsidian, containers and doors don't move.
//! - A **Dispenser** holds nine stacks and, each time power reaches it,
//!   fires out one thing: arrows fly, buckets pour or scoop, TNT lights,
//!   splash potions burst, Bone Dust fertilises, anything else is spat out.
//!
//! Pistons and dispensers face whoever placed them (up and down too).
//! Everything runs where the world lives, like the rest of the wiring.

use crate::block::*;
use crate::game::Game;
use crate::sound::{Mat, Sfx};
use crate::world::World;
use macroquad::math::{IVec3, Vec3};

/// Most blocks a piston pushes.
pub const PUSH_LIMIT: usize = 12;

/// Six ways a piston or dispenser can face: north, east, south, west, up, down.
pub fn dir6(f: u8) -> IVec3 {
    [IVec3::NEG_Z, IVec3::X, IVec3::Z, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y][(f % 6) as usize]
}

/// The facing (see `dir6`) that points along `d`.
pub fn facing_of(d: IVec3) -> u8 {
    (0..6).find(|&f| dir6(f) == d).unwrap_or(0)
}

/// The four flat ways (repeaters), same order as `dir6`.
pub fn dir4(f: u8) -> IVec3 {
    dir6(f % 4)
}

pub fn is_ztorch(id: Id) -> bool {
    id == ZTORCH_ON || id == ZTORCH_OFF
}
pub fn is_repeater(id: Id) -> bool {
    (REPEATER_FIRST..REPEATER_FIRST + 8).contains(&id)
}
/// (facing, on)
pub fn repeater_state(id: Id) -> (u8, bool) {
    let k = id - REPEATER_FIRST;
    ((k / 2) as u8, k % 2 == 1)
}
pub fn repeater(facing: u8, on: bool) -> Id {
    REPEATER_FIRST + (facing % 4) as Id * 2 + on as Id
}
pub fn is_piston(id: Id) -> bool {
    (PISTON_FIRST..PISTON_FIRST + 12).contains(&id) || (STICKY_FIRST..STICKY_FIRST + 12).contains(&id)
}
/// (facing, extended, sticky)
pub fn piston_state(id: Id) -> (u8, bool, bool) {
    let sticky = id >= STICKY_FIRST;
    let k = id - if sticky { STICKY_FIRST } else { PISTON_FIRST };
    ((k / 2) as u8, k % 2 == 1, sticky)
}
pub fn piston(facing: u8, extended: bool, sticky: bool) -> Id {
    (if sticky { STICKY_FIRST } else { PISTON_FIRST }) + (facing % 6) as Id * 2 + extended as Id
}
pub fn is_head(id: Id) -> bool {
    (HEAD_FIRST..HEAD_FIRST + 12).contains(&id)
}
/// (facing, sticky)
pub fn head_state(id: Id) -> (u8, bool) {
    let k = id - HEAD_FIRST;
    ((k / 2) as u8, k % 2 == 1)
}
pub fn head(facing: u8, sticky: bool) -> Id {
    HEAD_FIRST + (facing % 6) as Id * 2 + sticky as Id
}
pub fn is_dispenser(id: Id) -> bool {
    (DISPENSER_FIRST..DISPENSER_FIRST + 6).contains(&id)
}

/// Everything in this module, for the wiring's "is this zappy?" check.
pub fn is_contraption(id: Id) -> bool {
    (ZTORCH_ON..DISPENSER_FIRST + 6).contains(&id)
}

/// The item a family member is placed from (and drops as).
pub fn family(id: Id) -> Option<Id> {
    if is_ztorch(id) {
        Some(ZTORCH_ON)
    } else if is_repeater(id) {
        Some(REPEATER_FIRST)
    } else if (PISTON_FIRST..PISTON_FIRST + 12).contains(&id) {
        Some(PISTON_FIRST)
    } else if (STICKY_FIRST..STICKY_FIRST + 12).contains(&id) {
        Some(STICKY_FIRST)
    } else if is_dispenser(id) {
        Some(DISPENSER_FIRST)
    } else {
        None
    }
}

/// Pistons and dispensers show their face on the side they point (faces in
/// mesher::FACES order: +x, -x, +y, -y, +z, -z).
pub fn face_tile(id: Id, face: usize) -> Option<u16> {
    use crate::texture::*;
    let (facing, front, back) = if is_piston(id) {
        let (f, extended, sticky) = piston_state(id);
        (f, if extended { T_PISTON_BACK } else if sticky { T_STICKY_FACE } else { T_PISTON_FACE }, T_PISTON_BACK)
    } else if is_head(id) {
        let (f, sticky) = head_state(id);
        (f, if sticky { T_STICKY_FACE } else { T_PISTON_FACE }, T_PISTON_SIDE)
    } else if is_dispenser(id) {
        ((id - DISPENSER_FIRST) as u8, T_DISPENSER_FACE, T_COBBLE)
    } else {
        return None;
    };
    let d = dir6(facing);
    let n = [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z][face];
    Some(if n == d {
        front
    } else if n == -d {
        back
    } else if is_dispenser(id) {
        T_COBBLE
    } else {
        T_PISTON_SIDE
    })
}

/// Can a piston move this block?
pub fn movable(id: Id) -> bool {
    let fixed = block(id).hardness < 0.0 || matches!(id, OBSIDIAN | BEDROCK) || crate::containers::is_container(id) || is_door(id) || is_head(id) || is_dispenser(id) || crate::scorch::is_portal(id);
    if fixed {
        return false;
    }
    // A retracted piston moves; an extended one is stuck to its head.
    !is_piston(id) || !piston_state(id).1
}

/// Does the thing at `from` (beside `to`) send power into `to`?
pub fn powers(world: &World, from: IVec3, to: IVec3) -> bool {
    let id = world.get_v(from);
    if id == ZTORCH_ON {
        // Not down into the block it stands on.
        return to != from - IVec3::Y;
    }
    if is_repeater(id) {
        let (f, on) = repeater_state(id);
        return on && from + dir4(f) == to;
    }
    false
}

impl Game {
    /// A contraption at `p` looks at its power and acts.
    pub fn contraption_update(&mut self, p: IVec3) {
        let id = self.world.get_v(p);
        if is_ztorch(id) {
            // Lit unless the block below is powered (by anything but this torch).
            let lit = !crate::wiring::powered(&self.world, p - IVec3::Y);
            let want = if lit { ZTORCH_ON } else { ZTORCH_OFF };
            if want != id {
                self.world.set_v(p, want);
            }
        } else if is_repeater(id) {
            let (f, on) = repeater_state(id);
            let back = p - dir4(f);
            let b = self.world.get_v(back);
            let fed = crate::wiring::source_on(b) || b == WIRE_ON || powers(&self.world, back, p);
            if fed != on {
                self.world.set_v(p, repeater(f, fed));
            }
        } else if is_piston(id) {
            let (f, extended, sticky) = piston_state(id);
            let on = crate::wiring::powered(&self.world, p);
            if on && !extended {
                self.piston_push(p, f, sticky);
            } else if !on && extended {
                self.piston_pull(p, f, sticky);
            }
        } else if is_dispenser(id) {
            let on = crate::wiring::powered(&self.world, p);
            let was = self.dispensers_on.contains(&p);
            if on && !was {
                self.dispensers_on.insert(p);
                self.dispense(p, (id - DISPENSER_FIRST) as u8);
            } else if !on && was {
                self.dispensers_on.remove(&p);
            }
        }
    }

    fn piston_push(&mut self, p: IVec3, f: u8, sticky: bool) {
        let d = dir6(f);
        // What's in the way, up to the first gap.
        let mut line = Vec::new();
        let mut q = p + d;
        loop {
            let id = self.world.get_v(q);
            if replaceable(id) || !is_solid(id) && block(id).model == Model::Cross {
                break;
            }
            if !movable(id) || line.len() >= PUSH_LIMIT || !(0..crate::world::CH).contains(&q.y) {
                return; // too heavy, or stuck
            }
            line.push((q, id));
            q += d;
        }
        // Whatever soft thing sits at the end is squashed out of the way.
        let end = self.world.get_v(q);
        if end != AIR && !is_liquid(end) && block(end).drop != AIR {
            self.pop_drop(q.as_vec3() + Vec3::splat(0.5), block(end).drop, 1);
        }
        for &(c, id) in line.iter().rev() {
            self.world.set_v(c + d, id);
        }
        self.world.set_v(p + d, head(f, sticky));
        self.world.set_v(p, piston(f, true, sticky));
        self.shove_bodies(p + d, line.len() + 1, d);
        self.sfx(Sfx::Place(Mat::Stone), Some(p.as_vec3() + Vec3::splat(0.5)));
    }

    fn piston_pull(&mut self, p: IVec3, f: u8, sticky: bool) {
        let d = dir6(f);
        if is_head(self.world.get_v(p + d)) {
            self.world.set_v(p + d, AIR);
        }
        self.world.set_v(p, piston(f, false, sticky));
        if sticky {
            let grab = self.world.get_v(p + d * 2);
            if grab != AIR && is_solid(grab) && movable(grab) {
                self.world.set_v(p + d * 2, AIR);
                self.world.set_v(p + d, grab);
            }
        }
        self.sfx(Sfx::Place(Mat::Stone), Some(p.as_vec3() + Vec3::splat(0.5)));
    }

    /// Players and mobs in the pushed cells move along with the blocks.
    fn shove_bodies(&mut self, from: IVec3, n: usize, d: IVec3) {
        let cells: Vec<IVec3> = (0..n as i32).map(|i| from + d * i).collect();
        let inside = |pos: Vec3| cells.iter().any(|c| pos.floor().as_ivec3() == *c || (pos + Vec3::Y).floor().as_ivec3() == *c);
        if inside(self.player.body.pos) {
            self.player.body.pos += d.as_vec3();
        }
        for m in self.mobs.iter_mut() {
            if inside(m.body.pos) {
                m.body.pos += d.as_vec3();
            }
        }
    }

    /// Fire one thing out of the dispenser at `p`.
    fn dispense(&mut self, p: IVec3, f: u8) {
        let d = dir6(f);
        let front = p + d;
        let mouth = p.as_vec3() + Vec3::splat(0.5) + d.as_vec3() * 0.7;
        let Some(c) = self.world.containers.get(&p) else { return };
        let Some(slot) = c.slots.iter().position(|s| s.is_some()) else {
            self.sfx(Sfx::Click, Some(mouth));
            return;
        };
        let (item, n) = c.slots[slot].unwrap_or((AIR, 0));
        let wear = c.wear[slot];
        let keep = |g: &mut Game, replacement: Option<Id>| {
            if let Some(c) = g.world.containers.get_mut(&p) {
                c.slots[slot] = match replacement {
                    Some(r) => Some((r, 1)),
                    None if n > 1 => Some((item, n - 1)),
                    None => None,
                };
            }
            g.dirty_containers.insert(p);
        };
        match item {
            ARROW => {
                keep(self, None);
                self.spawn_arrow(mouth, d.as_vec3() * crate::entity::Arrow::SPEED, None);
                self.sfx(Sfx::Twang, Some(mouth));
            }
            WATER_BUCKET | LAVA_BUCKET if replaceable(self.world.get_v(front)) => {
                keep(self, Some(BUCKET));
                self.world.set_v(front, if item == WATER_BUCKET { WATER } else { LAVA });
            }
            BUCKET if matches!(self.world.get_v(front), WATER | LAVA) => {
                let full = if self.world.get_v(front) == WATER { WATER_BUCKET } else { LAVA_BUCKET };
                self.world.set_v(front, AIR);
                keep(self, Some(full));
            }
            TNT if replaceable(self.world.get_v(front)) => {
                keep(self, None);
                self.tnts.push(crate::entity::PrimedTnt { pos: front.as_vec3(), fuse: 3.0 });
                self.sfx(Sfx::Hiss, Some(mouth));
            }
            _ if matches!(crate::potions::potion_of(item), Some((_, true))) => {
                keep(self, None);
                if let Some((pot, _)) = crate::potions::potion_of(item) {
                    self.burst(pot, mouth + d.as_vec3() * 3.0);
                }
            }
            BONE_DUST if self.world.get_v(front) == SAPLING || crate::farming::Crop::of_block(self.world.get_v(front)).is_some() => {
                keep(self, None);
                if self.world.get_v(front) == SAPLING {
                    self.bone_sapling(front);
                } else {
                    self.farm_interact(front, BONE_DUST);
                }
            }
            _ => {
                keep(self, None);
                self.spawn_drop(mouth - Vec3::Y * 0.125, item, 1, wear, d.as_vec3() * 5.0 + Vec3::Y, 0.5);
                self.sfx(Sfx::Click, Some(mouth));
            }
        }
    }

    /// Which way a piston or dispenser placed now should face: toward the player.
    pub fn facing6(&self) -> u8 {
        let d = self.player.look_dir();
        if d.y > 0.7 {
            5 // looking up: it faces down at you
        } else if d.y < -0.7 {
            4
        } else {
            (self.facing() + 2) % 4
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use macroquad::math::ivec3;

    fn settle(g: &mut Game) {
        for _ in 0..40 {
            g.zap_tick(0.1);
        }
    }

    #[test]
    fn torches_invert() {
        let mut g = crate::game::tests::arena(71);
        let b = ivec3(2, 50, 2);
        g.world.set_v(b, STONE);
        g.world.set_v(b + IVec3::Y, ZTORCH_ON);
        settle(&mut g);
        assert_eq!(g.world.get_v(b + IVec3::Y), ZTORCH_ON);
        // A lamp beside the torch is lit by it.
        g.world.set_v(b + IVec3::Y + IVec3::X, LAMP);
        settle(&mut g);
        assert_eq!(g.world.get_v(b + IVec3::Y + IVec3::X), LAMP_ON);
        // Power the block the torch stands on: it goes out, and so does the lamp.
        g.world.set_v(b - IVec3::X, LEVER_ON);
        settle(&mut g);
        assert_eq!(g.world.get_v(b + IVec3::Y), ZTORCH_OFF);
        assert_eq!(g.world.get_v(b + IVec3::Y + IVec3::X), LAMP);
    }

    #[test]
    fn repeaters_carry_power_one_way() {
        let mut g = crate::game::tests::arena(72);
        let a = ivec3(0, 50, 0);
        // Lever, repeater pointing east, lamp; and a lamp behind the lever side stays dark.
        g.world.set_v(a, LEVER);
        g.world.set_v(a + IVec3::X, repeater(1, false));
        g.world.set_v(a + IVec3::X * 2, LAMP);
        settle(&mut g);
        assert_eq!(g.world.get_v(a + IVec3::X * 2), LAMP);
        g.world.set_v(a, LEVER_ON);
        settle(&mut g);
        assert!(repeater_state(g.world.get_v(a + IVec3::X)).1);
        assert_eq!(g.world.get_v(a + IVec3::X * 2), LAMP_ON);
        // Backwards does nothing.
        g.world.set_v(a, LEVER);
        g.world.set_v(a + IVec3::X, repeater(3, false));
        settle(&mut g);
        g.world.set_v(a, LEVER_ON);
        settle(&mut g);
        assert_eq!(g.world.get_v(a + IVec3::X * 2), LAMP);
    }

    #[test]
    fn pistons_push_and_sticky_ones_pull() {
        let mut g = crate::game::tests::arena(73);
        let p = ivec3(0, 50, -3);
        g.world.set_v(p, piston(1, false, true)); // facing east
        g.world.set_v(p + IVec3::X, PLANKS);
        g.world.set_v(p + IVec3::X * 2, COBBLE);
        g.world.set_v(p - IVec3::X, LEVER_ON);
        settle(&mut g);
        assert_eq!(g.world.get_v(p), piston(1, true, true));
        assert_eq!(g.world.get_v(p + IVec3::X), head(1, true));
        assert_eq!(g.world.get_v(p + IVec3::X * 2), PLANKS);
        assert_eq!(g.world.get_v(p + IVec3::X * 3), COBBLE);
        g.world.set_v(p - IVec3::X, LEVER);
        settle(&mut g);
        assert_eq!(g.world.get_v(p), piston(1, false, true));
        assert_eq!(g.world.get_v(p + IVec3::X), PLANKS, "sticky pulls back");
        assert_eq!(g.world.get_v(p + IVec3::X * 2), AIR);
        // Obsidian can't be pushed.
        g.world.set_v(p + IVec3::X, OBSIDIAN);
        g.world.set_v(p - IVec3::X, LEVER_ON);
        settle(&mut g);
        assert_eq!(g.world.get_v(p), piston(1, false, true));
    }

    #[test]
    fn dispensers_fire_on_each_pulse() {
        let mut g = crate::game::tests::arena(74);
        let d = ivec3(3, 50, 3);
        g.world.set_v(d, DISPENSER_FIRST + 1); // facing east
        if let Some(c) = g.world.containers.get_mut(&d) {
            c.slots[0] = Some((ARROW, 2));
            c.slots[1] = Some((COBBLE, 1));
        }
        g.world.set_v(d - IVec3::X, BUTTON_ON);
        settle(&mut g);
        assert_eq!(g.arrows.len(), 1);
        assert_eq!(g.world.containers[&d].slots[0], Some((ARROW, 1)));
        // The button pops out; press again for another shot.
        settle(&mut g);
        g.world.set_v(d - IVec3::X, BUTTON_ON);
        settle(&mut g);
        assert_eq!(g.arrows.len(), 2);
        assert_eq!(g.world.containers[&d].slots[0], None);
    }
}
