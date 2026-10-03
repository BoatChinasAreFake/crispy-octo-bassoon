//! Finding the way: mobs go round walls, up steps and through open doors
//! instead of walking straight into things.
//!
//! A mob heading for something (a player it's chasing, a partner, its home,
//! a chest) first looks along the straight line: if nothing's in the way it
//! just walks. Otherwise it searches (A*, a few hundred cells at most) over
//! the cells it could stand in, moving a block at a time: along the level, up
//! one block, or down at most three, never into lava, fire or cacti, with
//! room for its height. It follows the path a cell at a time, and searches
//! again every second or two, when its target moves, or when it gets stuck.
//! If the target can't be reached it goes as near as it can.

use crate::block::*;
use crate::world::World;
use macroquad::math::{ivec3, IVec3, Vec3};
use std::collections::{BinaryHeap, HashMap};

/// The most cells one search looks at, and how far it strays from the start.
pub const MAX_NODES: usize = 700;
pub const RANGE: i32 = 32;
/// Searched again after this long (seconds), or when the target has moved this far.
pub const REPATH: f32 = 1.6;
const MOVED: i32 = 3;
/// How far a mob drops down a step without minding.
pub const MAX_DROP: i32 = 3;

/// A mob's current way somewhere.
#[derive(Clone, Debug, Default)]
pub struct Path {
    pub target: IVec3,
    /// Cells still to walk to, next first.
    pub cells: Vec<IVec3>,
    pub age: f32,
}

/// Something in this cell stops you walking through it.
fn blocked(id: Id) -> bool {
    if let Some((_, open, _)) = door_state(id) {
        return !open;
    }
    is_solid(id)
}

/// Somewhere you'd rather not put your feet.
fn dangerous(id: Id) -> bool {
    is_lava(id) || matches!(id, FIRE | CACTUS)
}

/// Can a mob `height` blocks tall stand with its feet in `p`?
pub fn standable(world: &World, p: IVec3, height: i32) -> bool {
    let feet = world.get_v(p);
    if dangerous(feet) || (0..height).any(|y| blocked(world.get_v(p + IVec3::Y * y))) {
        return false;
    }
    let floor = world.get_v(p - IVec3::Y);
    !dangerous(floor) && (is_solid(floor) || is_water(feet))
}

/// The ground cell under a point (where the feet are, or the first standable
/// cell a little below).
pub fn cell_of(world: &World, at: Vec3, height: i32) -> IVec3 {
    let p = at.floor().as_ivec3();
    (0..=MAX_DROP).map(|d| p - IVec3::Y * d).find(|&c| standable(world, c, height)).unwrap_or(p)
}

/// Is the straight walk from `from` to `to` (feet) clear, at most `ahead` blocks of it?
pub fn clear_line(world: &World, from: Vec3, to: Vec3, height: i32, ahead: f32) -> bool {
    let flat = Vec3::new(to.x - from.x, 0.0, to.z - from.z);
    let len = flat.length().min(ahead);
    if len < 0.5 {
        return true;
    }
    let dir = flat.normalize();
    let mut y = from.y.floor() as i32;
    let mut s = 0.5;
    while s <= len {
        let p = from + dir * s;
        let (x, z) = (p.x.floor() as i32, p.z.floor() as i32);
        // Up a step, along, or down a little: anything more is in the way.
        let next = [y, y + 1, y - 1, y - 2, y - 3].into_iter().find(|&yy| standable(world, ivec3(x, yy, z), height));
        match next {
            Some(yy) => y = yy,
            None => return false,
        }
        s += 0.5;
    }
    true
}

#[derive(PartialEq, Eq)]
struct Open {
    f: i32,
    g: i32,
    p: IVec3,
}
impl Ord for Open {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        // Lowest estimate first (ties: the one further along, then position, so it's repeatable).
        o.f.cmp(&self.f).then(self.g.cmp(&o.g)).then(o.p.to_array().cmp(&self.p.to_array()))
    }
}
impl PartialOrd for Open {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}

fn estimate(a: IVec3, b: IVec3) -> i32 {
    let d = (a - b).abs();
    (d.x + d.z) * 10 + d.y * 5
}

/// A way from `start` to `goal` for a mob `height` tall: the cells to walk
/// through, next first (ending at the goal, or as near it as it could get).
pub fn find(world: &World, start: IVec3, goal: IVec3, height: i32) -> Vec<IVec3> {
    let mut open = BinaryHeap::new();
    let mut came: HashMap<IVec3, IVec3> = HashMap::new();
    let mut cost: HashMap<IVec3, i32> = HashMap::new();
    open.push(Open { f: estimate(start, goal), g: 0, p: start });
    cost.insert(start, 0);
    let mut best = (estimate(start, goal), start);
    let mut seen = 0;
    while let Some(Open { g, p, .. }) = open.pop() {
        if p == goal {
            best = (0, p);
            break;
        }
        if cost.get(&p).is_some_and(|&c| c < g) {
            continue;
        }
        seen += 1;
        if seen > MAX_NODES {
            break;
        }
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let side = p + ivec3(dx, 0, dz);
            if (side - start).abs().max_element() > RANGE {
                continue;
            }
            // Along, up a step (with headroom to jump), or down a little.
            let mut step = None;
            if standable(world, side, height) {
                step = Some((side, 10));
            } else if standable(world, side + IVec3::Y, height) && !blocked(world.get_v(p + IVec3::Y * height)) {
                step = Some((side + IVec3::Y, 16));
            } else if !(0..height).any(|y| blocked(world.get_v(side + IVec3::Y * y))) {
                step = (1..=MAX_DROP).map(|d| side - IVec3::Y * d).find(|&c| standable(world, c, height)).map(|c| (c, 10 + 4 * (side.y - c.y)));
            }
            let Some((next, step_cost)) = step else { continue };
            // Swimming is slow.
            let step_cost = if is_water(world.get_v(next)) { step_cost + 15 } else { step_cost };
            let g2 = g + step_cost;
            if cost.get(&next).is_none_or(|&c| g2 < c) {
                cost.insert(next, g2);
                came.insert(next, p);
                let h = estimate(next, goal);
                if h < best.0 {
                    best = (h, next);
                }
                open.push(Open { f: g2 + h, g: g2, p: next });
            }
        }
    }
    // Walk back from the goal (or the nearest we got to it).
    let mut cells = Vec::new();
    let mut at = best.1;
    while at != start {
        cells.push(at);
        match came.get(&at) {
            Some(&prev) => at = prev,
            None => break,
        }
    }
    cells.reverse();
    cells
}

/// Which way (yaw) a mob at `pos` should walk to get to `target`, following
/// (and keeping up) its `path`. None: just walk straight at it.
pub fn steer(world: &World, path: &mut Option<Box<Path>>, pos: Vec3, target: Vec3, height: f32, stuck: bool, dt: f32) -> Option<f32> {
    let h = height.ceil().max(1.0) as i32;
    let flat = Vec3::new(target.x - pos.x, 0.0, target.z - pos.z).length();
    // Close by, or nothing in the way on the level: straight there.
    if flat < 1.5 || (!stuck && (target.y - pos.y).abs() < 2.5 && clear_line(world, pos, target, h, 8.0)) {
        *path = None;
        return None;
    }
    let goal = cell_of(world, target, h);
    let start = cell_of(world, pos, h);
    let fresh = match path.as_deref_mut() {
        Some(p) => {
            p.age += dt;
            p.age > REPATH || (p.target - goal).abs().max_element() > MOVED || p.cells.is_empty() || (stuck && p.age > 0.4)
        }
        None => true,
    };
    if fresh {
        let cells = find(world, start, goal, h);
        *path = Some(Box::new(Path { target: goal, cells, age: 0.0 }));
    }
    let p = path.as_deref_mut()?;
    // Reached the next cell (near enough, across): on to the one after.
    while let Some(&c) = p.cells.first() {
        let centre = Vec3::new(c.x as f32 + 0.5, pos.y, c.z as f32 + 0.5);
        if centre.distance(Vec3::new(pos.x, pos.y, pos.z)) < 0.45 {
            p.cells.remove(0);
        } else {
            break;
        }
    }
    let c = *p.cells.first()?;
    let d = Vec3::new(c.x as f32 + 0.5 - pos.x, 0.0, c.z as f32 + 0.5 - pos.z);
    Some(d.x.atan2(-d.z))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A flat stone floor at y 49 with air above, from the test arena.
    fn floor() -> crate::game::Game {
        crate::game::tests::arena(101)
    }

    #[test]
    fn a_path_goes_round_a_wall_and_up_a_step() {
        let mut g = floor();
        // A wall across x = 3 from z -6 to 6, two high.
        for z in -6..=6 {
            for y in 50..52 {
                g.world.set(3, y, z, STONE);
            }
        }
        let path = find(&g.world, ivec3(0, 50, 0), ivec3(6, 50, 0), 2);
        assert_eq!(path.last(), Some(&ivec3(6, 50, 0)), "reaches the far side");
        assert!(path.iter().all(|c| !(c.x == 3 && (-6..=6).contains(&c.z))), "not through the wall");
        assert!(path.iter().any(|c| c.z.abs() >= 7), "round the end of it");
        // A step up onto a block.
        g.world.set(-3, 50, 0, STONE);
        let up = find(&g.world, ivec3(0, 50, 0), ivec3(-3, 51, 0), 2);
        assert_eq!(up.last(), Some(&ivec3(-3, 51, 0)));
        // No way through a closed box: it gets as near as it can.
        for (x, z) in [(8, -1), (8, 0), (8, 1), (9, -1), (9, 1), (10, -1), (10, 0), (10, 1)] {
            for y in 50..53 {
                g.world.set(x, y, z, STONE);
            }
        }
        g.world.set(9, 50, 0, AIR);
        let near = find(&g.world, ivec3(6, 50, 0), ivec3(9, 50, 0), 2);
        assert!(near.last().is_some_and(|c| (c.x - 9).abs() + c.z.abs() <= 3));
        assert!(!near.contains(&ivec3(9, 50, 0)));
    }

    #[test]
    fn lava_is_walked_round_and_the_straight_line_is_used_when_clear() {
        let mut g = floor();
        assert!(clear_line(&g.world, Vec3::new(0.5, 50.0, 0.5), Vec3::new(8.5, 50.0, 0.5), 2, 10.0));
        for z in -1..=1 {
            g.world.set(3, 49, z, LAVA);
        }
        assert!(!standable(&g.world, ivec3(3, 50, 0), 2));
        let path = find(&g.world, ivec3(0, 50, 0), ivec3(6, 50, 0), 2);
        assert!(!path.iter().any(|c| c.x == 3 && c.z.abs() <= 1), "no paddling in lava");
    }

    #[test]
    fn a_groaner_gets_round_a_wall_to_the_player() {
        let mut g = floor();
        g.rules.difficulty = crate::rules::Difficulty::Normal;
        for z in -5..=5 {
            for y in 50..53 {
                g.world.set(3, y, z, STONE);
            }
        }
        g.player.body.pos = Vec3::new(7.5, 50.0, 0.5);
        g.time = 0.75;
        let id = g.alloc_mob(crate::entity::MobKind::Groaner, Vec3::new(0.5, 50.0, 0.5));
        let mut closest = f32::MAX;
        for _ in 0..400 {
            g.player.hurt = 0.0;
            g.player.health = 20.0;
            g.update_entities(0.05);
            if let Some(m) = g.mobs.iter().find(|m| m.id == id) {
                closest = closest.min(m.body.pos.distance(g.player.body.pos));
            }
        }
        assert!(closest < 2.0, "it found a way round (got within {closest})");
    }
}
