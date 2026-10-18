//! Copper doors, trapdoors and grates (v0.8), weathering like copper blocks.
//!
//! Each comes in the four stages (new, exposed, weathered, oxidized), waxed
//! or not: eight looks, `VARIANT = waxed * 4 + stage`. Unwaxed ones move on a
//! stage now and then (see copper.rs), keeping whichever way they face and
//! whether they're open; Goo or honeycomb waxes them where they are. Copper
//! doors and trapdoors open by hand, like wooden ones.
//!
//! Blocks, from `COPPERWORKS_FIRST`: for each variant a door (`+ facing * 4
//! + open * 2 + top`, 16) then a trapdoor (`+ facing * 2 + open`, 8); then
//! the grates of variants 1 to 7 (variant 0's is the old `COPPER_GRATE`).

use crate::block::*;
use crate::texture::*;

pub const VARIANTS: usize = 8;
const PER: Id = 24;
pub const COUNT: Id = VARIANTS as Id * PER + VARIANTS as Id - 1;

const STAGES: [(&str, &str); 4] = [("", ""), ("exposed_", "Exposed "), ("weathered_", "Weathered "), ("oxidized_", "Oxidized ")];

pub fn door(v: usize) -> Id {
    COPPERWORKS_FIRST + v as Id * PER
}
pub fn trapdoor(v: usize) -> Id {
    door(v) + 16
}
pub fn grate(v: usize) -> Id {
    if v == 0 { COPPER_GRATE } else { COPPERWORKS_FIRST + VARIANTS as Id * PER + v as Id - 1 }
}
/// The item a variant's door is placed from.
pub fn door_item(v: usize) -> Id {
    COPPER_DOOR_FIRST + v as Id
}

/// Which variant a copper door, trapdoor or grate is, and its offset in its family.
pub fn variant_of(id: Id) -> Option<(usize, Id)> {
    if id == COPPER_GRATE {
        return Some((0, 0));
    }
    if !(COPPERWORKS_FIRST..COPPERWORKS_FIRST + COUNT).contains(&id) {
        return None;
    }
    let k = id - COPPERWORKS_FIRST;
    if k >= VARIANTS as Id * PER {
        return Some(((k - VARIANTS as Id * PER) as usize + 1, 0));
    }
    Some(((k / PER) as usize, k % PER))
}

/// The same block in variant `v` (same facing, open or shut, top or bottom).
pub fn as_variant(id: Id, v: usize) -> Option<Id> {
    let (_, k) = variant_of(id)?;
    Some(if is_grate(id) { grate(v) } else { door(v) + k })
}

pub fn is_grate(id: Id) -> bool {
    variant_of(id).is_some() && block(id).shape == Shape::Full
}

/// The next stage of weathering, if it has one (waxed ones don't).
pub fn weathered(id: Id) -> Option<Id> {
    let (v, _) = variant_of(id)?;
    (v < 3).then(|| as_variant(id, v + 1)).flatten()
}

/// The waxed version, if it's unwaxed.
pub fn waxed(id: Id) -> Option<Id> {
    let (v, _) = variant_of(id)?;
    (v < 4).then(|| as_variant(id, v + 4)).flatten()
}

/// The copper door variant an item places, if it's a copper door.
pub fn door_for_item(item: Id) -> Option<Id> {
    (COPPER_DOOR_FIRST..COPPER_DOOR_FIRST + VARIANTS as Id).contains(&item).then(|| door((item - COPPER_DOOR_FIRST) as usize))
}

/// The four stages' tiles: door top, door bottom, trapdoor, grate.
fn tiles(stage: usize) -> [u16; 4] {
    let s = stage as u16;
    [T_COPPER_DOOR_TOP + s, T_COPPER_DOOR_BOTTOM + s, T_COPPER_TRAPDOOR + s, if stage == 0 { T_COPPER_GRATE } else { T_COPPER_GRATE_STAGE + s - 1 }]
}

fn names(v: usize) -> (&'static str, &'static str) {
    let (k, n) = STAGES[v % 4];
    if v >= 4 { (leak(&format!("waxed_{k}")), leak(&format!("Waxed {n}"))) } else { (k, n) }
}

pub fn defs() -> Vec<BlockDef> {
    let mut out = Vec::new();
    for v in 0..VARIANTS {
        let (k, n) = names(v);
        let [top_t, bottom_t, trap_t, _] = tiles(v % 4);
        for facing in 0..4u8 {
            for open in [false, true] {
                for top in [false, true] {
                    let key = format!("{k}copper_door_{}_{}_{}", ["north", "east", "south", "west"][facing as usize], if open { "open" } else { "closed" }, if top { "top" } else { "bottom" });
                    let tile = if top { top_t } else { bottom_t };
                    let mut d = def(leak(&key), leak(&format!("{n}Copper Door")), Model::Shaped, true, false, [tile; 3], 3.0, 1, true, door_item(v), 0.0, S_STONE);
                    d.shape = Shape::Door { facing, open, top };
                    d.creative = false;
                    out.push(d);
                }
            }
        }
        for facing in 0..4u8 {
            for open in [false, true] {
                let key = format!("{k}copper_trapdoor{}{}", ["", "_east", "_south", "_west"][facing as usize], if open { "_open" } else { "" });
                let mut d = def(leak(&key), leak(&format!("{n}Copper Trapdoor")), Model::Shaped, true, false, [trap_t; 3], 3.0, 1, true, trapdoor(v), 0.0, S_STONE);
                d.shape = Shape::Trapdoor { facing, open };
                d.creative = facing == 0 && !open;
                d.see_through = true;
                out.push(d);
            }
        }
    }
    for v in 1..VARIANTS {
        let (k, n) = names(v);
        let mut d = def(leak(&format!("{k}copper_grate")), leak(&format!("{n}Copper Grate")), Model::Cube, true, false, [tiles(v % 4)[3]; 3], 3.0, 1, true, grate(v), 0.0, S_STONE);
        d.see_through = true;
        out.push(d);
    }
    out
}

pub fn items() -> Vec<ItemDef> {
    (0..VARIANTS)
        .map(|v| {
            let (k, n) = names(v);
            item(leak(&format!("{k}copper_door")), leak(&format!("{n}Copper Door")), T_COPPER_DOOR_ITEM + (v % 4) as u16)
        })
        .collect()
}

pub fn recipes() -> Vec<Recipe> {
    let r = |inputs: &[(Id, u8)], output: (Id, u8)| Recipe { inputs: inputs.to_vec(), output };
    let mut out = vec![r(&[(COPPER_INGOT, 6)], (door_item(0), 3)), r(&[(COPPER_INGOT, 4)], (trapdoor(0), 2))];
    // Waxing in the grid, as with copper blocks.
    for v in 0..4 {
        for wax in [GOO, HONEYCOMB] {
            out.push(r(&[(door_item(v), 1), (wax, 1)], (door_item(v + 4), 1)));
            out.push(r(&[(trapdoor(v), 1), (wax, 1)], (trapdoor(v + 4), 1)));
            out.push(r(&[(grate(v), 1), (wax, 1)], (grate(v + 4), 1)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use macroquad::math::{ivec3, IVec3};

    #[test]
    fn every_variant_has_a_door_a_trapdoor_and_a_grate() {
        for v in 0..VARIANTS {
            assert_eq!(door_base(door(v) + 13), Some(door(v)));
            assert_eq!(placing_item(door(v)), Some(door_item(v)));
            assert_eq!(door_for_item(door_item(v)), Some(door(v)));
            assert_eq!(placing_item(trapdoor(v) + 5), Some(trapdoor(v)));
            assert!(crate::carpentry::is_trapdoor(trapdoor(v) + 7));
            assert!(is_grate(grate(v)));
            assert_eq!(variant_of(grate(v)), Some((v, 0)));
        }
        assert_eq!(block(door(7) + 15).key, "waxed_oxidized_copper_door_west_open_top");
        assert_eq!(block(grate(7)).key, "waxed_oxidized_copper_grate");
        assert_eq!(COPPERWORKS_FIRST + COUNT, NUM_BLOCKS);
    }

    #[test]
    fn they_weather_in_place_until_waxed() {
        let open_top = door(0) + 4 + 2 + 1;
        assert_eq!(weathered(open_top), Some(door(1) + 7));
        assert_eq!(weathered(trapdoor(2) + 3), Some(trapdoor(3) + 3));
        assert_eq!(weathered(trapdoor(3)), None, "oxidized is as far as it goes");
        assert_eq!(weathered(COPPER_GRATE), Some(grate(1)));
        assert_eq!(waxed(grate(2)), Some(grate(6)));
        assert_eq!(weathered(waxed(door(1)).unwrap()), None);
        // In the world: a random tick moves both halves of a door on together.
        let mut g = crate::game::tests::arena(818);
        let floor = ivec3(3, 49, 0);
        g.place_door(door(0), floor, IVec3::Y, STONE);
        let (bottom, top) = (floor + IVec3::Y, floor + IVec3::Y * 2);
        assert_eq!(door_base(g.world.get_v(top)), Some(door(0)));
        g.weather_copperwork(bottom);
        assert_eq!(door_base(g.world.get_v(bottom)), Some(door(1)));
        assert_eq!(door_base(g.world.get_v(top)), Some(door(1)), "the top half too");
        // A copper door opens by hand.
        g.toggle_door(top);
        assert!(door_state(g.world.get_v(bottom)).is_some_and(|s| s.1));
        // Waxed, both halves stop.
        g.inv.slots[g.inv.selected] = Some((GOO, 1));
        assert!(g.wax_copper(bottom));
        assert_eq!(door_base(g.world.get_v(top)), Some(door(5)));
        assert!(g.advancements.has("copper_bottomed"));
        g.weather_copperwork(bottom);
        assert_eq!(door_base(g.world.get_v(bottom)), Some(door(5)));
        // Trapdoors open by hand too.
        let t = ivec3(-3, 50, 3);
        g.world.set_v(t, trapdoor(2));
        assert!(g.toggle_hinged(t));
        assert_eq!(g.world.get_v(t), trapdoor(2) + 1);
    }
}
