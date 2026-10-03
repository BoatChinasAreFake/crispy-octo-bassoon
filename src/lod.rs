//! Distant terrain: a cheap, low-detail picture of the land past the render
//! distance, so mountains and coasts show on the horizon.
//!
//! It's the world generator's own height and biome (see `Generator::column`),
//! in steps of four blocks near the loaded world and eight further out, built
//! as blocky terraces (a flat top and walls down to lower neighbours) in the
//! colours of the real surface blocks: grass, sand, red sand, snow, mud and
//! sea, with forests darkened toward their leaves. It reaches about two and a
//! half times the render distance.
//!
//! It's made of chunk-sized tiles, and each frame only the tiles whose real
//! chunk isn't on screen are drawn (see `Renderer::draw`), so the two meet
//! exactly however the loaded area is shaped, and still-loading chunks show
//! their far-off picture until they arrive. It's built on a background
//! thread, again whenever we move a couple of chunks, and only in the
//! ordinary world (the Scorchlands and the Hollow keep their haze). Player
//! builds don't show that far out.

use crate::block::*;
use crate::mesher::{MeshData, Vertex};
use crate::texture::T_WHITE;
use crate::world::{Biome, Generator, CW, SEA};
use macroquad::math::Vec3;
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

/// Blocks between samples, near the loaded world and further out.
pub const NEAR_STEP: i32 = 4;
pub const FAR_STEP: i32 = 8;
/// How many chunks past the render distance the finer steps go.
const NEAR_RING: i32 = 4;
/// How much further than the render distance it reaches, and at most how far.
pub const REACH: f32 = 2.5;
pub const MAX_FAR: i32 = 512;
/// Rebuilt when we've moved this many blocks from where it was built.
const REBUILD: i32 = 32;

/// How far it reaches (in blocks) for a render distance of `chunks`.
pub fn far_for(chunks: i32) -> i32 {
    (((chunks * 16) as f32 * REACH) as i32).clamp(96, MAX_FAR)
}

/// The distant terrain: one mesh, and where each chunk's tile is in it.
#[derive(Default)]
pub struct FarLand {
    pub mesh: MeshData,
    /// (chunk, first index, index count), row by row.
    pub tiles: Vec<((i32, i32), u32, u32)>,
}

/// What a column looks like from far off: its top block, and a colour
/// multiplier for grass, leaves and water.
fn surface(h: i32, biome: Biome) -> (Id, [f32; 3]) {
    let beach = (SEA - 1..=SEA + 1).contains(&h) && !matches!(biome, Biome::Snowy | Biome::Swamp | Biome::Badlands);
    if h < SEA - 1 {
        return (WATER, crate::tint::water(biome));
    }
    match biome {
        Biome::Badlands => (RED_SAND, [1.0; 3]),
        Biome::Desert => (SAND, [1.0; 3]),
        _ if beach => (SAND, [1.0; 3]),
        Biome::Snowy => (SNOW_GRASS, [1.0; 3]),
        Biome::Mangrove => (MUD, [1.0; 3]),
        Biome::PaleGarden => (PALE_MOSS, [1.0; 3]),
        b => (GRASS, crate::tint::grass(b)),
    }
}

/// Trees from far off: their leaves, and how much of the ground they hide.
fn canopy(biome: Biome) -> Option<(Id, f32)> {
    match biome {
        Biome::Forest => Some((LEAVES, 0.55)),
        Biome::Jungle => Some((JUNGLE_LEAVES, 0.75)),
        Biome::Taiga => Some((SPRUCE_LEAVES, 0.5)),
        Biome::Cherry => Some((CHERRY_LEAVES, 0.45)),
        Biome::Mangrove => Some((MANGROVE_LEAVES, 0.5)),
        Biome::PaleGarden => Some((PALE_OAK_LEAVES, 0.6)),
        _ => None,
    }
}

/// The colour a column shows from far off, from the blocks' own colours (see
/// `navigation::block_colors`; a plain guess if they're not known yet).
fn colour(h: i32, biome: Biome, colours: &[[u8; 3]]) -> [u8; 3] {
    let of = |id: Id| colours.get(id as usize).copied().filter(|c| *c != [0, 0, 0]);
    let (id, tint) = surface(h, biome);
    let base = of(id).unwrap_or(match id {
        WATER => [60, 95, 200],
        SAND => [215, 200, 150],
        RED_SAND => [190, 105, 60],
        SNOW_GRASS => [235, 240, 245],
        _ => [95, 150, 70],
    });
    let mut c = [0.0f32; 3];
    for k in 0..3 {
        c[k] = base[k] as f32 * tint[k];
    }
    if id == WATER {
        // Deeper is darker.
        let deep = 1.0 - ((SEA - h) as f32 / 30.0).clamp(0.0, 0.45);
        c = c.map(|v| v * deep);
    } else if let Some((leaves, amount)) = canopy(biome) {
        let l = of(leaves).unwrap_or([60, 110, 50]);
        let lt = if leaves == CHERRY_LEAVES || leaves == PALE_OAK_LEAVES { [1.0; 3] } else { crate::tint::grass(biome) };
        for k in 0..3 {
            c[k] = c[k] * (1.0 - amount) + l[k] as f32 * lt[k] * amount;
        }
    }
    c.map(|v| v.clamp(0.0, 255.0) as u8)
}

/// One flat face for the far mesh.
fn face(mesh: &mut MeshData, corners: [Vec3; 4], shade: f32, c: [u8; 3]) {
    let (u0, v0, s) = crate::texture::tile_uv(T_WHITE);
    let uvs = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
    let tint = [c[0] / 2, c[1] / 2, c[2] / 2, 128];
    let mut v = [Vertex::default(); 4];
    for k in 0..4 {
        v[k] = Vertex { pos: corners[k].to_array(), uv: [u0 + uvs[k][0] * s, v0 + uvs[k][1] * s], light: [shade, 1.0, 0.0], tile: [-u0 - 2.0, -v0 - 2.0], tint };
    }
    mesh.quad(v, false);
}

/// The far-off land around chunk (`pcx`, `pcz`), as tiles out to `outer`
/// chunks; tiles closer than `skip` chunks are left out (they're always loaded).
pub fn build(generator: &Generator, colours: &[[u8; 3]], pcx: i32, pcz: i32, skip: i32, near: i32, outer: i32) -> FarLand {
    let mut land = FarLand::default();
    // The top face of a column, from far off (the sea's surface over water).
    let top = |x: i32, z: i32| {
        let (h, biome) = generator.column(x, z);
        (h.max(SEA - 1) + 1, h, biome)
    };
    for dz in -outer..=outer {
        for dx in -outer..=outer {
            let d2 = dx * dx + dz * dz;
            if d2 > outer * outer || d2 < skip * skip {
                continue;
            }
            let (cx, cz) = (pcx + dx, pcz + dz);
            let x0 = cx * CW;
            if crate::scorch::in_scorch(x0 as f32) || crate::scorch::in_scorch((x0 + CW) as f32) || crate::hollow::in_hollow(x0 as f32) {
                continue;
            }
            let step = if d2 <= near * near { NEAR_STEP } else { FAR_STEP };
            let start = land.mesh.idx.len() as u32;
            let n = CW / step;
            for j in 0..n {
                for i in 0..n {
                    let (x, z) = (x0 + i * step, cz * CW + j * step);
                    let (mx, mz) = (x + step / 2, z + step / 2);
                    let (y, h, biome) = top(mx, mz);
                    let c = colour(h, biome, colours);
                    let (xa, za, xb, zb, yf) = (x as f32, z as f32, (x + step) as f32, (z + step) as f32, y as f32);
                    face(&mut land.mesh, [Vec3::new(xa, yf, zb), Vec3::new(xb, yf, zb), Vec3::new(xb, yf, za), Vec3::new(xa, yf, za)], 1.0, c);
                    // Walls down to any lower neighbour (each wall belongs to the higher side).
                    let side = |k: f32| c.map(|v| (v as f32 * k) as u8);
                    for (ox, oz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                        let ny = top(mx + ox * step, mz + oz * step).0 as f32;
                        if ny >= yf {
                            continue;
                        }
                        let corners = match (ox, oz) {
                            (1, _) => [Vec3::new(xb, ny, zb), Vec3::new(xb, ny, za), Vec3::new(xb, yf, za), Vec3::new(xb, yf, zb)],
                            (-1, _) => [Vec3::new(xa, ny, za), Vec3::new(xa, ny, zb), Vec3::new(xa, yf, zb), Vec3::new(xa, yf, za)],
                            (_, 1) => [Vec3::new(xa, ny, zb), Vec3::new(xb, ny, zb), Vec3::new(xb, yf, zb), Vec3::new(xa, yf, zb)],
                            _ => [Vec3::new(xb, ny, za), Vec3::new(xa, ny, za), Vec3::new(xa, yf, za), Vec3::new(xb, yf, za)],
                        };
                        let shade = if ox != 0 { 0.7 } else { 0.85 };
                        face(&mut land.mesh, corners, shade, side(0.92));
                    }
                }
            }
            let end = land.mesh.idx.len() as u32;
            if end > start {
                land.tiles.push(((cx, cz), start, end - start));
            }
        }
    }
    land
}

/// Keeps the far land up to date as we move (see `Game::stream`).
#[derive(Default)]
pub struct Lod {
    /// Where the current land was built: (x, z, render distance).
    pub built: Option<(i32, i32, i32)>,
    pending: Option<((i32, i32, i32), Receiver<FarLand>)>,
}

impl Lod {
    /// Start a rebuild if we've moved far enough (or the distance changed);
    /// returns the finished land, if it's ready.
    pub fn tick(&mut self, generator: &Arc<Generator>, colours: &[[u8; 3]], at: Vec3, chunks: i32) -> Option<FarLand> {
        let snap = |v: f32| (v.floor() as i32).div_euclid(REBUILD) * REBUILD;
        let want = (snap(at.x), snap(at.z), chunks);
        if let Some((key, rx)) = &self.pending {
            if let Ok(land) = rx.try_recv() {
                self.built = Some(*key);
                self.pending = None;
                return Some(land);
            }
            return None;
        }
        if self.built == Some(want) {
            return None;
        }
        let (tx, rx) = channel();
        let generator = generator.clone();
        let colours = colours.to_vec();
        let (pcx, pcz) = ((at.x / CW as f32).floor() as i32, (at.z / CW as f32).floor() as i32);
        // Tiles well inside the loaded world are never seen; the rest are drawn
        // only while their chunk isn't (so loading chunks have something too).
        let skip = (chunks - 3).max(0);
        let near = chunks + NEAR_RING;
        let outer = far_for(chunks) / CW;
        let _ = std::thread::Builder::new().name("distant".into()).spawn(move || {
            let _ = tx.send(build(&generator, &colours, pcx, pcz, skip, near, outer));
        });
        self.pending = Some((want, rx));
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_far_land_is_tiles_round_the_loaded_chunks() {
        let g = Generator::new(424242);
        let land = build(&g, &[], 0, 0, 4, 8, 12);
        assert!(!land.mesh.idx.is_empty());
        // One tile per chunk, none inside `skip`, none past `outer`, and the
        // index ranges laid end to end.
        let mut next = 0;
        for &((cx, cz), start, count) in &land.tiles {
            let d2 = cx * cx + cz * cz;
            assert!((16..=144).contains(&d2), "tile {cx},{cz}");
            assert_eq!(start, next);
            next = start + count;
        }
        assert_eq!(next as usize, land.mesh.idx.len());
        // A tile's tops sit on whole blocks inside its chunk.
        let &((cx, cz), start, count) = &land.tiles[0];
        for &i in &land.mesh.idx[start as usize..(start + count) as usize] {
            let p = land.mesh.verts[i as usize].pos;
            assert!(p[0] >= (cx * 16) as f32 && p[0] <= (cx * 16 + 16) as f32 && p[2] >= (cz * 16) as f32 && p[2] <= (cz * 16 + 16) as f32);
            assert_eq!(p[1].fract(), 0.0);
        }
        assert_eq!(far_for(8), 320);
        assert_eq!(far_for(32), MAX_FAR);
        // Sea looks like water, deserts like sand.
        assert_eq!(surface(SEA - 10, Biome::Ocean).0, WATER);
        assert_eq!(surface(SEA + 10, Biome::Desert).0, SAND);
    }
}
