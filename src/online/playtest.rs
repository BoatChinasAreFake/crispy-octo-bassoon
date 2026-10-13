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
//! everyone's statistics. (Later batches add their own checks, through to
//! teams, Trading Tables, sidebars, land claims and the post.)
//!
//! It prints what it did and what didn't match, and exits with 0 if all is
//! well (1 otherwise), so it can run in CI or before a release.
//!
//! Options: `--bots N` (default 3), `--seconds S` (default 20), `--seed N`, `--port N`.

use crate::block::*;
use crate::game::{Controls, Game};
use crate::net::{Conn, Msg, PROTOCOL};
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
            let proof = conn.auth_proof(nonce, "");
            conn.send(&Msg::Auth { proof });
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
    // (The newest generator, as a new world gets.)
    let mut host = Game::new_with(seed, false, false, crate::world::GenOptions::DEFAULT);
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
    // The checks below set up what they need themselves: no more monsters
    // wandering in (they'd kill a bot halfway through one), and the ones
    // the free play brought sent away.
    host.spawn_timer = f32::MAX;
    host.mobs.retain(|m| !m.menacing() || m.persistent);
    new_features(&mut host, &mut team, &mut report, &mut touched);
    latest_features(&mut host, &mut team, &mut report, &mut touched);
    batch_two(&mut host, &mut team, &mut report, &mut touched);
    batch_three(&mut host, &mut team, &mut report, &mut touched);
    underground(&mut host, &mut team, &mut report, &mut touched);
    deeper(&mut host, &mut team, &mut report, &mut touched);
    wider(&mut host, &mut team, &mut report, &mut touched);
    scorched(&mut host, &mut team, &mut report, &mut touched);
    travelled(&mut host, &mut team, &mut report, &mut touched);
    homely(&mut host, &mut team, &mut report, &mut touched);
    together(&mut host, &mut team, &mut report, &mut touched);
    village_life(&mut host, &mut team, &mut report, &mut touched);
    trade_levels(&mut host, &mut team, &mut report, &mut touched);
    growing_villages(&mut host, &mut team, &mut report);
    new_land(&mut host, &mut team, &mut report);
    reshaped_depths(&mut host, &mut team, &mut report, &mut touched);
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
    // (Nobody picks anything up while dead.)
    if team[k].game.dead.is_some() {
        revive(host, team);
    }
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
    // (Everyone up again first: the night may have had its way with someone.)
    revive(host, team);
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

/// v0.3 part 2: huge fungi, Tuskers, soul fire, Sporelings and a bastion's chest.
fn scorched(host: &mut Game, team: &mut [Bot], report: &mut Report, touched: &mut HashSet<IVec3>) {
    use crate::entity::MobKind;
    let n = team.len();
    revive(host, team);

    // 31. A bot grows a huge fungus with Bone Dust: everyone sees the stem and cap.
    let k = 0;
    ground(host, team, k);
    let id = team[k].game.my_id;
    let feet = team[k].game.player.body.pos.floor().as_ivec3();
    pad(host, touched, feet, 4);
    let spot = feet + IVec3::new(2, 0, 0);
    for y in 0..12 {
        for dz in -3..=3 {
            for dx in -3..=3 {
                let p = spot + IVec3::new(dx, y, dz);
                host.world.set_v(p, AIR);
                touched.insert(p);
            }
        }
    }
    host.world.set_v(spot - IVec3::Y, CRIMSON_NYLIUM);
    host.world.set_v(spot, CRIMSON_FUNGUS);
    host.give_peer(id, BONE_DUST, 1);
    pump(host, team, 1.0, |_| idle());
    let g = &mut team[k].game;
    if let Some(slot) = g.inv.slots.iter().position(|s| s.is_some_and(|s| s.0 == BONE_DUST)) {
        g.inv.selected = slot;
        g.farm_use(BONE_DUST, spot);
    }
    pump(host, team, 1.5, |_| idle());
    report.features.push("huge fungus");
    let grown = |w: &crate::world::World| w.get_v(spot) == CRIMSON_STEM && (3..12).any(|y| w.get_v(spot + IVec3::new(2, y, 0)) == CRIMSON_WART || w.get_v(spot + IVec3::new(1, y, 1)) == CRIMSON_WART);
    if !grown(&host.world) {
        report.problems.push(format!("{} put Bone Dust on a fungus and the host's didn't grow", team[k].name));
    } else {
        for b in team.iter() {
            if !grown(&b.game.world) {
                report.problems.push(format!("{} never saw the huge fungus", b.name));
            }
        }
    }

    // 32. A bot feeds two Tuskers Crimson Fungus: a baby Tusker, for everyone.
    let k = 1 % n;
    ground(host, team, k);
    let id = team[k].game.my_id;
    pad(host, touched, team[k].game.player.body.pos.floor().as_ivec3(), 4);
    let at = team[k].game.player.body.pos + Vec3::new(2.0, 0.3, 0.0);
    host.give_peer(id, CRIMSON_FUNGUS, 2);
    pump(host, team, 0.6, |_| idle());
    // (Tuskers charge on sight: they're fed as soon as the bot knows they're there.)
    let pair = [host.alloc_mob(MobKind::Tusker, at), host.alloc_mob(MobKind::Tusker, at + Vec3::new(0.0, 0.0, 0.9))];
    pump(host, team, 0.3, |_| idle());
    for t in pair {
        let g = &mut team[k].game;
        if let (Some(slot), Some(i)) = (g.inv.slots.iter().position(|s| s.is_some_and(|s| s.0 == CRIMSON_FUNGUS)), g.mobs.iter().position(|m| m.id == t)) {
            g.inv.selected = slot;
            g.use_on_mob(i);
        }
        pump(host, team, 0.4, |_| idle());
    }
    pump(host, team, 3.0, |_| idle());
    report.features.push("tusker");
    let baby = |mobs: &[crate::entity::Mob]| mobs.iter().find(|m| m.kind == MobKind::Tusker && m.baby > 0.0).map(|m| m.id);
    match baby(&host.mobs) {
        Some(b) => {
            for bot in team.iter() {
                if !bot.game.mobs.iter().any(|m| m.id == b) {
                    report.problems.push(format!("{} never saw the baby Tusker", bot.name));
                }
            }
            host.mobs.retain(|m| m.id != b);
        }
        None => report.problems.push(format!("{} fed two Tuskers and the host saw no baby", team[k].name)),
    }
    host.mobs.retain(|m| !pair.contains(&m.id));

    // 33. A bot lights Soul Soil with a Sparker: blue fire, for everyone.
    revive(host, team);
    let k = 2 % n;
    ground(host, team, k);
    let id = team[k].game.my_id;
    let feet = team[k].game.player.body.pos.floor().as_ivec3();
    pad(host, touched, feet, 3);
    let soil = feet + IVec3::new(2, -1, 0);
    host.world.set_v(soil, SOUL_SOIL);
    host.give_peer(id, SPARKER, 1);
    pump(host, team, 1.0, |_| idle());
    let g = &mut team[k].game;
    if let Some(slot) = g.inv.slots.iter().position(|s| s.is_some_and(|s| s.0 == SPARKER)) {
        g.inv.selected = slot;
        g.spark(soil, IVec3::Y);
    }
    pump(host, team, 1.5, |_| idle());
    report.features.push("soul fire");
    let lit = |w: &crate::world::World| w.get_v(soil + IVec3::Y) == SOUL_FIRE;
    if !lit(&host.world) {
        report.problems.push(format!("{} lit Soul Soil and the host has {} there", team[k].name, host.world.get_v(soil + IVec3::Y)));
    } else {
        for b in team.iter() {
            if !lit(&b.game.world) {
                report.problems.push(format!("{} never saw the soul fire", b.name));
            }
        }
    }
    host.world.set_v(soil + IVec3::Y, AIR);

    // 34. A bot hits a Sporeling: the spores slow it down.
    revive(host, team);
    let k = 3 % n;
    ground(host, team, k);
    let me = team[k].game.my_id;
    let from = team[k].game.player.body.pos;
    let spore = host.alloc_mob(MobKind::Sporeling, from + Vec3::new(1.2, 0.2, 0.0));
    pump(host, team, 1.0, |_| idle());
    team[k].game.net_send_msg(crate::net::Msg::Attack { mob: spore, dmg: 1.0, from });
    pump(host, team, 1.5, |_| idle());
    report.features.push("sporeling");
    if !team[k].game.has_effect(crate::potions::Potion::Slowness) {
        report.problems.push(format!("{} hit a Sporeling and wasn't slowed (id {me})", team[k].name));
    }
    host.mobs.retain(|m| m.id != spore);

    // 35. A bot opens a chest next to a Snout: the host's Snout gets cross.
    revive(host, team);
    let k = 4 % n;
    ground(host, team, k);
    let feet = team[k].game.player.body.pos.floor().as_ivec3();
    pad(host, touched, feet, 3);
    let chest = feet + IVec3::new(1, 0, 1);
    host.world.set_v(chest, CHEST);
    let snout = host.alloc_mob(MobKind::Snout, feet.as_vec3() + Vec3::new(-1.5, 0.2, 0.5));
    pump(host, team, 1.0, |_| idle());
    team[k].game.open_container(chest);
    pump(host, team, 1.0, |_| idle());
    team[k].game.close_container();
    report.features.push("snout chest");
    let cross = host.mobs.iter().any(|m| m.id == snout && m.angry);
    if !cross {
        report.problems.push(format!("{} opened a chest by a Snout and it didn't mind", team[k].name));
    }
    host.mobs.retain(|m| m.id != snout);
    host.world.set_v(chest, AIR);
}

/// Walk bot `k` into the portal at `base` and wait for the host to move it.
fn through_portal(host: &mut Game, team: &mut [Bot], k: usize, base: IVec3) {
    let g = &mut team[k].game;
    g.player.body.pos = base.as_vec3() + Vec3::new(1.0, 0.0, 0.5);
    g.player.body.vel = Vec3::ZERO;
    g.portal_cooldown = 0.0;
    g.left_portal = true;
    let from = g.dim;
    for _ in 0..60 {
        if team[k].game.dim != from {
            // (Standing in the portal at the other end, as a player would be.)
            team[k].game.left_portal = false;
            break;
        }
        team[k].game.portal_tick(0.05);
        pump(host, team, 0.05, |_| idle());
    }
    pump(host, team, 1.0, |_| idle());
    // (Bots have no renderer to load the land around them.)
    let at = team[k].game.player.body.pos;
    load_around(&mut team[k].game, at);
}

/// Separate dimensions (see realms.rs): bots go through a portal, see each
/// other (and what's built) there and not at home, and come back on dying.
fn travelled(host: &mut Game, team: &mut [Bot], report: &mut Report, touched: &mut HashSet<IVec3>) {
    use crate::dims::Dim;
    let n = team.len();
    revive(host, team);
    // 36. A portal by spawn takes a bot to the Scorchlands.
    let (a, b) = (0, 1 % n);
    ground(host, team, a);
    let feet = team[a].game.player.body.pos.floor().as_ivec3();
    pad(host, touched, feet, 4);
    let base = feet + IVec3::new(0, 0, 3);
    crate::scorch::build_portal(&mut host.world, base);
    for dx in -1..=2 {
        for dy in -1..=3 {
            touched.insert(base + IVec3::new(dx, dy, 0));
        }
    }
    pump(host, team, 1.0, |_| idle());
    let ida = team[a].game.my_id;
    through_portal(host, team, a, base);
    report.features.push("dimensions");
    if team[a].game.dim != Dim::Scorch || host.peer_dim(ida) != Some(Dim::Scorch) {
        report.problems.push(format!("{} walked into a portal and is in {:?} (the host says {:?})", team[a].name, team[a].game.dim, host.peer_dim(ida)));
        return;
    }
    // Something built down there is seen by whoever's there, and nobody at home.
    let there = team[a].game.player.body.pos.floor().as_ivec3() + IVec3::new(2, 0, 0);
    host.in_realm(Dim::Scorch, |h| h.world.set_v(there, GOLD_BLOCK));
    pump(host, team, 1.0, |_| idle());
    if team[a].game.world.get_v(there) != GOLD_BLOCK {
        report.problems.push(format!("{} never saw the gold block built next to it in the Scorchlands", team[a].name));
    }
    for (k, bot) in team.iter().enumerate() {
        if k != a && bot.game.peers.contains_key(&ida) {
            report.problems.push(format!("{} can still see {} (who's in the Scorchlands)", bot.name, team[a].name));
        }
    }
    // A second bot follows, and the two see each other.
    if b != a {
        let idb = team[b].game.my_id;
        ground(host, team, b);
        through_portal(host, team, b, base);
        if team[b].game.dim != Dim::Scorch {
            report.problems.push(format!("{} followed through the portal and is in {:?}", team[b].name, team[b].game.dim));
        } else {
            pump(host, team, 1.0, |_| idle());
            if !team[b].game.peers.contains_key(&ida) || !team[a].game.peers.contains_key(&idb) {
                let sees = |g: &Game, id: u32| format!("{:?}", g.realm_dims().into_iter().map(|d| (d, g.peer_dim(id))).collect::<Vec<_>>());
                report.problems.push(format!("{} and {} are both in the Scorchlands and can't see each other ({} {}; {:?} {:?} {:?})", team[a].name, team[b].name, sees(&team[a].game, idb), sees(&team[b].game, ida), team[a].game.dim, host.peer_dim(ida), host.peer_dim(idb)));
            }
            if team[b].game.world.get_v(there) != GOLD_BLOCK {
                report.problems.push(format!("{} arrived in the Scorchlands without the gold block that's there", team[b].name));
            }
        }
    }
    host.in_realm(Dim::Scorch, |h| h.world.set_v(there, AIR));
    // Dying there brings everyone home (and the Overworld's edits with it).
    for k in [a, b] {
        if team[k].game.dim != Dim::Over {
            team[k].game.dead = Some("tripped over a Snout".into());
            team[k].game.respawn();
        }
    }
    pump(host, team, 2.0, |_| idle());
    for k in [a, b] {
        if team[k].game.dim != Dim::Over || host.peer_dim(team[k].game.my_id) != Some(Dim::Over) {
            report.problems.push(format!("{} respawned and isn't back in the Overworld", team[k].name));
        }
        load_around(&mut team[k].game, host.spawn);
        if team[k].game.world.get_v(base) != PORTAL_X {
            report.problems.push(format!("{} came home and the portal by spawn is gone (it has {})", team[k].name, team[k].game.world.get_v(base)));
        }
    }
    for k in 0..n {
        ground(host, team, k);
    }
}

/// v0.3 part 3: cauldrons, composters, leads, pack animals and Galloper armour.
fn homely(host: &mut Game, team: &mut [Bot], report: &mut Report, touched: &mut HashSet<IVec3>) {
    use crate::entity::MobKind;
    let n = team.len();
    revive(host, team);
    // 37. A bot fills a cauldron and dyes the water: everyone sees the colour.
    let k = 0;
    ground(host, team, k);
    let id = team[k].game.my_id;
    let feet = team[k].game.player.body.pos.floor().as_ivec3();
    pad(host, touched, feet, 4);
    let pot = feet + IVec3::new(2, 0, 0);
    host.world.set_v(pot, CAULDRON);
    host.give_peer(id, WATER_BUCKET, 1);
    host.give_peer(id, DYE_FIRST + 6, 1);
    pump(host, team, 1.0, |_| idle());
    for item in [WATER_BUCKET, DYE_FIRST + 6] {
        let g = &mut team[k].game;
        if let Some(slot) = g.inv.slots.iter().position(|s| s.is_some_and(|s| s.0 == item)) {
            g.inv.selected = slot;
            g.use_cauldron(pot);
        }
        pump(host, team, 0.5, |_| idle());
    }
    pump(host, team, 1.0, |_| idle());
    report.features.push("cauldron");
    let blue = crate::homecraft::cauldron_block(3, Some(6));
    if host.world.get_v(pot) != blue {
        report.problems.push(format!("{} filled and dyed a cauldron and the host has {}", team[k].name, host.world.get_v(pot)));
    } else {
        for b in team.iter() {
            if b.game.world.get_v(pot) != blue {
                report.problems.push(format!("{} never saw the blue cauldron", b.name));
            }
        }
    }
    host.world.set_v(pot, AIR);

    // 38. A bot empties a full composter: Compost for them, an empty composter for everyone.
    let k = 1 % n;
    ground(host, team, k);
    let feet = team[k].game.player.body.pos.floor().as_ivec3();
    pad(host, touched, feet, 3);
    let bin = feet + IVec3::new(-2, 0, 0);
    host.world.set_v(bin, COMPOSTER + 7);
    pump(host, team, 1.0, |_| idle());
    let had = team[k].game.inv.count(COMPOST);
    let g = &mut team[k].game;
    g.inv.selected = g.inv.slots.iter().position(|s| s.is_none()).unwrap_or(0);
    g.inv.slots[g.inv.selected] = None;
    g.use_composter(bin);
    pump(host, team, 1.5, |_| idle());
    report.features.push("composter");
    if team[k].game.inv.count(COMPOST) <= had {
        report.problems.push(format!("{} emptied a composter and got no Compost", team[k].name));
    }
    for b in team.iter() {
        if b.game.world.get_v(bin) != COMPOSTER {
            report.problems.push(format!("{} sees the composter still full ({})", b.name, b.game.world.get_v(bin)));
        }
    }
    host.world.set_v(bin, AIR);

    // 39. A bot leads a Mooer and ties it to a fence: everyone sees the rope.
    let k = 2 % n;
    ground(host, team, k);
    let id = team[k].game.my_id;
    let feet = team[k].game.player.body.pos.floor().as_ivec3();
    pad(host, touched, feet, 4);
    let cow = host.alloc_mob(MobKind::Mooer, feet.as_vec3() + Vec3::new(1.5, 0.2, 1.5));
    host.give_peer(id, LEAD, 1);
    pump(host, team, 1.0, |_| idle());
    let g = &mut team[k].game;
    if let (Some(slot), Some(i)) = (g.inv.slots.iter().position(|s| s.is_some_and(|s| s.0 == LEAD)), g.mobs.iter().position(|m| m.id == cow)) {
        g.inv.selected = slot;
        g.use_on_mob(i);
    }
    pump(host, team, 1.5, |_| idle());
    report.features.push("lead");
    let led = host.mobs.iter().find(|m| m.id == cow).is_some_and(|m| matches!(m.leash, Some(crate::leads::Leash::Player(_))));
    if !led {
        report.problems.push(format!("{} put a lead on a Mooer and the host's isn't on one", team[k].name));
    } else {
        for b in team.iter() {
            if !b.game.mobs.iter().any(|m| m.id == cow && m.leash_end.is_some()) {
                report.problems.push(format!("{} never saw the Mooer's lead", b.name));
            }
        }
        let fence = feet + IVec3::new(-1, 0, 1);
        host.world.set_v(fence, FENCE_FIRST);
        touched.insert(fence);
        pump(host, team, 0.5, |_| idle());
        team[k].game.use_fence_for_leads(fence);
        pump(host, team, 1.0, |_| idle());
        let tied = host.mobs.iter().find(|m| m.id == cow).is_some_and(|m| m.leash == Some(crate::leads::Leash::Fence(fence)));
        if !tied {
            report.problems.push(format!("{} tied their Mooer to a fence and the host's isn't", team[k].name));
        }
        host.world.set_v(fence, AIR);
    }
    host.mobs.retain(|m| m.id != cow);

    // 40. A bot puts a chest on its Donkey and armour on its Galloper: everyone sees both.
    let k = 3 % n;
    ground(host, team, k);
    let id = team[k].game.my_id;
    let who = crate::players::record_key(&team[k].game.player_name);
    let feet = team[k].game.player.body.pos.floor().as_ivec3();
    pad(host, touched, feet, 4);
    let donkey = host.alloc_mob(MobKind::Donkey, feet.as_vec3() + Vec3::new(1.5, 0.2, 0.5));
    let horse = host.alloc_mob(MobKind::Galloper, feet.as_vec3() + Vec3::new(-1.5, 0.2, 0.5));
    for m in host.mobs.iter_mut().filter(|m| m.id == donkey || m.id == horse) {
        m.owner = Some(who.clone());
        m.persistent = true;
    }
    host.give_peer(id, CHEST, 1);
    host.give_peer(id, HORSE_ARMOR_GOLD, 1);
    pump(host, team, 1.0, |_| idle());
    for (mob, item) in [(donkey, CHEST), (horse, HORSE_ARMOR_GOLD)] {
        let g = &mut team[k].game;
        if let (Some(slot), Some(i)) = (g.inv.slots.iter().position(|s| s.is_some_and(|s| s.0 == item)), g.mobs.iter().position(|m| m.id == mob)) {
            g.inv.selected = slot;
            g.use_on_mob(i);
        }
        pump(host, team, 0.6, |_| idle());
    }
    pump(host, team, 1.5, |_| idle());
    report.features.push("donkey pack");
    report.features.push("galloper armour");
    if !host.mobs.iter().any(|m| m.id == donkey && m.pack) {
        report.problems.push(format!("{} gave their Donkey a chest and the host's has none", team[k].name));
    }
    if !host.mobs.iter().any(|m| m.id == horse && m.barding == 2) {
        report.problems.push(format!("{} armoured their Galloper and the host's isn't", team[k].name));
    }
    for b in team.iter() {
        if !b.game.mobs.iter().any(|m| m.id == donkey && m.pack) {
            report.problems.push(format!("{} never saw the Donkey's chest", b.name));
        }
        if !b.game.mobs.iter().any(|m| m.id == horse && m.barding == 2) {
            report.problems.push(format!("{} never saw the Galloper's gold armour", b.name));
        }
    }
    host.world.packs.remove(&donkey);
    host.mobs.retain(|m| m.id != donkey && m.id != horse);

    // 41. A bot cooks a stew in a pot on a campfire and serves it: everyone sees the pot.
    let k = 4 % n;
    ground(host, team, k);
    let id = team[k].game.my_id;
    let feet = team[k].game.player.body.pos.floor().as_ivec3();
    pad(host, touched, feet, 4);
    let fire = feet + IVec3::new(2, 0, -1);
    let pot = fire + IVec3::Y;
    touched.insert(fire);
    touched.insert(pot);
    host.world.set_v(fire, CAMPFIRE);
    host.world.set_v(pot, COOKING_POT);
    for item in [PORKCHOP, CARROT, SALT, BOWL] {
        host.give_peer(id, item, 1);
    }
    pump(host, team, 1.0, |_| idle());
    for item in [PORKCHOP, CARROT, SALT] {
        let g = &mut team[k].game;
        if let Some(slot) = g.inv.slots.iter().position(|s| s.is_some_and(|s| s.0 == item)) {
            g.inv.selected = slot;
            g.use_cooking_pot(pot);
        }
        pump(host, team, 0.4, |_| idle());
    }
    report.features.push("cooking pot");
    for b in team.iter() {
        if b.game.world.get_v(pot) != COOKING_POT_FULL {
            report.problems.push(format!("{} sees the pot as {}, not cooking", b.name, b.game.world.get_v(pot)));
        }
    }
    // (A stew wants half a minute on the fire.)
    if let Some(p) = host.pots.get_mut(&pot) {
        p.cooked = 30.0;
    }
    pump(host, team, 1.0, |_| idle());
    let g = &mut team[k].game;
    if let Some(slot) = g.inv.slots.iter().position(|s| s.is_some_and(|s| s.0 == BOWL)) {
        g.inv.selected = slot;
        g.use_cooking_pot(pot);
    }
    pump(host, team, 1.5, |_| idle());
    let stew = crate::cooking::Dish::HeartyStew.item();
    if !team[k].game.inv.slots.iter().any(|s| s.is_some_and(|s| s.0 == stew)) {
        report.problems.push(format!("{} served from a pot and got no stew", team[k].name));
    }
    if team[k].game.cookbook[crate::cooking::Dish::HeartyStew.index()] == 0 {
        report.problems.push(format!("{} made a stew and it's not in their Cookbook", team[k].name));
    }
    for b in team.iter() {
        if b.game.world.get_v(pot) != COOKING_POT {
            report.problems.push(format!("{} sees the pot still full after serving", b.name));
        }
    }
    host.world.set_v(pot, AIR);
    host.world.set_v(fire, AIR);

    // 42. A bot wears a skin of its own: the host and every other bot get it,
    // and it goes again when the bot changes back.
    let k = 5 % n;
    let id = team[k].game.my_id;
    let card = crate::skins::test_card();
    team[k].game.set_own_skin(Some(card.clone()));
    pump(host, team, 1.5, |_| idle());
    report.features.push("own skin");
    if host.peers.get(&id).and_then(|p| p.own_skin.as_ref()) != Some(&card) {
        report.problems.push(format!("{} put on a skin of its own and the host doesn't have it", team[k].name));
    }
    for (j, b) in team.iter().enumerate().filter(|(j, _)| *j != k) {
        if b.game.peers.get(&id).and_then(|p| p.own_skin.as_ref()) != Some(&card) {
            report.problems.push(format!("{} never saw {}'s own skin", team[j].name, team[k].name));
        }
    }
    team[k].game.set_own_skin(None);
    pump(host, team, 2.0, |_| idle());
    for b in team.iter().filter(|b| b.game.my_id != id) {
        if b.game.peers.get(&id).is_some_and(|p| p.own_skin.is_some()) {
            report.problems.push(format!("{} still sees {}'s old skin", b.name, team[k].name));
        }
    }

    // 43. While everyone plays, a server list asks the host how it is (over
    // its own encrypted connection) and finds it on the LAN.
    let port = match &host.net {
        Some(crate::multiplayer::Net::Host(s)) => s.port,
        _ => 0,
    };
    let asking = std::thread::spawn(move || crate::servers::ask_status(&format!("127.0.0.1:{port}")));
    let mut scan = crate::servers::LanScan::new();
    let start = Instant::now();
    while !asking.is_finished() && start.elapsed() < Duration::from_secs(10) {
        pump(host, team, 0.1, |_| idle());
        if let Some(s) = &mut scan {
            s.tick();
        }
    }
    for _ in 0..30 {
        if scan.as_ref().is_none_or(|s| s.games.iter().any(|g| g.info.port == port)) {
            break;
        }
        pump(host, team, 0.1, |_| idle());
        if let Some(s) = &mut scan {
            s.tick();
        }
    }
    report.features.push("server status");
    match asking.join() {
        Ok(Ok(st)) if st.players as usize == n && st.protocol == PROTOCOL => {}
        Ok(Ok(st)) => report.problems.push(format!("the server list was told {} players (protocol {}), not {n}", st.players, st.protocol)),
        Ok(Err(e)) => report.problems.push(format!("the server list couldn't ask the host: {e}")),
        Err(_) => report.problems.push("asking the host's status crashed".into()),
    }
    if scan.is_some_and(|s| !s.games.iter().any(|g| g.info.port == port)) {
        report.problems.push("the host never turned up on the LAN".into());
    }

    // 44. A bot opens a trapped chest: the lamp beside it lights for everyone,
    // and goes out again when the bot closes it.
    let k = 6 % n;
    ground(host, team, k);
    let feet = team[k].game.player.body.pos.floor().as_ivec3();
    pad(host, touched, feet, 4);
    let chest = feet + IVec3::new(2, 0, 0);
    let lamp = chest + IVec3::Z;
    for p in [chest, lamp] {
        touched.insert(p);
    }
    host.world.set_v(chest, TRAPPED_CHEST);
    host.world.set_v(lamp, LAMP);
    pump(host, team, 1.0, |_| idle());
    team[k].game.open_container(chest);
    pump(host, team, 1.0, |_| idle());
    report.features.push("trapped chest");
    if host.world.get_v(chest) != TRAPPED_CHEST_ON {
        report.problems.push(format!("{} opened a trapped chest and the host's isn't sprung", team[k].name));
    }
    for b in team.iter() {
        if b.game.world.get_v(lamp) != LAMP_ON {
            report.problems.push(format!("{} never saw the lamp by the sprung trapped chest light", b.name));
        }
    }
    team[k].game.close_container();
    pump(host, team, 1.0, |_| idle());
    for b in team.iter() {
        if b.game.world.get_v(lamp) != LAMP {
            report.problems.push(format!("{} still sees the trapped chest's lamp lit after it was shut", b.name));
        }
    }
    host.world.set_v(chest, AIR);
    host.world.set_v(lamp, AIR);

    // 45. A daylight sensor wakes up with the sun: everyone sees its lamp.
    let sensor = feet + IVec3::new(-2, 0, 1);
    let lamp = sensor + IVec3::X;
    for p in [sensor, lamp] {
        touched.insert(p);
    }
    host.world.set_v(sensor, DAYLIGHT_SENSOR);
    host.world.set_v(lamp, LAMP);
    let before = host.time;
    host.time = 0.3;
    pump(host, team, 2.0, |_| idle());
    report.features.push("daylight sensor");
    for b in team.iter() {
        if b.game.world.get_v(lamp) != LAMP_ON {
            report.problems.push(format!("{} never saw the daylight sensor's lamp come on in the morning", b.name));
        }
    }
    host.time = before;
    host.world.set_v(sensor, AIR);
    host.world.set_v(lamp, AIR);

    // 46. With a mod's dimension installed: a bot goes through its portal,
    // and the host and the bot agree where it is.
    if !crate::block::reg().dims.is_empty() {
        let k = 7 % n;
        revive(host, team);
        ground(host, team, k);
        let feet = team[k].game.player.body.pos.floor().as_ivec3();
        pad(host, touched, feet, 4);
        let base = feet + IVec3::new(0, 0, 3);
        crate::scorch::build_portal_of(&mut host.world, base, Some(0));
        for dx in -1..=2 {
            for dy in -1..=3 {
                touched.insert(base + IVec3::new(dx, dy, 0));
            }
        }
        pump(host, team, 1.0, |_| idle());
        let id = team[k].game.my_id;
        through_portal(host, team, k, base);
        report.features.push("mod dimension");
        let there = crate::dims::Dim::Mod(0);
        if team[k].game.dim != there || host.peer_dim(id) != Some(there) {
            report.problems.push(format!("{} walked into a mod's portal and is in {:?} (the host says {:?})", team[k].name, team[k].game.dim, host.peer_dim(id)));
        }
        // Home again.
        team[k].game.dead = Some("fell off the moon".into());
        team[k].game.respawn();
        pump(host, team, 2.0, |_| idle());
        load_around(&mut team[k].game, host.spawn);
        if team[k].game.dim != crate::dims::Dim::Over {
            report.problems.push(format!("{} respawned and is still in {:?}", team[k].name, team[k].game.dim));
        }
    }
}

/// Shift-click the first slot of the open container holding `item` (into the inventory).
fn take_out(g: &mut Game, item: Id) {
    let Some(p) = g.open else { return };
    let slot = crate::containers::store_ref(&g.world, &g.vehicles, p).and_then(|c| c.slots.iter().position(|s| s.is_some_and(|s| s.0 == item)));
    if let Some(i) = slot {
        g.container_click(i, false, true);
    }
}

/// Shift-click `item` from the inventory into the open container.
fn put_in(g: &mut Game, item: Id) {
    if let Some(slot) = g.inv.slots.iter().position(|s| s.is_some_and(|s| s.0 == item)) {
        g.container_quick_put(slot);
    }
}

/// v0.4 part 4: teams and friendly fire, a Trading Table, a minigame's
/// sidebar, land claims and the post.
fn together(host: &mut Game, team: &mut [Bot], report: &mut Report, touched: &mut HashSet<IVec3>) {
    let n = team.len();
    revive(host, team);
    for k in 0..n.min(3) {
        ground(host, team, k);
    }
    let (a, b) = (0, 1 % n);
    let (name_a, name_b) = (team[a].name.clone(), team[b].name.clone());
    let (id_a, id_b) = (team[a].game.my_id, team[b].game.my_id);

    // 47. Two bots make and join a team by command: everyone agrees who's on it,
    // and with /pvp on, teammates can't hurt each other but others can.
    if n >= 2 {
        team[a].game.send_chat("/team create Reds red");
        pump(host, team, 0.7, |_| idle());
        team[b].game.send_chat("/team join Reds");
        pump(host, team, 1.0, |_| idle());
        report.features.push("teams");
        if !host.teams.same_team(&name_a, &name_b) {
            report.problems.push(format!("{name_a} made a team and {name_b} joined it, and the host doesn't have them together"));
        }
        for bot in team.iter() {
            if bot.game.teams != host.teams {
                report.problems.push(format!("{} sees different teams from the host's", bot.name));
            }
        }
        host.pvp = true;
        host.share_teams();
        pump(host, team, 0.5, |_| idle());
        let before = team[a].game.player.health;
        team[b].game.net_send_msg(Msg::HitPlayer { target: id_a, dmg: 3.0 });
        pump(host, team, 0.7, |_| idle());
        if team[a].game.player.health < before {
            report.problems.push(format!("{name_b} hurt teammate {name_a} with friendly fire off"));
        }
        if n >= 3 {
            team[2].game.net_send_msg(Msg::HitPlayer { target: id_a, dmg: 3.0 });
            pump(host, team, 0.7, |_| idle());
            if team[a].game.player.health >= before {
                report.problems.push(format!("{} hit {name_a} with /pvp on and nothing happened", team[2].name));
            }
        }
        host.pvp = false;
        host.share_teams();
        revive(host, team);
    }

    // 48. Two bots swap at a Trading Table: coal for gold, once both accept.
    if n >= 2 {
        let feet = team[a].game.player.body.pos.floor().as_ivec3();
        let table = feet + IVec3::new(0, 0, 2);
        host.world.set_v(table - IVec3::Y, STONE);
        host.world.set_v(table, TRADING_TABLE);
        touched.insert(table);
        touched.insert(table - IVec3::Y);
        host.give_peer(id_a, COAL, 7);
        host.give_peer(id_b, GOLD_INGOT, 4);
        pump(host, team, 1.0, |_| idle());
        let (coal, gold) = (team[a].game.inv.count(COAL), team[b].game.inv.count(GOLD_INGOT));
        team[a].game.open_container(table);
        pump(host, team, 0.5, |_| idle());
        team[b].game.open_container(table);
        pump(host, team, 0.7, |_| idle());
        put_in(&mut team[a].game, COAL);
        put_in(&mut team[b].game, GOLD_INGOT);
        pump(host, team, 0.7, |_| idle());
        team[a].game.press_accept();
        team[b].game.press_accept();
        pump(host, team, 1.0, |_| idle());
        take_out(&mut team[a].game, GOLD_INGOT);
        take_out(&mut team[b].game, COAL);
        pump(host, team, 1.0, |_| idle());
        team[a].game.close_container();
        team[b].game.close_container();
        pump(host, team, 0.5, |_| idle());
        report.features.push("trading table");
        if host.deals.get(&table).map_or(0, |d| d.done) == 0 {
            report.problems.push(format!("{name_a} and {name_b} both accepted at a Trading Table and the host made no swap"));
        }
        if team[a].game.inv.count(GOLD_INGOT) == 0 || team[b].game.inv.count(COAL) < coal || team[a].game.inv.count(COAL) >= coal || team[b].game.inv.count(GOLD_INGOT) >= gold {
            report.problems.push(format!("after the swap {name_a} has {} gold and {} coal, {name_b} {} gold and {} coal", team[a].game.inv.count(GOLD_INGOT), team[a].game.inv.count(COAL), team[b].game.inv.count(GOLD_INGOT), team[b].game.inv.count(COAL)));
        }
        host.world.set_v(table, AIR);
    }

    // 49. A minigame's sidebar: everyone sees the host's scores.
    host.board.set("dug", &name_a, 7);
    host.board.set("dug", &name_b, 3);
    host.board.sidebar = Some(("dug".into(), "Dig Race".into()));
    pump(host, team, 1.0, |_| idle());
    report.features.push("sidebar");
    for bot in team.iter() {
        if bot.game.board.shown != host.board.shown || bot.game.board.shown.lines.is_empty() {
            report.problems.push(format!("{} sees the sidebar as {:?} (the host: {:?})", bot.name, bot.game.board.shown, host.board.shown));
        }
    }
    host.board = Default::default();
    pump(host, team, 0.5, |_| idle());

    // 50. Land claims: on the first bot's land, another can't build.
    if n >= 2 {
        let feet = team[a].game.player.body.pos.floor().as_ivec3();
        host.claims.on = true;
        let post = feet + IVec3::new(-2, 0, 0);
        if let Err(e) = host.claims.add(&name_a, post, 0) {
            report.problems.push(format!("couldn't claim land for {name_a}: {e}"));
        }
        host.share_claims();
        pump(host, team, 0.7, |_| idle());
        let spot = feet + IVec3::new(1, 0, -2);
        host.world.set_v(spot - IVec3::Y, STONE);
        host.world.set_v(spot, AIR);
        touched.insert(spot);
        touched.insert(spot - IVec3::Y);
        pump(host, team, 0.5, |_| idle());
        team[b].game.world.set_v(spot, COBBLE);
        pump(host, team, 1.5, |_| idle());
        report.features.push("claims");
        for bot in team.iter() {
            if bot.game.claims != host.claims {
                report.problems.push(format!("{} has different land claims from the host's", bot.name));
            }
        }
        if host.world.get_v(spot) != AIR {
            report.problems.push(format!("{name_b} built on {name_a}'s land"));
        }
        host.claims = Default::default();
        host.share_claims();
        pump(host, team, 0.5, |_| idle());
    }

    // 51. Post: one bot sends coal and a letter to another, who takes it out of their inbox.
    if n >= 2 {
        let feet = team[a].game.player.body.pos.floor().as_ivec3();
        let box_at = feet + IVec3::new(0, 0, -2);
        host.world.set_v(box_at - IVec3::Y, STONE);
        host.world.set_v(box_at, POST_BOX);
        touched.insert(box_at);
        touched.insert(box_at - IVec3::Y);
        host.give_peer(id_a, COAL, 5);
        pump(host, team, 1.0, |_| idle());
        let had = team[b].game.inv.count(COAL);
        team[a].game.open_post(false);
        pump(host, team, 0.7, |_| idle());
        put_in(&mut team[a].game, COAL);
        pump(host, team, 0.7, |_| idle());
        team[a].game.press_send(&name_b, "Coal for the kitchen.");
        pump(host, team, 1.0, |_| idle());
        team[a].game.close_container();
        report.features.push("post");
        if host.mail.parcels.len() != 1 {
            report.problems.push(format!("{name_a} posted something and the host has {} parcels on their way", host.mail.parcels.len()));
        }
        for p in host.mail.parcels.iter_mut() {
            p.due = 0.0;
        }
        pump(host, team, 1.5, |_| idle());
        let inbox = crate::post::inbox_key(&name_b);
        let waiting = host.world.mail_boxes.get(&inbox).map_or(0, |c| c.contents().iter().filter(|s| s.0 == COAL).map(|s| s.1 as u32).sum::<u32>());
        if waiting < 5 {
            report.problems.push(format!("{name_b}'s inbox has {waiting} coal after the post came"));
        }
        if team[b].game.letters_for(&name_b) != vec![(name_a.clone(), "Coal for the kitchen.".to_string())] {
            report.problems.push(format!("{name_b} has letters {:?}", team[b].game.letters_for(&name_b)));
        }
        if !team[b].game.advancements.has("youve_got_post") {
            report.problems.push(format!("{name_b} got post and no advancement"));
        }
        team[b].game.open_post(true);
        pump(host, team, 0.7, |_| idle());
        take_out(&mut team[b].game, COAL);
        pump(host, team, 0.7, |_| idle());
        team[b].game.close_container();
        pump(host, team, 0.5, |_| idle());
        if team[b].game.inv.count(COAL) < had + 5 {
            report.problems.push(format!("{name_b} took the coal out of their inbox and has {} (had {had})", team[b].game.inv.count(COAL)));
        }
        host.world.set_v(box_at, AIR);
    }
    let _ = id_b;
}

/// v0.5: a Hmmer with no job claims a bed and a Lectern near a bot and
/// becomes a Librarian, then goes to bed at night; everyone sees it.
fn village_life(host: &mut Game, team: &mut [Bot], report: &mut Report, touched: &mut HashSet<IVec3>) {
    use crate::entity::MobKind;
    use crate::village_life::JOBLESS;
    revive(host, team);
    let k = 0;
    ground(host, team, k);
    let feet = team[k].game.player.body.pos.floor().as_ivec3();
    pad(host, touched, feet, 6);
    let (bed, lectern) = (feet + IVec3::new(4, 0, 0), feet + IVec3::new(-4, 0, 0));
    host.world.set_v(bed, crate::beds::bed(1));
    for p in [bed, lectern] {
        touched.insert(p);
    }
    let id = host.alloc_mob(MobKind::Hmmer, feet.as_vec3() + Vec3::new(0.5, 0.0, 2.5));
    if let Some(m) = host.mobs.iter_mut().find(|m| m.id == id) {
        m.variant = JOBLESS;
        m.home = m.home.or(Some(m.body.pos));
        m.persistent = true;
    }
    let before = host.time;
    host.time = 0.1;
    // Everyone sees it out of work first; then along comes a Lectern.
    pump(host, team, 1.5, |_| idle());
    host.world.set_v(lectern, LECTERN);
    let librarian = 1 + crate::villagers::Job::Librarian.index();
    // (Every Hmmer in the world takes its turn to look round, a few a second.)
    for _ in 0..30 {
        pump(host, team, 0.5, |_| idle());
        if host.mobs.iter().any(|m| m.id == id && m.variant == librarian) {
            break;
        }
    }
    pump(host, team, 1.0, |_| idle());
    report.features.push("village life");
    let variant = host.mobs.iter().find(|m| m.id == id).map(|m| m.variant);
    if variant != Some(librarian) {
        report.problems.push(format!("a jobless Hmmer by a Lectern is still {variant:?} (and has claims {:?} / {:?})", host.life.beds.get(&id), host.life.stations.get(&id)));
    }
    for b in team.iter() {
        let theirs = b.game.mobs.iter().find(|m| m.id == id).map(|m| m.variant);
        if theirs != variant {
            report.problems.push(format!("{} sees the Hmmer as {theirs:?} (the host: {variant:?})", b.name));
        }
    }
    if !team[k].game.advancements.has("new_hire") {
        report.problems.push(format!("{} watched a Hmmer take a job and got no advancement", team[k].name));
    }
    // Night: off to bed.
    host.time = 0.7;
    for _ in 0..40 {
        pump(host, team, 0.5, |_| idle());
        if host.mobs.iter().any(|m| m.id == id && m.sitting) {
            break;
        }
    }
    pump(host, team, 1.0, |_| idle());
    if !host.mobs.iter().any(|m| m.id == id && m.sitting) {
        report.problems.push(format!("the Librarian never got into bed: {:?}", host.mobs.iter().find(|m| m.id == id).map(|m| m.body.pos)));
    }
    for b in team.iter() {
        if !b.game.mobs.iter().any(|m| m.id == id && m.sitting) {
            report.problems.push(format!("{} doesn't see the Librarian in bed", b.name));
        }
    }
    host.time = before;
    host.mobs.retain(|m| m.id != id);
    host.world.set_v(bed, AIR);
    host.world.set_v(lectern, AIR);
    pump(host, team, 0.5, |_| idle());
}

/// v0.5 part 2: a bot trades a Hmmer up to Apprentice (and sees its third
/// trade), the village warms to it, then it hits the Hmmer and the village
/// cools; the bot hears all of it as the host has it.
fn trade_levels(host: &mut Game, team: &mut [Bot], report: &mut Report, touched: &mut HashSet<IVec3>) {
    use crate::entity::MobKind;
    use crate::villagers::{level_of, Job};
    revive(host, team);
    let k = 1 % team.len();
    ground(host, team, k);
    let feet = team[k].game.player.body.pos.floor().as_ivec3();
    pad(host, touched, feet, 3);
    let id_k = team[k].game.my_id;
    let name = team[k].name.clone();
    let hmmer = host.alloc_mob(MobKind::Hmmer, feet.as_vec3() + Vec3::new(1.5, 0.0, 0.5));
    let farmer = (0..64u32).find(|&s| Job::of(s) == Job::Farmer).unwrap_or(0);
    if let Some(m) = host.mobs.iter_mut().find(|m| m.id == hmmer) {
        m.seed = farmer;
        m.variant = 1 + Job::Farmer.index();
        m.persistent = true;
    }
    // One trade short of Apprentice.
    host.hmmer_xp.insert(hmmer, crate::villagers::LEVELS[1].1 - 1);
    host.give_peer(id_k, WHEAT, 40);
    pump(host, team, 1.0, |_| idle());
    team[k].game.open_trade(hmmer);
    pump(host, team, 0.7, |_| idle());
    let before = team[k].game.trade_list().map_or(0, |l| l.1.len());
    team[k].game.make_trade(0);
    pump(host, team, 1.0, |_| idle());
    // (Ask again, as the trade screen does when it opens.)
    team[k].game.open_trade(hmmer);
    pump(host, team, 0.7, |_| idle());
    report.features.push("trade levels");
    let host_xp = host.hmmer_xp_of(hmmer);
    if level_of(host_xp) != 1 {
        report.problems.push(format!("{name} traded with a Hmmer one short of Apprentice and the host has it at {host_xp}"));
    }
    let after = team[k].game.trade_list().map_or(0, |l| l.1.len());
    if (before, after) != (2, 3) || team[k].game.hmmer_xp_of(hmmer) != host_xp {
        report.problems.push(format!("{name} saw {before} then {after} trades and experience {} (the host: {host_xp})", team[k].game.hmmer_xp_of(hmmer)));
    }
    let v = host.mob_village(hmmer);
    let liked = v.map_or(0, |v| host.rep.get(v, &name));
    if liked <= 0 || team[k].game.rep_seen.get(&hmmer).copied().unwrap_or(0) as i32 != liked {
        report.problems.push(format!("after a trade the village thinks {liked} of {name}, who was told {:?}", team[k].game.rep_seen.get(&hmmer)));
    }
    // Now a hit: the village thinks less of them, and they're told.
    team[k].game.net_send_msg(Msg::Attack { mob: hmmer, dmg: 1.0, from: team[k].game.player.eye() });
    pump(host, team, 1.0, |_| idle());
    report.features.push("reputation");
    let now = v.map_or(0, |v| host.rep.get(v, &name));
    if now >= liked {
        report.problems.push(format!("{name} hit a Hmmer and the village still thinks {now} of them (was {liked})"));
    }
    if team[k].game.rep_seen.get(&hmmer).copied().unwrap_or(0) as i32 != now {
        report.problems.push(format!("{name} was told {:?} after the hit (the host: {now})", team[k].game.rep_seen.get(&hmmer)));
    }
    team[k].game.trading = None;
    host.mobs.retain(|m| m.id != hmmer);
    pump(host, team, 0.5, |_| idle());
}

/// v0.5 part 3: a bot goes to a real village. Its Notice Board goes up; the
/// bot hands in a Bring bounty and claims a Defeat one (the host checks and
/// pays); then the village, full up, builds a house the bot sees go up.
fn growing_villages(host: &mut Game, team: &mut [Bot], report: &mut Report) {
    use crate::bounties::Task;
    use crate::entity::MobKind;
    revive(host, team);
    let k = team.len() - 1;
    let name = team[k].name.clone();
    let id_k = team[k].game.my_id;
    let Some(o) = host.world.nearest_site(crate::structures::Kind::Village, host.spawn, 60) else {
        report.problems.push("no village anywhere near spawn to try growing villages in".into());
        return;
    };
    let key = o + IVec3::Y;
    let (cx, cz) = (o.x.div_euclid(crate::world::CW), o.z.div_euclid(crate::world::CW));
    for dz in -2..=2 {
        for dx in -2..=2 {
            host.world.load_now(cx + dx, cz + dz);
        }
    }
    // The bot walks (well, appears) onto the square, and waits for the land to arrive.
    let spot = o.as_vec3() + Vec3::new(-1.5, 1.05, 0.5);
    let home = team[k].game.player.body.pos;
    // (The host moves them, as its own teleports do, so it doesn't think they're cheating.)
    if let Some(p) = host.peers.get_mut(&id_k) {
        p.target = spot;
    }
    let g = &mut team[k].game;
    g.player.body.pos = spot;
    g.player.body.vel = Vec3::ZERO;
    g.player.fall_start = spot.y;
    // (Bots have no renderer to load the land around them; the village spreads over a few chunks.)
    let start = Instant::now();
    let around = |g: &Game| (-1..=1).all(|dz| (-1..=1).all(|dx| g.world.is_loaded(o.x + dx * 24, o.z + dz * 24)));
    while !around(g) && start.elapsed() < Duration::from_secs(30) {
        g.world.stream(&[(spot, 3)]);
        std::thread::sleep(Duration::from_millis(2));
    }
    pump(host, team, 1.0, |_| idle());
    if !around(&team[k].game) {
        report.problems.push(format!("{name} never had the village at {o} load around them"));
    }
    let (time, day, difficulty) = (host.time, host.day, host.rules.difficulty);
    host.time = 0.1;
    // (Nothing to bother the bot while it's busy: a dead player isn't "there" to see things.)
    host.rules.difficulty = crate::rules::Difficulty::Peaceful;
    host.grow_village(key);
    pump(host, team, 1.0, |_| idle());
    let Some(site) = host.village_site(key) else {
        report.problems.push(format!("the village at {o} has no site"));
        return;
    };
    let board = crate::structures::village_board(site.origin);
    report.features.push("notice board");
    if host.world.get_v(board) != NOTICE_BOARD || team[k].game.world.get_v(board) != NOTICE_BOARD {
        report.problems.push(format!("the village's Notice Board: the host has {:?}, {name} sees {:?}", block(host.world.get_v(board)).key, block(team[k].game.world.get_v(board)).key));
    }
    host.world.set_v(board, NOTICE_BOARD);
    pump(host, team, 0.5, |_| idle());
    // A Bring bounty: the host hands over what it wants, the bot hands it in.
    let rows = host.rows_for(board, &crate::players::record_key(&name));
    let Task::Bring(item, n) = rows[0].bounty.task else { return };
    host.give_peer(id_k, item, n);
    pump(host, team, 1.0, |_| idle());
    team[k].game.open_board(board);
    pump(host, team, 1.0, |_| idle());
    let seen = team[k].game.board_rows().map(|r| r.1).unwrap_or_default();
    if seen.len() != crate::bounties::PER_BOARD || seen[0].bounty != rows[0].bounty {
        report.problems.push(format!("{name} read the board and saw {} bounties (the host: {:?})", seen.len(), rows.iter().map(|r| r.bounty.text()).collect::<Vec<_>>()));
    }
    let gold = team[k].game.inv.count(GOLD_INGOT);
    team[k].game.finish_bounty(0);
    pump(host, team, 1.0, |_| idle());
    report.features.push("bounties");
    let village = host.village_key(board.as_vec3());
    let me = crate::players::record_key(&name);
    if !host.bounties.done.contains(&(village, 0, me.clone())) || team[k].game.inv.count(GOLD_INGOT) != gold + rows[0].bounty.gold as u32 {
        report.problems.push(format!("{name} handed in \"{}\": the host has it done: {}, and their gold went {gold} -> {}", rows[0].bounty.text(), host.bounties.done.contains(&(village, 0, me.clone())), team[k].game.inv.count(GOLD_INGOT)));
    }
    if !team[k].game.advancements.earned.iter().any(|a| a == "help_wanted") {
        report.problems.push(format!("{name} finished a bounty but never heard about Help Wanted"));
    }
    // A Defeat bounty: the host counts what they beat.
    if let Task::Defeat(kind, n) = rows[2].bounty.task {
        for _ in 0..n {
            host.bounty_kill(&name, kind);
        }
        team[k].game.open_board(board);
        pump(host, team, 1.0, |_| idle());
        let have = team[k].game.board_rows().map_or(0, |r| r.1[2].have);
        team[k].game.finish_bounty(2);
        pump(host, team, 1.0, |_| idle());
        if have != n as u16 || !host.bounties.done.contains(&(village, 2, me.clone())) {
            report.problems.push(format!("{name} beat {n} for \"{}\", saw {have} on the board, and the host has it done: {}", rows[2].bounty.text(), host.bounties.done.contains(&(village, 2, me))));
        }
    }
    team[k].game.notice = None;
    // (Back on its feet, and back on the square, if anything happened to it.)
    if team[k].game.dead.is_some() {
        revive(host, team);
        if let Some(p) = host.peers.get_mut(&id_k) {
            p.target = spot;
        }
        team[k].game.player.body.pos = spot;
        team[k].game.player.fall_start = spot.y;
        pump(host, team, 1.0, |_| idle());
    }
    // Every bed taken and a couple of Hmmers about: a new house goes up.
    let mut taken = HashSet::new();
    while let Some(b) = crate::village_life::nearest(&host.world, key.as_vec3(), crate::beds::is_bed, &taken) {
        taken.insert(b);
    }
    for (i, b) in taken.iter().enumerate() {
        host.life.beds.insert(900_000 + i as u32, *b);
    }
    let mut helpers = Vec::new();
    for i in 0..2 {
        let id = host.alloc_mob(MobKind::Hmmer, key.as_vec3() + Vec3::new(i as f32 * 2.0 - 1.0, 0.0, 4.5));
        if let Some(m) = host.mobs.iter_mut().find(|m| m.id == id) {
            m.home = Some(m.body.pos);
        }
        helpers.push(id);
    }
    host.day += 1;
    host.grow_village(key);
    report.features.push("growing village");
    let Some(build) = host.growth.villages.get(&key).and_then(|v| v.build) else {
        report.problems.push(format!("the village at {o} was full up and didn't start a house"));
        return;
    };
    for _ in 0..400 {
        if host.growth.villages.get(&key).is_none_or(|v| v.build.is_none()) {
            break;
        }
        host.grow_village(key);
        pump(host, team, 0.05, |_| idle());
    }
    pump(host, team, 1.5, |_| idle());
    let (at, facing, seed) = host.growth_spots(&site)[build.spot as usize];
    let plan = crate::village_growth::house_plan(at, facing, seed, host.growth_style(&site));
    let differ: Vec<String> = plan.iter().filter(|(p, id)| *id != AIR && team[k].game.world.get_v(*p) != host.world.get_v(*p)).map(|(p, _)| format!("{p}: {} not {}", block(team[k].game.world.get_v(*p)).key, block(host.world.get_v(*p)).key)).collect();
    if host.growth.villages[&key].houses != 1 || !differ.is_empty() {
        report.problems.push(format!("the new house: the host has built {} and {name} sees {} of its blocks differently ({:?})", host.growth.villages[&key].houses, differ.len(), &differ[..differ.len().min(4)]));
    }
    if !team[k].game.advancements.earned.iter().any(|a| a == "new_neighbours") {
        report.problems.push(format!("{name} watched the house go up but never heard about New Neighbours"));
    }
    // v0.5 part 4: the bot opens a Shop Stall (it's theirs now), puts out a
    // lot of bread at a bargain, and a Hmmer buys it; the gold is in their till.
    let stall = key + IVec3::new(4, 0, 4);
    host.world.set_v(stall, SHOP_STALL);
    host.give_peer(id_k, BREAD, 10);
    pump(host, team, 1.0, |_| idle());
    team[k].game.open_stall(stall);
    pump(host, team, 1.0, |_| idle());
    put_in(&mut team[k].game, BREAD);
    team[k].game.set_stall_price(0, 1);
    pump(host, team, 1.0, |_| idle());
    report.features.push("shop stall");
    let owner = host.shops.stalls.get(&stall).map(|s| s.owner.clone()).unwrap_or_default();
    let lot = host.world.containers.get(&stall).and_then(|c| c.slots[0]);
    let price = host.shops.stalls.get(&stall).map_or(0, |s| s.prices[0]);
    if owner != crate::players::record_key(&name) || lot != Some((BREAD, 10)) || price != 1 {
        report.problems.push(format!("{name}'s stall: owner {owner:?}, lot {lot:?}, price {price}"));
    }
    let shopper = host.alloc_mob(MobKind::Hmmer, stall.as_vec3() + Vec3::new(0.5, 0.0, 1.5));
    let sold = (0..20).any(|_| host.customer_buys(stall, shopper));
    pump(host, team, 1.0, |_| idle());
    let till = team[k].game.world.containers.get(&stall).and_then(|c| c.slots[crate::stalls::TILL]);
    if !sold || till != Some((GOLD_INGOT, 1)) {
        report.problems.push(format!("a Hmmer at {name}'s stall bought: {sold}, and {name} sees {till:?} in the till"));
    }
    if !team[k].game.advancements.earned.iter().any(|a| a == "open_for_business") {
        report.problems.push(format!("{name} sold something but never heard about Open for Business"));
    }
    team[k].game.close_container();
    host.mobs.retain(|m| m.id != shopper);
    // The village paper: yesterday the bot punched a tree, so it's on the front page.
    host.news.note(host.day.saturating_sub(1), crate::paper::Event::TreePunched, &name, "", 1);
    team[k].game.open_board(board);
    pump(host, team, 0.5, |_| idle());
    team[k].game.take_paper();
    pump(host, team, 1.0, |_| idle());
    team[k].game.notice = None;
    report.features.push("village paper");
    let paper = team[k].game.inv.slots.iter().position(|s| s.is_some_and(|s| s.0 == NEWSPAPER));
    match paper {
        Some(i) => {
            let wear = team[k].game.inv.wear[i];
            let mut read = None;
            for _ in 0..10 {
                read = team[k].game.read_paper(wear, 0.5);
                if read.is_some() {
                    break;
                }
                pump(host, team, 0.5, |_| idle());
            }
            match read {
                // (It may not lead: the bots get up to all sorts.)
                Some((_, _, stories)) if stories.iter().any(|s| s.0.contains(&name) || s.1.contains(&name)) => {}
                other => report.problems.push(format!("{name}'s paper doesn't mention them: {:?}", other.map(|o| o.2))),
            }
            if !team[k].game.advancements.earned.iter().any(|a| a == "extra_extra") {
                report.problems.push(format!("{name} read the paper but never heard about Extra! Extra!"));
            }
        }
        None => report.problems.push(format!("{name} asked the Notice Board for today's paper and got none")),
    }
    host.mobs.retain(|m| !helpers.contains(&m.id));
    host.life.beds.retain(|id, _| *id < 900_000);
    (host.time, host.day, host.rules.difficulty) = (time, day, difficulty);
    if let Some(p) = host.peers.get_mut(&id_k) {
        p.target = home;
    }
    team[k].game.player.body.pos = home;
    team[k].game.player.fall_start = home.y;
    ground(host, team, k);
}

/// The new land (generator 7): a bot goes to a beach and then a river near
/// spawn, the host and the bot build the same land there, and the bot hears
/// about both advancements.
fn new_land(host: &mut Game, team: &mut [Bot], report: &mut Report) {
    use crate::world::Biome;
    revive(host, team);
    let k = team.len() - 1;
    let name = team[k].name.clone();
    let id_k = team[k].game.my_id;
    if host.world.generator.opts.version < 7 {
        report.problems.push("the playtest world isn't on the newest generator".into());
        return;
    }
    let home = team[k].game.player.body.pos;
    let (sx, sz) = (host.spawn.x as i32, host.spawn.z as i32);
    let find = |g: &crate::world::Generator, want: Biome| {
        (0..240).find_map(|r: i32| {
            (-r..=r).flat_map(move |dz| (-r..=r).map(move |dx| (dx, dz))).filter(|(dx, dz)| dx.abs().max(dz.abs()) == r).map(|(dx, dz)| (sx + dx * 8, sz + dz * 8)).find(|&(x, z)| g.column(x, z).1 == want)
        })
    };
    report.features.push("rivers and beaches");
    for (want, adv) in [(Biome::Beach, "lifes_a_beach"), (Biome::River, "down_by_the_river")] {
        let Some((x, z)) = find(&host.world.generator, want) else {
            report.problems.push(format!("no {want:?} anywhere near spawn"));
            continue;
        };
        let (cx, cz) = (x.div_euclid(crate::world::CW), z.div_euclid(crate::world::CW));
        for dz in -1..=1 {
            for dx in -1..=1 {
                host.world.load_now(cx + dx, cz + dz);
            }
        }
        let sea = host.world.sea();
        let h = host.world.generator.column(x, z).0.max(sea);
        let spot = Vec3::new(x as f32 + 0.5, h as f32 + 1.05, z as f32 + 0.5);
        if let Some(p) = host.peers.get_mut(&id_k) {
            p.target = spot;
        }
        let g = &mut team[k].game;
        g.player.body.pos = spot;
        g.player.body.vel = Vec3::ZERO;
        g.player.fall_start = spot.y;
        let start = Instant::now();
        // (All of the patch compared below.)
        let around = |g: &Game| [(-3, -3), (3, -3), (-3, 3), (3, 3)].iter().all(|&(dx, dz)| g.world.is_loaded(x + dx, z + dz));
        while !around(g) && start.elapsed() < Duration::from_secs(30) {
            g.world.stream(&[(spot, 2)]);
            std::thread::sleep(Duration::from_millis(2));
        }
        pump(host, team, 1.5, |_| idle());
        // The same land on both sides: the column and its neighbours, from the bed up.
        let differ = (-3..=3).flat_map(|dz| (-3..=3).map(move |dx| (dx, dz))).flat_map(|(dx, dz)| (sea - 10..sea + 10).map(move |y| IVec3::new(x + dx, y, z + dz))).find(|&p| host.world.get_v(p) != team[k].game.world.get_v(p));
        if let Some(p) = differ {
            report.problems.push(format!("at the {want:?} by {x},{z}, the host has {} at {p} and {name} sees {}", block(host.world.get_v(p)).key, block(team[k].game.world.get_v(p)).key));
        }
        if !team[k].game.advancements.earned.iter().any(|a| a == adv) {
            report.problems.push(format!("{name} stood in the {want:?} at {x},{z} but never heard about {adv}"));
        }
    }
    if let Some(p) = host.peers.get_mut(&id_k) {
        p.target = home;
    }
    team[k].game.player.body.pos = home;
    team[k].game.player.fall_start = home.y;
    ground(host, team, k);
}

/// Natural terrain is regenerated independently by host and joined player.
/// Also round-trip a block edit at each location through the socket/ledger.
fn compare_generated_patch(host: &mut Game, team: &mut [Bot], report: &mut Report, touched: &mut HashSet<IVec3>, spot: Vec3, label: &str) {
    let k = team.len()-1;
    let id = team[k].game.my_id;
    let mid = spot.as_ivec3();
    let (cx,cz) = (mid.x.div_euclid(crate::world::CW),mid.z.div_euclid(crate::world::CW));
    for dz in -1..=1 { for dx in -1..=1 { host.world.load_now(cx+dx,cz+dz); } }
    if let Some(peer) = host.peers.get_mut(&id) { peer.target = spot; }
    {
        let g = &mut team[k].game;
        g.player.body.pos = spot;
        g.player.body.vel = Vec3::ZERO;
        g.player.fall_start = spot.y;
        let start = Instant::now();
        while !(-1..=1).all(|dz| (-1..=1).all(|dx| g.world.is_loaded((cx+dx)*16,(cz+dz)*16))) && start.elapsed()<Duration::from_secs(30) {
            g.world.stream(&[(spot,2)]);
            std::thread::sleep(Duration::from_millis(2));
        }
        if !g.world.is_loaded(mid.x,mid.z) { report.problems.push(format!("{label}: joined player's chunks timed out")); return; }
    }
    pump(host,team,1.5,|_|idle());
    // Full central chunk, including hidden foundations and cave decoration.
    let mismatch = (0..crate::world::CW).flat_map(|z| (0..crate::world::CW).map(move |x|(x,z))).flat_map(|(x,z)| (1..crate::world::CH).map(move |y| IVec3::new(cx*16+x,y,cz*16+z))).find(|&p| host.world.get_v(p)!=team[k].game.world.get_v(p));
    if let Some(p)=mismatch { report.problems.push(format!("{label}: regenerated blocks disagree at {p}")); }
    let edited = mid + IVec3::new(2,4,0);
    let previous = host.world.get_v(edited);
    host.world.set_v(edited, COBBLE);
    touched.insert(edited);
    pump(host,team,0.5,|_|idle());
    if team[k].game.world.get_v(edited)!=COBBLE { report.problems.push(format!("{label}: host edit didn't arrive")); }
    host.world.set_v(edited,previous);
    pump(host,team,0.5,|_|idle());
    if team[k].game.world.get_v(edited)!=previous { report.problems.push(format!("{label}: restored edit didn't arrive")); }
}

fn reshaped_depths(host: &mut Game, team: &mut [Bot], report: &mut Report, touched: &mut HashSet<IVec3>) {
    use crate::caves::CaveBiome;
    if host.world.generator.opts.version != crate::world::GenOptions::DEFAULT.version {
        report.problems.push("Part 2 playtest isn't using the newest generator".into()); return;
    }
    let k=team.len()-1;
    let home=team[k].game.player.body.pos;
    report.features.push("climate and depth caves");
    for want in [CaveBiome::Dripstone,CaveBiome::Lush,CaveBiome::DeepDark] {
        let mut found=None;
        'search: for z in (-2400..2400).step_by(64) {
            for x in (-2400..2400).step_by(64) {
                let g=&host.world.generator;
                let y=if want==CaveBiome::DeepDark { 10 } else { 32 };
                if g.cave_biome_at(x,y,z)!=want { continue; }
                let (cx,cz)=(x.div_euclid(16),z.div_euclid(16));
                let blocks=g.generate(cx,cz);
                for lz in 3..13 { for lx in 3..13 {
                    for y in 4..g.column(cx*16+lx,cz*16+lz).0-10 {
                        if g.cave_biome_at(cx*16+lx,y,cz*16+lz)==want && is_solid(blocks[crate::world::idx(lx,y-1,lz)]) && (0..3).all(|dy| blocks[crate::world::idx(lx,y+dy,lz)]==AIR) {
                            found=Some(Vec3::new((cx*16+lx) as f32+0.5,y as f32+0.05,(cz*16+lz) as f32+0.5));
                            break 'search;
                        }
                    }
                } }
            }
        }
        let Some(spot)=found else { report.problems.push(format!("no naturally open {want:?} cave")); continue; };
        compare_generated_patch(host,team,report,touched,spot,&format!("{want:?} cave"));
        let adv=match want { CaveBiome::Dripstone=>Some("stalac_tight"),CaveBiome::Lush=>Some("lush_life"),_=>None };
        if let Some(adv)=adv && !team[k].game.advancements.earned.iter().any(|a|a==adv) { report.problems.push(format!("cave visit didn't award {adv}")); }
    }
    report.features.push("settled structures");
    if let Some(at)=host.world.generator.nearest_site(crate::structures::Kind::Village,home,160) {
        compare_generated_patch(host,team,report,touched,at.as_vec3()+Vec3::new(5.5,2.05,0.5),"village foundation");
    } else { report.problems.push("no village in the placement playtest".into()); }
    if host.world.generator.opts.version >= 9 {
        report.features.push("broader terrain and rocky coasts");
        for rocky in [false,true] {
            let g=&host.world.generator;
            let found=(-4096..4096).step_by(32).flat_map(|z|(-4096..4096).step_by(32).map(move|x|(x,z)))
                .find(|&(x,z)| {
                    let (h,b)=g.column(x,z);
                    if rocky { h>=g.sea() && !matches!(b,crate::world::Biome::Snowy | crate::world::Biome::IceSpikes) && g.shore_shape(x,z,&g.place_v7(x,z)).1 }
                    else { b==crate::world::Biome::River }
                });
            if let Some((x,z))=found {
                let h=g.column(x,z).0.max(g.sea());
                compare_generated_patch(host,team,report,touched,Vec3::new(x as f32+0.5,h as f32+1.05,z as f32+0.5),if rocky { "rocky coast" } else { "bounded river" });
            } else { report.problems.push(if rocky { "no rocky coast in the placement playtest" } else { "no river in the placement playtest" }.into()); }
        }
    }
    // The CLI installs this mod before both host and clients build generators.
    if host.world.generator.biomes.iter().any(|b|b.climate.is_some()) {
        report.features.push("mod climate targets");
        let g=&host.world.generator;
        let found=(-2400..2400).step_by(32).flat_map(|z|(-2400..2400).step_by(32).map(move|x|(x,z))).find(|&(x,z)|g.mod_biome_at(x,z,g.column(x,z).1).is_some_and(|i|g.biomes[i].climate.is_some()));
        if let Some((x,z))=found {
            let h=g.column(x,z).0;
            compare_generated_patch(host,team,report,touched,Vec3::new(x as f32+0.5,h as f32+1.05,z as f32+0.5),"mod climate biome");
            if !team[k].game.advancements.earned.iter().any(|a|a=="homebrew") { report.problems.push("mod climate visit didn't award homebrew".into()); }
        } else { report.problems.push("no climate-target mod patch".into()); }
    }
    if let Some(peer)=host.peers.get_mut(&team[k].game.my_id) { peer.target=home; }
    team[k].game.player.body.pos=home;
    team[k].game.player.fall_start=home.y;
    ground(host,team,k);
}

/// A small mod the `--playtest` command installs, so the bots can try a mod's
/// dimension (see modgen.rs). (Not for the unit test: it would change the
/// blocks under every other test running alongside.)
const PLAYTEST_MOD: &str = "[mod]\nname = Playtest Moon\n[block moon_rock]\ntexture = stone\n[block moon_frame]\ntexture = white\n[dimension moon]\nname = The Playtest Moon\nstone = moon_rock\ntop = moon_rock\nunder = moon_rock\nframe = moon_frame\nheight = 40\nhills = 4\n\n[biome climate_fields]\nshare = 0.15\nclimate_temperature = 0.1\nclimate_moisture = -0.1\nclimate_continentalness = 0.3\nclimate_erosion = 0.2\ntop = moon_rock\nunder = moon_rock\n";

/// The `--playtest` command.
pub fn run(args: &[String]) -> i32 {
    let get = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).and_then(|v| v.parse::<f64>().ok());
    let bots = get("--bots").map(|v| v as usize).unwrap_or(3).clamp(1, 12);
    let seconds = get("--seconds").map(|v| v as f32).unwrap_or(20.0).clamp(2.0, 600.0);
    let seed = get("--seed").map(|v| v as u32).unwrap_or(1234);
    let port = get("--port").map(|v| v as u16).unwrap_or(crate::net::DEFAULT_PORT);
    println!("Playtest: a host and {bots} bots for {seconds} seconds (seed {seed})...");
    crate::mods::install_sources(vec![crate::mods::ModSource { id: "playtestmoon".into(), files: [("mod.txt".to_string(), PLAYTEST_MOD.as_bytes().to_vec())].into_iter().collect() }], &[]);
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
        let _lock = crate::mods::registry_test_lock();
        let r = super::playtest(2, 5.0, 77, 26150).expect("it runs");
        assert!(r.ok(), "{:#?}", r.problems);
        // Long enough that more chat goes by than a player's chat log keeps.
        let r = super::playtest(6, 60.0, 1234, 26170).expect("it runs");
        assert!(r.chats > 110, "only {} lines", r.chats);
        assert!(r.placed > 0 && r.chats > 0, "the bots did nothing: {r:?}");
        assert_eq!(r.features, ["glide", "spear", "box", "spectator", "stats", "table", "honey", "brush", "smithing", "death message", "falling", "music", "fireball", "raid", "totem", "campfire", "llama", "armour stand", "glow berries", "tripwire", "allay", "witch potion", "rope", "wilter", "starred beacon", "cave-in", "birch door", "acacia boat", "mushmooer", "kelp", "huge fungus", "tusker", "soul fire", "sporeling", "snout chest", "dimensions", "cauldron", "composter", "lead", "donkey pack", "galloper armour", "cooking pot", "own skin", "server status", "trapped chest", "daylight sensor", "teams", "trading table", "sidebar", "claims", "post", "village life", "trade levels", "reputation", "notice board", "bounties", "growing village", "shop stall", "village paper", "rivers and beaches", "climate and depth caves", "settled structures", "broader terrain and rocky coasts"]);
        assert!(r.ok(), "{:#?}", r.problems);
    }
}
