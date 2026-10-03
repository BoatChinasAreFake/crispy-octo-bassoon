//! Three handy things:
//!
//! - The **Spyglass** (glass and two copper): hold right-click to look
//!   through it, zoomed right in.
//! - The **Lodestone** (eight stone bricks round an iron): use a compass on
//!   one and your compass points to it instead of home, until it's broken.
//! - The **Bundle** (two string and some wool): holds a mix of small stacks,
//!   up to 64 items' worth (things that stack to 16 take four times the room;
//!   tools and other bundles don't go in). In the inventory, left-click a
//!   Bundle while carrying a stack to put it in; right-click it with nothing
//!   to take the last thing out. Use it in the world to tip it all out.
//!   Its contents are kept where the world lives (like a Hollow Box's), so
//!   bundles are packed by the host and in single player.

use crate::block::*;
use crate::boxes::{box_id, box_wear};
use crate::containers::Container;
use crate::game::Game;
use crate::sound::{Mat, Sfx};
use macroquad::math::{IVec3, Vec3};

/// How much a Bundle holds.
pub const BUNDLE_ROOM: u32 = 64;
/// Field of view through a Spyglass (degrees).
pub const SPYGLASS_FOV: f32 = 14.0;

/// How much room `n` of `item` take in a Bundle.
pub fn weight(item: Id, n: u8) -> u32 {
    (BUNDLE_ROOM / max_stack(item) as u32) * n as u32
}

/// Can it go in a Bundle at all?
pub fn bundleable(item: Id) -> bool {
    item != BUNDLE && item != HOLLOW_BOX && !crate::inventory::keeps_wear(item)
}

impl Game {
    /// What's in the Bundle carrying `wear` (in the order it went in).
    pub fn bundle_contents(&self, wear: crate::inventory::Wear) -> Vec<(Id, u8)> {
        self.boxes.get(&box_id(wear)).map(|c| c.contents().into_iter().map(|(i, n, _)| (i, n)).collect()).unwrap_or_default()
    }

    pub fn bundle_fill(&self, wear: crate::inventory::Wear) -> u32 {
        self.bundle_contents(wear).iter().map(|&(i, n)| weight(i, n)).sum()
    }

    /// Clicked inventory slot `i`: if it's a Bundle, put the cursor's stack
    /// in (left) or take the last one out (right). True if it did anything.
    pub fn bundle_click(&mut self, i: usize, left: bool, right: bool) -> bool {
        if self.inv.slots[i].map(|s| s.0) != Some(BUNDLE) || !(left || right) {
            return false;
        }
        if self.is_client() {
            if left && self.inv.cursor.is_some() || right && self.inv.cursor.is_none() {
                self.msg("Bundles are packed where the world lives (in single player, or by the host).");
                return true;
            }
            return false;
        }
        let wear = self.inv.wear[i];
        if left && let Some((item, n)) = self.inv.cursor {
            if !bundleable(item) {
                return false;
            }
            let room = BUNDLE_ROOM.saturating_sub(self.bundle_fill(wear));
            let fit = (room / (BUNDLE_ROOM / max_stack(item) as u32)).min(n as u32) as u8;
            if fit == 0 {
                self.msg("The Bundle is full.");
                return true;
            }
            let id = match box_id(wear) {
                0 => (1..=u16::MAX).find(|k| !self.boxes.contains_key(k)).unwrap_or(1),
                id => id,
            };
            let c = self.boxes.entry(id).or_insert_with(|| Container::for_block(CHEST));
            match c.slots.iter_mut().find(|s| s.is_some_and(|(it, k)| it == item && k < max_stack(item))) {
                Some(Some((_, k))) if (*k as u32 + fit as u32) <= max_stack(item) as u32 => *k += fit,
                _ => {
                    let Some(free) = c.slots.iter_mut().find(|s| s.is_none()) else { return true };
                    *free = Some((item, fit));
                }
            }
            self.inv.wear[i] = box_wear(id);
            self.inv.cursor = if fit < n { Some((item, n - fit)) } else { None };
            self.sfx(Sfx::Place(Mat::Wood), None);
            return true;
        }
        if right && self.inv.cursor.is_none() {
            let id = box_id(wear);
            let Some(c) = self.boxes.get_mut(&id) else { return true };
            let Some(last) = c.slots.iter_mut().rev().find(|s| s.is_some()) else { return true };
            self.inv.cursor = last.take();
            self.inv.cursor_wear = 0;
            if c.contents().is_empty() {
                self.boxes.remove(&id);
                self.inv.wear[i] = 0;
            }
            self.sfx(Sfx::Place(Mat::Wood), None);
            return true;
        }
        false
    }

    /// Used the held Bundle in the world: everything tumbles out in front.
    pub fn tip_bundle(&mut self) {
        let i = self.inv.selected;
        let id = box_id(self.inv.wear[i]);
        self.player.swing = 1.0;
        if self.is_client() || id == 0 {
            return;
        }
        let Some(c) = self.boxes.remove(&id) else { return };
        self.inv.wear[i] = 0;
        let at = self.player.eye() + self.player.look_dir() * 0.8;
        for (item, n, _) in c.contents() {
            self.pop_drop(at, item, n);
        }
    }

    /// Used a compass on a Lodestone: it points there from now on.
    pub fn link_lodestone(&mut self, p: IVec3) {
        self.lodestone = Some(p);
        self.player.swing = 1.0;
        self.sfx(Sfx::Chime, Some(p.as_vec3() + Vec3::splat(0.5)));
        self.msg("The compass needle swings round to the Lodestone.");
        self.advance("homing_in");
    }

    /// Where the compass points: the linked Lodestone (if it's still there), or home.
    /// `None` means it's spinning: the Lodestone is gone.
    pub fn compass_target(&self) -> Option<Vec3> {
        match self.lodestone {
            None => Some(self.spawn),
            Some(p) => {
                let loaded = self.world.is_loaded(p.x, p.z);
                // (Too far to know: point anyway; there and broken: spin.)
                (!loaded || self.world.get_v(p) == LODESTONE).then(|| p.as_vec3() + Vec3::splat(0.5))
            }
        }
    }

    /// Looking through a Spyglass right now?
    pub fn spyglassing(&self) -> bool {
        self.spyglass && self.inv.held() == SPYGLASS && !self.menu
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::tests::arena;

    #[test]
    fn bundles_hold_mixed_small_stacks() {
        let mut g = arena(51);
        g.inv.slots[5] = Some((BUNDLE, 1));
        g.inv.cursor = Some((COBBLE, 40));
        assert!(g.bundle_click(5, true, false));
        assert_eq!(g.inv.cursor, None);
        g.inv.cursor = Some((PEARL, 16));
        assert!(g.bundle_click(5, true, false));
        // Pearls stack to 16: four room each, so only 6 fit after 40 cobble.
        assert_eq!(g.inv.cursor, Some((PEARL, 10)));
        let w = g.inv.wear[5];
        assert_eq!(g.bundle_fill(w), 64);
        assert_eq!(g.bundle_contents(w), vec![(COBBLE, 40), (PEARL, 6)]);
        // Tools don't go in.
        g.inv.cursor = Some((PICK_IRON, 1));
        assert!(!g.bundle_click(5, true, false));
        // Take the last thing out, then the rest.
        g.inv.cursor = None;
        assert!(g.bundle_click(5, false, true));
        assert_eq!(g.inv.cursor, Some((PEARL, 6)));
        g.inv.cursor = None;
        g.bundle_click(5, false, true);
        assert_eq!(g.inv.cursor, Some((COBBLE, 40)));
        assert_eq!(g.inv.wear[5], 0, "empty again");
        // Tipped out in the world.
        g.inv.cursor = Some((COBBLE, 3));
        g.bundle_click(5, true, false);
        g.inv.selected = 5;
        g.tip_bundle();
        assert!(g.drops.iter().any(|d| d.item == COBBLE && d.n == 3));
        assert_eq!(g.inv.wear[5], 0);
    }

    #[test]
    fn compasses_follow_a_lodestone_until_it_breaks() {
        let mut g = arena(52);
        assert_eq!(g.compass_target(), Some(g.spawn));
        let p = g.player.body.pos.floor().as_ivec3() + IVec3::new(3, 0, 0);
        g.world.set_v(p, LODESTONE);
        g.link_lodestone(p);
        assert_eq!(g.compass_target(), Some(p.as_vec3() + Vec3::splat(0.5)));
        g.world.set_v(p, AIR);
        assert_eq!(g.compass_target(), None, "spinning");
    }
}
