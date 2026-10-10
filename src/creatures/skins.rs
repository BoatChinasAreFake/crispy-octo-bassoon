//! Your own skin: a 64x64 PNG in the usual layout (head, body, arms and legs
//! each unfolded as a box, with a second layer on top for hats, jackets and
//! sleeves). Old 64x32 skins work too (their left arm and leg copy the right).
//!
//! - Put PNGs in the `skins` folder (beside the settings; the game makes it,
//!   with `template.png` showing the layout as Stove) and pick one at
//!   Options > Skin, after the six built-in looks.
//! - The game sends it to the host, which checks its size and passes it to
//!   everyone else (and to anyone who joins later). The host keeps no copy
//!   once you leave.
//! - Each skin in sight is cut into 36 tiles (six faces of six body parts)
//!   at the very end of the texture atlas: room for `SLOTS` skins at once
//!   (more than that, and the rest fall back to their built-in look).

use crate::entity::{humanoid, Limb, Part};
use crate::game::Game;
use crate::net::{Msg, SKIN_BYTES};
use crate::texture::{ATLAS, TILE, TILES_PER_ROW};

/// Skins shown at once.
pub const SLOTS: u16 = 16;
const TILES_PER_SKIN: u16 = 36;
/// Where skin tiles start: the end of the atlas.
pub const FIRST_SKIN_TILE: u16 = TILES_PER_ROW * TILES_PER_ROW - SLOTS * TILES_PER_SKIN;
/// The folder skins are picked from.
pub const SKINS_DIR: &str = "skins";
/// Whose skin a slot shows: this key for the local player.
pub const ME: u32 = u32::MAX - 1;

/// A body part's box on the skin: (u, v, width, height, depth) and where its
/// second layer is.
struct Box {
    at: (usize, usize),
    size: (usize, usize, usize),
    over: (usize, usize),
}

/// The parts in `humanoid` order: left leg, right leg, body, left arm, right arm, head.
const BOXES: [Box; 6] = [
    Box { at: (16, 48), size: (4, 12, 4), over: (0, 48) },
    Box { at: (0, 16), size: (4, 12, 4), over: (0, 32) },
    Box { at: (16, 16), size: (8, 12, 4), over: (16, 32) },
    Box { at: (32, 48), size: (4, 12, 4), over: (48, 48) },
    Box { at: (40, 16), size: (4, 12, 4), over: (40, 32) },
    Box { at: (0, 0), size: (8, 8, 8), over: (32, 0) },
];

/// A box's six faces on the skin, in tile order (+x, -x, +y, -y, +z, -z;
/// the model faces -z, so +x is its right side), and whether each is drawn
/// turned round (tops and bottoms).
fn faces(b: &Box, (u, v): (usize, usize)) -> [((usize, usize, usize, usize), bool); 6] {
    let (w, h, d) = b.size;
    [
        ((u, v + d, d, h), false),
        ((u + d + w, v + d, d, h), false),
        ((u + d, v, w, d), true),
        ((u + d + w, v, w, d), true),
        ((u + 2 * d + w, v + d, w, h), false),
        ((u + d, v + d, w, h), false),
    ]
}

/// Read a skin PNG: 64x64 RGBA (a 64x32 one is filled out).
pub fn load_png(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let img = macroquad::texture::Image::from_file_with_format(bytes, Some(macroquad::prelude::ImageFormat::Png)).map_err(|e| format!("not a PNG we can read ({e})"))?;
    normalize(img.width as usize, img.height as usize, &img.bytes)
}

/// Make any 64x64 or 64x32 RGBA picture a proper 64x64 skin.
pub fn normalize(w: usize, h: usize, rgba: &[u8]) -> Result<Vec<u8>, String> {
    if w != 64 || (h != 64 && h != 32) || rgba.len() != w * h * 4 {
        return Err(format!("a skin must be 64x64 pixels (this is {w}x{h})"));
    }
    let mut px = vec![0u8; SKIN_BYTES];
    px[..rgba.len()].copy_from_slice(rgba);
    if h == 32 {
        // An old skin: the left arm and leg are the right ones again.
        let copy = |px: &mut Vec<u8>, from: (usize, usize), to: (usize, usize)| {
            for y in 0..16 {
                for x in 0..16 {
                    let (s, d) = (((from.1 + y) * 64 + from.0 + x) * 4, ((to.1 + y) * 64 + to.0 + x) * 4);
                    let p: [u8; 4] = px[s..s + 4].try_into().unwrap();
                    px[d..d + 4].copy_from_slice(&p);
                }
            }
        };
        copy(&mut px, (40, 16), (32, 48));
        copy(&mut px, (0, 16), (16, 48));
        // Many old skins fill the hat with one solid colour: then it isn't a hat.
        let hat: Vec<[u8; 4]> = (8..16).flat_map(|y| (32..64).map(move |x| (x, y))).map(|(x, y)| px[(y * 64 + x) * 4..(y * 64 + x) * 4 + 4].try_into().unwrap()).collect();
        if hat.iter().all(|p| p[3] == 255 && *p == hat[0]) {
            for y in 0..16 {
                for x in 32..64 {
                    px[(y * 64 + x) * 4 + 3] = 0;
                }
            }
        }
    }
    Ok(px)
}

/// The 36 tiles cut from a skin (each part's six faces, second layer on top).
pub fn tiles(skin: &[u8]) -> Vec<[u8; TILE * TILE * 4]> {
    let at = |x: usize, y: usize| -> [u8; 4] {
        let i = (y.min(63) * 64 + x.min(63)) * 4;
        skin.get(i..i + 4).map(|p| p.try_into().unwrap()).unwrap_or([0; 4])
    };
    let mut out = Vec::with_capacity(36);
    for b in &BOXES {
        let base = faces(b, b.at);
        let over = faces(b, b.over);
        for f in 0..6 {
            let ((x, y, w, h), turned) = base[f];
            let ((ox, oy, _, _), _) = over[f];
            let mut t = [0u8; TILE * TILE * 4];
            for ty in 0..TILE {
                for tx in 0..TILE {
                    let (sx, sy) = if turned { (TILE - 1 - tx, TILE - 1 - ty) } else { (tx, ty) };
                    let (dx, dy) = (sx * w / TILE, sy * h / TILE);
                    let mut p = at(x + dx, y + dy);
                    p[3] = 255;
                    let o = at(ox + dx, oy + dy);
                    if o[3] >= 128 {
                        p = [o[0], o[1], o[2], 255];
                    }
                    t[(ty * TILE + tx) * 4..(ty * TILE + tx) * 4 + 4].copy_from_slice(&p);
                }
            }
            out.push(t);
        }
    }
    out
}

/// Tile of part `part`'s face `face` in skin slot `slot`.
pub const fn tile(slot: u16, part: u16, face: u16) -> u16 {
    FIRST_SKIN_TILE + slot * TILES_PER_SKIN + part * 6 + face
}

/// The body for skin slot `slot`.
pub fn model(slot: u16) -> [Part; 6] {
    let mut m = humanoid(0, 0, 0, 0, Limb::Swing(-1.0), Limb::Swing(1.0));
    for (p, part) in m.iter_mut().enumerate() {
        part.tiles = std::array::from_fn(|f| tile(slot, p as u16, f as u16));
    }
    m
}

/// Paint the skins in `slots` into the atlas.
pub fn paint(atlas: &mut [u8], slots: &[(u16, &[u8])]) {
    for &(slot, skin) in slots {
        for (k, t) in tiles(skin).iter().enumerate() {
            let id = tile(slot, (k / 6) as u16, (k % 6) as u16);
            let (tx, ty) = ((id % TILES_PER_ROW) as usize * TILE, (id / TILES_PER_ROW) as usize * TILE);
            for y in 0..TILE {
                let i = ((ty + y) * ATLAS + tx) * 4;
                atlas[i..i + TILE * 4].copy_from_slice(&t[y * TILE * 4..(y + 1) * TILE * 4]);
            }
        }
    }
}

/// A built-in skin unfolded into the PNG layout (for `template.png`).
pub fn unfold_builtin(atlas: &[u8], k: u16) -> Vec<u8> {
    let parts = &crate::nametags::SKIN_MODELS[k as usize % 6];
    let mut px = vec![0u8; SKIN_BYTES];
    for (b, part) in BOXES.iter().zip(parts.iter()) {
        for (f, ((x, y, w, h), turned)) in faces(b, b.at).into_iter().enumerate() {
            let tp = crate::texture::tile_pixels(atlas, part.tiles[f]);
            for dy in 0..h {
                for dx in 0..w {
                    let (tx, ty) = (dx * TILE / w, dy * TILE / h);
                    let (tx, ty) = if turned { (TILE - 1 - tx, TILE - 1 - ty) } else { (tx, ty) };
                    let s = (ty * TILE + tx) * 4;
                    let d = ((y + dy) * 64 + x + dx) * 4;
                    px[d..d + 4].copy_from_slice(&tp[s..s + 4]);
                    px[d + 3] = 255;
                }
            }
        }
    }
    px
}

/// A test card of a skin: each kind of face its own colour (right side red,
/// left green, top white, bottom black, back blue, front yellow), with a
/// magenta mark in every face's top-left corner (for checking which way up
/// and round faces are drawn).
pub fn test_card() -> Vec<u8> {
    let colours: [[u8; 4]; 6] = [[220, 40, 40, 255], [40, 200, 40, 255], [240, 240, 240, 255], [20, 20, 20, 255], [40, 60, 220, 255], [240, 220, 40, 255]];
    let mut px = vec![0u8; SKIN_BYTES];
    for b in &BOXES {
        for (f, ((x, y, w, h), _)) in faces(b, b.at).into_iter().enumerate() {
            for dy in 0..h {
                for dx in 0..w {
                    let mark = dx < 2.max(w / 4) && dy < 2.max(h / 4);
                    let c = if mark { [230, 40, 230, 255] } else { colours[f] };
                    px[((y + dy) * 64 + x + dx) * 4..((y + dy) * 64 + x + dx) * 4 + 4].copy_from_slice(&c);
                }
            }
        }
    }
    px
}

/// The skins folder's PNGs, by name.
pub fn list() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(SKINS_DIR)
        .map(|d| d.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.to_lowercase().ends_with(".png")).collect())
        .unwrap_or_default();
    v.sort();
    v
}

/// Make the skins folder (with a template to paint over) if it isn't there.
pub fn make_folder(atlas: &[u8]) {
    if std::path::Path::new(SKINS_DIR).exists() || std::fs::create_dir_all(SKINS_DIR).is_err() {
        return;
    }
    let img = macroquad::texture::Image { bytes: unfold_builtin(atlas, 0), width: 64, height: 64 };
    img.export_png(&format!("{SKINS_DIR}/template.png"));
}

/// Read a skin from the skins folder.
pub fn load_file(name: &str) -> Result<Vec<u8>, String> {
    if name.contains(['/', '\\']) || name.contains("..") {
        return Err("not a skin name".into());
    }
    let bytes = std::fs::read(format!("{SKINS_DIR}/{name}")).map_err(|e| e.to_string())?;
    load_png(&bytes)
}

impl Game {
    /// Wear our own skin (None: back to the built-in one), and tell everyone.
    pub fn set_own_skin(&mut self, pixels: Option<Vec<u8>>) {
        let pixels = pixels.filter(|p| p.len() == SKIN_BYTES);
        if pixels == self.my_skin {
            return;
        }
        self.my_skin = pixels;
        self.skin_gen += 1;
        if self.net.is_some() {
            let m = Msg::SkinData { id: self.my_id, pixels: self.my_skin.clone().unwrap_or_default() };
            self.net_send_msg(m);
        }
    }

    /// Someone's skin arrived (or went).
    pub fn note_peer_skin(&mut self, id: u32, pixels: Vec<u8>) {
        if let Some(p) = self.peers.get_mut(&id) {
            p.own_skin = (pixels.len() == SKIN_BYTES).then_some(pixels);
            self.skin_gen += 1;
        }
    }

    /// Who gets the skin slots now: us first, then the others by id.
    pub fn refresh_skin_slots(&mut self) {
        if self.my_skin.is_some() && !self.advancements.has("looking_sharp") {
            self.advance("looking_sharp");
        }
        let mut want: Vec<u32> = Vec::new();
        if self.my_skin.is_some() {
            want.push(ME);
        }
        let mut ids: Vec<u32> = self.peers.iter().filter(|(_, p)| p.own_skin.is_some()).map(|(&id, _)| id).collect();
        ids.sort();
        want.extend(ids);
        want.truncate(SLOTS as usize);
        if want != self.skin_slots {
            self.skin_slots = want;
            self.skin_gen += 1;
        }
    }

    /// The slot showing `who`'s skin, if any.
    pub fn skin_slot(&self, who: u32) -> Option<u16> {
        self.skin_slots.iter().position(|&k| k == who).map(|i| i as u16)
    }

    /// Each slot's skin, for painting into the atlas.
    pub fn skin_paint(&self) -> Vec<(u16, &[u8])> {
        self.skin_slots
            .iter()
            .enumerate()
            .filter_map(|(i, &k)| {
                let px = if k == ME { self.my_skin.as_deref() } else { self.peers.get(&k).and_then(|p| p.own_skin.as_deref()) };
                px.map(|p| (i as u16, p))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skins_load_cut_and_lay_out() {
        const { assert!(FIRST_SKIN_TILE >= crate::texture::FIRST_MOD_TILE + 2000, "mods keep plenty of room") };
        assert_eq!(tile(SLOTS - 1, 5, 5), TILES_PER_ROW * TILES_PER_ROW - 1);
        // A skin with a red face front and a hat brim of blue over its top row.
        let mut px = vec![0u8; SKIN_BYTES];
        let mut put = |x: usize, y: usize, c: [u8; 4]| px[(y * 64 + x) * 4..(y * 64 + x) * 4 + 4].copy_from_slice(&c);
        for y in 8..16 {
            for x in 8..16 {
                put(x, y, [200, 0, 0, 255]);
            }
        }
        for x in 40..48 {
            put(x, 8, [0, 0, 200, 255]);
        }
        let t = tiles(&px);
        assert_eq!(t.len(), 36);
        let front = &t[5 * 6 + 5];
        assert_eq!(&front[(15 * TILE + 3) * 4..(15 * TILE + 3) * 4 + 4], &[200, 0, 0, 255], "the face, low down");
        assert_eq!(&front[3 * 4..4 * 4], &[0, 0, 200, 255], "the hat over it, top row");
        // Wrong sizes are refused; old 64x32 skins are filled out.
        assert!(normalize(32, 32, &[0; 32 * 32 * 4]).is_err());
        let mut old = vec![0u8; 64 * 32 * 4];
        old[(20 * 64 + 44) * 4..(20 * 64 + 44) * 4 + 4].copy_from_slice(&[1, 2, 3, 255]);
        let full = normalize(64, 32, &old).unwrap();
        assert_eq!(full.len(), SKIN_BYTES);
        assert_eq!(&full[(52 * 64 + 36) * 4..(52 * 64 + 36) * 4 + 4], &[1, 2, 3, 255], "left arm copies the right");
        // A built-in skin unfolds into the layout and cuts back to the same faces.
        let atlas = crate::texture::build_atlas(1337);
        let stove = unfold_builtin(&atlas, 0);
        let back = tiles(&stove);
        let face_tile = crate::texture::tile_pixels(&atlas, crate::nametags::skin_tiles(0)[1]);
        // (8x8 from 16x16 and back: every other pixel survives.)
        assert_eq!(&back[5 * 6 + 5][0..4], &face_tile[0..4]);
    }

    #[test]
    fn skins_reach_slots_and_everyone() {
        let mut g = crate::game::tests::arena(950);
        g.set_own_skin(Some(vec![9; SKIN_BYTES]));
        g.refresh_skin_slots();
        assert_eq!(g.skin_slot(ME), Some(0));
        assert!(g.advancements.has("looking_sharp"));
        g.peers.insert(7, crate::multiplayer::Peer::new("Bob".into(), macroquad::math::Vec3::ZERO));
        g.note_peer_skin(7, vec![3; SKIN_BYTES]);
        g.refresh_skin_slots();
        assert_eq!(g.skin_slot(7), Some(1));
        assert_eq!(g.skin_paint().len(), 2);
        g.note_peer_skin(7, Vec::new());
        g.refresh_skin_slots();
        assert_eq!(g.skin_slot(7), None, "back to the built-in look");
        g.set_own_skin(Some(vec![1; 5]));
        assert!(g.my_skin.is_none(), "wrong sizes aren't worn");
    }
}
