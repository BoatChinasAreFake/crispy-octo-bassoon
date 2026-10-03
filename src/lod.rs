//! Distant terrain: a cheap, low-detail picture of the land past the render
//! distance, so mountains and coasts show on the horizon.
//!
//! It's the world generator's own height and biome (see `Generator::column`)
//! sampled every eight blocks, from the edge of the loaded chunks out to
//! about two and a half times the render distance, as one coloured mesh:
//! grass, sand, snow and sea in their biome's colours, a little shaded by
//! slope, and sunk a block so real terrain always wins where they meet. It's
//! built on a background thread, again whenever we move a couple of chunks,
//! and only in the ordinary world (the Scorchlands and the Hollow keep their
//! haze). Player builds don't show that far out.

use crate::texture::T_WHITE;
use crate::mesher::{MeshData, Vertex};
use crate::world::{Biome, Generator, SEA};
use macroquad::math::Vec3;
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

/// Blocks between samples.
pub const STEP: i32 = 8;
/// How much further than the render distance it reaches, and at most how far.
pub const REACH: f32 = 2.5;
pub const MAX_FAR: i32 = 512;
/// Rebuilt when we've moved this many blocks from where it was built.
const REBUILD: i32 = 32;

/// How far it reaches for a render distance of `chunks`.
pub fn far_for(chunks: i32) -> i32 {
    (((chunks * 16) as f32 * REACH) as i32).clamp(96, MAX_FAR)
}

/// A colour for the land at a sample.
fn colour(height: i32, biome: Biome) -> [u8; 3] {
    if height < SEA {
        let w = crate::tint::water(biome);
        let deep = ((SEA - height) as f32 / 24.0).clamp(0.0, 1.0);
        let base = [60.0 - deep * 20.0, 100.0 - deep * 35.0, 200.0 - deep * 40.0];
        return [(base[0] * w[0]).min(255.0) as u8, (base[1] * w[1]).min(255.0) as u8, (base[2] * w[2]).min(255.0) as u8];
    }
    let rgb = |c: [f32; 3]| c.map(|v| v.clamp(0.0, 255.0) as u8);
    match biome {
        Biome::Desert => rgb([215.0, 200.0, 150.0]),
        Biome::Badlands => rgb([190.0, 105.0, 60.0]),
        Biome::Snowy => rgb([235.0, 240.0, 245.0]),
        _ if height > SEA + 46 => rgb([235.0, 240.0, 245.0]),
        _ if height <= SEA + 1 => rgb([210.0, 200.0, 150.0]),
        b => {
            let g = crate::tint::grass(b);
            rgb([95.0 * g[0], 150.0 * g[1], 70.0 * g[2]])
        }
    }
}

/// The far-off land around (`cx`, `cz`), leaving out the square `inner` blocks
/// either way (the loaded chunks), out to `outer` blocks.
pub fn build(generator: &Generator, cx: i32, cz: i32, inner: i32, outer: i32) -> MeshData {
    let n = outer / STEP;
    let side = (2 * n + 1) as usize;
    // Heights (water at sea level) and colours at every sample.
    let mut top = vec![0.0f32; side * side];
    let mut col = vec![[0u8; 3]; side * side];
    for j in 0..side {
        for i in 0..side {
            let (x, z) = (cx + (i as i32 - n) * STEP, cz + (j as i32 - n) * STEP);
            let (h, biome) = generator.column(x, z);
            top[j * side + i] = h.max(SEA - 1) as f32 + 1.0;
            col[j * side + i] = colour(h, biome);
        }
    }
    let (u0, v0, s) = crate::texture::tile_uv(T_WHITE);
    let mut mesh = MeshData::default();
    for j in 0..side - 1 {
        for i in 0..side - 1 {
            let (x0, z0) = (cx + (i as i32 - n) * STEP, cz + (j as i32 - n) * STEP);
            let (mx, mz) = (x0 + STEP / 2 - cx, z0 + STEP / 2 - cz);
            // Not under the loaded chunks, and round rather than square.
            if (mx.abs() < inner && mz.abs() < inner) || ((mx * mx + mz * mz) as f32).sqrt() > outer as f32 {
                continue;
            }
            if crate::scorch::in_scorch(x0 as f32) || crate::hollow::in_hollow(x0 as f32) {
                continue;
            }
            let h = [top[j * side + i], top[j * side + i + 1], top[(j + 1) * side + i + 1], top[(j + 1) * side + i]];
            let c = col[j * side + i];
            // Steep slopes a little darker, so hills read as hills.
            let slope = ((h[0] - h[1]).abs() + (h[0] - h[3]).abs()) / STEP as f32;
            let shade = (1.0 - slope * 0.35).clamp(0.6, 1.0);
            let tint = [c[0] / 2, c[1] / 2, c[2] / 2, 128];
            let (x0, z0, x1, z1) = (x0 as f32, z0 as f32, (x0 + STEP) as f32, (z0 + STEP) as f32);
            // Sunk a block, so real terrain always wins at the seam.
            let corners = [Vec3::new(x0, h[0] - 1.0, z0), Vec3::new(x1, h[1] - 1.0, z0), Vec3::new(x1, h[2] - 1.0, z1), Vec3::new(x0, h[3] - 1.0, z1)];
            let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
            let mut v = [Vertex::default(); 4];
            for k in 0..4 {
                v[k] = Vertex { pos: corners[k].to_array(), uv: [u0 + uvs[k][0] * s, v0 + uvs[k][1] * s], light: [shade, 1.0, 0.0], tile: [-u0 - 2.0, -v0 - 2.0], tint };
            }
            // Counter-clockwise from above.
            mesh.quad([v[0], v[3], v[2], v[1]], false);
        }
    }
    mesh
}

/// Keeps the far mesh up to date as we move (see `Game::stream`).
#[derive(Default)]
pub struct Lod {
    /// Where the current mesh was built: (x, z, render distance).
    pub built: Option<(i32, i32, i32)>,
    pending: Option<((i32, i32, i32), Receiver<MeshData>)>,
}

impl Lod {
    /// Start a rebuild if we've moved far enough (or the distance changed);
    /// returns a finished mesh, if one's ready.
    pub fn tick(&mut self, generator: &Arc<Generator>, at: Vec3, chunks: i32) -> Option<((i32, i32, i32), MeshData)> {
        let snap = |v: f32| (v.floor() as i32).div_euclid(REBUILD) * REBUILD;
        let want = (snap(at.x), snap(at.z), chunks);
        if let Some((key, rx)) = &self.pending {
            if let Ok(mesh) = rx.try_recv() {
                let key = *key;
                self.pending = None;
                self.built = Some(key);
                return Some((key, mesh));
            }
            return None;
        }
        if self.built == Some(want) {
            return None;
        }
        let (tx, rx) = channel();
        let generator = generator.clone();
        // The loaded square, from the chunk we're in.
        let (pcx, pcz) = ((at.x / 16.0).floor() as i32, (at.z / 16.0).floor() as i32);
        let (cx, cz) = (pcx * 16 + 8, pcz * 16 + 8);
        let inner = chunks * 16;
        let outer = far_for(chunks);
        let _ = std::thread::Builder::new().name("distant".into()).spawn(move || {
            let _ = tx.send(build(&generator, cx, cz, inner, outer));
        });
        self.pending = Some((want, rx));
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_far_land_rings_the_loaded_chunks() {
        let g = Generator::new(424242);
        let (inner, outer) = (64, 192);
        let mesh = build(&g, 8, 8, inner, outer);
        assert!(!mesh.idx.is_empty());
        // Nothing under the loaded square, nothing past the far edge.
        for v in &mesh.verts {
            let (dx, dz) = (v.pos[0] - 8.0, v.pos[2] - 8.0);
            assert!(dx.abs() >= (inner - STEP) as f32 || dz.abs() >= (inner - STEP) as f32, "{:?}", v.pos);
            assert!((dx * dx + dz * dz).sqrt() <= outer as f32 + STEP as f32 * 1.5);
        }
        assert_eq!(far_for(8), 320);
        assert_eq!(far_for(32), MAX_FAR);
    }
}
