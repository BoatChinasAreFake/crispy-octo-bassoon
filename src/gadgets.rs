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
//!   Its contents are kept where the world lives (like a Hollow Box's); a
//!   joined player's host packs and unpacks theirs, checking what they own.

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
    /// What's in the Bundle carrying `wear` (in the order it went in). A
    /// joined player sees the host's word on it (`Msg::BundleState`).
    pub fn bundle_contents(&self, wear: crate::inventory::Wear) -> Vec<(Id, u8)> {
        let tag = box_id(wear);
        if self.is_client() {
            return self.bundle_mirror.get(&tag).cloned().unwrap_or_default();
        }
        self.boxes.get(&tag).map(|c| c.contents().into_iter().map(|(i, n, _)| (i, n)).collect()).unwrap_or_default()
    }

    pub fn bundle_fill(&self, wear: crate::inventory::Wear) -> u32 {
        self.bundle_contents(wear).iter().map(|&(i, n)| weight(i, n)).sum()
    }

    /// Where the world lives: put up to `n` of `item` in the Bundle tagged
    /// `tag` (0: an empty one). Returns (its tag now, how many fit).
    pub fn bundle_put(&mut self, tag: u16, item: Id, n: u8) -> (u16, u8) {
        if !bundleable(item) || n == 0 {
            return (tag, 0);
        }
        let room = BUNDLE_ROOM.saturating_sub(self.bundle_fill(box_wear(tag)));
        let fit = (room / (BUNDLE_ROOM / max_stack(item) as u32)).min(n as u32) as u8;
        if fit == 0 {
            return (tag, 0);
        }
        let id = match tag {
            0 => (1..=u16::MAX).find(|k| !self.boxes.contains_key(k)).unwrap_or(1),
            id => id,
        };
        let c = self.boxes.entry(id).or_insert_with(|| Container::for_block(CHEST));
        match c.slots.iter_mut().find(|s| s.is_some_and(|(it, k)| it == item && k < max_stack(item))) {
            Some(Some((_, k))) if (*k as u32 + fit as u32) <= max_stack(item) as u32 => *k += fit,
            _ => {
                let Some(free) = c.slots.iter_mut().find(|s| s.is_none()) else { return (tag, 0) };
                *free = Some((item, fit));
            }
        }
        (id, fit)
    }

    /// Where the world lives: take the last stack out of the Bundle tagged
    /// `tag`. Returns it, and the Bundle's tag now (0 once it's empty).
    pub fn bundle_take(&mut self, tag: u16) -> Option<((Id, u8), u16)> {
        let c = self.boxes.get_mut(&tag)?;
        let last = c.slots.iter_mut().rev().find(|s| s.is_some())?.take()?;
        if c.contents().is_empty() {
            self.boxes.remove(&tag);
            return Some((last, 0));
        }
        Some((last, tag))
    }

    /// Clicked inventory slot `i`: if it's a Bundle, put the cursor's stack
    /// in (left) or take the last one out (right). True if it did anything.
    pub fn bundle_click(&mut self, i: usize, left: bool, right: bool) -> bool {
        if self.inv.slots[i].map(|s| s.0) != Some(BUNDLE) || !(left || right) {
            return false;
        }
        let tag = box_id(self.inv.wear[i]);
        if left && let Some((item, n)) = self.inv.cursor {
            if !bundleable(item) {
                return false;
            }
            if self.is_client() {
                // The host packs it (and hands back whatever doesn't fit).
                self.inv.cursor = None;
                self.bundle_pending = Some(i);
                self.net_send_msg(crate::net::Msg::BundleUse { tag, item, n, put: true });
                self.sfx(Sfx::Place(Mat::Wood), None);
                return true;
            }
            let (new, fit) = self.bundle_put(tag, item, n);
            if fit == 0 {
                self.msg("The Bundle is full.");
                return true;
            }
            self.inv.wear[i] = box_wear(new);
            self.inv.cursor = if fit < n { Some((item, n - fit)) } else { None };
            self.sfx(Sfx::Place(Mat::Wood), None);
            return true;
        }
        if right && self.inv.cursor.is_none() {
            if tag == 0 {
                return true;
            }
            if self.is_client() {
                // The host hands it over (into the inventory).
                self.bundle_pending = Some(i);
                self.net_send_msg(crate::net::Msg::BundleUse { tag, item: AIR, n: 0, put: false });
                self.sfx(Sfx::Place(Mat::Wood), None);
                return true;
            }
            let Some((stack, new)) = self.bundle_take(tag) else { return true };
            self.inv.cursor = Some(stack);
            self.inv.cursor_wear = 0;
            self.inv.wear[i] = box_wear(new);
            self.sfx(Sfx::Place(Mat::Wood), None);
            return true;
        }
        false
    }

    /// Used the held Bundle in the world: everything tumbles out in front.
    pub fn tip_bundle(&mut self) {
        let i = self.inv.selected;
        let tag = box_id(self.inv.wear[i]);
        self.player.swing = 1.0;
        // (A joined player's host tips it out; see `host_bundle_tip`.)
        if self.is_client() || tag == 0 {
            return;
        }
        let Some(c) = self.boxes.remove(&tag) else { return };
        self.inv.wear[i] = 0;
        let at = self.player.eye() + self.player.look_dir() * 0.8;
        for (item, n, _) in c.contents() {
            self.pop_drop(at, item, n);
        }
    }

    /// A joined player packed or unpacked a Bundle: check it against what they own.
    pub fn host_bundle_use(&mut self, from: u32, tag: u16, item: Id, n: u8, put: bool) {
        // (No rate limit: every step is checked against what they own.)
        let owns = |g: &Game| g.peer_free(from) || g.peers.get(&from).is_some_and(|p| if tag == 0 { p.ledger.bag.has(BUNDLE) } else { p.ledger.owns_enchanted(BUNDLE, tag) });
        if !owns(self) {
            return;
        }
        let new = if put {
            if !valid_item(item) || n == 0 || !self.peer_take(from, item, n as u32) {
                return;
            }
            let (new, fit) = self.bundle_put(tag, item, n);
            // What didn't fit goes back.
            if fit < n {
                self.give_peer(from, item, n - fit);
            }
            new
        } else {
            let Some(((item, k), new)) = self.bundle_take(tag) else { return };
            self.give_peer(from, item, k);
            new
        };
        self.retag_peer_bundle(from, tag, new);
    }

    /// A joined player used their held Bundle in the world: it's tipped out in front of them.
    pub fn host_bundle_tip(&mut self, from: u32) {
        if self.verified_held(from) != BUNDLE {
            return;
        }
        let tag = self.verified_ench(from);
        if tag == 0 || !self.peers.get(&from).is_some_and(|p| self.peer_free(from) || p.ledger.owns_enchanted(BUNDLE, tag)) {
            return;
        }
        let Some(c) = self.boxes.remove(&tag) else { return };
        let Some(p) = self.peers.get(&from) else { return };
        let at = p.target + Vec3::Y * crate::player::EYE + Vec3::new(p.yaw.sin(), 0.0, -p.yaw.cos()) * 0.8;
        for (item, n, _) in c.contents() {
            self.pop_drop(at, item, n);
        }
        self.retag_peer_bundle(from, tag, 0);
    }

    /// Tell a joined player (and their ledger) what a Bundle of theirs holds now.
    fn retag_peer_bundle(&mut self, from: u32, old: u16, new: u16) {
        if old != new
            && let Some(l) = self.peers.get_mut(&from).map(|p| &mut p.ledger)
        {
            if old != 0 {
                l.remove_enchanted(BUNDLE, old);
            }
            if new != 0 {
                l.add_enchanted(BUNDLE, new, 1);
            }
        }
        let contents = self.boxes.get(&new).map(|c| c.contents().into_iter().map(|(i, n, _)| (i, n)).collect()).unwrap_or_default();
        self.net_send_to(from, crate::net::Msg::BundleState { old, new, contents });
    }

    /// The host's word on one of our Bundles.
    pub fn bundle_state(&mut self, old: u16, new: u16, contents: Vec<(Id, u8)>) {
        // The one we just used, else any carrying the old tag.
        let pending = self.bundle_pending.take().filter(|&i| self.inv.slots.get(i).is_some_and(|s| s.is_some_and(|s| s.0 == BUNDLE) ) && box_id(self.inv.wear[i]) == old);
        let slot = pending.or_else(|| (0..self.inv.slots.len()).find(|&i| self.inv.slots[i].is_some_and(|s| s.0 == BUNDLE) && box_id(self.inv.wear[i]) == old));
        if let Some(i) = slot {
            self.inv.wear[i] = box_wear(new);
        }
        self.bundle_mirror.remove(&old);
        if new != 0 {
            self.bundle_mirror.insert(new, contents);
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
