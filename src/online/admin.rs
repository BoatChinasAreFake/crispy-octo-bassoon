//! Running a server: an allow-list, operators, and a look at players' data.
//!
//! The same commands work from the dedicated server's console, from chat for
//! operators (`/kick Bob`), and for whoever hosts a LAN world (they're always
//! an operator). A dedicated server keeps its lists next to itself:
//!
//! * `allow-list.txt`: `enforce=yes` or `no`, then one name per line.
//! * `ops.txt`: one name per line.
//! * `banned-ips.txt`: one address per line.
//!
//! Names aren't accounts (see players.rs), so the allow-list keeps honest
//! strangers out; pair it with a password to keep everyone else out too.

use crate::block::*;
use crate::game::Game;
use crate::multiplayer::Net;
use crate::players::record_key;
use std::collections::{BTreeSet, HashSet};
use std::net::IpAddr;
use std::path::{Path, PathBuf};

pub const ALLOW_FILE: &str = "allow-list.txt";
pub const OPS_FILE: &str = "ops.txt";
pub const BAN_FILE: &str = "banned-ips.txt";

/// Who typed a command.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Caller {
    /// The dedicated server's console.
    Console,
    /// The player hosting this world.
    Host,
    /// A joined player.
    Player(u32),
}

#[derive(Default, Debug)]
pub struct Admin {
    /// Names (lower case) allowed in when `enforce` is on.
    pub allowed: BTreeSet<String>,
    pub enforce: bool,
    /// Names (lower case) that may run commands.
    pub ops: BTreeSet<String>,
    /// Where the lists live (only a dedicated server keeps them on disk).
    pub dir: Option<PathBuf>,
}

impl Admin {
    /// Read the lists from `dir` (missing files are empty lists).
    pub fn load(dir: &Path) -> Admin {
        let names = |file: &str| -> Vec<String> {
            std::fs::read_to_string(dir.join(file))
                .unwrap_or_default()
                .lines()
                .map(|l| l.split('#').next().unwrap_or("").trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        };
        let mut a = Admin { dir: Some(dir.to_path_buf()), ..Default::default() };
        for line in names(ALLOW_FILE) {
            match line.strip_prefix("enforce=") {
                Some(v) => a.enforce = matches!(v.trim(), "yes" | "on" | "true" | "1"),
                None => {
                    a.allowed.insert(record_key(&line));
                }
            }
        }
        a.ops = names(OPS_FILE).iter().map(|n| record_key(n)).collect();
        a
    }

    pub fn load_bans(dir: &Path) -> Vec<IpAddr> {
        std::fs::read_to_string(dir.join(BAN_FILE)).unwrap_or_default().lines().filter_map(|l| l.split('#').next()?.trim().parse().ok()).collect()
    }

    /// Write the lists back (a no-op without a directory). Returns a problem to report.
    fn save(&self, bans: Option<&HashSet<IpAddr>>) -> Option<String> {
        let dir = self.dir.as_ref()?;
        let list = |set: &BTreeSet<String>| set.iter().cloned().collect::<Vec<_>>().join("\n");
        let allow = format!(
            "# Players allowed to join, one name per line. With enforce=no anyone can join.\nenforce={}\n{}\n",
            if self.enforce { "yes" } else { "no" },
            list(&self.allowed)
        );
        let ops = format!("# Operators: players who may use server commands in chat.\n{}\n", list(&self.ops));
        let mut files = vec![(ALLOW_FILE, allow), (OPS_FILE, ops)];
        if let Some(bans) = bans {
            let mut ips: Vec<String> = bans.iter().map(|ip| ip.to_string()).collect();
            ips.sort();
            files.push((BAN_FILE, format!("# One IP address per line. Managed by the ban/unban commands.\n{}\n", ips.join("\n"))));
        }
        for (file, text) in files {
            if let Err(e) = std::fs::write(dir.join(file), text) {
                return Some(format!("Couldn't write {file}: {e}"));
            }
        }
        None
    }

    /// May this name come in?
    pub fn admits(&self, name: &str) -> bool {
        !self.enforce || self.allowed.contains(&record_key(name)) || self.ops.contains(&record_key(name))
    }

    pub fn is_op(&self, name: &str) -> bool {
        self.ops.contains(&record_key(name))
    }
}

pub const HELP: &str = "Commands: list, players, info <name>, say <text>, kick <name>, ban <name|ip>, unban <ip>, bans, \
time <day|noon|night|midnight|0-1>, op <name>, deop <name>, ops, allowlist <on|off|add|remove|list> [name], forget <name>, password <pw|off>";

/// A slot's contents as words: "Diamond Pickaxe (worn 12)", "Torch x16".
fn describe_stack(id: Id, n: u8, wear: crate::inventory::Wear) -> String {
    let mut s = block(id).name.split(" (").next().unwrap_or("?").to_string();
    if n > 1 {
        s += &format!(" x{n}");
    }
    if wear > 0 {
        s += &format!(" (worn {wear})");
    }
    s
}

/// The admin commands (without the slash); the cheats are in `cheats::WORDS`.
pub const WORDS: [&str; 17] = ["help", "list", "players", "info", "say", "kick", "ban", "unban", "bans", "time", "op", "deop", "ops", "allowlist", "whitelist", "forget", "password"];

impl Game {
    fn caller_name(&self, who: Caller) -> String {
        match who {
            Caller::Console => "Server".into(),
            Caller::Host => self.player_name.clone(),
            Caller::Player(id) => self.peer_name(id),
        }
    }

    fn caller_is_op(&self, who: Caller) -> bool {
        match who {
            Caller::Console | Caller::Host => true,
            Caller::Player(id) => self.peers.get(&id).is_some_and(|p| self.admin.is_op(&p.name)),
        }
    }

    /// Run an admin command (without its leading slash). None: not one of ours
    /// (so a script's or an unknown command); otherwise the lines to show whoever typed it.
    pub fn admin_command(&mut self, who: Caller, line: &str) -> Option<Vec<String>> {
        let line = line.trim().trim_start_matches('/');
        let (word, rest) = line.split_once(' ').map(|(a, b)| (a, b.trim())).unwrap_or((line, ""));
        let word = word.to_ascii_lowercase();
        let known = WORDS;
        let cheat = crate::cheats::WORDS.contains(&word.as_str());
        if !known.contains(&word.as_str()) && !cheat {
            return None;
        }
        let open = matches!(word.as_str(), "help" | "list" | "seed");
        if !open && !self.caller_is_op(who) {
            return Some(vec![format!("Only operators can use /{word}.")]);
        }
        let by = self.caller_name(who);
        if cheat {
            let out = self.cheat_command(who, &word, rest);
            if who != Caller::Console && !open && self.dedicated {
                println!("[{}] {by} ran /{line}", crate::server::timestamp());
            }
            return Some(out);
        }
        let mut out = Vec::new();
        let mut save = false;
        let mut save_bans = false;
        match (word.as_str(), rest) {
            ("help", _) => {
                if self.caller_is_op(who) {
                    out.push(HELP.into());
                    out.push(format!("Cheats: {}", crate::cheats::HELP));
                } else {
                    out.push("Commands: list, seed, help. Operators can do more.".into());
                }
                if !matches!(who, Caller::Console) {
                    out.push("Waypoints: /wp add <name>, /wp remove <name>, /wp list.".into());
                }
            }
            ("list", _) => {
                let mut names: Vec<String> = self.peers.values().map(|p| p.name.clone()).collect();
                if !self.dedicated && self.net.is_some() {
                    names.insert(0, self.player_name.clone());
                }
                out.push(format!("{} online: {}", names.len(), names.join(", ")));
            }
            ("players", _) => {
                let records = self.all_player_records();
                if records.is_empty() {
                    out.push("No players remembered yet.".into());
                }
                for (key, r) in &records {
                    let online = self.peers.values().any(|p| record_key(&p.name) == *key);
                    let op = if self.admin.is_op(key) { " [op]" } else { "" };
                    out.push(format!(
                        "{key}{op}: {} at {:.0}, {:.0}, {:.0}, health {:.0}, {} xp",
                        if online { "online" } else { "away" },
                        r.pos.x,
                        r.pos.y,
                        r.pos.z,
                        r.report.health,
                        r.xp
                    ));
                }
            }
            ("info", name) if !name.is_empty() => {
                let key = record_key(name);
                let live = self.peers.iter().find(|(_, p)| record_key(&p.name) == key).map(|(&id, _)| id);
                let record = live.and_then(|id| self.record_of(id)).map(|(_, r)| r).or_else(|| self.saved_players.get(&key).cloned());
                match record {
                    None => out.push(format!("Nobody called {name} has played here.")),
                    Some(r) => {
                        let rep = &r.report;
                        out.push(format!(
                            "{name}: {}, at {:.1}, {:.1}, {:.1}{}",
                            if live.is_some() { "online" } else { "away" },
                            r.pos.x,
                            r.pos.y,
                            r.pos.z,
                            if self.admin.is_op(name) { ", operator" } else { "" }
                        ));
                        out.push(format!(
                            "  health {:.1}/20, food {:.1}/20, saturation {:.1}, {} xp (level {}), enchanted {} time(s)",
                            rep.health,
                            rep.food,
                            rep.saturation,
                            r.xp,
                            crate::xp::level_of(r.xp).0,
                            r.enchant_count
                        ));
                        let (bag, armour) = rep.slots.split_at(rep.slots.len().min(36));
                        let things: Vec<String> = bag.iter().filter(|s| s.1 > 0).map(|&(id, n, w)| describe_stack(id, n, w)).collect();
                        let worn: Vec<String> = armour.iter().filter(|s| s.1 > 0).map(|&(id, n, w)| describe_stack(id, n, w)).collect();
                        out.push(format!("  carrying: {}", if things.is_empty() { "nothing".into() } else { things.join(", ") }));
                        out.push(format!("  wearing: {}", if worn.is_empty() { "nothing".into() } else { worn.join(", ") }));
                    }
                }
            }
            ("say", text) if !text.is_empty() => {
                let text = if who == Caller::Console { text.to_string() } else { format!("{text} (from {by})") };
                self.msg(format!("[Server] {text}"));
                self.system_message(None, &format!("[Server] {text}"));
            }
            ("kick", name) if !name.is_empty() => {
                if self.kick_player(name, &format!("Kicked by {by}.")) {
                    out.push(format!("Kicked {name}."));
                } else {
                    out.push(format!("No player called {name}."));
                }
            }
            ("ban", target) if !target.is_empty() => {
                let by_name = self.peer_by_name(target);
                match &mut self.net {
                    Some(Net::Host(s)) => {
                        let ip = match by_name {
                            Some(id) => s.ip_of(id),
                            None => target.parse().ok(),
                        };
                        match ip {
                            Some(ip) => {
                                s.banned.insert(ip);
                                let ids: Vec<u32> = s.clients.iter().filter(|c| c.conn.peer_ip() == Some(ip)).map(|c| c.id).collect();
                                for id in ids {
                                    s.kick(id, "You have been banned from this server.");
                                }
                                save_bans = true;
                                out.push(format!("Banned {ip}."));
                            }
                            None => out.push(format!("No player or address called {target}.")),
                        }
                    }
                    _ => out.push("Nobody can join this world, so there's nobody to ban.".into()),
                }
            }
            ("unban", ip) if !ip.is_empty() => {
                let removed = match (&mut self.net, ip.parse::<IpAddr>()) {
                    (Some(Net::Host(s)), Ok(addr)) => s.banned.remove(&addr),
                    _ => false,
                };
                if removed {
                    save_bans = true;
                    out.push(format!("Unbanned {ip}."));
                } else {
                    out.push(format!("{ip} isn't banned (unban takes an IP address; see bans)."));
                }
            }
            ("bans", _) => {
                let list: Vec<String> = match &self.net {
                    Some(Net::Host(s)) => s.banned.iter().map(|ip| ip.to_string()).collect(),
                    _ => Vec::new(),
                };
                out.push(format!("{} banned: {}", list.len(), list.join(", ")));
            }
            ("time", t) => {
                let t = match t {
                    "day" => Some(0.05),
                    "noon" => Some(0.25),
                    "night" => Some(0.55),
                    "midnight" => Some(0.75),
                    v => v.parse::<f32>().ok().filter(|t| t.is_finite()),
                };
                match t {
                    Some(t) => {
                        self.time = t.rem_euclid(1.0);
                        self.net_broadcast(self.time_msg());
                        out.push(format!("Time set to {:.2}.", self.time));
                    }
                    None => out.push("Usage: time <day|noon|night|midnight|0.0-1.0>".into()),
                }
            }
            ("op", name) if !name.is_empty() => {
                self.admin.ops.insert(record_key(name));
                save = true;
                out.push(format!("{name} is now an operator."));
                if let Some(id) = self.peer_by_name(name) {
                    self.system_message(Some(id), &format!("{by} made you an operator. Try /help."));
                }
            }
            ("deop", name) if !name.is_empty() => {
                if self.admin.ops.remove(&record_key(name)) {
                    save = true;
                    out.push(format!("{name} is no longer an operator."));
                } else {
                    out.push(format!("{name} wasn't an operator."));
                }
            }
            ("ops", _) => out.push(format!("Operators: {}", if self.admin.ops.is_empty() { "none".into() } else { self.admin.ops.iter().cloned().collect::<Vec<_>>().join(", ") })),
            ("allowlist" | "whitelist", args) => {
                let (sub, name) = args.split_once(' ').map(|(a, b)| (a, b.trim())).unwrap_or((args, ""));
                match (sub, name) {
                    ("on", _) => {
                        self.admin.enforce = true;
                        // Don't lock out whoever is here already.
                        let here: Vec<String> = self.peers.values().map(|p| record_key(&p.name)).collect();
                        self.admin.allowed.extend(here);
                        save = true;
                        out.push(format!("Allow-list on: only the {} listed player(s) (and operators) can join.", self.admin.allowed.len()));
                    }
                    ("off", _) => {
                        self.admin.enforce = false;
                        save = true;
                        out.push("Allow-list off: anyone can join.".into());
                    }
                    ("add", n) if !n.is_empty() => {
                        self.admin.allowed.insert(record_key(n));
                        save = true;
                        out.push(format!("Added {n} to the allow-list."));
                    }
                    ("remove", n) if !n.is_empty() => {
                        if self.admin.allowed.remove(&record_key(n)) {
                            save = true;
                            out.push(format!("Took {n} off the allow-list."));
                        } else {
                            out.push(format!("{n} wasn't on the allow-list."));
                        }
                    }
                    ("list" | "", _) => out.push(format!(
                        "Allow-list is {}: {}",
                        if self.admin.enforce { "on" } else { "off" },
                        if self.admin.allowed.is_empty() { "nobody listed".into() } else { self.admin.allowed.iter().cloned().collect::<Vec<_>>().join(", ") }
                    )),
                    _ => out.push("Usage: allowlist <on|off|list|add <name>|remove <name>>".into()),
                }
            }
            ("forget", name) if !name.is_empty() => {
                if self.peer_by_name(name).is_some() {
                    out.push(format!("{name} is online; kick them first."));
                } else if self.saved_players.remove(&record_key(name)).is_some() {
                    out.push(format!("Forgot {name}: next time they start afresh."));
                } else {
                    out.push(format!("Nobody called {name} is remembered."));
                }
            }
            ("password", pw) => {
                let pw = if pw.is_empty() || pw == "off" { None } else { Some(pw.to_string()) };
                let on = pw.is_some();
                match &mut self.net {
                    Some(Net::Host(s)) => {
                        s.password = pw;
                        out.push(if on { "Password set (applies to new logins).".into() } else { "Password removed: anyone can join.".into() });
                    }
                    _ => out.push("This world isn't open to anyone.".into()),
                }
            }
            _ => out.push(format!("Usage: see /help (/{word} needs more).")),
        }
        if save || save_bans {
            let bans = match &self.net {
                Some(Net::Host(s)) if save_bans => Some(s.banned.clone()),
                _ => None,
            };
            if let Some(problem) = self.admin.save(bans.as_ref()) {
                out.push(problem);
            }
        }
        // Operators' actions leave a trace in the server log.
        if who != Caller::Console && !open && self.dedicated {
            println!("[{}] {by} ran /{line}", crate::server::timestamp());
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_round_trip_through_files() {
        let dir = std::env::temp_dir().join(format!("minceraft-admin-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut a = Admin { dir: Some(dir.clone()), enforce: true, ..Default::default() };
        a.allowed.insert("alice".into());
        a.ops.insert("bob".into());
        let bans: HashSet<IpAddr> = ["10.0.0.7".parse().unwrap()].into_iter().collect();
        assert_eq!(a.save(Some(&bans)), None);
        let back = Admin::load(&dir);
        assert!(back.enforce);
        assert_eq!(back.allowed, a.allowed);
        assert_eq!(back.ops, a.ops);
        assert_eq!(Admin::load_bans(&dir), vec!["10.0.0.7".parse::<IpAddr>().unwrap()]);
        // Operators get in even when they aren't listed; others don't.
        assert!(back.admits("ALICE") && back.admits("Bob") && !back.admits("Mallory"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn commands_need_an_operator() {
        let mut g = Game::new(5, false, false);
        assert_eq!(g.admin_command(Caller::Host, "/dance"), None);
        let out = g.admin_command(Caller::Host, "/time night").unwrap();
        assert_eq!(out, vec!["Time set to 0.55.".to_string()]);
        assert_eq!(g.time, 0.55);
        g.admin_command(Caller::Console, "op Stove");
        assert!(g.admin.is_op("stove"));
        g.admin_command(Caller::Host, "allowlist add Pan");
        g.admin_command(Caller::Host, "allowlist on");
        assert!(g.admin.admits("pan") && !g.admin.admits("pot"));
        let out = g.admin_command(Caller::Host, "info nobody").unwrap();
        assert!(out[0].contains("Nobody called"));
    }
}
