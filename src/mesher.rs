//! Turns chunk voxels into triangles: hidden-face culling, per-vertex ambient
//! occlusion and smoothed sky light.

use crate::block::*;
use crate::texture::tile_uv;
use crate::light::shade;
use crate::world::{idx, Chunk, World, CH, CW};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Vertex {
    pub pos: [f32; 3],
    /// Atlas coordinates, or (when `tile` is set) tile repeats from 0 up.
    pub uv: [f32; 2],
    /// x: ambient occlusion * face shade, y: sky light, z: block light
    /// (below zero: use the nearby point lights instead, for things that move).
    pub light: [f32; 3],
    /// Where a repeating tile starts in the atlas (x below zero: `uv` is plain).
    pub tile: [f32; 2],
}

impl Default for Vertex {
    fn default() -> Self {
        Vertex { pos: [0.0; 3], uv: [0.0; 2], light: [0.0, 0.0, -1.0], tile: [-1.0; 2] }
    }
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
const AO_CURVE: [f32; 4] = [0.58, 0.73, 0.87, 1.0];
/// Face shading for foliage (same order as `FACES`). Light scatters through
/// leaves, so their sides and undersides are much less dark than solid blocks',
/// which keeps the dark bottom face and the brighter faces seen through its holes
/// from clashing.
const FOLIAGE_SHADE: [f32; 6] = [0.9, 0.9, 1.0, 0.8, 0.95, 0.95];
/// Tiny inset keeps nearest-neighbour sampling inside the tile.
const UV_EPS: f32 = 1.0 / 4096.0;

pub fn face_tile(id: Id, face: usize) -> u16 {
    if let Some(t) = crate::contraptions::face_tile(id, face) {
        return t;
    }
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
    /// The middle chunk unpacked from its palettes: nearly every lookup lands here.
    own: Vec<Id>,
}

impl<'a> Hood<'a> {
    #[inline]
    fn chunk_of(&self, lx: i32, lz: i32) -> (Option<&'a Chunk>, i32, i32) {
        let (ox, oz) = (lx.div_euclid(CW), lz.div_euclid(CW));
        (self.c[((oz + 1) * 3 + ox + 1) as usize], lx.rem_euclid(CW), lz.rem_euclid(CW))
    }
    #[inline]
    fn get(&self, lx: i32, y: i32, lz: i32) -> Id {
        if y < 0 {
            return BEDROCK;
        }
        if y >= CH {
            return AIR;
        }
        if (0..CW).contains(&lx) && (0..CW).contains(&lz) {
            return self.own[idx(lx, y, lz)];
        }
        match self.chunk_of(lx, lz) {
            (Some(c), x, z) => c.blocks.get(idx(x, y, z)),
            _ => STONE,
        }
    }
    /// Light byte of a cell (see light.rs): sky in the high bits, block in the low.
    #[inline]
    fn raw_light(&self, lx: i32, y: i32, lz: i32) -> u8 {
        if y >= CH {
            return 0xF0;
        }
        if y < 0 {
            return 0;
        }
        match self.chunk_of(lx, lz) {
            (Some(c), x, z) => c.light.get(idx(x, y, z)),
            _ => 0xF0,
        }
    }
    /// Sky and block brightness (0..1) of a cell.
    #[inline]
    fn lit(&self, lx: i32, y: i32, lz: i32) -> (f32, f32) {
        let b = self.raw_light(lx, y, lz);
        (shade(b >> 4), shade(b & 15))
    }
}

/// Smooth lighting (Options): light blends across each face, with ambient
/// occlusion in the corners. Off, each face is lit evenly by the cell in front
/// of it (flat faces also merge into fewer, bigger quads).
static SMOOTH: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub fn set_smooth(on: bool) {
    SMOOTH.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub fn smooth() -> bool {
    SMOOTH.load(std::sync::atomic::Ordering::Relaxed)
}

fn vert(pos: [f32; 3], tile: u16, uv: [f32; 2], light: [f32; 3]) -> Vertex {
    let (u0, v0, s) = tile_uv(tile);
    let u = u0 + UV_EPS + uv[0] * (s - 2.0 * UV_EPS);
    let v = v0 + UV_EPS + uv[1] * (s - 2.0 * UV_EPS);
    Vertex { pos, uv: [u, v], light, tile: [-1.0; 2] }
}

/// A face of a plain cube waiting to be merged with its like: its tile and the
/// light at its four corners. `tile == u16::MAX` marks an empty cell.
#[derive(Clone, Copy, PartialEq)]
struct Flat {
    tile: u16,
    light: [[f32; 3]; 4],
}

const NO_FLAT: Flat = Flat { tile: u16::MAX, light: [[0.0; 3]; 4] };

/// Greedy meshing: cube faces with the same tile and light join into big
/// rectangles (the shader repeats the tile across them). Faces only join
/// along a direction their light doesn't change in, so shading gradients
/// (ambient occlusion, sky light) come out as they would face by face.
fn merge_flats(flats: &[Flat], dims: [i32; 3], origin: [f32; 3], out: &mut MeshData) {
    let cell = |f: usize, p: [i32; 3]| ((f as i32 * dims[1] + p[1]) * dims[2] + p[2]) as usize * dims[0] as usize + p[0] as usize;
    let mut used = vec![false; flats.len()];
    for (f, (nrm, corners, _)) in FACES.iter().enumerate() {
        let a = if nrm[0] != 0 { 0 } else if nrm[1] != 0 { 1 } else { 2 };
        let (t1, t2) = match a {
            0 => (2, 1),
            1 => (0, 2),
            _ => (0, 1),
        };
        // Which axes the tile's u and v run along (see FACES and CORNER_UV).
        let differs = |i: usize, j: usize| (0..3).find(|&k| corners[i][k] != corners[j][k]).unwrap_or(0);
        let (u_axis, v_axis) = (differs(0, 1), differs(1, 2));
        // Light that doesn't change along an axis: each corner matches the one across from it.
        let across = |axis: usize, i: usize| (0..4).find(|&j| (0..3).all(|k| (corners[i][k] != corners[j][k]) == (k == axis))).unwrap_or(i);
        let steady = |key: &Flat, axis: usize| (0..4).all(|i| key.light[i] == key.light[across(axis, i)]);
        for layer in 0..dims[a] {
            for q in 0..dims[t2] {
                for pp in 0..dims[t1] {
                    let mut p = [0; 3];
                    p[a] = layer;
                    p[t1] = pp;
                    p[t2] = q;
                    let i0 = cell(f, p);
                    let key = flats[i0];
                    if key.tile == u16::MAX || used[i0] {
                        continue;
                    }
                    let fits = |p: [i32; 3]| {
                        let i = cell(f, p);
                        !used[i] && flats[i] == key
                    };
                    let mut w = 1;
                    while steady(&key, t1) && pp + w < dims[t1] && fits({ let mut r = p; r[t1] += w; r }) {
                        w += 1;
                    }
                    let mut h = 1;
                    'grow: while steady(&key, t2) && q + h < dims[t2] {
                        for k in 0..w {
                            let mut r = p;
                            r[t1] += k;
                            r[t2] += h;
                            if !fits(r) {
                                break 'grow;
                            }
                        }
                        h += 1;
                    }
                    for dh in 0..h {
                        for k in 0..w {
                            let mut r = p;
                            r[t1] += k;
                            r[t2] += dh;
                            used[cell(f, r)] = true;
                        }
                    }
                    let mut ext = [1.0f32; 3];
                    ext[t1] = w as f32;
                    ext[t2] = h as f32;
                    let (u0, v0, _) = tile_uv(key.tile);
                    let mut v = [Vertex::default(); 4];
                    for i in 0..4 {
                        let c = corners[i];
                        let pos = [0, 1, 2].map(|k| origin[k] + p[k] as f32 + c[k] * ext[k]);
                        let uv = [CORNER_UV[i][0] * ext[u_axis], CORNER_UV[i][1] * ext[v_axis]];
                        v[i] = Vertex { pos, uv, light: key.light[i], tile: [u0, v0] };
                    }
                    // Flip the diagonal so shading gradients don't crease.
                    let l = key.light.map(|l| l[0]);
                    out.quad(v, l[0] + l[2] < l[1] + l[3]);
                }
            }
        }
    }
}

pub fn mesh_chunk(world: &World, cx: i32, cz: i32) -> ChunkMesh {
    let mut hood = Hood { c: [None; 9], own: Vec::new() };
    for dz in -1..=1 {
        for dx in -1..=1 {
            hood.c[((dz + 1) * 3 + dx + 1) as usize] = world.chunks.get(&(cx + dx, cz + dz));
        }
    }
    let mut out = ChunkMesh { opaque: MeshData::default(), water: MeshData::default(), lights: Vec::new() };
    let Some(me) = hood.c[4] else { return out };
    hood.own = vec![AIR; (CW * CW * CH) as usize];
    me.blocks.decode_into(&mut hood.own);
    let (bx, bz) = ((cx * CW) as f32, (cz * CW) as f32);

    // Only walk the vertical span that contains anything.
    let mut max_y = 0;
    for h in me.canopy.iter() {
        max_y = max_y.max(*h as i32);
    }
    let max_y = (max_y + 2).min(CH);
    let dims = [CW, max_y, CW];
    let mut flats = vec![NO_FLAT; (6 * CW * CW * max_y) as usize];
    let flat_at = |f: usize, lx: i32, y: i32, lz: i32| ((f as i32 * max_y + y) * CW + lz) as usize * CW as usize + lx as usize;

    for y in 0..max_y {
        // All-air sections have nothing to draw (their neighbours draw the faces).
        if me.blocks.sections()[y as usize / 16].is_uniform(AIR) {
            continue;
        }
        for lz in 0..CW {
            for lx in 0..CW {
                let id = hood.own[idx(lx, y, lz)];
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
                        let (sky, blk) = hood.lit(lx, y, lz);
                        let tile = def.tex[1];
                        let (a, b) = if id == TORCH || crate::contraptions::is_ztorch(id) { (0.3, 0.7) } else { (0.15, 0.85) };
                        let diag = [
                            [[a, 0., a], [b, 0., b], [b, 1., b], [a, 1., a]],
                            [[b, 0., a], [a, 0., b], [a, 1., b], [b, 1., a]],
                        ];
                        for d in diag {
                            let v = |i: usize| vert([wx + d[i][0], wy + d[i][1], wz + d[i][2]], tile, CORNER_UV[i], [0.9, sky, blk]);
                            out.opaque.quad([v(0), v(1), v(2), v(3)], false);
                            // Back side with reversed winding.
                            let w = |i: usize| vert([wx + d[i][0], wy + d[i][1], wz + d[i][2]], tile, CORNER_UV[i], [0.9, sky, blk]);
                            out.opaque.quad([w(1), w(0), w(3), w(2)], false);
                        }
                    }
                    Model::Liquid => {
                        // Water is translucent; lava glows and hides what's behind it.
                        let lava = is_lava(id);
                        let same = |b: Id| if lava { is_lava(b) } else { is_water(b) };
                        // Surface height of a same-kind cell (1.0 if more of it sits on top).
                        let height = |x: i32, z: i32| -> Option<f32> {
                            let b = hood.get(x, y, z);
                            if !same(b) {
                                return None;
                            }
                            if same(hood.get(x, y + 1, z)) {
                                return Some(1.0);
                            }
                            let reach = if lava { LAVA_REACH } else { WATER_REACH } as f32;
                            Some(0.875 * (1.0 - liquid_level(b) as f32 / (reach + 1.0)))
                        };
                        // Each top corner averages the cells around it, so flows slope.
                        let corner = |cx: i32, cz: i32| -> f32 {
                            let mut sum = 0.0;
                            let mut n = 0.0;
                            for dz in cz - 1..=cz {
                                for dx in cx - 1..=cx {
                                    match height(lx + dx, lz + dz) {
                                        Some(1.0) => return 1.0,
                                        Some(h) => {
                                            sum += h;
                                            n += 1.0;
                                        }
                                        None => {}
                                    }
                                }
                            }
                            if n > 0.0 { sum / n } else { 0.1 }
                        };
                        let tops = [[corner(0, 0), corner(1, 0)], [corner(0, 1), corner(1, 1)]];
                        let tile = def.tex[1];
                        if lava && (wx as i32).rem_euclid(3) == 0 && (wz as i32).rem_euclid(3) == 0 && !same(hood.get(lx, y + 1, lz)) {
                            out.lights.push([wx + 0.5, wy + 1.0, wz + 0.5, 9.0]);
                        }
                        for (f, (n, corners, shade)) in FACES.iter().enumerate() {
                            let nb = hood.get(lx + n[0], y + n[1], lz + n[2]);
                            if same(nb) || (is_opaque(nb) && f != 2) || (f == 2 && is_opaque(nb)) {
                                continue;
                            }
                            let (sky, blk) = hood.lit(lx + n[0], y + n[1].max(0), lz + n[2]);
                            // Lava lights itself (the shader reads x above 1.5 as "glowing").
                            let light = if lava { [2.45, sky, blk] } else { [*shade, sky, blk] };
                            let mut v = [Vertex::default(); 4];
                            for i in 0..4 {
                                let c = corners[i];
                                let cy = if c[1] > 0.5 { tops[c[2] as usize][c[0] as usize] } else { 0.0 };
                                v[i] = vert([wx + c[0], wy + cy, wz + c[2]], tile, CORNER_UV[i], light);
                            }
                            if lava {
                                out.opaque.quad(v, false);
                            } else {
                                out.water.quad(v, false);
                            }
                        }
                    }
                    Model::Shaped => {
                        // Slabs, stairs, doors: each box's faces, skipping only those
                        // flush against an opaque neighbour. The texture follows the
                        // box's position in the cell, so a slab shows half a tile.
                        let ((s0, b0), (s1, b1)) = (hood.lit(lx, y, lz), hood.lit(lx, y + 1, lz));
                        let (sky, blk) = (s0.max(s1), b0.max(b1));
                        let (boxes, n) = def.shape.boxes();
                        for &(bmin, bmax) in &boxes[..n] {
                            for (f, (nrm, corners, shade)) in FACES.iter().enumerate() {
                                let axis = if nrm[0] != 0 { 0 } else if nrm[1] != 0 { 1 } else { 2 };
                                let flush = if nrm[axis] > 0 { bmax[axis] >= 1.0 } else { bmin[axis] <= 0.0 };
                                if flush && is_opaque(hood.get(lx + nrm[0], y + nrm[1], lz + nrm[2])) {
                                    continue;
                                }
                                let tile = face_tile(id, f);
                                let mut v = [Vertex::default(); 4];
                                for i in 0..4 {
                                    let c = corners[i];
                                    let p = [0, 1, 2].map(|k| if c[k] > 0.5 { bmax[k] } else { bmin[k] });
                                    let uv = match f {
                                        0 => [1.0 - p[2], 1.0 - p[1]],
                                        1 => [p[2], 1.0 - p[1]],
                                        2 => [p[0], p[2]],
                                        3 => [p[0], 1.0 - p[2]],
                                        4 => [p[0], 1.0 - p[1]],
                                        _ => [1.0 - p[0], 1.0 - p[1]],
                                    };
                                    // Portals glow (see the shader's "above 1.5" rule).
                                    let lx = if crate::scorch::is_portal(id) { 2.3 } else { shade * 0.95 };
                                    v[i] = vert([wx + p[0], wy + p[1], wz + p[2]], tile, uv, [lx, sky, blk]);
                                }
                                out.opaque.quad(v, false);
                            }
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
                            let shade = if dapples_sky(id) { &FOLIAGE_SHADE[f] } else { shade };
                            let axis = if n[0] != 0 { 0 } else if n[1] != 0 { 1 } else { 2 };
                            let (t1, t2) = match axis {
                                0 => (1, 2),
                                1 => (0, 2),
                                _ => (0, 1),
                            };
                            let mut v = [Vertex::default(); 4];
                            let mut ao = [0.0f32; 4];
                            if !smooth() {
                                let (sky, blk) = hood.lit(nx, ny, nz);
                                for i in 0..4 {
                                    let c = corners[i];
                                    v[i] = vert([wx + c[0], wy + c[1], wz + c[2]], tile, CORNER_UV[i], [shade * 1.0, sky, blk]);
                                }
                                flats[flat_at(f, lx, y, lz)] = Flat { tile, light: v.map(|x| x.light) };
                                continue;
                            }
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
                                // Smooth light: average over the non-solid cells touching this corner.
                                let (mut sky, mut blk) = hood.lit(nx, ny, nz);
                                let mut n_s = 1.0;
                                let mut add = |p: (i32, i32, i32)| {
                                    let (s, b) = hood.lit(p.0, p.1, p.2);
                                    sky += s;
                                    blk += b;
                                    n_s += 1.0;
                                };
                                if !s1 {
                                    add(p1);
                                }
                                if !s2 {
                                    add(p2);
                                }
                                if !sc && !(s1 && s2) {
                                    add(pc);
                                }
                                v[i] = vert([wx + c[0], wy + c[1], wz + c[2]], tile, CORNER_UV[i], [ao[i] * shade, sky / n_s, blk / n_s]);
                            }
                            // Faces wait to be merged with their like (see `merge_flats`).
                            flats[flat_at(f, lx, y, lz)] = Flat { tile, light: v.map(|x| x.light) };
                        }
                    }
                }
            }
        }
    }
    merge_flats(&flats, dims, [bx, 0.0, bz], &mut out.opaque);
    out
}


#[cfg(test)]
mod tests {
    use super::*;

    fn tops(dims: [i32; 3], fill: impl Fn(i32, i32) -> Option<u16>) -> Vec<Flat> {
        let mut flats = vec![NO_FLAT; (6 * dims[0] * dims[1] * dims[2]) as usize];
        for z in 0..dims[2] {
            for x in 0..dims[0] {
                if let Some(tile) = fill(x, z) {
                    // Face 2 is the top, at y 0.
                    flats[((2 * dims[1]) * dims[2] + z) as usize * dims[0] as usize + x as usize] = Flat { tile, light: [[1.0, 1.0, 0.0]; 4] };
                }
            }
        }
        flats
    }

    #[test]
    fn a_flat_field_is_one_quad() {
        let dims = [16, 1, 16];
        let mut out = MeshData::default();
        merge_flats(&tops(dims, |_, _| Some(3)), dims, [0.0; 3], &mut out);
        assert_eq!(out.verts.len(), 4);
        // The tile repeats sixteen times each way.
        let (lo, hi) = out.verts.iter().fold((f32::MAX, f32::MIN), |(lo, hi), v| (lo.min(v.uv[0]), hi.max(v.uv[0])));
        assert_eq!((lo, hi), (0.0, 16.0));
        assert!(out.verts.iter().all(|v| v.tile[0] >= 0.0 && v.pos[1] == 1.0));
    }

    #[test]
    fn different_tiles_and_holes_split_the_field() {
        let dims = [16, 1, 16];
        let mut out = MeshData::default();
        merge_flats(&tops(dims, |x, z| if (x, z) == (5, 5) { Some(4) } else if x == 9 { None } else { Some(3) }), dims, [0.0; 3], &mut out);
        let quads = out.verts.len() / 4;
        assert!(quads > 2 && quads < 12, "{quads} quads");
        // Every face is still covered exactly once: 255 - 16 of tile 3, one of tile 4.
        let area: f32 = out.verts.chunks(4).map(|q| (q[1].pos[0] - q[0].pos[0]).abs().max((q[2].pos[0] - q[1].pos[0]).abs()) * (q[1].pos[2] - q[0].pos[2]).abs().max((q[2].pos[2] - q[1].pos[2]).abs())).sum();
        assert_eq!(area, 256.0 - 16.0);
    }
}
