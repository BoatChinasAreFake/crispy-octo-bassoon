//! Detailed models for blocks whose collision boxes are too plain to draw:
//! lanterns, the bell, the enchanting table, the grindstone, decorated pots,
//! scaffolding, chains, the Charred Skull, eye frames and the brewing stand.
//!
//! A model is a list of pieces: boxes with a tile for each face (and either
//! the tile cut to match where the box sits in its cell, or a set part of the
//! tile stretched over every face). Collision, targeting and placement still
//! use the block's shape (see `Shape::boxes`); this is only how it looks, in
//! the world, in the hand and in the inventory.

use crate::block::*;
use crate::texture::*;

/// One box of a model. Faces are in the mesher's order: +x, -x, +y, -y, +z, -z.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Piece {
    pub lo: [f32; 3],
    pub hi: [f32; 3],
    pub tiles: [u16; 6],
    /// `None`: each face shows the part of its tile under it (like a slab).
    /// Some(rect): every face shows this part of its tile (0..1 across, 0..1 down).
    pub uv: Option<[f32; 4]>,
    /// Faces not to draw (bit per face), for faces hidden inside the model or
    /// the empty sides of a flat sheet.
    pub hide: u8,
}

const fn px(v: f32) -> f32 {
    v / 16.0
}

/// A box from pixel coordinates (0..16), every face one tile.
const fn bx(lo: [f32; 3], hi: [f32; 3], tile: u16) -> Piece {
    Piece { lo: [px(lo[0]), px(lo[1]), px(lo[2])], hi: [px(hi[0]), px(hi[1]), px(hi[2])], tiles: [tile; 6], uv: None, hide: 0 }
}

impl Piece {
    /// The whole tile (or `rect` of it) on every face.
    const fn stretched(mut self, rect: [f32; 4]) -> Piece {
        self.uv = Some(rect);
        self
    }
    /// Top and bottom tiles (sides keep theirs).
    const fn caps(mut self, top: u16, bottom: u16) -> Piece {
        self.tiles[2] = top;
        self.tiles[3] = bottom;
        self
    }
    const fn face(mut self, f: usize, tile: u16) -> Piece {
        self.tiles[f] = tile;
        self
    }
    const fn hiding(mut self, mask: u8) -> Piece {
        self.hide = mask;
        self
    }
}

const ALL: [f32; 4] = [0.0, 0.0, 1.0, 1.0];
/// Faces of a flat sheet across x (only its two big sides show), and across z.
const SHEET_X: u8 = 0b111100;
const SHEET_Z: u8 = 0b001111;

/// A lantern's body and cap, `y` pixels off the floor.
fn lantern(body: u16, y: f32) -> Vec<Piece> {
    vec![
        bx([5.0, y, 5.0], [11.0, y + 7.0, 11.0], body).stretched(ALL).caps(T_LANTERN_CAP, T_LANTERN_CAP),
        bx([6.0, y + 7.0, 6.0], [10.0, y + 9.0, 10.0], T_LANTERN_CAP).stretched(ALL),
    ]
}

/// The model for a block, if it has one beyond its collision boxes.
pub fn pieces(id: Id) -> Option<Vec<Piece>> {
    let t = block(id).tex;
    Some(match id {
        LANTERN => lantern(T_LANTERN_BODY, 0.0),
        SOUL_LANTERN => lantern(T_SOUL_LANTERN_BODY, 0.0),
        LANTERN_HANGING => {
            let mut v = lantern(T_LANTERN_BODY, 1.0);
            // The handle up to whatever it hangs from.
            v.push(bx([8.0, 10.0, 6.5], [8.0, 16.0, 9.5], T_CHAIN).hiding(SHEET_X));
            v
        }
        BELL => vec![
            // A beam across the top, posts down to the floor, and the bell between.
            bx([0.0, 13.0, 7.0], [16.0, 15.0, 9.0], T_LOG_SIDE),
            bx([0.0, 0.0, 6.0], [2.0, 15.0, 10.0], T_COBBLE),
            bx([14.0, 0.0, 6.0], [16.0, 15.0, 10.0], T_COBBLE),
            bx([5.0, 6.0, 5.0], [11.0, 13.0, 11.0], T_BELL),
            bx([4.0, 4.0, 4.0], [12.0, 6.0, 12.0], T_BELL),
        ],
        ENCHANTING_TABLE => vec![
            bx([0.0, 0.0, 0.0], [16.0, 12.0, 16.0], t[1]).caps(t[0], t[2]),
            // The book, open, floating over it.
            bx([4.0, 13.5, 5.0], [12.0, 14.5, 11.0], T_ENCH_BOOK).stretched([0.0, 0.0, 1.0, 0.2]).face(2, T_ENCH_BOOK).caps(T_ENCH_BOOK, T_ENCH_BOOK),
        ],
        GRINDSTONE => vec![
            // A stone wheel on an axle, held up by two wooden legs.
            bx([4.0, 4.0, 2.0], [12.0, 16.0, 14.0], T_GRINDSTONE_TOP).face(0, T_GRINDSTONE_SIDE).face(1, T_GRINDSTONE_SIDE).stretched(ALL),
            bx([2.0, 0.0, 6.0], [4.0, 12.0, 10.0], T_GRIND_LEG),
            bx([12.0, 0.0, 6.0], [14.0, 12.0, 10.0], T_GRIND_LEG),
        ],
        _ if (POT_FIRST..POT_FIRST + 13).contains(&id) => vec![
            // A round-shouldered body, a neck and a lip.
            bx([1.0, 0.0, 1.0], [15.0, 12.0, 15.0], t[1]).stretched([0.0, 0.0, 1.0, 1.0]).caps(T_POT_TOP, T_POT_TOP),
            bx([4.0, 12.0, 4.0], [12.0, 14.0, 12.0], T_POT_NECK).stretched(ALL).hiding(1 << 3),
            bx([3.0, 14.0, 3.0], [13.0, 16.0, 13.0], T_POT_NECK).stretched(ALL).caps(T_POT_TOP, T_POT_NECK),
        ],
        SCAFFOLDING => vec![
            bx([0.0, 14.0, 0.0], [16.0, 16.0, 16.0], T_SCAFFOLD_TOP).face(0, T_SCAFFOLD_RIM).face(1, T_SCAFFOLD_RIM).face(4, T_SCAFFOLD_RIM).face(5, T_SCAFFOLD_RIM),
            bx([0.0, 0.0, 0.0], [2.0, 14.0, 2.0], T_SCAFFOLD_POST).hiding(1 << 2),
            bx([14.0, 0.0, 0.0], [16.0, 14.0, 2.0], T_SCAFFOLD_POST).hiding(1 << 2),
            bx([0.0, 0.0, 14.0], [2.0, 14.0, 16.0], T_SCAFFOLD_POST).hiding(1 << 2),
            bx([14.0, 0.0, 14.0], [16.0, 14.0, 16.0], T_SCAFFOLD_POST).hiding(1 << 2),
        ],
        CHAIN => vec![
            // Two crossed sheets of links, one a half-link down from the other.
            bx([8.0, 0.0, 5.0], [8.0, 16.0, 11.0], T_CHAIN).hiding(SHEET_X),
            bx([5.0, 0.0, 8.0], [11.0, 16.0, 8.0], T_CHAIN2).hiding(SHEET_Z),
        ],
        CHARRED_SKULL => vec![
            // A cranium facing south, and a jaw.
            bx([4.0, 2.0, 4.0], [12.0, 9.0, 12.0], T_CHARRED_SKULL).face(4, T_CHARRED_SKULL_FACE).stretched(ALL),
            bx([5.0, 0.0, 5.0], [11.0, 2.0, 12.0], T_CHARRED_SKULL).face(4, T_CHARRED_JAW).stretched(ALL),
        ],
        EYE_FRAME | EYE_FRAME_FULL => {
            let mut v = vec![bx([0.0, 0.0, 0.0], [16.0, 13.0, 16.0], T_EYE_FRAME_SIDE).caps(t[0], T_HOLLOW_STONE)];
            if id == EYE_FRAME_FULL {
                v.push(bx([4.0, 13.0, 4.0], [12.0, 16.0, 12.0], T_EYE_FRAME_EYE).stretched(ALL).hiding(1 << 3));
            }
            v
        }
        BREWING_STAND => vec![
            // A rod up the middle, three stone feet, and arms out to hold bottles over them.
            bx([7.0, 0.0, 7.0], [9.0, 14.0, 9.0], T_BREWING_ROD),
            bx([6.5, 14.0, 6.5], [9.5, 15.0, 9.5], T_BREWING_ROD),
            bx([9.0, 0.0, 5.0], [15.0, 2.0, 11.0], T_BREWING_BASE),
            bx([1.0, 0.0, 1.0], [7.0, 2.0, 7.0], T_BREWING_BASE),
            bx([1.0, 0.0, 9.0], [7.0, 2.0, 15.0], T_BREWING_BASE),
            bx([9.0, 10.0, 7.5], [12.0, 11.0, 8.5], T_BREWING_ROD),
            bx([3.5, 10.0, 7.5], [7.0, 11.0, 8.5], T_BREWING_ROD),
            bx([3.5, 10.0, 3.5], [4.5, 11.0, 12.5], T_BREWING_ROD),
        ],
        _ if crate::homecraft::is_cauldron(id) => {
            // Four walls on stubby feet, a floor, and the water (or dye) inside.
            let mut v = vec![
                bx([0.0, 3.0, 0.0], [16.0, 16.0, 2.0], T_CAULDRON_SIDE).caps(T_CAULDRON_TOP, T_CAULDRON_INNER),
                bx([0.0, 3.0, 14.0], [16.0, 16.0, 16.0], T_CAULDRON_SIDE).caps(T_CAULDRON_TOP, T_CAULDRON_INNER),
                bx([0.0, 3.0, 2.0], [2.0, 16.0, 14.0], T_CAULDRON_SIDE).caps(T_CAULDRON_TOP, T_CAULDRON_INNER),
                bx([14.0, 3.0, 2.0], [16.0, 16.0, 14.0], T_CAULDRON_SIDE).caps(T_CAULDRON_TOP, T_CAULDRON_INNER),
                bx([2.0, 3.0, 2.0], [14.0, 4.0, 14.0], T_CAULDRON_INNER),
                bx([0.0, 0.0, 0.0], [4.0, 3.0, 4.0], T_CAULDRON_SIDE),
                bx([12.0, 0.0, 0.0], [16.0, 3.0, 4.0], T_CAULDRON_SIDE),
                bx([0.0, 0.0, 12.0], [4.0, 3.0, 16.0], T_CAULDRON_SIDE),
                bx([12.0, 0.0, 12.0], [16.0, 3.0, 16.0], T_CAULDRON_SIDE),
            ];
            let (level, dye) = crate::homecraft::cauldron_state(id);
            if level > 0 {
                let top = 4.0 + level as f32 * 3.6;
                let tile = dye.map_or(T_WATER, |c| T_DYED_WATER + c as u16);
                v.push(bx([2.0, top - 0.5, 2.0], [14.0, top, 14.0], tile).hiding(!(1 << 2)));
            }
            v
        }
        _ if crate::homecraft::is_composter(id) => {
            let level = (id - COMPOSTER) as f32;
            let mut v = vec![
                bx([0.0, 0.0, 0.0], [16.0, 16.0, 2.0], T_COMPOSTER_SIDE).caps(T_COMPOSTER_TOP, T_COMPOSTER_SIDE),
                bx([0.0, 0.0, 14.0], [16.0, 16.0, 16.0], T_COMPOSTER_SIDE).caps(T_COMPOSTER_TOP, T_COMPOSTER_SIDE),
                bx([0.0, 0.0, 2.0], [2.0, 16.0, 14.0], T_COMPOSTER_SIDE).caps(T_COMPOSTER_TOP, T_COMPOSTER_SIDE),
                bx([14.0, 0.0, 2.0], [16.0, 16.0, 14.0], T_COMPOSTER_SIDE).caps(T_COMPOSTER_TOP, T_COMPOSTER_SIDE),
                bx([2.0, 0.0, 2.0], [14.0, 2.0, 14.0], T_COMPOSTER_SIDE).caps(T_COMPOSTER_SIDE, T_COMPOSTER_SIDE),
            ];
            if level > 0.0 {
                let tile = if level >= 7.0 { T_COMPOSTER_READY } else { T_COMPOSTER_FILL };
                v.push(bx([2.0, 2.0, 2.0], [14.0, 2.0 + level * 1.85, 14.0], tile).hiding(0b110011));
            }
            v
        }
        _ if crate::homecraft::is_head(id) => vec![
            // A head facing south, sitting on the floor.
            bx([4.0, 0.0, 4.0], [12.0, 8.0, 12.0], t[0]).face(4, t[1]).stretched(ALL),
        ],
        LIGHTNING_ROD | LIGHTNING_ROD_ON => vec![
            // A thin copper rod with a knob on top.
            bx([7.0, 0.0, 7.0], [9.0, 13.0, 9.0], t[0]),
            bx([6.0, 13.0, 6.0], [10.0, 16.0, 10.0], t[0]).stretched(ALL),
        ],
        _ => return None,
    })
}

/// Blocks shown as a flat picture in the inventory and the hand (as Minecraft
/// does for lanterns, chains and the like), and that picture.
pub fn flat_icon(id: Id) -> Option<u16> {
    match id {
        LANTERN | LANTERN_HANGING => Some(T_LANTERN),
        SOUL_LANTERN => Some(T_SOUL_LANTERN),
        BELL => Some(T_BELL_ITEM),
        CHAIN => Some(T_CHAIN),
        BREWING_STAND => Some(T_BREWING_ITEM),
        LIGHTNING_ROD | LIGHTNING_ROD_ON => Some(T_LIGHTNING_ROD_ITEM),
        _ => None,
    }
}

/// Is this item drawn as a little block (in the hand, the inventory, on the
/// ground)? Otherwise it's a flat picture.
pub fn drawn_as_block(id: Id) -> bool {
    is_block_item(id) && matches!(block(id).model, Model::Cube | Model::Shaped) && !matches!(block(id).shape, Shape::Dust) && flat_icon(id).is_none()
}

/// The picture for an item drawn flat.
pub fn flat_tile(id: Id) -> u16 {
    flat_icon(id).unwrap_or_else(|| if is_block_item(id) { block(id).tex[1] } else { item_tile(id) })
}

/// A box (lo, hi), its six face tiles and the part of the tiles to show.
pub type ItemPart = (([f32; 3], [f32; 3]), [u16; 6], [f32; 4]);

/// A block item's boxes for drawing it small: each with its face tiles (the
/// mesher's order) and the part of the tiles to show.
pub fn item_parts(id: Id) -> Vec<ItemPart> {
    if let Some(v) = pieces(id) {
        return v.iter().filter(|p| p.hide == 0).map(|p| ((p.lo, p.hi), p.tiles, p.uv.unwrap_or([p.lo[0], 1.0 - p.hi[1], p.hi[0], 1.0 - p.lo[1]]))).collect();
    }
    let t = block(id).tex;
    let (boxes, n) = block_boxes(id);
    boxes[..n].iter().map(|&(a, b)| ((a, b), [t[1], t[1], t[0], t[2], t[1], t[1]], [a[0], 1.0 - b[1], b[0], 1.0 - a[1]])).collect()
}

/// Where a face's corner `p` (0..1 in the cell) lands in its tile, for face `f`.
pub fn face_uv(piece: &Piece, f: usize, p: [f32; 3]) -> [f32; 2] {
    match piece.uv {
        None => match f {
            0 => [1.0 - p[2], 1.0 - p[1]],
            1 => [p[2], 1.0 - p[1]],
            2 => [p[0], p[2]],
            3 => [p[0], 1.0 - p[2]],
            4 => [p[0], 1.0 - p[1]],
            _ => [1.0 - p[0], 1.0 - p[1]],
        },
        Some([u0, v0, u1, v1]) => {
            // Where the corner sits across the face, 0..1 each way.
            let (lo, hi) = (piece.lo, piece.hi);
            let along = |k: usize| if hi[k] > lo[k] { (p[k] - lo[k]) / (hi[k] - lo[k]) } else { 0.0 };
            let (s, t) = match f {
                0 => (1.0 - along(2), 1.0 - along(1)),
                1 => (along(2), 1.0 - along(1)),
                2 => (along(0), along(2)),
                3 => (along(0), 1.0 - along(2)),
                4 => (along(0), 1.0 - along(1)),
                _ => (1.0 - along(0), 1.0 - along(1)),
            };
            [u0 + s * (u1 - u0), v0 + t * (v1 - v0)]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_stay_inside_their_cell_and_have_something_to_show() {
        for id in [LANTERN, SOUL_LANTERN, LANTERN_HANGING, BELL, ENCHANTING_TABLE, GRINDSTONE, POT_FIRST, POT_FIRST + 12, SCAFFOLDING, CHAIN, CHARRED_SKULL, EYE_FRAME, EYE_FRAME_FULL, BREWING_STAND] {
            let v = pieces(id).unwrap_or_else(|| panic!("{id} has a model"));
            assert!(!v.is_empty());
            for p in &v {
                for k in 0..3 {
                    assert!(p.lo[k] >= 0.0 && p.hi[k] <= 1.0 && p.lo[k] <= p.hi[k], "{id}: {p:?}");
                }
                assert!(p.hide != 0b111111, "{id}: a piece with nothing to draw");
            }
        }
        // Scaffolding stands on four legs.
        assert_eq!(pieces(SCAFFOLDING).unwrap().iter().filter(|p| p.hi[1] < 1.0 && p.lo[1] == 0.0).count(), 4);
        assert!(pieces(STONE).is_none());
    }

    #[test]
    fn a_stretched_face_covers_its_whole_rect() {
        let p = bx([4.0, 0.0, 4.0], [12.0, 8.0, 12.0], T_STONE).stretched([0.0, 0.0, 1.0, 1.0]);
        assert_eq!(face_uv(&p, 4, [0.25, 0.0, 0.75]), [0.0, 1.0]);
        assert_eq!(face_uv(&p, 4, [0.75, 0.5, 0.75]), [1.0, 0.0]);
    }
}
