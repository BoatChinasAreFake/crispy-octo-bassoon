//! Commands for operators and creative builders: /tp, /give, /gamemode,
//! /weather, /seed, /summon, /locate, /setblock and /kill. They run where the world lives (single player, the
//! host, a dedicated server) through `admin_command`, so joined players can
//! use them too when they're operators. Hardcore worlds allow no cheating.

use crate::admin::Caller;
use crate::block::*;
use crate::game::Game;
use crate::modes::GameMode;
use crate::scripting::Cmd;
use macroquad::math::Vec3;

/// The commands this module answers (without the slash).
pub const WORDS: [&str; 11] = ["tp", "teleport", "give", "gamemode", "gm", "weather", "seed", "summon", "locate", "setblock", "kill"];

pub const HELP: &str = "tp [player] <x y z | player>, give [player] <item> [count], gamemode <survival|creative|spectator> [player], weather <clear|rain|thunder>, seed, summon <mob> [x y z] [count], locate <structure|biome>, setblock <x y z> <block>, kill [player | mobs | <mob>]";

/// Find a block or item by key ("oak_log"), by key without the mod prefix, or
/// by its name ("Tree Chunk", "tree chunk", "tree_chunk").
pub fn find_item(name: &str) -> Option<Id> {
    let r = reg();
    let n = name.trim().to_ascii_lowercase().replace(' ', "_");
    if n.is_empty() {
        return None;
    }
    if let Some(id) = r.lookup(&n) {
        return Some(id).filter(|&id| id != AIR);
    }
    if let Some(i) = r.blocks.iter().position(|b| b.key.ends_with(&format!(":{n}"))) {
        return Some(i as Id);
    }
    // By display name, ignoring the joke in brackets: "Dimond Pickaxe (Shiny)" -> "dimond_pickaxe".
    let plain = |s: &str| s.split(" (").next().unwrap_or(s).trim().to_ascii_lowercase().replace(' ', "_");
    for id in 1..r.blocks.len() as Id {
        if r.blocks[id as usize].creative && plain(r.blocks[id as usize].name) == n {
            return Some(id);
        }
    }
    r.items.iter().enumerate().find(|(_, it)| it.real && plain(it.name) == n).map(|(i, _)| i as Id + FIRST_ITEM)
}

/// "12", "~", "~-3": a coordinate, maybe relative to `base`.
fn coord(s: &str, base: f32) -> Option<f32> {
    let v = match s.strip_prefix('~') {
        Some("") => base,
        Some(rel) => base + rel.parse::<f32>().ok()?,
        None => s.parse::<f32>().ok()?,
    };
    (v.is_finite() && v.abs() < 30_000_000.0).then_some(v)
}

impl Game {
    /// Where a player (by name) is.
    fn player_pos(&self, name: &str) -> Option<Vec3> {
        if self.is_local_player(name) {
            Some(self.player.body.pos)
        } else {
            self.peer_by_name(name).and_then(|id| self.peers.get(&id)).map(|p| p.target)
        }
    }

    fn is_player(&self, name: &str) -> bool {
        (!self.dedicated && name.eq_ignore_ascii_case(&self.player_name)) || self.peer_by_name(name).is_some()
    }

    /// Run a cheat command. `me` is the caller's name ("" for the console).
    pub fn cheat_command(&mut self, who: Caller, word: &str, rest: &str) -> Vec<String> {
        let me = match who {
            Caller::Console => String::new(),
            Caller::Host => self.player_name.clone(),
            Caller::Player(id) => self.peer_name(id),
        };
        let args: Vec<&str> = rest.split_whitespace().collect();
        if self.rules.hardcore && word != "seed" && !(word == "weather" || matches!(who, Caller::Console)) {
            return vec!["This is a hardcore world: no cheating.".into()];
        }
        match word {
            "seed" => vec![format!("Seed: {}", self.world.seed())],
            "tp" | "teleport" => self.cmd_tp(&me, &args),
            "give" => self.cmd_give(&me, &args),
            "gamemode" | "gm" => self.cmd_gamemode(&me, &args),
            "summon" => self.cmd_summon(&me, &args),
            "locate" => self.cmd_locate(&me, &args),
            "setblock" => self.cmd_setblock(&me, &args),
            "kill" => self.cmd_kill(&me, &args),
            "weather" => {
                let kind = match args.first().map(|a| a.to_ascii_lowercase()) {
                    Some(a) if a == "clear" || a == "sun" => crate::weather::Weather::Clear,
                    Some(a) if a == "rain" || a == "snow" => crate::weather::Weather::Rain,
                    Some(a) if a == "thunder" || a == "storm" => crate::weather::Weather::Thunder,
                    _ => return vec!["Usage: weather <clear|rain|thunder>".into()],
                };
                self.set_weather(kind);
                vec![format!("Weather set to {}.", args[0].to_ascii_lowercase())]
            }
            _ => Vec::new(),
        }
    }

    fn cmd_tp(&mut self, me: &str, args: &[&str]) -> Vec<String> {
        // Who moves: the first word if it names a player (and more follows), else the caller.
        let (who, rest) = match args {
            [name, rest @ ..] if !rest.is_empty() && self.is_player(name) => (name.to_string(), rest),
            _ => (me.to_string(), args),
        };
        if who.is_empty() {
            return vec!["The console has no body; say who to move: tp <player> <x y z | player>".into()];
        }
        let Some(from) = self.player_pos(&who) else { return vec![format!("No player called {who}.")] };
        let to = match rest {
            [x, y, z] => match (coord(x, from.x), coord(y, from.y), coord(z, from.z)) {
                (Some(x), Some(y), Some(z)) => Vec3::new(x, y, z),
                _ => return vec!["Usage: tp [player] <x y z> (numbers, or ~ for here)".into()],
            },
            [target] => match self.player_pos(target) {
                Some(p) => p,
                None => return vec![format!("No player called {target}.")],
            },
            _ => return vec!["Usage: tp [player] <x y z | player>".into()],
        };
        let to = Vec3::new(to.x, to.y.clamp(-30.0, crate::world::CH as f32 + 60.0), to.z);
        self.apply_cmds(vec![Cmd::Teleport(who.clone(), to)]);
        vec![format!("Teleported {who} to {:.1}, {:.1}, {:.1}.", to.x, to.y, to.z)]
    }

    fn cmd_give(&mut self, me: &str, args: &[&str]) -> Vec<String> {
        let (who, rest) = match args {
            [name, rest @ ..] if !rest.is_empty() && self.is_player(name) && find_item(name).is_none() => (name.to_string(), rest),
            _ => (me.to_string(), args),
        };
        if who.is_empty() {
            return vec!["Say who gets it: give <player> <item> [count]".into()];
        }
        // "give diamond 5", "give tree chunk", "give dimond pickaxe 1".
        let (name_words, count) = match rest.split_last() {
            Some((last, init)) if !init.is_empty() && last.parse::<u32>().is_ok() => (init, last.parse::<u32>().unwrap()),
            _ => (rest, 1),
        };
        let name = name_words.join(" ");
        let Some(item) = find_item(&name) else { return vec![format!("No block or item called \"{name}\".")] };
        let count = count.clamp(1, 36 * 64);
        let stack = if item >= FIRST_ITEM { reg().items[(item - FIRST_ITEM) as usize].stack.max(1) as u32 } else { 64 };
        let mut left = count;
        let mut cmds = Vec::new();
        while left > 0 {
            let n = left.min(stack);
            cmds.push(Cmd::Give(who.clone(), item, n as u8));
            left -= n;
        }
        self.apply_cmds(cmds);
        vec![format!("Gave {count} {} to {who}.", item_name(item))]
    }

    /// Where the caller is (the console uses spawn), and which way they face.
    fn caller_spot(&self, me: &str) -> (Vec3, Vec3) {
        if self.is_local_player(me) {
            return (self.player.body.pos, self.player.look_dir());
        }
        match self.peer_by_name(me).and_then(|id| self.peers.get(&id)) {
            Some(p) => (p.target, Vec3::new(p.yaw.sin(), 0.0, -p.yaw.cos())),
            None => (self.spawn, Vec3::Z),
        }
    }

    fn cmd_summon(&mut self, me: &str, args: &[&str]) -> Vec<String> {
        let usage = || vec!["Usage: summon <mob> [x y z] [count]".to_string()];
        let Some(kind) = args.first().and_then(|a| crate::entity::MobKind::from_name(a)) else { return usage() };
        if kind == crate::entity::MobKind::Wyrm {
            return vec!["The Wyrm only comes when it's good and ready (see the Hollow).".into()];
        }
        let (here, look) = self.caller_spot(me);
        let base = here + Vec3::new(look.x, 0.0, look.z).normalize_or_zero() * 3.0;
        let (at, rest) = match &args[1..] {
            [x, y, z, rest @ ..] => match (coord(x, here.x), coord(y, here.y), coord(z, here.z)) {
                (Some(x), Some(y), Some(z)) => (Vec3::new(x, y, z), rest),
                _ => return usage(),
            },
            rest => (base, rest),
        };
        let n = rest.first().and_then(|c| c.parse::<u32>().ok()).unwrap_or(1).clamp(1, 20);
        for i in 0..n {
            self.alloc_mob(kind, at + Vec3::new((i % 5) as f32 * 0.8, 0.0, (i / 5) as f32 * 0.8));
        }
        vec![format!("Summoned {n} {}.", kind.name())]
    }

    fn cmd_locate(&mut self, me: &str, args: &[&str]) -> Vec<String> {
        let what = args.join(" ");
        if what.is_empty() {
            return vec!["Usage: locate <structure|biome> (village, dungeon, tower, hut, well, desert ruins, trail ruins, ocean ruins, hushed city, or a biome)".into()];
        }
        let (here, _) = self.caller_spot(me);
        let g = self.world.generator.clone();
        if let Some(kind) = crate::structures::Kind::from_name(&what) {
            return match g.nearest_site(kind, here, 160) {
                Some(p) => {
                    let d = p.as_vec3() - here;
                    vec![format!("The nearest {} is at {}, {}, {} ({:.0} blocks {}).", kind.name(), p.x, p.y, p.z, Vec3::new(d.x, 0.0, d.z).length(), crate::archaeology::compass_word(d))]
                }
                None => vec![format!("No {} within 2,500 blocks.", kind.name())],
            };
        }
        if let Some(biome) = crate::world::Biome::from_name(&what) {
            let (cx, cz) = (here.x as i32, here.z as i32);
            for r in 0..160 {
                let step = 16;
                for k in 0..(r * 8).max(1) {
                    let a = k as f32 / (r * 8).max(1) as f32 * std::f32::consts::TAU;
                    let (x, z) = (cx + (a.cos() * (r * step) as f32) as i32, cz + (a.sin() * (r * step) as f32) as i32);
                    if g.column(x, z).1 == biome {
                        return vec![format!("The nearest {} is around {x}, {z} ({} blocks away).", biome.name(), r * step)];
                    }
                }
            }
            return vec![format!("No {} within 2,500 blocks.", biome.name())];
        }
        vec![format!("Don't know \"{what}\". Try a structure (village, hushed city, desert ruins...) or a biome.")]
    }

    fn cmd_setblock(&mut self, me: &str, args: &[&str]) -> Vec<String> {
        let (here, _) = self.caller_spot(me);
        let [x, y, z, name @ ..] = args else { return vec!["Usage: setblock <x y z> <block>".into()] };
        let (Some(x), Some(y), Some(z)) = (coord(x, here.x), coord(y, here.y), coord(z, here.z)) else {
            return vec!["Usage: setblock <x y z> <block> (numbers, or ~ for here)".into()];
        };
        let name = name.join(" ");
        let id = if name.eq_ignore_ascii_case("air") { Some(AIR) } else { find_item(&name).filter(|&i| i < FIRST_ITEM) };
        let Some(id) = id else { return vec![format!("No block called \"{name}\".")] };
        let p = macroquad::math::IVec3::new(x.floor() as i32, y.floor() as i32, z.floor() as i32);
        if !(1..crate::world::CH).contains(&p.y) {
            return vec!["That's outside the world.".into()];
        }
        self.world.set_v(p, id);
        vec![format!("Set {}, {}, {} to {}.", p.x, p.y, p.z, if id == AIR { "air" } else { item_name(id) })]
    }

    fn cmd_kill(&mut self, me: &str, args: &[&str]) -> Vec<String> {
        let target = args.first().copied().unwrap_or(me);
        if target.eq_ignore_ascii_case("mobs") || target.eq_ignore_ascii_case("monsters") {
            let monsters = target.eq_ignore_ascii_case("monsters");
            let before = self.mobs.len();
            self.mobs.retain(|m| m.persistent || m.kind == crate::entity::MobKind::Wyrm || (monsters && !m.kind.hostile()));
            return vec![format!("Removed {} mob{}.", before - self.mobs.len(), if before - self.mobs.len() == 1 { "" } else { "s" })];
        }
        if let Some(kind) = crate::entity::MobKind::from_name(target).filter(|_| !self.is_player(target)) {
            let before = self.mobs.len();
            self.mobs.retain(|m| m.kind != kind || m.persistent);
            return vec![format!("Removed {} {}.", before - self.mobs.len(), kind.name())];
        }
        if target.is_empty() {
            return vec!["Usage: kill [player | mobs | monsters | <mob>]".into()];
        }
        if self.is_local_player(target) {
            self.player.hurt = 0.0;
            let was = self.creative;
            self.creative = false;
            self.hurt_player(1000.0, "was struck down by a command. Harsh.");
            self.creative = was;
        } else if let Some(id) = self.peer_by_name(target) {
            self.hurt_peer(id, 1000.0, "was struck down by a command. Harsh.", Vec3::ZERO);
        } else {
            return vec![format!("No player or mob called {target}.")];
        }
        vec![format!("Goodbye, {target}.")]
    }

    fn cmd_gamemode(&mut self, me: &str, args: &[&str]) -> Vec<String> {
        let Some(mode) = args.first().and_then(|a| GameMode::from_name(a)) else {
            return vec!["Usage: gamemode <survival|creative|spectator> [player]".into()];
        };
        let who = args.get(1).map(|s| s.to_string()).unwrap_or_else(|| me.to_string());
        if who.is_empty() {
            return vec!["Say whose mode: gamemode <mode> <player>".into()];
        }
        if self.is_local_player(&who) {
            self.set_mode(mode);
        } else if let Some(id) = self.peer_by_name(&who) {
            self.set_peer_mode(id, mode);
        } else {
            return vec![format!("No player called {who}.")];
        }
        vec![format!("{who} is now in {} mode.", mode.name())]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_commands() {
        let mut g = Game::new(5, true, false);
        let start = std::time::Instant::now();
        while !g.world.chunks.contains_key(&(0, 0)) && start.elapsed().as_secs() < 20 {
            g.world.stream(&[(Vec3::ZERO, 1)]);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let out = g.cheat_command(Caller::Host, "summon", "fox 1 90 1 3");
        assert!(out[0].contains("Summoned 3 Sneaker"), "{out:?}");
        assert_eq!(g.mobs.iter().filter(|m| m.kind == crate::entity::MobKind::Sneaker).count(), 3);
        let out = g.cheat_command(Caller::Host, "kill", "sneaker");
        assert!(out[0].contains("Removed 3"), "{out:?}");
        let out = g.cheat_command(Caller::Host, "setblock", "2 90 2 beehive");
        assert!(out[0].starts_with("Set"), "{out:?}");
        assert_eq!(g.world.get(2, 90, 2), BEEHIVE);
        let out = g.cheat_command(Caller::Host, "locate", "desert");
        assert!(out[0].contains("nearest") || out[0].contains("No "), "{out:?}");
        let out = g.cheat_command(Caller::Host, "locate", "nonsense");
        assert!(out[0].starts_with("Don't know"));
    }

    #[test]
    fn items_are_found_by_key_or_name() {
        assert_eq!(find_item("stone"), Some(STONE));
        assert_eq!(find_item("Stone"), Some(STONE));
        assert_eq!(find_item("dimond"), Some(DIAMOND));
        assert_eq!(find_item("torch"), Some(TORCH));
        assert_eq!(find_item("unobtainium"), None);
        assert_eq!(find_item("air"), None);
    }

    #[test]
    fn relative_coordinates() {
        assert_eq!(coord("~", 5.0), Some(5.0));
        assert_eq!(coord("~-2", 5.0), Some(3.0));
        assert_eq!(coord("12.5", 5.0), Some(12.5));
        assert_eq!(coord("nope", 5.0), None);
    }

    #[test]
    fn cheats_work_for_the_host() {
        let mut g = Game::new(9, false, false);
        let before = g.inv.count(DIAMOND);
        let out = g.admin_command(Caller::Host, "/give dimond 70").unwrap();
        assert!(out[0].starts_with("Gave 70"), "{out:?}");
        assert_eq!(g.inv.count(DIAMOND), before + 70);
        g.admin_command(Caller::Host, "/tp 10 90 -4");
        assert_eq!(g.player.body.pos, Vec3::new(10.0, 90.0, -4.0));
        g.admin_command(Caller::Host, "/tp ~ ~5 ~");
        assert_eq!(g.player.body.pos.y, 95.0);
        g.admin_command(Caller::Host, "/gamemode creative");
        assert!(g.creative && !g.spectator);
        g.admin_command(Caller::Host, "/gm sp");
        assert!(g.spectator);
        g.admin_command(Caller::Host, "/weather thunder");
        assert_eq!(g.weather.kind, crate::weather::Weather::Thunder);
        assert_eq!(g.admin_command(Caller::Host, "/seed").unwrap(), vec!["Seed: 9".to_string()]);
        // Hardcore: no cheating.
        g.rules.hardcore = true;
        let out = g.admin_command(Caller::Host, "/give stone").unwrap();
        assert!(out[0].contains("hardcore"));
    }
}
