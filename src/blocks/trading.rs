//! **The Trading Table.** Two players swap items without either having to
//! trust the other. The table holds 18 slots: the left nine are the first
//! seat's offer, the right nine the second's. Each player can only put
//! things into (and take things out of) their own side. When both press
//! Accept, the two sides change places, and each takes their new things out.
//!
//! - The first two players to open a table take its seats (by name, so a
//!   seat is kept if you step away with things still on your side). A seat
//!   is free again once its side is empty and its player has closed the table.
//! - Any change to either side takes back both Accepts, so nobody can slip
//!   something out after the other has said yes.
//! - Everything goes through the host and its ledger (see containers.rs), like
//!   any other container; hoppers and droppers leave a Trading Table alone.

use crate::block::*;
use crate::game::Game;
use crate::net::Msg;
use macroquad::math::IVec3;

pub const TRADE_SLOTS: usize = 18;
pub const HALF: usize = 9;

/// One table's seats and who's said yes (the host's, or a joined player's copy).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Deal {
    /// Who sits on each side ("": nobody).
    pub seats: [String; 2],
    pub accepted: [bool; 2],
    /// How many swaps this table has made (so a joined player can tell one just happened).
    pub done: u32,
    /// A joined player's own seat at this table, as the host told them.
    pub mine: Option<usize>,
}

/// Which side a slot is on.
pub fn side_of(slot: usize) -> usize {
    (slot / HALF).min(1)
}

/// Swap the two sides of a table's contents.
pub fn swap_sides(c: &mut crate::containers::Container) {
    for i in 0..HALF {
        c.slots.swap(i, i + HALF);
        c.wear.swap(i, i + HALF);
    }
}

impl Game {
    /// The seat `name` has at the table at `p`, if any.
    pub fn seat_at(&self, p: IVec3, name: &str) -> Option<usize> {
        let d = self.deals.get(&p)?;
        if self.is_client() {
            return d.mine;
        }
        d.seats.iter().position(|s| !s.is_empty() && s.eq_ignore_ascii_case(name))
    }

    fn side_empty(&self, p: IVec3, side: usize) -> bool {
        self.world.containers.get(&p).is_none_or(|c| c.slots[side * HALF..side * HALF + HALF].iter().all(Option::is_none))
    }

    /// Is the player named `name` looking at the table at `p`?
    fn viewing(&self, p: IVec3, name: &str) -> bool {
        (self.open == Some(p) && !self.away() && self.player_name.eq_ignore_ascii_case(name)) || self.viewers.get(&p).is_some_and(|v| v.iter().any(|&id| self.peer_name(id).eq_ignore_ascii_case(name)))
    }

    /// Sit `name` at the table at `p` if there's room (the host, or single player).
    pub fn take_seat(&mut self, p: IVec3, name: &str) -> Option<usize> {
        if let Some(s) = self.seat_at(p, name) {
            return Some(s);
        }
        let deal = self.deals.entry(p).or_default().clone();
        let free = (0..2).find(|&s| deal.seats[s].is_empty() || (self.side_empty(p, s) && !self.viewing(p, &deal.seats[s])))?;
        let d = self.deals.entry(p).or_default();
        d.seats[free] = name.to_string();
        d.accepted = [false; 2];
        Some(free)
    }

    /// `name` closed the table: their seat goes if their side is empty.
    pub fn leave_seat(&mut self, p: IVec3, name: &str) {
        if let Some(s) = self.seat_at(p, name)
            && self.side_empty(p, s)
            && let Some(d) = self.deals.get_mut(&p)
        {
            d.seats[s].clear();
            d.accepted = [false; 2];
        }
        self.send_deal(p);
    }

    /// Can the player named `name` touch slot `slot` of the table at `p`?
    pub fn may_touch(&self, p: IVec3, name: &str, slot: usize) -> bool {
        slot < TRADE_SLOTS && self.seat_at(p, name) == Some(side_of(slot))
    }

    /// Something on the table changed: both Accepts are taken back.
    pub fn deal_changed(&mut self, p: IVec3) {
        if let Some(d) = self.deals.get_mut(&p)
            && d.accepted != [false; 2]
        {
            d.accepted = [false; 2];
            self.send_deal(p);
        }
    }

    /// `name` pressed Accept (or took it back) at the table at `p`. When both
    /// have accepted, the sides swap. Returns true if a swap happened.
    pub fn accept_deal(&mut self, p: IVec3, name: &str, yes: bool) -> bool {
        let Some(s) = self.seat_at(p, name) else { return false };
        let Some(d) = self.deals.get_mut(&p) else { return false };
        d.accepted[s] = yes;
        let both = d.accepted == [true; 2] && d.seats.iter().all(|n| !n.is_empty());
        if both {
            d.accepted = [false; 2];
            d.done += 1;
            if let Some(c) = self.world.containers.get_mut(&p) {
                swap_sides(c);
            }
            self.dirty_containers.insert(p);
            // (The host's own player gets the advancement here; joined players see `done` go up.)
            if self.viewing(p, &self.player_name.clone()) {
                self.advance("fair_trade");
            }
            self.sfx(crate::sound::Sfx::Chime, Some(p.as_vec3()));
        }
        self.send_deal(p);
        both
    }

    /// Tell everyone looking at the table how things stand (each their own seat).
    pub fn send_deal(&mut self, p: IVec3) {
        if !self.is_host() {
            return;
        }
        let d = self.deals.get(&p).cloned().unwrap_or_default();
        let viewers: Vec<u32> = self.viewers.get(&p).map(|v| v.iter().copied().collect()).unwrap_or_default();
        for id in viewers {
            let name = self.peer_name(id);
            let mine = d.seats.iter().position(|s| !s.is_empty() && s.eq_ignore_ascii_case(&name)).map_or(255, |s| s as u8);
            let m = Msg::TradeState { x: p.x, y: p.y, z: p.z, seats: d.seats.clone(), accepted: d.accepted, done: d.done, mine };
            self.net_send_to(id, m);
        }
    }

    /// Our own Accept button (the host or single player decides; a joined player asks).
    pub fn press_accept(&mut self) {
        let Some(p) = self.open else { return };
        if store_kind_is_table(self, p) {
            let me = self.player_name.clone();
            let yes = self.seat_at(p, &me).is_some_and(|s| !self.deals.get(&p).is_some_and(|d| d.accepted[s]));
            if self.is_client() {
                self.net_send_msg(Msg::TradeAccept { x: p.x, y: p.y, z: p.z, accept: yes });
            } else {
                self.accept_deal(p, &me, yes);
            }
        }
    }

    /// A joined player: the host's word on a table.
    pub fn apply_deal(&mut self, p: IVec3, seats: [String; 2], accepted: [bool; 2], done: u32, mine: u8) {
        // (The first word after opening only says where things stand.)
        let fresh = !self.deals.contains_key(&p);
        let d = self.deals.entry(p).or_default();
        let swapped = !fresh && done > d.done;
        *d = Deal { seats, accepted, done, mine: (mine < 2).then_some(mine as usize) };
        if swapped && self.open == Some(p) {
            self.advance("fair_trade");
        }
    }
}

fn store_kind_is_table(g: &Game, p: IVec3) -> bool {
    g.world.get_v(p) == TRADING_TABLE
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table_game() -> (Game, IVec3) {
        let mut g = crate::game::tests::arena(970);
        g.player_name = "Ann".into();
        let p = IVec3::new(2, 50, 2);
        g.world.set_v(p, TRADING_TABLE);
        g.ensure_container(p);
        (g, p)
    }

    #[test]
    fn seats_sides_and_a_swap() {
        let (mut g, p) = table_game();
        assert_eq!(g.world.containers[&p].slots.len(), TRADE_SLOTS);
        // Ann (us) has it open; Bob has something on his side.
        g.open = Some(p);
        assert_eq!(g.take_seat(p, "Ann"), Some(0));
        assert_eq!(g.take_seat(p, "Bob"), Some(1));
        g.world.containers.get_mut(&p).unwrap().slots[9] = Some((GOLD_INGOT, 10));
        assert_eq!(g.take_seat(p, "Cat"), None, "two seats only");
        assert_eq!(g.take_seat(p, "ann"), Some(0), "a seat is kept by name");
        assert!(g.may_touch(p, "Ann", 3) && !g.may_touch(p, "Ann", 12) && g.may_touch(p, "Bob", 12) && !g.may_touch(p, "Cat", 0));
        // Offers on each side.
        g.world.containers.get_mut(&p).unwrap().slots[0] = Some((DIAMOND, 2));
        // One yes isn't a deal; a change takes it back.
        assert!(!g.accept_deal(p, "Ann", true));
        g.deal_changed(p);
        assert_eq!(g.deals[&p].accepted, [false; 2]);
        assert!(!g.accept_deal(p, "Ann", true));
        assert!(!g.accept_deal(p, "Cat", true), "not seated");
        assert!(g.accept_deal(p, "Bob", true));
        let c = &g.world.containers[&p];
        assert_eq!((c.slots[0], c.slots[9]), (Some((GOLD_INGOT, 10)), Some((DIAMOND, 2))));
        assert_eq!(g.deals[&p].accepted, [false; 2]);
        assert_eq!(g.deals[&p].done, 1);
    }

    #[test]
    fn a_seat_frees_up_once_its_side_is_empty() {
        let (mut g, p) = table_game();
        g.open = Some(p);
        g.take_seat(p, "Ann");
        g.take_seat(p, "Bob");
        g.world.containers.get_mut(&p).unwrap().slots[10] = Some((STICK, 1));
        // Bob walks off with a stick still out: his seat stays.
        g.leave_seat(p, "Bob");
        assert_eq!(g.take_seat(p, "Cat"), None);
        g.world.containers.get_mut(&p).unwrap().slots[10] = None;
        g.leave_seat(p, "Bob");
        assert_eq!(g.take_seat(p, "Cat"), Some(1));
    }
}
