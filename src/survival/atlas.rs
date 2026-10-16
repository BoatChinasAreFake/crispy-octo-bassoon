//! The Explorer's Atlas: a book (a Book and a Map) that fills in a map of the
//! biomes as you travel with it, a square of 64 blocks at a time, and lists
//! the landmarks you've stood at. Each biome it's seen gets a note on its rare
//! finds. Hold it to read (see hud.rs).
//!
//! It's yours (your game keeps it, like the Cookbook) and only fills in out in
//! the Overworld, while you carry one.

use crate::block::*;
use crate::game::Game;
use crate::landmarks::Kind;
use crate::world::Biome;
use std::collections::HashMap;

/// Blocks to a square of the Atlas's map.
pub const CELL: i32 = 64;
/// Biomes to have found for the Well Travelled advancement.
pub const WELL_TRAVELLED: usize = 12;

#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub name: String,
    pub kind: Kind,
    pub x: i32,
    pub z: i32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Atlas {
    /// The biome seen in each square (an index into `Biome::ALL`).
    pub cells: HashMap<(i32, i32), u8>,
    /// Landmarks stood at, in the order they were found.
    pub landmarks: Vec<Found>,
}

impl Atlas {
    /// Biomes found so far, in `Biome::ALL` order.
    pub fn biomes(&self) -> Vec<Biome> {
        let mut seen = [false; Biome::ALL.len()];
        for &b in self.cells.values() {
            if let Some(s) = seen.get_mut(b as usize) {
                *s = true;
            }
        }
        Biome::ALL.into_iter().zip(seen).filter(|&(_, s)| s).map(|(b, _)| b).collect()
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend((self.cells.len() as u32).to_le_bytes());
        let mut cells: Vec<_> = self.cells.iter().collect();
        cells.sort();
        for (&(x, z), &b) in cells {
            v.extend(x.to_le_bytes());
            v.extend(z.to_le_bytes());
            v.push(b);
        }
        v.extend((self.landmarks.len() as u16).to_le_bytes());
        for f in &self.landmarks {
            let name = f.name.as_bytes();
            v.push(name.len().min(255) as u8);
            v.extend(&name[..name.len().min(255)]);
            v.push(Kind::ALL.iter().position(|&k| k == f.kind).unwrap_or(0) as u8);
            v.extend(f.x.to_le_bytes());
            v.extend(f.z.to_le_bytes());
        }
        v
    }

    /// (Anything it can't read is left out, never a panic.)
    pub fn decode(b: &[u8]) -> Atlas {
        let mut a = Atlas::default();
        let mut at = 0;
        let mut take = |n: usize| {
            let s = b.get(at..at + n);
            at += n;
            s
        };
        let i32_of = |s: &[u8]| i32::from_le_bytes(s.try_into().unwrap());
        let Some(n) = take(4).map(|s| u32::from_le_bytes(s.try_into().unwrap())) else { return a };
        for _ in 0..n.min(1 << 20) {
            let (Some(x), Some(z), Some(k)) = (take(4), take(4), take(1)) else { return a };
            if (k[0] as usize) < Biome::ALL.len() {
                a.cells.insert((i32_of(x), i32_of(z)), k[0]);
            }
        }
        let Some(n) = take(2).map(|s| u16::from_le_bytes(s.try_into().unwrap())) else { return a };
        for _ in 0..n {
            let Some(len) = take(1) else { return a };
            let (Some(name), Some(k), Some(x), Some(z)) = (take(len[0] as usize), take(1), take(4), take(4)) else { return a };
            let kind = Kind::ALL[(k[0] as usize).min(Kind::ALL.len() - 1)];
            a.landmarks.push(Found { name: String::from_utf8_lossy(name).into_owned(), kind, x: i32_of(x), z: i32_of(z) });
        }
        a
    }
}

/// A biome's colour on the Atlas's map.
pub fn colour(b: Biome) -> [u8; 3] {
    use Biome::*;
    match b {
        Plains => [141, 179, 96],
        Forest => [56, 120, 52],
        Desert => [226, 206, 140],
        Snowy => [236, 242, 246],
        Ocean => [52, 88, 168],
        Swamp => [82, 102, 70],
        Jungle => [34, 140, 40],
        Badlands => [196, 104, 56],
        Taiga => [70, 104, 86],
        Cherry => [240, 170, 200],
        Mangrove => [90, 118, 66],
        PaleGarden => [170, 176, 166],
        Savanna => [190, 182, 92],
        BirchForest => [120, 168, 96],
        DarkForest => [40, 74, 36],
        MushroomIslands => [160, 112, 156],
        IceSpikes => [186, 220, 238],
        Meadow => [160, 204, 110],
        StonyPeaks => [140, 140, 140],
        WarmOcean => [60, 160, 196],
        LukewarmOcean => [56, 120, 184],
        FrozenOcean => [120, 150, 210],
        River => [70, 120, 220],
        Beach => [236, 222, 168],
    }
}

/// What's rare and worth looking for in a biome.
pub fn note(b: Biome) -> &'static str {
    use Biome::*;
    match b {
        Plains => "Gallopers, villages, and the odd beehive.",
        Forest => "Bees in the oaks, and wood for everything.",
        Desert => "Pyramids with a trap in the floor, and desert villages.",
        Snowy => "Igloos with a secret, polar bears, and hot springs up in the hills.",
        Ocean => "Shipwrecks, ocean ruins and buried treasure maps.",
        Swamp => "Witch huts, mud, and lily pads to walk on.",
        Jungle => "Jungle temples, melons, pandas in the bamboo.",
        Badlands => "Gold close to the surface, and mineshafts in the open.",
        Taiga => "Spruce villages, Woofers, and the odd llama.",
        Cherry => "Pink petals, and the bees love it.",
        Mangrove => "Mud, roots, and frogs in the shallows.",
        PaleGarden => "Something in the pale oaks wakes at night. Eyeblossoms.",
        Savanna => "Acacias, llamas and savanna villages.",
        BirchForest => "Tall white birches, quiet and bright.",
        DarkForest => "Woodland mansions full of Pilferers, and huge mushrooms.",
        MushroomIslands => "Mushmooers, and no monsters at all.",
        IceSpikes => "Spikes of packed ice taller than houses.",
        Meadow => "Flowers, bees, and a long view.",
        StonyPeaks => "Bare stone, llamas, and the best view in the world.",
        WarmOcean => "Coral reefs, tropical Fishies, sea pickles.",
        LukewarmOcean => "Seagrass, dolphins, and ruins in the sand.",
        FrozenOcean => "Icebergs, and sea ice thick enough to walk on.",
        River => "Fishies, and a winding way down to the sea.",
        Beach => "Driftwood, shells, turtles, and tide pools.",
    }
}

impl Game {
    pub fn carries_atlas(&self) -> bool {
        self.inv.count(EXPLORERS_ATLAS) > 0 || self.inv.offhand.is_some_and(|s| s.0 == EXPLORERS_ATLAS)
    }

    /// Fill in the square you're in, and any landmark you're at (out in the
    /// Overworld, carrying an Atlas). Called a couple of times a second.
    pub fn atlas_tick(&mut self) {
        if self.away() || self.dead.is_some() || self.world.dim() != crate::dims::Dim::Over || !self.carries_atlas() {
            return;
        }
        let p = self.player.body.pos;
        let (x, z) = (p.x.floor() as i32, p.z.floor() as i32);
        let biome = self.world.generator.column(x, z).1;
        let k = Biome::ALL.iter().position(|&b| b == biome).unwrap_or(0) as u8;
        let before = self.atlas.biomes().len();
        self.atlas.cells.insert((x.div_euclid(CELL), z.div_euclid(CELL)), k);
        let after = self.atlas.biomes().len();
        if after > before {
            self.msg(format!("Atlas: {} added. {}", biome.name(), note(biome)));
        }
        if after >= WELL_TRAVELLED {
            self.advance("well_travelled");
        }
        if let Some(l) = self.world.generator.landmark_at(x, z)
            && !self.atlas.landmarks.iter().any(|f| (f.x, f.z) == (l.x, l.z))
        {
            self.msg(format!("Atlas: {} (a {}) marked.", l.name, l.kind.label()));
            self.atlas.landmarks.push(Found { name: l.name, kind: l.kind, x: l.x, z: l.z });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_atlas_saves_and_shrugs_off_bad_data() {
        let mut a = Atlas::default();
        a.cells.insert((-3, 7), 2);
        a.cells.insert((40, -1), 23);
        a.landmarks.push(Found { name: "Lake Harrowmere".into(), kind: Kind::CraterLake, x: -900, z: 1200 });
        let back = Atlas::decode(&a.encode());
        assert_eq!(back, a);
        assert_eq!(back.biomes(), vec![Biome::Desert, Biome::Beach]);
        let bytes = a.encode();
        for cut in 0..bytes.len() {
            Atlas::decode(&bytes[..cut]);
        }
        assert_eq!(Atlas::decode(&[]), Atlas::default());
    }

    #[test]
    fn every_biome_has_a_colour_and_a_note() {
        let colours: std::collections::HashSet<[u8; 3]> = Biome::ALL.iter().map(|&b| colour(b)).collect();
        assert_eq!(colours.len(), Biome::ALL.len(), "each biome its own colour");
        assert!(Biome::ALL.iter().all(|&b| !note(b).is_empty()));
    }

    #[test]
    fn carrying_an_atlas_fills_it_in_and_marks_landmarks() {
        let mut g = Game::new_with(424242, false, false, crate::world::GenOptions::DEFAULT);
        g.ready = true;
        let l = (-6..6).flat_map(|gz| (-6..6).map(move |gx| (gx, gz))).find_map(|(gx, gz)| g.world.generator.landmark_in_cell(gx, gz)).unwrap();
        g.player.body.pos = macroquad::math::Vec3::new(l.x as f32 + 0.5, l.y as f32 + 1.0, l.z as f32 + 0.5);
        g.atlas_tick();
        assert!(g.atlas.cells.is_empty(), "not without an Atlas");
        g.inv.slots[0] = Some((EXPLORERS_ATLAS, 1));
        g.atlas_tick();
        assert_eq!(g.atlas.cells.len(), 1);
        assert_eq!(g.atlas.landmarks.len(), 1);
        assert_eq!(g.atlas.landmarks[0].name, l.name);
        g.atlas_tick();
        assert_eq!(g.atlas.landmarks.len(), 1, "once each");
        // And it's kept in the save.
        let back = Game::from_save(g.to_save());
        assert_eq!(back.atlas, g.atlas);
    }
}
