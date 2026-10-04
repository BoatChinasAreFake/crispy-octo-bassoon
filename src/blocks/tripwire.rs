//! Tripwires: string laid across a floor between two hooks.
//!
//! Lay **string** on a floor (right-click with it) for a stretch of tripwire,
//! running the way you're facing; put a **Tripwire Hook** on a wall at each
//! end. Walk (or let anything walk) through any of it and the whole line
//! trips: it and both hooks give off Zappy power for a moment, which is how
//! jungle temples' dispensers know to fire arrows at you. Like pressure
//! plates, this happens where the world lives (see wiring.rs).

use crate::block::*;
use crate::world::World;
use macroquad::math::IVec3;

/// Longest line of string a trip runs along.
const MAX_LINE: i32 = 40;

pub fn is_tripwire(id: Id) -> bool {
    (TRIPWIRE_FIRST..TRIPWIRE_FIRST + 4).contains(&id)
}

pub fn is_hook(id: Id) -> bool {
    (TRIPWIRE_HOOK_FIRST..TRIPWIRE_HOOK_ON_FIRST + 4).contains(&id)
}

/// Tripwire running along `axis` (0 north-south, 1 east-west).
pub fn tripwire(axis: u8, on: bool) -> Id {
    TRIPWIRE_FIRST + (axis % 2) as Id * 2 + on as Id
}

/// A hook on the `facing` side of its cell.
pub fn hook(facing: u8, on: bool) -> Id {
    (if on { TRIPWIRE_HOOK_ON_FIRST } else { TRIPWIRE_HOOK_FIRST }) + (facing % 4) as Id
}

pub fn hook_facing(id: Id) -> u8 {
    ((id - TRIPWIRE_HOOK_FIRST) % 4) as u8
}

/// Tripped string or a tripped hook: these give off power.
pub fn tripped(id: Id) -> bool {
    (is_tripwire(id) && (id - TRIPWIRE_FIRST) % 2 == 1) || (TRIPWIRE_HOOK_ON_FIRST..TRIPWIRE_HOOK_ON_FIRST + 4).contains(&id)
}

/// The same thing, tripped or not.
pub fn with_tripped(id: Id, on: bool) -> Id {
    if is_tripwire(id) {
        tripwire(((id - TRIPWIRE_FIRST) / 2) as u8, on)
    } else if is_hook(id) {
        hook(hook_facing(id), on)
    } else {
        id
    }
}

/// The way a tripwire runs.
fn along(id: Id) -> IVec3 {
    if (id - TRIPWIRE_FIRST) / 2 == 0 { IVec3::Z } else { IVec3::X }
}

/// Every cell of the line a tripwire (or hook) at `p` is part of: the string
/// along its axis, and the hooks at its ends.
pub fn line(world: &World, p: IVec3) -> Vec<IVec3> {
    let id = world.get_v(p);
    let axis = if is_tripwire(id) {
        along(id)
    } else if is_hook(id) {
        // A hook's string runs away from its wall.
        let f = hook_facing(id);
        if f.is_multiple_of(2) { IVec3::Z } else { IVec3::X }
    } else {
        return Vec::new();
    };
    let mut out = vec![p];
    for dir in [axis, -axis] {
        let mut q = p + dir;
        for _ in 0..MAX_LINE {
            let here = world.get_v(q);
            if is_tripwire(here) && along(here) == axis {
                out.push(q);
            } else if is_hook(here) {
                out.push(q);
                break;
            } else {
                break;
            }
            q += dir;
        }
    }
    out
}

impl crate::game::Game {
    /// Something stood in tripwire at these cells: trip their lines (they
    /// reset with the pressure plates, a moment after everyone's gone).
    pub fn trip(&mut self, cells: &[IVec3]) {
        for &p in cells {
            for q in line(&self.world, p) {
                self.plates.insert(q, 0.5);
                let id = self.world.get_v(q);
                if !tripped(id) {
                    self.world.set_v(q, with_tripped(id, true));
                    if is_hook(id) {
                        self.sfx(crate::sound::Sfx::Click, Some(q.as_vec3() + macroquad::math::Vec3::splat(0.5)));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use macroquad::math::ivec3;

    #[test]
    fn a_line_trips_end_to_end_and_fires_a_dispenser() {
        let mut g = crate::game::tests::arena(91);
        let y = 50;
        // Hooks on walls at x = 0 and x = 4, string between, running east-west.
        g.world.set_v(ivec3(-1, y, 3), STONE);
        g.world.set_v(ivec3(0, y, 3), hook(3, false));
        for x in 1..4 {
            g.world.set_v(ivec3(x, y, 3), tripwire(1, false));
        }
        g.world.set_v(ivec3(4, y, 3), hook(1, false));
        // A dispenser of arrows next to the far hook, facing back along the line.
        let disp = ivec3(5, y, 3);
        g.world.set_v(disp, DISPENSER_FIRST + 3);
        let mut c = crate::containers::Container::for_block(DISPENSER_FIRST);
        c.slots[0] = Some((ARROW, 5));
        g.world.containers.insert(disp, c);
        assert_eq!(line(&g.world, ivec3(2, y, 3)).len(), 5);
        assert!(!crate::wiring::powered(&g.world, disp));
        g.trip(&[ivec3(1, y, 3)]);
        assert!(tripped(g.world.get_v(ivec3(4, y, 3))), "the far hook trips too");
        assert!(crate::wiring::powered(&g.world, disp));
        for _ in 0..10 {
            g.zap_tick(0.1);
        }
        let left = g.world.containers.get(&disp).and_then(|c| c.slots.iter().flatten().map(|s| s.1).next()).unwrap_or(0);
        assert!(left < 5, "it fired");
        // Nobody there now: it all resets.
        for _ in 0..20 {
            g.zap_tick(0.1);
        }
        assert!(!tripped(g.world.get_v(ivec3(2, y, 3))) && !tripped(g.world.get_v(ivec3(0, y, 3))));
    }
}
