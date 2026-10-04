//! **Seasons** (a world setting, off unless you turn it on): the year goes
//! spring, summer, autumn, winter, eight days each (a moon cycle). Leaves and
//! grass turn with the seasons (golden-orange in autumn, pale in winter);
//! crops grow faster in spring and hardly at all in winter; and in winter
//! the rain falls as snow almost everywhere (not in deserts, badlands or the
//! warm, wet places).
//!
//! The season is worked out from the day count, which joined players already
//! get, so everyone agrees on it. The chunk mesher reads the season to tint
//! with from `shown()` (set by the game's window, which re-meshes everything
//! when it changes).

use crate::game::Game;
use crate::world::Biome;
use std::sync::atomic::{AtomicU8, Ordering};

/// Days in each season.
pub const SEASON_DAYS: u32 = 8;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

impl Season {
    pub const ALL: [Season; 4] = [Season::Spring, Season::Summer, Season::Autumn, Season::Winter];

    pub fn of_day(day: u32) -> Season {
        Season::ALL[((day / SEASON_DAYS) % 4) as usize]
    }

    pub fn name(self) -> &'static str {
        match self {
            Season::Spring => "Spring",
            Season::Summer => "Summer",
            Season::Autumn => "Autumn",
            Season::Winter => "Winter",
        }
    }
}

/// How grass and leaves look this season, as a multiplier on the biome's own colour.
pub fn grass_tint(s: Option<Season>) -> [f32; 3] {
    match s {
        None | Some(Season::Summer) => [1.0, 1.0, 1.0],
        Some(Season::Spring) => [0.95, 1.1, 0.9],
        Some(Season::Autumn) => [1.55, 0.92, 0.42],
        Some(Season::Winter) => [1.0, 0.92, 0.95],
    }
}

/// How leaves look this season: they turn further than the grass (a tint
/// can at most double a colour, so autumn's is as far as green leaves go
/// toward orange).
pub fn leaf_tint(s: Option<Season>) -> [f32; 3] {
    match s {
        None | Some(Season::Summer) => [1.0, 1.0, 1.0],
        Some(Season::Spring) => [0.95, 1.12, 0.9],
        Some(Season::Autumn) => [1.99, 0.55, 0.22],
        Some(Season::Winter) => [1.05, 0.85, 0.8],
    }
}

/// How fast crops grow this season.
pub fn growth(s: Option<Season>) -> f32 {
    match s {
        None | Some(Season::Summer) => 1.0,
        Some(Season::Spring) => 1.3,
        Some(Season::Autumn) => 0.8,
        Some(Season::Winter) => 0.35,
    }
}

/// Does rain fall as snow here?
pub fn snows(biome: Biome, s: Option<Season>) -> bool {
    biome == Biome::Snowy || (s == Some(Season::Winter) && !biome.dry() && !matches!(biome, Biome::Jungle | Biome::Mangrove | Biome::Swamp))
}

/// The season the chunk mesher tints with (4: none).
static SHOWN: AtomicU8 = AtomicU8::new(4);

pub fn shown() -> Option<Season> {
    Season::ALL.get(SHOWN.load(Ordering::Relaxed) as usize).copied()
}

pub fn set_shown(s: Option<Season>) {
    SHOWN.store(s.map(|s| s as u8).unwrap_or(4), Ordering::Relaxed);
}

impl Game {
    /// The season, if this world has them (none in the Scorchlands or the Hollow).
    pub fn season(&self) -> Option<Season> {
        (self.rules.seasons && !self.elsewhere()).then(|| Season::of_day(self.day))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_year_turns_and_winter_brings_snow() {
        assert_eq!(Season::of_day(0), Season::Spring);
        assert_eq!(Season::of_day(SEASON_DAYS * 2 + 3), Season::Autumn);
        assert_eq!(Season::of_day(SEASON_DAYS * 7), Season::Winter);
        assert_eq!(Season::of_day(SEASON_DAYS * 4), Season::Spring, "and round again");
        assert!(!snows(Biome::Plains, Some(Season::Summer)) && snows(Biome::Plains, Some(Season::Winter)));
        assert!(!snows(Biome::Desert, Some(Season::Winter)) && snows(Biome::Snowy, None));
        assert!(growth(Some(Season::Spring)) > growth(None) && growth(Some(Season::Winter)) < 0.5);
        let mut g = crate::game::tests::arena(191);
        g.day = SEASON_DAYS * 3;
        assert_eq!(g.season(), None, "off unless the world has seasons");
        g.rules.seasons = true;
        assert_eq!(g.season(), Some(Season::Winter));
        // Autumn leaves are orange: more red, less blue.
        let a = grass_tint(Some(Season::Autumn));
        assert!(a[0] > 1.2 && a[2] < 0.6);
    }
}
