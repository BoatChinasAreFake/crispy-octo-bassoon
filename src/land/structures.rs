//! Things the world generator builds: mossy dungeons deep underground, ruined
//! towers, huts that are definitely not a village, desert wells, and (on the
//! plains) villages, which definitely are. Each chunk may be home to one,
//! chosen from the seed, so every world (and every player in it) sees the
//! same ones. Their chests are filled from a loot table the first time the
//! chunk loads where the world lives.
//!
//! A **village** is a well on a little square, gravel paths running out from
//! it, a handful of huts along them (each with a Hmmer), a farm or two of
//! watered crops, lamp posts, and a Clanker (legally distinct iron golem)
//! who keeps monsters off the place. It moves in when the square's chest is
//! first filled, like a hut's Hmmer.

use crate::dims::Dim;
use crate::block::*;
use crate::containers::Container;
use crate::inventory::Wear;
use crate::noise::{hash2, hash3, Rng};
use crate::world::{Biome, Generator, World, CH, CW};
use macroquad::math::{ivec3, IVec3};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Kind {
    Dungeon,
    Tower,
    Hut,
    Well,
    Village,
    /// A spire on one of the Hollow's outer islands (see hollow.rs).
    Spire,
    /// Dig sites (see archaeology.rs): buried under desert sand, forest
    /// floors and warm sea beds.
    DesertRuins,
    TrailRuins,
    OceanRuins,
    /// The Hushed Ones' city, deep in the Deep Dark (see deepdark.rs).
    HushedCity,
    /// The Scorchlands' bridges and halls, and the Snouts' camps (see fortress.rs).
    Fortress,
    SnoutCamp,
    /// A Pilferer lookout tower (see raids.rs).
    Outpost,
    /// Copper-and-tuff halls deep underground (see trial.rs).
    TrialChambers,
    /// Wrecks on the sea bed, and chests buried in beaches (see treasure.rs).
    Shipwreck,
    BuriedTreasure,
    /// Temples, mineshafts and igloos (see temples.rs).
    DesertPyramid,
    JungleTemple,
    Mineshaft,
    Igloo,
    /// On the deep sea floor (see monument.rs).
    Monument,
    /// The Snouts' great gilded ruins (see bastion.rs), and (for loot only)
    /// the treasure room in the middle of one.
    Bastion,
    BastionTreasure,
    /// Dark oak mansions in the dark forests (see mansion.rs), and (for loot
    /// only) the chests in their secret rooms.
    Mansion,
    MansionSecret,
    /// A steaming pool in snowy mountains (see springs.rs).
    HotSpring,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Dungeon => "Dungeon (Mossy, Moody)",
            Kind::Tower => "Ruined Tower (Structurally Optimistic)",
            Kind::Hut => "Hut (Definitely Not a Village)",
            Kind::Well => "Wishing Well (Doesn't Grant)",
            Kind::Village => "Village (Actually a Village)",
            Kind::Spire => "Hollow Spire (Loot at the Top of the World)",
            Kind::DesertRuins => "Desert Ruins (Bring a Brush)",
            Kind::TrailRuins => "Trail Ruins (Muddy, Historic)",
            Kind::OceanRuins => "Ocean Ruins (Damp History)",
            Kind::HushedCity => "Hushed City (Tiptoe)",
            Kind::Fortress => "Scorch Fortress (Bring Fire Resistance)",
            Kind::SnoutCamp => "Snout Camp (Gold Accepted)",
            Kind::Outpost => "Pilferer Outpost (Keep Out)",
            Kind::TrialChambers => "Trial Chambers (Bring Keys Back)",
            Kind::Shipwreck => "Shipwreck (Abandoned, Soggy)",
            Kind::BuriedTreasure => "Buried Treasure (X Marks the Spot)",
            Kind::DesertPyramid => "Desert Pyramid (Mind the Floor)",
            Kind::JungleTemple => "Jungle Temple (Mind the String)",
            Kind::Mineshaft => "Abandoned Mineshaft (Mind the Webs)",
            Kind::Igloo => "Igloo (Mind the Basement)",
            Kind::Monument => "Ocean Monument (Mind the Eyes)",
            Kind::Bastion => "Snout Bastion (Gilded, Guarded)",
            Kind::BastionTreasure => "Bastion Treasure Room (Mind the Brute)",
            Kind::Mansion => "Woodland Mansion (Pilferers' Country House)",
            Kind::MansionSecret => "Mansion Secret Room (Shh)",
            Kind::HotSpring => "Hot Spring (Bring a Towel)",
        }
    }

    /// Names /locate understands.
    pub fn from_name(s: &str) -> Option<Kind> {
        Some(match s.to_ascii_lowercase().replace([' ', '-'], "_").as_str() {
            "dungeon" => Kind::Dungeon,
            "tower" | "ruined_tower" => Kind::Tower,
            "hut" => Kind::Hut,
            "well" => Kind::Well,
            "village" => Kind::Village,
            "desert_ruins" | "desert_ruin" => Kind::DesertRuins,
            "trail_ruins" | "trail_ruin" => Kind::TrailRuins,
            "ocean_ruins" | "ocean_ruin" => Kind::OceanRuins,
            "hushed_city" | "city" | "ancient_city" => Kind::HushedCity,
            "fortress" | "scorch_fortress" | "nether_fortress" => Kind::Fortress,
            "snout_camp" | "camp" => Kind::SnoutCamp,
            "bastion" | "snout_bastion" | "bastion_remnant" => Kind::Bastion,
            "outpost" | "pilferer_outpost" | "pillager_outpost" => Kind::Outpost,
            "trial_chambers" | "trial_chamber" | "trials" => Kind::TrialChambers,
            "shipwreck" | "wreck" => Kind::Shipwreck,
            "buried_treasure" | "treasure" => Kind::BuriedTreasure,
            "desert_pyramid" | "pyramid" | "desert_temple" => Kind::DesertPyramid,
            "jungle_temple" | "jungle_pyramid" => Kind::JungleTemple,
            "mineshaft" | "abandoned_mineshaft" => Kind::Mineshaft,
            "igloo" => Kind::Igloo,
            "monument" | "ocean_monument" => Kind::Monument,
            "mansion" | "woodland_mansion" => Kind::Mansion,
            "hot_spring" | "spring" | "hotspring" => Kind::HotSpring,
            _ => return None,
        })
    }

    /// Stands on the surface (so it's settled into the land around it).
    fn on_surface(self) -> bool {
        matches!(self, Kind::Hut | Kind::Tower | Kind::Well | Kind::Outpost | Kind::DesertPyramid | Kind::JungleTemple | Kind::Igloo | Kind::Mansion)
    }

    /// Wide enough that it reaches two chunks out.
    fn wide(self) -> bool {
        matches!(self, Kind::Village | Kind::HushedCity | Kind::Fortress | Kind::TrialChambers | Kind::Mineshaft | Kind::Monument | Kind::Bastion | Kind::Mansion)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Site {
    pub kind: Kind,
    /// Ground level (or the floor, underground) at the middle.
    pub origin: IVec3,
    /// Which way the door faces (0 north .. 3 west).
    pub facing: u8,
    pub seed: u32,
}

impl Generator {
    /// The structure (if any) that starts in chunk (cx, cz).
    /// (In generator coordinates; see dims.rs.)
    pub fn site(&self, cx: i32, cz: i32) -> Option<Site> {
        match self.dim {
            Dim::Scorch => return self.scorch_site(cx, cz),
            Dim::Hollow => return None,
            Dim::Over => {}
        }
        let s = self.seed ^ 0x57_C0DE;
        let village_spot = self.village_spot(cx, cz);
        // New worlds: the surface's sites only start in a structure slot (see
        // `structure_slot`), so no two sit side by side. Older worlds keep the
        // old rolls, chunk by chunk, so they carry on as they were.
        let spaced = self.opts.version >= 1;
        let slot = spaced && self.structure_slot(cx, cz);
        let r = if spaced { hash2(s, cx, cz) * 0.1 } else { hash2(s, cx, cz) };
        // (Villages and the rarer sites roll on their own, below; the rest only here.)
        let common = if spaced { slot } else { r < 0.10 };
        // Trial Chambers: rare, deep, wherever (but never two in reach of each other).
        let trial_spot = self.opts.structures > 0 && (cx.rem_euclid(5), cz.rem_euclid(5)) == (2, 2) && hash2(s ^ 0x7A1, cx.div_euclid(5), cz.div_euclid(5)) < 0.3;
        let (wreck_roll, treasure_roll, outpost_roll) = if spaced {
            (slot && hash2(s ^ 0x5419, cx, cz) < 0.55, slot && hash2(s ^ 0x7EA5, cx, cz) < 0.7, slot && hash2(s ^ 0x0B0, cx, cz) < 0.15)
        } else {
            (hash2(s ^ 0x5419, cx, cz) < 0.035, hash2(s ^ 0x7EA5, cx, cz) < 0.06, hash2(s ^ 0x0B0, cx, cz) < 0.01)
        };
        let city_roll = self.opts.structures > 0 && hash2(s ^ 0xC17, cx, cz) < 0.035;
        // Mineshafts: deep, on a grid of their own (and never where Trial Chambers are).
        let shaft_spot = self.opts.structures > 0 && !trial_spot && (cx.rem_euclid(6), cz.rem_euclid(6)) == (3, 3) && hash2(s ^ 0x5AF7, cx.div_euclid(6), cz.div_euclid(6)) < 0.45;
        // Ocean Monuments: rare, on a grid of their own, on the deep sea floor.
        let monument_spot = self.opts.structures > 0 && (cx.rem_euclid(8), cz.rem_euclid(8)) == (4, 4) && hash2(s ^ 0x30A, cx.div_euclid(8), cz.div_euclid(8)) < 0.6;
        // Woodland Mansions: rarer still, on their own grid, in dark forests (see mansion.rs).
        let mansion_spot = self.mansion_spot(cx, cz);
        // Hot springs: in snowy mountains (see springs.rs).
        let spring_spot = self.spring_spot(cx, cz);
        if !common && !village_spot && !trial_spot && !wreck_roll && !treasure_roll && !outpost_roll && !city_roll && !shaft_spot && !monument_spot && !mansion_spot && !spring_spot {
            return None;
        }
        let ox = cx * CW + 5 + (hash2(s ^ 1, cx, cz) * 6.0) as i32;
        let oz = cz * CW + 5 + (hash2(s ^ 2, cx, cz) * 6.0) as i32;
        let (h, biome) = self.column(ox, oz);
        let seed = (hash2(s ^ 3, cx, cz) * u32::MAX as f32) as u32;
        let facing = (hash2(s ^ 4, cx, cz) * 4.0) as u8 % 4;
        // Surface buildings want ground that's roughly flat.
        let flat = || [(-3, -3), (3, -3), (-3, 3), (3, 3)].iter().all(|&(dx, dz)| (self.column(ox + dx, oz + dz).0 - h).abs() <= 2);
        // Bigger buildings check further out.
        let flat_r = |r: i32, tol: i32| [(-r, -r), (r, -r), (-r, r), (r, r), (0, r), (0, -r), (r, 0), (-r, 0)].iter().all(|&(dx, dz)| (self.column(ox + dx, oz + dz).0 - h).abs() <= tol);
        // Villages: rare, on wide flat plains.
        let wide_flat = || (0..8).all(|k| {
            let a = k as f32 * std::f32::consts::FRAC_PI_4;
            let (dx, dz) = ((a.cos() * 15.0) as i32, (a.sin() * 15.0) as i32);
            let (hh, b) = self.column(ox + dx, oz + dz);
            (hh - h).abs() <= 4 && !b.is_ocean()
        });
        if mansion_spot && let Some(m) = self.mansion_site(ox, oz, h, biome, facing, seed) {
            return Some(m);
        }
        if spring_spot && let Some(m) = self.spring_site(ox, oz, h, biome, facing, seed) {
            return Some(m);
        }
        if village_spot && matches!(biome, Biome::Plains | Biome::Desert | Biome::Taiga | Biome::Snowy) && h > self.sea() + 1 && h < self.sea() + 58 && wide_flat() {
            return Some(Site { kind: Kind::Village, origin: ivec3(ox, h, oz), facing, seed });
        }
        // Pilferers build lookouts on open, flat ground (and, in newer worlds, never within sight of a village).
        let village_near = || self.opts.version >= 2 && (-7..=7).any(|dz| (-7..=7).any(|dx| self.village_spot(cx + dx, cz + dz)));
        if outpost_roll && matches!(biome, Biome::Plains | Biome::Desert | Biome::Taiga | Biome::Snowy) && h > self.sea() + 1 && h < self.sea() + 58 && flat() && flat_r(7, 2) && !village_near() {
            return Some(Site { kind: Kind::Outpost, origin: ivec3(ox, h, oz), facing, seed });
        }
        if trial_spot && h > self.sea() + 4 && !self.deep_dark(ox, oz) {
            return Some(Site { kind: Kind::TrialChambers, origin: ivec3(ox, crate::trial::chamber_y(s, cx, cz), oz), facing, seed });
        }
        if monument_spot && biome.is_ocean() && h <= self.sea() - crate::monument::DEPTH {
            // Deep, open sea all round (it levels its own basin; see monument.rs).
            let r = crate::monument::BASIN;
            let open = [(-r, -r), (r, -r), (-r, r), (r, r), (0, r), (0, -r), (r, 0), (-r, 0)].iter().all(|&(dx, dz)| {
                let (hh, b) = self.column(ox + dx, oz + dz);
                b.is_ocean() && hh < self.sea() - 2 && hh >= h - crate::monument::FILL
            });
            if open {
                return Some(Site { kind: Kind::Monument, origin: ivec3(ox, h, oz), facing, seed });
            }
        }
        if shaft_spot && h > self.sea() + 10 && !biome.is_ocean() {
            let y = 20 + (hash2(s ^ 0x5AF8, cx, cz) * (h - 45).clamp(1, 20) as f32) as i32;
            return Some(Site { kind: Kind::Mineshaft, origin: ivec3(ox, y, oz), facing, seed });
        }
        // The Hushed Ones built rarely, and only in the Deep Dark.
        if city_roll && self.deep_dark(ox, oz) && h > crate::deepdark::DEEP_TOP + 8 {
            return Some(Site { kind: Kind::HushedCity, origin: ivec3(ox, crate::deepdark::CITY_Y, oz), facing, seed });
        }
        // Wrecks lie on the sea bed; treasure is buried in beaches (see treasure.rs).
        if wreck_roll && biome.is_ocean() && h < self.sea() - 7 && h > 6 {
            return Some(Site { kind: Kind::Shipwreck, origin: ivec3(ox, h, oz), facing, seed });
        }
        if treasure_roll && (self.sea() - 1..=self.sea() + 1).contains(&h) && !matches!(biome, Biome::Snowy | Biome::Swamp | Biome::Badlands | Biome::Mangrove) {
            return Some(Site { kind: Kind::BuriedTreasure, origin: ivec3(ox, h - 2, oz), facing, seed });
        }
        if !common {
            return None;
        }
        let kind = if r < 0.055 {
            if h < self.sea() - 10 || biome.is_ocean() {
                return None;
            }
            Kind::Dungeon
        } else if biome == Biome::Jungle && h > self.sea() + 1 && h < self.sea() + 58 && r < 0.085 && flat_r(6, 3) {
            // (Rolls nothing else took, so older worlds' sites stay put.)
            Kind::JungleTemple
        } else if biome == Biome::Snowy && h > self.sea() + 1 && r >= 0.085 && flat_r(5, 2) {
            Kind::Igloo
        } else if biome == Biome::Desert && h > self.sea() + 1 && h < self.sea() + 68 && (0.075..0.09).contains(&r) && flat() && flat_r(7, 2) {
            Kind::DesertPyramid
        } else if r < 0.057 && matches!(biome, Biome::Plains | Biome::Forest) && h > self.sea() + 1 && flat() {
            // A lone hut now and then (villages are where the houses are).
            Kind::Hut
        } else if r < 0.085 && matches!(biome, Biome::Plains | Biome::Forest | Biome::Snowy) && h > self.sea() + 1 && h < self.sea() + 68 && flat() {
            Kind::Tower
        } else if biome == Biome::Desert && h > self.sea() + 1 && r < 0.075 {
            Kind::DesertRuins
        } else if biome == Biome::Desert && h > self.sea() + 1 && flat() {
            Kind::Well
        } else if matches!(biome, Biome::Forest | Biome::Taiga | Biome::Jungle | Biome::Plains) && h > self.sea() + 1 && r >= 0.085 && flat() {
            Kind::TrailRuins
        } else if biome.is_ocean() && h < self.sea() - 4 && h > 8 && r < 0.09 {
            Kind::OceanRuins
        } else {
            return None;
        };
        let y = match kind {
            // Well below the surface, well above bedrock.
            Kind::Dungeon => 10 + (hash2(s ^ 5, cx, cz) * (h - 24).max(1) as f32) as i32,
            _ => h,
        };
        Some(Site { kind, origin: ivec3(ox, y, oz), facing, seed })
    }

    /// Villages: at most one per 6x6-chunk region, at a spot away from its
    /// edges (so neighbours never overlap), in most regions (fewer, or none,
    /// if the world was made that way).
    pub fn village_spot(&self, cx: i32, cz: i32) -> bool {
        let s = self.seed ^ 0x57_C0DE;
        // Newer worlds space them out more: one region in ten chunks, not six.
        let (size, spread) = if self.opts.version >= 2 { (10, 4) } else { (6, 2) };
        let (rx, rz) = (cx.div_euclid(size), cz.div_euclid(size));
        let chance = match self.opts.version {
            0 => 0.85,
            1 => [0.0, 0.45, 0.85, 0.95][self.opts.structures as usize % 4],
            _ => [0.0, 0.3, 0.55, 0.8][self.opts.structures as usize % 4],
        };
        let at = |r: i32, salt: u32| r * size + if size == 6 { 2 + (hash2(s ^ salt, rx, rz) * 2.0) as i32 % 2 } else { 3 + (hash2(s ^ salt, rx, rz) * spread as f32) as i32 % spread };
        cx == at(rx, 0xA11) && cz == at(rz, 0xA12) && hash2(s ^ 0x7111, rx, rz) < chance
    }

    /// New worlds' structure slots: one chunk in the middle of each 4x4-chunk
    /// region may hold a site (so any two are at least two chunks apart), if
    /// the region rolls one and no village is within two chunks.
    pub fn structure_slot(&self, cx: i32, cz: i32) -> bool {
        let s = self.seed ^ 0x5107;
        let (gx, gz) = (cx.div_euclid(4), cz.div_euclid(4));
        let (sx, sz) = (gx * 4 + 1 + (hash2(s, gx, gz) * 2.0) as i32 % 2, gz * 4 + 1 + (hash2(s ^ 1, gx, gz) * 2.0) as i32 % 2);
        let chance = [0.0, 0.35, 0.7, 1.0][self.opts.structures as usize % 4];
        (cx, cz) == (sx, sz) && hash2(s ^ 2, gx, gz) < chance && !(-2..=2).any(|dz| (-2..=2).any(|dx| self.village_spot(cx + dx, cz + dz)))
    }

    /// Every block a site places, in world coordinates (AIR clears space).
    pub fn site_blocks(&self, site: &Site) -> Vec<(IVec3, Id)> {
        let mut out = Vec::new();
        let o = site.origin;
        let s = site.seed;
        // Local (x, z) with the door on the south side, turned to face `facing`.
        let turn = |x: i32, z: i32| -> (i32, i32) {
            let (mut x, mut z) = (x, z);
            for _ in 0..(site.facing + 2) % 4 {
                (x, z) = (-z, x);
            }
            (x, z)
        };
        let mut put = |x: i32, y: i32, z: i32, id: Id| {
            let (tx, tz) = turn(x, z);
            out.push((ivec3(o.x + tx, o.y + y, o.z + tz), id));
        };
        let door_facing = site.facing;
        match site.kind {
            // Built by the Hollow's own generator.
            Kind::Spire => {}
            Kind::DesertRuins | Kind::TrailRuins | Kind::OceanRuins => return ruin_blocks(site),
            Kind::HushedCity => return crate::deepdark::city_blocks(site),
            Kind::Fortress => return crate::fortress::fortress_blocks(site, self.opts.version >= 2),
            Kind::SnoutCamp => return crate::fortress::camp_blocks(site),
            Kind::Bastion => return crate::bastion::bastion_blocks(site),
            // (Never a site of its own: just a bastion's best chest.)
            Kind::BastionTreasure => return Vec::new(),
            Kind::Mansion => return crate::mansion::mansion_blocks(site),
            Kind::MansionSecret => return Vec::new(),
            Kind::HotSpring => return crate::springs::spring_blocks(site),
            Kind::Outpost => return crate::raids::outpost_blocks(site, self.opts.version >= 2),
            Kind::TrialChambers => return crate::trial::chamber_blocks(site.origin, site.seed),
            Kind::Shipwreck => return crate::treasure::shipwreck_blocks(site, self.sea()),
            Kind::BuriedTreasure => return crate::treasure::treasure_blocks(site),
            Kind::DesertPyramid => return crate::temples::pyramid_blocks(site),
            Kind::JungleTemple => return crate::temples::jungle_temple_blocks(site),
            Kind::Mineshaft => return crate::temples::mineshaft_blocks(site),
            Kind::Igloo => return crate::temples::igloo_blocks(site),
            Kind::Monument => return crate::monument::monument_blocks(site, self.sea()),
            Kind::Dungeon => {
                for x in -4..=4i32 {
                    for z in -4..=4i32 {
                        for y in 0..=5 {
                            let shell = x.abs() == 4 || z.abs() == 4 || y == 0 || y == 5;
                            let id = if !shell {
                                AIR
                            } else if hash3(s, x, y, z) < 0.45 {
                                MOSSY_COBBLE
                            } else {
                                COBBLE
                            };
                            put(x, y, z, id);
                        }
                    }
                }
                put(-3, 1, 0, CHEST);
                if hash2(s, 1, 2) < 0.5 {
                    put(3, 1, 1, CHEST);
                }
                // Whatever lives here keeps coming out of the cage in the middle.
                put(0, 1, 0, SPAWNER);
                put(3, 1, -3, GLOWSHROOM); // something to see by
            }
            Kind::Tower => {
                let height = 7 + (hash2(s, 7, 7) * 4.0) as i32;
                for x in -3..=3 {
                    for z in -3..=3 {
                        for y in -3..=0 {
                            put(x, y, z, if y == 0 { COBBLE } else { STONE });
                        }
                        for y in 1..=height + 1 {
                            put(x, y, z, AIR);
                        }
                    }
                }
                for x in -2..=2i32 {
                    for z in -2..=2i32 {
                        let wall = x.abs() == 2 || z.abs() == 2;
                        if !wall {
                            continue;
                        }
                        for y in 1..=height {
                            // Crumbles more toward the top.
                            let keep = 1.0 - (y as f32 / height as f32).powi(3) * 0.85;
                            if hash3(s, x, y, z) > keep {
                                continue;
                            }
                            let doorway = x == 0 && z == 2 && y <= 2;
                            if !doorway {
                                put(x, y, z, if hash3(s ^ 9, x, y, z) < 0.35 { MOSSY_COBBLE } else { STONE_BRICKS });
                            }
                        }
                    }
                }
                // A floor halfway up, mostly fallen in.
                for x in -1..=1 {
                    for z in -1..=1 {
                        if hash3(s ^ 11, x, 4, z) < 0.5 {
                            put(x, 4, z, slab(2, false));
                        }
                    }
                }
                put(-1, 1, -1, CHEST);
            }
            Kind::Hut => {
                for x in -3..=3i32 {
                    for z in -3..=3i32 {
                        for y in -3..=0 {
                            put(x, y, z, if y == 0 { PLANKS } else { COBBLE });
                        }
                        for y in 1..=6 {
                            put(x, y, z, AIR);
                        }
                        let (ex, ez) = (x.abs() == 3, z.abs() == 3);
                        for y in 1..=3 {
                            if ex && ez {
                                put(x, y, z, LOG);
                            } else if ex || ez {
                                let window = y == 2 && (x == 0 || z == 0) && !(z == 3 && x == 0);
                                put(x, y, z, if window { GLASS } else { PLANKS });
                            }
                        }
                        put(x, 4, z, slab(0, false));
                    }
                }
                put(0, 1, 3, door(door_facing, false, false));
                put(0, 2, 3, door(door_facing, false, true));
                put(-2, 1, -2, TABLE);
                put(2, 1, -2, CHEST);
                put(-2, 1, 2, BED);
                put(2, 1, 2, TORCH);
                put(2, 1, -1, FURNACE);
            }
            Kind::Village => return self.village_blocks(site),
            Kind::Well => {
                for x in -2..=2i32 {
                    for z in -2..=2i32 {
                        for y in 1..=5 {
                            put(x, y, z, AIR);
                        }
                        put(x, 0, z, SANDSTONE);
                        if x.abs() <= 1 && z.abs() <= 1 {
                            put(x, 4, z, SANDSTONE);
                            if x.abs() == 1 && z.abs() == 1 {
                                put(x, 2, z, SANDSTONE);
                                put(x, 3, z, SANDSTONE);
                            }
                            if (x == 0) != (z == 0) {
                                put(x, 1, z, SANDSTONE);
                            }
                        }
                    }
                }
                for y in -3..=0 {
                    put(0, y, 0, WATER);
                }
                put(0, -4, 0, CHEST);
            }
        }
        out
    }

    /// A village (see the top of this file). Houses and farms follow the
    /// lie of the land; everything else sits at the square's level.
    fn village_blocks(&self, site: &Site) -> Vec<(IVec3, Id)> {
        let o = site.origin;
        let s = site.seed;
        let mut out = Vec::new();
        let ground = |x: i32, z: i32| self.column(o.x + x, o.z + z).0;
        // Clear a patch of sky (trees, grass) over somewhere built on.
        let clear = |out: &mut Vec<(IVec3, Id)>, x: i32, z: i32, from: i32| {
            for y in from..from + 8 {
                out.push((ivec3(o.x + x, y, o.z + z), AIR));
            }
        };
        // Gravel paths out from the square, along the ground.
        for k in -18..=18i32 {
            for w in -1..=1i32 {
                for (x, z) in [(k, w), (w, k)] {
                    if x.abs() <= 3 && z.abs() <= 3 {
                        continue;
                    }
                    let g = ground(x, z);
                    out.push((ivec3(o.x + x, g, o.z + z), GRAVEL));
                    out.push((ivec3(o.x + x, g - 1, o.z + z), DIRT));
                    clear(&mut out, x, z, g + 1);
                }
            }
        }
        // The square: cobblestone round a well.
        for x in -3..=3i32 {
            for z in -3..=3i32 {
                for y in -3..0 {
                    out.push((ivec3(o.x + x, o.y + y, o.z + z), if y == -3 { COBBLE } else { DIRT }));
                }
                out.push((ivec3(o.x + x, o.y, o.z + z), COBBLE));
                clear(&mut out, x, z, o.y + 1);
            }
        }
        for x in -1..=1i32 {
            for z in -1..=1i32 {
                if x == 0 && z == 0 {
                    for y in -2..=0 {
                        out.push((ivec3(o.x, o.y + y, o.z), WATER));
                    }
                } else {
                    out.push((ivec3(o.x + x, o.y + 1, o.z + z), COBBLE));
                }
            }
        }
        // Lamp posts at the square's corners.
        for (x, z) in [(3, 3), (-3, -3), (3, -3), (-3, 3)] {
            out.push((ivec3(o.x + x, o.y + 1, o.z + z), FENCE_FIRST));
            out.push((ivec3(o.x + x, o.y + 2, o.z + z), FENCE_FIRST));
            out.push((ivec3(o.x + x, o.y + 3, o.z + z), TORCH));
        }
        out.push((village_chest(site), CHEST));
        // The bell, on a post by the well (see raids.rs).
        out.push((ivec3(o.x - 2, o.y + 1, o.z + 2), FENCE_FIRST));
        out.push((ivec3(o.x - 2, o.y + 2, o.z + 2), BELL));
        // Houses beside the paths, doors to the path. Newer worlds have proper houses (see houses.rs).
        if self.opts.version >= 2 {
            for (at, facing, seed) in self.village_houses(site) {
                out.extend(crate::houses::house_blocks(at, facing, seed));
            }
        }
        for (i, &(x, z, facing)) in [(9, -6, 2u8), (-9, -6, 2), (9, 6, 0), (6, 13, 3), (-6, -13, 1), (6, -14, 3)].iter().enumerate() {
            if self.opts.version >= 2 || i >= 3 && hash2(s ^ 0x4053, i as i32, 0) < 0.35 {
                continue;
            }
            let g = ground(x, z);
            let hut = Site { kind: Kind::Hut, origin: ivec3(o.x + x, g, o.z + z), facing, seed: s.wrapping_add(i as u32 * 7919) };
            // Level ground under it first.
            for dx in -3..=3 {
                for dz in -3..=3 {
                    for y in g - 4..g - 3 {
                        out.push((ivec3(o.x + x + dx, y, o.z + z + dz), DIRT));
                    }
                    clear(&mut out, x + dx, z + dz, g + 1);
                }
            }
            out.extend(self.site_blocks(&hut));
        }
        // Farms: rows of crops either side of a water channel, fenced with logs.
        for (k, &(x0, z0)) in [(-10, 7), (-6, 15)].iter().enumerate() {
            if k == 1 && hash2(s ^ 0xFA4, 0, 0) < 0.4 {
                continue;
            }
            let g = ground(x0, z0);
            for dx in -4..=4i32 {
                for dz in -3..=3i32 {
                    let (x, z) = (x0 + dx, z0 + dz);
                    let edge = dx.abs() == 4 || dz.abs() == 3;
                    out.push((ivec3(o.x + x, g - 1, o.z + z), DIRT));
                    clear(&mut out, x, z, g + 1);
                    let (block, top) = if edge {
                        (LOG, AIR)
                    } else if dz == 0 {
                        (WATER, AIR)
                    } else {
                        // Each farm grows one crop, at whatever stage.
                        let crop = [WHEAT_0, CARROT_0, POTATO_0][(hash2(s ^ 0xC40, k as i32, 0) * 3.0) as usize % 3];
                        let stage = (hash3(s ^ 0x57A, x, 0, z) * 4.0) as Id % 4;
                        (FARMLAND_WET, crop + stage)
                    };
                    out.push((ivec3(o.x + x, g, o.z + z), block));
                    out.push((ivec3(o.x + x, g + 1, o.z + z), top));
                }
            }
        }
        out
    }

    /// A newer village's houses: where each stands (its ground floor), which
    /// way its door faces (0 north .. 3 west), and its seed. Spaced so that
    /// their roofs (a block over a 7×7 footprint) never meet.
    pub fn village_houses(&self, site: &Site) -> Vec<(IVec3, u8, u32)> {
        let (o, s) = (site.origin, site.seed);
        [(9, -6, 2u8), (-9, -6, 2), (9, 6, 0), (6, 15, 3), (-6, -15, 1), (6, -15, 3)]
            .iter()
            .enumerate()
            .filter(|&(i, _)| i < 3 || hash2(s ^ 0x4053, i as i32, 0) >= 0.35)
            .map(|(i, &(x, z, facing))| (ivec3(o.x + x, self.column(o.x + x, o.z + z).0, o.z + z), facing, s.wrapping_add(i as u32 * 7919)))
            .collect()
    }

    /// (The next few are in generator coordinates; `World` has versions in a
    /// dimension's own, see dims.rs.)
    /// Where a village's farmland is (so it can be given soil when it first loads).
    pub fn village_farmland(&self, site: &Site) -> Vec<IVec3> {
        self.village_blocks(site).into_iter().filter(|b| b.1 == FARMLAND_WET).map(|b| b.0).collect()
    }

    /// Stamp every structure that reaches into chunk (cx, cz) onto its blocks.
    /// (Villages reach two chunks out; everything else, one.)
    pub fn place_structures(&self, cx: i32, cz: i32, b: &mut [Id]) {
        let reach = self.site_reach();
        for dz in -reach..=reach {
            for dx in -reach..=reach {
                let Some(site) = self.site(cx + dx, cz + dz) else { continue };
                if (dx.abs() >= 2 || dz.abs() >= 2) && !site.kind.wide() {
                    continue;
                }
                let blocks = self.site_blocks(&site);
                // In newer worlds, buildings on the surface settle into the land (see `settle`).
                if self.opts.version >= 2 && site.kind.on_surface() {
                    self.settle(site.origin, &blocks, cx, cz, b);
                }
                if self.opts.version >= 2 && site.kind == Kind::Village {
                    for (at, facing, seed) in self.village_houses(&site) {
                        self.settle(at, &crate::houses::house_blocks(at, facing, seed), cx, cz, b);
                    }
                }
                for (p, id) in blocks {
                    let (lx, lz) = (p.x - cx * CW, p.z - cz * CW);
                    if (0..CW).contains(&lx) && (0..CW).contains(&lz) && (1..CH).contains(&p.y) {
                        b[crate::world::idx(lx, p.y, lz)] = id;
                    }
                }
            }
        }
    }

    /// Chests of structures that sit in chunk (cx, cz), with what kind of place they're in.
    /// (Chunk (cx, cz) and what's returned are in the dimension's own
    /// coordinates; the sites are found in generator coordinates, see dims.rs.)
    pub fn structure_chests(&self, cx: i32, cz: i32) -> Vec<(IVec3, Kind, u32)> {
        let (cx, back) = (cx + self.dim.gen_cx(), ivec3(self.dim.gen_x(), 0, 0));
        if self.dim == Dim::Hollow {
            return crate::hollow::spire_chests(self.seed, cx, cz).into_iter().map(|(p, s)| (p - back, Kind::Spire, s)).collect();
        }
        let mut v = Vec::new();
        for (site, blocks) in self.sites_near(cx, cz) {
            for (p, id) in blocks {
                if id == CHEST && p.x.div_euclid(CW) == cx && p.z.div_euclid(CW) == cz {
                    // A village's houses have hut chests; only the square's is the village's.
                    let kind = if site.kind == Kind::Village && p != village_chest(&site) {
                        Kind::Hut
                    } else if site.kind == Kind::Bastion && p == crate::bastion::treasure_chest(&site) {
                        Kind::BastionTreasure
                    } else if site.kind == Kind::Mansion && crate::mansion::secret_chests(&site).contains(&p) {
                        Kind::MansionSecret
                    } else {
                        site.kind
                    };
                    v.push((p - back, kind, site.seed ^ (p.x as u32).wrapping_mul(31) ^ (p.y as u32).wrapping_mul(17) ^ p.z as u32));
                }
            }
        }
        // The Crypts' side rooms (see hollow.rs).
        v.extend(self.crypt_chests(cx, cz).into_iter().map(|p| (p, Kind::Dungeon, self.seed ^ (p.x as u32).wrapping_mul(31) ^ (p.z as u32).wrapping_mul(7))));
        v
    }

    /// Dispensers built into chunk (cx, cz) (jungle temples' traps).
    pub fn structure_dispensers(&self, cx: i32, cz: i32) -> Vec<IVec3> {
        let (cx, back) = (cx + self.dim.gen_cx(), ivec3(self.dim.gen_x(), 0, 0));
        let mut v = Vec::new();
        for (site, blocks) in self.sites_near(cx, cz) {
            if site.kind == Kind::JungleTemple {
                v.extend(blocks.into_iter().filter(|(p, id)| crate::contraptions::is_dispenser(*id) && p.x.div_euclid(CW) == cx && p.z.div_euclid(CW) == cz).map(|(p, _)| p - back));
            }
        }
        v
    }

    /// Monster and Sizzler Cages built into chunk (cx, cz) (see fortress.rs).
    pub fn structure_cages(&self, cx: i32, cz: i32) -> Vec<IVec3> {
        let (cx, back) = (cx + self.dim.gen_cx(), ivec3(self.dim.gen_x(), 0, 0));
        let mut v = Vec::new();
        for (_, blocks) in self.sites_near(cx, cz) {
            v.extend(blocks.into_iter().filter(|(p, id)| crate::fortress::is_cage(*id) && p.x.div_euclid(CW) == cx && p.z.div_euclid(CW) == cz).map(|(p, _)| p - back));
        }
        v
    }

    /// Sites that can reach into chunk (cx, cz), with their blocks.
    /// Settle a building into the land (newer worlds): fill in under it down
    /// to the ground, and over a few blocks round it slope the land to meet it,
    /// building up where it's low and cutting back where it's high, so nothing
    /// stands on stilts of air or sits in a sheer-sided hole. `base` is the
    /// building's ground floor (`origin.y`).
    fn settle(&self, origin: IVec3, blocks: &[(IVec3, Id)], cx: i32, cz: i32, b: &mut [Id]) {
        const R: i32 = 4;
        let base = origin.y;
        // The footprint: everything it builds at ground level.
        let (mut x0, mut x1, mut z0, mut z1) = (i32::MAX, i32::MIN, i32::MAX, i32::MIN);
        for (p, id) in blocks {
            if *id != AIR && (base - 1..=base + 3).contains(&p.y) {
                (x0, x1, z0, z1) = (x0.min(p.x), x1.max(p.x), z0.min(p.z), z1.max(p.z));
            }
        }
        if x0 > x1 || x1 + R < cx * CW || x0 - R >= cx * CW + CW || z1 + R < cz * CW || z0 - R >= cz * CW + CW {
            return;
        }
        let ground_like = |id: Id| is_solid(id) && !is_leaves(id) && !is_log(id) && !is_liquid(id);
        for lz in 0..CW {
            for lx in 0..CW {
                let (x, z) = (cx * CW + lx, cz * CW + lz);
                let d = (x0 - x).max(x - x1).max(0).max((z0 - z).max(z - z1).max(0));
                if d > R {
                    continue;
                }
                let at = |y: i32| crate::world::idx(lx, y, lz);
                let top = (1..(base + R + 10).min(CH - 2)).rev().find(|&y| ground_like(b[at(y)])).unwrap_or(1);
                let (cover, fill) = match self.column(x, z).1 {
                    Biome::Desert | Biome::Badlands => (SAND, SAND),
                    Biome::Snowy | Biome::IceSpikes => (SNOW_GRASS, DIRT),
                    _ => (GRASS, DIRT),
                };
                if d == 0 {
                    // Under the building: no gaps down to the ground.
                    for y in (top + 1..base).rev() {
                        if !ground_like(b[at(y)]) {
                            b[at(y)] = if y < base - 3 { STONE } else { fill };
                        }
                    }
                    continue;
                }
                let (lo, hi) = (base - d, base + d);
                if top < lo {
                    for y in top + 1..lo {
                        b[at(y)] = fill;
                    }
                    b[at(lo)] = cover;
                } else if top > hi && hi > 1 {
                    for y in hi + 1..=top {
                        b[at(y)] = AIR;
                    }
                    if ground_like(b[at(hi)]) {
                        b[at(hi)] = cover;
                    }
                }
            }
        }
    }

    /// How many chunks out a site can reach into its neighbours. (Newer
    /// worlds' Scorchlands fortresses are bigger: three chunks.)
    fn site_reach(&self) -> i32 {
        if self.opts.version >= 2 && self.dim == Dim::Scorch { 3 } else { 2 }
    }

    fn sites_near(&self, cx: i32, cz: i32) -> Vec<(Site, Vec<(IVec3, Id)>)> {
        let mut v = Vec::new();
        let reach = self.site_reach();
        for dz in -reach..=reach {
            for dx in -reach..=reach {
                let Some(site) = self.site(cx + dx, cz + dz) else { continue };
                if (dx.abs() >= 2 || dz.abs() >= 2) && !site.kind.wide() {
                    continue;
                }
                v.push((site, self.site_blocks(&site)));
            }
        }
        v
    }

    /// The nearest `kind` of site to `from`, searching rings of chunks out to `radius`.
    pub fn nearest_site(&self, kind: Kind, from: macroquad::math::Vec3, radius: i32) -> Option<IVec3> {
        let (cx, cz) = ((from.x as i32).div_euclid(CW), (from.z as i32).div_euclid(CW));
        for r in 0..=radius {
            let mut best: Option<(f32, IVec3)> = None;
            for dz in -r..=r {
                for dx in -r..=r {
                    if dx.abs().max(dz.abs()) != r {
                        continue;
                    }
                    if let Some(site) = self.site(cx + dx, cz + dz).filter(|s| s.kind == kind) {
                        let d = site.origin.as_vec3().distance(from);
                        if best.is_none_or(|(b, _)| d < b) {
                            best = Some((d, site.origin));
                        }
                    }
                }
            }
            if let Some((_, p)) = best {
                return Some(p);
            }
        }
        None
    }

    /// The kind of site whose middle is within `r` of `p`, if any.
    pub fn site_near(&self, p: macroquad::math::Vec3, r: f32) -> Option<Kind> {
        let (cx, cz) = ((p.x as i32).div_euclid(CW), (p.z as i32).div_euclid(CW));
        (-1..=1).flat_map(|dz| (-1..=1).map(move |dx| (dx, dz))).filter_map(|(dx, dz)| self.site(cx + dx, cz + dz)).find(|s| s.origin.as_vec3().distance(p) < r).map(|s| s.kind)
    }

    /// Villages that reach into chunk (cx, cz).
    pub fn villages_near(&self, cx: i32, cz: i32) -> Vec<Site> {
        let mut v = Vec::new();
        for dz in -2..=2 {
            for dx in -2..=2 {
                if let Some(site) = self.site(cx + dx, cz + dz).filter(|s| s.kind == Kind::Village) {
                    v.push(site);
                }
            }
        }
        v
    }
}

/// Where a village keeps its chest: on the square, by the well.
pub fn village_chest(site: &Site) -> IVec3 {
    site.origin + ivec3(2, 1, 2)
}

/// What a structure's chest holds.
pub fn loot(kind: Kind, seed: u32) -> Container {
    let mut rng = Rng::new(seed as u64 | 1);
    let mut c = Container::for_block(CHEST);
    // (item, most, chance)
    let table: &[(Id, u8, f32)] = match kind {
        Kind::Dungeon => &[
            (BREAD, 3, 0.6),
            (IRON, 5, 0.6),
            (COAL, 10, 0.6),
            (GOLD_INGOT, 4, 0.4),
            (DIAMOND, 2, 0.2),
            (STRING, 5, 0.5),
            (BONE, 5, 0.5),
            (GUNPOWDER, 4, 0.4),
            (PEARL, 1, 0.1),
            (ARMOR_FIRST + 4 + CHESTPLATE as Id, 1, 0.12),
            (PICK_IRON, 1, 0.15),
            (SWORD_IRON, 1, 0.12),
            (ANVIL, 1, 0.05),
        ],
        Kind::Tower => &[(ARROW, 12, 0.6), (BOW, 1, 0.3), (IRON, 4, 0.5), (BREAD, 2, 0.4), (DIAMOND, 1, 0.1), (ARMOR_FIRST + 4, 1, 0.2), (ARMOR_FIRST + 4 + BOOTS as Id, 1, 0.2), (STONE_BRICKS, 16, 0.3), (PEARL, 2, 0.15)],
        Kind::Hut => &[(BREAD, 4, 0.7), (WHEAT_SEEDS, 8, 0.6), (CARROT, 4, 0.4), (POTATO, 4, 0.4), (TORCH, 8, 0.6), (HOE, 1, 0.3), (COOKED_CHOP, 3, 0.3), (PLANKS, 16, 0.4), (ROD, 1, 0.2), (BOOKSHELF, 1, 0.1)],
        Kind::Well => &[(BOOT, 1, 0.8), (GOLD_INGOT, 6, 0.5), (BOTTLE, 1, 0.6), (DIAMOND, 1, 0.15)],
        Kind::Village => &[(BREAD, 6, 0.8), (APPLE, 4, 0.6), (IRON, 4, 0.5), (GOLD_INGOT, 5, 0.5), (BOOK, 2, 0.4), (SADDLE, 1, 0.2), (NAME_TAG, 1, 0.25), (WHEAT_SEEDS, 12, 0.5), (TORCH, 12, 0.5), (HORSE_ARMOR_IRON, 1, 0.1)],
        Kind::Spire => &[
            (GLIDER, 1, 0.45),
            (ROCKET, 16, 0.7),
            (HOLLOW_BOX, 1, 0.5),
            (DIAMOND, 4, 0.5),
            (STARING_EYE, 3, 0.4),
            (PICK_DIAMOND, 1, 0.25),
            (SWORD_DIAMOND, 1, 0.25),
            (ARMOR_FIRST + 12 + CHESTPLATE as Id, 1, 0.2),
            (GOLDEN_CHOP, 1, 0.2),
            (OBSIDIAN, 8, 0.3),
        ],
        // Ruins have no chests: their treasure is in the ground.
        Kind::DesertRuins | Kind::TrailRuins | Kind::OceanRuins => &[(ANCIENT_COIN, 4, 0.8)],
        Kind::HushedCity => &[
            (ECHO_SHARD, 3, 0.6),
            (UPGRADE_TEMPLATE, 1, 0.3),
            (ENCHANTED_BOOK, 1, 0.35),
            (DIAMOND, 2, 0.3),
            (SCULK_SENSOR, 2, 0.3),
            (SOUL_LANTERN, 3, 0.4),
            (BONE, 6, 0.5),
            (COAL, 8, 0.5),
            (GOLDEN_CHOP, 1, 0.2),
            (ARMOR_FIRST + 8 + LEGGINGS as Id, 1, 0.2),
            (RECOVERY_COMPASS, 1, 0.08),
            (DIAMOND_BRUSH, 1, 0.1),
        ],
        Kind::Fortress => &[
            (GOLD_INGOT, 6, 0.6),
            (IRON, 5, 0.5),
            (DIAMOND, 3, 0.25),
            (SADDLE, 1, 0.3),
            (OBSIDIAN, 4, 0.3),
            (EMBER_SHROOM, 6, 0.5),
            (SPARKER, 1, 0.3),
            (SIZZLE_ROD, 2, 0.3),
            (ARMOR_FIRST + 8 + CHESTPLATE as Id, 1, 0.15),
            (SCORCHITE_SCRAP, 1, 0.08),
            (DRIED_FLOATY, 1, 0.12),
            (HORSE_ARMOR_GOLD, 1, 0.15),
            (HORSE_ARMOR_DIAMOND, 1, 0.06),
        ],
        Kind::Outpost => &[
            (CROSSBOW, 1, 0.5),
            (ARROW, 12, 0.7),
            (IRON, 4, 0.5),
            (WHEAT, 6, 0.5),
            (CARROT, 4, 0.4),
            (POTATO, 4, 0.4),
            (ENCHANTED_BOOK, 1, 0.25),
            (SPRUCE_LOG, 8, 0.4),
            (BOTTLE, 2, 0.3),
        ],
        Kind::TrialChambers => &[
            (WIND_CHARGE, 4, 0.5),
            (ARROW, 10, 0.5),
            (BREAD, 4, 0.5),
            (IRON, 4, 0.4),
            (COPPER_INGOT, 6, 0.5),
            (TRIAL_KEY, 1, 0.25),
            (TRIM_FIRST, 1, 0.12),
            (TRIM_FIRST + 1, 1, 0.12),
            (CROSSBOW, 1, 0.2),
            (GOLDEN_CHOP, 1, 0.08),
        ],
        Kind::Shipwreck => &[
            (TREASURE_MAP, 1, 0.6),
            (BREAD, 3, 0.5),
            (CARROT, 4, 0.4),
            (POTATO, 5, 0.4),
            (COAL, 6, 0.5),
            (BOOK, 1, 0.3),
            (IRON, 4, 0.4),
            (GOLD_INGOT, 3, 0.3),
            (COMPASS, 1, 0.15),
            (ARMOR_FIRST + CHESTPLATE as Id, 1, 0.2),
            (TNT, 2, 0.1),
        ],
        Kind::BuriedTreasure => &[
            (HEART_OF_THE_SEA, 1, 1.0),
            (GOLD_INGOT, 8, 1.0),
            (IRON, 6, 0.8),
            (DIAMOND, 2, 0.5),
            (COOKED_COD, 4, 0.6),
            (TNT, 2, 0.3),
            (ENCHANTED_BOOK, 1, 0.3),
            (PEARL, 1, 0.2),
            (TURTLE_SHELL, 1, 0.12),
            (SPEAR, 1, 0.1),
        ],
        Kind::DesertPyramid => &[
            (BONE, 6, 0.6),
            (GOO, 7, 0.6),
            (GOLD_INGOT, 5, 0.5),
            (IRON, 4, 0.5),
            (DIAMOND, 2, 0.25),
            (ENCHANTED_BOOK, 1, 0.3),
            (SADDLE, 1, 0.25),
            (GOLDEN_CHOP, 1, 0.2),
            (GUNPOWDER, 4, 0.4),
            (SAND, 8, 0.3),
            (HORSE_ARMOR_IRON, 1, 0.12),
            (HORSE_ARMOR_GOLD, 1, 0.08),
            (HORSE_ARMOR_DIAMOND, 1, 0.04),
        ],
        Kind::JungleTemple => &[(BONE, 6, 0.6), (GOO, 6, 0.5), (GOLD_INGOT, 6, 0.5), (IRON, 5, 0.5), (DIAMOND, 2, 0.25), (SADDLE, 1, 0.25), (ENCHANTED_BOOK, 1, 0.25), (BAMBOO, 8, 0.4), (ARROW, 8, 0.4), (HORSE_ARMOR_IRON, 1, 0.1), (HORSE_ARMOR_GOLD, 1, 0.06)],
        Kind::Mineshaft => &[
            (RAIL_FIRST, 12, 0.6),
            (TORCH, 12, 0.5),
            (BREAD, 3, 0.5),
            (COAL, 8, 0.5),
            (IRON, 5, 0.5),
            (GOLD_INGOT, 3, 0.3),
            (ZAP_DUST, 6, 0.3),
            (DIAMOND, 2, 0.15),
            (POWERED_RAIL, 4, 0.2),
            (PICK_IRON, 1, 0.1),
            (GLOW_BERRIES, 4, 0.25),
            (ENCHANTED_BOOK, 1, 0.1),
        ],
        Kind::Monument => &[
            (PRISMARINE_CRYSTALS, 6, 0.7),
            (GOLD_INGOT, 6, 0.6),
            (NAUTILUS_SHELL, 3, 0.6),
            (DIAMOND, 2, 0.3),
            (SPONGE, 2, 0.4),
            (ENCHANTED_BOOK, 1, 0.3),
            (HEART_OF_THE_SEA, 1, 0.15),
            (PRISMARINE_SHARD, 8, 0.5),
        ],
        Kind::Igloo => &[(GOLDEN_CHOP, 1, 1.0), (COAL, 4, 0.6), (APPLE, 3, 0.5), (BREAD, 2, 0.5), (WHEAT, 4, 0.3), (GOLD_INGOT, 2, 0.3), (SWORD_STONE, 1, 0.2)],
        Kind::Bastion => &[
            (GOLD_INGOT, 12, 0.8),
            (GOLD_BLOCK, 2, 0.3),
            (CRIMSON_FUNGUS, 6, 0.4),
            (ARROW, 12, 0.4),
            (CROSSBOW, 1, 0.3),
            (IRON, 6, 0.4),
            (MAGMA_CREAM, 4, 0.3),
            (SCORCHITE_SCRAP, 1, 0.15),
            (UPGRADE_TEMPLATE, 1, 0.12),
            (GOLDEN_CHOP, 1, 0.2),
            (DISC_FIRST + 7, 1, 0.08),
        ],
        Kind::Mansion => &[
            (BREAD, 4, 0.5),
            (WHEAT, 6, 0.4),
            (IRON, 4, 0.4),
            (GOLD_INGOT, 3, 0.3),
            (ARROW, 12, 0.4),
            (CROSSBOW, 1, 0.25),
            (BOOK, 3, 0.3),
            (LEAD, 2, 0.3),
            (NAME_TAG, 1, 0.2),
            (ENCHANTED_BOOK, 1, 0.15),
            (HORSE_ARMOR_IRON, 1, 0.1),
            (DISC_FIRST + 2, 1, 0.08),
        ],
        // (No chests at a spring.)
        Kind::HotSpring => &[],
        Kind::MansionSecret => &[
            (DIAMOND, 4, 0.8),
            (ENCHANTED_BOOK, 2, 0.6),
            (GOLD_BLOCK, 1, 0.5),
            (GOLDEN_CHOP, 2, 0.4),
            (HORSE_ARMOR_DIAMOND, 1, 0.25),
            (PICK_DIAMOND, 1, 0.25),
            (ARMOR_FIRST + 12 + CHESTPLATE as Id, 1, 0.2),
            (NAME_TAG, 1, 0.3),
        ],
        Kind::BastionTreasure => &[
            (UPGRADE_TEMPLATE, 1, 1.0),
            (SCORCHITE_SCRAP, 3, 0.7),
            (SCORCHITE_INGOT, 1, 0.3),
            (DIAMOND, 5, 0.6),
            (GOLD_BLOCK, 4, 0.6),
            (ENCHANTED_BOOK, 1, 0.4),
            (SWORD_DIAMOND, 1, 0.3),
            (PICK_DIAMOND, 1, 0.25),
            (GOLDEN_CHOP, 2, 0.4),
            (DISC_FIRST + 7, 1, 0.2),
        ],
        Kind::SnoutCamp => &[
            (GOLD_INGOT, 9, 0.8),
            (GOLD_BLOCK, 2, 0.3),
            (CROSSBOW, 1, 0.25),
            (SHROOM_STICK, 1, 0.2),
            (PEARL, 3, 0.25),
            (DISC_FIRST + 7, 1, 0.3),
            (SCORCHITE_SCRAP, 2, 0.15),
            (UPGRADE_TEMPLATE, 1, 0.1),
            (GOLDEN_CHOP, 2, 0.3),
        ],
    };
    let mut free: Vec<usize> = (0..c.slots.len()).collect();
    for &(item, most, chance) in table {
        if !rng.chance(chance) || free.is_empty() {
            continue;
        }
        let n = rng.int(1, most as i32) as u8;
        let slot = free.remove(rng.int(0, free.len() as i32 - 1) as usize);
        c.slots[slot] = Some((item, n));
        c.wear[slot] = loot_wear(item, kind, &mut rng);
    }
    // Now and then, a music disc (Oinkstep is the Snouts'; see fortress.rs).
    let disc = match kind {
        Kind::Dungeon => 0.3,
        Kind::HushedCity => 0.3,
        Kind::Tower | Kind::Spire => 0.12,
        _ => 0.0,
    };
    if rng.chance(disc) && !free.is_empty() {
        let slot = free.remove(rng.int(0, free.len() as i32 - 1) as usize);
        c.slots[slot] = Some((DISC_FIRST + rng.int(0, 6) as Id, 1));
    }
    c
}

/// Treasure Maps in a freshly filled chest are marked with the nearest
/// Buried Treasure (or are plain maps if there's none in range).
pub fn mark_maps(generator: &Generator, at: IVec3, c: &mut Container) {
    for i in 0..c.slots.len() {
        if c.slots[i].is_some_and(|s| s.0 == TREASURE_MAP) {
            match generator.nearest_site(Kind::BuriedTreasure, at.as_vec3(), crate::treasure::MAP_RANGE) {
                Some(t) => c.wear[i] = crate::treasure::mark(t),
                None => c.slots[i] = Some((MAP, 1)),
            }
        }
    }
}

/// Tools and armour in chests are used, and sometimes enchanted.
fn loot_wear(item: Id, kind: Kind, rng: &mut Rng) -> Wear {
    if item == ENCHANTED_BOOK {
        return crate::enchant::random_book(rng);
    }
    let Some(max) = durability(item) else { return 0 };
    let used = rng.range(0.1, 0.7) * max as f32;
    let power = match kind {
        Kind::Spire | Kind::HushedCity | Kind::BastionTreasure | Kind::MansionSecret => 20,
        Kind::Dungeon => 15,
        Kind::Tower => 10,
        _ => 5,
    };
    let enchanted = if rng.chance(0.4) { crate::enchant::roll(item, rng.int(1, power) as u8, rng) } else { 0 };
    crate::inventory::with_uses(enchanted, used as u16)
}

impl World {
    /// Roughly the middle of the hut a chest is in (the free floor nearest it).
    fn hut_middle(&self, chest: IVec3) -> macroquad::math::Vec3 {
        let mut best = chest.as_vec3() + macroquad::math::Vec3::new(0.5, 0.0, 0.5);
        for d in [IVec3::new(-2, 0, 2), IVec3::new(2, 0, 2), IVec3::new(-2, 0, -2), IVec3::new(2, 0, -2)] {
            let q = chest + d;
            // (The Hmmer stands halfway there, so that has to be clear too, not in a wall.)
            let half = chest + d / 2;
            let clear = |p: IVec3| self.get_v(p) == AIR && self.get_v(p + IVec3::Y) == AIR;
            if clear(q) && clear(half) {
                best = q.as_vec3() + macroquad::math::Vec3::new(0.5, 0.0, 0.5);
                break;
            }
        }
        best
    }

    /// A chunk just arrived: fill any structure chests in it that have never been filled.
    pub fn fill_structure_chests(&mut self, cx: i32, cz: i32) {
        // Creaking Hearts grow in pale oaks (see creaking.rs).
        for p in self.generator.creaking_hearts(cx, cz) {
            if crate::creaking::is_heart(self.get_v(p)) {
                self.cages.insert(p);
            }
        }
        for p in self.generator.structure_cages(cx, cz) {
            if crate::fortress::is_cage(self.get_v(p)) {
                self.cages.insert(p);
            }
        }
        // Temple dispensers are loaded with arrows.
        for p in self.generator.structure_dispensers(cx, cz) {
            if crate::contraptions::is_dispenser(self.get_v(p)) && !self.containers.contains_key(&p) {
                let mut c = crate::containers::Container::for_block(DISPENSER_FIRST);
                c.slots[0] = Some((ARROW, 9));
                self.containers.insert(p, c);
            }
        }
        for (p, kind, seed) in self.generator.structure_chests(cx, cz) {
            if self.get_v(p) == CHEST && !self.containers.contains_key(&p) {
                let mut c = loot(kind, seed);
                mark_maps(&self.generator, p, &mut c);
                if kind == Kind::Mineshaft {
                    // A mineshaft's "chests" are carts of loot parked on its rails.
                    let ew = [IVec3::X, IVec3::NEG_X].iter().any(|&d| crate::vehicles::is_rail(self.get_v(p + d)));
                    self.set_v(p, if ew { RAIL_FIRST + 1 } else { RAIL_FIRST });
                    self.new_carts.push((p.as_vec3() + macroquad::math::Vec3::new(0.5, 0.0, 0.5), c));
                    continue;
                }
                self.containers.insert(p, c);
                if kind == Kind::Monument {
                    for at in crate::monument::elder_spots(p) {
                        self.new_residents.push((at, crate::entity::MobKind::ElderGuardian));
                    }
                }
                if kind == Kind::Outpost {
                    // The Allay they keep caged beside the tower.
                    self.new_residents.push((crate::raids::allay_cage(p), crate::entity::MobKind::Allay));
                }
                if kind == Kind::Igloo {
                    // The basement's prisoners.
                    let [hmmer, zombie] = crate::temples::igloo_cells(p);
                    self.new_residents.push((hmmer, crate::entity::MobKind::Hmmer));
                    self.new_residents.push((zombie, crate::entity::MobKind::ZombieHmmer));
                }
                if kind == Kind::Village {
                    // The Clanker moves in: it stands guard on the square.
                    self.new_clankers.push(p.as_vec3() + macroquad::math::Vec3::new(-1.5, 0.0, -3.5));
                }
                if kind == Kind::Hut {
                    // Someone lives here: they stand a step in from the chest.
                    let spot = (p.as_vec3() + macroquad::math::Vec3::new(0.5, 0.0, 0.5)).lerp(self.hut_middle(p), 0.5);
                    self.new_huts.push((spot, seed));
                }
            }
        }
        // Village farmland in this chunk gets its soil (tilled land always has
        // some, so any without is fresh from the generator).
        for village in self.generator.villages_near(cx, cz) {
            for f in self.generator.village_farmland(&village) {
                if f.x.div_euclid(CW) == cx && f.z.div_euclid(CW) == cz && crate::farming::is_farmland(self.get_v(f)) {
                    self.farm.entry(f).or_default();
                }
            }
        }
    }
}

/// A buried ruin: walls sunk into the ground (a few stones poking up give it
/// away) and fill hiding suspicious blocks, more of them the deeper you go.
fn ruin_blocks(site: &Site) -> Vec<(IVec3, Id)> {
    let o = site.origin;
    let s = site.seed;
    let mut out = Vec::new();
    let (wall, fill, sus, floor): (&[Id], Id, Id, Id) = match site.kind {
        Kind::DesertRuins => (&[SANDSTONE, SANDSTONE, TERRACOTTA + 1], SAND, SUSPICIOUS_SAND, SANDSTONE),
        Kind::TrailRuins => (&[TERRACOTTA, TERRACOTTA + 2, BRICK, MUD, TERRACOTTA + 3], GRAVEL, SUSPICIOUS_GRAVEL, BRICK),
        _ => (&[STONE_BRICKS, MOSSY_COBBLE, STONE_BRICKS], SAND, SUSPICIOUS_SAND, STONE_BRICKS),
    };
    let ocean = site.kind == Kind::OceanRuins;
    // Two overlapping rooms, so the footprint isn't just a square.
    let rooms = [(0, 0, 4 + (s % 2) as i32), (3 + (s % 3) as i32, -2 - (s % 2) as i32, 3)];
    let inside = |x: i32, z: i32| rooms.iter().any(|&(rx, rz, r)| (x - rx).abs() < r && (z - rz).abs() < r);
    let on_wall = |x: i32, z: i32| !inside(x, z) && rooms.iter().any(|&(rx, rz, r)| (x - rx).abs() <= r && (z - rz).abs() <= r);
    let depth = if ocean { 4 } else { 8 };
    for x in -8..=10 {
        for z in -8..=8 {
            let (wall_here, in_here) = (on_wall(x, z), inside(x, z));
            if !wall_here && !in_here {
                continue;
            }
            for y in -depth..=2 {
                let p = ivec3(o.x + x, o.y + y, o.z + z);
                let r = hash3(s, x, y, z);
                if y == -depth {
                    out.push((p, floor));
                } else if wall_here {
                    // Ruined: underground it's whole, above ground just a stub here and there.
                    let keep = if ocean { y <= 1 && r < 0.75 } else { y < 0 || (y == 0 && r < 0.35) || (y == 1 && r < 0.12) };
                    if keep {
                        out.push((p, wall[(hash3(s ^ 9, x, y, z) * wall.len() as f32) as usize % wall.len()]));
                    }
                } else if y < 0 {
                    // Fill, with finds; deeper fill hides more.
                    let chance = 0.05 + (-y) as f32 * 0.012;
                    out.push((p, if r < chance { sus } else { fill }));
                } else if ocean && y == 0 && r < 0.08 {
                    out.push((p, sus));
                }
            }
            // The odd pot left standing at the bottom of a room.
            if in_here && hash3(s ^ 0x907, x, 0, z) < 0.04 {
                let shard = 1 + (hash3(s ^ 0x908, x, 0, z) * 12.0) as Id % 12;
                out.push((ivec3(o.x + x, o.y - depth + 1, o.z + z), POT_FIRST + shard));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_world_has_villages_and_few_lone_huts() {
        for seed in 1..=12u32 {
            let g = Generator::new(seed);
            let near = g.nearest_site(Kind::Village, macroquad::math::Vec3::new(0.0, 70.0, 0.0), 30);
            assert!(near.is_some_and(|p| p.as_vec3().length() < 400.0), "seed {seed}: no village within 400 blocks");
            let huts = (-20..20).flat_map(|z| (-20..20).map(move |x| (x, z))).filter(|&(x, z)| g.site(x, z).is_some_and(|s| s.kind == Kind::Hut)).count();
            assert!(huts <= 6, "seed {seed}: {huts} lone huts in 40x40 chunks");
        }
    }

    #[test]
    fn villages_have_houses_farms_and_a_square() {
        let g = Generator::new(4242);
        let village = (-150..150).flat_map(|cz| (-150..150).map(move |cx| (cx, cz))).find_map(|(cx, cz)| g.site(cx, cz).filter(|s| s.kind == Kind::Village));
        let v = village.expect("no village anywhere near");
        let blocks = g.site_blocks(&v);
        assert!(blocks.iter().filter(|b| b.1 == CHEST).count() >= 4, "a square chest and a few houses");
        assert!(blocks.iter().any(|b| b.1 == GRAVEL) && blocks.iter().any(|b| b.1 == FARMLAND_WET) && blocks.iter().any(|b| b.1 == WATER));
        assert!(blocks.iter().any(|b| (b.0, b.1) == (village_chest(&v), CHEST)));
        // Its chests are the square's (a Clanker moves in) and huts' (each gets a Hmmer).
        let (cx, cz) = (v.origin.x.div_euclid(CW), v.origin.z.div_euclid(CW));
        let kinds: Vec<Kind> = (-2..=2).flat_map(|dz| (-2..=2).map(move |dx| (dx, dz))).flat_map(|(dx, dz)| g.structure_chests(cx + dx, cz + dz)).map(|c| c.1).collect();
        assert_eq!(kinds.iter().filter(|k| **k == Kind::Village).count(), 1);
        assert!(kinds.iter().filter(|k| **k == Kind::Hut).count() >= 3);
    }

    #[test]
    fn structures_are_found_and_filled() {
        let g = Generator::new(1234);
        // Plenty of each kind across a patch of the world, all repeatable.
        let mut found = std::collections::HashMap::new();
        for cz in -40..40 {
            for cx in -40..40 {
                if let Some(site) = g.site(cx, cz) {
                    assert_eq!(g.site(cx, cz), Some(site));
                    *found.entry(site.kind).or_insert(0) += 1;
                }
            }
        }
        assert!(found.get(&Kind::Dungeon).copied().unwrap_or(0) > 50, "{found:?}");
        // Lone huts are rare now (villages are where the houses are).
        assert!(found.get(&Kind::Hut).copied().unwrap_or(0) > 0, "{found:?}");
        assert!(found.get(&Kind::Village).copied().unwrap_or(0) < 100, "villages don't crowd each other: {found:?}");
        // A chunk with a structure shows it, and its chests know their loot.
        let (cx, cz, site) = (-40..40).flat_map(|z| (-40..40).map(move |x| (x, z))).find_map(|(x, z)| g.site(x, z).filter(|s| s.kind == Kind::Hut).map(|s| (x, z, s))).unwrap();
        let mut blocks = g.generate(cx, cz);
        g.place_structures(cx, cz, &mut blocks);
        let chests = g.structure_chests(cx, cz);
        assert!(!chests.is_empty());
        let c = loot(chests[0].1, chests[0].2);
        assert!(c.slots.iter().any(|s| s.is_some()), "never empty handed");
        assert_eq!(loot(chests[0].1, chests[0].2), c, "the same every time");
        let o = site.origin;
        let (lx, lz) = (o.x - cx * CW, o.z - cz * CW);
        assert_eq!(blocks[crate::world::idx(lx, o.y, lz)], PLANKS, "the hut floor");
    }

    #[test]
    fn new_worlds_space_their_structures_out_and_grow_bigger_biomes() {
        use crate::world::GenOptions;
        let g = Generator::with(31337, GenOptions::DEFAULT);
        let mut surface = Vec::new();
        for cz in -40..40 {
            for cx in -40..40 {
                if let Some(site) = g.site(cx, cz).filter(|s| !matches!(s.kind, Kind::TrialChambers | Kind::HushedCity | Kind::Mineshaft)) {
                    surface.push((cx, cz, site.kind));
                }
            }
        }
        assert!(surface.len() > 40, "still plenty: {}", surface.len());
        for (i, a) in surface.iter().enumerate() {
            for b in &surface[i + 1..] {
                assert!((a.0 - b.0).abs().max((a.1 - b.1).abs()) >= 2, "{a:?} and {b:?} are side by side");
            }
        }
        // None means none.
        let none = Generator::with(31337, GenOptions { structures: 0, ..GenOptions::DEFAULT });
        assert!((-20..20).all(|cz| (-20..20).all(|cx| none.site(cx, cz).is_none())));
        // Bigger biomes: walking along some long lines, the stretch of one land biome
        // you're typically in (weighted by how long you spend in it) is longer.
        let typical = |g: &Generator| {
            let (mut sum, mut sq) = (0.0f32, 0.0f32);
            for z in [777, -3000, 5100] {
                let v: Vec<_> = (0..6000).step_by(4).map(|x| g.column(x, z).1).collect();
                let mut run = 1.0f32;
                for w in v.windows(2) {
                    if w[0].is_ocean() {
                        run = 1.0;
                    } else if w[0] == w[1] {
                        run += 1.0;
                    } else {
                        sum += run;
                        sq += run * run;
                        run = 1.0;
                    }
                }
                sum += run;
                sq += run * run;
            }
            sq / sum * 4.0
        };
        // (Version 2 grew them; version 3 trims them back a bit; see world.rs.)
        let v2 = Generator::with(31337, GenOptions { version: 2, ..GenOptions::DEFAULT });
        let (old, new) = (typical(&Generator::new(31337)), typical(&v2));
        assert!(new > old * 1.4, "old {old}, new {new}");
        // The options pack into the save and the join message.
        let o = GenOptions { version: 1, structures: 3, biome_size: 2, terrain: 0 };
        assert_eq!(GenOptions::unpack(o.pack()), o);
        assert_eq!(GenOptions::unpack(0), GenOptions::LEGACY);
    }
}
