//! The moon's phases: full, waning, new and waxing again over eight nights.
//!
//! The world counts its days (`Game::day`, kept with the world and sent to
//! joined players with the time of day). The moon is drawn in its phase, a
//! full moon brings out more monsters, and eggs (Sniffer eggs, turtle eggs)
//! hatch faster under it, as they do in the game this parodies.

use crate::game::Game;

/// How many phases, and which is which.
pub const PHASES: u32 = 8;
pub const NAMES: [&str; 8] = ["Full Moon", "Waning Gibbous", "Last Quarter", "Waning Crescent", "New Moon", "Waxing Crescent", "First Quarter", "Waxing Gibbous"];

/// How lit the moon's face is in phase `p` (1 full, 0 new).
pub fn brightness(p: u32) -> f32 {
    let a = (p % PHASES) as f32 / PHASES as f32 * std::f32::consts::TAU;
    (1.0 + a.cos()) * 0.5
}

/// Monsters at once, as a share of the usual: more at full moon, fewer at new.
pub fn monster_scale(p: u32) -> f32 {
    0.75 + 0.5 * brightness(p)
}

/// How much more often eggs hatch.
pub fn hatch_scale(p: u32) -> f32 {
    0.5 + brightness(p)
}

impl Game {
    /// Tonight's phase.
    pub fn moon_phase(&self) -> u32 {
        self.day % PHASES
    }

    /// The clock moved from `before` to `self.time`: past midnight-into-morning
    /// (time wraps to 0 at sunrise), it's a new day.
    pub fn count_days(&mut self, before: f32) {
        if self.time < before {
            self.day = self.day.wrapping_add(1);
        }
    }

    /// The time of day for joined players, with the day count in front
    /// (`day + fraction`, so older games still read the time).
    pub fn time_msg(&self) -> crate::net::Msg {
        crate::net::Msg::Time((self.day % 4096) as f32 + self.time)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_moon_goes_round_and_days_are_counted() {
        assert_eq!(brightness(0), 1.0);
        assert!(brightness(4) < 0.001);
        assert!(monster_scale(0) > monster_scale(4));
        assert!(hatch_scale(0) > 1.0 && hatch_scale(4) < 1.0);
        let mut g = crate::game::Game::new(1, false, false);
        g.time = 0.99;
        let before = g.time;
        g.time = 0.01;
        g.count_days(before);
        assert_eq!(g.day, 1);
        assert_eq!(g.moon_phase(), 1);
        let crate::net::Msg::Time(t) = g.time_msg() else { panic!() };
        assert!((t - 1.01).abs() < 1e-4);
    }
}
