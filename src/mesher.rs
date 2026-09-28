//! Turns chunk voxels into triangles: hidden-face culling, per-vertex ambient
//! occlusion and smoothed sky light.

use crate::block::*;
use crate::texture::tile_uv;
use crate::world::{idx, Chunk, World, CH, CW};

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    /// x: ambient occlusion * face shade, y: sky exposure.
    pub light: [f32; 2],
}

#[derive(Default)]
pub struct MeshData {
    pub verts: Vec<Vertex>,
    pub idx: Vec<u32>,
}

impl MeshData {
    pub fn quad(&mut self, v: [Vertex; 4], flip: bool) {
        let b = self.verts.len() as u32;
        self.verts.extend_from_slice(&v);
        if flip {
            self.idx.extend_from_slice(&[b + 1, b + 2, b + 3, b + 1, b + 3, b]);
        } else {
            self.idx.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
        }
    }
}

pub struct ChunkMesh {
    pub opaque: MeshData,
    pub water: MeshData,
    /// Light-emitting blocks: (x, y, z, radius).
    pub lights: Vec<[f32; 4]>,
}

/// For each face: normal, then corners bottom-left, bottom-right, top-right, top-left
/// as seen from outside (counter-clockwise).
pub const FACES: [([i32; 3], [[f32; 3]; 4], f32); 6] = [
    ([1, 0, 0], [[1., 0., 1.], [1., 0., 0.], [1., 1., 0.], [1., 1., 1.]], 0.7),
    ([-1, 0, 0], [[0., 0., 0.], [0., 0., 1.], [0., 1., 1.], [0., 1., 0.]], 0.7),
    ([0, 1, 0], [[0., 1., 1.], [1., 1., 1.], [1., 1., 0.], [0., 1., 0.]], 1.0),
    ([0, -1, 0], [[0., 0., 0.], [1., 0., 0.], [1., 0., 1.], [0., 0., 1.]], 0.5),
    ([0, 0, 1], [[0., 0., 1.], [1., 0., 1.], [1., 1., 1.], [0., 1., 1.]], 0.85),
    ([0, 0, -1], [[1., 0., 0.], [0., 0., 0.], [0., 1., 0.], [1., 1., 0.]], 0.85),
];
const CORNER_UV: [[f32; 2]; 4] = [[0., 1.], [1., 1.], [1., 0.], [0., 0.]];
const AO_CURVE: [f32; 4] = [0.5, 0.68, 0.84, 1.0];
/// Tiny inset keeps nearest-neighbour sampling inside the tile.
const UV_EPS: f32 = 1.0 / 4096.0;

pub fn face_tile(id: u8, face: usize) -> u16 {
    let t = block(id).tex;
    match face {
        2 => t[0],
        3 => t[2],
        _ => t[1],
    }
}

/// 3x3 chunk neighbourhood with fast local lookups (lx/lz in -16..32).
struct Hood<'a> {
    c: [Option<&'a Chunk>; 9],
}

impl<'a> Hood<'a> {
    #[inline]
    fn chunk_of(&self, lx: i32, lz: i32) -> (Option<&'a Chunk>, i32, i32) {
        let (ox, oz) = (lx.div_euclid(CW), lz.div_euclid(CW));
        (self.c[((oz + 1) * 3 + ox + 1) as usize], lx.rem_euclid(CW), lz.rem_euclid(CW))
    }
    #[inline]
    fn get(&self, lx: i32, y: i32, lz: i32) -> u8 {
        if y < 0 {
            return BEDROCK;
        }
        if y >= CH {
            return AIR;
        }
        match self.chunk_of(lx, lz) {
            (Some(c), x, z) => c.blocks[idx(x, y, z)],
            _ => STONE,
        }
    }
    #[inline]
    fn sky(&self, lx: i32, y: i32, lz: i32) -> f32 {
        let h = match self.chunk_of(lx, lz) {
            (Some(c), x, z) => c.heights[(z * CW + x) as usize] as i32,
            _ => 0,
        };
        if y >= h { 1.0 } else { (1.0 - (h - y) as f32 * 0.09).max(0.0) }
    }
}

fn vert(pos: [f32; 3], tile: u16, uv: [f32; 2], light: [f32; 2]) -> Vertex {
    let (u0, v0, s) = tile_uv(tile);
    let u = u0 + UV_EPS + uv[0] * (s - 2.0 * UV_EPS);
    let v = v0 + UV_EPS + uv[1] * (s - 2.0 * UV_EPS);
    Vertex { pos, uv: [u, v], light }
}

pub fn mesh_chunk(world: &World, cx: i32, cz: i32) -> ChunkMesh {
    let mut hood = Hood { c: [None; 9] };
    for dz in -1..=1 {
        for dx in -1..=1 {
            hood.c[((dz + 1) * 3 + dx + 1) as usize] = world.chunks.get(&(cx + dx, cz + dz));
        }
    }
    let mut out = ChunkMesh { opaque: MeshData::default(), water: MeshData::default(), lights: Vec::new() };
    let Some(me) = hood.c[4] else { return out };
    let (bx, bz) = ((cx * CW) as f32, (cz * CW) as f32);

    // Only walk the vertical span that contains anything.
    let mut max_y = 0;
    for h in me.heights.iter() {
        max_y = max_y.max(*h as i32);
    }
    let max_y = (max_y + 2).min(CH);

    for y in 0..max_y {
        for lz in 0..CW {
            for lx in 0..CW {
                let id = me.blocks[idx(lx, y, lz)];
                if id == AIR {
                    continue;
                }
                let def = block(id);
                let (wx, wy, wz) = (bx + lx as f32, y as f32, bz + lz as f32);
                if def.light > 0.0 {
                    out.lights.push([wx + 0.5, wy + 0.6, wz + 0.5, def.light]);
                }
                match def.model {
                    Model::Empty => {}
                    Model::Cross => {
                        let sky = hood.sky(lx, y, lz);
                        let tile = def.tex[1];
                        let (a, b) = if id == TORCH { (0.3, 0.7) } else { (0.15, 0.85) };
                        let diag = [
                            [[a, 0., a], [b, 0., b], [b, 1., b], [a, 1., a]],
                            [[b, 0., a], [a, 0., b], [a, 1., b], [b, 1., a]],
                        ];
                        for d in diag {
                            let v = |i: usize| vert([wx + d[i][0], wy + d[i][1], wz + d[i][2]], tile, CORNER_UV[i], [0.9, sky]);
                            out.opaque.quad([v(0), v(1), v(2), v(3)], false);
                            // Back side with reversed winding.
                            let w = |i: usize| vert([wx + d[i][0], wy + d[i][1], wz + d[i][2]], tile, CORNER_UV[i], [0.9, sky]);
                            out.opaque.quad([w(1), w(0), w(3), w(2)], false);
                        }
                    }
                    Model::Liquid => {
                        let above = hood.get(lx, y + 1, lz);
                        let top_h = if above == WATER { 1.0 } else { 0.875 };
                        for (f, (n, corners, shade)) in FACES.iter().enumerate() {
                            let nb = hood.get(lx + n[0], y + n[1], lz + n[2]);
                            if nb == WATER || is_opaque(nb) && f != 2 {
                                continue;
                            }
                            if f == 2 && is_opaque(nb) {
                                continue;
                            }
                            let sky = hood.sky(lx + n[0], y + n[1].max(0), lz + n[2]);
                            let mut v = [Vertex::default(); 4];
                            for i in 0..4 {
                                let c = corners[i];
                                let cy = if c[1] > 0.5 { top_h } else { 0.0 };
                                v[i] = vert([wx + c[0], wy + cy, wz + c[2]], T_WATER_TILE, CORNER_UV[i], [*shade, sky]);
                            }
                            out.water.quad(v, false);
                        }
                    }
                    Model::Cube => {
                        for (f, (n, corners, shade)) in FACES.iter().enumerate() {
                            let (nx, ny, nz) = (lx + n[0], y + n[1], lz + n[2]);
                            let nb = hood.get(nx, ny, nz);
                            if is_opaque(nb) || (nb == id && def.see_through) {
                                continue;
                            }
                            let tile = face_tile(id, f);
                            let axis = if n[0] != 0 { 0 } else if n[1] != 0 { 1 } else { 2 };
                            let (t1, t2) = match axis {
                                0 => (1, 2),
                                1 => (0, 2),
                                _ => (0, 1),
                            };
                            let mut v = [Vertex::default(); 4];
                            let mut ao = [0.0f32; 4];
                            for i in 0..4 {
                                let c = corners[i];
                                let mut d1 = [0i32; 3];
                                let mut d2 = [0i32; 3];
                                d1[t1] = if c[t1] > 0.5 { 1 } else { -1 };
                                d2[t2] = if c[t2] > 0.5 { 1 } else { -1 };
                                let p1 = (nx + d1[0], ny + d1[1], nz + d1[2]);
                                let p2 = (nx + d2[0], ny + d2[1], nz + d2[2]);
                                let pc = (nx + d1[0] + d2[0], ny + d1[1] + d2[1], nz + d1[2] + d2[2]);
                                let s1 = is_opaque(hood.get(p1.0, p1.1, p1.2));
                                let s2 = is_opaque(hood.get(p2.0, p2.1, p2.2));
                                let sc = is_opaque(hood.get(pc.0, pc.1, pc.2));
                                let level = if s1 && s2 { 0 } else { 3 - s1 as usize - s2 as usize - sc as usize };
                                ao[i] = AO_CURVE[level];
                                // Smooth sky light: average over the non-solid cells touching this corner.
                                let mut sky = hood.sky(nx, ny, nz);
                                let mut n_s = 1.0;
                                if !s1 {
                                    sky += hood.sky(p1.0, p1.1, p1.2);
                                    n_s += 1.0;
                                }
                                if !s2 {
                                    sky += hood.sky(p2.0, p2.1, p2.2);
                                    n_s += 1.0;
                                }
                                if !sc && !(s1 && s2) {
                                    sky += hood.sky(pc.0, pc.1, pc.2);
                                    n_s += 1.0;
                                }
                                v[i] = vert([wx + c[0], wy + c[1], wz + c[2]], tile, CORNER_UV[i], [ao[i] * shade, sky / n_s]);
                            }
                            // Flip the diagonal so AO gradients don't crease.
                            let flip = ao[0] + ao[2] < ao[1] + ao[3];
                            out.opaque.quad(v, flip);
                        }
                    }
                }
            }
        }
    }
    out
}

const T_WATER_TILE: u16 = crate::texture::T_WATER;
