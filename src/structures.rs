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

use crate::block::*;
use crate::containers::Container;
use crate::inventory::Wear;
use crate::noise::{hash2, hash3, Rng};
use crate::world::{Biome, Generator, World, CH, CW, SEA};
use macroquad::math::{ivec3, IVec3};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Kind {
    Dungeon,
    Tower,
    Hut,
    Well,
    Village,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Dungeon => "Dungeon (Mossy, Moody)",
            Kind::Tower => "Ruined Tower (Structurally Optimistic)",
            Kind::Hut => "Hut (Definitely Not a Village)",
            Kind::Well => "Wishing Well (Doesn't Grant)",
            Kind::Village => "Village (Actually a Village)",
        }
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
    pub fn site(&self, cx: i32, cz: i32) -> Option<Site> {
        if cx * CW >= crate::scorch::SCORCH_X - crate::scorch::WALL - 2 * CW {
            return None;
        }
        let s = self.seed ^ 0x57_C0DE;
        let r = hash2(s, cx, cz);
        if r >= 0.10 {
            return None;
        }
        let ox = cx * CW + 5 + (hash2(s ^ 1, cx, cz) * 6.0) as i32;
        let oz = cz * CW + 5 + (hash2(s ^ 2, cx, cz) * 6.0) as i32;
        let (h, biome) = self.column(ox, oz);
        let seed = (hash2(s ^ 3, cx, cz) * u32::MAX as f32) as u32;
        let facing = (hash2(s ^ 4, cx, cz) * 4.0) as u8 % 4;
        // Surface buildings want ground that's roughly flat.
        let flat = || [(-3, -3), (3, -3), (-3, 3), (3, 3)].iter().all(|&(dx, dz)| (self.column(ox + dx, oz + dz).0 - h).abs() <= 2);
        // Villages: rare, on wide flat plains.
        let wide_flat = || (0..8).all(|k| {
            let a = k as f32 * std::f32::consts::FRAC_PI_4;
            let (dx, dz) = ((a.cos() * 15.0) as i32, (a.sin() * 15.0) as i32);
            let (hh, b) = self.column(ox + dx, oz + dz);
            (hh - h).abs() <= 3 && b != Biome::Ocean
        });
        if hash2(s ^ 0x7111, cx, cz) < 0.02 && biome == Biome::Plains && h > SEA + 1 && h < CH - 30 && wide_flat() {
            return Some(Site { kind: Kind::Village, origin: ivec3(ox, h, oz), facing, seed });
        }
        let kind = if r < 0.055 {
            if h < 30 || biome == Biome::Ocean {
                return None;
            }
            Kind::Dungeon
        } else if r < 0.072 && matches!(biome, Biome::Plains | Biome::Forest) && h > SEA + 1 && flat() {
            Kind::Hut
        } else if r < 0.085 && matches!(biome, Biome::Plains | Biome::Forest | Biome::Snowy) && h > SEA + 1 && h < CH - 20 && flat() {
            Kind::Tower
        } else if biome == Biome::Desert && h > SEA + 1 && flat() {
            Kind::Well
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
                put(0, 1, 0, GLOWSHROOM); // something to see by
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
        // Houses beside the paths, doors to the path (facing: 0 north .. 3 west).
        for (i, &(x, z, facing)) in [(9, -6, 2u8), (-9, -6, 2), (9, 6, 0), (6, 13, 3), (-6, -13, 1), (6, -14, 3)].iter().enumerate() {
            if i >= 3 && hash2(s ^ 0x4053, i as i32, 0) < 0.35 {
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

    /// Where a village's farmland is (so it can be given soil when it first loads).
    pub fn village_farmland(&self, site: &Site) -> Vec<IVec3> {
        self.village_blocks(site).into_iter().filter(|b| b.1 == FARMLAND_WET).map(|b| b.0).collect()
    }

    /// Stamp every structure that reaches into chunk (cx, cz) onto its blocks.
    /// (Villages reach two chunks out; everything else, one.)
    pub fn place_structures(&self, cx: i32, cz: i32, b: &mut [Id]) {
        for dz in -2..=2 {
            for dx in -2..=2 {
                let Some(site) = self.site(cx + dx, cz + dz) else { continue };
                if (dx.abs() == 2 || dz.abs() == 2) && site.kind != Kind::Village {
                    continue;
                }
                for (p, id) in self.site_blocks(&site) {
                    let (lx, lz) = (p.x - cx * CW, p.z - cz * CW);
                    if (0..CW).contains(&lx) && (0..CW).contains(&lz) && (1..CH).contains(&p.y) {
                        b[crate::world::idx(lx, p.y, lz)] = id;
                    }
                }
            }
        }
    }

    /// Chests of structures that sit in chunk (cx, cz), with what kind of place they're in.
    pub fn structure_chests(&self, cx: i32, cz: i32) -> Vec<(IVec3, Kind, u32)> {
        let mut v = Vec::new();
        for (site, blocks) in self.sites_near(cx, cz) {
            for (p, id) in blocks {
                if id == CHEST && p.x.div_euclid(CW) == cx && p.z.div_euclid(CW) == cz {
                    // A village's houses have hut chests; only the square's is the village's.
                    let kind = if site.kind == Kind::Village && p != village_chest(&site) { Kind::Hut } else { site.kind };
                    v.push((p, kind, site.seed ^ (p.x as u32).wrapping_mul(31) ^ (p.y as u32).wrapping_mul(17) ^ p.z as u32));
                }
            }
        }
        v
    }

    /// Sites that can reach into chunk (cx, cz), with their blocks.
    fn sites_near(&self, cx: i32, cz: i32) -> Vec<(Site, Vec<(IVec3, Id)>)> {
        let mut v = Vec::new();
        for dz in -2..=2 {
            for dx in -2..=2 {
                let Some(site) = self.site(cx + dx, cz + dz) else { continue };
                if (dx.abs() == 2 || dz.abs() == 2) && site.kind != Kind::Village {
                    continue;
                }
                v.push((site, self.site_blocks(&site)));
            }
        }
        v
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
        Kind::Village => &[(BREAD, 6, 0.8), (APPLE, 4, 0.6), (IRON, 4, 0.5), (GOLD_INGOT, 5, 0.5), (BOOK, 2, 0.4), (SADDLE, 1, 0.2), (NAME_TAG, 1, 0.25), (WHEAT_SEEDS, 12, 0.5), (TORCH, 12, 0.5)],
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
    c
}

/// Tools and armour in chests are used, and sometimes enchanted.
fn loot_wear(item: Id, kind: Kind, rng: &mut Rng) -> Wear {
    let Some(max) = durability(item) else { return 0 };
    let used = rng.range(0.1, 0.7) * max as f32;
    let power = match kind {
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
            if self.get_v(q) == AIR && self.get_v(q + IVec3::Y) == AIR {
                best = q.as_vec3() + macroquad::math::Vec3::new(0.5, 0.0, 0.5);
                break;
            }
        }
        best
    }

    /// A chunk just arrived: fill any structure chests in it that have never been filled.
    pub fn fill_structure_chests(&mut self, cx: i32, cz: i32) {
        for (p, kind, seed) in self.generator.structure_chests(cx, cz) {
            if self.get_v(p) == CHEST && !self.containers.contains_key(&p) {
                self.containers.insert(p, loot(kind, seed));
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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(found.get(&Kind::Hut).copied().unwrap_or(0) > 3, "{found:?}");
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
}
