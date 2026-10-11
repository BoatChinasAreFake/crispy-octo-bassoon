//! Village blocks (v0.5): the Cartography Table, the Cartographer's
//! workstation. (Which block is whose workstation is in village_life.rs.)

use crate::block::*;

pub fn defs() -> Vec<BlockDef> {
    use crate::texture::*;
    let table = def("cartography_table", "Cartography Table (Here Be Dragons)", Model::Cube, true, true, [T_CARTO_TOP, T_CARTO_SIDE, T_PLANKS], 2.5, 0, false, CARTOGRAPHY_TABLE, 0.0, S_WOOD);
    vec![table]
}

pub fn recipes() -> Vec<Recipe> {
    let r = |inputs: &[(Id, u8)], output: (Id, u8)| Recipe { inputs: inputs.to_vec(), output };
    vec![r(&[(PLANKS, 4), (WHEAT, 2)], (CARTOGRAPHY_TABLE, 1))]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_blocks_are_where_their_ids_say() {
        assert_eq!(block(CARTOGRAPHY_TABLE).key, "cartography_table");
        assert_eq!(defs().len() as Id, NUM_BLOCKS - CARTOGRAPHY_TABLE);
        for (id, _) in recipes().iter().map(|r| r.output) {
            assert!(reg().recipes.iter().any(|r| r.output.0 == id), "{id} can be made");
        }
    }
}
