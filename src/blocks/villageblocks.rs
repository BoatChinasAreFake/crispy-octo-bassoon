//! Village blocks (v0.5): the Cartography Table, the Cartographer's
//! workstation (which block is whose workstation is in village_life.rs),
//! the Notice Board (each village puts one up on its square: see
//! bounties.rs), the Shop Stall (see stalls.rs), and three blocks only villages have (see the styles in houses.rs): Mud
//! Bricks (desert walls), Thatch (savanna roofs) and Frost Shingles (snowy
//! roofs). There's no recipe for those: take them from a village (politely).

use crate::block::*;

pub fn defs() -> Vec<BlockDef> {
    use crate::texture::*;
    let table = def("cartography_table", "Cartography Table (Here Be Dragons)", Model::Cube, true, true, [T_CARTO_TOP, T_CARTO_SIDE, T_PLANKS], 2.5, 0, false, CARTOGRAPHY_TABLE, 0.0, S_WOOD);
    let mud = def("mud_bricks", "Mud Bricks (Sun-Baked, Slightly Crumbly)", Model::Cube, true, true, [T_MUD_BRICKS; 3], 1.5, 1, false, MUD_BRICKS, 0.0, S_STONE);
    let thatch = def("thatch", "Thatch (Keeps the Rain Off, Mostly)", Model::Cube, true, true, [T_THATCH_TOP, T_THATCH, T_THATCH_TOP], 0.6, 0, false, THATCH, 0.0, S_GRASS);
    let frost = def("frost_shingles", "Frost Shingles (Cold to the Touch)", Model::Cube, true, true, [T_FROST_SHINGLES; 3], 1.5, 1, false, FROST_SHINGLES, 0.0, S_STONE);
    let board = def("notice_board", "Notice Board (Help Wanted)", Model::Cube, true, true, [T_PLANKS, T_BOARD_FRONT, T_PLANKS], 2.5, 0, false, NOTICE_BOARD, 0.0, S_WOOD);
    let stall = def("shop_stall", "Shop Stall (Everything Must Go)", Model::Cube, true, true, [T_STALL_TOP, T_STALL_SIDE, T_PLANKS], 2.5, 0, false, SHOP_STALL, 0.0, S_WOOD);
    vec![table, mud, thatch, frost, board, stall]
}

pub fn recipes() -> Vec<Recipe> {
    let r = |inputs: &[(Id, u8)], output: (Id, u8)| Recipe { inputs: inputs.to_vec(), output };
    vec![r(&[(PLANKS, 4), (WHEAT, 2)], (CARTOGRAPHY_TABLE, 1)), r(&[(PLANKS, 6), (STICK, 2), (BOOK, 1)], (NOTICE_BOARD, 1)), r(&[(PLANKS, 4), (WOOL, 2), (CHEST, 1)], (SHOP_STALL, 1))]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_blocks_are_where_their_ids_say() {
        assert_eq!(block(CARTOGRAPHY_TABLE).key, "cartography_table");
        assert_eq!(block(MUD_BRICKS).key, "mud_bricks");
        assert_eq!(block(FROST_SHINGLES).drop, FROST_SHINGLES);
        assert_eq!(defs().len() as Id, NUM_BLOCKS - CARTOGRAPHY_TABLE);
        for (id, _) in recipes().iter().map(|r| r.output) {
            assert!(reg().recipes.iter().any(|r| r.output.0 == id), "{id} can be made");
        }
    }
}
