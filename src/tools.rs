//! Axes and shovels, and the copper tier.
//!
//! Every tool comes in five tiers: wood, stone, copper, iron and dimond.
//! Pickaxes speed up stone and ores (and decide what they drop), axes speed up
//! anything wooden, shovels anything earthy (dirt, grass, sand, gravel, snow,
//! mud). Copper sits between stone and iron: it mines what stone does, a bit
//! faster, and lasts longer.

use crate::block::*;

/// Speeds by tier: wood, stone, copper, iron, dimond, and Scorchite (see smithing.rs).
pub const SPEEDS: [f32; 6] = [2.0, 4.0, 5.0, 6.0, 8.0, 9.0];
/// Uses by tier (copper's is between stone and iron).
pub const USES: [u16; 6] = [59, 131, 190, 250, 1561, 2031];
pub const TIER_NAMES: [(&str, &str); 5] = [("wooden", "Wooden"), ("stone", "Stone"), ("copper", "Copper"), ("iron", "Iron"), ("diamond", "Dimond")];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Pick,
    Axe,
    Shovel,
}

/// Tier index (0 wood .. 4 dimond) of an axe.
pub fn axe_tier(id: Id) -> Option<usize> {
    if id == AXE_SCORCHITE {
        return Some(5);
    }
    (AXE_FIRST..AXE_FIRST + 5).contains(&id).then(|| (id - AXE_FIRST) as usize)
}

pub fn shovel_tier(id: Id) -> Option<usize> {
    if id == SHOVEL_SCORCHITE {
        return Some(5);
    }
    (SHOVEL_FIRST..SHOVEL_FIRST + 5).contains(&id).then(|| (id - SHOVEL_FIRST) as usize)
}

/// Which tool digs a block fastest (None: nothing helps).
pub fn best_tool(id: Id) -> Option<Tool> {
    let b = block(id);
    if b.hardness <= 0.0 {
        return None;
    }
    if b.pick_block {
        return Some(Tool::Pick);
    }
    if b.sound == 1 && b.model != Model::Cross {
        return Some(Tool::Axe);
    }
    if matches!(b.sound, 2 | 3) && b.model == Model::Cube && !is_leaves(id) {
        return Some(Tool::Shovel);
    }
    None
}

/// How much faster than bare hands `held` digs, if it's the right kind of tool.
pub fn speed(held: Id, kind: Tool) -> Option<f32> {
    let tier = match kind {
        Tool::Axe => axe_tier(held)?,
        Tool::Shovel => shovel_tier(held)?,
        Tool::Pick => {
            if held == PICK_COPPER {
                2
            } else if held == PICK_SCORCHITE {
                5
            } else {
                // Base and mod pickaxes: tier 1..4 is wood..dimond.
                let t = pick_tier(held) as usize;
                if t == 0 {
                    return None;
                }
                [0, 0, 1, 3, 4][t.min(4)]
            }
        }
    };
    Some(SPEEDS[tier])
}

/// Uses before a base-game tool breaks.
pub fn tool_uses(id: Id) -> Option<u16> {
    if let Some(t) = axe_tier(id).or_else(|| shovel_tier(id)) {
        return Some(USES[t]);
    }
    if matches!(id, PICK_SCORCHITE | SWORD_SCORCHITE) {
        return Some(USES[5]);
    }
    matches!(id, PICK_COPPER | SWORD_COPPER).then_some(USES[2])
}

/// Is it an axe or a shovel (for enchanting: Efficiency, Fortune and Unbreaking fit)?
pub fn is_digger(id: Id) -> bool {
    axe_tier(id).is_some() || shovel_tier(id).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_right_tool_is_faster() {
        let hand = break_time_with(LOG, AIR, 0).0;
        let axe = break_time_with(LOG, AXE_FIRST + 4, 0).0;
        let wrong = break_time_with(LOG, SHOVEL_FIRST + 4, 0).0;
        assert!(axe < hand / 4.0, "dimond axe {axe} vs hand {hand}");
        assert_eq!(wrong, hand, "a shovel doesn't help with wood");
        assert!(break_time_with(DIRT, SHOVEL_FIRST, 0).0 < break_time_with(DIRT, AIR, 0).0);
        assert!(break_time_with(SAND, SHOVEL_FIRST + 3, 0).0 < break_time_with(SAND, SHOVEL_FIRST + 1, 0).0);
        // Copper: faster than stone, slower than iron, and mines iron ore.
        let t = |p| break_time_with(IRON_ORE, p, 0);
        assert!(t(PICK_COPPER).0 < t(PICK_STONE).0 && t(PICK_COPPER).0 > t(PICK_IRON).0);
        assert!(t(PICK_COPPER).1);
        assert!(!break_time_with(DIAMOND_ORE, PICK_COPPER, 0).1, "not dimond");
        assert_eq!(best_tool(PLANKS), Some(Tool::Axe));
        assert_eq!(best_tool(GRAVEL), Some(Tool::Shovel));
        assert_eq!(best_tool(LEAVES), None);
    }
}
