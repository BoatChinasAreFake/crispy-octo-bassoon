//! **Shipwrecks**, **Treasure Maps** and **Buried Treasure**.
//!
//! Wrecks lie on the sea bed, broken open, with a chest in the bow and one
//! in the stern: supplies, and often a Treasure Map. A map is marked with
//! the nearest Buried Treasure to where it was found (the spot is kept in
//! the map's wear: x in the low half, z in the high half). Hold it to see
//! the land around the X and how far off it is. Buried Treasure is a chest
//! a couple of blocks down in a beach; digging it up is the
//! "X Marks the Spot" advancement. Cartographers sell maps too (see
//! villagers.rs), marked from wherever they live.

use crate::block::*;
use crate::inventory::Wear;
use crate::noise::hash3;
use crate::structures::{Kind, Site};
use crate::world::SEA;
use macroquad::math::{ivec3, IVec3, Vec3};

/// How far (chunks) a map looks for treasure to mark.
pub const MAP_RANGE: i32 = 24;

/// A map's wear, marking treasure at `t`.
pub fn mark(t: IVec3) -> Wear {
    (t.x as i16 as u16 as u32) | ((t.z as i16 as u16 as u32) << 16)
}

/// Where a map's X is (None: a blank map). Treasure is never at x = 0 (see `Generator::site`).
pub fn marked(w: Wear) -> Option<(i32, i32)> {
    let (x, z) = (w as u16 as i16 as i32, (w >> 16) as u16 as i16 as i32);
    (x != 0 || z != 0).then_some((x, z))
}

/// How far and which way the X is from `from`, for the map's caption.
pub fn caption(w: Wear, from: Vec3) -> String {
    let Some((x, z)) = marked(w) else { return "A blank map. Look in shipwrecks for a marked one.".into() };
    let (dx, dz) = (x as f32 + 0.5 - from.x, z as f32 + 0.5 - from.z);
    let d = (dx * dx + dz * dz).sqrt();
    if d < 3.0 {
        return "X marks the spot: dig here!".into();
    }
    let ns = if dz < -d * 0.38 { "north" } else if dz > d * 0.38 { "south" } else { "" };
    let ew = if dx > d * 0.38 { "east" } else if dx < -d * 0.38 { "west" } else { "" };
    let way = if !ns.is_empty() && !ew.is_empty() { format!("{ns}-{ew}") } else { format!("{ns}{ew}") };
    format!("Treasure: {} blocks {way}", d as i32)
}

/// A wreck's hull: how wide it is (each side of the keel) along its length.
fn half_width(z: i32) -> i32 {
    match z.abs() {
        0..=3 => 2,
        4 | 5 => 1,
        _ => 0,
    }
}

/// A wreck's blocks, in world coordinates: a broken hull of planks on the sea
/// bed with a snapped mast, water inside, and a chest at each end.
pub fn shipwreck_blocks(site: &Site) -> Vec<(IVec3, Id)> {
    let (o, s) = (site.origin, site.seed);
    let mut out = Vec::new();
    // Lengthways along z, or along x.
    let along_x = site.facing % 2 == 1;
    let mut put = |x: i32, y: i32, z: i32, id: Id| {
        let (x, z) = if along_x { (z, x) } else { (x, z) };
        out.push((ivec3(o.x + x, o.y + y, o.z + z), id));
    };
    let fill = |y: i32| if o.y + y <= SEA { WATER } else { AIR };
    for z in -6..=6 {
        let w = half_width(z);
        for x in -w..=w {
            // Rotted through here and there.
            let broken = |y: i32| hash3(s, x, y, z) < 0.16;
            put(x, 0, z, if broken(0) { fill(0) } else { PLANKS });
            for y in 1..=2 {
                let wall = x.abs() == w || z.abs() == 6;
                put(x, y, z, if wall && !broken(y) { PLANKS } else { fill(y) });
            }
            // The deck, with a hatch in the middle.
            let hatch = x == 0 && (1..=2).contains(&z);
            put(x, 3, z, if hatch || broken(3) || hash3(s ^ 9, x, 3, z) < 0.25 { fill(3) } else { PLANKS });
            for y in 4..=6 {
                put(x, y, z, fill(y));
            }
        }
    }
    // The mast, snapped off.
    let mast = 4 + (hash3(s, 0, 9, 0) * 3.0) as i32;
    for y in 1..=mast {
        put(0, y, -1, SPRUCE_LOG);
    }
    put(0, 1, 4, CHEST);
    put(0, 1, -4, CHEST);
    out
}

/// Buried treasure: a chest two blocks down in the sand.
pub fn treasure_blocks(site: &Site) -> Vec<(IVec3, Id)> {
    vec![(site.origin, CHEST)]
}

impl crate::game::Game {
    /// Opened a chest: was it a wreck's, or buried treasure?
    pub fn treasure_advancements(&mut self, pos: IVec3) {
        let (cx, cz) = (pos.x.div_euclid(crate::world::CW), pos.z.div_euclid(crate::world::CW));
        for dz in -1..=1 {
            for dx in -1..=1 {
                match self.world.generator.site(cx + dx, cz + dz) {
                    Some(s) if s.kind == Kind::BuriedTreasure && s.origin == pos => self.advance("x_marks_the_spot"),
                    Some(s) if s.kind == Kind::Shipwreck && s.origin.as_vec3().distance(pos.as_vec3()) < 8.0 => self.advance("ahoy"),
                    Some(s) if s.kind == Kind::Bastion && crate::bastion::treasure_chest(&s) == pos => self.advance("gilded_age"),
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_remember_where_the_x_is() {
        for (x, z) in [(5, 7), (-1234, 8765), (32_000, -32_000), (21, 0)] {
            assert_eq!(marked(mark(ivec3(x, 60, z))), Some((x, z)));
        }
        assert_eq!(marked(0), None);
        assert_eq!(crate::inventory::sanitize_wear(TREASURE_MAP, mark(ivec3(-40, 0, 90))), mark(ivec3(-40, 0, 90)), "the mark survives the trip");
        assert!(caption(mark(ivec3(100, 0, 0)), Vec3::new(0.0, 60.0, 0.0)).contains("east"));
        assert!(caption(mark(ivec3(0, 0, -100)), Vec3::new(5.0, 60.0, 0.0)).contains("north"));
        assert!(caption(mark(ivec3(10, 0, 10)), Vec3::new(10.5, 60.0, 10.5)).contains("dig"));
    }

    #[test]
    fn wrecks_lie_on_the_sea_bed_with_chests_and_maps_find_treasure() {
        let g = crate::world::Generator::new(7);
        let wreck = (0..200).flat_map(|cx| (-100..100).map(move |cz| (cx, cz))).find_map(|(cx, cz)| g.site(cx, cz).filter(|s| s.kind == Kind::Shipwreck)).expect("a wreck somewhere");
        assert!(wreck.origin.y < SEA - 3, "under the sea");
        let blocks = shipwreck_blocks(&wreck);
        assert_eq!(blocks.iter().filter(|b| b.1 == CHEST).count(), 2);
        assert!(blocks.iter().any(|b| b.1 == SPRUCE_LOG) && blocks.iter().filter(|b| b.1 == PLANKS).count() > 30);
        let t = g.nearest_site(Kind::BuriedTreasure, wreck.origin.as_vec3(), MAP_RANGE).expect("treasure near the coast");
        let (h, _) = g.column(t.x, t.z);
        assert!((SEA - 1..=SEA + 1).contains(&h) && t.y == h - 2, "buried in a beach: {t} (ground {h})");
        // Loot: wreck chests often hold a map, treasure chests hold riches.
        let maps = (0..40u32).filter(|&s| crate::structures::loot(Kind::Shipwreck, s.wrapping_mul(2_654_435_761)).slots.iter().flatten().any(|s| s.0 == TREASURE_MAP)).count();
        assert!(maps > 15, "{maps}/40 with a map");
        let gold: u32 = (0..20).map(|s| crate::structures::loot(Kind::BuriedTreasure, s).slots.iter().flatten().filter(|s| s.0 == GOLD_INGOT).map(|s| s.1 as u32).sum::<u32>()).sum();
        assert!(gold > 40, "riches: {gold}");
    }

    #[test]
    fn a_wrecks_map_is_marked_when_its_chest_is_filled() {
        let mut g = crate::game::tests::arena(141);
        let generator = g.world.generator.clone();
        let wreck = (0..200).flat_map(|cx| (-100..100).map(move |cz| (cx, cz))).find_map(|(cx, cz)| generator.site(cx, cz).filter(|s| s.kind == Kind::Shipwreck)).unwrap();
        let chest = shipwreck_blocks(&wreck).into_iter().find(|b| b.1 == CHEST).unwrap().0;
        g.world.set_v(chest, CHEST);
        let mut c = crate::containers::Container::for_block(CHEST);
        c.slots[3] = Some((TREASURE_MAP, 1));
        crate::structures::mark_maps(&generator, chest, &mut c);
        let t = generator.nearest_site(Kind::BuriedTreasure, chest.as_vec3(), MAP_RANGE).unwrap();
        assert_eq!(marked(c.wear[3]), Some((t.x, t.z)));
    }
}
