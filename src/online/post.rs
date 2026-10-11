//! **Post boxes** (an original): send things and a letter to another player,
//! and it gets there even if they're not playing.
//!
//! - A Post Box opens on your **outbox**: nine slots for the parcel, who it's
//!   for, and a letter. **Send** hands it to the host, and a little later
//!   (the Cluckster takes its time) it lands in their **inbox**, which any
//!   Post Box opens for them.
//! - If they're about when it arrives, a slightly put-upon Cluckster turns
//!   up beside them with it, sighs, and waddles off. If they're not, it waits;
//!   they're told there's post when they next join.
//! - Inboxes are take-only (27 slots, and the letters). A parcel that won't
//!   fit waits for room.
//!
//! Like Personal Chests (see stash.rs), each player's inbox and outbox are
//! containers under keys that aren't real block positions, so opening them,
//! moving things and the host's checks work like any chest's. The parcels on
//! their way are the host's, saved with the world.

use crate::block::*;
use crate::containers::Container;
use crate::game::Game;
use crate::net::Msg;
use macroquad::math::{IVec3, Vec3};

const POST_X: i32 = i32::MIN + 3;
const INBOX_Y: i32 = -4098;
const OUTBOX_Y: i32 = -4099;
pub const INBOX_SLOTS: usize = 27;
pub const OUTBOX_SLOTS: usize = 9;
/// How far from a Post Box a player can use theirs.
pub const REACH: i32 = 6;
/// Seconds a parcel takes (the Cluckster won't be hurried).
pub const DELIVERY_SECS: f32 = 20.0;
/// How long the courier hangs about before going.
const COURIER_SECS: f32 = 8.0;
/// Most letters an inbox keeps, the longest letter, and most parcels on their way.
pub const MAX_LETTERS: usize = 20;
pub const LETTER_LEN: usize = 240;
const MAX_PARCELS: usize = 512;

fn hash(name: &str) -> i32 {
    let key = crate::players::record_key(name);
    let mut h: u32 = 0x811C_9DC5;
    for b in key.bytes() {
        h = (h ^ b as u32).wrapping_mul(0x0100_0193);
    }
    h as i32
}

pub fn inbox_key(name: &str) -> IVec3 {
    IVec3::new(POST_X, INBOX_Y, hash(name))
}

pub fn outbox_key(name: &str) -> IVec3 {
    IVec3::new(POST_X, OUTBOX_Y, hash(name))
}

pub fn is_post_key(p: IVec3) -> bool {
    p.x == POST_X && (p.y == INBOX_Y || p.y == OUTBOX_Y)
}

pub fn is_inbox(p: IVec3) -> bool {
    p.x == POST_X && p.y == INBOX_Y
}

/// An empty inbox or outbox for this key.
pub fn fresh(p: IVec3) -> Container {
    let n = if is_inbox(p) { INBOX_SLOTS } else { OUTBOX_SLOTS };
    Container { slots: vec![None; n], wear: vec![0; n], burn: 0.0, burn_total: 0.0, cook: 0.0 }
}

/// Is there a Post Box within reach of `at`?
pub fn near_post_box(world: &crate::world::World, at: Vec3) -> bool {
    let c = at.floor().as_ivec3();
    (-REACH..=REACH).any(|dy| (-REACH..=REACH).any(|dz| (-REACH..=REACH).any(|dx| world.get_v(c + IVec3::new(dx, dy, dz)) == POST_BOX)))
}

/// Something on its way.
#[derive(Clone, Debug, PartialEq)]
pub struct Parcel {
    pub from: String,
    pub to: String,
    pub items: Vec<(Id, u8, crate::inventory::Wear)>,
    pub letter: String,
    /// Seconds until it arrives.
    pub due: f32,
}

/// Parcels on their way, and everyone's letters, for the save.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mail {
    pub parcels: Vec<Parcel>,
    /// Inbox key's z -> (from, letter), oldest first.
    pub letters: std::collections::HashMap<i32, Vec<(String, String)>>,
}

fn clean(s: &str, max: usize) -> String {
    s.chars().filter(|c| !c.is_control() || *c == '\n').take(max).collect::<String>().trim().to_string()
}

impl Mail {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = crate::save::W(Vec::new());
        w.u32(self.parcels.len() as u32);
        for p in &self.parcels {
            w.str(&p.from);
            w.str(&p.to);
            w.str(&p.letter);
            w.f32(p.due);
            w.u32(p.items.len() as u32);
            for &(id, n, wear) in &p.items {
                w.str(reg().key_of(id));
                w.u8(n);
                w.u32(wear);
            }
        }
        w.u32(self.letters.len() as u32);
        for (k, list) in &self.letters {
            w.i32(*k);
            w.u32(list.len() as u32);
            for (from, text) in list {
                w.str(from);
                w.str(text);
            }
        }
        w.0
    }

    pub fn decode(b: &[u8]) -> Mail {
        let read = || -> std::io::Result<Mail> {
            let mut r = crate::save::R(b);
            let mut m = Mail::default();
            for _ in 0..r.u32()?.min(MAX_PARCELS as u32) {
                let (from, to, letter, due) = (r.str()?, r.str()?, r.str()?, r.f32()?);
                let mut items = Vec::new();
                for _ in 0..r.u32()?.min(OUTBOX_SLOTS as u32) {
                    let (key, n, wear) = (r.str()?, r.u8()?, r.u32()?);
                    // (Items from a mod that's gone are lost, like anywhere else.)
                    if let Some(id) = reg().lookup(&key) {
                        items.push((id, n, wear));
                    }
                }
                m.parcels.push(Parcel { from, to, items, letter, due: if due.is_finite() { due } else { 0.0 } });
            }
            for _ in 0..r.u32()?.min(100_000) {
                let k = r.i32()?;
                let mut list = Vec::new();
                for _ in 0..r.u32()?.min(MAX_LETTERS as u32) {
                    list.push((r.str()?, r.str()?));
                }
                m.letters.insert(k, list);
            }
            Ok(m)
        };
        read().unwrap_or_default()
    }
}

/// Put a parcel's things into an inbox, if they all fit (else nothing moves).
pub fn deliver_into(inbox: &mut Container, items: &[(Id, u8, crate::inventory::Wear)]) -> bool {
    let mut c = inbox.clone();
    for &(id, n, wear) in items {
        let mut left = n;
        // Top up matching stacks (not worn things), then empty slots.
        if wear == 0 {
            for i in 0..c.slots.len() {
                if let Some((sid, sn)) = c.slots[i]
                    && sid == id
                    && c.wear[i] == 0
                {
                    let put = left.min(max_stack(id).saturating_sub(sn));
                    c.slots[i] = Some((id, sn + put));
                    left -= put;
                }
            }
        }
        if left > 0 {
            let Some(i) = c.slots.iter().position(Option::is_none) else { return false };
            c.slots[i] = Some((id, left));
            c.wear[i] = wear;
        }
    }
    *inbox = c;
    true
}

impl Game {
    /// Right-clicked a Post Box: open our outbox (or inbox).
    pub fn open_post(&mut self, inbox: bool) {
        let me = self.player_name.clone();
        let key = if inbox { inbox_key(&me) } else { outbox_key(&me) };
        if !self.is_client() {
            self.world.mail_boxes.entry(key).or_insert_with(|| fresh(key));
        }
        self.inv.return_cursor();
        if let Some(old) = self.open.take()
            && self.is_client()
        {
            self.net_send_msg(Msg::CloseContainer { x: old.x, y: old.y, z: old.z });
        }
        self.open = Some(key);
        if self.is_client() {
            self.net_send_msg(Msg::OpenContainer { x: key.x, y: key.y, z: key.z });
        }
    }

    /// Is `name` someone post can go to (a player this world knows)?
    pub fn knows_player(&self, name: &str) -> bool {
        let key = crate::players::record_key(name);
        (!self.dedicated && crate::players::record_key(&self.player_name) == key) || self.all_peers().any(|(_, p)| crate::players::record_key(&p.name) == key) || self.saved_players.contains_key(&key)
    }

    /// The host, or single player: `from` sends their outbox to `to`. Err says why not.
    pub fn send_post(&mut self, from: &str, to: &str, letter: &str) -> Result<String, String> {
        let to = clean(to, 32);
        let letter = clean(letter, LETTER_LEN);
        if to.is_empty() {
            return Err("Who's it for? Write a name.".into());
        }
        if crate::players::record_key(&to) == crate::players::record_key(from) {
            return Err("Posting things to yourself? The Cluckster refuses.".into());
        }
        if !self.knows_player(&to) {
            return Err(format!("Nobody called {to} has played here."));
        }
        if self.mail.parcels.len() >= MAX_PARCELS {
            return Err("The Cluckster is snowed under. Try again later.".into());
        }
        let out = outbox_key(from);
        let items: Vec<_> = self.world.mail_boxes.get(&out).map(|c| c.contents()).unwrap_or_default();
        if items.is_empty() && letter.is_empty() {
            return Err("Put something in, or write a letter.".into());
        }
        self.world.mail_boxes.insert(out, fresh(out));
        self.dirty_containers.insert(out);
        let n = items.len();
        self.mail.parcels.push(Parcel { from: from.to_string(), to: to.clone(), items, letter, due: DELIVERY_SECS });
        if self.is_local_player(from) {
            self.advance("first_class");
        }
        Ok(if n == 0 { format!("Your letter is on its way to {to}.") } else { format!("Your parcel ({n} stack{}) is on its way to {to}.", if n == 1 { "" } else { "s" }) })
    }

    /// The host, or single player: parcels arrive when they're due (and if they fit).
    pub fn post_tick(&mut self, dt: f32) {
        if self.is_client() {
            return;
        }
        let mut gone = Vec::new();
        self.couriers.retain_mut(|c| {
            c.1 -= dt;
            if c.1 <= 0.0 {
                gone.push(c.0);
            }
            c.1 > 0.0
        });
        if !gone.is_empty() {
            // (Off it waddles, out of sight.)
            self.mobs.retain(|m| !gone.contains(&m.id));
        }
        if self.mail.parcels.is_empty() {
            return;
        }
        for p in self.mail.parcels.iter_mut() {
            p.due -= dt;
        }
        let due: Vec<usize> = (0..self.mail.parcels.len()).filter(|&i| self.mail.parcels[i].due <= 0.0).collect();
        let mut delivered = Vec::new();
        for i in due {
            let p = self.mail.parcels[i].clone();
            let key = inbox_key(&p.to);
            let inbox = self.world.mail_boxes.entry(key).or_insert_with(|| fresh(key));
            if !deliver_into(inbox, &p.items) {
                // Full: it waits, and tries again in a while.
                self.mail.parcels[i].due = 30.0;
                continue;
            }
            self.dirty_containers.insert(key);
            if !p.letter.is_empty() {
                let list = self.mail.letters.entry(key.z).or_default();
                list.push((p.from.clone(), p.letter.clone()));
                if list.len() > MAX_LETTERS {
                    list.remove(0);
                }
            }
            delivered.push(i);
            self.courier_arrives(&p);
        }
        for i in delivered.into_iter().rev() {
            self.mail.parcels.remove(i);
        }
    }

    /// A Cluckster turns up beside them with the post (if they're about).
    fn courier_arrives(&mut self, p: &Parcel) {
        let what = if p.items.is_empty() { "a letter" } else { "a parcel" };
        let line = format!("A Cluckster arrives with {what} from {}, sighs, and waddles off. (It's in your inbox at any Post Box.)", p.from);
        let at = if self.is_local_player(&p.to) {
            self.msg(line);
            self.advance("youve_got_post");
            Some(self.player.body.pos)
        } else if let Some(id) = self.peer_by_name(&p.to) {
            self.system_message(Some(id), &line);
            self.send_letters(id, true);
            self.peers.get(&id).map(|q| q.target)
        } else {
            None
        };
        if let Some(at) = at {
            let id = self.alloc_mob(crate::entity::MobKind::Cluckster, at + Vec3::new(1.5, 0.5, 0.5));
            // (Not kept in the save: it's off on its round by the time anyone's back.)
            if let Some(m) = self.mobs.iter_mut().find(|m| m.id == id) {
                m.persistent = false;
            }
            self.mob_names.insert(id, "Courier".into());
            self.couriers.push((id, COURIER_SECS));
            self.sfx(crate::sound::Sfx::Cluck, Some(at));
        }
    }

    /// How many parcels (and letters) are waiting in `name`'s inbox.
    pub fn post_waiting(&self, name: &str) -> usize {
        let key = inbox_key(name);
        self.world.mail_boxes.get(&key).map_or(0, |c| c.slots.iter().filter(|s| s.is_some()).count()) + self.mail.letters.get(&key.z).map_or(0, Vec::len)
    }

    /// The letters in `name`'s inbox (the host's copy, or ours as it told us).
    pub fn letters_for(&self, name: &str) -> Vec<(String, String)> {
        self.mail.letters.get(&inbox_key(name).z).cloned().unwrap_or_default()
    }

    /// Throw our letters away.
    pub fn bin_letters(&mut self) {
        let key = inbox_key(&self.player_name.clone());
        self.mail.letters.remove(&key.z);
        if self.is_client() {
            self.net_send_msg(Msg::PostBin);
        }
    }

    /// The host: tell a joined player their letters.
    pub fn send_letters(&mut self, id: u32, arrived: bool) {
        let name = self.peer_name(id);
        let list = self.letters_for(&name);
        self.net_send_to(id, Msg::Letters { list, arrived });
    }

    /// The host: someone joined; tell them their letters, and if there's post.
    pub fn post_on_join(&mut self, id: u32, name: &str) {
        self.send_letters(id, false);
        let n = self.post_waiting(name);
        if n > 0 {
            self.system_message(Some(id), &format!("You've got post! ({n} waiting at any Post Box.)"));
        }
    }

    /// Our Send button. False if it plainly didn't go (a joined player hears from the host).
    pub fn press_send(&mut self, to: &str, letter: &str) -> bool {
        if self.is_client() {
            // (The host says if it went; anything to send, to anyone, earns the advancement.)
            let something = !letter.trim().is_empty() || self.open.and_then(|p| self.world.mail_boxes.get(&p)).is_some_and(|c| c.slots.iter().any(Option::is_some));
            if something && !to.trim().is_empty() {
                self.advance("first_class");
            }
            self.net_send_msg(Msg::PostSend { to: to.to_string(), letter: letter.to_string() });
            return true;
        }
        let me = self.player_name.clone();
        let r = self.send_post(&me, to, letter);
        let ok = r.is_ok();
        let (Ok(s) | Err(s)) = r;
        self.msg(s);
        ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parcels_fit_or_wait() {
        let mut inbox = fresh(inbox_key("Ann"));
        assert_eq!(inbox.slots.len(), INBOX_SLOTS);
        assert!(deliver_into(&mut inbox, &[(DIAMOND, 3, 0), (DIAMOND, 2, 0), (PICK_IRON, 1, 40)]));
        assert_eq!(inbox.slots[0], Some((DIAMOND, 5)));
        assert_eq!((inbox.slots[1], inbox.wear[1]), (Some((PICK_IRON, 1)), 40));
        // Full: nothing goes in at all.
        for s in inbox.slots.iter_mut().filter(|s| s.is_none()) {
            *s = Some((COBBLE, 64));
        }
        let before = inbox.clone();
        assert!(!deliver_into(&mut inbox, &[(STICK, 1, 0)]));
        assert_eq!(inbox, before);
        assert!(is_inbox(inbox_key("ann")) && !is_inbox(outbox_key("Ann")) && is_post_key(outbox_key("x")));
        assert_eq!(inbox_key("Ann"), inbox_key("ANN"));
    }

    #[test]
    fn a_parcel_goes_from_outbox_to_inbox_and_waits_for_someone_away() {
        let mut g = crate::game::tests::arena(995);
        g.player_name = "Ann".into();
        // Bob's played here before (he's remembered), but isn't on now.
        let report = crate::players::Report { slots: Vec::new(), health: 20.0, food: 20.0, saturation: 5.0 };
        let bob = crate::players::PlayerRecord { pos: Vec3::ZERO, xp: 0, bag: Vec::new(), report, enchanted: Vec::new(), enchant_count: 0, stats: Vec::new(), mode: 0, dim: crate::dims::Dim::Over };
        g.saved_players.insert(crate::players::record_key("Bob"), bob);
        let out = outbox_key("Ann");
        let mut c = fresh(out);
        c.slots[0] = Some((DIAMOND, 4));
        g.world.mail_boxes.insert(out, c);
        assert!(g.send_post("Ann", "Nobody", "hi").is_err());
        assert!(g.send_post("Ann", "Ann", "hi").is_err());
        assert!(g.send_post("Ann", "Bob", "Happy birthday!").is_ok());
        assert!(g.advancements.has("first_class"));
        assert!(g.world.mail_boxes[&out].slots.iter().all(Option::is_none), "the outbox is emptied");
        g.post_tick(DELIVERY_SECS / 2.0);
        assert_eq!(g.post_waiting("Bob"), 0, "not yet");
        g.post_tick(DELIVERY_SECS);
        assert_eq!(g.world.mail_boxes[&inbox_key("bob")].slots[0], Some((DIAMOND, 4)));
        assert_eq!(g.letters_for("Bob"), vec![("Ann".to_string(), "Happy birthday!".to_string())]);
        assert_eq!(g.post_waiting("Bob"), 2);
        assert!(g.mail.parcels.is_empty());
        // Kept with the world.
        let back = Game::from_save(g.to_save());
        assert_eq!(back.mail, g.mail);
        assert_eq!(back.world.mail_boxes.get(&inbox_key("Bob")), g.world.mail_boxes.get(&inbox_key("Bob")));
    }
}
