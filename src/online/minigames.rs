//! **Minigames for mods.** What a script needs to run a game on top of the
//! world (see SCRIPTING.md, Minigames):
//!
//! - **Scores**: any number of named objectives, each a score per player
//!   (`score_add("dug", player, 1)`).
//! - **The sidebar**: one objective shown down the right of everyone's
//!   screen, highest first, with a title.
//! - **Timers**: `timer_start("round", 120)` runs `on_timer("round")` when
//!   it's up; one timer can be shown as a countdown at the top of the screen.
//! - **Teams** and players hurting each other (see teams.rs), and where each
//!   player comes back when they die (`set_spawn`).
//!
//! It all lives with the world (saved with it), and the host sends what's on
//! screen (the sidebar and the countdown) to every player when it changes.

use std::collections::BTreeMap;

/// Most lines the sidebar shows, objectives, players an objective keeps, and timers.
pub const SIDEBAR_LINES: usize = 10;
pub const MAX_OBJECTIVES: usize = 32;
pub const MAX_ENTRIES: usize = 256;
pub const MAX_TIMERS: usize = 32;
/// Longest name of an objective, timer or player, and of a title.
pub const NAME_LEN: usize = 32;

/// What everyone sees: the sidebar's title and lines, and the countdown line.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shown {
    pub title: String,
    pub lines: Vec<(String, i32)>,
    pub countdown: String,
}

impl Shown {
    pub fn is_empty(&self) -> bool {
        self.title.is_empty() && self.lines.is_empty() && self.countdown.is_empty()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Board {
    /// Objective -> player -> score.
    pub scores: BTreeMap<String, BTreeMap<String, i64>>,
    /// The objective on the sidebar, and its title.
    pub sidebar: Option<(String, String)>,
    /// Timers counting down (seconds left).
    pub timers: BTreeMap<String, f32>,
    /// The timer shown at the top of the screen, and what it says before the time.
    pub countdown: Option<(String, String)>,
    /// What's on screen (worked out by the host, sent to joined players).
    pub shown: Shown,
}

pub fn clean(s: &str, max: usize) -> String {
    s.chars().filter(|c| !c.is_control()).take(max).collect::<String>().trim().to_string()
}

/// "1:05", "0:09".
pub fn clock(secs: f32) -> String {
    let s = secs.max(0.0).ceil() as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

impl Board {
    pub fn score(&self, objective: &str, player: &str) -> i64 {
        self.scores.get(objective).and_then(|o| o.iter().find(|(p, _)| p.eq_ignore_ascii_case(player))).map_or(0, |(_, s)| *s)
    }

    fn entry(&mut self, objective: &str, player: &str) -> Option<&mut i64> {
        let (objective, player) = (clean(objective, NAME_LEN), clean(player, NAME_LEN));
        if objective.is_empty() || player.is_empty() || (!self.scores.contains_key(&objective) && self.scores.len() >= MAX_OBJECTIVES) {
            return None;
        }
        let o = self.scores.entry(objective).or_default();
        // The same player whatever the capitals.
        let key = o.keys().find(|p| p.eq_ignore_ascii_case(&player)).cloned().unwrap_or(player);
        if !o.contains_key(&key) && o.len() >= MAX_ENTRIES {
            return None;
        }
        Some(o.entry(key).or_insert(0))
    }

    pub fn set(&mut self, objective: &str, player: &str, n: i64) {
        if let Some(s) = self.entry(objective, player) {
            *s = n;
        }
    }

    pub fn add(&mut self, objective: &str, player: &str, n: i64) {
        if let Some(s) = self.entry(objective, player) {
            *s = s.saturating_add(n);
        }
    }

    pub fn reset(&mut self, objective: &str) {
        self.scores.remove(objective);
    }

    pub fn start_timer(&mut self, name: &str, secs: f32) {
        let name = clean(name, NAME_LEN);
        if !name.is_empty() && secs.is_finite() && (self.timers.contains_key(&name) || self.timers.len() < MAX_TIMERS) {
            self.timers.insert(name, secs.clamp(0.0, 86_400.0));
        }
    }

    /// Run the timers down; returns the ones that just ran out (they're gone).
    pub fn tick(&mut self, dt: f32) -> Vec<String> {
        let mut done = Vec::new();
        for (name, left) in self.timers.iter_mut() {
            *left -= dt;
            if *left <= 0.0 {
                done.push(name.clone());
            }
        }
        for name in &done {
            self.timers.remove(name);
        }
        done
    }

    /// What everyone should see now.
    pub fn work_out_shown(&self) -> Shown {
        let mut s = Shown::default();
        if let Some((objective, title)) = &self.sidebar {
            s.title = if title.is_empty() { objective.clone() } else { title.clone() };
            let mut lines: Vec<(String, i32)> = self.scores.get(objective).map(|o| o.iter().map(|(p, &n)| (p.clone(), n.clamp(i32::MIN as i64, i32::MAX as i64) as i32)).collect()).unwrap_or_default();
            // Highest first, then by name.
            lines.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            lines.truncate(SIDEBAR_LINES);
            s.lines = lines;
        }
        if let Some((timer, label)) = &self.countdown
            && let Some(&left) = self.timers.get(timer)
        {
            s.countdown = if label.is_empty() { clock(left) } else { format!("{label} {}", clock(left)) };
        }
        s
    }

    /// The board for saving: one line per thing, fields split by tabs.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = String::new();
        for (o, players) in &self.scores {
            for (p, n) in players {
                out += &format!("s\t{o}\t{p}\t{n}\n");
            }
        }
        if let Some((o, t)) = &self.sidebar {
            out += &format!("b\t{o}\t{t}\n");
        }
        for (name, left) in &self.timers {
            out += &format!("t\t{name}\t{left}\n");
        }
        if let Some((timer, label)) = &self.countdown {
            out += &format!("c\t{timer}\t{label}\n");
        }
        out.into_bytes()
    }

    pub fn decode(bytes: &[u8]) -> Board {
        let mut b = Board::default();
        for line in String::from_utf8_lossy(bytes).lines() {
            let f: Vec<&str> = line.split('\t').collect();
            match f.as_slice() {
                ["s", o, p, n] => b.set(o, p, n.parse().unwrap_or(0)),
                ["b", o, t] => b.sidebar = Some((clean(o, NAME_LEN), clean(t, NAME_LEN))),
                ["t", name, left] => b.start_timer(name, left.parse().unwrap_or(0.0)),
                ["c", timer, label] => b.countdown = Some((clean(timer, NAME_LEN), clean(label, NAME_LEN))),
                _ => {}
            }
        }
        b
    }
}

// ------------------------------------------------------------------ in the game

use crate::game::Game;
use crate::net::Msg;

impl Game {
    /// The host (or single player): run the timers (each that runs out tells
    /// the scripts), and send what's on screen when it changes.
    pub fn minigame_tick(&mut self, dt: f32) {
        if self.is_client() {
            return;
        }
        for name in self.board.tick(dt) {
            self.fire("on_timer", vec![rhai::Dynamic::from(name)]);
        }
        let shown = self.board.work_out_shown();
        if shown != self.board.shown {
            self.board.shown = shown.clone();
            self.net_broadcast_all(Msg::Sidebar { title: shown.title, lines: shown.lines, countdown: shown.countdown });
        }
    }

    pub fn sidebar_msg(&self) -> Msg {
        let s = &self.board.shown;
        Msg::Sidebar { title: s.title.clone(), lines: s.lines.clone(), countdown: s.countdown.clone() }
    }

    /// A player comes back here when they die (a script's `set_spawn`).
    pub fn set_player_spawn(&mut self, name: &str, at: macroquad::math::Vec3) {
        if !at.is_finite() {
            return;
        }
        if self.is_local_player(name) {
            self.personal_spawn = Some(at);
        } else if let Some(id) = self.peer_by_name(name) {
            self.net_send_to(id, Msg::SetSpawn { pos: at });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scores_sidebar_and_timers() {
        let mut b = Board::default();
        b.add("dug", "Ann", 3);
        b.add("dug", "bob", 5);
        b.add("dug", "ANN", 4);
        assert_eq!(b.score("dug", "ann"), 7);
        assert_eq!(b.scores["dug"].len(), 2, "the same player whatever the capitals");
        b.set("dug", "Cat", 7);
        b.sidebar = Some(("dug".into(), "Dig Race".into()));
        b.start_timer("round", 61.0);
        b.countdown = Some(("round".into(), "Ends in".into()));
        let s = b.work_out_shown();
        assert_eq!(s.title, "Dig Race");
        assert_eq!(s.lines, vec![("Ann".to_string(), 7), ("Cat".to_string(), 7), ("bob".to_string(), 5)]);
        assert_eq!(s.countdown, "Ends in 1:01");
        // Timers run down, and say when they're done.
        assert!(b.tick(30.0).is_empty());
        assert_eq!(b.tick(31.5), vec!["round".to_string()]);
        assert!(b.timers.is_empty() && b.work_out_shown().countdown.is_empty());
        // Saved and back.
        b.start_timer("next", 5.0);
        assert_eq!(Board::decode(&b.encode()), b);
        b.reset("dug");
        assert!(b.work_out_shown().lines.is_empty());
    }

    #[test]
    fn the_dig_race_example_runs_a_whole_round() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("example-mods");
        let sources: Vec<_> = crate::mods::read_disk(&dir).into_iter().filter(|m| m.id == "digrace").collect();
        let (host, problems) = crate::scripting::ScriptHost::new(&sources);
        assert!(problems.is_empty(), "{problems:?}");
        let mut g = crate::game::tests::arena(980);
        g.player_name = "Stove".into();
        g.scripts = Some(host);
        assert!(!g.fire("on_chat", vec!["Stove".into(), "/digrace".into()]), "the command is handled");
        assert_eq!(g.teams.team_of("Stove").map(|t| t.name.as_str()), Some("Reds"));
        assert!(g.personal_spawn.is_some(), "the start is where we come back");
        for _ in 0..5 {
            g.fire("on_block_break", vec!["Stove".into(), rhai::Dynamic::from(0 as rhai::INT), rhai::Dynamic::from(50 as rhai::INT), rhai::Dynamic::from(0 as rhai::INT), "stone".into()]);
        }
        g.minigame_tick(0.1);
        assert_eq!(g.board.shown.title, "Dig Race");
        assert_eq!(g.board.shown.lines, vec![("Stove".to_string(), 5)]);
        assert!(g.board.shown.countdown.starts_with("Dig Race ends in 2:00") || g.board.shown.countdown.starts_with("Dig Race ends in 1:"), "{}", g.board.shown.countdown);
        // Two minutes on: the race ends, the Reds win cake.
        let cakes = g.inv.count(crate::block::CAKE);
        for _ in 0..1250 {
            g.minigame_tick(0.1);
        }
        assert_eq!(g.inv.count(crate::block::CAKE), cakes + 1);
        assert!(g.board.shown.countdown.is_empty());
        assert_eq!(g.board.shown.title, "Final score");
        // And then the scores go away.
        for _ in 0..200 {
            g.minigame_tick(0.1);
        }
        assert!(g.board.shown.is_empty());
    }

    #[test]
    fn limits_hold() {
        let mut b = Board::default();
        for i in 0..MAX_OBJECTIVES + 5 {
            b.add(&format!("o{i}"), "p", 1);
        }
        assert_eq!(b.scores.len(), MAX_OBJECTIVES);
        for i in 0..MAX_ENTRIES + 5 {
            b.add("o0", &format!("p{i}"), 1);
        }
        assert_eq!(b.scores["o0"].len(), MAX_ENTRIES);
        b.add("o0", "p0", i64::MAX);
        assert_eq!(b.score("o0", "p0"), i64::MAX);
        b.start_timer("t", f32::NAN);
        b.add("", "p", 1);
        b.add("x\ty", "p\nq", 1);
        assert!(b.timers.is_empty());
        assert_eq!(clock(125.2), "2:06");
    }
}
