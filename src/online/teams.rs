//! **Teams.** Players can make teams and join them: a team has a name and a
//! colour, its members' name tags (and their dots on held maps) are in that
//! colour, and `/tc` talks to your team only. When players can hurt each
//! other (`/pvp on`), a team can choose whether its members can hurt each
//! other too (`/team friendlyfire`).
//!
//! The host keeps the teams (saved with the world) and sends them to every
//! player whenever they change. Members are kept by player name, so a team
//! still has you after you log off and back on.
//!
//! - `/team create <name> [colour]` makes a team (and puts you in it).
//! - `/team join <name>`, `/team leave`, `/team list`.
//! - `/team colour <colour>`, `/team friendlyfire on|off` change your own team.
//! - `/team remove <name>` (operators) breaks a team up.
//! - `/tc <message>` says something to your team only.

use std::collections::BTreeMap;

/// The colours a team can be: (name, colour).
pub const COLOURS: [(&str, [u8; 3]); 8] = [
    ("red", [235, 70, 60]),
    ("blue", [70, 120, 245]),
    ("green", [70, 200, 80]),
    ("yellow", [245, 215, 60]),
    ("purple", [170, 90, 230]),
    ("orange", [245, 150, 50]),
    ("aqua", [70, 215, 215]),
    ("pink", [245, 130, 190]),
];

/// Most teams at once, and the longest name one can have.
pub const MAX_TEAMS: usize = 16;
pub const NAME_LEN: usize = 16;

#[derive(Clone, Debug, PartialEq)]
pub struct Team {
    pub name: String,
    /// An index into `COLOURS`.
    pub colour: u8,
    /// Can members hurt each other (when players can be hurt at all)?
    pub friendly_fire: bool,
}

impl Team {
    pub fn rgb(&self) -> [u8; 3] {
        COLOURS[self.colour as usize % COLOURS.len()].1
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Teams {
    pub teams: Vec<Team>,
    /// Player name (lower case) -> their team's name.
    pub members: BTreeMap<String, String>,
}

/// A colour by name, or None.
pub fn colour_named(name: &str) -> Option<u8> {
    let name = name.trim().to_ascii_lowercase();
    let name = match name.as_str() {
        "grey" | "gray" => return None,
        "violet" => "purple",
        "cyan" | "teal" => "aqua",
        n => n,
    };
    COLOURS.iter().position(|(n, _)| *n == name).map(|i| i as u8)
}

fn clean_name(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-').take(NAME_LEN).collect()
}

impl Teams {
    pub fn find(&self, name: &str) -> Option<usize> {
        self.teams.iter().position(|t| t.name.eq_ignore_ascii_case(name))
    }

    /// The team a player is on.
    pub fn team_of(&self, player: &str) -> Option<&Team> {
        let t = self.members.get(&player.to_ascii_lowercase())?;
        self.find(t).map(|i| &self.teams[i])
    }

    pub fn same_team(&self, a: &str, b: &str) -> bool {
        match (self.members.get(&a.to_ascii_lowercase()), self.members.get(&b.to_ascii_lowercase())) {
            (Some(x), Some(y)) => x.eq_ignore_ascii_case(y),
            _ => false,
        }
    }

    /// Can `attacker` hurt `victim` (players being hurtable at all is up to the world)?
    pub fn may_hurt(&self, attacker: &str, victim: &str) -> bool {
        !self.same_team(attacker, victim) || self.team_of(attacker).is_some_and(|t| t.friendly_fire)
    }

    /// The members of a team (lower-case names).
    pub fn members_of(&self, team: &str) -> Vec<String> {
        self.members.iter().filter(|(_, t)| t.eq_ignore_ascii_case(team)).map(|(p, _)| p.clone()).collect()
    }

    /// Put a player on a team (None: on none). Teams left empty are kept.
    pub fn set_member(&mut self, player: &str, team: Option<&str>) {
        let key = player.to_ascii_lowercase();
        match team.and_then(|t| self.find(t)) {
            Some(i) => {
                self.members.insert(key, self.teams[i].name.clone());
            }
            None => {
                self.members.remove(&key);
            }
        }
    }

    /// Make a team; Err says why not.
    pub fn create(&mut self, name: &str, colour: Option<u8>) -> Result<usize, String> {
        let name = clean_name(name);
        if name.is_empty() {
            return Err("A team needs a name (letters, numbers, - and _).".into());
        }
        if self.find(&name).is_some() {
            return Err(format!("There's already a team called {name}."));
        }
        if self.teams.len() >= MAX_TEAMS {
            return Err(format!("That's {MAX_TEAMS} teams already. Remove one first."));
        }
        // The first colour nobody's using, unless one was asked for.
        let colour = colour.unwrap_or_else(|| (0..COLOURS.len() as u8).find(|c| !self.teams.iter().any(|t| t.colour == *c)).unwrap_or(self.teams.len() as u8 % COLOURS.len() as u8));
        self.teams.push(Team { name, colour, friendly_fire: false });
        Ok(self.teams.len() - 1)
    }

    pub fn remove(&mut self, name: &str) -> bool {
        let Some(i) = self.find(name) else { return false };
        let gone = self.teams.remove(i).name;
        self.members.retain(|_, t| !t.eq_ignore_ascii_case(&gone));
        true
    }

    /// `/team ...` from `player` (`op`: an operator). Returns what to tell
    /// them, and whether the teams changed (so they're sent out again).
    pub fn command(&mut self, player: &str, args: &str, op: bool) -> (Vec<String>, bool) {
        let mut words = args.split_whitespace();
        let sub = words.next().unwrap_or("").to_ascii_lowercase();
        let arg = words.next().unwrap_or("");
        let arg2 = words.next().unwrap_or("");
        let mine = self.team_of(player).map(|t| t.name.clone());
        let say = |s: String| (vec![s], false);
        match sub.as_str() {
            "create" | "new" | "make" => {
                let colour = if arg2.is_empty() {
                    None
                } else {
                    match colour_named(arg2) {
                        Some(c) => Some(c),
                        None => return say(format!("Colours: {}.", COLOURS.map(|c| c.0).join(", "))),
                    }
                };
                match self.create(arg, colour) {
                    Ok(i) => {
                        let (name, col) = (self.teams[i].name.clone(), COLOURS[self.teams[i].colour as usize].0);
                        self.set_member(player, Some(&name));
                        (vec![format!("Made team {name} ({col}), and you're on it.")], true)
                    }
                    Err(e) => say(e),
                }
            }
            "join" => match self.find(arg) {
                Some(i) => {
                    let name = self.teams[i].name.clone();
                    if mine.as_deref().is_some_and(|m| m.eq_ignore_ascii_case(&name)) {
                        return say(format!("You're already on {name}."));
                    }
                    self.set_member(player, Some(&name));
                    (vec![format!("You're on team {name} now.")], true)
                }
                None => say(format!("No team called {arg}. /team list shows them.")),
            },
            "leave" => match mine {
                Some(t) => {
                    self.set_member(player, None);
                    (vec![format!("You left {t}.")], true)
                }
                None => say("You're not on a team.".into()),
            },
            "colour" | "color" => match (mine, colour_named(arg)) {
                (Some(t), Some(c)) => {
                    let i = self.find(&t).unwrap_or(0);
                    self.teams[i].colour = c;
                    (vec![format!("{t} is {} now.", COLOURS[c as usize].0)], true)
                }
                (None, _) => say("Join a team first.".into()),
                (_, None) => say(format!("Colours: {}.", COLOURS.map(|c| c.0).join(", "))),
            },
            "friendlyfire" | "ff" => match (mine, arg.to_ascii_lowercase().as_str()) {
                (Some(t), v @ ("on" | "off")) => {
                    let i = self.find(&t).unwrap_or(0);
                    self.teams[i].friendly_fire = v == "on";
                    let how = if v == "on" { "can" } else { "can't" };
                    (vec![format!("{t}'s members {how} hurt each other now.")], true)
                }
                (None, _) => say("Join a team first.".into()),
                _ => say("/team friendlyfire on or off.".into()),
            },
            "remove" | "delete" if op => {
                if self.remove(arg) {
                    (vec![format!("Team {arg} is no more.")], true)
                } else {
                    say(format!("No team called {arg}."))
                }
            }
            "remove" | "delete" => say("Only operators can remove teams.".into()),
            "list" | "" => {
                if self.teams.is_empty() {
                    return say("No teams yet. /team create <name> [colour] makes one.".into());
                }
                let lines = self
                    .teams
                    .iter()
                    .map(|t| {
                        let who = self.members_of(&t.name);
                        let ff = if t.friendly_fire { ", friendly fire on" } else { "" };
                        format!("{} ({}{ff}): {}", t.name, COLOURS[t.colour as usize % COLOURS.len()].0, if who.is_empty() { "nobody".to_string() } else { who.join(", ") })
                    })
                    .collect();
                (lines, false)
            }
            _ => say("/team create <name> [colour], join <name>, leave, list, colour <colour>, friendlyfire on|off, remove <name>.".into()),
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut b = Vec::new();
        let put = |b: &mut Vec<u8>, s: &str| {
            b.push(s.len().min(255) as u8);
            b.extend_from_slice(&s.as_bytes()[..s.len().min(255)]);
        };
        b.push(self.teams.len().min(MAX_TEAMS) as u8);
        for t in self.teams.iter().take(MAX_TEAMS) {
            put(&mut b, &t.name);
            b.push(t.colour);
            b.push(t.friendly_fire as u8);
        }
        let members: Vec<_> = self.members.iter().take(4096).collect();
        b.extend_from_slice(&(members.len() as u16).to_le_bytes());
        for (p, t) in members {
            put(&mut b, p);
            put(&mut b, t);
        }
        b
    }

    pub fn decode(b: &[u8]) -> Option<Teams> {
        let mut out = Teams::default();
        // (Strings are a length byte, then UTF-8.)
        let mut r = Reader { b, at: 0 };
        let n = r.u8()?;
        for _ in 0..n.min(MAX_TEAMS as u8) {
            let name = clean_name(&r.str()?);
            let colour = r.u8()? % COLOURS.len() as u8;
            let friendly_fire = r.u8()? != 0;
            out.teams.push(Team { name, colour, friendly_fire });
        }
        let m = u16::from_le_bytes([r.u8()?, r.u8()?]);
        for _ in 0..m {
            let (p, t) = (r.str()?, r.str()?);
            out.members.insert(p.to_ascii_lowercase(), t);
        }
        Some(out)
    }
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn u8(&mut self) -> Option<u8> {
        let v = *self.b.get(self.at)?;
        self.at += 1;
        Some(v)
    }
    fn str(&mut self) -> Option<String> {
        let n = self.u8()? as usize;
        let s = self.b.get(self.at..self.at + n)?;
        self.at += n;
        Some(String::from_utf8_lossy(s).into_owned())
    }
}

// ------------------------------------------------------------------ in the game

use crate::game::Game;
use crate::net::{Msg, SYSTEM};

/// The hardest one blow can land (a Mace from a great height is less; this only stops nonsense).
const MAX_HIT: f32 = 40.0;
/// How far a player can reach another (a little more than the reach, for lag).
const HIT_RANGE: f32 = 6.5;

impl Game {
    pub fn teams_msg(&self) -> Msg {
        Msg::Teams { pvp: self.pvp, data: self.teams.encode() }
    }

    /// The host tells everyone about the teams (after a change).
    pub fn share_teams(&mut self) {
        if self.is_host() {
            let m = self.teams_msg();
            self.net_broadcast_all(m);
        }
    }

    /// A player's team colour, if they're on a team.
    pub fn team_colour(&self, player: &str) -> Option<[u8; 3]> {
        self.teams.team_of(player).map(Team::rgb)
    }

    /// `/tc`: say something to your team only (on the host).
    pub fn team_chat(&mut self, from: u32, text: &str) -> Vec<String> {
        let name = self.peer_name(from);
        let Some(team) = self.teams.team_of(&name).map(|t| t.name.clone()) else {
            return vec!["You're not on a team. /team join <name>, or /team create <name>.".into()];
        };
        let text: String = text.trim().chars().take(200).collect();
        if text.is_empty() {
            return vec!["/tc <message> talks to your team.".into()];
        }
        let line = format!("[{team}] {text}");
        let mates: Vec<u32> = self.all_peers().filter(|(_, p)| self.teams.same_team(&p.name, &name)).map(|(&id, _)| id).collect();
        for id in mates {
            if id != from {
                self.net_send_to(id, Msg::Chat { from, text: line.clone() });
            }
        }
        let host_on_team = !self.dedicated && self.teams.same_team(&self.player_name, &name);
        if from != self.my_id && from != SYSTEM {
            // The sender sees it too (they don't echo commands).
            self.net_send_to(from, Msg::Chat { from, text: line.clone() });
        }
        if host_on_team || from == self.my_id {
            self.msg(format!("<{name}> {line}"));
        }
        Vec::new()
    }

    /// `/pvp on|off`: can players hurt each other?
    pub fn pvp_command(&mut self, arg: &str) -> Vec<String> {
        match arg.trim().to_ascii_lowercase().as_str() {
            "on" | "true" => self.pvp = true,
            "off" | "false" => self.pvp = false,
            _ => return vec![format!("Players {} hurt each other. /pvp on or off.", if self.pvp { "can" } else { "can't" })],
        }
        self.share_teams();
        vec![if self.pvp { "Players can hurt each other now (teams decide about their own, see /team friendlyfire).".into() } else { "Players can't hurt each other now.".into() }]
    }

    /// Can `attacker` hurt `victim` (both players) right now?
    pub fn may_hit_player(&self, attacker: &str, victim: &str) -> bool {
        self.pvp && attacker != victim && self.teams.may_hurt(attacker, victim)
    }

    /// The host: player `from` hit player `target` for `dmg` (checked here).
    pub fn host_hit_player(&mut self, from: u32, target: u32, dmg: f32) {
        if !dmg.is_finite() || dmg <= 0.0 || from == target {
            return;
        }
        let (attacker, victim) = (self.peer_name(from), self.peer_name(target));
        if !self.may_hit_player(&attacker, &victim) {
            return;
        }
        let pos_of = |g: &Game, id: u32| if id == g.my_id && !g.dedicated { Some(g.player.body.pos) } else { g.peer_ref(id).filter(|p| p.alive()).map(|p| p.target) };
        let (Some(a), Some(v)) = (pos_of(self, from), pos_of(self, target)) else { return };
        if a.distance(v) > HIT_RANGE {
            return;
        }
        let dmg = dmg.min(MAX_HIT);
        let cause = format!("was struck down by {attacker}");
        let knock = (v - a).normalize_or_zero() * 4.0 + macroquad::math::Vec3::Y * 2.0;
        if target == self.my_id && !self.dedicated {
            self.hurt_player_from(dmg, &cause, Some(a), false);
            self.player.body.vel += knock;
        } else {
            self.hurt_peer(target, dmg, &cause, knock);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn teams_are_made_joined_left_and_kept_apart() {
        let mut t = Teams::default();
        let (out, changed) = t.command("Ann", "create Reds red", false);
        assert!(changed && out[0].contains("Made team Reds"), "{out:?}");
        assert_eq!(t.team_of("ann").map(|t| t.rgb()), Some(COLOURS[0].1));
        // The next team gets the next free colour.
        t.command("Bob", "create Blues", false);
        assert_eq!(t.team_of("Bob").unwrap().colour, 1);
        assert!(t.command("Cat", "create Reds", false).0[0].contains("already"));
        assert!(t.command("Cat", "join reds", false).1);
        assert!(t.same_team("Ann", "CAT") && !t.same_team("Ann", "Bob"));
        // Friendly fire: off at first, so teammates can't hurt each other.
        assert!(!t.may_hurt("Ann", "Cat") && t.may_hurt("Ann", "Bob") && t.may_hurt("Dan", "Ann"));
        t.command("Cat", "friendlyfire on", false);
        assert!(t.may_hurt("Ann", "Cat"));
        t.command("Ann", "colour aqua", false);
        assert_eq!(t.team_of("Cat").unwrap().colour, 6);
        assert!(t.command("Ann", "leave", false).1);
        assert!(t.team_of("Ann").is_none());
        // Only operators break teams up.
        assert!(!t.command("Cat", "remove Reds", false).1);
        assert!(t.command("Op", "remove Reds", true).1);
        assert!(t.team_of("Cat").is_none() && t.find("Reds").is_none());
        let (list, _) = t.command("Bob", "list", false);
        assert_eq!(list, vec!["Blues (blue): bob".to_string()]);
    }

    #[test]
    fn names_colours_and_limits_are_checked() {
        let mut t = Teams::default();
        assert!(t.create("!!!", None).is_err());
        assert_eq!(t.teams.len(), 0);
        assert!(t.command("A", "create Ok pinkish", false).0[0].starts_with("Colours"));
        let i = t.create("A very long team name indeed", None).unwrap();
        assert_eq!(t.teams[i].name.len(), NAME_LEN);
        for k in 1..MAX_TEAMS {
            t.create(&format!("t{k}"), None).unwrap();
        }
        assert!(t.create("more", None).is_err());
        assert_eq!(colour_named("Cyan"), Some(6));
    }

    #[test]
    fn players_hurt_each_other_only_when_allowed() {
        let mut g = crate::game::tests::arena(960);
        g.player_name = "Ann".into();
        let me = g.my_id;
        let near = g.player.body.pos + macroquad::math::Vec3::X;
        let mut bob = crate::multiplayer::Peer::new("Bob".into(), near);
        bob.target = near;
        g.peers.insert(7, bob);
        let health = g.player.health;
        // Nobody can be hurt until /pvp on.
        g.host_hit_player(7, me, 4.0);
        assert_eq!(g.player.health, health);
        assert!(g.admin_command(crate::admin::Caller::Host, "/pvp on").unwrap()[0].contains("can hurt"));
        g.host_hit_player(7, me, 4.0);
        assert!(g.player.health < health, "{}", g.player.health);
        // Teammates can't, until their team says they can.
        g.player.health = health;
        g.player.hurt = 0.0;
        g.admin_command(crate::admin::Caller::Host, "/team create Reds");
        g.teams.set_member("Bob", Some("Reds"));
        g.host_hit_player(7, me, 4.0);
        assert_eq!(g.player.health, health);
        g.admin_command(crate::admin::Caller::Host, "/team friendlyfire on");
        g.player.hurt = 0.0;
        g.host_hit_player(7, me, 4.0);
        assert!(g.player.health < health);
        // Too far away, or nonsense, isn't a hit.
        g.player.health = health;
        g.player.hurt = 0.0;
        g.peers.get_mut(&7).unwrap().target = near + macroquad::math::Vec3::X * 20.0;
        g.host_hit_player(7, me, 4.0);
        g.peers.get_mut(&7).unwrap().target = near;
        g.host_hit_player(7, me, f32::NAN);
        g.host_hit_player(7, 7, 4.0);
        assert_eq!(g.player.health, health);
        // Teams and the rule are kept with the world.
        let back = Game::from_save(g.to_save());
        assert!(back.pvp && back.teams == g.teams);
    }

    #[test]
    fn teams_round_trip_and_bad_bytes_are_refused() {
        let mut t = Teams::default();
        t.command("Ann", "create Reds red", false);
        t.command("Bob", "create Blues", false);
        t.command("Bob", "friendlyfire on", false);
        assert_eq!(Teams::decode(&t.encode()), Some(t.clone()));
        let b = t.encode();
        assert_eq!(Teams::decode(&b[..b.len() - 2]), None);
        assert_eq!(Teams::decode(&[]), None);
    }
}
