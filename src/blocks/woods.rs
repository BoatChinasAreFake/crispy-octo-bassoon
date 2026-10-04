//! The woods of v0.3: acacia (savannas), birch (birch forests) and dark oak
//! (dark forests), each with the full set: log, leaves, planks, slabs,
//! stairs, fences, fence gates, doors and a boat.
//!
//! Every wood's blocks sit together, `PER_WOOD` of them from
//! `WOOD_FIRST + wood * PER_WOOD`, in the order of the `part` constants
//! here. Fences, gates and doors work out which wood they are from their
//! shape (see carpentry.rs and `door_base`), so every wood's behave alike.
//! Their planks also count as planks for anything that just wants planks
//! (crafting tables, sticks, chests...).

use crate::block::*;
use crate::texture::*;

/// The parts of a wood, as offsets from its first block.
pub mod part {
    use crate::block::Id;
    pub const LOG: Id = 0;
    pub const LEAVES: Id = 1;
    pub const PLANKS: Id = 2;
    /// Bottom, then top.
    pub const SLAB: Id = 3;
    /// North, east, south, west.
    pub const STAIRS: Id = 5;
    /// `+ mask` of the sides joined.
    pub const FENCE: Id = 9;
    /// `+ x_axis * 2 + open`.
    pub const GATE: Id = 25;
    /// `+ facing * 4 + open * 2 + top`.
    pub const DOOR: Id = 29;
    pub const COUNT: Id = 45;
}

pub const PER_WOOD: Id = part::COUNT;

/// One of the new woods.
pub struct Wood {
    pub key: &'static str,
    pub name: &'static str,
    /// What its planks say about it, for the names.
    pub quip: &'static str,
    pub log_side: u16,
    pub log_top: u16,
    pub leaves: u16,
    pub planks: u16,
    pub door_top: u16,
    pub door_bottom: u16,
    pub door_item: Id,
    pub boat_item: Id,
}

pub const WOODS: [Wood; 3] = [
    Wood { key: "acacia", name: "Acacia", quip: "Sunbaked Orange", log_side: T_ACACIA_LOG_SIDE, log_top: T_ACACIA_LOG_TOP, leaves: T_ACACIA_LEAVES, planks: T_ACACIA_PLANKS, door_top: T_ACACIA_DOOR_TOP, door_bottom: T_ACACIA_DOOR_BOTTOM, door_item: ACACIA_DOOR, boat_item: ACACIA_BOAT },
    Wood { key: "birch", name: "Birch", quip: "Pale and Interesting", log_side: T_BIRCH_LOG_SIDE, log_top: T_BIRCH_LOG_TOP, leaves: T_BIRCH_LEAVES, planks: T_BIRCH_PLANKS, door_top: T_BIRCH_DOOR_TOP, door_bottom: T_BIRCH_DOOR_BOTTOM, door_item: BIRCH_DOOR, boat_item: BIRCH_BOAT },
    Wood { key: "dark_oak", name: "Dark Oak", quip: "Brooding", log_side: T_DARK_OAK_LOG_SIDE, log_top: T_DARK_OAK_LOG_TOP, leaves: T_DARK_OAK_LEAVES, planks: T_DARK_OAK_PLANKS, door_top: T_DARK_OAK_DOOR_TOP, door_bottom: T_DARK_OAK_DOOR_BOTTOM, door_item: DARK_OAK_DOOR, boat_item: DARK_OAK_BOAT },
];

/// The block for `part` of wood `w`.
pub const fn id(w: usize, part: Id) -> Id {
    WOOD_FIRST + w as Id * PER_WOOD + part
}

/// Which new wood a block belongs to, and which part of it it is.
pub fn wood_of(id: Id) -> Option<(usize, Id)> {
    (WOOD_FIRST..WOOD_FIRST + WOODS.len() as Id * PER_WOOD).contains(&id).then(|| (((id - WOOD_FIRST) / PER_WOOD) as usize, (id - WOOD_FIRST) % PER_WOOD))
}

/// The wood whose door (or boat) item this is.
pub fn wood_of_item(item: Id) -> Option<usize> {
    WOODS.iter().position(|w| w.door_item == item || w.boat_item == item)
}

/// Is this one of the new woods' planks?
pub fn is_planks(id: Id) -> bool {
    wood_of(id).is_some_and(|(_, p)| p == part::PLANKS)
}

/// The door blocks an item places (the plain door, or a wood's).
pub fn door_for_item(item: Id) -> Option<Id> {
    if item == DOOR {
        return Some(DOOR_FIRST);
    }
    WOODS.iter().position(|w| w.door_item == item).map(|w| id(w, part::DOOR))
}

/// Every block of every new wood, in id order.
pub fn defs() -> Vec<BlockDef> {
    let mut out = Vec::new();
    for (w, wood) in WOODS.iter().enumerate() {
        let (k, n) = (wood.key, wood.name);
        let planks = id(w, part::PLANKS);
        out.push(def(leak(&format!("{k}_log")), leak(&format!("{n} Log")), Model::Cube, true, true, [wood.log_top, wood.log_side, wood.log_top], 2.0, 0, false, id(w, part::LOG), 0.0, S_WOOD));
        out.push(def(leak(&format!("{k}_leaves")), leak(&format!("{n} Leaves")), Model::Cube, true, false, [wood.leaves; 3], 0.2, 0, false, AIR, 0.0, S_GRASS));
        out.push(def(leak(&format!("{k}_planks")), leak(&format!("{n} Planks ({})", wood.quip)), Model::Cube, true, true, [wood.planks; 3], 2.0, 0, false, planks, 0.0, S_WOOD));
        for top in [false, true] {
            let key = format!("{k}_slab{}", if top { "_top" } else { "" });
            let mut d = def(leak(&key), leak(&format!("{n} Slab")), Model::Shaped, true, false, [wood.planks; 3], 2.0, 0, false, id(w, part::SLAB), 0.0, S_WOOD);
            d.shape = Shape::Slab { top };
            d.creative = !top;
            d.family = id(w, part::SLAB);
            d.full = planks;
            out.push(d);
        }
        for facing in 0..4u8 {
            let key = format!("{k}_stairs{}", ["", "_east", "_south", "_west"][facing as usize]);
            let mut d = def(leak(&key), leak(&format!("{n} Stairs")), Model::Shaped, true, false, [wood.planks; 3], 2.0, 0, false, id(w, part::STAIRS), 0.0, S_WOOD);
            d.shape = Shape::Stairs { facing };
            d.creative = facing == 0;
            d.family = id(w, part::STAIRS);
            d.full = planks;
            out.push(d);
        }
        for mask in 0..16u8 {
            let key = format!("{k}_fence{}", if mask == 0 { String::new() } else { format!("_{mask}") });
            let mut d = def(leak(&key), leak(&format!("{n} Fence")), Model::Shaped, true, false, [wood.planks; 3], 2.0, 0, false, id(w, part::FENCE), 0.0, S_WOOD);
            d.shape = Shape::Fence { mask };
            d.creative = mask == 0;
            d.see_through = true;
            out.push(d);
        }
        for (x_axis, open) in [(false, false), (false, true), (true, false), (true, true)] {
            let key = format!("{k}_fence_gate{}{}", if x_axis { "_x" } else { "" }, if open { "_open" } else { "" });
            let mut d = def(leak(&key), leak(&format!("{n} Fence Gate")), Model::Shaped, !open, false, [wood.planks; 3], 2.0, 0, false, id(w, part::GATE), 0.0, S_WOOD);
            d.shape = Shape::Gate { x_axis, open };
            d.creative = !x_axis && !open;
            d.see_through = true;
            out.push(d);
        }
        for facing in 0..4u8 {
            for open in [false, true] {
                for top in [false, true] {
                    let key = format!("{k}_door_{}_{}_{}", ["north", "east", "south", "west"][facing as usize], if open { "open" } else { "closed" }, if top { "top" } else { "bottom" });
                    let tile = if top { wood.door_top } else { wood.door_bottom };
                    let mut d = def(leak(&key), leak(&format!("{n} Door")), Model::Shaped, true, false, [tile; 3], 1.5, 0, false, wood.door_item, 0.0, S_WOOD);
                    d.shape = Shape::Door { facing, open, top };
                    d.creative = false;
                    out.push(d);
                }
            }
        }
    }
    out
}

/// Each wood's items: its door and its boat.
pub fn items() -> Vec<ItemDef> {
    let mut out = Vec::new();
    for wood in &WOODS {
        out.push(item(leak(&format!("{}_door", wood.key)), leak(&format!("{} Door", wood.name)), T_ACACIA_DOOR_ITEM + wood_index(wood) as u16));
    }
    for wood in &WOODS {
        out.push(ItemDef { stack: 1, ..item(leak(&format!("{}_boat", wood.key)), leak(&format!("{} Boat", wood.name)), T_ACACIA_BOAT_ITEM + wood_index(wood) as u16) });
    }
    out
}

fn wood_index(wood: &Wood) -> usize {
    WOODS.iter().position(|w| w.key == wood.key).unwrap_or(0)
}

/// Each wood's recipes.
pub fn recipes() -> Vec<Recipe> {
    let r = |inputs: &[(Id, u8)], output: (Id, u8)| Recipe { inputs: inputs.to_vec(), output };
    let mut out = Vec::new();
    for (w, wood) in WOODS.iter().enumerate() {
        let planks = id(w, part::PLANKS);
        out.push(r(&[(id(w, part::LOG), 1)], (planks, 4)));
        // (Planks are planks: anything that wants plain planks takes these too.)
        out.push(r(&[(planks, 1)], (PLANKS, 1)));
        out.push(r(&[(planks, 3)], (id(w, part::SLAB), 6)));
        out.push(r(&[(planks, 6)], (id(w, part::STAIRS), 4)));
        out.push(r(&[(planks, 4), (STICK, 2)], (id(w, part::FENCE), 3)));
        out.push(r(&[(STICK, 4), (planks, 2)], (id(w, part::GATE), 1)));
        out.push(r(&[(planks, 6)], (wood.door_item, 3)));
        out.push(r(&[(planks, 5)], (wood.boat_item, 1)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carrying_all_three_logs_is_branching_out() {
        let mut g = crate::game::tests::arena(711);
        let idle = crate::game::Controls::default();
        g.inv.add(id(0, part::LOG), 1);
        g.inv.add(id(1, part::LOG), 1);
        g.update(0.05, &idle);
        assert!(!g.advancements.has("branching_out"), "two woods aren't enough");
        g.inv.add(id(2, part::LOG), 1);
        g.update(0.05, &idle);
        assert!(g.advancements.has("branching_out"));
    }

    #[test]
    fn every_wood_has_its_whole_set_in_order() {
        for (w, wood) in WOODS.iter().enumerate() {
            assert_eq!(block(id(w, part::LOG)).key, format!("{}_log", wood.key));
            assert_eq!(block(id(w, part::PLANKS)).key, format!("{}_planks", wood.key));
            assert_eq!(slab_of(id(w, part::SLAB) + 1), Some((id(w, part::SLAB), true)));
            assert_eq!(stairs_of(id(w, part::STAIRS) + 2).map(|s| s.0), Some(id(w, part::STAIRS)));
            assert_eq!(crate::carpentry::fence_base(id(w, part::FENCE) + 9), Some(id(w, part::FENCE)));
            assert_eq!(crate::carpentry::gate_base(id(w, part::GATE) + 3), Some(id(w, part::GATE)));
            assert_eq!(door_base(id(w, part::DOOR) + 15), Some(id(w, part::DOOR)));
            assert_eq!(block(id(w, part::DOOR) + 15).key, format!("{}_door_west_open_top", wood.key));
            assert_eq!(placing_item(id(w, part::DOOR)), Some(wood.door_item));
            assert_eq!(placing_item(id(w, part::FENCE) + 5), Some(id(w, part::FENCE)));
            assert_eq!(door_for_item(wood.door_item), Some(id(w, part::DOOR)));
            assert_eq!(wood_of(id(w, part::GATE)), Some((w, part::GATE)));
            assert!(is_log(id(w, part::LOG)) && is_leaves(id(w, part::LEAVES)) && is_planks(id(w, part::PLANKS)));
        }
        assert_eq!(id(WOODS.len() - 1, part::COUNT), MYCELIUM);
    }

    #[test]
    fn a_birch_door_fence_gate_and_boat_work_like_the_plain_ones() {
        use macroquad::math::{ivec3, IVec3, Vec3};
        let mut g = crate::game::tests::arena(601);
        let w = 1;
        // A door: placed, opened, shut, and broken whole.
        let floor = ivec3(3, 49, 0);
        g.place_door(id(w, part::DOOR), floor, IVec3::Y, STONE);
        let (bottom, top) = (floor + IVec3::Y, floor + IVec3::Y * 2);
        assert_eq!(door_base(g.world.get_v(bottom)), Some(id(w, part::DOOR)));
        assert_eq!(door_state(g.world.get_v(top)).map(|s| s.2), Some(true));
        g.toggle_door(top);
        assert!(door_state(g.world.get_v(bottom)).is_some_and(|s| s.1), "open");
        assert_eq!(door_base(g.world.get_v(bottom)), Some(id(w, part::DOOR)), "still birch");
        // Fences join birch to oak, and a birch gate swings.
        let a = ivec3(-3, 50, 3);
        g.world.set_v(a, id(w, part::FENCE));
        g.world.set_v(a + IVec3::X, FENCE_FIRST);
        assert_eq!(g.world.get_v(a), id(w, part::FENCE) + 1);
        assert_eq!(g.world.get_v(a + IVec3::X), FENCE_FIRST + 2);
        let gate = a - IVec3::X;
        g.world.set_v(gate, id(w, part::GATE));
        assert_eq!(g.world.get_v(a), id(w, part::FENCE) + 1 + 2, "fences join gates");
        assert!(g.toggle_hinged(gate));
        assert_eq!(g.world.get_v(gate), id(w, part::GATE) + 1);
        // A birch boat goes down on water and comes back as a birch boat.
        let kind = crate::vehicles::kind_of_item(BIRCH_BOAT).unwrap();
        assert!(crate::vehicles::is_boat(kind));
        let v = g.spawn_vehicle(kind, Vec3::new(0.5, 50.0, 0.5), 0.0);
        let boat = g.vehicles.iter().find(|x| x.id == v).unwrap();
        assert_eq!(boat.item(), BIRCH_BOAT);
        // Logs smelt to charcoal, and planks burn.
        assert_eq!(crate::containers::smelt(id(w, part::LOG)), Some(COAL));
        assert!(crate::containers::fuel_secs(id(w, part::PLANKS)).is_some());
    }
}
