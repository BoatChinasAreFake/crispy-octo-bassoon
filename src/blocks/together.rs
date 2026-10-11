//! The blocks for playing together (v0.4's last part): the Trading Table
//! (see trading.rs), the Survey Post (see claims.rs) and the Post Box (see
//! post.rs). Their looks and recipes are here; what they do is in those files.

use crate::block::*;

pub fn defs() -> Vec<BlockDef> {
    use crate::texture::*;
    let mut table = def("trading_table", "Trading Table (No Takebacks)", Model::Shaped, true, false, [T_TRADE_TOP, T_TRADE_SIDE, T_PLANKS], 2.0, 0, false, TRADING_TABLE, 0.0, S_WOOD);
    table.shape = Shape::Table;
    let post = def("survey_post", "Survey Post (This Is Mine Now)", Model::Cube, true, true, [T_SURVEY_TOP, T_SURVEY_SIDE, T_SURVEY_TOP], 1.5, 0, false, SURVEY_POST, 0.0, S_WOOD);
    let post_box = def("post_box", "Post Box (Cluckster Delivers)", Model::Cube, true, true, [T_POSTBOX_TOP, T_POSTBOX_FRONT, T_POSTBOX_TOP], 2.5, 1, false, POST_BOX, 0.0, S_STONE);
    vec![table, post, post_box]
}

pub fn recipes() -> Vec<Recipe> {
    let r = |inputs: &[(Id, u8)], output: (Id, u8)| Recipe { inputs: inputs.to_vec(), output };
    vec![
        r(&[(PLANKS, 4), (GOLD_INGOT, 2), (CHEST, 1)], (TRADING_TABLE, 1)),
        r(&[(LOG, 2), (COMPASS, 1), (STICK, 2)], (SURVEY_POST, 1)),
        r(&[(IRON, 5), (CHEST, 1), (FEATHER, 1)], (POST_BOX, 1)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_blocks_are_where_their_ids_say() {
        assert_eq!(block(TRADING_TABLE).key, "trading_table");
        assert_eq!(block(SURVEY_POST).key, "survey_post");
        assert_eq!(block(POST_BOX).key, "post_box");
        for (id, _) in recipes().iter().map(|r| r.output) {
            assert!(reg().recipes.iter().any(|r| r.output.0 == id), "{id} can be made");
        }
    }
}
