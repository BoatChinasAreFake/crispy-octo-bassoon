//! Slabs, stairs and doors: which way they face when placed, two slabs making
//! a block, and doors (two blocks tall) opening, closing and breaking together.
//! Their shapes live in block.rs (`Shape`); the mesher, physics and raycasts
//! use those boxes.

use crate::block::*;
use crate::game::Game;
use crate::sound::{Mat, Sfx};
use macroquad::math::{IVec3, Vec3};

impl Game {
    /// Which way the player is looking, as a block facing (0 north .. 3 west).
    pub fn facing(&self) -> u8 {
        let d = self.player.look_dir();
        if d.x.abs() > d.z.abs() {
            if d.x > 0.0 { 1 } else { 3 }
        } else if d.z > 0.0 {
            2
        } else {
            0
        }
    }

    /// Is someone (the player, a mob, another player) standing in this cell?
    pub fn cell_occupied(&self, p: IVec3) -> bool {
        self.player.body.intersects_block(p.x, p.y, p.z) || self.mobs.iter().any(|m| m.body.intersects_block(p.x, p.y, p.z)) || self.peers.values().any(|q| q.intersects_block(p))
    }

    /// The block to place for the held item: stairs face away from you, and a
    /// slab goes on top when you aim at the upper half of something.
    /// `hit_y` is how far up the aimed-at face was hit (0..1).
    pub fn oriented(&self, held: Id, normal: IVec3, hit_y: f32) -> Id {
        if let Some((family, _)) = stairs_of(held) {
            return family + self.facing() as Id;
        }
        if let Some((family, _)) = slab_of(held) {
            let top = normal == IVec3::NEG_Y || (normal.y == 0 && hit_y > 0.5);
            return family + top as Id;
        }
        // Signs face whoever put them up; frames hang on the wall they were put on.
        if held == SIGN_FIRST {
            return SIGN_FIRST + self.facing() as Id;
        }
        if let Some(id) = self.hinged_facing(held, normal) {
            return id;
        }
        // Pistons and dispensers face you; repeaters point away from you.
        match held {
            PISTON_FIRST => return crate::contraptions::piston(self.facing6(), false, false),
            STICKY_FIRST => return crate::contraptions::piston(self.facing6(), false, true),
            DISPENSER_FIRST => return DISPENSER_FIRST + self.facing6() as Id,
            CRAFTER_FIRST => return CRAFTER_FIRST + self.facing6() as Id,
            // An observer looks the way you're looking (at what you placed it against).
            OBSERVER_FIRST => return crate::contraptions::observer(crate::contraptions::facing_of(-crate::contraptions::dir6(self.facing6())), false),
            REPEATER_FIRST => return crate::contraptions::repeater(self.facing(), false),
            COMPARATOR_FIRST => return crate::contraptions::comparator(self.facing(), false, false),
            HOPPER_FIRST => return Game::hopper_facing(normal),
            _ => {}
        }
        // A bed's head goes the way you're looking (you climb in from the foot).
        if held == BED {
            return crate::beds::bed(self.facing());
        }
        // A lantern put on the underside of a block hangs from it.
        if held == LANTERN && normal == IVec3::NEG_Y {
            return LANTERN_HANGING;
        }
        if held == FRAME_FIRST || held == PAINTING_FIRST || held == TRIPWIRE_HOOK_FIRST {
            return held + crate::decor::frame_facing(normal).unwrap_or(0) as Id;
        }
        // An armour stand faces whoever put it down.
        if held == ARMOUR_STAND_FIRST {
            return held + ((self.facing() + 2) % 4) as Id;
        }
        held
    }

    /// A slab placed onto a matching slab's open side makes the full block.
    /// Returns whether that's what happened.
    pub fn try_merge_slab(&mut self, held: Id, hit: IVec3, hit_id: Id, normal: IVec3) -> bool {
        let (Some((family, false)), Some((hit_family, top))) = (slab_of(held), slab_of(hit_id)) else { return false };
        let full = made_of(held);
        if family != hit_family || full == AIR || !((normal == IVec3::Y && !top) || (normal == IVec3::NEG_Y && top)) {
            return false;
        }
        if self.cell_occupied(hit) {
            return true; // someone's in the way: don't place it beside instead either
        }
        self.world.set_v(hit, full);
        self.sfx(Sfx::Place(crate::sound::material(full)), Some(hit.as_vec3() + Vec3::splat(0.5)));
        self.player.swing = 1.0;
        if !self.creative {
            self.inv.consume_held();
        }
        true
    }

    /// Place a door (two blocks tall, on something solid) facing away from the player.
    pub fn place_door(&mut self, hit: IVec3, normal: IVec3, hit_id: Id) {
        let bottom = if replaceable(hit_id) { hit } else { hit + normal };
        let top = bottom + IVec3::Y;
        if top.y >= crate::world::CH || bottom.y < 1 {
            return;
        }
        if !replaceable(self.world.get_v(bottom)) || !replaceable(self.world.get_v(top)) || !is_solid(self.world.get_v(bottom - IVec3::Y)) {
            return;
        }
        if self.cell_occupied(bottom) || self.cell_occupied(top) {
            return;
        }
        if !self.is_client() {
            use rhai::INT;
            let args = vec![self.player_name.clone().into(), (bottom.x as INT).into(), (bottom.y as INT).into(), (bottom.z as INT).into(), "door".into()];
            if !self.fire("on_block_place", args) {
                return;
            }
        }
        let f = self.facing();
        self.world.set_v(bottom, door(f, false, false));
        self.world.set_v(top, door(f, false, true));
        self.sfx(Sfx::Place(Mat::Wood), Some(bottom.as_vec3() + Vec3::splat(0.5)));
        self.player.swing = 1.0;
        if !self.creative {
            self.inv.consume_held();
        }
    }

    /// Open or close the door at `pos` (both halves).
    pub fn toggle_door(&mut self, pos: IVec3) {
        let Some((f, open, top)) = door_state(self.world.get_v(pos)) else { return };
        let (bottom, upper) = if top { (pos - IVec3::Y, pos) } else { (pos, pos + IVec3::Y) };
        for (p, is_top) in [(bottom, false), (upper, true)] {
            if self.world.get_v(p) == door(f, open, is_top) {
                self.world.set_v(p, door(f, !open, is_top));
            }
        }
        self.sfx(Sfx::Place(Mat::Wood), Some(pos.as_vec3() + Vec3::splat(0.5)));
        self.player.swing = 1.0;
        if !open {
            self.advance("open_door_policy");
        }
    }

    /// Half a door was broken: the other half goes too (without a second drop).
    pub fn remove_door_partner(&mut self, pos: IVec3, old: Id) {
        let Some((f, _, top)) = door_state(old) else { return };
        let other = if top { pos - IVec3::Y } else { pos + IVec3::Y };
        if door_state(self.world.get_v(other)).is_some_and(|(of, _, otop)| of == f && otop != top) {
            self.world.set_v(other, AIR);
        }
    }
}
