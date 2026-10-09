//! Game modes: survival, creative, and spectator (fly through everything,
//! touch nothing, unseen by players and mobs). Hardcore worlds give one life:
//! after dying you can only watch.
//!
//! A world has a default mode (what new players start in); each player can be
//! switched with `/gamemode`. Spectators also count as creative internally, so
//! nothing hurts them and nothing is used up.

use crate::game::Game;
use crate::net::Msg;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameMode {
    Survival,
    Creative,
    Spectator,
}

impl GameMode {
    pub fn index(self) -> u8 {
        self as u8
    }

    pub fn from_index(i: u8) -> GameMode {
        match i {
            1 => GameMode::Creative,
            2 => GameMode::Spectator,
            _ => GameMode::Survival,
        }
    }

    /// "survival", "s", "0", "creative", "c", "1", "spectator", "sp", "3"...
    pub fn from_name(s: &str) -> Option<GameMode> {
        match s.trim().to_ascii_lowercase().as_str() {
            "survival" | "s" | "0" => Some(GameMode::Survival),
            "creative" | "c" | "1" => Some(GameMode::Creative),
            "spectator" | "sp" | "spec" | "3" => Some(GameMode::Spectator),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            GameMode::Survival => "Survival",
            GameMode::Creative => "Creative",
            GameMode::Spectator => "Spectator",
        }
    }
}

impl Game {
    /// The local player's mode.
    pub fn mode(&self) -> GameMode {
        if self.spectator {
            GameMode::Spectator
        } else if self.creative {
            GameMode::Creative
        } else {
            GameMode::Survival
        }
    }

    /// Switch the local player's mode.
    pub fn set_mode(&mut self, mode: GameMode) {
        let was = self.spectator;
        self.spectator = mode == GameMode::Spectator;
        self.creative = mode != GameMode::Survival;
        self.player.ghost = self.spectator;
        if self.spectator {
            self.player.flying = true;
            self.breaking = None;
            self.open = None;
        } else if was {
            // Out of the walls: land somewhere you can stand.
            self.player.flying = mode == GameMode::Creative;
            let p = self.player.body.pos;
            let (x, z) = (p.x.floor() as i32, p.z.floor() as i32);
            let blocked = |y: i32| crate::block::is_solid(self.world.get(x, y, z)) || crate::block::is_solid(self.world.get(x, y + 1, z));
            let mut y = p.y.floor() as i32;
            while blocked(y) && y < crate::world::CH - 2 {
                y += 1;
            }
            self.player.body.pos.y = self.player.body.pos.y.max(y as f32);
            self.player.body.vel = macroquad::math::Vec3::ZERO;
        } else if mode == GameMode::Survival {
            self.player.flying = false;
        }
        self.player.fall_start = self.player.body.pos.y;
    }

    /// A joined player's mode, as the host knows it.
    pub fn peer_mode(&self, id: u32) -> GameMode {
        self.peer_ref(id).map(|p| p.mode).unwrap_or(GameMode::Survival)
    }

    /// Joined players in creative (or spectating) get things for free.
    pub fn peer_free(&self, id: u32) -> bool {
        self.peer_mode(id) != GameMode::Survival
    }

    /// Set a joined player's mode (host side) and tell them.
    pub fn set_peer_mode(&mut self, id: u32, mode: GameMode) {
        if let Some(p) = self.peer_mut(id) {
            p.mode = mode;
        }
        self.net_send_to(id, Msg::GameMode { mode: mode.index() });
    }

    /// Hardcore: after dying, the only way on is to watch.
    pub fn spectate_after_death(&mut self) {
        self.dead = None;
        self.player.health = crate::player::MAX_HEALTH;
        self.set_mode(GameMode::Spectator);
        self.msg("Game over. You can still look around.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_parse_and_round_trip() {
        for m in [GameMode::Survival, GameMode::Creative, GameMode::Spectator] {
            assert_eq!(GameMode::from_index(m.index()), m);
            assert_eq!(GameMode::from_name(m.name()), Some(m));
        }
        assert_eq!(GameMode::from_name("SP"), Some(GameMode::Spectator));
        assert_eq!(GameMode::from_name("adventure"), None);
    }

    #[test]
    fn spectators_are_untouchable_and_come_back_down() {
        let mut g = Game::new(3, false, false);
        g.set_mode(GameMode::Spectator);
        assert!(g.creative && g.spectator && g.player.ghost);
        assert_eq!(g.mode(), GameMode::Spectator);
        g.hurt_player(50.0, "tested");
        assert!(g.dead.is_none());
        assert!(g.player_targets().iter().all(|(id, _)| *id != g.my_id), "mobs don't see spectators");
        g.set_mode(GameMode::Survival);
        assert!(!g.creative && !g.player.ghost && !g.player.flying);
    }
}
