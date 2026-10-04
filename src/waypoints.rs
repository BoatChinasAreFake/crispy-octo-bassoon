//! **Waypoints**: places you name with `/waypoint add <name>` (or `/wp`).
//! Each shows as a coloured label over the spot, with how far off it is,
//! and as a coloured diamond on a held map. `/wp list` says where they
//! all are and `/wp remove <name>` forgets one.
//!
//! They're yours: typed commands are handled on your own game, not sent to
//! the host. They're kept with the world you made them in (a joined player's
//! last until they leave).

use crate::game::Game;
use macroquad::math::Vec3;

/// At most this many waypoints, with names at most `NAME_LEN` long.
pub const MAX_WAYPOINTS: usize = 32;
pub const NAME_LEN: usize = 24;
/// Labels in the world show this far off (blocks).
pub const SHOW_RANGE: f32 = 2000.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Waypoint {
    pub name: String,
    pub pos: Vec3,
}

/// A waypoint's colour (by its place in the list).
pub fn colour(i: usize) -> [u8; 3] {
    const COLOURS: [[u8; 3]; 8] = [[255, 90, 90], [90, 200, 255], [255, 210, 70], [120, 230, 110], [230, 120, 255], [255, 150, 60], [90, 255, 210], [250, 250, 250]];
    COLOURS[i % COLOURS.len()]
}

/// Which world a spot is in (the Scorchlands and the Hollow live far off along x).
pub fn realm(x: f32) -> u8 {
    if crate::hollow::in_hollow(x) {
        2
    } else if crate::scorch::in_scorch(x) {
        1
    } else {
        0
    }
}

pub fn encode(w: &[Waypoint]) -> Vec<u8> {
    let mut out = Vec::new();
    for p in w {
        out.push(p.name.len() as u8);
        out.extend_from_slice(p.name.as_bytes());
        for v in p.pos.to_array() {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    out
}

pub fn decode(mut b: &[u8]) -> Vec<Waypoint> {
    let mut out = Vec::new();
    while let Some((&n, rest)) = b.split_first() {
        let n = n as usize;
        if rest.len() < n + 12 {
            break;
        }
        let name = String::from_utf8_lossy(&rest[..n]).into_owned();
        let f = |o: usize| f32::from_le_bytes(rest[n + o..n + o + 4].try_into().unwrap());
        let pos = Vec3::new(f(0), f(4), f(8));
        if pos.is_finite() && !name.is_empty() && out.len() < MAX_WAYPOINTS {
            out.push(Waypoint { name, pos });
        }
        b = &rest[n + 12..];
    }
    out
}

/// Is `text` a waypoint command (handled here, not sent anywhere)?
pub fn is_command(text: &str) -> bool {
    matches!(text.split_whitespace().next(), Some("/waypoint" | "/waypoints" | "/wp"))
}

impl Game {
    /// `/waypoint add|remove|list [name]`: what to say back.
    pub fn waypoint_command(&mut self, text: &str) -> Vec<String> {
        let mut words = text.split_whitespace().skip(1);
        let verb = words.next().unwrap_or("").to_ascii_lowercase();
        let name: String = words.collect::<Vec<_>>().join(" ").chars().take(NAME_LEN).collect();
        let me = self.player.body.pos;
        let find = |w: &[Waypoint], n: &str| w.iter().position(|p| p.name.eq_ignore_ascii_case(n));
        match verb.as_str() {
            "add" | "set" if !name.is_empty() => {
                let at = Vec3::new(me.x.floor() + 0.5, me.y.floor(), me.z.floor() + 0.5);
                if let Some(i) = find(&self.waypoints, &name) {
                    self.waypoints[i].pos = at;
                    return vec![format!("Moved waypoint \"{name}\" here.")];
                }
                if self.waypoints.len() >= MAX_WAYPOINTS {
                    return vec![format!("That's {MAX_WAYPOINTS} waypoints already. Remove one first (/wp remove <name>).")];
                }
                self.waypoints.push(Waypoint { name: name.clone(), pos: at });
                vec![format!("Waypoint \"{name}\" set at {:.0} {:.0} {:.0}.", at.x.floor(), at.y, at.z.floor())]
            }
            "remove" | "delete" | "del" | "rm" if !name.is_empty() => match find(&self.waypoints, &name) {
                Some(i) => {
                    let w = self.waypoints.remove(i);
                    vec![format!("Forgot waypoint \"{}\".", w.name)]
                }
                None => vec![format!("No waypoint called \"{name}\". /wp list shows them.")],
            },
            "list" | "" if !self.waypoints.is_empty() || verb == "list" => {
                if self.waypoints.is_empty() {
                    return vec!["No waypoints yet. /wp add <name> marks where you're standing.".into()];
                }
                let mut out = vec![format!("{} waypoint{}:", self.waypoints.len(), if self.waypoints.len() == 1 { "" } else { "s" })];
                for w in &self.waypoints {
                    let d = w.pos - me;
                    let far = if realm(w.pos.x) != realm(me.x) {
                        "in another world".to_string()
                    } else {
                        format!("{:.0} blocks {}", Vec3::new(d.x, 0.0, d.z).length(), crate::archaeology::compass_word(d))
                    };
                    out.push(format!("  {}: {:.0} {:.0} {:.0} ({far})", w.name, w.pos.x.floor(), w.pos.y, w.pos.z.floor()));
                }
                out
            }
            _ => vec!["Waypoints: /wp add <name>, /wp remove <name>, /wp list.".into()],
        }
    }

    /// The waypoints in the world you're in, with their colours.
    pub fn waypoints_here(&self) -> impl Iterator<Item = (&Waypoint, [u8; 3])> {
        let here = realm(self.player.body.pos.x);
        self.waypoints.iter().enumerate().filter(move |(_, w)| realm(w.pos.x) == here).map(|(i, w)| (w, colour(i)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waypoints_are_added_listed_removed_and_saved() {
        let mut g = crate::game::tests::arena(234);
        g.player.body.pos = Vec3::new(10.3, 50.0, -4.7);
        assert!(is_command("/wp add Home") && is_command("/waypoint list") && !is_command("/w hi"));
        assert!(g.waypoint_command("/wp add Home")[0].contains("Home"));
        assert_eq!(g.waypoints[0].pos, Vec3::new(10.5, 50.0, -4.5));
        g.player.body.pos = Vec3::new(100.0, 60.0, 0.0);
        g.waypoint_command("/waypoint add The Mine");
        assert!(g.waypoint_command("/wp add home")[0].starts_with("Moved"), "names ignore case");
        assert_eq!(g.waypoints.len(), 2);
        let list = g.waypoint_command("/wp list");
        assert_eq!(list.len(), 3);
        assert!(list[2].contains("The Mine"));
        assert_eq!(decode(&encode(&g.waypoints)), g.waypoints);
        assert!(g.waypoint_command("/wp remove the mine")[0].contains("Forgot"));
        assert_eq!(g.waypoints.len(), 1);
        assert!(g.waypoint_command("/wp remove nowhere")[0].contains("No waypoint"));
        assert_eq!(g.waypoints_here().count(), 1);
        // Kept with the world.
        let back = Game::from_save(g.to_save());
        assert_eq!(back.waypoints, g.waypoints);
    }
}
