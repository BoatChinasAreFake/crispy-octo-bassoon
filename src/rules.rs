//! World rules, set when a world is created and changed later from the pause
//! menu's World Settings: keep inventory, difficulty and the daylight cycle.
//! The host's rules apply to everyone in its world.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Difficulty {
    Peaceful,
    Easy,
    Normal,
    Hard,
}

impl Difficulty {
    pub const ALL: [Difficulty; 4] = [Difficulty::Peaceful, Difficulty::Easy, Difficulty::Normal, Difficulty::Hard];

    pub fn index(self) -> u8 {
        self as u8
    }

    pub fn from_index(i: u8) -> Difficulty {
        Difficulty::ALL.get(i as usize).copied().unwrap_or(Difficulty::Normal)
    }

    pub fn name(self) -> &'static str {
        match self {
            Difficulty::Peaceful => "Peaceful (Nobody Hisses)",
            Difficulty::Easy => "Easy (Gentle Hissing)",
            Difficulty::Normal => "Normal (Standard Hissing)",
            Difficulty::Hard => "Hard (Aggressive Hissing)",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Difficulty::Peaceful => "No monsters, no hunger. Health comes back on its own.",
            Difficulty::Easy => "Monsters hit softer. Starving stops at five hearts.",
            Difficulty::Normal => "As intended. Starving stops at half a heart.",
            Difficulty::Hard => "Monsters hit harder, and starving can kill you.",
        }
    }

    /// Damage from monsters (bites, arrows, blasts) at this difficulty (Minecraft's formula).
    pub fn mob_damage(self, base: f32) -> f32 {
        match self {
            Difficulty::Peaceful => 0.0,
            Difficulty::Easy => (base / 2.0 + 1.0).min(base),
            Difficulty::Normal => base,
            Difficulty::Hard => base * 1.5,
        }
    }

    /// Starving never takes you below this much health.
    pub fn starve_floor(self) -> f32 {
        match self {
            Difficulty::Peaceful | Difficulty::Easy => 10.0,
            Difficulty::Normal => 1.0,
            Difficulty::Hard => 0.0,
        }
    }

    pub fn monsters(self) -> bool {
        self != Difficulty::Peaceful
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct WorldRules {
    /// Keep your things when you die (otherwise they fall on the ground).
    pub keep_inventory: bool,
    pub difficulty: Difficulty,
    /// Off: the sun stays where it is.
    pub daylight_cycle: bool,
}

impl Default for WorldRules {
    fn default() -> Self {
        WorldRules { keep_inventory: false, difficulty: Difficulty::Normal, daylight_cycle: true }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn difficulty_scales_things() {
        assert_eq!(Difficulty::Peaceful.mob_damage(6.0), 0.0);
        assert_eq!(Difficulty::Easy.mob_damage(6.0), 4.0);
        assert_eq!(Difficulty::Easy.mob_damage(1.0), 1.0, "easy never hits harder");
        assert_eq!(Difficulty::Hard.mob_damage(6.0), 9.0);
        for d in Difficulty::ALL {
            assert_eq!(Difficulty::from_index(d.index()), d);
        }
        assert_eq!(Difficulty::from_index(99), Difficulty::Normal);
        assert!(!Difficulty::Peaceful.monsters());
        assert_eq!(Difficulty::Hard.starve_floor(), 0.0);
    }
}
