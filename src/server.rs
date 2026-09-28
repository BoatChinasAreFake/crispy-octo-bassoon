//! Headless dedicated server: `minceraft --server [options]`.
//! No window, no GPU, no audio: just the world, the mobs and the network.
//! Run it on a VPS or a spare machine so friends can play over the internet.

use crate::game::Game;
use crate::multiplayer::Net;
use crate::net::DEFAULT_PORT;
use crate::save;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

const TICK: f32 = 0.05; // 20 ticks per second
const AUTOSAVE_SECS: f32 = 300.0;

pub const HELP: &str = "\
Minceraft dedicated server

USAGE:
    minceraft --server [OPTIONS]

OPTIONS:
    --port <N>          TCP port to listen on (default 25565)
    --password <PW>     Require this password to join
    --world <FILE>      World save file (default saves/server.mncr; created if missing)
    --seed <N>          Seed for a new world
    --creative          New worlds are creative (default survival)
    --max-players <N>   Player limit (default 16)
    --upnp              Ask your router to forward the port (for home servers)
    --help              Show this help

CONSOLE COMMANDS:
    list, say <text>, kick <name>, save, time <day|night|0.0-1.0>,
    password <pw|off>, ban <name|ip>, unban <ip>, bans, help, stop

Bans are by IP address and kept in banned-ips.txt next to the server.
Five wrong passwords from one address lock it out for ten minutes.";

const BAN_FILE: &str = "banned-ips.txt";

fn load_bans() -> Vec<std::net::IpAddr> {
    std::fs::read_to_string(BAN_FILE).unwrap_or_default().lines().filter_map(|l| l.split('#').next()?.trim().parse().ok()).collect()
}

fn save_bans(bans: &std::collections::HashSet<std::net::IpAddr>) {
    let mut list: Vec<String> = bans.iter().map(|ip| ip.to_string()).collect();
    list.sort();
    let text = format!("# One IP address per line. Managed by the server's ban/unban commands.\n{}\n", list.join("\n"));
    if let Err(e) = std::fs::write(BAN_FILE, text) {
        log(&format!("Couldn't write {BAN_FILE}: {e}"));
    }
}

pub fn timestamp() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!("{:02}:{:02}:{:02}", (secs / 3600) % 24, (secs / 60) % 60, secs % 60)
}

fn log(s: &str) {
    println!("[{}] {s}", timestamp());
}

pub fn run(args: &[String]) -> i32 {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{HELP}");
        return 0;
    }
    let get = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let port: u16 = get("--port").and_then(|p| p.parse().ok()).unwrap_or(DEFAULT_PORT);
    let password = get("--password");
    let world_path = PathBuf::from(get("--world").unwrap_or_else(|| "saves/server.mncr".into()));
    let max_players: usize = get("--max-players").and_then(|p| p.parse().ok()).unwrap_or(16);
    let creative = args.iter().any(|a| a == "--creative");

    let infos = crate::mods::install_local();
    for m in infos.iter().filter(|m| m.enabled) {
        let (b, i, r) = m.added;
        log(&format!("Mod {} {}: {b} blocks, {i} items, {r} recipes (sent to players when they join)", m.name, m.version));
        for e in &m.errors {
            log(&format!("  mod {} problem: {e}", m.id));
        }
    }

    let mut game = match save::read_from(&world_path) {
        Ok(d) => {
            log(&format!("Loaded world {} (seed {})", world_path.display(), d.seed));
            Game::from_save(d)
        }
        Err(_) => {
            let seed = get("--seed").and_then(|s| s.parse().ok()).unwrap_or_else(|| {
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos() ^ d.as_secs() as u32).unwrap_or(1)
            });
            log(&format!("Creating new {} world at {} (seed {seed})", if creative { "creative" } else { "survival" }, world_path.display()));
            Game::new(seed, creative, false)
        }
    };
    game.dedicated = true;
    game.ready = true;
    game.messages.clear();
    game.start_scripts();

    let port = match game.open_server(port, password.clone(), max_players) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Couldn't listen on port {port}: {e}");
            return 1;
        }
    };
    log(&format!("Listening on TCP port {port} (IPv4{}), max {max_players} players", if crate::net::public_ipv6().is_some() { " + IPv6" } else { "" }));
    let bans = load_bans();
    if !bans.is_empty() {
        log(&format!("{} banned address(es) loaded from {BAN_FILE}", bans.len()));
    }
    if let Some(Net::Host(s)) = &mut game.net {
        s.banned.extend(bans);
    }
    log(if password.is_some() { "Password required to join" } else { "No password: anyone who can reach this port can join (use --password)" });
    if let Some(ip) = crate::net::lan_ip() {
        log(&format!("LAN address: {ip}:{port}"));
    }
    if let Some(ip6) = crate::net::public_ipv6() {
        log(&format!("Public IPv6 address: [{ip6}]:{port}"));
    }
    let mapping = if args.iter().any(|a| a == "--upnp") {
        log("Asking the router to forward the port (UPnP)...");
        match crate::upnp::open_port(port) {
            Ok(m) => {
                match &m.external_ip {
                    Some(ip) if m.behind_second_nat() => log(&format!(
                        "Port forwarded, but the router's own address ({ip}) is private: your ISP uses carrier-grade NAT, so people outside can't reach it. Use IPv6 or a VPS."
                    )),
                    Some(ip) => log(&format!("Internet address: {ip}:{port}")),
                    None => log("Port forwarded (the router didn't say its public address)."),
                }
                Some(m)
            }
            Err(e) => {
                log(&format!("UPnP failed: {e} Forward TCP port {port} manually if players can't connect."));
                None
            }
        }
    } else {
        None
    };
    log("Type \"help\" for commands.");

    // Console input on its own thread; EOF (e.g. running as a service) just ends it.
    let (tx, rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        let mut line = String::new();
        while stdin.read_line(&mut line).map(|n| n > 0).unwrap_or(false) {
            if tx.send(line.trim().to_string()).is_err() {
                break;
            }
            line.clear();
        }
    });

    let save_world = |game: &mut Game| match save::write_to(&world_path, &game.to_save()) {
        Ok(()) => log(&format!("Saved {}", world_path.display())),
        Err(e) => log(&format!("SAVE FAILED: {e}")),
    };

    let mut next = Instant::now();
    let mut since_save = 0.0f32;
    'outer: loop {
        game.server_tick(TICK);
        since_save += TICK;
        if since_save >= AUTOSAVE_SECS {
            since_save = 0.0;
            save_world(&mut game);
        }
        while let Ok(cmd) = rx.try_recv() {
            let (word, rest) = cmd.split_once(' ').map(|(a, b)| (a, b.trim())).unwrap_or((cmd.as_str(), ""));
            match word {
                "" => {}
                "help" | "?" => println!("{HELP}"),
                "stop" | "exit" | "quit" => break 'outer,
                "save" => save_world(&mut game),
                "list" => {
                    let names: Vec<&str> = game.peers.values().map(|p| p.name.as_str()).collect();
                    log(&format!("{} player(s) online: {}", names.len(), names.join(", ")));
                }
                "say" if !rest.is_empty() => game.send_chat(rest),
                "kick" if !rest.is_empty() => {
                    if !game.kick_player(rest, "Kicked by the server operator.") {
                        log(&format!("No player called {rest}"));
                    }
                }
                "time" => {
                    let t = match rest {
                        "day" => Some(0.05),
                        "noon" => Some(0.25),
                        "night" => Some(0.55),
                        "midnight" => Some(0.75),
                        v => v.parse::<f32>().ok(),
                    };
                    match t {
                        Some(t) => {
                            game.time = t.rem_euclid(1.0);
                            game.net_broadcast(crate::net::Msg::Time(game.time));
                            log(&format!("Time set to {:.2}", game.time));
                        }
                        None => log("Usage: time <day|noon|night|midnight|0.0-1.0>"),
                    }
                }
                "ban" if !rest.is_empty() => {
                    // A player's name, or an address.
                    let by_name = game.peer_by_name(rest);
                    if let Some(Net::Host(s)) = &mut game.net {
                        let ip = match by_name {
                            Some(id) => s.ip_of(id),
                            None => rest.parse().ok(),
                        };
                        match ip {
                            Some(ip) => {
                                s.banned.insert(ip);
                                save_bans(&s.banned);
                                let ids: Vec<u32> = s.clients.iter().filter(|c| c.conn.peer_ip() == Some(ip)).map(|c| c.id).collect();
                                for id in ids {
                                    s.kick(id, "You have been banned from this server.");
                                }
                                log(&format!("Banned {ip}"));
                            }
                            None => log(&format!("No player or address called {rest}")),
                        }
                    }
                }
                "unban" if !rest.is_empty() => {
                    if let Some(Net::Host(s)) = &mut game.net {
                        match rest.parse::<std::net::IpAddr>() {
                            Ok(ip) if s.banned.remove(&ip) => {
                                save_bans(&s.banned);
                                log(&format!("Unbanned {ip}"));
                            }
                            _ => log(&format!("{rest} isn't banned (unban takes an IP address; see 'bans')")),
                        }
                    }
                }
                "bans" => {
                    if let Some(Net::Host(s)) = &game.net {
                        let list: Vec<String> = s.banned.iter().map(|ip| ip.to_string()).collect();
                        log(&format!("{} banned: {}", list.len(), list.join(", ")));
                    }
                }
                "password" => {
                    let pw = if rest.is_empty() || rest == "off" { None } else { Some(rest.to_string()) };
                    let on = pw.is_some();
                    if let Some(Net::Host(s)) = &mut game.net {
                        s.password = pw;
                    }
                    log(if on { "Password set (applies to new logins)." } else { "Password removed: anyone can join." });
                }
                // "/something" goes to script mods as chat from "Server".
                _ if cmd.starts_with('/') => {
                    if game.fire("on_chat", vec!["Server".into(), cmd.clone().into()]) {
                        log(&format!("No script handled {cmd}"));
                    }
                }
                _ => log(&format!("Unknown command \"{cmd}\". Type help, or /command for script mods.")),
            }
        }
        next += Duration::from_secs_f32(TICK);
        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        } else if now - next > Duration::from_secs(2) {
            next = now; // we fell far behind (machine asleep?); don't try to catch up
        }
    }

    log("Stopping server...");
    if let Some(Net::Host(s)) = &mut game.net {
        let ids: Vec<u32> = s.clients.iter().map(|c| c.id).collect();
        for id in ids {
            s.kick(id, "The server is shutting down.");
        }
    }
    save_world(&mut game);
    if let Some(m) = mapping {
        crate::upnp::close(&m);
    }
    0
}
