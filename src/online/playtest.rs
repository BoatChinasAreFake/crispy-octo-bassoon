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
//! Then the newer things get a turn: a bot glides (the host sees it
//! gliding), throws a Soggy Spear (it lands on the host's side and comes
//! back), breaks a full Hollow Box, picks it up and puts it down somewhere
//! else (the contents come too), and is made a spectator (the host hides it
//! from the others and refuses its block edits). Last, the host must have
//! everyone's statistics.
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
    let Msg::Welcome { id, seed, time, creative, spawn, worldgen, .. } = welcome else { unreachable!() };
    let mut g = Game::new_client(id, seed, worldgen, time, creative, spawn, conn, name, leftover);
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
    /// Every chat line it has seen (the game keeps only the last 100 on screen).
    heard: HashSet<String>,
}

/// What happened, and what didn't match.
#[derive(Default, Debug)]
pub struct Report {
    pub placed: u32,
    pub broken: u32,
    pub chats: u32,
    pub deposits: u32,
    /// Which of the newer features were checked (glide, spear, box, spectator, stats, and the latest batch).
    pub features: Vec<&'static str>,
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
        team.push(Bot { game, name, rng: Rng::new(seed as u64 * 31 + k as u64 + 1), heading: k as f32 * 2.1, towers: Vec::new(), next_act: 1.0 + k as f32 * 0.1, acts: 0, said: Vec::new(), heard: HashSet::new() });
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
            note_chat(bot);
        }
        std::thread::sleep(Duration::from_micros(300));
    }
    new_features(&mut host, &mut team, &mut report, &mut touched);
    latest_features(&mut host, &mut team, &mut report, &mut touched);
    batch_two(&mut host, &mut team, &mut report, &mut touched);
    batch_three(&mut host, &mut team, &mut report, &mut touched);
    underground(&mut host, &mut team, &mut report, &mut touched);
    deeper(&mut host, &mut team, &mut report, &mut touched);
    wider(&mut host, &mut team, &mut report, &mut touched);
    // Let everything arrive.
    for _ in 0..(3.0 / DT) as usize {
        host.update(DT, &idle());
        for bot in team.iter_mut() {
            bot.game.update(DT, &idle());
            note_chat(bot);
        }
        std::thread::sleep(Duration::from_micros(300));
    }
    check(&host, &team, &touched, chest, &mut report);
    Ok(report)
}

/// Run the host and every bot for `secs`, with `c` choosing bot `k`'s controls.
fn pump(host: &mut Game, team: &mut [Bot], secs: f32, c: impl Fn(usize) -> Controls) {
    for _ in 0..(secs / DT) as usize {
        host.update(DT, &idle());
        for (k, bot) in team.iter_mut().enumerate() {
            bot.game.update(DT, &c(k));
            note_chat(bot);
        }
        std::thread::sleep(Duration::from_micros(300));
    }
}

/// A flat stone floor (with room above) around `mid`, so nothing thrown or
/// dropped there rolls into a hole or a cave.
fn pad(host: &mut Game, touched: &mut HashSet<IVec3>, mid: IVec3, r: i32) {
    for dz in -r..=r {
        for dx in -r..=r {
            let p = mid + IVec3::new(dx, 0, dz);
            for down in 1..4 {
                host.world.set_v(p - IVec3::Y * down, STONE);
                touched.insert(p - IVec3::Y * down);
            }
            for up in 0..3 {
                host.world.set_v(p + IVec3::Y * up, AIR);
                touched.insert(p + IVec3::Y * up);
            }
        }
    }
}

/// Which bot has `item` (in its bag), if any.
fn holder(team: &[Bot], item: Id) -> Option<usize> {
    team.iter().position(|b| b.game.inv.count(item) > 0)
}

/// Put bot `k` back on its feet by spawn (after gliding off, say).
fn ground(host: &mut Game, team: &mut [Bot], k: usize) {
    let (x, z) = (host.spawn.x.floor() as i32 + k as i32 * 2 - 3, host.spawn.z.floor() as i32 - 4);
    let y = host.world.surface_y(x, z) + 1;
    let g = &mut team[k].game;
    g.player.body.pos = Vec3::new(x as f32 + 0.5, y as f32, z as f32 + 0.5);
    g.player.body.vel = Vec3::ZERO;
    g.player.gliding = false;
    g.player.fall_start = y as f32;
    pump(host, team, 0.5, |_| idle());
}

/// Walk bot `k` onto whatever `item` lies on the host's ground, trying spots
/// around it in case a wall or a tower is in the way. True once it has one.
fn fetch(host: &mut Game, team: &mut [Bot], k: usize, item: Id) -> bool {
    let offsets = [(0.0, 0.0), (0.6, 0.0), (-0.6, 0.0), (0.0, 0.6), (0.0, -0.6), (0.6, 0.6), (-0.6, -0.6), (0.6, -0.6), (-0.6, 0.6)];
    for round in 0..3 {
        for (dx, dz) in offsets {
            if team[k].game.inv.count(item) > 0 {
                return true;
            }
            let Some(at) = host.drops.iter().find(|d| d.item == item).map(|d| d.body.pos) else { return team[k].game.inv.count(item) > 0 };
            let g = &mut team[k].game;
            g.player.body.pos = at + Vec3::new(dx, 0.05 + round as f32 * 0.3, dz);
            g.player.body.vel = Vec3::ZERO;
            pump(host, team, 0.15, |_| idle());
        }
    }
    team[k].game.inv.count(item) > 0
}

/// A bot that fell somewhere nasty while wandering gets back up (the checks need everyone alive).
fn revive(host: &mut Game, team: &mut [Bot]) {
    for b in team.iter_mut().filter(|b| b.game.dead.is_some()) {
        b.game.respawn();
        // (No renderer here to say the chunks are in.)
        b.game.ready = true;
    }
    pump(host, team, 0.5, |_| idle());
}

/// Gliding, spears, Hollow Boxes, spectators and statistics, end to end.
fn new_features(host: &mut Game, team: &mut [Bot], report: &mut Report, touched: &mut HashSet<IVec3>) {
    use crate::net::{FLAG_GHOST, FLAG_GLIDE};
    let n = team.len();
    let id = |team: &[Bot], k: usize| team[k % n].game.my_id;
    // The host's own player stands by spawn and would pick things up first: out of the way.
    host.set_mode(crate::modes::GameMode::Spectator);
    host.player.body.pos += Vec3::Y * 40.0;
    revive(host, team);

    // 1. A Glider, from high up: the host should see the bot gliding.
    let (glider_bot, gid) = (0, id(team, 0));
    host.give_peer(gid, GLIDER, 1);
    pump(host, team, 0.5, |_| idle());
    let g = &mut team[glider_bot].game;
    if let Some(slot) = g.inv.slots.iter().position(|s| s.is_some_and(|s| s.0 == GLIDER)) {
        g.inv.equip(slot);
    }
    g.player.body.pos.y += 30.0;
    g.player.body.vel = Vec3::new(0.0, -4.0, -6.0);
    g.player.body.on_ground = false;
    // (Lifted out of wherever it was standing: a pond, say.)
    g.player.body.in_water = false;
    g.player.body.in_lava = false;
    g.player.gliding = true;
    let mut seen = false;
    for _ in 0..40 {
        pump(host, team, 0.05, |_| idle());
        seen |= host.peers.get(&gid).is_some_and(|p| p.flags & FLAG_GLIDE != 0);
    }
    report.features.push("glide");
    if !seen {
        report.problems.push(format!("the host never saw {} gliding", team[glider_bot].name));
    }
    ground(host, team, glider_bot);

    // 2. A spear, thrown and picked up again.
    let (spear_bot, sid) = (1 % n, id(team, 1));
    ground(host, team, spear_bot);
    let feet = team[spear_bot].game.player.body.pos.floor().as_ivec3();
    pad(host, touched, feet, 3);
    host.give_peer(sid, SPEAR, 1);
    pump(host, team, 0.5, |_| idle());
    let g = &mut team[spear_bot].game;
    if let Some(slot) = g.inv.slots.iter().position(|s| s.is_some_and(|s| s.0 == SPEAR)) {
        g.inv.selected = slot;
        // Down at the ground a few steps ahead, so it lands close by.
        g.player.pitch = -0.7;
        pump(host, team, 0.3, |_| idle());
        pump(host, team, 0.05, |k| Controls { use_pressed: k == spear_bot, use_held: k == spear_bot, ..idle() });
        pump(host, team, 3.0, |_| idle());
        report.features.push("spear");
        // Go and get it (unless a bot standing nearby already has).
        if host.drops.iter().any(|d| d.item == SPEAR) {
            fetch(host, team, spear_bot, SPEAR);
        }
        match holder(team, SPEAR) {
            None => report.problems.push("the thrown spear was lost: nobody picked it up".into()),
            Some(k) => {
                let pid = team[k].game.my_id;
                let ledger = host.peers.get(&pid).map(|p| p.ledger.bag.count(SPEAR)).unwrap_or(0);
                if ledger != 1 {
                    report.problems.push(format!("{} picked up the spear but the host's ledger says {ledger}", team[k].name));
                }
            }
        }
    }

    // 3. A Hollow Box full of diamonds: broken, picked up, put down elsewhere.
    let box_bot = 2 % n;
    ground(host, team, box_bot);
    let feet = team[box_bot].game.player.body.pos.floor().as_ivec3();
    let spot = feet + IVec3::X * 2;
    // A flat pad round it, so the box can't roll into a hole nobody fits in.
    pad(host, touched, spot, 1);
    host.world.set_v(spot, HOLLOW_BOX);
    if let Some(c) = host.world.containers.get_mut(&spot) {
        c.slots[0] = Some((DIAMOND, 7));
    }
    touched.insert(spot);
    pump(host, team, 1.0, |_| idle());
    let g = &mut team[box_bot].game;
    if g.world.get_v(spot) == HOLLOW_BOX {
        g.world.set_v(spot, AIR);
    }
    pump(host, team, 1.5, |_| idle());
    report.features.push("box");
    if host.world.get_v(spot) == HOLLOW_BOX {
        report.problems.push("the host kept the Hollow Box a bot broke".into());
    }
    if host.drops.iter().any(|d| d.item == HOLLOW_BOX) {
        fetch(host, team, box_bot, HOLLOW_BOX);
    }
    match holder(team, HOLLOW_BOX) {
        None => report.problems.push("the broken Hollow Box was lost: nobody picked it up".into()),
        Some(box_bot) => {
            let g = &mut team[box_bot].game;
            match g.inv.slots.iter().position(|s| s.is_some_and(|s| s.0 == HOLLOW_BOX)) {
                None => unreachable!("holder has one"),
                Some(slot) => {
                    g.inv.selected = slot;
                    let there = g.player.body.pos.floor().as_ivec3() + IVec3::Z * 2;
                    // Somewhere clear, on stone.
                    host.world.set_v(there - IVec3::Y, STONE);
                    host.world.set_v(there, AIR);
                    pump(host, team, 0.5, |_| idle());
                    let g = &mut team[box_bot].game;
                    g.world.set_v(there, HOLLOW_BOX);
                    g.inv.consume_held();
                    touched.insert(there);
                    pump(host, team, 1.5, |_| idle());
                    let inside = host.world.containers.get(&there).map(|c| c.slots.iter().flatten().filter(|s| s.0 == DIAMOND).map(|s| s.1 as u32).sum::<u32>()).unwrap_or(0);
                    if inside != 7 {
                        report.problems.push(format!("the Hollow Box put down again holds {inside} diamonds, not 7"));
                    }
                }
            }
        }
    }

    // 4. A spectator: hidden from the others, and its edits refused.
    let (ghost_bot, ghid) = (n - 1, id(team, n - 1));
    ground(host, team, ghost_bot);
    let name = team[ghost_bot].name.clone();
    host.admin_command(crate::admin::Caller::Host, &format!("/gamemode spectator {name}"));
    pump(host, team, 1.0, |_| idle());
    report.features.push("spectator");
    if !team[ghost_bot].game.spectator {
        report.problems.push(format!("{name} was never told it's a spectator"));
    }
    if host.peers.get(&ghid).is_none_or(|p| p.flags & FLAG_GHOST == 0) {
        report.problems.push(format!("the host doesn't see {name} as a spectator"));
    }
    for other in team.iter().filter(|b| b.name != name) {
        if other.game.peers.get(&ghid).is_some_and(|p| p.alive()) {
            report.problems.push(format!("{} can still see the spectator {name}", other.name));
        }
    }
    let g = &mut team[ghost_bot].game;
    let poke = g.player.body.pos.floor().as_ivec3() + IVec3::new(0, 3, 0);
    let before = host.world.get_v(poke);
    g.world.set_v(poke, COBBLE);
    touched.insert(poke);
    pump(host, team, 1.0, |_| idle());
    if host.world.get_v(poke) != before {
        report.problems.push(format!("the host let the spectator {name} place a block"));
    }
    host.admin_command(crate::admin::Caller::Host, &format!("/gamemode survival {name}"));
    pump(host, team, 0.5, |_| idle());

    // 5. Statistics: every bot's have reached the host.
    pump(host, team, crate::players::REPORT_SECS + 0.5, |_| idle());
    report.features.push("stats");
    for bot in team.iter() {
        let kept = host.peers.get(&bot.game.my_id).map(|p| crate::stats::Stats::decode(&p.stats)).unwrap_or_default();
        // (A bot that died early has done nothing worth counting.)
        if kept.played <= 0.0 && bot.game.dead.is_none() {
            report.problems.push(format!("the host has no statistics for {}", bot.name));
        }
    }
}

/// Crafting tables, beehives, brushing, smithing and death messages, checked by the host.
fn latest_features(host: &mut Game, team: &mut [Bot], report: &mut Report, touched: &mut HashSet<IVec3>) {
    let n = team.len();
    let ledger = |host: &Game, id: u32, item: Id| host.peers.get(&id).map(|p| p.ledger.bag.count(item)).unwrap_or(0);
    revive(host, team);

    // 6. A chest needs a crafting table nearby: refused without one, fine with one.
    let k = 0;
    ground(host, team, k);
    let id = team[k].game.my_id;
    let feet = team[k].game.player.body.pos.floor().as_ivec3();
    pad(host, touched, feet, 4);
    host.give_peer(id, PLANKS, 16);
    pump(host, team, 0.5, |_| idle());
    let chest = recipes().iter().position(|r| r.output.0 == CHEST && r.inputs == vec![(PLANKS, 8)]).expect("chest recipe") as u16;
    team[k].game.net_send_msg(Msg::Craft { recipe: chest, times: 1 });
    pump(host, team, 0.5, |_| idle());
    report.features.push("table");
    if ledger(host, id, CHEST) != 0 {
        report.problems.push(format!("the host let {} craft a chest with no crafting table", team[k].name));
    }
    let table = feet + IVec3::X * 2;
    host.world.set_v(table, TABLE);
    touched.insert(table);
    pump(host, team, 0.5, |_| idle());
    team[k].game.net_send_msg(Msg::Craft { recipe: chest, times: 1 });
    pump(host, team, 0.5, |_| idle());
    if ledger(host, id, CHEST) != 1 {
        report.problems.push(format!("the host refused {}'s chest at a crafting table", team[k].name));
    }

    // 7. Honey: smoke a full hive, then bottle some.
    let k = 1 % n;
    ground(host, team, k);
    let id = team[k].game.my_id;
    let feet = team[k].game.player.body.pos.floor().as_ivec3();
    pad(host, touched, feet, 3);
    let hive = feet + IVec3::X * 2;
    host.world.set_v(hive, BEEHIVE_HONEY);
    let mut colony = crate::bees::Colony::founded(crate::bees::Queen::Gentle);
    colony.honey = 60.0;
    colony.nectar = [0.0, 60.0, 0.0, 0.0, 0.0];
    host.hives.insert(hive, colony);
    touched.insert(hive);
    host.give_peer(id, BEE_SMOKER, 1);
    host.give_peer(id, GLASS_BOTTLE, 1);
    pump(host, team, 0.8, |_| idle());
    for item in [BEE_SMOKER, GLASS_BOTTLE] {
        // In hand first: whatever's held is what gets used up.
        let g = &mut team[k].game;
        if let Some(slot) = g.inv.slots.iter().position(|s| s.is_some_and(|s| s.0 == item)) {
            g.inv.selected = slot;
            g.use_on_hive(hive, item);
        }
        pump(host, team, if item == BEE_SMOKER { 0.4 } else { 2.0 }, |_| idle());
    }
    report.features.push("honey");
    let honey = crate::bees::Flavour::Sunny.item();
    if host.drops.iter().any(|d| d.item == honey) {
        fetch(host, team, k, honey);
    }
    if holder(team, honey).is_none() {
        report.problems.push("nobody got the bottle of honey from the hive".into());
    }
    if host.hives.get(&hive).is_none_or(|c| c.honey > 45.0) {
        report.problems.push("the hive still has all its honey after bottling".into());
    }

    // 8. Archaeology: brush a suspicious block (the host rolls the find).
    let k = 2 % n;
    ground(host, team, k);
    let id = team[k].game.my_id;
    let feet = team[k].game.player.body.pos.floor().as_ivec3();
    pad(host, touched, feet, 3);
    let sus = feet + IVec3::X * 2;
    host.world.set_v(sus, SUSPICIOUS_SAND);
    touched.insert(sus);
    host.give_peer(id, BRUSH, 1);
    pump(host, team, 1.0, |_| idle());
    team[k].game.net_send_msg(Msg::Excavate { x: sus.x, y: sus.y, z: sus.z, cracks: 0 });
    pump(host, team, 1.5, |_| idle());
    report.features.push("brush");
    if host.world.get_v(sus) != SAND {
        report.problems.push(format!("the suspicious sand {} brushed is still suspicious", team[k].name));
    }

    // 9. Smithing: a Dimond pickaxe becomes Scorchite.
    let k = 3 % n;
    ground(host, team, k);
    let id = team[k].game.my_id;
    let feet = team[k].game.player.body.pos.floor().as_ivec3();
    pad(host, touched, feet, 3);
    let bench = feet + IVec3::X * 2;
    host.world.set_v(bench, SMITHING_TABLE);
    touched.insert(bench);
    for item in [PICK_DIAMOND, UPGRADE_TEMPLATE, SCORCHITE_INGOT] {
        host.give_peer(id, item, 1);
    }
    pump(host, team, 1.0, |_| idle());
    let g = &mut team[k].game;
    g.open_bench(bench, crate::smithing::Bench::Smithing);
    for (slot, item) in [(0, UPGRADE_TEMPLATE), (1, PICK_DIAMOND), (2, SCORCHITE_INGOT)] {
        if let Some(i) = g.inv.slots.iter().position(|s| s.is_some_and(|s| s.0 == item)) {
            g.inv.cursor = g.inv.slots[i].take();
            g.bench_click(slot, false);
        }
    }
    g.bench_take();
    g.close_bench();
    pump(host, team, 1.0, |_| idle());
    report.features.push("smithing");
    if ledger(host, id, PICK_SCORCHITE) != 1 || ledger(host, id, PICK_DIAMOND) != 0 {
        report.problems.push(format!("the host didn't see {} upgrade a pickaxe to Scorchite", team[k].name));
    }

    // 10. A death message reaches everyone.
    let k = n - 1;
    let name = team[k].name.clone();
    host.admin_command(crate::admin::Caller::Host, &format!("/kill {name}"));
    pump(host, team, 1.5, |_| idle());
    report.features.push("death message");
    let heard = |log: &std::collections::VecDeque<String>| log.iter().any(|l| l.contains(&name) && l.contains("struck down"));
    if !heard(&host.chat_log) {
        report.problems.push(format!("the host never heard how {name} died"));
    }
    for other in team.iter().filter(|b| b.name != name) {
        if !heard(&other.game.chat_log) {
            report.problems.push(format!("{} never heard how {name} died", other.name));
        }
    }
    revive(host, team);
}

/// Falling sand, note blocks, jukeboxes, fireballs, raids and totems, end to end.
fn batch_two(host: &mut Game, team: &mut [Bot], report: &mut Report, touched: &mut HashSet<IVec3>) {
    let n = team.len();
    let ledger = |host: &Game, id: u32, item: Id| host.peers.get(&id).map(|p| p.ledger.bag.count(item)).unwrap_or(0);
    revive(host, team);

    // 11. Sand with nothing under it falls, for everyone.
    let k = 0;
    ground(host, team, k);
    let feet = team[k].game.player.body.pos.floor().as_ivec3();
    pad(host, touched, feet, 4);
    let base = feet + IVec3::new(-3, 0, 0);
    host.world.set_v(base, STONE);
    host.world.set_v(base + IVec3::Y, SAND);
    host.world.set_v(base + IVec3::Y * 2, GRAVEL);
    for y in 0..3 {
        touched.insert(base + IVec3::Y * y);
    }
    pump(host, team, 0.5, |_| idle());
    host.world.set_v(base, AIR);
    pump(host, team, 2.0, |_| idle());
    report.features.push("falling");
    if host.world.get_v(base) != SAND || host.world.get_v(base + IVec3::Y) != GRAVEL {
        report.problems.push("the sand and gravel didn't fall when the stone under them went".into());
    }

    // 12. A note block, tuned by a bot; a jukebox it puts a disc into and takes it out of.
    let k = 1 % n;
    ground(host, team, k);
    let id = team[k].game.my_id;
    let feet = team[k].game.player.body.pos.floor().as_ivec3();
    pad(host, touched, feet, 3);
    let note = feet + IVec3::X * 2;
    let juke = feet + IVec3::NEG_X * 2;
    host.world.set_v(note, NOTE_BLOCK);
    host.world.set_v(juke, JUKEBOX);
    touched.insert(note);
    touched.insert(juke);
    host.give_peer(id, DISC_FIRST + 2, 1);
    pump(host, team, 1.0, |_| idle());
    team[k].game.tune_note_block(note);
    let g = &mut team[k].game;
    if let Some(slot) = g.inv.slots.iter().position(|s| s.is_some_and(|s| s.0 == DISC_FIRST + 2)) {
        g.inv.selected = slot;
        g.use_jukebox(juke, DISC_FIRST + 2);
    }
    pump(host, team, 1.0, |_| idle());
    report.features.push("music");
    if host.world.get_v(note) != NOTE_BLOCK + 1 {
        report.problems.push(format!("the host didn't see {} tune the note block", team[k].name));
    }
    if crate::music::disc_in(host.world.get_v(juke)) != Some(2) || ledger(host, id, DISC_FIRST + 2) != 0 {
        report.problems.push(format!("the host didn't see {} put a disc in the jukebox", team[k].name));
    }
    team[k].game.use_jukebox(juke, AIR);
    pump(host, team, 1.0, |_| idle());
    if host.drops.iter().any(|d| d.item == DISC_FIRST + 2) {
        fetch(host, team, k, DISC_FIRST + 2);
    }
    if host.world.get_v(juke) != JUKEBOX || holder(team, DISC_FIRST + 2).is_none() {
        report.problems.push("the disc didn't come back out of the jukebox".into());
    }

    // 13. A fireball, swatted back by a bot.
    let k = 2 % n;
    ground(host, team, k);
    let bot_id = team[k].game.my_id;
    let feet = team[k].game.player.body.pos.floor().as_ivec3();
    pad(host, touched, feet, 4);
    // Looking up at the open sky (a fireball knocked back flies off into it).
    team[k].game.player.pitch = 1.2;
    team[k].game.player.yaw = 0.0;
    pump(host, team, 0.3, |_| idle());
    let (eye, dir) = (team[k].game.player.eye(), team[k].game.player.look_dir());
    host.spawn_fireball(eye + dir * 3.0, -dir * 1.0, false, 0);
    pump(host, team, 0.4, |_| idle());
    let swatted = team[k].game.deflect_fireball(eye, dir, bot_id);
    // (Not long: knocked back, it's fast, and soon hits something.)
    pump(host, team, 0.15, |_| idle());
    report.features.push("fireball");
    if !swatted || !host.fireballs.iter().any(|f| f.returned == Some(bot_id)) {
        report.problems.push(format!("{} couldn't swat a fireball back", team[k].name));
    }
    host.fireballs.clear();

    // 14. Bad Omen reaches a bot; a raid's bar reaches everyone near it.
    let k = 3 % n;
    let bot_id = team[k].game.my_id;
    host.give_effect_to(bot_id, crate::potions::Potion::BadOmen, 60.0);
    let centre = team[k].game.player.body.pos;
    host.start_raid(centre);
    pump(host, team, 2.5, |_| idle());
    report.features.push("raid");
    if !team[k].game.has_effect(crate::potions::Potion::BadOmen) {
        report.problems.push(format!("{} was never told about its Bad Omen", team[k].name));
    }
    if team[k].game.raid_hud.is_none() {
        report.problems.push(format!("{} never saw the raid bar", team[k].name));
    }
    host.raid = None;
    host.mobs.retain(|m| !m.kind.raider());

    // 15. A Totem of Not Dying, used up by a bot that should have died.
    let k = n - 1;
    let id = team[k].game.my_id;
    let name = team[k].name.clone();
    host.give_peer(id, TOTEM, 1);
    pump(host, team, 1.0, |_| idle());
    let g = &mut team[k].game;
    if let Some(slot) = g.inv.slots.iter().position(|s| s.is_some_and(|s| s.0 == TOTEM))
        && slot >= 9
    {
        // Into the hotbar.
        g.inv.slots.swap(slot, 0);
    }
    host.admin_command(crate::admin::Caller::Host, &format!("/kill {name}"));
    pump(host, team, 1.5, |_| idle());
    report.features.push("totem");
    if team[k].game.dead.is_some() || ledger(host, id, TOTEM) != 0 {
        report.problems.push(format!("the totem didn't save {name} (or the host still thinks they have it)"));
    }
    revive(host, team);
}

/// Remember the chat lines a bot can see now (before they scroll away).
fn note_chat(bot: &mut Bot) {
    for line in bot.game.chat_log.iter().rev().take(16) {
        if line.contains(" says hi #") {
            bot.heard.insert(line.clone());
        }
    }
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
            let heard = other.said.iter().filter(|line| bot.heard.iter().any(|l| l.contains(line.as_str()))).count();
            if heard < other.said.len() {
                report.problems.push(format!("{} heard {} of {}'s {} lines", bot.name, heard, other.name, other.said.len()));
            }
        }
        if let Some(peer) = host.peers.get(&g.my_id) {
            for item in [COBBLE, DIAMOND, SPEAR, GLIDER, HOLLOW_BOX] {
                let worn = g.inv.armor.iter().flatten().filter(|s| s.0 == item).count() as u32;
                let (mine, ledger) = (g.inv.count(item) + worn, peer.ledger.bag.count(item));
                if mine != ledger {
                    report.problems.push(format!("{} holds {mine} {} but the host's ledger says {ledger}", bot.name, item_name(item)));
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

/// Campfires, Llama packs and Armour Stands, end to end.
fn batch_three(host: &mut Game, team: &mut [Bot], report: &mut Report, touched: &mut HashSet<IVec3>) {
    let n = team.len();
    let ledger = |host: &Game, id: u32, item: Id| host.peers.get(&id).map(|p| p.ledger.bag.count(item)).unwrap_or(0);
    revive(host, team);

    // 16. A bot puts a porkchop on a campfire; it cooks where the world lives.
    let k = 0;
    ground(host, team, k);
    let id = team[k].game.my_id;
    let feet = team[k].game.player.body.pos.floor().as_ivec3();
    pad(host, touched, feet, 3);
    let fire = feet + IVec3::X * 2;
    host.world.set_v(fire, CAMPFIRE);
    touched.insert(fire);
    host.give_peer(id, PORKCHOP, 1);
    pump(host, team, 1.0, |_| idle());
    let g = &mut team[k].game;
    if let Some(slot) = g.inv.slots.iter().position(|s| s.is_some_and(|s| s.0 == PORKCHOP)) {
        g.inv.selected = slot;
        g.use_campfire(fire);
    }
    pump(host, team, 1.0, |_| idle());
    report.features.push("campfire");
    let on_fire = host.drops.iter().any(|d| d.item == PORKCHOP && d.body.pos.distance(fire.as_vec3() + macroquad::math::Vec3::splat(0.5)) < 1.2);
    if !on_fire || ledger(host, id, PORKCHOP) != 0 {
        report.problems.push(format!("the host didn't see {} put a porkchop on the campfire", team[k].name));
    }
    host.drops.retain(|d| d.item != PORKCHOP);
    host.world.set_v(fire, AIR);

    // 17. A bot's own Llama: it can open the pack; another bot can't.
    let k = 1 % n;
    ground(host, team, k);
    let at = team[k].game.player.body.pos + macroquad::math::Vec3::X * 1.5;
    let llama = host.alloc_mob(crate::entity::MobKind::Llama, at);
    let owner = crate::players::record_key(&team[k].name);
    if let Some(m) = host.mobs.iter_mut().find(|m| m.id == llama) {
        m.owner = Some(owner);
        m.saddled = true;
        m.sitting = true;
        m.persistent = true;
    }
    let mut pack = crate::containers::Container::for_block(CHEST);
    pack.slots[0] = Some((DIAMOND, 3));
    host.world.packs.insert(llama, pack);
    pump(host, team, 1.0, |_| idle());
    let key = crate::wildlife::pack_key(llama);
    team[k].game.open_container(key);
    pump(host, team, 1.0, |_| idle());
    report.features.push("llama");
    if team[k].game.world.packs.get(&llama).and_then(|c| c.slots[0]) != Some((DIAMOND, 3)) {
        report.problems.push(format!("{} couldn't see into its own Llama's pack", team[k].name));
    }
    team[k].game.close_container();
    if n > 1 {
        let j = (k + 1) % n;
        let other = team[j].game.my_id;
        team[j].game.player.body.pos = at - macroquad::math::Vec3::X * 1.5;
        team[j].game.open_container(key);
        pump(host, team, 1.0, |_| idle());
        if host.viewers.get(&key).is_some_and(|v| v.contains(&other)) {
            report.problems.push(format!("{} got into someone else's Llama's pack", team[j].name));
        }
        team[j].game.close_container();
    }
    host.mobs.retain(|m| m.id != llama);
    host.world.packs.remove(&llama);

    // 18. An Armour Stand dressed by the host: everyone sees what it wears.
    let k = 2 % n;
    ground(host, team, k);
    let feet = team[k].game.player.body.pos.floor().as_ivec3();
    pad(host, touched, feet, 3);
    let stand = feet + IVec3::Z * 2;
    host.world.set_v(stand, ARMOUR_STAND_FIRST);
    touched.insert(stand);
    host.ensure_container(stand);
    if let Some(c) = host.world.containers.get_mut(&stand) {
        c.slots[0] = Some((TURTLE_SHELL, 1));
    }
    host.dirty_containers.insert(stand);
    pump(host, team, 1.0, |_| idle());
    report.features.push("armour stand");
    for b in team.iter() {
        if b.game.world.containers.get(&stand).and_then(|c| c.slots[0]) != Some((TURTLE_SHELL, 1)) {
            report.problems.push(format!("{} never saw the armour stand's Turtle Shell", b.name));
        }
    }
}

/// v0.2: glow berries, tripwires, the Allay and Witches' potions.
fn underground(host: &mut Game, team: &mut [Bot], report: &mut Report, touched: &mut HashSet<IVec3>) {
    let n = team.len();
    revive(host, team);

    // 19. A bot picks glow berries off a vine: the host sees the vine picked, and the berries drop.
    let k = 0;
    ground(host, team, k);
    let feet = team[k].game.player.body.pos.floor().as_ivec3();
    pad(host, touched, feet, 3);
    let vine = feet + IVec3::new(1, 2, 0);
    host.world.set_v(vine + IVec3::Y, STONE);
    host.world.set_v(vine, CAVE_VINES_LIT);
    touched.extend([vine, vine + IVec3::Y]);
    pump(host, team, 1.0, |_| idle());
    let picked = team[k].game.pick_berries(vine);
    pump(host, team, 1.5, |_| idle());
    report.features.push("glow berries");
    // (Whoever's nearest may have picked the berries up.)
    let held: u32 = host.peers.values().map(|p| p.ledger.bag.count(GLOW_BERRIES)).sum();
    let dropped = host.drops.iter().any(|d| d.item == GLOW_BERRIES);
    if !picked || host.world.get_v(vine) != CAVE_VINES || (held == 0 && !dropped) {
        report.problems.push(format!("the host didn't see {} pick glow berries (picked {picked}, vine {}, berries {held}, dropped {dropped})", team[k].name, host.world.get_v(vine)));
    }
    host.drops.retain(|d| d.item != GLOW_BERRIES);

    // 20. A bot walks into tripwire: the whole line trips, for everyone.
    let k = 1 % n;
    ground(host, team, k);
    let feet = team[k].game.player.body.pos.floor().as_ivec3();
    pad(host, touched, feet, 4);
    let (west, east) = (feet + IVec3::new(-2, 0, 2), feet + IVec3::new(2, 0, 2));
    host.world.set_v(west - IVec3::X, STONE);
    host.world.set_v(east + IVec3::X, STONE);
    host.world.set_v(west, crate::tripwire::hook(3, false));
    host.world.set_v(east, crate::tripwire::hook(1, false));
    for x in west.x + 1..east.x {
        host.world.set_v(IVec3::new(x, feet.y, feet.z + 2), crate::tripwire::tripwire(1, false));
    }
    touched.extend((west.x - 1..=east.x + 1).map(|x| IVec3::new(x, feet.y, feet.z + 2)));
    pump(host, team, 1.0, |_| idle());
    team[k].game.player.body.pos = (feet + IVec3::Z * 2).as_vec3() + Vec3::new(0.5, 0.0, 0.5);
    pump(host, team, 0.4, |_| idle());
    report.features.push("tripwire");
    if !crate::tripwire::tripped(host.world.get_v(east)) {
        report.problems.push(format!("{} walked into tripwire and the host's line didn't trip", team[k].name));
    } else {
        for b in team.iter() {
            if !crate::tripwire::tripped(b.game.world.get_v(west)) {
                report.problems.push(format!("{} never saw the tripwire trip", b.name));
            }
        }
    }
    team[k].game.player.body.pos = feet.as_vec3() + Vec3::new(0.5, 0.0, 0.5);
    pump(host, team, 1.5, |_| idle());

    // 21. A bot hands an Allay a diamond: it's that bot's Allay now, fetching diamonds.
    let k = 2 % n;
    ground(host, team, k);
    let me = team[k].game.my_id;
    let allay = host.alloc_mob(crate::entity::MobKind::Allay, team[k].game.player.body.pos + Vec3::new(1.5, 1.0, 0.0));
    host.give_peer(me, DIAMOND, 1);
    pump(host, team, 1.0, |_| idle());
    let g = &mut team[k].game;
    if let (Some(slot), Some(i)) = (g.inv.slots.iter().position(|s| s.is_some_and(|s| s.0 == DIAMOND)), g.mobs.iter().position(|m| m.id == allay)) {
        g.inv.selected = slot;
        g.use_on_mob(i);
    }
    pump(host, team, 1.0, |_| idle());
    report.features.push("allay");
    let mine = host.mobs.iter().find(|m| m.id == allay).is_some_and(|m| m.seed == DIAMOND as u32 && m.owner.as_deref() == Some(crate::players::record_key(&team[k].name).as_str()));
    if !mine {
        report.problems.push(format!("{} gave an Allay a diamond, and the host's Allay didn't take it", team[k].name));
    }
    host.mobs.retain(|m| m.id != allay);

    // 22. A Witch's bottle bursts on a bot: it's slowed.
    let k = 3 % n;
    ground(host, team, k);
    let at = team[k].game.player.body.pos;
    host.throw_witch_potion(at + Vec3::new(0.0, 3.0, 0.0), Vec3::new(0.0, -6.0, 0.0), crate::potions::Potion::Slowness);
    pump(host, team, 1.5, |_| idle());
    report.features.push("witch potion");
    if !team[k].game.has_effect(crate::potions::Potion::Slowness) {
        report.problems.push(format!("a Witch's potion burst on {} and it wasn't slowed", team[k].name));
    }
}

/// v0.2 part 2: rope, the Wilter, a Starred Beacon and a cave-in.
fn deeper(host: &mut Game, team: &mut [Bot], report: &mut Report, touched: &mut HashSet<IVec3>) {
    let n = team.len();
    let ledger = |host: &Game, id: u32, item: Id| host.peers.get(&id).map(|p| p.ledger.bag.count(item)).unwrap_or(0);
    revive(host, team);

    // 23. A bot hangs a rope on a wall: the host unrolls it, and everyone sees all of it.
    let k = 0;
    ground(host, team, k);
    let id = team[k].game.my_id;
    let feet = team[k].game.player.body.pos.floor().as_ivec3();
    pad(host, touched, feet, 3);
    // A wall to hang it on, with a clear drop beside it to the floor.
    let wall = feet + IVec3::new(2, 4, 0);
    host.world.set_v(wall, STONE);
    for d in 0..=5 {
        host.world.set_v(feet + IVec3::new(1, d, 0), AIR);
    }
    touched.extend((0..=5).map(|d| feet + IVec3::new(1, d, 0)));
    touched.insert(wall);
    host.give_peer(id, ROPE, 1);
    pump(host, team, 1.0, |_| idle());
    let g = &mut team[k].game;
    if let Some(slot) = g.inv.slots.iter().position(|s| s.is_some_and(|s| s.0 == ROPE)) {
        g.inv.selected = slot;
        g.throw_rope(wall, IVec3::NEG_X);
    }
    pump(host, team, 1.5, |_| idle());
    report.features.push("rope");
    let hung = |w: &crate::world::World| (0..=4).all(|d| w.get_v(feet + IVec3::new(1, d, 0)) == ROPE);
    if !hung(&host.world) || ledger(host, id, ROPE) != 0 {
        report.problems.push(format!("the host didn't unroll {}'s rope", team[k].name));
    } else {
        for b in team.iter() {
            if !hung(&b.game.world) {
                report.problems.push(format!("{} never saw the whole rope", b.name));
            }
        }
    }
    for d in 0..=4 {
        host.world.set_v(feet + IVec3::new(1, d, 0), AIR);
    }

    // 24. A bot puts the last skull on a Wilter's T: the host wakes it, and everyone sees it.
    let k = 1 % n;
    ground(host, team, k);
    let id = team[k].game.my_id;
    let feet = team[k].game.player.body.pos.floor().as_ivec3();
    pad(host, touched, feet, 4);
    let mid = feet + IVec3::new(0, 2, 3);
    for x in -1..=1 {
        host.world.set_v(mid + IVec3::new(x, -1, 0), SORROW_SAND);
    }
    host.world.set_v(mid - IVec3::Y * 2, SORROW_SAND);
    host.world.set_v(mid - IVec3::X, CHARRED_SKULL);
    host.world.set_v(mid, CHARRED_SKULL);
    touched.extend([mid - IVec3::X, mid, mid + IVec3::X, mid - IVec3::Y - IVec3::X, mid - IVec3::Y, mid - IVec3::Y + IVec3::X, mid - IVec3::Y * 2]);
    host.give_peer(id, CHARRED_SKULL, 1);
    pump(host, team, 1.0, |_| idle());
    team[k].game.world.set_v(mid + IVec3::X, CHARRED_SKULL);
    pump(host, team, 1.5, |_| idle());
    report.features.push("wilter");
    let wilter = host.mobs.iter().find(|m| m.kind == crate::entity::MobKind::Wilter).map(|m| m.id);
    match wilter {
        None => report.problems.push(format!("{} finished a Wilter's T and nothing woke", team[k].name)),
        Some(w) => {
            for b in team.iter() {
                if !b.game.mobs.iter().any(|m| m.id == w && m.kind == crate::entity::MobKind::Wilter) {
                    report.problems.push(format!("{} never saw the Wilter", b.name));
                }
            }
        }
    }
    host.mobs.retain(|m| m.kind != crate::entity::MobKind::Wilter);
    host.arrows.clear();

    // 25. A bot sets a Wilter Star in a full beacon: the host takes the star and stars it.
    let k = 2 % n;
    ground(host, team, k);
    let id = team[k].game.my_id;
    let feet = team[k].game.player.body.pos.floor().as_ivec3();
    let top = feet + IVec3::new(3, 3, 0);
    for d in 1..=3 {
        for dx in -d..=d {
            for dz in -d..=d {
                let p = top + IVec3::new(dx, -d, dz);
                host.world.set_v(p, OBSIDIAN);
                touched.insert(p);
            }
        }
    }
    for y in top.y + 1..top.y + 40 {
        host.world.set_v(IVec3::new(top.x, y, top.z), AIR);
    }
    host.world.set_v(top, BEACON_FIRST);
    touched.insert(top);
    team[k].game.player.body.pos = (top + IVec3::new(-1, 1, 0)).as_vec3() + macroquad::math::Vec3::new(0.5, 0.0, 0.5);
    host.give_peer(id, WILTER_STAR, 1);
    pump(host, team, 1.0, |_| idle());
    let g = &mut team[k].game;
    if let Some(slot) = g.inv.slots.iter().position(|s| s.is_some_and(|s| s.0 == WILTER_STAR)) {
        g.inv.selected = slot;
        let b = g.world.get_v(top);
        g.use_switch(top, b);
    }
    pump(host, team, 1.5, |_| idle());
    report.features.push("starred beacon");
    if !crate::beacon::starred(host.world.get_v(top)) || ledger(host, id, WILTER_STAR) != 0 {
        report.problems.push(format!("{} set a Wilter Star in a beacon, and the host didn't star it", team[k].name));
    }

    // 26. A cave-in where the world lives: everyone sees the ceiling come down.
    // (Well under spawn, clear of the chest and anything the bots built.)
    let floor = host.spawn.floor().as_ivec3() - IVec3::Y * 18;
    for y in 0..9 {
        for dz in -9..=9 {
            for dx in -9..=9 {
                let p = floor + IVec3::new(dx, y, dz);
                let inside = dx.abs() <= 7 && dz.abs() <= 7 && (1..4).contains(&y);
                host.world.set_v(p, if inside { AIR } else { STONE });
                touched.insert(p);
            }
        }
    }
    for y in 1..4 {
        for dz in -7..=7 {
            for dx in -7..=7 {
                host.dug_out(floor + IVec3::new(dx, y, dz));
            }
        }
    }
    let roof = floor + IVec3::Y * 4;
    pump(host, team, crate::caveins::WARN_SECS + 3.0, |_| idle());
    report.features.push("cave-in");
    if host.world.get_v(roof) == STONE {
        report.problems.push("a cave-in never came down on the host".into());
    } else {
        for b in team.iter() {
            if b.game.world.get_v(roof) == STONE {
                report.problems.push(format!("{} never saw the cave-in", b.name));
            }
        }
    }
}

/// v0.3 part 1: wood doors, new boats, Mushmooers and kelp.
fn wider(host: &mut Game, team: &mut [Bot], report: &mut Report, touched: &mut HashSet<IVec3>) {
    let n = team.len();
    let ledger = |host: &Game, id: u32, item: Id| host.peers.get(&id).map(|p| p.ledger.bag.count(item)).unwrap_or(0);
    revive(host, team);

    // 27. A bot opens a birch door: the host and everyone see it open.
    let k = 0;
    ground(host, team, k);
    let feet = team[k].game.player.body.pos.floor().as_ivec3();
    pad(host, touched, feet, 3);
    let door = feet + IVec3::new(2, 0, 0);
    let base = crate::block::door_base(crate::woods::id(1, crate::woods::part::DOOR)).expect("a birch door");
    host.world.set_v(door, door_of(base, 0, false, false));
    host.world.set_v(door + IVec3::Y, door_of(base, 0, false, true));
    touched.extend([door, door + IVec3::Y]);
    pump(host, team, 1.0, |_| idle());
    team[k].game.toggle_door(door);
    pump(host, team, 1.5, |_| idle());
    report.features.push("birch door");
    let open = |w: &crate::world::World| door_state(w.get_v(door)).is_some_and(|(_, o, _)| o) && door_state(w.get_v(door + IVec3::Y)).is_some_and(|(_, o, _)| o);
    if !open(&host.world) {
        report.problems.push(format!("{} opened a birch door and the host's stayed shut", team[k].name));
    } else {
        for b in team.iter() {
            if !open(&b.game.world) {
                report.problems.push(format!("{} never saw the birch door open", b.name));
            }
        }
    }

    // 28. A bot puts down an acacia boat: the host takes the boat, and everyone sees it.
    let k = 1 % n;
    ground(host, team, k);
    let id = team[k].game.my_id;
    let feet = team[k].game.player.body.pos.floor().as_ivec3();
    pad(host, touched, feet, 3);
    host.give_peer(id, ACACIA_BOAT, 1);
    pump(host, team, 1.0, |_| idle());
    let before = host.vehicles.len();
    let g = &mut team[k].game;
    if let Some(slot) = g.inv.slots.iter().position(|s| s.is_some_and(|s| s.0 == ACACIA_BOAT)) {
        g.inv.selected = slot;
        g.player.pitch = -1.2;
        g.place_vehicle(ACACIA_BOAT);
    }
    pump(host, team, 1.5, |_| idle());
    report.features.push("acacia boat");
    let kind = crate::vehicles::kind_of_item(ACACIA_BOAT);
    match host.vehicles.iter().find(|v| Some(v.kind) == kind).map(|v| v.id) {
        Some(v) if host.vehicles.len() > before && ledger(host, id, ACACIA_BOAT) == 0 => {
            for b in team.iter() {
                if !b.game.vehicles.iter().any(|w| w.id == v && Some(w.kind) == kind) {
                    report.problems.push(format!("{} never saw the acacia boat", b.name));
                }
            }
            host.vehicles.retain(|w| w.id != v);
        }
        _ => report.problems.push(format!("{} put down an acacia boat and the host didn't take it", team[k].name)),
    }

    // 29. A bot shears a Mushmooer: it's a plain Mooer for everyone, and mushrooms drop.
    let k = 2 % n;
    ground(host, team, k);
    let id = team[k].game.my_id;
    let moo = host.alloc_mob(crate::entity::MobKind::Mushmooer, team[k].game.player.body.pos + Vec3::new(1.5, 0.5, 0.0));
    host.give_peer(id, SHEARS, 1);
    pump(host, team, 1.0, |_| idle());
    let g = &mut team[k].game;
    if let (Some(slot), Some(i)) = (g.inv.slots.iter().position(|s| s.is_some_and(|s| s.0 == SHEARS)), g.mobs.iter().position(|m| m.id == moo)) {
        g.inv.selected = slot;
        g.use_on_mob(i);
    }
    pump(host, team, 1.5, |_| idle());
    report.features.push("mushmooer");
    let plain = |mobs: &[crate::entity::Mob]| mobs.iter().any(|m| m.id == moo && m.kind == crate::entity::MobKind::Mooer);
    let mushrooms = host.drops.iter().any(|d| d.item == MUSHROOM) || host.peers.values().any(|p| p.ledger.bag.count(MUSHROOM) > 0);
    if !plain(&host.mobs) || !mushrooms {
        report.problems.push(format!("{} sheared a Mushmooer and the host didn't see it (Mooer {}, mushrooms {mushrooms})", team[k].name, plain(&host.mobs)));
    } else {
        for b in team.iter() {
            if !plain(&b.game.mobs) {
                report.problems.push(format!("{} never saw the Mushmooer sheared", b.name));
            }
        }
    }
    host.mobs.retain(|m| m.id != moo);
    host.drops.retain(|d| d.item != MUSHROOM);

    // 30. A bot breaks the foot of a kelp column: it all comes away, and the water stays, for everyone.
    let k = 3 % n;
    ground(host, team, k);
    let feet = team[k].game.player.body.pos.floor().as_ivec3();
    let foot = feet + IVec3::new(2, -6, 0);
    for y in -1..=6 {
        for dz in -1..=1 {
            for dx in -1..=1 {
                let p = foot + IVec3::new(dx, y, dz);
                let inside = dx == 0 && dz == 0 && y >= 0;
                host.world.set_v(p, if !inside { STONE } else if y < 4 { KELP } else { WATER });
                touched.insert(p);
            }
        }
    }
    pump(host, team, 1.0, |_| idle());
    team[k].game.break_block(foot, true);
    pump(host, team, 1.5, |_| idle());
    report.features.push("kelp");
    let wet = |w: &crate::world::World| (0..4).all(|y| w.get_v(foot + IVec3::Y * y) == WATER);
    if !wet(&host.world) {
        report.problems.push(format!("{} broke kelp and the host's column didn't come away into water ({:?})", team[k].name, (0..4).map(|y| host.world.get_v(foot + IVec3::Y * y)).collect::<Vec<_>>()));
    } else {
        for b in team.iter() {
            if !wet(&b.game.world) {
                report.problems.push(format!("{} never saw the kelp come away", b.name));
            }
        }
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
            println!("Also checked: {}.", r.features.join(", "));
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
        assert!(r.ok(), "{:#?}", r.problems);
        // Long enough that more chat goes by than a player's chat log keeps.
        let r = super::playtest(6, 60.0, 1234, 26170).expect("it runs");
        assert!(r.chats > 110, "only {} lines", r.chats);
        assert!(r.placed > 0 && r.chats > 0, "the bots did nothing: {r:?}");
        assert_eq!(r.features, ["glide", "spear", "box", "spectator", "stats", "table", "honey", "brush", "smithing", "death message", "falling", "music", "fireball", "raid", "totem", "campfire", "llama", "armour stand", "glow berries", "tripwire", "allay", "witch potion", "rope", "wilter", "starred beacon", "cave-in", "birch door", "acacia boat", "mushmooer", "kelp"]);
        assert!(r.ok(), "{:#?}", r.problems);
    }
}
