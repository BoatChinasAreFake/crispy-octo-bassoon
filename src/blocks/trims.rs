//! Armour trims: a bit of decoration for your armour, done at the Smithing
//! Table.
//!
//! Put a **trim template** (found in Trial Chambers' chests and vaults), a
//! piece of armour and a trim material (iron, gold, dimond or copper) on the
//! table: out comes the armour with the template's pattern picked out in
//! that material. Coast is a band round the middle, Wild a band along the
//! bottom, Ward bands top and bottom, Spire a stripe down the front. A new
//! trim replaces the old. Templates can be copied: seven dimonds, the
//! template and some tuff bricks make two.
//!
//! The trim lives in the armour's wear: bit 31 says it's trimmed, and bits
//! 12-15 say which (so a trimmed piece counts its uses in 11 bits, which is
//! plenty for armour). Other players see your trims (`Msg::PlayerState`).
//!
//! Woolly armour can't be trimmed, but it can be dyed in a cauldron (see
//! homecraft.rs): bit 31 and bit 11 (`DYED`) set, and bits 12-14 the colour.

use crate::block::*;
use crate::inventory::Wear;

/// Set on a trimmed piece's wear.
pub const TRIMMED: Wear = 0x8000_0000;
const SHIFT: u32 = 12;
/// Set (with `TRIMMED`) on dyed woolly armour.
pub const DYED: Wear = 0x800;
/// What a trim can be picked out in (with their spark-free tile colours).
pub const MATERIALS: [Id; 4] = [IRON, GOLD_INGOT, DIAMOND, COPPER_INGOT];
pub const PATTERN_NAMES: [&str; TRIMS] = ["Coast", "Wild", "Ward", "Spire"];
const MATERIAL_NAMES: [&str; 4] = ["Iron", "Gold", "Dimond", "Copper"];
/// Tiles for each material (see texture.rs): iron, gold, dimond, copper.
pub const MATERIAL_TILES: [u16; 4] = [crate::texture::T_TRIM_FIRST, crate::texture::T_TRIM_FIRST + 1, crate::texture::T_TRIM_FIRST + 2, crate::texture::T_TRIM_FIRST + 3];

/// (pattern, material index) of a trimmed piece.
pub fn trim_of(w: Wear) -> Option<(usize, usize)> {
    (w & TRIMMED != 0 && w & DYED == 0).then(|| {
        let t = ((w >> SHIFT) & 0xF) as usize;
        (t / 4, t % 4)
    })
}

/// The same piece with this trim (uses and enchantments kept; uses are capped to 12 bits).
pub fn with_trim(w: Wear, pattern: usize, material: usize) -> Wear {
    let uses = crate::inventory::uses(w).min(0x7FF) as Wear;
    let t = ((pattern % TRIMS) * 4 + material % 4) as Wear;
    (w & 0x7FFF_0000) | TRIMMED | (t << SHIFT) | uses
}

/// Woolly armour (the only kind that takes a dye).
pub fn is_woolly(id: Id) -> bool {
    (ARMOR_FIRST..ARMOR_FIRST + 4).contains(&id)
}

/// The colour a piece of woolly armour is dyed (see carpentry::COLOURS).
pub fn dye_of(w: Wear) -> Option<usize> {
    (w & TRIMMED != 0 && w & DYED != 0).then_some(((w >> SHIFT) & 7) as usize)
}

/// The same piece dyed `colour`, or washed clean (None). Uses and enchantments kept.
pub fn with_dye(w: Wear, colour: Option<usize>) -> Wear {
    let uses = crate::inventory::uses(w).min(0x7FF) as Wear;
    match colour {
        Some(c) => (w & 0x7FFF_0000) | TRIMMED | DYED | (((c % 8) as Wear) << SHIFT) | uses,
        None => (w & 0x7FFF_0000) | uses,
    }
}

pub fn material_index(id: Id) -> Option<usize> {
    MATERIALS.iter().position(|&m| m == id)
}

pub fn is_template(id: Id) -> bool {
    (TRIM_FIRST..TRIM_FIRST + TRIMS as Id).contains(&id)
}

/// "Coast trim (Gold)".
pub fn describe(w: Wear) -> Option<String> {
    trim_of(w).map(|(p, m)| format!("{} trim ({})", PATTERN_NAMES[p], MATERIAL_NAMES[m]))
}

/// What the Smithing Table makes from a template, armour and material.
pub fn trimmed(template: Id, armour: Id, wear: Wear, material: Id) -> Option<Wear> {
    if !is_template(template) || armor_of(armour).is_none() {
        return None;
    }
    // (A Glider isn't armour you can trim, and woolly armour is dyed instead.)
    if armour == GLIDER || is_woolly(armour) {
        return None;
    }
    Some(with_trim(wear, (template - TRIM_FIRST) as usize, material_index(material)?))
}

/// Per armour slot, 5 bits: 0 for none, else the trim + 1, or 17 + a woolly
/// piece's dye (for the network and drawing).
pub fn look(armour: &[Option<(Id, u8)>], wear: &[Wear]) -> u32 {
    let mut v = 0;
    for (i, (s, w)) in armour.iter().zip(wear).enumerate() {
        if s.is_some()
            && let Some((p, m)) = trim_of(*w)
        {
            v |= ((p * 4 + m + 1) as u32) << (i * 5);
        }
        if s.is_some_and(|s| is_woolly(s.0))
            && let Some(c) = dye_of(*w)
        {
            v |= ((17 + c) as u32) << (i * 5);
        }
    }
    v
}

/// "Dyed Red".
pub fn describe_dye(w: Wear) -> Option<String> {
    dye_of(w).map(|c| format!("Dyed {}", crate::carpentry::COLOURS[c].1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{uses, with_uses};

    #[test]
    fn trims_keep_wear_and_enchantments() {
        let w = crate::enchant::with_level(with_uses(0, 300), crate::enchant::Enchant::Protection, 3);
        let t = trimmed(TRIM_FIRST + 2, ARMOR_FIRST + 13, w, GOLD_INGOT).expect("a trim");
        assert_eq!(trim_of(t), Some((2, 1)));
        assert_eq!(uses(t), 300);
        assert_eq!(crate::enchant::level(t, crate::enchant::Enchant::Protection), 3);
        assert_eq!(crate::enchant::enchants(t), crate::enchant::enchants(w), "the trim isn't an enchantment");
        // Wearing it down keeps the trim.
        let worn = with_uses(t, 301);
        assert_eq!((uses(worn), trim_of(worn)), (301, Some((2, 1))));
        assert_eq!(describe(worn).as_deref(), Some("Ward trim (Gold)"));
        assert!(trimmed(TRIM_FIRST, SWORD_IRON, 0, IRON).is_none(), "armour only");
        assert!(trimmed(TRIM_FIRST, ARMOR_FIRST, 0, STONE).is_none(), "trim materials only");
        assert_eq!(look(&[Some((ARMOR_FIRST + 4, 1)), None], &[t, 0]), (2 * 4 + 1 + 1));
        assert!(trimmed(TRIM_FIRST, ARMOR_FIRST, 0, IRON).is_none(), "woolly armour is dyed, not trimmed");
    }

    #[test]
    fn dyes_keep_wear_and_wash_out() {
        let w = crate::enchant::with_level(with_uses(0, 70), crate::enchant::Enchant::Protection, 2);
        let red = with_dye(w, Some(2));
        assert_eq!((dye_of(red), trim_of(red), uses(red)), (Some(2), None, 70));
        assert_eq!(crate::enchant::level(red, crate::enchant::Enchant::Protection), 2);
        let worn = with_uses(red, 71);
        assert_eq!((dye_of(worn), uses(worn)), (Some(2), 71));
        assert_eq!(describe_dye(worn).as_deref(), Some("Dyed Red"));
        let clean = with_dye(worn, None);
        assert_eq!((dye_of(clean), uses(clean)), (None, 71));
        assert_eq!(crate::enchant::enchants(clean), crate::enchant::enchants(w));
        assert_eq!(look(&[Some((ARMOR_FIRST + 1, 1))], &[red]), 17 + 2);
        assert_eq!(dye_of(0), None);
    }

    #[test]
    fn the_smithing_table_trims_armour() {
        use crate::smithing::{plan, Bench};
        let slots = [Some((TRIM_FIRST + 3, 1)), Some((ARMOR_FIRST + 4, 1)), Some((COPPER_INGOT, 1))];
        let (out, w, _) = plan(Bench::Smithing, &slots, &[0, with_uses(0, 12), 0]).expect("trimmed");
        assert_eq!(out, ARMOR_FIRST + 4);
        assert_eq!((trim_of(w), uses(w)), (Some((3, 3)), 12));
        // The upgrade still works as before.
        let up = [Some((UPGRADE_TEMPLATE, 1)), Some((PICK_DIAMOND, 1)), Some((SCORCHITE_INGOT, 1))];
        assert_eq!(plan(Bench::Smithing, &up, &[0; 3]).map(|p| p.0), Some(PICK_SCORCHITE));
    }
}
