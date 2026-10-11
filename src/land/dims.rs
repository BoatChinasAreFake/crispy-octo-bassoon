//! **Dimensions.** The ordinary world, the Scorchlands and the Hollow are
//! separate worlds, each with its own coordinates (all centred near 0, so
//! nothing is ever drawn or moved tens of thousands of blocks from the
//! origin, where 32-bit floats get coarse).
//!
//! Each dimension's terrain is generated from the same seed as always: the
//! generator samples it at "generator coordinates", which are the dimension's
//! own plus a fixed offset (`gen_x`). That offset is where the dimension used
//! to sit when all three shared one map, so worlds made before they were
//! separated come out block for block the same (see `migrate` in realms.rs).
//!
//! Mods can add dimensions of their own (`Dim::Mod`, see modgen.rs): they're
//! numbered in order of their names, so a save finds them again whichever
//! order the mods load in, and their region folders and save sections go
//! by name.

/// Which world something is in.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, PartialOrd, Ord)]
pub enum Dim {
    #[default]
    Over,
    Scorch,
    Hollow,
    /// A mod's dimension: an index into `Registry::dims`.
    Mod(u8),
}

impl Dim {
    pub const ALL: [Dim; 3] = [Dim::Over, Dim::Scorch, Dim::Hollow];

    pub fn index(self) -> u8 {
        match self {
            Dim::Over => 0,
            Dim::Scorch => 1,
            Dim::Hollow => 2,
            Dim::Mod(k) => 3 + k,
        }
    }

    pub fn from_index(i: u8) -> Option<Dim> {
        match i {
            0..=2 => Dim::ALL.get(i as usize).copied(),
            k if ((k - 3) as usize) < crate::block::reg().dims.len() => Some(Dim::Mod(k - 3)),
            _ => None,
        }
    }

    /// Every dimension there is, the mods' too.
    pub fn every() -> Vec<Dim> {
        let mut v = Dim::ALL.to_vec();
        v.extend((0..crate::block::reg().dims.len().min(200)).map(|k| Dim::Mod(k as u8)));
        v
    }

    /// Its name in save sections and region folders: stable for mods' dimensions.
    pub fn save_name(self) -> String {
        match self {
            Dim::Mod(k) => {
                let key = crate::block::reg().dims.get(k as usize).map(|d| d.key.clone()).unwrap_or_default();
                format!("mod_{}", key.replace(':', "_"))
            }
            d => format!("{}", d.index()),
        }
    }

    /// A dimension from its `save_name` (a mod's only while that mod is on).
    pub fn from_save_name(s: &str) -> Option<Dim> {
        if let Some(key) = s.strip_prefix("mod_") {
            return crate::block::reg().dims.iter().position(|d| d.key.replace(':', "_") == key).map(|k| Dim::Mod(k as u8));
        }
        s.parse::<u8>().ok().filter(|&i| i < 3).and_then(Dim::from_index)
    }

    /// Generator coordinates are this many blocks east of the dimension's own
    /// (a whole number of chunks).
    pub const fn gen_x(self) -> i32 {
        match self {
            Dim::Over => 0,
            Dim::Scorch => crate::scorch::SCORCH_ORIGIN,
            Dim::Hollow => crate::hollow::GEN_ORIGIN.x,
            Dim::Mod(_) => 0,
        }
    }

    /// The same, in chunks.
    pub const fn gen_cx(self) -> i32 {
        self.gen_x() / crate::world::CW
    }

    pub fn name(self) -> &'static str {
        match self {
            Dim::Over => "the Overworld",
            Dim::Scorch => "the Scorchlands",
            Dim::Hollow => "the Hollow",
            Dim::Mod(k) => crate::block::reg().dims.get(k as usize).map(|d| d.name).unwrap_or("somewhere that's gone"),
        }
    }

    /// Where a block was in the old one-map layout, when a save from before
    /// the dimensions were separated is loaded: which dimension, and how far
    /// to shift it (subtract from x).
    pub fn of_old_x(x: i32) -> Dim {
        if x >= crate::scorch::SCORCH_X - 16 {
            Dim::Scorch
        } else if x <= crate::hollow::HOLLOW_X + 16 {
            Dim::Hollow
        } else {
            Dim::Over
        }
    }

    /// Has weather, a sun and a moon, seasons.
    pub fn open_sky(self) -> bool {
        self == Dim::Over
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimensions_round_trip_and_sit_on_whole_chunks() {
        for d in Dim::ALL {
            assert_eq!(Dim::from_index(d.index()), Some(d));
            assert_eq!(d.gen_x() % crate::world::CW, 0, "{d:?}");
        }
        assert_eq!(Dim::of_old_x(crate::scorch::SCORCH_ORIGIN), Dim::Scorch);
        assert_eq!(Dim::of_old_x(crate::hollow::GEN_ORIGIN.x), Dim::Hollow);
        assert_eq!(Dim::of_old_x(1000), Dim::Over);
    }
}
