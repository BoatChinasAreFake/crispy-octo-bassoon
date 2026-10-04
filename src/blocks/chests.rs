//! Chests: **tiers** and a **lid** that opens.
//!
//! - A Chest (27 slots) becomes an **Iron Chest** (45), a **Gold Chest** (54)
//!   and a **Diamond Chest** (72): sneak and right-click one holding 8 Iron, 8
//!   Gold Ingots or 4 Diamonds and it's upgraded where it stands, contents and
//!   all. (Copper Chests upgrade to Iron too.) The bigger chests can be
//!   crafted outright as well.
//! - Opening a chest swings its lid up on its hinge, away from you; it stays
//!   open while the chest's screen is, and swings shut when you leave it.
//!   The chunk mesher leaves out the lid of a chest that's open (see
//!   `lid_lifted`) and this draws it at its angle instead.

use crate::block::*;
use crate::game::Game;
use crate::net::Msg;
use crate::render::{DynGeo, Pass};
use crate::sound::{Mat, Sfx};
use macroquad::math::{IVec3, Mat4, Vec3};
use std::collections::HashSet;
use std::sync::{LazyLock, RwLock};

/// Any chest a lid swings on.
pub fn is_chest(id: Id) -> bool {
    matches!(id, CHEST | COPPER_CHEST | IRON_CHEST | GOLD_CHEST | DIAMOND_CHEST)
}

/// How many slots a chest of this kind has.
pub fn slots(id: Id) -> usize {
    match id {
        IRON_CHEST => 45,
        GOLD_CHEST => 54,
        DIAMOND_CHEST => 72,
        _ => crate::containers::CHEST_SLOTS,
    }
}

/// The most any chest holds (for reading saves).
pub const MAX_SLOTS: usize = 72;

/// Columns of slots its screen shows (the biggest is wider than it is tall).
pub fn columns(n: usize) -> usize {
    if n > 54 { 12 } else { 9 }
}

/// What upgrades a chest of kind `id`: (the material, how many, what it becomes).
pub fn upgrade(id: Id) -> Option<(Id, u8, Id)> {
    match id {
        CHEST | COPPER_CHEST => Some((IRON, 8, IRON_CHEST)),
        IRON_CHEST => Some((GOLD_INGOT, 8, GOLD_CHEST)),
        GOLD_CHEST => Some((DIAMOND, 4, DIAMOND_CHEST)),
        _ => None,
    }
}

/// Chests whose lids are up (so the mesher leaves them out).
static LIFTED: LazyLock<RwLock<HashSet<IVec3>>> = LazyLock::new(|| RwLock::new(HashSet::new()));

pub fn lid_lifted(p: IVec3) -> bool {
    LIFTED.read().map(|s| !s.is_empty() && s.contains(&p)).unwrap_or(false)
}

pub(crate) fn set_lifted(p: IVec3, up: bool) {
    if let Ok(mut s) = LIFTED.write() {
        if up {
            s.insert(p);
        } else {
            s.remove(&p);
        }
    }
}

/// A lid on the move: how far open (0 shut, 1 open) and which way the chest
/// faces (the latch side, radians about y; the hinge is opposite).
#[derive(Clone, Copy, Debug)]
pub struct Lid {
    pub open: f32,
    pub yaw: f32,
}

/// The lid box (the body is the rest of `Shape::Chest`).
const LID_MIN: [f32; 3] = [1.0 / 16.0, 10.0 / 16.0, 1.0 / 16.0];
const LID_MAX: [f32; 3] = [15.0 / 16.0, 14.0 / 16.0, 15.0 / 16.0];
/// How fast a lid swings (fractions of its way a second), and how far it goes.
const SWING: f32 = 4.0;
const WIDE: f32 = 1.75;

impl Game {
    /// Sneak-right-clicked a chest: upgrade it if we're holding the right
    /// stuff. True if that's what happened (or was asked of the host).
    pub fn upgrade_chest(&mut self, pos: IVec3) -> bool {
        let id = self.world.get_v(pos);
        let Some((material, n, to)) = upgrade(id) else { return false };
        let held = self.inv.slots[self.inv.selected];
        if held.map(|(i, _)| i) != Some(material) {
            return false;
        }
        if !self.creative && held.is_none_or(|(_, have)| have < n) {
            self.msg(format!("Upgrading this takes {n} {}.", item_name(material)));
            return true;
        }
        if self.is_client() {
            self.net_send_msg(Msg::ChestUpgrade { x: pos.x, y: pos.y, z: pos.z });
        } else {
            self.apply_upgrade(pos, to);
        }
        if !self.creative {
            for _ in 0..n {
                self.inv.consume_held();
            }
        }
        self.player.swing = 1.0;
        self.msg(format!("It's a {} now.", block(to).name));
        true
    }

    /// Where the world lives: turn the chest at `pos` into `to`, keeping what's in it.
    fn apply_upgrade(&mut self, pos: IVec3, to: Id) {
        let kept = self.world.containers.remove(&pos);
        self.world.set_v(pos, to);
        if let Some(mut c) = kept {
            let n = slots(to);
            c.slots.resize(n, None);
            c.wear.resize(n, 0);
            self.world.containers.insert(pos, c);
        }
        self.dirty_containers.insert(pos);
        self.sfx(Sfx::Place(Mat::Stone), Some(pos.as_vec3() + Vec3::splat(0.5)));
    }

    /// The host: a joined player wants the chest at `pos` upgraded (they must
    /// be near it and have the material).
    pub fn host_chest_upgrade(&mut self, from: u32, pos: IVec3) {
        let Some((material, n, to)) = upgrade(self.world.get_v(pos)) else { return };
        if !self.peer_near(from, pos) {
            return;
        }
        if self.peer_take(from, material, n as u32) {
            self.apply_upgrade(pos, to);
        }
    }

    /// Lids swing toward open while their chest's screen is up, shut otherwise.
    pub fn lids_tick(&mut self, dt: f32) {
        let open = self.open.filter(|p| is_chest(self.world.get_v(*p)));
        if let Some(p) = open
            && !self.lids.contains_key(&p)
        {
            // Hinged on the far side, so it opens away from whoever opened it.
            let d = self.player.body.pos - (p.as_vec3() + Vec3::splat(0.5));
            let yaw = if d.x.abs() > d.z.abs() {
                if d.x > 0.0 { -std::f32::consts::FRAC_PI_2 } else { std::f32::consts::FRAC_PI_2 }
            } else if d.z > 0.0 {
                std::f32::consts::PI
            } else {
                0.0
            };
            self.lids.insert(p, Lid { open: 0.0, yaw });
            set_lifted(p, true);
            self.world.dirty.insert((p.x.div_euclid(16), p.z.div_euclid(16)));
        }
        let mut shut = Vec::new();
        for (p, lid) in self.lids.iter_mut() {
            let want = if Some(*p) == open { 1.0 } else { 0.0 };
            let before = lid.open;
            lid.open = (lid.open + (want - lid.open).signum() * dt * SWING).clamp(0.0, 1.0);
            if before > 0.0 && lid.open == 0.0 {
                shut.push(*p);
            }
        }
        for p in shut {
            self.lids.remove(&p);
            set_lifted(p, false);
            self.world.dirty.insert((p.x.div_euclid(16), p.z.div_euclid(16)));
            self.sfx(Sfx::Place(Mat::Wood), Some(p.as_vec3() + Vec3::splat(0.5)));
        }
        // A chest broken (or changed) while open: forget its lid.
        let gone: Vec<IVec3> = self.lids.keys().copied().filter(|p| !is_chest(self.world.get_v(*p))).collect();
        for p in gone {
            self.lids.remove(&p);
            set_lifted(p, false);
        }
    }

    /// Draw the lids that are lifted.
    pub fn draw_lids(&self, g: &mut DynGeo) {
        for (p, lid) in &self.lids {
            let id = self.world.get_v(*p);
            if !is_chest(id) {
                continue;
            }
            let sky = self.world.sky_shade(p.x, p.y + 1, p.z);
            let hinge = Vec3::new(0.0, LID_MIN[1], LID_MAX[2]);
            // Ease in and out.
            let k = lid.open * lid.open * (3.0 - 2.0 * lid.open);
            let m = Mat4::from_translation(p.as_vec3() + Vec3::new(0.5, 0.0, 0.5))
                * Mat4::from_rotation_y(lid.yaw)
                * Mat4::from_translation(Vec3::new(-0.5, 0.0, -0.5))
                * Mat4::from_translation(hinge)
                * Mat4::from_rotation_x(k * WIDE)
                * Mat4::from_translation(-hinge)
                * Mat4::from_translation(Vec3::from_array(LID_MIN))
                * Mat4::from_scale(Vec3::from_array(LID_MAX) - Vec3::from_array(LID_MIN));
            g.begin(Pass::Opaque, [1.0; 4], false);
            let tex = block(id).tex;
            for (f, (_, corners, shade)) in crate::mesher::FACES.iter().enumerate() {
                let c = corners.map(|q| m.transform_point3(Vec3::from_array(q)));
                // The sides show the top rows of the chest's side picture (the lid's part).
                let (tile, uv) = match f {
                    2 => (tex[0], [LID_MIN[0], LID_MIN[2], LID_MAX[0], LID_MAX[2]]),
                    3 => (tex[2], [LID_MIN[0], LID_MIN[2], LID_MAX[0], LID_MAX[2]]),
                    _ => (tex[1], [LID_MIN[0], 1.0 - LID_MAX[1], LID_MAX[0], 1.0 - LID_MIN[1]]),
                };
                g.quad(c, tile, uv, [*shade, sky]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use macroquad::math::ivec3;

    #[test]
    fn chests_upgrade_keeping_their_things_and_lids_swing() {
        let mut g = crate::game::tests::arena(241);
        let p = ivec3(2, 50, 2);
        g.world.set_v(p, CHEST);
        g.world.containers.get_mut(&p).unwrap().slots[5] = Some((COBBLE, 30));
        // Not enough iron.
        g.inv.slots[0] = Some((IRON, 3));
        g.inv.selected = 0;
        assert!(g.upgrade_chest(p));
        assert_eq!(g.world.get_v(p), CHEST);
        g.inv.slots[0] = Some((IRON, 10));
        assert!(g.upgrade_chest(p));
        assert_eq!(g.world.get_v(p), IRON_CHEST);
        let c = &g.world.containers[&p];
        assert_eq!((c.slots.len(), c.slots[5]), (45, Some((COBBLE, 30))));
        assert_eq!(g.inv.slots[0], Some((IRON, 2)));
        // On to gold and diamond.
        g.inv.slots[0] = Some((GOLD_INGOT, 8));
        assert!(g.upgrade_chest(p));
        g.inv.slots[0] = Some((DIAMOND, 4));
        assert!(g.upgrade_chest(p));
        assert_eq!(g.world.get_v(p), DIAMOND_CHEST);
        assert_eq!(g.world.containers[&p].slots.len(), 72);
        assert!(!g.upgrade_chest(p), "as good as it gets");
        assert!(crate::containers::accepts(DIAMOND_CHEST, 71, DIAMOND) && !crate::containers::accepts(DIAMOND_CHEST, 72, DIAMOND));
        // Saved and loaded with all its slots.
        let back = crate::containers::decode(&crate::containers::encode(&g.world.containers), 4);
        assert_eq!(back[&p].slots.len(), 72);
        // The lid lifts while it's open, then shuts.
        g.open = Some(p);
        g.lids_tick(0.05);
        assert!(lid_lifted(p));
        for _ in 0..20 {
            g.lids_tick(0.05);
        }
        assert_eq!(g.lids[&p].open, 1.0);
        g.open = None;
        for _ in 0..20 {
            g.lids_tick(0.05);
        }
        assert!(g.lids.is_empty() && !lid_lifted(p));
    }
}
