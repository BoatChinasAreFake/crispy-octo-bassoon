//! Statistics: what you've done in this world (blocks mined, distance walked,
//! mobs defeated...), shown on the Statistics screen and kept in the save.

use crate::game::Game;
use macroquad::math::Vec3;

#[derive(Clone, Default, Debug, PartialEq)]
pub struct Stats {
    pub mined: u64,
    pub placed: u64,
    pub crafted: u64,
    pub kills: u64,
    pub deaths: u64,
    pub damage_dealt: f64,
    pub damage_taken: f64,
    /// Metres.
    pub walked: f64,
    pub swum: f64,
    pub flown: f64,
    pub ridden: f64,
    pub jumps: u64,
    /// Seconds in this world.
    pub played: f64,
    pub fish: u64,
    pub eaten: u64,
}

/// (key, label) in screen order, and how each is shown.
const FIELDS: [(&str, &str); 15] = [
    ("played", "Time played"),
    ("mined", "Blocks mined"),
    ("placed", "Blocks placed"),
    ("crafted", "Items crafted"),
    ("kills", "Mobs defeated"),
    ("deaths", "Deaths"),
    ("damage_dealt", "Damage dealt"),
    ("damage_taken", "Damage taken"),
    ("walked", "Distance walked"),
    ("swum", "Distance swum"),
    ("flown", "Distance flown"),
    ("ridden", "Distance ridden"),
    ("jumps", "Jumps"),
    ("fish", "Fish caught"),
    ("eaten", "Things eaten"),
];

impl Stats {
    fn get(&self, key: &str) -> f64 {
        match key {
            "mined" => self.mined as f64,
            "placed" => self.placed as f64,
            "crafted" => self.crafted as f64,
            "kills" => self.kills as f64,
            "deaths" => self.deaths as f64,
            "damage_dealt" => self.damage_dealt,
            "damage_taken" => self.damage_taken,
            "walked" => self.walked,
            "swum" => self.swum,
            "flown" => self.flown,
            "ridden" => self.ridden,
            "jumps" => self.jumps as f64,
            "played" => self.played,
            "fish" => self.fish as f64,
            "eaten" => self.eaten as f64,
            _ => 0.0,
        }
    }

    fn set(&mut self, key: &str, v: f64) {
        let v = if v.is_finite() { v.max(0.0) } else { 0.0 };
        let n = v.min(u64::MAX as f64 / 2.0) as u64;
        match key {
            "mined" => self.mined = n,
            "placed" => self.placed = n,
            "crafted" => self.crafted = n,
            "kills" => self.kills = n,
            "deaths" => self.deaths = n,
            "damage_dealt" => self.damage_dealt = v,
            "damage_taken" => self.damage_taken = v,
            "walked" => self.walked = v,
            "swum" => self.swum = v,
            "flown" => self.flown = v,
            "ridden" => self.ridden = v,
            "jumps" => self.jumps = n,
            "played" => self.played = v,
            "fish" => self.fish = n,
            "eaten" => self.eaten = n,
            _ => {}
        }
    }

    /// "key=value" lines: unknown keys are skipped, so the list can grow.
    pub fn encode(&self) -> Vec<u8> {
        FIELDS.iter().map(|(k, _)| format!("{k}={}\n", self.get(k))).collect::<String>().into_bytes()
    }

    pub fn decode(bytes: &[u8]) -> Stats {
        let mut s = Stats::default();
        for line in String::from_utf8_lossy(bytes).lines() {
            if let Some((k, v)) = line.split_once('=')
                && let Ok(v) = v.trim().parse::<f64>()
            {
                s.set(k.trim(), v);
            }
        }
        s
    }

    /// (label, value as shown) for the Statistics screen.
    pub fn lines(&self) -> Vec<(&'static str, String)> {
        FIELDS
            .iter()
            .map(|&(k, label)| {
                let v = self.get(k);
                let shown = match k {
                    "played" => duration(v),
                    "walked" | "swum" | "flown" | "ridden" => distance(v),
                    "damage_dealt" | "damage_taken" => format!("{:.1} hearts", v / 2.0),
                    _ => format!("{}", v as u64),
                };
                (label, shown)
            })
            .collect()
    }
}

fn duration(secs: f64) -> String {
    let s = secs as u64;
    match s {
        0..60 => format!("{s} s"),
        60..3600 => format!("{} min", s / 60),
        _ => format!("{} h {} min", s / 3600, s / 60 % 60),
    }
}

fn distance(m: f64) -> String {
    if m < 1000.0 { format!("{m:.0} m") } else { format!("{:.2} km", m / 1000.0) }
}

impl Game {
    /// Count this frame's travel: how far the player moved, and how.
    pub fn track_travel(&mut self, before: Vec3, dt: f32) {
        if self.menu || self.dedicated || self.dead.is_some() {
            return;
        }
        self.stats.played += dt as f64;
        let moved = self.player.body.pos - before;
        let flat = Vec3::new(moved.x, 0.0, moved.z).length() as f64;
        // Teleports and respawns aren't journeys.
        if flat > 20.0 * dt.max(0.001) as f64 + 1.0 {
            return;
        }
        if self.riding.is_some() || self.mounted.is_some() {
            self.stats.ridden += flat;
        } else if self.player.flying || self.player.gliding {
            self.stats.flown += moved.length() as f64;
        } else if self.player.body.in_water {
            self.stats.swum += moved.length() as f64;
        } else {
            self.stats.walked += flat;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_round_trip_and_read_nicely() {
        let s = Stats { mined: 1234, walked: 2500.4, played: 3725.0, damage_taken: 7.0, ..Default::default() };
        let back = Stats::decode(&s.encode());
        assert_eq!(back, s);
        let lines = back.lines();
        assert!(lines.contains(&("Blocks mined", "1234".to_string())));
        assert!(lines.contains(&("Distance walked", "2.50 km".to_string())));
        assert!(lines.contains(&("Time played", "1 h 2 min".to_string())));
        assert!(lines.contains(&("Damage taken", "3.5 hearts".to_string())));
        // Junk and unknown keys don't hurt.
        assert_eq!(Stats::decode(b"mined=nope\nwho=3\nkills=2"), Stats { kills: 2, ..Default::default() });
    }

    #[test]
    fn walking_counts_but_teleporting_doesnt() {
        let mut g = Game::new(3, false, false);
        let start = g.player.body.pos;
        g.player.body.pos += Vec3::X * 0.1;
        g.track_travel(start, 1.0 / 60.0);
        assert!((g.stats.walked - 0.1).abs() < 1e-3);
        let here = g.player.body.pos;
        g.player.body.pos += Vec3::X * 500.0;
        g.track_travel(here, 1.0 / 60.0);
        assert!(g.stats.walked < 1.0);
    }
}
