//! Flowing water and lava, and buckets to carry them.
//!
//! Liquids are ordinary blocks: a source (`WATER`, `LAVA`) and flowing ones a
//! few steps away from it (water runs 7 blocks, lava 3). Nothing flows until
//! something changes nearby: the world queues the cells around every edit
//! that touches a liquid (see `World::wake_liquids`), and where the world
//! lives (single player, host, server) those cells work out what they should
//! be from their neighbours, water every quarter second and lava every second
//! and a half. So oceans sit still until someone digs a hole next to them.
//!
//! The rules, Minecraft's more or less:
//! - A liquid falls first; it only spreads sideways from something it can't fall through.
//! - Each step sideways is one level weaker; past its reach it stops.
//! - Two water sources beside each other on firm ground make a third.
//! - Take the source away and the rest drains.
//! - Water on a lava source makes obsidian, on flowing lava cobblestone,
//!   and lava poured onto water turns it to stone.
//! - Plants and torches are washed away (water) or burnt (lava).

use crate::block::*;
use crate::game::Game;
use crate::sound::Sfx;
use crate::world::{World, CH};
use macroquad::math::{IVec3, Vec3};

pub const WATER_STEP: f32 = 0.25;
pub const LAVA_STEP: f32 = 1.5;
/// Most cells looked at per step (a hole into the sea spreads over several).
const MAX_PER_STEP: usize = 4096;

const SIDES: [IVec3; 4] = [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z];

/// Cells a liquid simply runs into: air, plants, torches (but not kelp and
/// the like, which are full of water already).
pub fn open(id: Id) -> bool {
    id == AIR || (block(id).model == Model::Cross && !waterlogged(id))
}

/// Can liquid fall from above into a cell holding `below`?
fn falls_into(below: Id) -> bool {
    open(below) || (is_liquid(below) && liquid_level(below) > 0)
}

/// How strong a liquid of this kind would be at `p`, fed by its neighbours
/// (Some(0): a new source). None: nothing of this kind reaches it.
fn feed(get: &impl Fn(IVec3) -> Id, p: IVec3, lava: bool) -> Option<u8> {
    // (Waterlogged blocks count as water sources.)
    let same = |id: Id| if lava { is_lava(id) } else { is_wet(id) };
    let reach = if lava { LAVA_REACH } else { WATER_REACH };
    if same(get(p + IVec3::Y)) {
        return Some(1);
    }
    let mut best: Option<u8> = None;
    let mut sources = 0;
    for d in SIDES {
        let n = p + d;
        let id = get(n);
        if !same(id) {
            continue;
        }
        let l = liquid_level(id);
        if l == 0 {
            sources += 1;
        }
        // Something that can still fall doesn't spread sideways.
        if falls_into(get(n - IVec3::Y)) || l >= reach {
            continue;
        }
        best = Some(best.map_or(l + 1, |b| b.min(l + 1)));
    }
    if !lava && sources >= 2 {
        let below = get(p - IVec3::Y);
        if is_solid(below) || below == WATER || waterlogged(below) {
            return Some(0);
        }
    }
    best
}

/// What cell `p` should become now (None: leave it). Pure, for testing.
pub fn settle(get: &impl Fn(IVec3) -> Id, p: IVec3) -> Option<Id> {
    let cur = get(p);
    let touches_water = || SIDES.iter().chain([IVec3::Y].iter()).any(|d| is_wet(get(p + *d)));
    match cur {
        // Sources stay put, unless the other liquid gets to them.
        WATER => return is_lava(get(p + IVec3::Y)).then_some(STONE),
        // Kelp and the like hold their water, like a source.
        _ if waterlogged(cur) => return None,
        LAVA => return touches_water().then_some(OBSIDIAN),
        _ if !(open(cur) || is_liquid(cur)) => return None,
        _ => {}
    }
    let new = match (feed(get, p, false), feed(get, p, true)) {
        (Some(_), Some(_)) => COBBLE,
        (Some(_), None) if is_lava(cur) => COBBLE,
        (Some(level), None) => liquid_at(false, level),
        (None, Some(_)) if is_water(cur) || touches_water() => COBBLE,
        (None, Some(level)) => liquid_at(true, level),
        (None, None) if is_liquid(cur) => AIR,
        (None, None) => return None,
    };
    (new != cur).then_some(new)
}

/// Which way flowing water (or lava) pushes whatever is in it, if at all.
pub fn current(world: &World, pos: Vec3) -> Vec3 {
    let p = IVec3::new(pos.x.floor() as i32, pos.y.floor() as i32, pos.z.floor() as i32);
    let id = world.get_v(p);
    if !is_liquid(id) {
        return Vec3::ZERO;
    }
    let level = liquid_level(id) as i32;
    let mut push = Vec3::ZERO;
    for d in SIDES {
        let n = world.get_v(p + d);
        if is_liquid(n) && is_lava(n) == is_lava(id) {
            // Toward weaker neighbours (sources count as strongest).
            push += d.as_vec3() * (liquid_level(n) as i32 - level) as f32;
        } else if open(n) && falls_into(world.get_v(p + d - IVec3::Y)) {
            // Toward a drop.
            push += d.as_vec3() * 2.0;
        }
    }
    push.normalize_or_zero()
}

impl Game {
    /// Let queued cells flow (only where the world lives).
    pub fn liquid_tick(&mut self, dt: f32) {
        if !self.world.simulate_liquids {
            self.world.liquid_dirty.clear();
            return;
        }
        self.liquid_timers[0] += dt;
        self.liquid_timers[1] += dt;
        if self.liquid_timers[0] >= WATER_STEP {
            self.liquid_timers[0] = 0.0;
            self.liquid_step(false);
        }
        if self.liquid_timers[1] >= LAVA_STEP {
            self.liquid_timers[1] = 0.0;
            self.liquid_step(true);
        }
    }

    /// One step: water cells now, lava cells saved for lava's slower beat.
    fn liquid_step(&mut self, lava_turn: bool) {
        let mut cells: Vec<IVec3> = if lava_turn { self.lava_waiting.drain().collect() } else { Vec::new() };
        let dirty: Vec<IVec3> = self.world.liquid_dirty.iter().copied().take(MAX_PER_STEP).collect();
        for p in &dirty {
            self.world.liquid_dirty.remove(p);
        }
        for p in dirty {
            let lava_here = is_lava(self.world.get_v(p)) || [IVec3::Y, IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z].iter().any(|d| is_lava(self.world.get_v(p + *d)));
            if lava_here && !lava_turn {
                self.lava_waiting.insert(p);
            } else {
                cells.push(p);
            }
        }
        cells.sort_unstable_by_key(|p| (p.y, p.x, p.z));
        cells.dedup();
        for p in cells {
            if !(0..CH).contains(&p.y) || !self.world.is_loaded(p.x, p.z) {
                continue;
            }
            // Cells at the edge of the loaded world stay put (they'd flow into nothing).
            if SIDES.iter().any(|d| !self.world.is_loaded(p.x + d.x, p.z + d.z)) {
                continue;
            }
            let world = &self.world;
            let Some(new) = settle(&|q: IVec3| world.get_v(q), p) else { continue };
            let old = self.world.get_v(p);
            // Plants and torches wash out (and pop off as items) or burn up.
            if old != AIR && open(old) && is_water(new) {
                let d = block(old).drop;
                if d != AIR {
                    self.pop_drop(p.as_vec3() + Vec3::splat(0.5), d, 1);
                }
            }
            if matches!(new, OBSIDIAN | COBBLE | STONE) {
                self.sfx(Sfx::Hiss, Some(p.as_vec3() + Vec3::splat(0.5)));
                self.smoke(p.as_vec3() + Vec3::new(0.5, 1.0, 0.5), 6, 0.3);
            }
            self.world.set_v(p, new);
            if is_lava(new) {
                self.lava_ignites(p);
            }
        }
    }

    /// Right-click with a bucket: scoop up a source, or pour one out.
    /// True if it did something.
    pub fn use_bucket(&mut self, held: Id) -> bool {
        let eye = self.player.eye();
        let dir = self.player.look_dir();
        let reach = if self.creative { 6.5 } else { 5.0 };
        if held == BUCKET {
            let Some(h) = self.world.raycast_liquid(eye, dir, reach) else { return false };
            let id = self.world.get_v(h.pos);
            if id != WATER && id != LAVA {
                return false;
            }
            self.world.set_v(h.pos, AIR);
            let full = if id == WATER { WATER_BUCKET } else { LAVA_BUCKET };
            if !self.creative {
                self.inv.consume_held();
                self.give(full, 1);
            }
            self.sfx(if id == WATER { Sfx::Splash } else { Sfx::Hiss }, Some(h.pos.as_vec3() + Vec3::splat(0.5)));
            self.player.swing = 1.0;
            if id == LAVA {
                self.advance("hot_stuff");
            }
            return true;
        }
        let liquid = match held {
            WATER_BUCKET => WATER,
            LAVA_BUCKET => LAVA,
            _ => return false,
        };
        let Some(h) = self.world.raycast(eye, dir, reach) else { return false };
        let hit = self.world.get_v(h.pos);
        let place = if replaceable(hit) { h.pos } else { h.pos + h.normal };
        let there = self.world.get_v(place);
        if !(0..CH).contains(&place.y) || !(replaceable(there) || open(there)) || there == liquid {
            return false;
        }
        // Water boils away in the Scorchlands.
        if liquid == WATER && self.world.is_scorch() {
            if !self.creative {
                let slot = self.inv.selected;
                self.inv.slots[slot] = Some((BUCKET, 1));
            }
            self.sfx(Sfx::Hiss, Some(place.as_vec3() + Vec3::splat(0.5)));
            self.smoke(place.as_vec3() + Vec3::splat(0.5), 10, 0.4);
            self.msg("The water boiled away. It's the Scorchlands, what did you expect?");
            return true;
        }
        self.world.set_v(place, liquid);
        if !self.creative {
            let slot = self.inv.selected;
            self.inv.slots[slot] = Some((BUCKET, 1));
            self.inv.wear[slot] = 0;
        }
        self.sfx(if liquid == WATER { Sfx::Splash } else { Sfx::Hiss }, Some(place.as_vec3() + Vec3::splat(0.5)));
        self.player.swing = 1.0;
        true
    }

    /// Lava burns, fire lingers, water puts it out (the local player).
    pub fn lava_tick(&mut self, dt: f32) {
        if self.creative || self.dead.is_some() || self.has_effect(crate::potions::Potion::FireResistance) {
            self.on_fire = 0.0;
            return;
        }
        let b = &self.player.body;
        let (min, max) = (b.min() + Vec3::splat(0.05), b.max() - Vec3::splat(0.05));
        let mut lava = false;
        for y in min.y.floor() as i32..=max.y.floor() as i32 {
            for z in min.z.floor() as i32..=max.z.floor() as i32 {
                for x in min.x.floor() as i32..=max.x.floor() as i32 {
                    lava |= is_lava(self.world.get(x, y, z));
                }
            }
        }
        if !lava && crate::fire::touches_fire(&self.world, min, max) {
            self.on_fire = self.on_fire.max(4.0);
        }
        if lava {
            self.on_fire = 8.0;
            self.hurt_player(4.0, "tried to swim in lava. It's not that kind of pool");
            self.advance("hot_stuff");
            return;
        }
        let p = self.player.body.pos;
        let wet = self.player.body.in_water || self.rained_on(p.x.floor() as i32, p.y.floor() as i32 + 1, p.z.floor() as i32);
        if wet && self.on_fire > 0.0 {
            self.on_fire = 0.0;
            self.sfx(Sfx::Hiss, None);
        }
        if self.on_fire > 0.0 {
            let before = self.on_fire;
            self.on_fire = (self.on_fire - dt).max(0.0);
            // A point of damage a second while it lasts.
            if before.ceil() != self.on_fire.ceil() {
                self.player.hurt = 0.0;
                self.hurt_player(1.0, "burned to a crisp. Extra crispy");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// A little world in a map: stone floor at y = 0, air above.
    struct Grid(HashMap<IVec3, Id>);
    impl Grid {
        fn new() -> Grid {
            let mut m = HashMap::new();
            for x in -12..=12 {
                for z in -12..=12 {
                    m.insert(IVec3::new(x, 0, z), STONE);
                }
            }
            Grid(m)
        }
        fn get(&self, p: IVec3) -> Id {
            if p.y < 0 { STONE } else { self.0.get(&p).copied().unwrap_or(AIR) }
        }
        /// Settle everything until nothing changes.
        fn run(&mut self) -> usize {
            for round in 0..200 {
                let cells: Vec<IVec3> = (-10..=10).flat_map(|x| (0..6).flat_map(move |y| (-10..=10).map(move |z| IVec3::new(x, y, z)))).collect();
                let mut changes = Vec::new();
                for p in cells {
                    if let Some(n) = settle(&|q| self.get(q), p) {
                        changes.push((p, n));
                    }
                }
                if changes.is_empty() {
                    return round;
                }
                for (p, n) in changes {
                    self.0.insert(p, n);
                }
            }
            panic!("never settled");
        }
    }

    #[test]
    fn water_spreads_falls_and_drains() {
        let mut g = Grid::new();
        g.0.insert(IVec3::new(0, 1, 0), WATER);
        g.run();
        // Seven blocks out in a diamond, one level weaker each step.
        assert_eq!(g.get(IVec3::new(1, 1, 0)), liquid_at(false, 1));
        assert_eq!(g.get(IVec3::new(3, 1, 4)), liquid_at(false, 7));
        assert_eq!(g.get(IVec3::new(4, 1, 4)), AIR);
        assert_eq!(g.get(IVec3::new(0, 1, 7)), liquid_at(false, 7));
        // Take the source away and it all drains.
        g.0.insert(IVec3::new(0, 1, 0), AIR);
        g.run();
        assert!((-8..=8).all(|x| (-8..=8).all(|z| g.get(IVec3::new(x, 1, z)) == AIR)));

        // Poured from a height it falls first, then spreads at the bottom.
        let mut g = Grid::new();
        g.0.insert(IVec3::new(0, 4, 0), STONE);
        g.0.insert(IVec3::new(0, 5, 0), WATER);
        g.0.insert(IVec3::new(1, 4, 0), AIR);
        g.run();
        assert_eq!(g.get(IVec3::new(1, 5, 0)), liquid_at(false, 1));
        assert_eq!(g.get(IVec3::new(1, 3, 0)), liquid_at(false, 1), "falling water is strong");
        assert_eq!(g.get(IVec3::new(2, 5, 0)), AIR, "it falls rather than spreading");
        assert_eq!(g.get(IVec3::new(1, 1, 0)), liquid_at(false, 1));
        assert_eq!(g.get(IVec3::new(3, 1, 0)), liquid_at(false, 3));

        // Two sources make a third between them.
        let mut g = Grid::new();
        g.0.insert(IVec3::new(0, 1, 0), WATER);
        g.0.insert(IVec3::new(2, 1, 0), WATER);
        g.run();
        assert_eq!(g.get(IVec3::new(1, 1, 0)), WATER);
    }

    #[test]
    fn lava_is_short_and_meets_water() {
        let mut g = Grid::new();
        g.0.insert(IVec3::new(0, 1, 0), LAVA);
        g.run();
        assert_eq!(g.get(IVec3::new(3, 1, 0)), liquid_at(true, 3));
        assert_eq!(g.get(IVec3::new(4, 1, 0)), AIR);
        // Water reaching the source makes obsidian.
        g.0.insert(IVec3::new(-1, 1, 0), WATER);
        g.run();
        assert_eq!(g.get(IVec3::new(0, 1, 0)), OBSIDIAN);
        // Water meeting flowing lava cools it to cobblestone.
        let mut g = Grid::new();
        g.0.insert(IVec3::new(0, 1, 0), WATER);
        g.0.insert(IVec3::new(1, 1, 0), liquid_at(true, 1));
        g.0.insert(IVec3::new(2, 1, 0), LAVA);
        assert_eq!(settle(&|q| g.get(q), IVec3::new(1, 1, 0)), Some(COBBLE));
        // Lava poured on a water source makes stone.
        let mut g = Grid::new();
        g.0.insert(IVec3::new(0, 1, 0), WATER);
        g.0.insert(IVec3::new(0, 2, 0), LAVA);
        for x in -1..=1 {
            for z in -1..=1 {
                if (x, z) != (0, 0) {
                    g.0.insert(IVec3::new(x, 1, z), STONE);
                    g.0.insert(IVec3::new(x, 2, z), STONE);
                }
            }
        }
        g.run();
        assert_eq!(g.get(IVec3::new(0, 1, 0)), STONE);
        // Torches and flowers are just in the way.
        assert!(open(TORCH) && open(FLOWER) && !open(STONE));
    }
}
