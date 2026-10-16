//! Landmarks: rare one-off natural wonders, each with a name of its own.
//!
//! - **Stone arches.** A great span of rock standing over open ground, sandstone
//!   in the desert, banded terracotta in the badlands.
//! - **Crater lakes.** A round, deep lake in a bowl, ringed by a raised rim of rock.
//! - **Petrified forests.** A grove of trees turned to stone, standing and fallen,
//!   on bare dry ground. Their **Petrified Logs** can be mined and built with.
//!
//! There's at most one in every 1,536 blocks square, and only in worlds from
//! generator 8 on (see `GenOptions`). Each is worked out from the seed alone,
//! column by column, so every chunk (and every player's game) builds the same
//! one. Maps and the Explorer's Atlas show them by name.

use crate::block::*;
use crate::noise::{hash2, hash3};
use crate::world::{idx, Biome, Generator, CH, CW};

/// One landmark per cell this size, at most.
pub const GRID: i32 = 1536;
/// How far a landmark reaches from its middle (it fits in this square).
const REACH: i32 = 44;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Arch,
    CraterLake,
    PetrifiedForest,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Arch, Kind::CraterLake, Kind::PetrifiedForest];

    pub fn label(self) -> &'static str {
        match self {
            Kind::Arch => "stone arch",
            Kind::CraterLake => "crater lake",
            Kind::PetrifiedForest => "petrified forest",
        }
    }

    /// Where each kind turns up.
    fn suits(self, biome: Biome) -> bool {
        use Biome::*;
        match self {
            Kind::Arch => matches!(biome, Desert | Badlands | Savanna | Plains | Meadow | StonyPeaks | Taiga | Snowy),
            Kind::CraterLake => matches!(biome, Plains | Forest | BirchForest | Meadow | Taiga | Snowy | Savanna | DarkForest),
            Kind::PetrifiedForest => matches!(biome, Desert | Badlands | Savanna | Plains),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Landmark {
    pub kind: Kind,
    pub x: i32,
    pub z: i32,
    /// The ground at its middle.
    pub y: i32,
    pub seed: u32,
    pub name: String,
}

/// A name for a landmark: "The Grey Mother", "Lake Harrowmere", "The Ashen Grove".
pub fn name_for(kind: Kind, seed: u32) -> String {
    const OLD: [&str; 16] = ["Harrow", "Wend", "Ald", "Merrow", "Thorn", "Cold", "Gall", "Brin", "Kesh", "Ober", "Tam", "Ivel", "Sorrow", "Lark", "Dun", "Hollin"];
    const END: [&str; 8] = ["mere", "wick", "ford", "stead", "moor", "holt", "by", "ton"];
    const HUE: [&str; 8] = ["Grey", "Red", "Old", "Pale", "Weeping", "Sleeping", "Long", "Crooked"];
    const FIGURE: [&str; 8] = ["Mother", "Bridge", "Gate", "Giant", "Window", "Elbow", "Saddle", "Yawn"];
    const GROVE: [&str; 6] = ["Ashen Grove", "Stone Wood", "Quiet Grove", "Grey Timber", "Still Forest", "Fossil Wood"];
    let pick = |n: usize, k: i32| ((hash2(seed, k, 0x4E4D) * n as f32) as usize).min(n - 1);
    let place = format!("{}{}", OLD[pick(OLD.len(), 1)], END[pick(END.len(), 2)]);
    match kind {
        Kind::Arch => format!("The {} {}", HUE[pick(HUE.len(), 3)], FIGURE[pick(FIGURE.len(), 4)]),
        Kind::CraterLake => format!("Lake {place}"),
        Kind::PetrifiedForest => format!("The {} of {place}", GROVE[pick(GROVE.len(), 5)]),
    }
}

impl Generator {
    /// The landmark in grid cell (gx, gz), if there is one.
    pub fn landmark_in_cell(&self, gx: i32, gz: i32) -> Option<Landmark> {
        if self.opts.version < 8 || self.dim != crate::dims::Dim::Over {
            return None;
        }
        let s = self.seed ^ 0x1A4D_3A4C;
        if hash2(s, gx, gz) > 0.55 {
            return None;
        }
        // Somewhere well inside the cell, so neighbours never crowd each other.
        let margin = 200;
        let span = (GRID - 2 * margin) as f32;
        let (x, z) = (gx * GRID + margin + (hash2(s ^ 1, gx, gz) * span) as i32, gz * GRID + margin + (hash2(s ^ 2, gx, gz) * span) as i32);
        let (h, biome) = self.column(x, z);
        if h <= self.sea() + 2 || biome.is_ocean() || matches!(biome, Biome::River | Biome::Beach | Biome::Swamp | Biome::Mangrove) {
            return None;
        }
        // Level enough, and on dry land all round.
        let ring = [(-24, 0), (24, 0), (0, -24), (0, 24), (-17, -17), (17, 17), (-17, 17), (17, -17)];
        let around: Vec<(i32, Biome)> = ring.iter().map(|&(dx, dz)| self.column(x + dx, z + dz)).collect();
        if around.iter().any(|&(hh, b)| b.is_ocean() || b == Biome::River || (hh - h).abs() > 10) {
            return None;
        }
        let seed = (hash2(s ^ 3, gx, gz) * u32::MAX as f32) as u32;
        let first = (seed % 3) as usize;
        let kind = (0..3).map(|k| Kind::ALL[(first + k) % 3]).find(|k| k.suits(biome))?;
        Some(Landmark { kind, x, z, y: h, seed, name: name_for(kind, seed) })
    }

    /// Every landmark within `r` blocks (by cell) of (x, z), nearest first.
    pub fn landmarks_near(&self, x: i32, z: i32, r: i32) -> Vec<Landmark> {
        let cells = r / GRID + 1;
        let (gx, gz) = (x.div_euclid(GRID), z.div_euclid(GRID));
        let mut v: Vec<Landmark> = (-cells..=cells)
            .flat_map(|dz| (-cells..=cells).map(move |dx| (gx + dx, gz + dz)))
            .filter_map(|(cx, cz)| self.landmark_in_cell(cx, cz))
            .filter(|l| (l.x - x).abs().max((l.z - z).abs()) <= r)
            .collect();
        v.sort_by_key(|l| (l.x - x).pow(2) + (l.z - z).pow(2));
        v
    }

    /// The landmark whose ground (x, z) is on, if any.
    pub fn landmark_at(&self, x: i32, z: i32) -> Option<Landmark> {
        let l = self.landmark_in_cell(x.div_euclid(GRID), z.div_euclid(GRID))?;
        let d = (((x - l.x).pow(2) + (z - l.z).pow(2)) as f32).sqrt();
        (d <= landmark_radius(&l) as f32).then_some(l)
    }

    /// Build the part of any landmark that falls in chunk (cx, cz).
    pub fn place_landmarks(&self, cx: i32, cz: i32, b: &mut [Id]) {
        let (x0, z0) = (cx * CW, cz * CW);
        let (gx, gz) = (x0.div_euclid(GRID), z0.div_euclid(GRID));
        for dz in -1..=1 {
            for dx in -1..=1 {
                let Some(l) = self.landmark_in_cell(gx + dx, gz + dz) else { continue };
                if (l.x - (x0 + 8)).abs() > REACH + 8 || (l.z - (z0 + 8)).abs() > REACH + 8 {
                    continue;
                }
                for lz in 0..CW {
                    for lx in 0..CW {
                        let (x, z) = (x0 + lx, z0 + lz);
                        match l.kind {
                            Kind::Arch => self.arch_column(&l, x, z, lx, lz, b),
                            Kind::CraterLake => self.crater_column(&l, x, z, lx, lz, b),
                            Kind::PetrifiedForest => self.grove_column(&l, x, z, lx, lz, b),
                        }
                    }
                }
            }
        }
    }

    fn arch_column(&self, l: &Landmark, x: i32, z: i32, lx: i32, lz: i32, b: &mut [Id]) {
        let s = l.seed;
        let along_x = s.is_multiple_of(2);
        let (u, v) = if along_x { (x - l.x, z - l.z) } else { (z - l.z, x - l.x) };
        let span = 11.0 + (s >> 3) as f32 % 6.0;
        let thick = 3.5 + (s >> 7) as f32 % 2.0;
        let half_width = 2.5 + (hash2(s, u, 0) * 1.5);
        if v.abs() as f32 > half_width + 1.0 || u.abs() as f32 > span + thick + 1.0 {
            return;
        }
        let biome = self.column(l.x, l.z).1;
        let rock = |y: i32| match biome {
            Biome::Desert => SANDSTONE,
            Biome::Badlands => TERRACOTTA + [0, 1, 0, 2, 3, 0, 1, 2][(y.rem_euclid(24) / 3) as usize] as Id,
            _ => {
                if hash3(s, x, y, z) < 0.2 { COBBLE } else { STONE }
            }
        };
        // Taper the sides of the span so it isn't a slab.
        let width_here = half_width - (v.abs() as f32 - half_width).max(0.0);
        if v.abs() as f32 > width_here + 0.5 {
            return;
        }
        let ground = self.column(x, z).0;
        let base = l.y;
        // Nothing grows in the opening.
        if (u.abs() as f32) < span {
            for y in ground + 1..CH {
                let id = b[idx(lx, y, lz)];
                if is_leaves(id) || is_log(id) {
                    b[idx(lx, y, lz)] = AIR;
                }
            }
        }
        for y in (ground - 4).max(1)..CH.min(base + span as i32 + thick as i32 + 3) {
            let up = (y - base) as f32;
            let r = ((u * u) as f32 + up * up).sqrt();
            let ring = up >= -1.0 && r >= span && r <= span + thick + hash3(s ^ 5, x, y, z) * 0.8;
            // The legs carry on down into the ground.
            let leg = up < 0.0 && (u.abs() as f32) >= span - 0.5 && (u.abs() as f32) <= span + thick;
            if ring || leg {
                b[idx(lx, y, lz)] = rock(y);
            }
        }
    }

    fn crater_column(&self, l: &Landmark, x: i32, z: i32, lx: i32, lz: i32, b: &mut [Id]) {
        let s = l.seed;
        let r = 18.0 + (s >> 4) as f32 % 9.0;
        let wobble = (hash2(s, (x - l.x) / 4, (z - l.z) / 4) - 0.5) * 3.0;
        let d = (((x - l.x).pow(2) + (z - l.z).pow(2)) as f32).sqrt() + wobble;
        let rim_w = 6.0;
        if d > r + rim_w {
            return;
        }
        let level = l.y;
        let ground = self.column(x, z).0;
        let cold = matches!(self.column(l.x, l.z).1, Biome::Snowy | Biome::Taiga);
        if d < r {
            // The bowl: water down to a deep round floor of gravel and clay.
            let depth = (4.0 + 10.0 * (1.0 - (d / r).powi(2))) as i32;
            let floor = level - depth;
            for y in floor + 1..CH {
                b[idx(lx, y, lz)] = if y < level { if cold && y == level - 1 && d < r - 2.0 { ICE } else { WATER } } else { AIR };
            }
            for y in (floor - 2).max(1)..=floor {
                b[idx(lx, y, lz)] = if hash3(s, x, y, z) < 0.3 { SAND } else { GRAVEL };
            }
            // (Shore stone where the bowl rises toward the rim.)
            if d > r - 2.5 {
                b[idx(lx, floor, lz)] = STONE;
            }
        } else {
            // The rim: a raised ring of rock, falling away outside.
            let k = 1.0 - (d - r) / rim_w;
            let top = (level + 1 + (k * 4.0 + hash2(s ^ 9, x, z) * 1.5) as i32).max(ground);
            for y in (ground - 3).max(1)..=top {
                b[idx(lx, y, lz)] = if y == top && k < 0.4 { GRASS } else if hash3(s ^ 3, x, y, z) < 0.25 { GRAVEL } else { STONE };
            }
            for y in top + 1..CH {
                if is_leaves(b[idx(lx, y, lz)]) || is_log(b[idx(lx, y, lz)]) || block(b[idx(lx, y, lz)]).model == crate::block::Model::Cross {
                    b[idx(lx, y, lz)] = AIR;
                }
            }
        }
    }

    fn grove_column(&self, l: &Landmark, x: i32, z: i32, lx: i32, lz: i32, b: &mut [Id]) {
        let s = l.seed;
        let r = 30.0 + (s >> 5) as f32 % 10.0;
        let d = (((x - l.x).pow(2) + (z - l.z).pow(2)) as f32).sqrt();
        if d > r + 2.0 {
            return;
        }
        let ground = self.column(x, z).0;
        // Clear what grows here now, and leave bare, dry ground.
        for y in ground + 1..CH {
            let id = b[idx(lx, y, lz)];
            if is_leaves(id) || is_log(id) || block(id).model == crate::block::Model::Cross {
                b[idx(lx, y, lz)] = AIR;
            }
        }
        let edge = d > r - 4.0 && hash2(s ^ 7, x, z) < (d - (r - 4.0)) / 6.0;
        if !edge && ground >= 1 {
            let v = hash2(s ^ 11, x, z);
            if v < 0.35 {
                b[idx(lx, ground, lz)] = GRAVEL;
            } else if v < 0.55 {
                b[idx(lx, ground, lz)] = RED_SAND;
            } else if v < 0.6 && ground + 1 < CH {
                b[idx(lx, ground, lz)] = DIRT;
                b[idx(lx, ground + 1, lz)] = DEAD_BUSH;
            }
        }
        // Standing trunks on a loose grid, and a few fallen ones.
        const STEP: i32 = 7;
        let (gx, gz) = ((x - l.x).div_euclid(STEP), (z - l.z).div_euclid(STEP));
        for (tx, tz) in [(gx, gz), (gx - 1, gz), (gx, gz - 1)] {
            let (ax, az) = (l.x + tx * STEP + (hash2(s ^ 13, tx, tz) * 4.0) as i32, l.z + tz * STEP + (hash2(s ^ 17, tx, tz) * 4.0) as i32);
            let ad = (((ax - l.x).pow(2) + (az - l.z).pow(2)) as f32).sqrt();
            if ad > r - 3.0 || hash2(s ^ 19, tx, tz) > 0.6 {
                continue;
            }
            let base = self.column(ax, az).0;
            if hash2(s ^ 23, tx, tz) < 0.3 {
                // Fallen: lying along x or z.
                let len = 4 + (hash2(s ^ 29, tx, tz) * 4.0) as i32;
                let on_x = hash2(s ^ 31, tx, tz) < 0.5;
                let hit = if on_x { z == az && (ax..ax + len).contains(&x) } else { x == ax && (az..az + len).contains(&z) };
                if hit && base + 1 < CH {
                    b[idx(lx, base + 1, lz)] = if on_x { PETRIFIED_LOG_X } else { PETRIFIED_LOG_Z };
                }
            } else if x == ax && z == az {
                let tall = 4 + (hash2(s ^ 37, tx, tz) * 6.0) as i32;
                for y in base + 1..(base + 1 + tall).min(CH) {
                    b[idx(lx, y, lz)] = PETRIFIED_LOG;
                }
            }
        }
    }
}

/// How far a landmark's own ground reaches from its middle (for "you're at it").
pub fn landmark_radius(l: &Landmark) -> i32 {
    match l.kind {
        Kind::Arch => 22,
        Kind::CraterLake => 30,
        Kind::PetrifiedForest => 38,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::GenOptions;

    fn some(g: &Generator) -> Vec<Landmark> {
        (-8..8).flat_map(|gz| (-8..8).map(move |gx| (gx, gz))).filter_map(|(gx, gz)| g.landmark_in_cell(gx, gz)).collect()
    }

    #[test]
    fn landmarks_are_rare_varied_and_only_in_new_worlds() {
        let g = Generator::with(424242, GenOptions::DEFAULT);
        let found = some(&g);
        // 256 cells, roughly a quarter of them with one.
        assert!((20..=160).contains(&found.len()), "{} landmarks", found.len());
        for k in Kind::ALL {
            assert!(found.iter().any(|l| l.kind == k), "no {k:?}");
        }
        // Each is on dry land, and the same every time it's asked for.
        for l in &found {
            assert!(l.y > g.sea());
            assert_eq!(g.landmark_in_cell(l.x.div_euclid(GRID), l.z.div_euclid(GRID)).as_ref(), Some(l));
            assert!(!l.name.is_empty());
        }
        let old = Generator::with(424242, GenOptions { version: 7, ..GenOptions::DEFAULT });
        assert!(some(&old).is_empty(), "released worlds keep their land");
    }

    #[test]
    fn names_differ_and_read_well() {
        let names: std::collections::HashSet<String> = (0..200).map(|s| name_for(Kind::CraterLake, s * 7919)).collect();
        assert!(names.len() > 60);
        assert!(name_for(Kind::CraterLake, 5).starts_with("Lake "));
        assert!(name_for(Kind::Arch, 5).starts_with("The "));
    }

    #[test]
    fn each_kind_builds_what_it_should() {
        let _lock = crate::mods::registry_test_lock();
        let g = Generator::with(424242, GenOptions::DEFAULT);
        let found = some(&g);
        for k in Kind::ALL {
            let l = found.iter().find(|l| l.kind == k).unwrap();
            let mut counts = std::collections::HashMap::new();
            for cz in (l.z - REACH).div_euclid(CW)..=(l.z + REACH).div_euclid(CW) {
                for cx in (l.x - REACH).div_euclid(CW)..=(l.x + REACH).div_euclid(CW) {
                    for id in g.generate(cx, cz) {
                        *counts.entry(id).or_insert(0usize) += 1;
                    }
                }
            }
            match k {
                Kind::PetrifiedForest => {
                    assert!(counts.get(&PETRIFIED_LOG).copied().unwrap_or(0) > 20, "{k:?}");
                    assert!(counts.get(&PETRIFIED_LOG_X).copied().unwrap_or(0) + counts.get(&PETRIFIED_LOG_Z).copied().unwrap_or(0) > 4, "fallen logs lie on their side");
                }
                Kind::CraterLake => assert!(counts.get(&WATER).copied().unwrap_or(0) + counts.get(&ICE).copied().unwrap_or(0) > 2000, "{k:?}"),
                Kind::Arch => {
                    // Rock in the air over the middle.
                    let chunk = g.generate(l.x.div_euclid(CW), l.z.div_euclid(CW));
                    let (lx, lz) = (l.x.rem_euclid(CW), l.z.rem_euclid(CW));
                    let solid_above = (l.y + 6..CH).filter(|&y| is_solid(chunk[idx(lx, y, lz)])).count();
                    let open_below = (l.y + 1..l.y + 6).all(|y| !is_solid(chunk[idx(lx, y, lz)]));
                    assert!(solid_above >= 3 && open_below, "an arch over {},{}", l.x, l.z);
                }
            }
        }
    }
}
