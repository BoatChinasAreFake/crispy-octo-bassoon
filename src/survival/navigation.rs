//! Finding your way: a compass (points home) and a map (a little picture of
//! the land around you, drawn while you hold it).

use crate::block::*;
use crate::world::{World, CH};
use macroquad::math::Vec3;

/// The map shows this many blocks across.
pub const MAP_SIZE: usize = 96;

/// Which way the compass needle points, relative to straight ahead (radians,
/// clockwise). In the Scorchlands it just spins.
pub fn compass_needle(pos: Vec3, yaw: f32, target: Vec3, clock: f32) -> f32 {
    if crate::scorch::in_scorch(pos.x) {
        return clock * 7.0;
    }
    let d = target - pos;
    let bearing = d.x.atan2(-d.z);
    bearing - yaw
}

/// One colour per block: the average of its top texture (from the atlas).
pub fn block_colors(atlas: &[u8]) -> Vec<[u8; 3]> {
    let r = reg();
    (0..r.blocks.len())
        .map(|id| {
            if id == AIR as usize {
                return [0, 0, 0];
            }
            let px = crate::texture::tile_pixels(atlas, r.blocks[id].tex[0]);
            let (mut sum, mut n) = ([0u32; 3], 0u32);
            for p in px.chunks(4) {
                if p[3] > 60 {
                    for k in 0..3 {
                        sum[k] += p[k] as u32;
                    }
                    n += 1;
                }
            }
            if n == 0 { [120, 120, 120] } else { sum.map(|v| (v / n) as u8) }
        })
        .collect()
}

/// The map around `center`, north at the top, as RGBA pixels. Under a roof
/// (the Scorchlands, a cave) it shows the floor below you instead of the sky's view.
/// `scale`: blocks per pixel (a zoomed-out map sees further, in less detail).
pub fn map_pixels(world: &World, center: Vec3, colors: &[[u8; 3]], scale: i32) -> Vec<u8> {
    let (cx, cz) = (center.x.floor() as i32, center.z.floor() as i32);
    let covered = crate::scorch::in_scorch(center.x);
    let start_y = if covered { (center.y as i32 + 2).min(CH - 2) } else { CH - 1 };
    let half = (MAP_SIZE / 2) as i32;
    let mut heights = vec![0i32; MAP_SIZE * MAP_SIZE];
    let mut out = vec![0u8; MAP_SIZE * MAP_SIZE * 4];
    for row in 0..MAP_SIZE {
        for col in 0..MAP_SIZE {
            let (x, z) = (cx + (col as i32 - half) * scale, cz + (row as i32 - half) * scale);
            let i = row * MAP_SIZE + col;
            if !world.is_loaded(x, z) {
                // Unexplored: plain parchment.
                out[i * 4..i * 4 + 4].copy_from_slice(&[196, 178, 140, 255]);
                heights[i] = -1;
                continue;
            }
            let mut y = start_y;
            while y > 0 && !is_map_visible(world.get(x, y, z)) {
                y -= 1;
            }
            let id = world.get(x, y, z);
            heights[i] = y;
            let mut c = colors.get(id as usize).copied().unwrap_or([128, 128, 128]);
            // Hills catch the light from the north.
            if row > 0 && heights[i - MAP_SIZE] >= 0 {
                let k = match y.cmp(&heights[i - MAP_SIZE]) {
                    std::cmp::Ordering::Greater => 1.15,
                    std::cmp::Ordering::Less => 0.82,
                    _ => 1.0,
                };
                c = c.map(|v| (v as f32 * k).min(255.0) as u8);
            }
            out[i * 4..i * 4 + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    out
}

/// Zoom levels: blocks per map pixel.
pub const ZOOMS: [i32; 4] = [1, 2, 4, 8];

/// The map colour of the top of column (x, z), shaded by the height to its north;
/// None where the world isn't loaded.
pub fn column_colour(world: &World, x: i32, z: i32, colors: &[[u8; 3]]) -> Option<[u8; 3]> {
    if !world.is_loaded(x, z) {
        return None;
    }
    let top = |x: i32, z: i32| {
        let mut y = CH - 1;
        while y > 0 && !is_map_visible(world.get(x, y, z)) {
            y -= 1;
        }
        y
    };
    let y = top(x, z);
    let c = colors.get(world.get(x, y, z) as usize).copied().unwrap_or([128, 128, 128]);
    let k = match y.cmp(&top(x, z - 1)) {
        std::cmp::Ordering::Greater => 1.15,
        std::cmp::Ordering::Less => 0.82,
        _ => 1.0,
    };
    Some(c.map(|v| (v as f32 * k).min(255.0) as u8))
}

/// Worth drawing on a map (not air, not a flower too small to see).
fn is_map_visible(id: Id) -> bool {
    id != AIR && block(id).model != Model::Cross
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compass_points_home() {
        // Looking north (yaw 0) with home due north: straight ahead.
        let a = compass_needle(Vec3::new(0.0, 70.0, 10.0), 0.0, Vec3::ZERO, 0.0);
        assert!(a.abs() < 1e-5);
        // Home due east: a quarter turn right.
        let a = compass_needle(Vec3::ZERO, 0.0, Vec3::new(10.0, 0.0, 0.0), 0.0);
        assert!((a - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
    }
}
