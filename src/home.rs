//! Things for a home: **Campfires**, **Smokers** and **Blast Furnaces**,
//! **Barrels**, **Armour Stands** and **Paintings**.
//!
//! A Campfire cooks food lying on it: right-click with something raw to put
//! one on (or just drop it there), and after `CAMPFIRE_SECS` it's done and
//! can be picked up. It needs no fuel, smokes, lights the place up, and
//! burns anyone who stands in it. Cooking happens where the world lives, on
//! the dropped items themselves, so everyone sees the same thing.
//!
//! Smokers and Blast Furnaces are furnaces that work twice as fast (and burn
//! their fuel twice as fast): a Smoker only takes food, a Blast Furnace
//! anything else. Barrels are chests with a lid on top. Armour Stands hold a
//! set of armour (in a little four-slot container, helmet first) and wear
//! it; everyone nearby is told when that changes. Paintings hang on walls
//! like item frames, and the picture depends on where one hangs.

use crate::block::*;
use crate::containers::{smelt, Container};
use crate::drops::ItemDrop;
use crate::game::Game;
use crate::net::Msg;
use crate::render::{DynGeo, Pass};
use crate::sound::{Mat, Sfx};
use macroquad::math::{ivec3, IVec3, Mat4, Vec3};

/// Seconds for a Campfire to cook something.
pub const CAMPFIRE_SECS: f32 = 15.0;
/// At most this many things cook on one Campfire at once.
pub const ON_FIRE: usize = 4;
/// An Armour Stand's slots: helmet, chestplate, leggings, boots.
pub const STAND_SLOTS: usize = 4;
/// How far round us Campfires smoke (and how often it's looked for).
const SMOKE_RANGE: i32 = 16;
const SMOKE_EVERY: f32 = 0.12;

pub fn is_painting(id: Id) -> bool {
    (PAINTING_FIRST..PAINTING_FIRST + 4).contains(&id)
}

pub fn is_stand(id: Id) -> bool {
    (ARMOUR_STAND_FIRST..ARMOUR_STAND_FIRST + 4).contains(&id)
}

/// Which picture a painting at `p` shows (the same everywhere it's drawn).
pub fn painting_tile(x: i32, y: i32, z: i32) -> u16 {
    let h = (x as u32).wrapping_mul(73_856_093) ^ (y as u32).wrapping_mul(19_349_663) ^ (z as u32).wrapping_mul(83_492_791);
    crate::texture::T_PAINTING_FIRST + (h.wrapping_mul(2_654_435_761) >> 16) as u16 % crate::texture::PAINTINGS
}

pub fn is_food(id: Id) -> bool {
    food_value(id).is_some()
}

/// What a Campfire turns `item` into (food only).
pub fn campfire_cooks(item: Id) -> Option<Id> {
    smelt(item).filter(|r| is_food(*r))
}

/// Can `item` go in to cook in a furnace of kind `kind`?
pub fn cooks_in(kind: Id, item: Id) -> bool {
    match kind {
        SMOKER | SMOKER_LIT => campfire_cooks(item).is_some(),
        BLAST_FURNACE | BLAST_FURNACE_LIT => smelt(item).is_some_and(|r| !is_food(r)),
        _ => true,
    }
}

/// How much faster than a Furnace this one works.
pub fn furnace_speed(kind: Id) -> f32 {
    if matches!(kind, SMOKER | SMOKER_LIT | BLAST_FURNACE | BLAST_FURNACE_LIT) {
        2.0
    } else {
        1.0
    }
}

/// A furnace of this kind, lit or not.
pub fn furnace_lit(kind: Id, lit: bool) -> Id {
    let base = match kind {
        SMOKER | SMOKER_LIT => SMOKER,
        BLAST_FURNACE | BLAST_FURNACE_LIT => BLAST_FURNACE,
        _ => FURNACE,
    };
    if lit {
        base + 1
    } else {
        base
    }
}

/// How the armour on a stand looks (as `Inventory::armor_look`), and its trims.
pub fn stand_look(c: &Container) -> (u16, u32) {
    let slots: Vec<Option<(Id, u8)>> = c.slots.iter().take(STAND_SLOTS).copied().collect();
    let look = slots.iter().enumerate().map(|(i, s)| s.and_then(|(id, _)| armor_of(id)).map(|(_, t)| (t as u16 + 1) << (i * 4)).unwrap_or(0)).sum();
    (look, crate::trims::look(&slots, &c.wear[..slots.len()]))
}

/// The cell a dropped item is lying in, if that's on a Campfire.
fn on_campfire(world: &crate::world::World, d: &ItemDrop) -> Option<IVec3> {
    let p = (d.body.pos + Vec3::Y * 0.05).floor().as_ivec3();
    [p, p - IVec3::Y].into_iter().find(|&c| world.get_v(c) == CAMPFIRE)
}

impl Game {
    /// Right-clicked a Campfire: put one of what we're holding on to cook (true if it was cookable).
    pub fn use_campfire(&mut self, pos: IVec3) -> bool {
        let held = self.inv.held();
        if self.world.get_v(pos) != CAMPFIRE || campfire_cooks(held).is_none() {
            return false;
        }
        if self.is_client() {
            if !self.creative {
                self.inv.consume_held();
            }
            self.net_send_msg(Msg::CampfirePut { x: pos.x, y: pos.y, z: pos.z, item: held });
        } else if self.put_on_campfire(pos, held) && !self.creative {
            self.inv.consume_held();
        }
        self.player.swing = 1.0;
        true
    }

    /// The host: a joined player put something on a Campfire (they must have it, and be near).
    pub fn host_campfire_put(&mut self, from: u32, pos: IVec3, item: Id) {
        let near = self.peers.get(&from).is_some_and(|p| p.target.distance(pos.as_vec3()) < 8.0);
        if !near || !valid_item(item) || campfire_cooks(item).is_none() || self.world.get_v(pos) != CAMPFIRE || self.cooking_on(pos) >= ON_FIRE || !self.peer_take(from, item, 1) {
            return;
        }
        self.put_on_campfire(pos, item);
    }

    /// How many things are cooking on the Campfire at `pos`.
    fn cooking_on(&self, pos: IVec3) -> usize {
        self.drops.iter().filter(|d| on_campfire(&self.world, d) == Some(pos) && campfire_cooks(d.item).is_some()).map(|d| d.n as usize).sum()
    }

    /// Where the world lives: lay one `item` on the Campfire (false if it's full).
    pub fn put_on_campfire(&mut self, pos: IVec3, item: Id) -> bool {
        if self.cooking_on(pos) >= ON_FIRE {
            return false;
        }
        let k = self.cooking_on(pos) as f32;
        let at = pos.as_vec3() + Vec3::new(0.3 + 0.4 * (k % 2.0), 0.5, 0.3 + 0.4 * (k / 2.0).floor());
        // It stays put until it's cooked.
        self.spawn_drop(at, item, 1, 0, Vec3::ZERO, CAMPFIRE_SECS + 1.0);
        self.sfx(Sfx::Place(Mat::Wood), Some(at));
        true
    }

    /// Where the world lives: food lying on a Campfire cooks.
    pub fn campfires_tick(&mut self, dt: f32) {
        let mut done = Vec::new();
        for i in 0..self.drops.len() {
            let Some(result) = campfire_cooks(self.drops[i].item) else { continue };
            if on_campfire(&self.world, &self.drops[i]).is_none() {
                self.drops[i].cooking = 0.0;
                continue;
            }
            let d = &mut self.drops[i];
            d.cooking += dt;
            // It isn't going anywhere while it cooks.
            d.delay = d.delay.max(d.age + 0.5);
            if d.cooking >= CAMPFIRE_SECS {
                d.cooking = 0.0;
                d.item = result;
                d.delay = d.age;
                done.push(d.body.pos);
            }
        }
        for p in done {
            self.sfx(Sfx::Hiss, Some(p));
            self.smoke(p + Vec3::Y * 0.2, 3, 0.1);
        }
    }

    /// Everyone: smoke drifting up from Campfires near us.
    pub fn campfire_smoke(&mut self, dt: f32) {
        if self.dedicated || self.menu {
            return;
        }
        self.smoke_acc += dt;
        if self.smoke_acc < SMOKE_EVERY {
            return;
        }
        self.smoke_acc = 0.0;
        let me = self.player.body.pos.floor().as_ivec3();
        for _ in 0..40 {
            let p = me + ivec3(self.rng.int(-SMOKE_RANGE, SMOKE_RANGE), self.rng.int(-6, 6), self.rng.int(-SMOKE_RANGE, SMOKE_RANGE));
            if self.world.get_v(p) != CAMPFIRE {
                continue;
            }
            let r = &mut self.rng;
            let at = p.as_vec3() + Vec3::new(r.range(0.3, 0.7), 0.5, r.range(0.3, 0.7));
            let vel = Vec3::new(r.range(-0.15, 0.15), r.range(0.8, 1.4), r.range(-0.15, 0.15));
            self.particles.push(crate::entity::Particle { pos: at, vel, life: r.range(2.5, 4.0), tile: crate::texture::T_CLOUD, uv: [0.0, 0.0], size: r.range(0.25, 0.45), gravity: -0.15 });
            if self.rng.chance(0.4) {
                let r = &mut self.rng;
                let spark = Vec3::new(r.range(-0.2, 0.2), r.range(1.0, 2.0), r.range(-0.2, 0.2));
                self.particles.push(crate::entity::Particle { pos: at, vel: spark, life: r.range(0.4, 0.8), tile: crate::texture::T_SPARK_FIRST + 1, uv: [0.25, 0.25], size: 0.05, gravity: 0.5 });
            }
        }
    }

    /// The armour on every Armour Stand nearby.
    pub fn draw_stands(&self, g: &mut DynGeo, eye: Vec3, range: f32) {
        g.begin(Pass::Opaque, [1.0; 4], false);
        for (&pos, c) in &self.world.containers {
            let id = self.world.get_v(pos);
            if !is_stand(id) || pos.as_vec3().distance(eye) > range {
                continue;
            }
            let (look, trims) = stand_look(c);
            if look == 0 {
                continue;
            }
            let facing = (id - ARMOUR_STAND_FIRST) as f32;
            let sky = self.world.sky_shade(pos.x, pos.y + 1, pos.z).max(0.3);
            let root = Mat4::from_translation(pos.as_vec3() + Vec3::new(0.5, 0.0, 0.5)) * Mat4::from_rotation_y(-facing * std::f32::consts::FRAC_PI_2);
            crate::entity::draw_armor(g, &root, look, trims, 0.0, sky, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::containers::{accepts, FUEL, INPUT, OUTPUT};

    #[test]
    fn smokers_take_food_and_blast_furnaces_everything_else_twice_as_fast() {
        assert!(accepts(SMOKER, INPUT, PORKCHOP) && !accepts(SMOKER, INPUT, SAND));
        assert!(accepts(BLAST_FURNACE, INPUT, COBBLE) && !accepts(BLAST_FURNACE, INPUT, COD));
        assert!(accepts(FURNACE, INPUT, COD) && accepts(FURNACE, INPUT, SAND));
        assert!(accepts(SMOKER, FUEL, COAL));
        let mut g = crate::game::tests::arena(131);
        let (f, s) = (ivec3(0, 50, 0), ivec3(2, 50, 0));
        g.world.set_v(f, FURNACE);
        g.world.set_v(s, SMOKER);
        for p in [f, s] {
            g.ensure_container(p);
            let c = g.world.containers.get_mut(&p).unwrap();
            c.slots[INPUT] = Some((PORKCHOP, 4));
            c.slots[FUEL] = Some((COAL, 1));
        }
        for _ in 0..(crate::containers::COOK_SECS * 2.0 * 10.0) as i32 + 5 {
            g.container_tick(0.1);
        }
        let out = |g: &Game, p| g.world.containers[&p].slots[OUTPUT].map(|s| s.1).unwrap_or(0);
        assert_eq!(g.world.get_v(s), SMOKER_LIT, "it glows while it works");
        assert!(out(&g, s) >= 2 * out(&g, f).max(1) - 1 && out(&g, s) >= 3, "smoker {} vs furnace {}", out(&g, s), out(&g, f));
        assert_eq!(g.world.containers[&s].slots[OUTPUT].unwrap().0, COOKED_CHOP);
    }

    #[test]
    fn a_campfire_cooks_food_put_on_it_and_burns_feet() {
        let mut g = crate::game::tests::arena(132);
        let fire = ivec3(3, 50, 0);
        g.world.set_v(fire, CAMPFIRE);
        g.player.body.pos = Vec3::new(0.5, 50.0, 0.5);
        g.inv.slots[0] = Some((COD, 6));
        g.inv.slots[1] = Some((SAND, 1));
        g.inv.selected = 1;
        assert!(!g.use_campfire(fire), "sand doesn't cook on a campfire");
        g.inv.selected = 0;
        for _ in 0..6 {
            g.use_campfire(fire);
        }
        assert_eq!(g.inv.slots[0], Some((COD, 2)), "four fit at once");
        for _ in 0..((CAMPFIRE_SECS + 2.0) * 20.0) as i32 {
            g.drops_tick(0.05);
            g.campfires_tick(0.05);
        }
        let cooked: u8 = g.drops.iter().filter(|d| d.item == COOKED_COD).map(|d| d.n).sum();
        assert_eq!(cooked, 4, "all cooked: {:?}", g.drops.iter().map(|d| (d.item, d.n, d.body.pos)).collect::<Vec<_>>());
        assert!(crate::fire::touches_fire(&g.world, Vec3::new(3.2, 50.0, 0.2), Vec3::new(3.8, 51.8, 0.8)), "standing in it burns");
    }

    #[test]
    fn armour_stands_hold_only_the_right_armour_in_each_slot() {
        let helmet = ARMOR_FIRST + 4; // iron helmet
        assert!(accepts(ARMOUR_STAND_FIRST, 0, helmet) && !accepts(ARMOUR_STAND_FIRST, 1, helmet));
        assert!(!accepts(ARMOUR_STAND_FIRST + 2, 0, DIAMOND));
        assert!(accepts(ARMOUR_STAND_FIRST, 0, TURTLE_SHELL));
        let mut c = Container::for_block(ARMOUR_STAND_FIRST + 1);
        assert_eq!(c.slots.len(), STAND_SLOTS);
        c.slots[0] = Some((helmet, 1));
        assert_eq!(stand_look(&c).0, 2, "an iron helmet");
        assert!(crate::containers::is_container(ARMOUR_STAND_FIRST + 3) && crate::containers::is_container(BARREL));
    }

    #[test]
    fn paintings_pick_a_picture_by_where_they_hang() {
        let tiles: std::collections::HashSet<u16> = (0..40).map(|i| painting_tile(i * 3, 60, -i)).collect();
        assert!(tiles.len() >= 5, "a good mix: {tiles:?}");
        assert!(tiles.iter().all(|t| (crate::texture::T_PAINTING_FIRST..crate::texture::T_PAINTING_FIRST + crate::texture::PAINTINGS).contains(t)));
        assert_eq!(painting_tile(5, 6, 7), painting_tile(5, 6, 7));
        assert_eq!(placing_item(PAINTING_FIRST + 2), Some(PAINTING_FIRST));
        assert_eq!(placing_item(SMOKER_LIT), Some(SMOKER));
    }
}
