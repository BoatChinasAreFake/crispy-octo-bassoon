//! **Land claims** (an original): on a server that turns them on
//! (`/claims on`, operators), a **Survey Post** placed in the ground claims
//! the land around it, 33 by 33 blocks from bedrock to sky, for whoever put
//! it there. Only they, and the friends they name, can build, break or open
//! things there. Operators (and the host) can always build.
//!
//! - A player can have up to three posts; claims can't overlap someone
//!   else's (your own can touch).
//! - Breaking your Survey Post gives the land back.
//! - `/claim friend add <name>`, `/claim friend remove <name>`, `/claim friends`,
//!   `/claim list`, and `/claim` on its own says whose land you're on.
//! - Claims show on held maps, and you're told when you walk onto someone's land.
//!
//! The host keeps the claims (saved with the world) and checks every edit a
//! joined player sends; they're sent to every player whenever they change.

use macroquad::math::IVec3;
use std::collections::BTreeMap;

/// How far a claim reaches from its post, each way.
pub const RADIUS: i32 = 16;
pub const MAX_PER_PLAYER: usize = 3;
pub const MAX_FRIENDS: usize = 32;

#[derive(Clone, Debug, PartialEq)]
pub struct Claim {
    /// Who put the post down (as they're named).
    pub owner: String,
    pub post: IVec3,
    /// Which dimension (see dims.rs).
    pub dim: u8,
}

impl Claim {
    pub fn covers(&self, dim: u8, x: i32, z: i32) -> bool {
        self.dim == dim && (x - self.post.x).abs() <= RADIUS && (z - self.post.z).abs() <= RADIUS
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Claims {
    /// Are claims switched on (on this server)?
    pub on: bool,
    pub list: Vec<Claim>,
    /// Owner (lower case) -> the friends they've named (lower case).
    pub friends: BTreeMap<String, Vec<String>>,
}

impl Claims {
    /// Whose land this is, if anyone's.
    pub fn claim_at(&self, dim: u8, x: i32, z: i32) -> Option<&Claim> {
        self.list.iter().find(|c| c.covers(dim, x, z))
    }

    pub fn is_friend(&self, owner: &str, who: &str) -> bool {
        owner.eq_ignore_ascii_case(who) || self.friends.get(&owner.to_ascii_lowercase()).is_some_and(|f| f.iter().any(|n| n.eq_ignore_ascii_case(who)))
    }

    /// Can `who` build at (x, z)? (Operators are let through by the caller.)
    pub fn may_build(&self, dim: u8, who: &str, x: i32, z: i32) -> bool {
        !self.on || self.claim_at(dim, x, z).is_none_or(|c| self.is_friend(&c.owner, who))
    }

    /// `owner` put a Survey Post at `post`: claim the land, or say why not.
    pub fn add(&mut self, owner: &str, post: IVec3, dim: u8) -> Result<(), String> {
        if !self.on {
            return Err("Land claims are off here (an operator can turn them on with /claims on).".into());
        }
        let mine = self.list.iter().filter(|c| c.owner.eq_ignore_ascii_case(owner)).count();
        if mine >= MAX_PER_PLAYER {
            return Err(format!("You have {MAX_PER_PLAYER} Survey Posts already. Break one to move it."));
        }
        let reach = 2 * RADIUS;
        if let Some(c) = self.list.iter().find(|c| c.dim == dim && !c.owner.eq_ignore_ascii_case(owner) && (c.post.x - post.x).abs() <= reach && (c.post.z - post.z).abs() <= reach) {
            return Err(format!("That would overlap {}'s land.", c.owner));
        }
        self.list.push(Claim { owner: owner.to_string(), post, dim });
        Ok(())
    }

    /// A Survey Post went: so does its claim. Returns its owner.
    pub fn remove_post(&mut self, post: IVec3, dim: u8) -> Option<String> {
        let i = self.list.iter().position(|c| c.post == post && c.dim == dim)?;
        Some(self.list.remove(i).owner)
    }

    /// `/claim ...` and `/claims on|off` from `player`. Returns what to tell
    /// them, and whether the claims changed (so they're sent out again).
    pub fn command(&mut self, player: &str, word: &str, args: &str, op: bool, here: (u8, i32, i32)) -> (Vec<String>, bool) {
        let mut w = args.split_whitespace();
        let (a, b, c) = (w.next().unwrap_or("").to_ascii_lowercase(), w.next().unwrap_or("").to_ascii_lowercase(), w.next().unwrap_or(""));
        let say = |s: String| (vec![s], false);
        if word == "claims" {
            return match (a.as_str(), op) {
                ("on", true) => {
                    self.on = true;
                    (vec!["Land claims are on: a Survey Post claims the land around it.".into()], true)
                }
                ("off", true) => {
                    self.on = false;
                    (vec!["Land claims are off (posts stay put, and come back into force if they're turned on again).".into()], true)
                }
                (_, false) if !a.is_empty() => say("Only operators can turn land claims on or off.".into()),
                _ => say(format!("Land claims are {}.", if self.on { "on" } else { "off" })),
            };
        }
        let key = player.to_ascii_lowercase();
        match (a.as_str(), b.as_str()) {
            ("friend", "add") if !c.is_empty() => {
                let f = self.friends.entry(key).or_default();
                if f.iter().any(|n| n.eq_ignore_ascii_case(c)) {
                    return say(format!("{c} is already your friend."));
                }
                if f.len() >= MAX_FRIENDS {
                    return say(format!("That's {MAX_FRIENDS} friends already."));
                }
                f.push(c.to_ascii_lowercase());
                (vec![format!("{c} can build on your land now.")], true)
            }
            ("friend", "remove") if !c.is_empty() => {
                let f = self.friends.entry(key.clone()).or_default();
                let before = f.len();
                f.retain(|n| !n.eq_ignore_ascii_case(c));
                let (now, empty) = (f.len(), f.is_empty());
                if empty {
                    self.friends.remove(&key);
                }
                if now == before {
                    return say(format!("{c} wasn't on your list."));
                }
                (vec![format!("{c} can't build on your land any more.")], true)
            }
            ("friends", _) | ("friend", "list") => {
                let f = self.friends.get(&key).cloned().unwrap_or_default();
                say(if f.is_empty() { "No friends named yet. /claim friend add <name>".into() } else { format!("Your friends: {}", f.join(", ")) })
            }
            ("list", _) => {
                let mine: Vec<String> = self.list.iter().filter(|cl| cl.owner.eq_ignore_ascii_case(player)).map(|cl| format!("{}, {}, {}", cl.post.x, cl.post.y, cl.post.z)).collect();
                say(if mine.is_empty() { "You haven't claimed any land. Place a Survey Post.".into() } else { format!("Your Survey Posts: {}", mine.join("; ")) })
            }
            ("", _) => {
                let (dim, x, z) = here;
                say(match self.claim_at(dim, x, z) {
                    _ if !self.on => "Land claims are off here.".into(),
                    Some(cl) if cl.owner.eq_ignore_ascii_case(player) => "This is your land.".into(),
                    Some(cl) => format!("This is {}'s land{}.", cl.owner, if self.is_friend(&cl.owner, player) { " (and you're a friend)" } else { "" }),
                    None => "Nobody has claimed this land. A Survey Post would.".into(),
                })
            }
            _ => say("/claim, /claim list, /claim friend add|remove <name>, /claim friends.".into()),
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = format!("on\t{}\n", self.on as u8);
        for c in &self.list {
            out += &format!("c\t{}\t{}\t{}\t{}\t{}\n", c.owner, c.post.x, c.post.y, c.post.z, c.dim);
        }
        for (owner, f) in &self.friends {
            for n in f {
                out += &format!("f\t{owner}\t{n}\n");
            }
        }
        out.into_bytes()
    }

    pub fn decode(b: &[u8]) -> Claims {
        let mut out = Claims::default();
        let clean = |s: &str| s.chars().filter(|c| !c.is_control()).take(32).collect::<String>();
        for line in String::from_utf8_lossy(b).lines() {
            let f: Vec<&str> = line.split('\t').collect();
            match f.as_slice() {
                ["on", v] => out.on = *v == "1",
                ["c", owner, x, y, z, dim] => {
                    if let (Ok(x), Ok(y), Ok(z), Ok(dim)) = (x.parse(), y.parse(), z.parse(), dim.parse()) {
                        out.list.push(Claim { owner: clean(owner), post: IVec3::new(x, y, z), dim });
                    }
                }
                ["f", owner, n] => out.friends.entry(clean(owner).to_ascii_lowercase()).or_default().push(clean(n).to_ascii_lowercase()),
                _ => {}
            }
        }
        out.list.truncate(4096);
        out
    }
}

// ------------------------------------------------------------------ in the game

use crate::block::SURVEY_POST;
use crate::game::Game;
use crate::net::Msg;

impl Game {
    pub fn claims_msg(&self) -> Msg {
        Msg::Claims { data: self.claims.encode() }
    }

    pub fn share_claims(&mut self) {
        if self.is_host() {
            let m = self.claims_msg();
            self.net_broadcast_all(m);
        }
    }

    fn here_dim(&self) -> u8 {
        self.world.dim().index()
    }

    /// Can the player named `who` build at `p` (operators always can)?
    pub fn may_build_at(&self, who: &str, p: IVec3) -> bool {
        self.claims.may_build(self.here_dim(), who, p.x, p.z) || self.admin.is_op(who)
    }

    /// The host, or single player: someone put a Survey Post down. Err says
    /// why it doesn't claim anything (and the post shouldn't stay).
    pub fn post_placed(&mut self, who: &str, p: IVec3) -> Result<(), String> {
        let dim = self.here_dim();
        self.claims.add(who, p, dim)?;
        self.share_claims();
        if self.is_local_player(who) {
            self.advance("staked_claim");
        }
        Ok(())
    }

    /// A Survey Post was broken (by whoever was allowed to).
    pub fn post_broken(&mut self, p: IVec3) {
        let dim = self.here_dim();
        if let Some(owner) = self.claims.remove_post(p, dim) {
            self.msg(format!("{owner}'s claim is gone."));
            self.share_claims();
        }
    }

    /// Checked on the host for an edit by a joined player: false if it's on
    /// someone else's land (and tells them, now and then).
    pub fn host_claim_ok(&mut self, from: u32, p: IVec3, old: crate::block::Id, id: crate::block::Id) -> bool {
        let who = self.peer_name(from);
        if !self.may_build_at(&who, p) {
            if self.peer_rate_ok(from, "claim_note", 3.0) {
                let owner = self.claims.claim_at(self.here_dim(), p.x, p.z).map(|c| c.owner.clone()).unwrap_or_default();
                self.system_message(Some(from), &format!("This is {owner}'s land. Ask them to /claim friend add you."));
            }
            return false;
        }
        if id == SURVEY_POST && old != SURVEY_POST && self.claims.on {
            if let Err(e) = self.post_placed(&who, p) {
                self.system_message(Some(from), &e);
                return false;
            }
        } else if old == SURVEY_POST && id != SURVEY_POST {
            self.post_broken(p);
        }
        true
    }

    /// Our own player walked onto (or off) someone's land: say so.
    pub fn note_land(&mut self) {
        if !self.claims.on {
            return;
        }
        let p = self.player.body.pos.floor().as_ivec3();
        let owner = self.claims.claim_at(self.here_dim(), p.x, p.z).map(|c| c.owner.clone());
        if owner != self.on_land {
            match &owner {
                Some(o) if o.eq_ignore_ascii_case(&self.player_name) => self.msg("Your land."),
                Some(o) => self.msg(format!("Now on {o}'s land.")),
                None if self.on_land.is_some() => self.msg("Leaving claimed land."),
                None => {}
            }
            self.on_land = owner;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on() -> Claims {
        Claims { on: true, ..Default::default() }
    }

    #[test]
    fn posts_claim_land_and_friends_can_build() {
        let mut c = on();
        let p = IVec3::new(100, 60, 100);
        c.add("Ann", p, 0).unwrap();
        assert!(c.may_build(0, "ann", 116, 84) && !c.may_build(0, "Bob", 116, 84), "the corners are hers");
        assert!(c.may_build(0, "Bob", 117, 100), "one past is free");
        assert!(c.may_build(1, "Bob", 100, 100), "another dimension is another place");
        // Bob can't claim on top of her; Ann can claim next door to herself.
        assert!(c.add("Bob", IVec3::new(130, 60, 100), 0).unwrap_err().contains("Ann"));
        c.add("Bob", IVec3::new(133, 60, 100), 0).unwrap();
        c.add("Ann", IVec3::new(100, 60, 120), 0).unwrap();
        c.add("Ann", IVec3::new(60, 60, 60), 0).unwrap();
        assert!(c.add("Ann", IVec3::new(0, 60, 0), 0).unwrap_err().contains("3"));
        // Friends.
        let (out, changed) = c.command("Ann", "claim", "friend add Bob", false, (0, 0, 0));
        assert!(changed, "{out:?}");
        assert!(c.may_build(0, "BOB", 100, 100));
        c.command("Ann", "claim", "friend remove bob", false, (0, 0, 0));
        assert!(!c.may_build(0, "Bob", 100, 100));
        assert_eq!(c.command("Bob", "claim", "", false, (0, 100, 100)).0[0], "This is Ann's land.");
        // Breaking the post frees it.
        assert_eq!(c.remove_post(p, 0).as_deref(), Some("Ann"));
        assert!(c.may_build(0, "Bob", 100, 100));
        // Off: nothing's protected, and nothing new is claimed.
        let (_, ch) = c.command("Bob", "claims", "off", false, (0, 0, 0));
        assert!(!ch && c.on);
        c.command("Op", "claims", "off", true, (0, 0, 0));
        assert!(c.may_build(0, "Bob", 100, 120));
        assert!(c.add("Bob", IVec3::new(500, 60, 500), 0).is_err());
        assert_eq!(Claims::decode(&c.encode()), c);
    }

    #[test]
    fn a_joined_player_cant_build_on_someone_elses_land() {
        let mut g = crate::game::tests::arena(990);
        g.player_name = "Ann".into();
        g.claims.on = true;
        let post = IVec3::new(0, 50, 0);
        g.post_placed("Ann", post).unwrap();
        assert!(g.advancements.has("staked_claim"));
        g.peers.insert(7, crate::multiplayer::Peer::new("Bob".into(), macroquad::math::Vec3::new(3.0, 50.0, 3.0)));
        use crate::block::{AIR, STONE};
        assert!(!g.host_claim_ok(7, IVec3::new(2, 50, 2), AIR, STONE));
        g.claims.command("Ann", "claim", "friend add Bob", false, (0, 0, 0));
        assert!(g.host_claim_ok(7, IVec3::new(2, 50, 2), AIR, STONE));
        // Bob's own post too close to Ann's isn't a claim (and doesn't stay).
        g.claims.command("Ann", "claim", "friend remove Bob", false, (0, 0, 0));
        assert!(!g.host_claim_ok(7, IVec3::new(20, 50, 0), AIR, SURVEY_POST));
        assert!(g.host_claim_ok(7, IVec3::new(40, 50, 0), AIR, SURVEY_POST));
        assert_eq!(g.claims.list.len(), 2);
        // Kept with the world.
        let back = Game::from_save(g.to_save());
        assert_eq!(back.claims, g.claims);
    }
}
