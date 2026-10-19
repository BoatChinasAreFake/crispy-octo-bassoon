//! Coloured beds and candles (v0.8), in the eight colours of carpentry.rs.
//!
//! The plain bed's blanket is red, so the coloured beds are the other seven;
//! candles come plain or in any of the eight. A coloured bed is made like a
//! plain one, from that colour of wool; a candle and a dye make a coloured
//! candle. They behave exactly like the plain ones (sleeping, lighting).
//!
//! Blocks, from `DYED_FIRST`: the beds (`+ k * 4 + facing`, `k` counting the
//! colours but red), then the candles (`+ colour * 2 + lit`).

use crate::block::*;
use crate::carpentry::COLOURS;
use crate::texture::*;

/// The plain bed's colour.
pub const RED: usize = 2;
const BEDS: Id = 7 * 4;
pub const COUNT: Id = BEDS + 8 * 2;

fn bed_slot(colour: usize) -> usize {
    if colour > RED { colour - 1 } else { colour }
}

/// The bed of `colour` facing `f` (red is the plain bed).
pub fn bed(colour: usize, f: u8) -> Id {
    if colour % 8 == RED {
        return crate::beds::bed(f);
    }
    DYED_FIRST + bed_slot(colour % 8) as Id * 4 + (f % 4) as Id
}

/// A coloured bed's colour and facing.
pub fn bed_of(id: Id) -> Option<(usize, u8)> {
    (DYED_FIRST..DYED_FIRST + BEDS).contains(&id).then(|| {
        let k = ((id - DYED_FIRST) / 4) as usize;
        (if k >= RED { k + 1 } else { k }, ((id - DYED_FIRST) % 4) as u8)
    })
}

/// A bed's colour (any bed).
pub fn bed_colour(id: Id) -> usize {
    bed_of(id).map(|b| b.0).unwrap_or(RED)
}

/// The candle of `colour` (None: plain), lit or not.
pub fn candle(colour: Option<usize>, lit: bool) -> Id {
    match colour {
        None => if lit { CANDLE_LIT } else { CANDLE },
        Some(c) => DYED_FIRST + BEDS + (c % 8) as Id * 2 + lit as Id,
    }
}

/// Any candle's (colour, lit).
pub fn candle_of(id: Id) -> Option<(Option<usize>, bool)> {
    match id {
        CANDLE => Some((None, false)),
        CANDLE_LIT => Some((None, true)),
        _ if (DYED_FIRST + BEDS..DYED_FIRST + COUNT).contains(&id) => {
            let k = id - DYED_FIRST - BEDS;
            Some((Some((k / 2) as usize), k % 2 == 1))
        }
        _ => None,
    }
}

pub fn is_candle(id: Id) -> bool {
    candle_of(id).is_some()
}

pub fn defs() -> Vec<BlockDef> {
    let mut out = Vec::new();
    for c in (0..8).filter(|&c| c != RED) {
        let (k, n) = COLOURS[c];
        let s = bed_slot(c) as u16;
        for f in 0..4u8 {
            let key = format!("{k}_bed{}", ["", "_east", "_south", "_west"][f as usize]);
            let mut b = def(leak(&key), leak(&format!("{n} Bed")), Model::Shaped, true, false, [T_DYED_BED_TOP + s * 4 + f as u16, T_DYED_BED_SIDE + s, T_PLANKS], 0.4, 0, false, bed(c, 0), 0.0, S_WOOD);
            b.shape = Shape::Bed { facing: f };
            b.creative = f == 0;
            out.push(b);
        }
    }
    for (c, (k, n)) in COLOURS.iter().enumerate() {
        for lit in [false, true] {
            let key = format!("{k}_candle{}", if lit { "_lit" } else { "" });
            let mut d = def(leak(&key), leak(&format!("{n} Candle{}", if lit { " (Lit)" } else { "" })), Model::Shaped, false, false, [T_DYED_CANDLE + c as u16 * 2 + lit as u16; 3], 0.1, 0, false, candle(Some(c), false), if lit { 6.0 } else { 0.0 }, S_GRASS);
            d.shape = Shape::Candle;
            d.creative = !lit;
            out.push(d);
        }
    }
    out
}

pub fn recipes() -> Vec<Recipe> {
    let r = |inputs: &[(Id, u8)], output: (Id, u8)| Recipe { inputs: inputs.to_vec(), output };
    let mut out = Vec::new();
    for c in 0..8 {
        let dye = DYE_FIRST + c as Id;
        out.push(r(&[(CANDLE, 1), (dye, 1)], (candle(Some(c), false), 1)));
        if c != RED {
            // (Coloured wool: plain wool is white.)
            let wool = if c == 0 { WOOL } else { DYED_WOOL + c as Id - 1 };
            out.push(r(&[(wool, 3), (PLANKS, 3)], (bed(c, 0), 1)));
            out.push(r(&[(BED, 1), (dye, 1)], (bed(c, 0), 1)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_colour_has_a_bed_and_a_candle() {
        for c in 0..8 {
            for f in 0..4 {
                let b = bed(c, f);
                assert!(crate::beds::is_bed(b));
                assert_eq!(crate::beds::facing(b), f);
                assert_eq!(bed_colour(b), c);
                assert_eq!(placing_item(b), Some(bed(c, 0)));
            }
            for lit in [false, true] {
                let k = candle(Some(c), lit);
                assert_eq!(candle_of(k), Some((Some(c), lit)));
                assert_eq!(placing_item(k), Some(candle(Some(c), false)));
            }
            assert_eq!(crate::light::emission(candle(Some(c), true)), crate::light::emission(CANDLE_LIT));
        }
        assert_eq!(block(bed(6, 3)).key, "blue_bed_west");
        assert_eq!(block(candle(Some(1), true)).key, "black_candle_lit");
        assert_eq!(DYED_FIRST + COUNT, STRUCTURE_BLOCK);
    }

    #[test]
    fn a_blue_bed_goes_down_facing_you_and_a_green_candle_lights() {
        use macroquad::math::{ivec3, IVec3};
        let mut g = crate::game::tests::arena(828);
        g.inv.slots[g.inv.selected] = Some((bed(6, 0), 1));
        let b = g.oriented(bed(6, 0), IVec3::Y, 0.5);
        assert_eq!(bed_colour(b), 6);
        assert_eq!(crate::beds::facing(b), g.facing());
        let p = ivec3(2, 50, 2);
        g.world.set_v(p, candle(Some(5), false));
        g.inv.slots[g.inv.selected] = Some((TORCH, 1));
        assert!(g.use_candle(p));
        assert_eq!(g.world.get_v(p), candle(Some(5), true));
        g.inv.slots[g.inv.selected] = None;
        assert!(g.use_candle(p));
        assert_eq!(g.world.get_v(p), candle(Some(5), false));
        // A night in it.
        let at = ivec3(4, 50, 4);
        g.world.set_v(at, b);
        g.sleeping = Some(crate::beds::Sleep { bed: at, secs: 0.0 });
        g.finish_night();
        assert!(g.advancements.has("colour_coordinated"));
    }
}
