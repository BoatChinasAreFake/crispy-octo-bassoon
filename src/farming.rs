//! Farming, with considerably more moving parts than it needs.
//!
//! Till grass or dirt with a hoe to make farmland, then plant wheat seeds,
//! carrots or potatoes. Every tilled block keeps its own `Soil` record (on the
//! machine that owns the world), and each second every crop's growth is the
//! product of:
//!
//! - **moisture**: water within 4 blocks makes farmland hydrated (x1.0), else x0.35;
//! - **light**: sunshine or a nearby torch/lantern (x1.0), dim (x0.4), dark (x0);
//! - **nutrients**: wheat eats nitrogen, carrots phosphorus, potatoes potassium.
//!   Starved crops crawl along (x0.1); well-fed ones get up to x1.0;
//! - **crop rotation**: a different crop from the last harvest here is x1.5,
//!   the same crop three times running is "soil fatigue" (x0.6);
//! - **company**: crops grow 20% faster with a player nearby (quantum farming).
//!
//! Meanwhile weeds sprout on bare farmland and steal nutrients from their
//! neighbours, Clucksters peck at seedlings unless a Scarecrow is nearby,
//! jumping on farmland tramples it, and dry, unused farmland goes back to dirt.
//! Bone Dust, Compost and Wood Ash put nutrients back; a Soil Probe tells you
//! all of this in one very long sentence.

use crate::block::*;
use crate::noise::Rng;
use macroquad::math::{ivec3, IVec3, Vec3};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Crop {
    Wheat,
    Carrot,
    Potato,
}

/// Seconds per growth stage at x1.0.
pub const STAGE_SECONDS: f32 = 40.0;
/// Nutrient used up per growth stage.
const STAGE_COST: f32 = 6.0;

impl Crop {
    pub const ALL: [Crop; 3] = [Crop::Wheat, Crop::Carrot, Crop::Potato];

    pub fn name(self) -> &'static str {
        match self {
            Crop::Wheat => "wheat",
            Crop::Carrot => "carrots",
            Crop::Potato => "potatoes",
        }
    }
    /// Block id of stage 0.
    pub fn base(self) -> Id {
        match self {
            Crop::Wheat => WHEAT_0,
            Crop::Carrot => CARROT_0,
            Crop::Potato => POTATO_0,
        }
    }
    pub fn block(self, stage: u8) -> Id {
        self.base() + stage.min(3) as Id
    }
    /// The crop (and growth stage) a block is, if any.
    pub fn of_block(id: Id) -> Option<(Crop, u8)> {
        Crop::ALL.into_iter().find(|c| (c.base()..c.base() + 4).contains(&id)).map(|c| (c, (id - c.base()) as u8))
    }
    /// What you plant to grow it.
    pub fn from_item(item: Id) -> Option<Crop> {
        match item {
            WHEAT_SEEDS => Some(Crop::Wheat),
            CARROT => Some(Crop::Carrot),
            POTATO => Some(Crop::Potato),
            _ => None,
        }
    }
    /// Index into `Soil::nutrients` of what it feeds on.
    pub fn nutrient(self) -> usize {
        match self {
            Crop::Wheat => 0,
            Crop::Carrot => 1,
            Crop::Potato => 2,
        }
    }
    fn index(self) -> u8 {
        Crop::ALL.iter().position(|c| *c == self).unwrap_or(0) as u8
    }
    fn from_index(i: u8) -> Option<Crop> {
        Crop::ALL.get(i as usize).copied()
    }

    /// Drops when broken at `stage`. Unripe crops just give the seed back.
    pub fn harvest(self, stage: u8, rng: &mut Rng) -> Vec<(Id, u8)> {
        if stage < 3 {
            let seed = match self {
                Crop::Wheat => WHEAT_SEEDS,
                Crop::Carrot => CARROT,
                Crop::Potato => POTATO,
            };
            return vec![(seed, 1)];
        }
        match self {
            Crop::Wheat => vec![(WHEAT, 1), (WHEAT_SEEDS, rng.int(1, 3) as u8)],
            Crop::Carrot => vec![(CARROT, rng.int(2, 4) as u8)],
            Crop::Potato => vec![(POTATO, rng.int(2, 4) as u8)],
        }
    }
}

pub fn is_farmland(id: Id) -> bool {
    id == FARMLAND || id == FARMLAND_WET
}

/// Everything a tilled block remembers.
#[derive(Clone, Debug, PartialEq)]
pub struct Soil {
    /// Nitrogen, phosphorus, potassium, 0..100.
    pub nutrients: [f32; 3],
    pub wet: bool,
    /// Progress toward the next growth stage of the crop on top (0..1).
    pub progress: f32,
    /// The crop last seen growing here, and its stage.
    pub crop: Option<Crop>,
    pub stage: u8,
    /// The crop last harvested here, and how many times in a row.
    pub last: Option<Crop>,
    pub streak: u8,
    /// Seconds spent dry and bare (it reverts to dirt eventually).
    pub idle: f32,
}

impl Default for Soil {
    fn default() -> Soil {
        Soil { nutrients: [60.0; 3], wet: false, progress: 0.0, crop: None, stage: 0, last: None, streak: 0, idle: 0.0 }
    }
}

/// The multipliers behind a crop's growth rate (shown by the Soil Probe).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Factors {
    pub moisture: f32,
    pub light: f32,
    pub nutrient: f32,
    pub rotation: f32,
    pub company: f32,
}

impl Factors {
    pub fn total(&self) -> f32 {
        self.moisture * self.light * self.nutrient * self.rotation * self.company
    }
}

impl Soil {
    /// Is the crop now growing here different from the last one harvested?
    pub fn rotated(&self) -> bool {
        matches!((self.crop, self.last), (Some(c), Some(l)) if c != l)
    }

    pub fn factors(&self, crop: Crop, light: f32, watched: bool) -> Factors {
        let n = self.nutrients[crop.nutrient()];
        Factors {
            moisture: if self.wet { 1.0 } else { 0.35 },
            light: if light >= 0.5 {
                1.0
            } else if light >= 0.2 {
                0.4
            } else {
                0.0
            },
            nutrient: if n < 10.0 { 0.1 } else { 0.6 + n / 250.0 },
            rotation: if self.rotated() {
                1.5
            } else if self.last == Some(crop) && self.streak >= 2 {
                0.6
            } else {
                1.0
            },
            company: if watched { 1.2 } else { 1.0 },
        }
    }

    /// Grow for `dt` seconds. Returns the new stage if it advanced.
    pub fn grow(&mut self, crop: Crop, f: &Factors, dt: f32) -> Option<u8> {
        if self.stage >= 3 {
            return None;
        }
        self.progress += f.total() * dt / STAGE_SECONDS;
        if self.progress < 1.0 {
            return None;
        }
        self.progress = 0.0;
        self.stage += 1;
        let n = &mut self.nutrients[crop.nutrient()];
        *n = (*n - STAGE_COST).max(0.0);
        Some(self.stage)
    }

    /// Note what's on top now; returns the crop that was just harvested
    /// (removed when ripe), if that's what happened.
    pub fn observe(&mut self, top: Option<(Crop, u8)>) -> Option<Crop> {
        let mut harvested = None;
        let same = matches!((self.crop, top), (Some(a), Some((b, _))) if a == b);
        if !same {
            if let Some(c) = self.crop
                && self.stage >= 3
            {
                // Harvested ripe: that's what the next crop rotates against.
                if self.last == Some(c) {
                    self.streak = self.streak.saturating_add(1);
                } else {
                    self.last = Some(c);
                    self.streak = 1;
                }
                harvested = Some(c);
            }
            self.progress = 0.0;
        }
        self.crop = top.map(|t| t.0);
        self.stage = top.map(|t| t.1).unwrap_or(0);
        harvested
    }

    /// Apply a fertiliser item. Returns a description, or None if it isn't one.
    pub fn fertilize(&mut self, item: Id) -> Option<&'static str> {
        let (add, what): ([f32; 3], &str) = match item {
            BONE_DUST => ([5.0, 25.0, 0.0], "Phosphorus up (and a little nitrogen). The soil feels... bony."),
            COMPOST => ([25.0, 0.0, 5.0], "Nitrogen up. It smells exactly how you'd expect."),
            WOOD_ASH => ([0.0, 0.0, 25.0], "Potassium up. The soil is now slightly campfire-scented."),
            _ => return None,
        };
        for (n, a) in self.nutrients.iter_mut().zip(add) {
            *n = (*n + a).min(100.0);
        }
        Some(what)
    }

    /// The Soil Probe's lab report.
    pub fn report(&self, light: f32, watched: bool) -> String {
        let [n, p, k] = self.nutrients.map(|v| v.round() as i32);
        let mut s = format!("Soil: {} | N {n} P {p} K {k}", if self.wet { "hydrated" } else { "dry (needs water within 4 blocks)" });
        match self.last {
            Some(l) => s += &format!(" | last harvest: {} (x{})", l.name(), self.streak),
            None => s += " | never harvested",
        }
        match self.crop {
            Some(c) if self.stage >= 3 => s += &format!(" | {} ready to harvest!", c.name()),
            Some(c) => {
                let f = self.factors(c, light, watched);
                let mut why = Vec::new();
                if f.moisture < 1.0 {
                    why.push("thirsty");
                }
                if f.light == 0.0 {
                    why.push("too dark");
                } else if f.light < 1.0 {
                    why.push("dim");
                }
                if f.nutrient < 0.5 {
                    why.push(match c.nutrient() {
                        0 => "starving for nitrogen (Compost)",
                        1 => "starving for phosphorus (Bone Dust)",
                        _ => "starving for potassium (Wood Ash)",
                    });
                }
                if f.rotation > 1.0 {
                    why.push("rotation bonus!");
                } else if f.rotation < 1.0 {
                    why.push("soil fatigue (rotate your crops)");
                }
                if f.company > 1.0 {
                    why.push("enjoying the company");
                }
                s += &format!(" | {} stage {}/3, {:.0}% there, growing x{:.2}", c.name(), self.stage, self.progress * 100.0, f.total());
                if !why.is_empty() {
                    s += &format!(" ({})", why.join(", "));
                }
            }
            None => s += " | nothing planted",
        }
        s
    }
}

// ------------------------------------------------------------------ saving

/// Pack the farm for the save file.
pub fn encode(farm: &std::collections::HashMap<IVec3, Soil>) -> Vec<u8> {
    let mut out = Vec::with_capacity(farm.len() * 34);
    let mut entries: Vec<_> = farm.iter().collect();
    entries.sort_by_key(|(p, _)| (p.x, p.y, p.z));
    for (p, s) in entries {
        for v in [p.x, p.y, p.z] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for v in s.nutrients.iter().chain([&s.progress, &s.idle]) {
            out.extend_from_slice(&v.to_le_bytes());
        }
        let crop = s.crop.map(|c| c.index() + 1).unwrap_or(0);
        let last = s.last.map(|c| c.index() + 1).unwrap_or(0);
        out.extend_from_slice(&[s.wet as u8, crop, s.stage, last, s.streak]);
    }
    out
}

/// Unpack `encode`'s output (malformed tails are ignored).
pub fn decode(b: &[u8]) -> std::collections::HashMap<IVec3, Soil> {
    const SIZE: usize = 12 + 20 + 5;
    let mut farm = std::collections::HashMap::new();
    for e in b.chunks_exact(SIZE) {
        let i = |o: usize| i32::from_le_bytes(e[o..o + 4].try_into().unwrap());
        let f = |o: usize| f32::from_le_bytes(e[o..o + 4].try_into().unwrap()).clamp(0.0, 1000.0);
        let soil = Soil {
            nutrients: [f(12).min(100.0), f(16).min(100.0), f(20).min(100.0)],
            progress: f(24).min(1.0),
            idle: f(28),
            wet: e[32] != 0,
            crop: e[33].checked_sub(1).and_then(Crop::from_index),
            stage: e[34].min(3),
            last: e[35].checked_sub(1).and_then(Crop::from_index),
            streak: e[36],
        };
        farm.insert(ivec3(i(0), i(4), i(8)), soil);
    }
    farm
}

/// Offsets checked for hydrating water: 4 blocks around, level or one up.
pub fn water_offsets() -> impl Iterator<Item = IVec3> {
    (0..=1).flat_map(|dy| (-4..=4).flat_map(move |dz| (-4..=4).map(move |dx| ivec3(dx, dy, dz))))
}

/// A light source's reach counts as "a nearby torch" for crops.
pub fn is_farm_light(id: Id) -> bool {
    block(id).light > 0.0
}

/// Where a Cluckster at `pos` would peck: crop cells within 1.6 blocks.
pub fn in_peck_range(cluckster: Vec3, crop: IVec3) -> bool {
    let c = crop.as_vec3() + Vec3::new(0.5, 0.0, 0.5);
    Vec3::new(c.x - cluckster.x, 0.0, c.z - cluckster.z).length() < 1.6 && (c.y - cluckster.y).abs() < 1.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crops_map_to_blocks_and_back() {
        for c in Crop::ALL {
            for s in 0..4 {
                assert_eq!(Crop::of_block(c.block(s)), Some((c, s)));
            }
        }
        assert_eq!(Crop::of_block(STONE), None);
        assert_eq!(Crop::from_item(WHEAT_SEEDS), Some(Crop::Wheat));
        assert_eq!(block(Crop::Potato.block(3)).key, "potatoes_3");
    }

    #[test]
    fn growth_depends_on_everything() {
        let mut s = Soil { wet: true, ..Soil::default() };
        s.observe(Some((Crop::Wheat, 0)));
        let sunny = s.factors(Crop::Wheat, 1.0, false);
        assert_eq!(s.factors(Crop::Wheat, 0.0, false).total(), 0.0, "no growth in the dark");
        let dry = Soil { wet: false, ..s.clone() }.factors(Crop::Wheat, 1.0, false);
        assert!(dry.total() < sunny.total());
        let starved = Soil { nutrients: [2.0, 60.0, 60.0], ..s.clone() }.factors(Crop::Wheat, 1.0, false);
        assert!(starved.total() < sunny.total() * 0.2);
        assert!(s.factors(Crop::Wheat, 1.0, true).total() > sunny.total(), "company helps");

        // One stage takes STAGE_SECONDS / rate seconds and costs nitrogen.
        let secs = STAGE_SECONDS / sunny.total();
        let n0 = s.nutrients[0];
        assert_eq!(s.grow(Crop::Wheat, &sunny, secs * 0.9), None);
        assert_eq!(s.grow(Crop::Wheat, &sunny, secs * 0.2), Some(1));
        assert!(s.nutrients[0] < n0);
    }

    #[test]
    fn rotation_and_fatigue() {
        let mut s = Soil { wet: true, ..Soil::default() };
        let harvest = |s: &mut Soil, c: Crop| {
            s.observe(Some((c, 0)));
            s.observe(Some((c, 3)));
            s.observe(None)
        };
        assert_eq!(harvest(&mut s, Crop::Wheat), Some(Crop::Wheat));
        assert_eq!(harvest(&mut s, Crop::Wheat), Some(Crop::Wheat));
        s.observe(Some((Crop::Wheat, 0)));
        assert_eq!(s.factors(Crop::Wheat, 1.0, false).rotation, 0.6, "wheat after wheat after wheat");
        s.observe(None);
        s.observe(Some((Crop::Potato, 0)));
        assert!(s.rotated());
        assert_eq!(s.factors(Crop::Potato, 1.0, false).rotation, 1.5);
        // Breaking an unripe crop isn't a harvest.
        assert_eq!(s.observe(None), None);
        assert_eq!(s.last, Some(Crop::Wheat));
    }

    #[test]
    fn fertilisers_and_saving() {
        let mut s = Soil { nutrients: [0.0, 0.0, 0.0], ..Soil::default() };
        assert!(s.fertilize(BONE_DUST).is_some());
        assert!(s.fertilize(DIRT).is_none());
        assert_eq!(s.nutrients, [5.0, 25.0, 0.0]);
        for _ in 0..10 {
            s.fertilize(WOOD_ASH);
        }
        assert_eq!(s.nutrients[2], 100.0, "capped");
        s.crop = Some(Crop::Carrot);
        s.stage = 2;
        s.last = Some(Crop::Wheat);
        s.streak = 3;
        let mut farm = std::collections::HashMap::new();
        farm.insert(ivec3(-5, 44, 1000), s.clone());
        farm.insert(ivec3(0, 1, 2), Soil::default());
        assert_eq!(decode(&encode(&farm)), farm);
        assert!(s.report(1.0, false).contains("carrots stage 2/3"));
    }
}

// ------------------------------------------------------------------ in the game

use crate::entity::MobKind;
use crate::game::Game;
use crate::sound::{Mat, Sfx};

impl Game {
    /// Once a second: water, light, growth, weeds, fallow recovery, pests.
    /// Runs wherever the world lives (single player, host, server).
    pub fn farm_tick(&mut self, dt: f32) {
        self.farm_timer += dt;
        if self.farm_timer < 1.0 {
            return;
        }
        let step = std::mem::take(&mut self.farm_timer);
        let daylight = self.daylight();
        let players: Vec<Vec3> = self.player_targets().into_iter().map(|t| t.1).collect();
        let keys: Vec<IVec3> = self.world.farm.keys().copied().filter(|p| self.world.is_loaded(p.x, p.z)).collect();
        let mut edits: Vec<(IVec3, Id)> = Vec::new();
        for &p in &keys {
            let above = p + IVec3::Y;
            let top_id = self.world.get_v(above);
            let top = Crop::of_block(top_id);
            let wet = water_offsets().any(|o| self.world.get_v(p + o) == WATER);
            let sky = self.world.sky_light(above.x, above.y, above.z) * daylight;
            let light = if sky < 0.5 && self.light_near(above, 5) { 0.8 } else { sky };
            let watched = players.iter().any(|q| q.distance(above.as_vec3()) < 10.0);
            let weed_roll = self.rng.chance(step / 150.0);
            let Some(soil) = self.world.farm.get_mut(&p) else { continue };
            soil.wet = wet;
            soil.observe(top);
            match top {
                Some((crop, _)) => {
                    soil.idle = 0.0;
                    let f = soil.factors(crop, light, watched);
                    if let Some(stage) = soil.grow(crop, &f, step) {
                        edits.push((above, crop.block(stage)));
                    }
                }
                None => {
                    // Fallow soil slowly recovers (weeds, meanwhile, eat it).
                    for n in soil.nutrients.iter_mut() {
                        if *n < 70.0 {
                            *n = (*n + 0.05 * step).min(70.0);
                        }
                        if top_id == WEEDS {
                            *n = (*n - 0.15 * step).max(0.0);
                        }
                    }
                    if !wet && top_id == AIR {
                        soil.idle += step;
                    } else {
                        soil.idle = 0.0;
                    }
                    if soil.idle > 120.0 {
                        edits.push((p, DIRT));
                        continue;
                    }
                    if top_id == AIR && weed_roll {
                        edits.push((above, WEEDS));
                    }
                }
            }
            let want = if wet { FARMLAND_WET } else { FARMLAND };
            if self.world.get_v(p) != want {
                edits.push((p, want));
            }
            // Weeds also steal from the neighbours.
            if top_id == WEEDS {
                for d in [IVec3::X, -IVec3::X, IVec3::Z, -IVec3::Z] {
                    if let Some(n) = self.world.farm.get_mut(&(p + d)) {
                        for v in n.nutrients.iter_mut() {
                            *v = (*v - 0.1 * step).max(0.0);
                        }
                    }
                }
            }
        }
        // Clucksters peck at seedlings, unless a Scarecrow is keeping watch.
        let clucksters: Vec<Vec3> = self.mobs.iter().filter(|m| m.kind == MobKind::Cluckster).map(|m| m.body.pos).collect();
        for c in clucksters {
            for &p in &keys {
                let above = p + IVec3::Y;
                let seedling = matches!(Crop::of_block(self.world.get_v(above)), Some((_, s)) if s <= 1);
                if seedling && in_peck_range(c, above) && self.rng.chance(0.12 * step) && !self.scarecrow_near(above, 8) {
                    edits.push((above, AIR));
                    if players.iter().any(|q| q.distance(c) < 24.0) {
                        self.msg("A Cluckster ate your seedlings. Consider a Scarecrow.");
                    }
                }
            }
        }
        for (p, id) in edits {
            self.world.set_v(p, id);
        }
    }

    fn light_near(&self, p: IVec3, r: i32) -> bool {
        (-2..=2).any(|dy| (-r..=r).any(|dz| (-r..=r).any(|dx| is_farm_light(self.world.get_v(p + ivec3(dx, dy, dz))))))
    }

    fn scarecrow_near(&self, p: IVec3, r: i32) -> bool {
        (-2..=2).any(|dy| (-r..=r).any(|dz| (-r..=r).any(|dx| self.world.get_v(p + ivec3(dx, dy, dz)) == SCARECROW)))
    }

    /// Fertiliser or Soil Probe used on farmland (or a crop on it) at `pos`.
    /// Returns what to tell the player; None if there's no soil there.
    pub fn farm_interact(&mut self, pos: IVec3, item: Id) -> Option<String> {
        let soil_pos = if is_farmland(self.world.get_v(pos)) { pos } else { pos - IVec3::Y };
        let above = soil_pos + IVec3::Y;
        let sky = self.world.sky_light(above.x, above.y, above.z) * self.daylight();
        let light = if sky < 0.5 && self.light_near(above, 5) { 0.8 } else { sky };
        let watched = true; // somebody is holding the probe
        let top = Crop::of_block(self.world.get_v(above));
        let soil = self.world.farm.get_mut(&soil_pos)?;
        soil.observe(top);
        if item == SOIL_PROBE {
            return Some(soil.report(light, watched));
        }
        soil.fertilize(item).map(str::to_string)
    }

    /// Right-click farming actions: tilling, planting, fertilising, probing.
    /// Returns true if the click was used up.
    pub fn farm_use(&mut self, held: Id, pos: IVec3) -> bool {
        let id = self.world.get_v(pos);
        let above = pos + IVec3::Y;
        let above_id = self.world.get_v(above);
        if held == HOE && matches!(id, GRASS | DIRT | SNOW_GRASS) && matches!(above_id, AIR | TALL_GRASS | FLOWER | WEEDS) {
            if above_id != AIR {
                self.world.set_v(above, AIR);
            }
            self.world.set_v(pos, FARMLAND);
            self.sfx(Sfx::Place(Mat::Grass), Some(pos.as_vec3() + Vec3::splat(0.5)));
            self.player.swing = 1.0;
            return true;
        }
        if let Some(crop) = Crop::from_item(held)
            && is_farmland(id)
            && above_id == AIR
        {
            self.world.set_v(above, crop.block(0));
            self.sfx(Sfx::Place(Mat::Grass), Some(above.as_vec3() + Vec3::splat(0.5)));
            self.player.swing = 1.0;
            if !self.creative {
                self.inv.consume_held();
            }
            return true;
        }
        let soily = is_farmland(id) || (Crop::of_block(id).is_some() && is_farmland(self.world.get_v(pos - IVec3::Y)));
        if matches!(held, BONE_DUST | COMPOST | WOOD_ASH | SOIL_PROBE) && soily {
            self.player.swing = 1.0;
            if held == SOIL_PROBE {
                self.advance("soil_scientist");
            } else {
                self.sfx(Sfx::Place(Mat::Sand), Some(pos.as_vec3() + Vec3::splat(0.5)));
                if !self.creative {
                    self.inv.consume_held();
                }
            }
            if self.is_client() {
                // The soil lives on the host: it replies with a chat message.
                self.net_send_msg(crate::net::Msg::Interact { x: pos.x, y: pos.y, z: pos.z, item: held });
            } else if let Some(m) = self.farm_interact(pos, held) {
                self.msg(m);
            }
            return true;
        }
        false
    }

    /// Extra drops when the local player breaks farm-ish things.
    pub fn farm_drops(&mut self, pos: IVec3, id: Id) -> Vec<(Id, u8)> {
        if let Some((crop, stage)) = Crop::of_block(id) {
            if stage == 3 {
                self.advance("green_thumb");
                let rotated = self.world.farm.get(&(pos - IVec3::Y)).map(|s| s.rotated()).unwrap_or(false);
                if rotated {
                    self.advance("crop_rotation");
                    self.msg("Crop rotation bonus! The soil appreciates the variety.");
                }
            }
            return crop.harvest(stage, &mut self.rng);
        }
        let r = self.rng.f32();
        match id {
            TALL_GRASS if r < 0.12 => vec![(WHEAT_SEEDS, 1)],
            TALL_GRASS if r < 0.15 => vec![(CARROT, 1)],
            TALL_GRASS if r < 0.18 => vec![(POTATO, 1)],
            WEEDS => {
                self.advance("weed_whacker");
                if r < 0.25 { vec![(WHEAT_SEEDS, 1)] } else { vec![] }
            }
            GRASS | DIRT | FARMLAND | FARMLAND_WET if r < 0.04 => {
                self.msg("You found a Wiggly Worm. The fish will love it.");
                vec![(BAIT, 1)]
            }
            _ => vec![],
        }
    }

    /// Landing on farmland from a height tramples it.
    pub fn trample(&mut self, under: IVec3, fall: f32) {
        if fall > 1.2 && !self.player.sneaking && is_farmland(self.world.get_v(under)) {
            let above = under + IVec3::Y;
            if Crop::of_block(self.world.get_v(above)).is_some() {
                self.world.set_v(above, AIR);
            }
            self.world.set_v(under, DIRT);
            self.msg("You trampled the farmland. The crops will remember this.");
        }
    }
}
