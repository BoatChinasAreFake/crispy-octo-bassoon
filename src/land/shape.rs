//! The land's shape, and the biomes on it, in worlds from version 7.
//!
//! Three slow noises shape the land, as in modern Minecraft:
//!
//! - **Continentalness**: deep ocean, ocean, the coast, inland and far inland.
//!   It sets how high the land sits (a spline: the sea floor, rising to the
//!   coast, then gently higher the further inland).
//! - **Erosion**: from rugged (low) to worn flat (high). It sets how much the
//!   peaks and valleys count.
//! - **Peaks and valleys**, folded from a "weirdness" noise: valley floors
//!   where weirdness is near nought, ridges where it's large either way.
//!   Rivers run along the valley floors, down to the sea.
//!
//! The biome then comes from the shape first (the sea, a river, the beach, a
//! peak, wet lowland), then from the temperature bands of the older
//! generator (so neighbours still make sense: snow gives way to taiga before
//! anything milder), moisture within a band, and weirdness for the rarer
//! variants (Cherry Groves, the Pale Garden, Ice Spikes). Swamps and
//! mangroves are wet lowland that was low already; nothing sinks the land
//! afterwards.

use crate::world::smoothstep;
use crate::world::{Biome, Generator};

/// A curve through some points (x rising), straight between them and flat
/// beyond the ends.
pub struct Spline(pub &'static [(f32, f32)]);

impl Spline {
    pub fn at(&self, x: f32) -> f32 {
        let p = self.0;
        if x <= p[0].0 {
            return p[0].1;
        }
        for w in p.windows(2) {
            let ((x0, y0), (x1, y1)) = (w[0], w[1]);
            if x <= x1 {
                return y0 + (y1 - y0) * (x - x0) / (x1 - x0);
            }
        }
        p[p.len() - 1].1
    }
}

/// Continentalness to height above (or below) the sea: the deep sea floor,
/// the shelf, the coast at about nought, then gently up inland.
pub const CONTINENT: Spline = Spline(&[(-1.0, -46.0), (-0.5, -40.0), (-0.32, -24.0), (-0.22, -10.0), (-0.15, -3.0), (-0.11, 0.0), (-0.05, 2.0), (0.1, 5.0), (0.3, 11.0), (0.6, 20.0), (1.0, 26.0)]);
/// Erosion to ruggedness: how much the peaks and valleys count.
pub const RUGGED: Spline = Spline(&[(-1.0, 1.0), (-0.5, 0.95), (-0.25, 0.6), (0.0, 0.32), (0.25, 0.14), (0.5, 0.06), (1.0, 0.04)]);
/// Peaks and valleys to height: valley floors, gentle middles, steep peaks.
pub const PEAKS: Spline = Spline(&[(-1.0, -12.0), (-0.6, -5.0), (-0.2, 0.0), (0.2, 7.0), (0.5, 20.0), (0.8, 40.0), (1.0, 54.0)]);

/// Where the coast is, in continentalness: beaches between these.
pub const COAST: (f32, f32) = (-0.2, -0.06);
/// How close to nought weirdness is along a river's channel, and its valley.
pub const RIVER: f32 = 0.045;
pub const RIVER_VALLEY: f32 = 0.32;

/// Everything the biome rules look at for a column.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    pub h: i32,
    pub sea: i32,
    /// Continentalness, erosion, weirdness and peaks-and-valleys (each about -1 to 1).
    pub c: f32,
    pub e: f32,
    pub w: f32,
    pub pv: f32,
    /// Temperature and moisture (see `Generator::climate`).
    pub t: f32,
    pub m: f32,
    /// In a river's channel.
    pub river: bool,
    /// How much this is wet lowland (0 to 1): low, flat-ish and damp.
    pub wet_low: f32,
}

/// Fold weirdness into peaks and valleys: -1 on the valley floors (weirdness
/// near nought), 1 on the ridges.
pub fn peaks_and_valleys(w: f32) -> f32 {
    1.0 - (3.0 * w.abs() - 2.0).abs()
}

/// The seas, by temperature.
fn sea_by_temperature(t: f32) -> Biome {
    if t > 0.3 {
        Biome::WarmOcean
    } else if t > 0.12 {
        Biome::LukewarmOcean
    } else if t < -0.32 {
        Biome::FrozenOcean
    } else {
        Biome::Ocean
    }
}

/// The biome rules, in order: the shape of the land first, then the
/// temperature band, moisture within it, and weirdness for the rare ones.
pub fn pick(p: &Place) -> Biome {
    let (h, sea, t, m, w) = (p.h, p.sea, p.t, p.m, p.w);
    // The shape: water, the shore, the heights, wet lowland.
    if p.river {
        return Biome::River;
    }
    if h < sea - 1 {
        return sea_by_temperature(t);
    }
    if h > sea + 52 || (p.pv > 0.8 && p.e < -0.35 && h > sea + 44) {
        return if t > -0.05 { Biome::StonyPeaks } else { Biome::Snowy };
    }
    let coast = (COAST.0..COAST.1).contains(&p.c);
    if coast && h <= sea + 2 && p.e > -0.45 && p.wet_low < 0.5 {
        return if t < -0.32 { Biome::Snowy } else { Biome::Beach };
    }
    // Then the temperature bands.
    if t < -0.32 {
        // Frozen: here and there, where it's weird, fields of ice spikes.
        return if w > 0.42 && h < sea + 30 { Biome::IceSpikes } else { Biome::Snowy };
    }
    if t < -0.17 {
        // Cold: always taiga, between the snow and everything milder.
        return Biome::Taiga;
    }
    if t < 0.22 {
        // Temperate.
        return if p.wet_low > 0.5 && t > -0.12 {
            Biome::Swamp
        } else if m > 0.24 && t > -0.02 {
            Biome::DarkForest
        } else if m > 0.12 && t < -0.04 && w < -0.42 {
            Biome::PaleGarden
        } else if m > 0.06 && (0.04..0.16).contains(&t) {
            Biome::BirchForest
        } else if m > 0.06 {
            Biome::Forest
        } else if w > 0.42 && m > -0.08 && h > sea + 5 {
            Biome::Cherry
        } else if h > sea + 16 && p.e > 0.05 && m > -0.1 {
            // High, worn-flat grassland.
            Biome::Meadow
        } else {
            Biome::Plains
        };
    }
    if t < 0.36 {
        // Warm: jungle where it's wet (mangroves on its wet lowland), savanna where it isn't.
        return if m > 0.16 {
            if p.wet_low > 0.4 { Biome::Mangrove } else { Biome::Jungle }
        } else {
            Biome::Savanna
        };
    }
    // Hot.
    if m > 0.12 {
        if p.wet_low > 0.4 { Biome::Mangrove } else { Biome::Jungle }
    } else if p.e < -0.12 {
        // Hot, dry and rugged: the stripy hills.
        Biome::Badlands
    } else {
        Biome::Desert
    }
}

/// Revision 9 separates alpine snow from cold lowlands and gives gentle
/// shores room for sand, leaving steep headlands to their inland biome.
pub fn pick_v9(p: &Place, slope: f32, rocky: bool) -> Biome {
    if p.river { return Biome::River; }
    if p.h < p.sea - 1 {
        return if p.t < -0.38 { Biome::FrozenOcean } else { sea_by_temperature(p.t.max(-0.32)) };
    }
    let alpine = p.h > p.sea + 52 || (p.pv > 0.8 && p.e < -0.35 && p.h > p.sea + 44);
    if alpine {
        let cooling = ((p.h - p.sea - 24).max(0) as f32 * 0.007).min(0.3);
        return if p.t - cooling < -0.32 { Biome::Snowy } else { Biome::StonyPeaks };
    }
    let coast = (COAST.0..COAST.1).contains(&p.c);
    let beach_top = if slope <= 0.125 { p.sea + 2 } else { p.sea + 1 };
    if coast && !rocky && slope <= 0.375 && p.h <= beach_top && p.wet_low < 0.5 {
        return if p.t < -0.38 { Biome::Snowy } else { Biome::Beach };
    }
    if p.t < -0.38 {
        return if p.w > 0.42 && p.h < p.sea + 30 { Biome::IceSpikes } else { Biome::Snowy };
    }
    if p.t < -0.17 { return Biome::Taiga; }
    // The shape and cold rules above replace the old ones; the warm inland
    // climate choices still use the same temperature/moisture bands.
    pick(&Place { c: if coast { COAST.1 } else { p.c }, ..*p })
}

/// River channel half-width in blocks; the broad valley has its own scale.
pub fn river_half_width(detail: f32) -> f32 {
    (7.0 + detail * 4.0).clamp(5.0, 11.0)
}

/// Convert a weirdness value into an approximate distance to its zero line.
/// Flat extrema with no measurable gradient do not create enormous lakes.
pub fn river_channel(reach: f32, gradient: f32, half_width: f32) -> f32 {
    (RIVER * reach).min(gradient.max(0.0) * half_width * reach)
}

/// A shallow centre, an underwater shelf, then a gradual bank. Both
/// sections meet with zero slope, including where the bank meets the valley.
pub fn river_bed(sea: f32, distance_in_widths: f32) -> f32 {
    if distance_in_widths <= 1.0 {
        sea - 4.0 + 3.0 * smoothstep(0.0, 1.0, distance_in_widths)
    } else {
        sea - 1.0 + 2.5 * smoothstep(1.0, 2.0, distance_in_widths)
    }
}

/// Ordinary inland climate choices are available to target-climate mods.
/// Shape landmarks and rare variants always keep their built-in identity.
pub fn mod_climate_space(p: &Place, biome: Biome) -> bool {
    p.h > p.sea + 2 && p.c >= COAST.1 && p.h <= p.sea + 44
        && !p.river && p.wet_low <= 0.4
        && matches!(biome, Biome::Plains | Biome::Forest | Biome::BirchForest | Biome::DarkForest | Biome::Taiga | Biome::Snowy | Biome::Savanna | Biome::Jungle | Biome::Desert | Biome::Badlands | Biome::Meadow)
}

impl Generator {
    /// The three shape noises at a column: continentalness, erosion and weirdness.
    pub fn shape_noise(&self, x: i32, z: i32) -> (f32, f32, f32) {
        let (fx, fz) = (x as f32, z as f32);
        let bs = self.opts.biome_scale().sqrt();
        // (A little warp, so no ridge or coast runs straight.)
        let (wx, wz) = (self.hills.noise2(fx / 97.0 - 210.0, fz / 97.0) * 30.0, self.hills.noise2(fx / 97.0, fz / 97.0 + 330.0) * 30.0);
        let norm = |v: f32| (v * 2.6).clamp(-1.0, 1.0);
        let scale = if self.opts.version >= 9 { 3.0 } else { 1.0 };
        let mut c = norm(self.continent.fbm2((fx + wx) / (900.0 * bs * scale), (fz + wz) / (900.0 * bs * scale), 4) + 0.11);
        let e = norm(self.erosion.fbm2((fx - wz) / (520.0 * bs * scale), (fz + wx) / (520.0 * bs * scale), 3));
        let w = norm(self.weird.fbm2((fx + wz * 0.5) / (440.0 * bs * scale), (fz - wx * 0.5) / (440.0 * bs * scale), 3));
        if self.opts.version >= 9 {
            // Bays/headlands add local variation only near the coast.
            let coastal = 1.0 - smoothstep(0.12, 0.3, (c + 0.12).abs());
            c = (c + self.hills.fbm2(fx / 180.0, fz / 180.0, 2) * 0.045 * coastal).clamp(-1.0, 1.0);
        }
        (c, e, w)
    }

    /// Surface height and biome for a column, in worlds from version 7.
    pub(crate) fn column_v7(&self, x: i32, z: i32) -> (i32, Biome) {
        let place = self.place_v7(x, z);
        let biome = if self.opts.version >= 9 {
            let (slope, rocky) = self.shore_shape(x, z, &place);
            pick_v9(&place, slope, rocky)
        } else { pick(&place) };
        let (h, biome) = (place.h, biome);
        // Far out in deep water, now and then, the sea floor rises into a mushroom island.
        if biome.is_ocean() && h < place.sea - 4 {
            let k = self.mushroom_isle(x, z);
            if k > 0.0 {
                let seaf = place.sea as f32;
                let top = seaf + 1.0 + (k - 0.5).max(0.0) * 10.0;
                let hh = (h as f32 + (top - h as f32) * (k * 2.0).min(1.0)).round() as i32;
                return (hh, if hh >= place.sea - 1 { Biome::MushroomIslands } else { biome });
            }
        }
        (h, biome)
    }

    /// The land's height and what the biome rules need, in worlds from version 7.
    pub fn place_v7(&self, x: i32, z: i32) -> Place {
        let (fx, fz) = (x as f32, z as f32);
        let sea = self.sea();
        let seaf = sea as f32;
        let (c, e, w) = self.shape_noise(x, z);
        let pv = peaks_and_valleys(w);
        let relief = self.opts.relief();
        // Inland the peaks and valleys count; at sea, hardly.
        let inland = smoothstep(-0.16, 0.08, c);
        let rugged = RUGGED.at(e);
        let mut h = seaf + CONTINENT.at(c);
        h += inland * PEAKS.at(pv) * rugged * relief;
        if self.opts.version >= 9 {
            let mountain = inland * smoothstep(0.15, 0.8, pv) * (1.0 - smoothstep(-0.15, 0.35, e));
            // Local saddles interrupt long ridges; shoulders soften their feet.
            let pass = smoothstep(0.12, 0.42, self.hills.noise2(fx / 360.0 + 91.0, fz / 360.0));
            h -= mountain * pass * 18.0 * relief;
            h += inland * smoothstep(-0.5, 0.3, pv) * (1.0 - smoothstep(0.1, 0.5, e))
                * (3.0 + self.hills.fbm2(fx / 240.0, fz / 240.0, 2) * 5.0) * relief;
            // Worn uplands flatten into plateaus, while occasional rugged
            // summits get a sharper cap without enlarging surface detail.
            let plateau = inland * smoothstep(0.1, 0.45, e) * smoothstep(0.1, 0.6, pv) * 0.45;
            h += (seaf + 18.0 + c.max(0.0) * 12.0 - h) * plateau;
            h += mountain * smoothstep(0.25, 0.55, self.hills.noise2(fx / 110.0 - 73.0, fz / 110.0)) * 16.0 * relief;
        }
        // A little detail on top: more where it's rugged.
        h += self.hills.fbm2(fx / 70.0, fz / 70.0, 3) * (1.5 + 6.0 * rugged) * (0.3 + 0.7 * inland) * relief.sqrt();
        // Tall peaks ease off toward the top of the world instead of being cut flat.
        let knee = seaf + 44.0;
        if h > knee {
            h = knee + (h - knee) / (1.0 + (h - knee) / 24.0);
        }
        // Rivers: along the valley floors (weirdness near nought), inland, and
        // not up in the mountains. The valley's sides come down to a bank a
        // little over the sea, then the channel drops below it.
        let reach = smoothstep(-0.26, -0.12, c) * (1.0 - smoothstep(seaf + 34.0, seaf + 60.0, h));
        let aw = w.abs();
        let valley = (1.0 - smoothstep(RIVER, RIVER_VALLEY, aw)) * reach;
        let bank = seaf + 1.5;
        h -= (h - bank).max(0.0) * valley;
        let channel = if self.opts.version >= 9 && aw < RIVER * reach * 2.0 {
            let dx = (self.shape_noise(x + 8, z).2 - self.shape_noise(x - 8, z).2) / 16.0;
            let dz = (self.shape_noise(x, z + 8).2 - self.shape_noise(x, z - 8).2) / 16.0;
            let gradient = (dx * dx + dz * dz).sqrt();
            let width = river_half_width(self.hills.noise2(fx / 170.0, fz / 170.0));
            river_channel(reach, gradient, width)
        } else { RIVER * reach };
        let river = aw < channel && h < seaf + 3.0;
        if self.opts.version >= 9 {
            // Shape the dry bank as well as the waterbed. The old cut only
            // touched river columns, leaving a three-block ledge at its edge.
            if channel > 0.0 && aw < channel * 2.0 && h < seaf + 3.0 {
                h = h.min(river_bed(seaf, aw / channel));
            }
        } else if river {
            // Deepest in the middle.
            let bed = seaf - 1.5 - 3.5 * (1.0 - aw / channel.max(1e-4));
            h = h.min(bed);
        }
        let (t, m) = self.climate(x, z);
        // Wet lowland: low, damp, not rugged. It gets a bumpy floor just
        // around the sea, pools and islands of mud and grass (low already,
        // so this moves it only a block or two).
        let wet_low = smoothstep(0.1, 0.26, m) * (1.0 - smoothstep(seaf + 1.0, seaf + 5.0, h)) * smoothstep(seaf - 4.0, seaf - 1.0, h) * smoothstep(-0.3, 0.0, e) * smoothstep(-0.14, -0.04, c);
        if !river {
            let bumps = seaf - 0.6 + self.hills.noise2(fx / 9.0, fz / 9.0) * 2.2;
            h += (bumps - h) * wet_low * 0.7;
        }
        let h = (h.round() as i32).clamp(4, sea + 68);
        Place { h, sea, c, e, w, pv, t, m, river, wet_low }
    }

    /// Local slope and rocky-headland flag, sampled only in the coastal band.
    pub fn shore_shape(&self, x: i32, z: i32, p: &Place) -> (f32, bool) {
        if !(COAST.0..COAST.1).contains(&p.c) || !(p.sea - 3..=p.sea + 5).contains(&p.h) || p.river || p.wet_low > 0.5 {
            return (0.0, false);
        }
        let slope = [(4, 0), (-4, 0), (0, 4), (0, -4)].iter()
            .map(|&(dx, dz)| (self.place_v7(x + dx, z + dz).h - p.h).abs() as f32 / 4.0)
            .fold(0.0, f32::max);
        let rocky = slope > 0.375 || (p.e < -0.25 && self.hills.noise2(x as f32 / 90.0, z as f32 / 90.0) > 0.05);
        (slope, rocky)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::GenOptions;

    fn place(h: i32, t: f32, m: f32) -> Place {
        Place { h, sea: 63, c: 0.3, e: 0.0, w: 0.2, pv: peaks_and_valleys(0.2), t, m, river: false, wet_low: 0.0 }
    }

    #[test]
    fn splines_pass_through_their_points_and_level_off() {
        assert_eq!(CONTINENT.at(-0.11), 0.0, "the coast is at the sea");
        assert_eq!(CONTINENT.at(-5.0), -46.0);
        assert_eq!(CONTINENT.at(5.0), 26.0);
        assert!((PEAKS.at(0.0) - 3.5).abs() < 1e-4, "halfway between its neighbours");
        for s in [&CONTINENT, &RUGGED, &PEAKS] {
            for w in s.0.windows(2) {
                assert!(w[0].0 < w[1].0);
            }
        }
        assert_eq!(peaks_and_valleys(0.0), -1.0, "valley floors where weirdness is nought");
        assert!((peaks_and_valleys(2.0 / 3.0) - 1.0).abs() < 1e-5, "ridges");
    }

    #[test]
    fn rivers_have_block_widths_independent_of_valley_scale() {
        for detail in [-2.0, -0.5, 0.0, 0.5, 2.0] {
            assert!((5.0..=11.0).contains(&river_half_width(detail)));
        }
        assert_eq!(river_channel(1.0, 0.0, 7.0), 0.0);
        for gradient in [0.0001, 0.001, 0.004] {
            let channel = river_channel(1.0, gradient, 7.0);
            assert!((channel / gradient - 7.0).abs() < 0.001);
            assert_eq!(river_channel(0.0, gradient, 7.0), 0.0);
        }
    }

    #[test]
    fn river_banks_rise_continuously_into_the_valley() {
        assert_eq!(river_bed(63.0,0.0),59.0);
        assert_eq!(river_bed(63.0,1.0),62.0);
        assert_eq!(river_bed(63.0,2.0),64.5);
        assert_eq!(river_bed(63.0,3.0),64.5);
        for width in [5.0,7.0,11.0] {
            let mut previous=river_bed(63.0,0.0);
            for block in 1..=(width*2.0) as i32 {
                let next=river_bed(63.0,block as f32/width);
                assert!(next>=previous && next-previous<=1.0,"width {width}: {previous} to {next}");
                previous=next;
            }
            let epsilon=0.001;
            assert!((river_bed(63.0,1.0-epsilon)-river_bed(63.0,1.0+epsilon)).abs()<0.001);
        }
    }

    #[test]
    fn generated_river_edges_do_not_leave_vertical_bank_ledges() {
        let mut checked=0;
        for seed in [424242,77,1234] {
            let g=Generator::with(seed,GenOptions::DEFAULT);
            for z in (-2048..2048).step_by(64) {
                for x in (-2048..2048).step_by(16) {
                    let (h,b)=g.column(x,z);
                    if b!=Biome::River { continue; }
                    for (dx,dz) in [(1,0),(-1,0),(0,1),(0,-1)] {
                        let (other,biome)=g.column(x+dx,z+dz);
                        if biome==Biome::River || biome.is_ocean() { continue; }
                        assert!((other-h).abs()<=2,"seed {seed}: bank jumps from {h} to {other} at {x},{z}");
                        checked+=1;
                    }
                }
            }
        }
        assert!(checked>=40,"only {checked} river bank edges exercised");
    }

    #[test]
    fn alpine_snow_and_cold_lowlands_are_separate() {
        assert_eq!(pick_v9(&place(120, -0.08, 0.0), 0.0, false), Biome::StonyPeaks);
        assert_eq!(pick_v9(&place(120, -0.14, 0.0), 0.0, false), Biome::Snowy);
        assert_eq!(pick_v9(&place(75, -0.35, 0.0), 0.0, false), Biome::Taiga);
        assert_eq!(pick_v9(&place(75, -0.4, 0.0), 0.0, false), Biome::Snowy);
        assert_eq!(pick_v9(&place(50, -0.35, 0.0), 0.0, false), Biome::Ocean);
        assert_eq!(pick_v9(&place(50, -0.4, 0.0), 0.0, false), Biome::FrozenOcean);
    }

    #[test]
    fn beaches_follow_gentle_slopes_and_leave_rocky_headlands() {
        let p = Place { c: -0.12, ..place(65, 0.1, 0.0) };
        assert_eq!(pick_v9(&p, 0.0, false), Biome::Beach);
        assert_ne!(pick_v9(&p, 0.25, false), Biome::Beach);
        assert_ne!(pick_v9(&p, 0.0, true), Biome::Beach);
        assert_ne!(pick_v9(&Place { h: 64, ..p }, 0.5, false), Biome::Beach);
        assert_eq!(pick_v9(&Place { river: true, ..p }, 0.0, true), Biome::River);
    }

    #[test]
    fn broader_generation_reduces_fragmentation_without_losing_landmarks() {
        let mut transitions = [0usize; 2];
        let mut landmarks = std::collections::HashSet::new();
        for seed in [424242, 77, 1234] {
            for (i, version) in [8, 9].into_iter().enumerate() {
                let g = Generator::with(seed, GenOptions { version, ..GenOptions::DEFAULT });
                for z in (-4096..4096).step_by(64) {
                    let mut previous = None;
                    for x in (-4096..4096).step_by(64) {
                        let (h, b) = g.column(x, z);
                        assert!((4..=g.sea()+68).contains(&h));
                        transitions[i] += usize::from(previous.is_some_and(|old| old != b));
                        previous = Some(b);
                        if version == 9 { landmarks.insert(b); }
                    }
                }
            }
        }
        assert!(transitions[1] * 5 < transitions[0] * 4, "{transitions:?}");
        for b in [Biome::River, Biome::Beach, Biome::StonyPeaks, Biome::Meadow, Biome::Snowy, Biome::Ocean] { assert!(landmarks.contains(&b), "missing {b:?}"); }
    }

    #[test]
    fn the_rules_read_the_shape_first() {
        let base = place(70, 0.0, 0.0);
        assert_eq!(pick(&base), Biome::Plains);
        assert_eq!(pick(&Place { river: true, ..base }), Biome::River);
        assert_eq!(pick(&Place { h: 50, ..base }), Biome::Ocean);
        assert_eq!(pick(&Place { h: 50, t: 0.4, ..base }), Biome::WarmOcean);
        assert_eq!(pick(&Place { h: 64, c: -0.12, ..base }), Biome::Beach);
        assert_eq!(pick(&Place { h: 64, c: -0.12, t: -0.5, ..base }), Biome::Snowy, "a snowy shore");
        assert_eq!(pick(&Place { h: 120, ..base }), Biome::StonyPeaks);
        assert_eq!(pick(&Place { h: 120, t: -0.2, ..base }), Biome::Snowy);
        // Then the bands, moisture, and weirdness for the rare ones.
        assert_eq!(pick(&Place { t: -0.4, ..base }), Biome::Snowy);
        assert_eq!(pick(&Place { t: -0.4, w: 0.6, ..base }), Biome::IceSpikes);
        assert_eq!(pick(&Place { t: -0.2, ..base }), Biome::Taiga);
        assert_eq!(pick(&Place { m: 0.1, t: 0.1, ..base }), Biome::BirchForest);
        assert_eq!(pick(&Place { m: 0.1, t: -0.1, ..base }), Biome::Forest);
        assert_eq!(pick(&Place { m: 0.3, ..base }), Biome::DarkForest);
        assert_eq!(pick(&Place { m: 0.2, t: -0.1, w: -0.6, ..base }), Biome::PaleGarden);
        assert_eq!(pick(&Place { w: 0.6, ..base }), Biome::Cherry);
        assert_eq!(pick(&Place { h: 85, e: 0.3, ..base }), Biome::Meadow);
        assert_eq!(pick(&Place { wet_low: 0.8, h: 64, ..base }), Biome::Swamp);
        assert_eq!(pick(&Place { t: 0.3, ..base }), Biome::Savanna);
        assert_eq!(pick(&Place { t: 0.3, m: 0.2, ..base }), Biome::Jungle);
        assert_eq!(pick(&Place { t: 0.3, m: 0.2, wet_low: 0.8, ..base }), Biome::Mangrove);
        assert_eq!(pick(&Place { t: 0.5, ..base }), Biome::Desert);
        assert_eq!(pick(&Place { t: 0.5, e: -0.4, ..base }), Biome::Badlands);
    }

    /// A wide look at a seed: biomes (and heights) every few blocks.
    fn survey(seed: u32, step: i32, n: i32) -> Vec<(i32, Biome)> {
        let g = Generator::with(seed, GenOptions::DEFAULT);
        let mut out = Vec::new();
        for j in -n / 2..n / 2 {
            for i in -n / 2..n / 2 {
                out.push(g.column(i * step, j * step));
            }
        }
        out
    }

    #[test]
    fn new_worlds_have_rivers_beaches_and_plenty_of_land() {
        for seed in [424242, 1234, 77] {
            let cols = survey(seed, 16, 128);
            let share = |b: Biome| cols.iter().filter(|c| c.1 == b).count() as f32 / cols.len() as f32;
            let sea: f32 = Biome::ALL.iter().filter(|b| b.is_ocean()).map(|&b| share(b)).sum();
            assert!((0.2..0.7).contains(&sea), "seed {seed}: sea {sea}");
            assert!(share(Biome::River) > 0.004, "seed {seed}: rivers {}", share(Biome::River));
            assert!(share(Biome::Beach) > 0.004, "seed {seed}: beaches {}", share(Biome::Beach));
            let kinds = Biome::ALL.iter().filter(|&&b| share(b) > 0.0).count();
            assert!(kinds >= 16, "seed {seed}: only {kinds} kinds of biome");
            // The heights make sense for their biome.
            for &(h, b) in &cols {
                match b {
                    Biome::River => assert!(h < 63, "a river below the sea"),
                    Biome::Beach => assert!((62..=65).contains(&h)),
                    b if b.is_ocean() => assert!(h < 62),
                    _ => assert!(h >= 62),
                }
            }
        }
    }

    /// Fixed, broad seed coverage rather than assertions about exact rarity.
    const SURVEY_SEEDS: [u32; 12] = [424242, 1234, 77, 1, 3, 6, 11, 42, 2024, 31337, 65535, 987654321];

    #[test]
    fn many_seeds_keep_variety_depth_and_sensible_neighbours() {
        let mut all = std::collections::HashSet::new();
        let mut caves = std::collections::HashSet::new();
        for seed in SURVEY_SEEDS {
            let g = Generator::with(seed, GenOptions::DEFAULT);
            let mut kinds = std::collections::HashSet::new();
            let mut ocean = 0;
            for z in (-4096..4096).step_by(64) {
                for x in (-4096..4096).step_by(64) {
                    let (h, b) = g.column(x, z);
                    kinds.insert(b);
                    all.insert(b);
                    ocean += usize::from(b.is_ocean());
                    for y in [10, 32, 48] { caves.insert(g.cave_biome_at(x, y, z)); }
                    for (dx, dz) in [(16, 0), (0, 16)] {
                        let neighbour = g.column(x + dx, z + dz).1;
                        let frozen = |b| matches!(b, Biome::Snowy | Biome::IceSpikes | Biome::FrozenOcean);
                        let hot = |b| matches!(b, Biome::Desert | Biome::Badlands | Biome::Jungle | Biome::Mangrove | Biome::WarmOcean);
                        assert!(!(frozen(b) && hot(neighbour) || hot(b) && frozen(neighbour)), "seed {seed}: {b:?} next to {neighbour:?} at {x},{z}");
                    }
                    if b == Biome::River { assert!(h < g.sea()); }
                    if b == Biome::Beach { assert!((g.sea()-1..=g.sea()+2).contains(&h)); }
                }
            }
            assert!(kinds.len() >= 12, "seed {seed}: {kinds:?}");
            assert!((800..14000).contains(&ocean), "seed {seed}: {ocean} ocean columns of 16384");
        }
        assert!(all.len() >= 20, "only {all:?}");
        for b in [crate::caves::CaveBiome::Plain, crate::caves::CaveBiome::Dripstone, crate::caves::CaveBiome::Lush, crate::caves::CaveBiome::DeepDark] { assert!(caves.contains(&b)); }
    }

    #[test]
    fn rivers_reach_the_sea() {
        // Follow river columns from the sea: some river touches an ocean.
        let g = Generator::with(424242, GenOptions { version: 7, ..GenOptions::DEFAULT });
        let mut meets = 0;
        for z in (-1500..1500).step_by(6) {
            for x in (-1500..1500).step_by(6) {
                if g.column(x, z).1 == Biome::River && [(6, 0), (-6, 0), (0, 6), (0, -6)].iter().any(|&(dx, dz)| g.column(x + dx, z + dz).1.is_ocean()) {
                    meets += 1;
                }
            }
        }
        assert!(meets > 0, "no river meets the sea");
    }

    #[test]
    fn new_worlds_start_on_dry_level_ground() {
        for seed in [1234u32, 3, 6, 77] {
            let w = crate::world::World::with_options(seed, GenOptions::DEFAULT);
            let s = w.find_spawn();
            let (x, z) = (s.x as i32, s.z as i32);
            let h = w.generator.column(x, z).0;
            for (dx, dz) in [(8, 0), (-8, 0), (0, 8), (0, -8)] {
                let (hh, b) = w.generator.column(x + dx, z + dz);
                assert!((hh - h).abs() <= 4 && !b.is_ocean() && b != Biome::River, "seed {seed}: {b:?} at {hh} beside a spawn at {h}");
            }
        }
    }

    /// Writes biome maps (raw bytes, one per column) for a look at them:
    /// `MINCERAFT_BIOME_MAPS=dir cargo test --release biome_maps -- --ignored`.
    #[test]
    #[ignore]
    fn biome_maps() {
        let Ok(dir) = std::env::var("MINCERAFT_BIOME_MAPS") else { return };
        let chunks = std::env::var("MINCERAFT_MAP_CHUNKS").map_or(256, |v| v.parse::<i32>().expect("map chunks must be an integer"));
        assert!((2..=1024).contains(&chunks) && chunks % 2 == 0, "map chunks must be even and between 2 and 1024");
        let n = chunks * 2; // One sample every eight blocks, sixteen blocks/chunk.
        let surface_only = std::env::var("MINCERAFT_MAP_SURFACE_ONLY").is_ok_and(|v| v == "1");
        let names: Vec<String> = Biome::ALL.iter().map(|b| format!("{b:?}")).collect();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(format!("{dir}/biome_names.txt"), names.join("\n")).unwrap();
        std::fs::write(format!("{dir}/cave_names.txt"), "Plain\nDripstone\nLush\nDeepDark").unwrap();
        std::fs::write(format!("{dir}/map_metadata.txt"), format!("generator={}\nchunks={}\nsamples={}\nstep=8\nminimum={}\n", GenOptions::DEFAULT.version, chunks, n, -chunks*8)).unwrap();
        for seed in SURVEY_SEEDS {
            let g = Generator::with(seed, GenOptions::DEFAULT);
            let cols = survey(seed, 8, n);
            let raw: Vec<u8> = cols.iter().map(|c| Biome::ALL.iter().position(|&b| b == c.1).unwrap() as u8).collect();
            std::fs::write(format!("{dir}/biomes_{seed}.raw"), raw).unwrap();
            let hs: Vec<u8> = cols.iter().map(|c| c.0.clamp(0, 255) as u8).collect();
            std::fs::write(format!("{dir}/heights_{seed}.raw"), hs).unwrap();
            if surface_only { continue; }
            for depth in [10, 32, 48] {
                let raw: Vec<u8> = (-n/2..n/2).flat_map(|j| (-n/2..n/2).map(move |i| (i * 8, j * 8))).map(|(x,z)| g.cave_biome_at(x, depth, z) as u8).collect();
                std::fs::write(format!("{dir}/caves_{seed}_{depth}.raw"), raw).unwrap();
            }
            let mods: Vec<u8> = (-n/2..n/2).flat_map(|j| (-n/2..n/2).map(move |i| (i * 8, j * 8))).map(|(x,z)| g.mod_biome_at(x,z,g.column(x,z).1).map_or(0, |i| i as u8 + 1)).collect();
            std::fs::write(format!("{dir}/mods_{seed}.raw"), mods).unwrap();
            let mut sites = String::from("kind,x,y,z\n");
            for cz in -chunks/2..chunks/2 {
                for cx in -chunks/2..chunks/2 {
                    if let Some(site) = g.site(cx,cz) { sites.push_str(&format!("{:?},{},{},{}\n",site.kind,site.origin.x,site.origin.y,site.origin.z)); }
                }
            }
            std::fs::write(format!("{dir}/sites_{seed}.csv"), sites).unwrap();
        }
    }
}
