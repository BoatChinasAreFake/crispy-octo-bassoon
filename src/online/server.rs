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
    --keep-inventory    Players keep their things when they die (default: they drop them)
    --difficulty <D>    peaceful, easy, normal or hard (default: the world's own, normal if new)
    --max-players <N>   Player limit (default 16)
    --allow-list        Only let in players on allow-list.txt (and operators)
    --upnp              Ask your router to forward the port (for home servers)
    --help              Show this help

CONSOLE COMMANDS:
    list, players, info <name>, say <text>, kick <name>, save, stop, help,
    time <day|noon|night|midnight|0.0-1.0>, password <pw|off>,
    ban <name|ip>, unban <ip>, bans, op <name>, deop <name>, ops,
    allowlist <on|off|list|add <name>|remove <name>>, forget <name>

Operators (ops.txt) can use the same commands in chat: /kick Bob.
The allow-list (allow-list.txt) and bans (banned-ips.txt, by IP address)
live next to the server. Five wrong passwords from one address lock it
out for ten minutes.";

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
    if args.iter().any(|a| a == "--keep-inventory") {
        game.rules.keep_inventory = true;
    }
    if let Some(d) = get("--difficulty") {
        match crate::rules::Difficulty::ALL.iter().find(|x| x.name().to_lowercase().starts_with(&d.to_lowercase())) {
            Some(x) => game.rules.difficulty = *x,
            None => log(&format!("Unknown difficulty '{d}' (try peaceful, easy, normal or hard); keeping {}", game.rules.difficulty.name())),
        }
    }
    game.admin = crate::admin::Admin::load(std::path::Path::new("."));
    if args.iter().any(|a| a == "--allow-list") {
        game.admin.enforce = true;
    }
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
    let bans = crate::admin::Admin::load_bans(std::path::Path::new("."));
    if !bans.is_empty() {
        log(&format!("{} banned address(es) loaded from {}", bans.len(), crate::admin::BAN_FILE));
    }
    if game.admin.enforce {
        log(&format!("Allow-list on: {} player(s) listed in {}", game.admin.allowed.len(), crate::admin::ALLOW_FILE));
    }
    if !game.admin.ops.is_empty() {
        log(&format!("Operators: {}", game.admin.ops.iter().cloned().collect::<Vec<_>>().join(", ")));
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

    game.use_regions(crate::regions::region_dir(&world_path));
    let save_world = |game: &mut Game| match game.flush_regions().and_then(|_| save::write_to(&world_path, &game.to_save())) {
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
            let word = cmd.split_whitespace().next().unwrap_or("");
            match word {
                "" => {}
                "help" | "?" => println!("{HELP}"),
                "stop" | "exit" | "quit" => break 'outer,
                "save" => save_world(&mut game),
                // "/something" goes to script mods as chat from "Server".
                _ if cmd.starts_with('/') => {
                    if game.fire("on_chat", vec!["Server".into(), cmd.clone().into()]) {
                        log(&format!("No script handled {cmd}"));
                    }
                }
                _ => match game.admin_command(crate::admin::Caller::Console, &cmd) {
                    Some(lines) => lines.iter().for_each(|l| log(l)),
                    None => log(&format!("Unknown command \"{cmd}\". Type help, or /command for script mods.")),
                },
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
