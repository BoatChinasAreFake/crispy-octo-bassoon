//! `minceraft --playtest`: a multiplayer smoke test in one command.
//!
//! It opens a world as a host on this computer and connects a few bot
//! players to it over real sockets, all in this one process and without a
//! window. For a while the bots wander about, build and knock down cobblestone
//! towers, chat, and put things in a shared chest. Then everyone settles and
//! the playtest checks that they all agree:
//!
//! - every block the bots touched is the same for the host and each bot;
//! - each bot sees all the others;
//! - every bot heard every other bot's chat;
//! - each bot's inventory matches the host's ledger of what it really owns;
//! - the chest holds the same things for everyone who looks;
//! - nobody was kicked.
//!
//! It prints what it did and what didn't match, and exits with 0 if all is
//! well (1 otherwise), so it can run in CI or before a release.
//!
//! Options: `--bots N` (default 3), `--seconds S` (default 20), `--seed N`, `--port N`.

use crate::block::*;
use crate::game::{Controls, Game};
use crate::net::{auth_proof, Conn, Msg, PROTOCOL};
use crate::noise::Rng;
use crate::player::Input;
use macroquad::math::{IVec3, Vec3};
use std::collections::HashSet;
use std::time::{Duration, Instant};

const DT: f32 = 1.0 / 60.0;

fn idle() -> Controls {
    Controls {
        input: Input { forward: 0.0, strafe: 0.0, jump: false, jump_pressed: false, sneak: false, sprint: false },
        attack_held: false,
        attack_pressed: false,
        use_held: false,
        use_pressed: false,
        pick: false,
        drop: false,
        drop_all: false,
    }
}

/// Load the chunks around a spot (so a game can be played there).
fn load_around(g: &mut Game, p: Vec3) {
    let start = Instant::now();
    while g.world.chunks.len() < 25 && start.elapsed() < Duration::from_secs(20) {
        g.world.stream(&[(p, 2)]);
        std::thread::sleep(Duration::from_millis(2));
    }
    g.ready = true;
}

/// Join the host as `name` (driving the host along while it answers).
pub fn connect(host: &mut Game, port: u16, name: &str) -> Result<Game, String> {
    let mut conn = Conn::connect(&format!("127.0.0.1:{port}")).map_err(|e| e.to_string())?;
    conn.send(&Msg::Hello { protocol: PROTOCOL, name: name.into() });
    conn.flush();
    let start = Instant::now();
    let (welcome, leftover) = loop {
        host.update(DT, &idle());
        let mut msgs = conn.poll();
        if let Some(Msg::Kick { reason }) = msgs.iter().find(|m| matches!(m, Msg::Kick { .. })) {
            return Err(reason.clone());
        }
        if let Some(Msg::Challenge { nonce, .. }) = msgs.iter().find(|m| matches!(m, Msg::Challenge { .. })) {
            conn.send(&Msg::Auth { proof: auth_proof(nonce, "") });
            conn.flush();
        }
        if let Some(i) = msgs.iter().position(|m| matches!(m, Msg::Welcome { .. })) {
            let rest = msgs.split_off(i + 1);
            break (msgs.pop().expect("found above"), rest);
        }
        if conn.closed.is_some() || start.elapsed() > Duration::from_secs(10) {
            return Err(conn.closed.clone().unwrap_or_else(|| "no welcome".into()));
        }
        std::thread::sleep(Duration::from_millis(2));
    };
    let Msg::Welcome { id, seed, time, creative, spawn, .. } = welcome else { unreachable!() };
    let mut g = Game::new_client(id, seed, time, creative, spawn, conn, name, leftover);
    load_around(&mut g, spawn);
    Ok(g)
}

/// What one bot is up to.
struct Bot {
    game: Game,
    name: String,
    rng: Rng,
    /// Where it's heading, and the towers it has built.
    heading: f32,
    towers: Vec<IVec3>,
    next_act: f32,
    /// How many things it has done (it takes turns at each kind).
    acts: u32,
    said: Vec<String>,
}

/// What happened, and what didn't match.
#[derive(Default, Debug)]
pub struct Report {
    pub placed: u32,
    pub broken: u32,
    pub chats: u32,
    pub deposits: u32,
    pub problems: Vec<String>,
}

impl Report {
    pub fn ok(&self) -> bool {
        self.problems.is_empty()
    }
}

/// Run a playtest: `bots` players for `seconds` of game time on a world from `seed`.
pub fn playtest(bots: usize, seconds: f32, seed: u32, port: u16) -> Result<Report, String> {
    let mut report = Report::default();
    let mut host = Game::new(seed, false, false);
    let spawn = host.spawn;
    load_around(&mut host, spawn);
    let port = host.open_server(port, None, bots + 4).map_err(|e| format!("couldn't host: {e}"))?;
    host.player_name = "Host".into();
    // A shared chest by spawn, on solid ground.
    let chest = IVec3::new(spawn.x.floor() as i32 + 2, spawn.y.floor() as i32, spawn.z.floor() as i32 + 2);
    host.world.set_v(chest - IVec3::Y, STONE);
    host.world.set_v(chest, CHEST);

    let mut team = Vec::new();
    for k in 0..bots {
        let name = format!("Bot{}", k + 1);
        let game = connect(&mut host, port, &name).map_err(|e| format!("{name} couldn't join: {e}"))?;
        team.push(Bot { game, name, rng: Rng::new(seed as u64 * 31 + k as u64 + 1), heading: k as f32 * 2.1, towers: Vec::new(), next_act: 1.0 + k as f32 * 0.1, acts: 0, said: Vec::new() });
    }
    // Everyone starts with some cobblestone (given by the host, so its ledger knows).
    let ids: Vec<u32> = team.iter().map(|b| b.game.my_id).collect();
    for id in &ids {
        host.give_peer(*id, COBBLE, 48);
        host.give_peer(*id, DIAMOND, 3);
    }

    let mut touched: HashSet<IVec3> = HashSet::new();
    touched.insert(chest);
    let steps = (seconds / DT) as usize;
    for step in 0..steps {
        host.update(DT, &idle());
        for bot in team.iter_mut() {
            let t = step as f32 * DT;
            let mut c = idle();
            // Wander in a loose circle round spawn, turning now and then.
            let home = spawn - bot.game.player.body.pos;
            if Vec3::new(home.x, 0.0, home.z).length() > 10.0 {
                bot.heading = home.x.atan2(-home.z);
            } else if bot.rng.chance(DT * 0.5) {
                bot.heading += bot.rng.range(-1.5, 1.5);
            }
            bot.game.player.yaw = bot.heading;
            c.input.forward = if (t % 6.0) < 4.0 { 1.0 } else { 0.0 };
            c.input.jump = bot.game.player.body.on_ground && bot.rng.chance(DT * 0.8);
            if t >= bot.next_act {
                bot.next_act = t + bot.rng.range(0.3, 0.6);
                act(bot, &mut report, &mut touched, chest);
            }
            bot.game.update(DT, &c);
        }
        std::thread::sleep(Duration::from_micros(300));
    }
    // Let everything arrive.
    for _ in 0..(3.0 / DT) as usize {
        host.update(DT, &idle());
        for bot in team.iter_mut() {
            bot.game.update(DT, &idle());
        }
        std::thread::sleep(Duration::from_micros(300));
    }
    check(&host, &team, &touched, chest, &mut report);
    Ok(report)
}

/// Do something: build, knock down, chat, or put something in the chest.
fn act(bot: &mut Bot, report: &mut Report, touched: &mut HashSet<IVec3>, chest: IVec3) {
    let g = &mut bot.game;
    let feet = g.player.body.pos.floor().as_ivec3();
    bot.acts += 1;
    match bot.acts % 6 {
        // Build: a block beside it (on the ground, somewhere empty).
        0 | 1 => {
            let side = [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z][bot.rng.int(0, 3) as usize] * 2;
            let at = feet + side;
            if g.inv.count(COBBLE) > 0 && replaceable(g.world.get_v(at)) && is_solid(g.world.get_v(at - IVec3::Y)) && at != chest {
                g.world.set_v(at, COBBLE);
                g.inv.remove(COBBLE, 1);
                bot.towers.push(at);
                touched.insert(at);
                report.placed += 1;
            }
        }
        // Knock down one of its own blocks (if it's still near).
        2 => {
            if let Some(i) = bot.towers.iter().position(|p| p.as_vec3().distance(g.player.body.pos) < 4.0) {
                let at = bot.towers.swap_remove(i);
                if g.world.get_v(at) == COBBLE {
                    g.world.set_v(at, AIR);
                    touched.insert(at);
                    report.broken += 1;
                }
            }
        }
        // Say something.
        3 => {
            let line = format!("{} says hi #{}", bot.name, bot.said.len() + 1);
            g.send_chat(&line);
            bot.said.push(line);
            report.chats += 1;
        }
        // Put a diamond in the shared chest, if close enough.
        _ => {
            if g.inv.count(DIAMOND) > 0 && chest.as_vec3().distance(g.player.body.pos) < 6.0 {
                g.open_container(chest);
                if let Some(slot) = g.inv.slots.iter().position(|s| s.is_some_and(|s| s.0 == DIAMOND)) {
                    // Only one at a time: split the stack onto the cursor, then drop it in.
                    if let Some((id, n)) = g.inv.slots[slot] {
                        g.inv.slots[slot] = if n > 1 { Some((id, n - 1)) } else { None };
                        g.inv.cursor = Some((id, 1));
                        let free = crate::containers::store_ref(&g.world, &g.vehicles, chest).and_then(|c| c.slots.iter().position(|s| s.is_none()));
                        if let Some(free) = free {
                            g.container_click(free, false, false);
                            report.deposits += 1;
                        }
                        g.inv.return_cursor();
                    }
                }
                g.close_container();
            }
        }
    }
}

/// Does everyone agree?
fn check(host: &Game, team: &[Bot], touched: &HashSet<IVec3>, chest: IVec3, report: &mut Report) {
    let mut cells: Vec<&IVec3> = touched.iter().collect();
    cells.sort_by_key(|p| (p.x, p.y, p.z));
    for bot in team {
        let g = &bot.game;
        if let Some(e) = &g.net_error {
            report.problems.push(format!("{} was disconnected: {e}", bot.name));
            continue;
        }
        let differ: Vec<String> = cells
            .iter()
            .filter(|p| g.world.is_loaded(p.x, p.z) && g.world.get_v(***p) != host.world.get_v(***p))
            .map(|p| format!("{} has {} where the host has {}", p, block(g.world.get_v(**p)).name, block(host.world.get_v(**p)).name))
            .collect();
        if !differ.is_empty() {
            report.problems.push(format!("{}: {} of {} blocks differ from the host's, e.g. {}", bot.name, differ.len(), cells.len(), differ[0]));
        }
        for other in team.iter().filter(|o| o.name != bot.name) {
            if !g.peers.values().any(|p| p.name == other.name) {
                report.problems.push(format!("{} can't see {}", bot.name, other.name));
            }
            let heard = other.said.iter().filter(|line| g.chat_log.iter().any(|l| l.contains(line.as_str()))).count();
            if heard < other.said.len() {
                report.problems.push(format!("{} heard {} of {}'s {} lines", bot.name, heard, other.name, other.said.len()));
            }
        }
        if let Some(peer) = host.peers.get(&g.my_id) {
            for item in [COBBLE, DIAMOND] {
                let (mine, ledger) = (g.inv.count(item), peer.ledger.bag.count(item));
                if mine != ledger {
                    report.problems.push(format!("{} holds {mine} {} but the host's ledger says {ledger}", bot.name, block(item).name));
                }
            }
        } else {
            report.problems.push(format!("the host doesn't know {}", bot.name));
        }
    }
    let in_chest = |c: Option<&crate::containers::Container>| c.map(|c| c.slots.iter().flatten().filter(|s| s.0 == DIAMOND).map(|s| s.1 as u32).sum::<u32>()).unwrap_or(0);
    let host_has = in_chest(host.world.containers.get(&chest));
    if host_has != report.deposits {
        report.problems.push(format!("the chest holds {host_has} diamonds for the host, but {} went in", report.deposits));
    }
}

/// The `--playtest` command.
pub fn run(args: &[String]) -> i32 {
    let get = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).and_then(|v| v.parse::<f64>().ok());
    let bots = get("--bots").map(|v| v as usize).unwrap_or(3).clamp(1, 12);
    let seconds = get("--seconds").map(|v| v as f32).unwrap_or(20.0).clamp(2.0, 600.0);
    let seed = get("--seed").map(|v| v as u32).unwrap_or(1234);
    let port = get("--port").map(|v| v as u16).unwrap_or(crate::net::DEFAULT_PORT);
    println!("Playtest: a host and {bots} bots for {seconds} seconds (seed {seed})...");
    let started = Instant::now();
    match playtest(bots, seconds, seed, port) {
        Err(e) => {
            println!("FAILED to start: {e}");
            1
        }
        Ok(r) => {
            println!("Placed {} blocks, broke {}, said {} things, put {} diamonds in the chest ({:.1}s).", r.placed, r.broken, r.chats, r.deposits, started.elapsed().as_secs_f32());
            if r.ok() {
                println!("OK: everyone agrees.");
                0
            } else {
                for p in &r.problems {
                    println!("MISMATCH: {p}");
                }
                1
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_short_playtest_passes() {
        let r = super::playtest(2, 5.0, 77, 26150).expect("it runs");
        assert!(r.placed > 0 && r.chats > 0, "the bots did nothing: {r:?}");
        assert!(r.ok(), "{:#?}", r.problems);
    }
}
