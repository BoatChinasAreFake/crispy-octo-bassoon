//! The Scorchlands: a second, hotter world under a bedrock sky, reached
//! through portals of obsidian.
//!
//! It lives in the same `World` as everything else, a long way east
//! (past `SCORCH_X`): saves, multiplayer, liquids and mobs all work there
//! without knowing it's special. A band of bedrock keeps anyone from simply
//! walking in. Travel is by portal: every block in the Scorchlands is eight
//! in the ordinary world, Minecraft style, and a portal is built at the other
//! end if there isn't one near already.
//!
//! Build a frame of obsidian (at least 4 wide and 5 tall, corners optional)
//! and light it with a Sparker; stand in it for a couple of seconds.

use crate::block::*;
use crate::game::Game;
use crate::net::Msg;
use crate::noise::hash3;
use crate::sound::Sfx;
use crate::world::{Generator, World, CH, CW};
use macroquad::math::{IVec3, Vec3};

/// Where the Scorchlands start (blocks east), and where their middle is.
pub const SCORCH_X: i32 = 32_768;
pub const SCORCH_ORIGIN: i32 = SCORCH_X + 1024;
/// The bedrock band between the two, in the ordinary world.
pub const WALL: i32 = 2048;
/// Lava sea level down there.
pub const LAVA_SEA: i32 = 31;
/// Seconds standing in a portal before it takes you (creative: less).
pub const PORTAL_SECS: f32 = 2.0;
/// Biggest frame interior a Sparker will light.
const MAX_PORTAL: i32 = 21;

pub fn in_scorch(x: f32) -> bool {
    x >= (SCORCH_X - 16) as f32
}

/// In the bedrock band that separates the worlds.
pub fn in_wall(x: i32) -> bool {
    (SCORCH_X - WALL..SCORCH_X).contains(&x)
}

pub fn is_portal(id: Id) -> bool {
    id == PORTAL_X || id == PORTAL_Z
}

/// Where a portal at `from` leads (before looking for a place to stand).
pub fn destination(from: IVec3) -> IVec3 {
    let reach = (SCORCH_ORIGIN - SCORCH_X - 64) * 8;
    if in_scorch(from.x as f32) {
        IVec3::new((from.x - SCORCH_ORIGIN) * 8, 70, from.z * 8)
    } else {
        IVec3::new(SCORCH_ORIGIN + from.x.clamp(-reach, reach) / 8, 64, from.z / 8)
    }
}

impl Generator {
    /// A chunk of the Scorchlands, or of the bedrock band before them.
    pub fn generate_scorch(&self, cx: i32, cz: i32) -> Vec<Id> {
        let vol = (CW * CW * CH) as usize;
        let mut b = vec![AIR; vol];
        let s = self.seed ^ 0x5C0_4C4;
        for lz in 0..CW {
            for lx in 0..CW {
                let (x, z) = (cx * CW + lx, cz * CW + lz);
                // The wall between the worlds, and the Scorchlands' own west edge.
                if in_wall(x) || (SCORCH_X..SCORCH_X + 16).contains(&x) {
                    for y in 0..CH {
                        b[crate::world::idx(lx, y, lz)] = BEDROCK;
                    }
                    continue;
                }
                for y in 0..CH {
                    let i = crate::world::idx(lx, y, lz);
                    // Bedrock floor and ceiling, a little ragged.
                    if y == 0 || y == CH - 1 || (y <= 3 && hash3(s, x, y, z) < 0.5) || (y >= CH - 4 && hash3(s ^ 1, x, y, z) < 0.5) {
                        b[i] = BEDROCK;
                        continue;
                    }
                    // Big open caverns in the middle, solid toward floor and ceiling.
                    let solid = self.scorch_solid(x, y, z);
                    b[i] = if solid {
                        if hash3(s ^ 2, x, y, z) < 0.012 {
                            SCORCH_GOLD_ORE
                        } else if y < 22 && hash3(s ^ 0xDEB, x, y, z) < 0.0022 {
                            // Old Debris: rare, deep down, and the only source of Scorchite (see smithing.rs).
                            OLD_DEBRIS
                        } else {
                            SCORCHROCK
                        }
                    } else if y <= LAVA_SEA {
                        LAVA
                    } else {
                        AIR
                    };
                }
                // Ember sand on the shores of the lava sea, glowrock hanging from ceilings.
                for y in LAVA_SEA + 1..CH - 5 {
                    let i = crate::world::idx(lx, y, lz);
                    let below = b[crate::world::idx(lx, y - 1, lz)];
                    if b[i] == AIR && below == SCORCHROCK && y < LAVA_SEA + 6 && self.scorch.noise3(x as f32 / 9.0, 7.0, z as f32 / 9.0) > 0.15 {
                        b[crate::world::idx(lx, y - 1, lz)] = EMBERSAND;
                    }
                    // Sorrow Sand in patches on the higher floors (half of what builds a Wilter).
                    if b[i] == AIR && below == SCORCHROCK && y >= LAVA_SEA + 6 && self.scorch.noise3(x as f32 / 14.0, 23.0, z as f32 / 14.0) > 0.38 {
                        b[crate::world::idx(lx, y - 1, lz)] = SORROW_SAND;
                    }
                    let above = b[crate::world::idx(lx, y + 1, lz)];
                    if b[i] == AIR && above == SCORCHROCK && hash3(s ^ 3, x >> 2, y >> 1, z >> 2) < 0.06 && hash3(s ^ 4, x, y, z) < 0.6 {
                        b[i] = GLOWROCK;
                    }
                    // Ember Shrooms grow in clumps on the rock (a brewing ingredient).
                    if b[i] == AIR && matches!(below, SCORCHROCK | EMBERSAND) && hash3(s ^ 5, x >> 3, 0, z >> 3) < 0.3 && hash3(s ^ 6, x, y, z) < 0.08 {
                        b[i] = EMBER_SHROOM;
                    }
                }
            }
        }
        // Fungus forests, basalt deltas and soul sand valleys (see wilds.rs).
        self.decorate_scorch(cx, cz, &mut b);
        // Fortresses and Snout camps (see fortress.rs).
        self.place_structures(cx, cz, &mut b);
        b
    }
}

impl World {
    /// Generate (and keep) a chunk right now, rather than on the worker threads.
    pub fn load_now(&mut self, cx: i32, cz: i32) {
        if self.chunks.contains_key(&(cx, cz)) {
            return;
        }
        let blocks = crate::palette::PalettedBlocks::from_ids(&self.generator.generate(cx, cz));
        self.insert_chunk(cx, cz, blocks);
    }
}

impl Game {
    /// Right-click with a Sparker: light a portal in an obsidian frame (or TNT).
    pub fn use_sparker(&mut self, hit: IVec3, normal: IVec3) -> bool {
        let id = self.world.get_v(hit);
        if id == TNT {
            return false; // TNT has its own right-click
        }
        let start = hit + normal;
        for axis_x in [true, false] {
            if let Some(cells) = portal_frame(&self.world, start, axis_x) {
                let pid = if axis_x { PORTAL_X } else { PORTAL_Z };
                for c in cells {
                    self.world.set_v(c, pid);
                }
                self.sfx(Sfx::Warp, Some(start.as_vec3()));
                self.use_tool(1);
                self.player.swing = 1.0;
                self.advance("portal_open");
                return true;
            }
        }
        // Not a portal: set it alight.
        self.spark(hit, normal)
    }

    /// Standing in a portal long enough takes you through (the local player).
    pub fn portal_tick(&mut self, dt: f32) {
        self.portal_cooldown = (self.portal_cooldown - dt).max(0.0);
        let b = &self.player.body;
        let feet = IVec3::new(b.pos.x.floor() as i32, (b.pos.y + 0.2).floor() as i32, b.pos.z.floor() as i32);
        let inside = is_portal(self.world.get_v(feet)) || is_portal(self.world.get_v(feet + IVec3::Y));
        if !inside || self.dead.is_some() {
            self.portal_time = 0.0;
            self.left_portal = true;
            return;
        }
        if !self.left_portal || self.portal_cooldown > 0.0 {
            return;
        }
        self.portal_time += dt;
        let needed = if self.creative { 0.4 } else { PORTAL_SECS };
        if self.portal_time < needed {
            return;
        }
        self.portal_time = 0.0;
        self.left_portal = false;
        self.portal_cooldown = 4.0;
        if self.is_client() {
            // The host builds the other end and moves us there.
            self.net_send_msg(Msg::UsePortal { x: feet.x, y: feet.y, z: feet.z });
            return;
        }
        let to = self.travel(feet);
        self.player.body.pos = to;
        self.player.body.vel = Vec3::ZERO;
        self.player.fall_start = to.y;
        self.ready = false;
        self.sfx(Sfx::Warp, None);
        if in_scorch(to.x) {
            self.advance("hotter");
            self.msg("Welcome to the Scorchlands. Mind the lava. Also everything else.");
        }
    }

    /// A joined player walked into a portal.
    pub fn host_use_portal(&mut self, from: u32, at: IVec3) {
        let Some(p) = self.peers.get(&from) else { return };
        let near = p.target.distance(at.as_vec3() + Vec3::splat(0.5)) < 3.0;
        // The Hollow's portals go straight there (or home).
        if near && self.world.get_v(at) == HOLLOW_PORTAL {
            let to = self.hollow_destination(at);
            if let Some(p) = self.peers.get_mut(&from) {
                p.target = to;
                p.pos = to;
            }
            self.net_send_to(from, Msg::Effect { heal: 0.0, teleport: Some(to), launch: None, take: None });
            return;
        }
        let here = is_portal(self.world.get_v(at)) || is_portal(self.world.get_v(at + IVec3::Y));
        if !near || !here {
            return;
        }
        let to = self.travel(at);
        if let Some(p) = self.peers.get_mut(&from) {
            p.target = to;
            p.pos = to;
        }
        self.net_send_to(from, Msg::Effect { heal: 0.0, teleport: Some(to), launch: None, take: None });
    }

    /// Where the world lives: find (or build) the portal at the other end, and a place to stand in it.
    /// Portals remember each other, so a round trip comes back where it started.
    pub fn travel(&mut self, from: IVec3) -> Vec3 {
        let key = portal_key(&self.world, from);
        if let Some(&to) = self.portal_links.get(&key) {
            let (cx, cz) = (to.x.div_euclid(CW), to.z.div_euclid(CW));
            for dz in -1..=1 {
                for dx in -1..=1 {
                    self.world.load_now(cx + dx, cz + dz);
                }
            }
            if is_portal(self.world.get_v(to)) {
                return stand_in(&self.world, to);
            }
            self.portal_links.remove(&key);
        }
        let there = self.find_or_build_portal(from);
        let there_key = portal_key(&self.world, there);
        self.portal_links.insert(key, there_key);
        self.portal_links.insert(there_key, key);
        stand_in(&self.world, there_key)
    }

    /// A portal near where `from` leads (an existing one close by, or a new one). Returns one of its cells.
    fn find_or_build_portal(&mut self, from: IVec3) -> IVec3 {
        let dest = destination(from);
        let (cx, cz) = (dest.x.div_euclid(CW), dest.z.div_euclid(CW));
        for dz in -2..=2 {
            for dx in -2..=2 {
                self.world.load_now(cx + dx, cz + dz);
            }
        }
        // An existing portal nearby?
        let mut best: Option<(i32, IVec3)> = None;
        for dz in -16..=16 {
            for dx in -16..=16 {
                for y in 1..CH - 1 {
                    let p = IVec3::new(dest.x + dx, y, dest.z + dz);
                    if is_portal(self.world.get_v(p)) && !is_portal(self.world.get_v(p - IVec3::Y)) {
                        let d = dx * dx + dz * dz + (y - dest.y).pow(2) / 4;
                        if best.is_none_or(|b| d < b.0) {
                            best = Some((d, p));
                        }
                    }
                }
            }
        }
        if let Some((_, p)) = best {
            return p;
        }
        let base = self.portal_site(dest);
        build_portal(&mut self.world, base);
        base
    }

    /// Somewhere to put a new portal near `dest`: on solid ground with room around it.
    fn portal_site(&self, dest: IVec3) -> IVec3 {
        let w = &self.world;
        let fits = |p: IVec3| -> bool {
            (-1..=2).all(|dx| (-1..=1).all(|dz| is_solid(w.get_v(p + IVec3::new(dx, -1, dz))) && (0..4).all(|dy| !is_solid(w.get_v(p + IVec3::new(dx, dy, dz))) && !is_liquid(w.get_v(p + IVec3::new(dx, dy, dz))))))
        };
        let ys: Vec<i32> = if in_scorch(dest.x as f32) { (LAVA_SEA + 2..CH - 8).collect() } else { vec![] };
        for r in 0..12i32 {
            for dz in -r..=r {
                for dx in -r..=r {
                    if dx.abs().max(dz.abs()) != r {
                        continue;
                    }
                    let (x, z) = (dest.x + dx * 2, dest.z + dz * 2);
                    if ys.is_empty() {
                        let p = IVec3::new(x, w.surface_y(x, z) + 1, z);
                        if fits(p) {
                            return p;
                        }
                    } else {
                        for &y in &ys {
                            let p = IVec3::new(x, y, z);
                            if fits(p) {
                                return p;
                            }
                        }
                    }
                }
            }
        }
        // Nowhere nice: carve a room.
        let y = if in_scorch(dest.x as f32) { 64 } else { w.surface_y(dest.x, dest.z) + 1 };
        IVec3::new(dest.x, y, dest.z)
    }
}

/// A portal's name: its lowest (then westmost, northmost) cell.
pub fn portal_key(world: &World, cell: IVec3) -> IVec3 {
    let id = world.get_v(cell);
    if !is_portal(id) {
        return cell;
    }
    let mut seen = std::collections::HashSet::from([cell]);
    let mut stack = vec![cell];
    let mut best = cell;
    while let Some(p) = stack.pop() {
        if (p.y, p.x, p.z) < (best.y, best.x, best.z) {
            best = p;
        }
        for d in [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z] {
            let q = p + d;
            if seen.len() < 512 && world.get_v(q) == id && seen.insert(q) {
                stack.push(q);
            }
        }
    }
    best
}

/// Where to stand in the portal whose key is `key`: its middle, along its width.
fn stand_in(world: &World, key: IVec3) -> Vec3 {
    let id = world.get_v(key);
    let along = if id == PORTAL_Z { IVec3::Z } else { IVec3::X };
    let wide = world.get_v(key + along) == id;
    let mut p = key.as_vec3() + Vec3::new(0.5, 0.0, 0.5);
    if wide {
        p += along.as_vec3() * 0.5;
    }
    p
}

/// Pack portal links for the save file.
pub fn encode_links(links: &std::collections::HashMap<IVec3, IVec3>) -> Vec<u8> {
    let mut out = Vec::new();
    let mut pairs: Vec<(&IVec3, &IVec3)> = links.iter().collect();
    pairs.sort_by_key(|(a, _)| (a.x, a.y, a.z));
    out.extend_from_slice(&(pairs.len() as u32).to_le_bytes());
    for (a, b) in pairs {
        for v in [a.x, a.y, a.z, b.x, b.y, b.z] {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    out
}

pub fn decode_links(b: &[u8]) -> std::collections::HashMap<IVec3, IVec3> {
    let mut map = std::collections::HashMap::new();
    let n = b.get(0..4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]])).unwrap_or(0) as usize;
    for k in 0..n.min(100_000) {
        let at = 4 + k * 24;
        let Some(s) = b.get(at..at + 24) else { break };
        let v: Vec<i32> = (0..6).map(|i| i32::from_le_bytes([s[i * 4], s[i * 4 + 1], s[i * 4 + 2], s[i * 4 + 3]])).collect();
        map.insert(IVec3::new(v[0], v[1], v[2]), IVec3::new(v[3], v[4], v[5]));
    }
    map
}

/// The cells inside an obsidian frame around `start`, in the x-y plane
/// (`axis_x`) or the z-y plane. None if it isn't a closed frame.
pub fn portal_frame(world: &World, start: IVec3, axis_x: bool) -> Option<Vec<IVec3>> {
    if !matches!(world.get_v(start), AIR | PORTAL_X | PORTAL_Z) {
        return None;
    }
    let side = if axis_x { IVec3::X } else { IVec3::Z };
    let mut cells = vec![start];
    let mut seen = std::collections::HashSet::from([start]);
    let mut i = 0;
    while i < cells.len() {
        let p = cells[i];
        i += 1;
        for d in [side, -side, IVec3::Y, IVec3::NEG_Y] {
            let q = p + d;
            let id = world.get_v(q);
            if id == OBSIDIAN || seen.contains(&q) {
                continue;
            }
            // Cells already lit (a joined player's edits arrive one at a time) count as inside.
            let lit = if axis_x { PORTAL_X } else { PORTAL_Z };
            if id != AIR && id != lit {
                return None;
            }
            seen.insert(q);
            cells.push(q);
            if cells.len() > (MAX_PORTAL * MAX_PORTAL) as usize {
                return None;
            }
        }
    }
    // At least 2 wide and 3 tall.
    let (min_s, max_s) = cells.iter().map(|c| if axis_x { c.x } else { c.z }).fold((i32::MAX, i32::MIN), |a, v| (a.0.min(v), a.1.max(v)));
    let (min_y, max_y) = cells.iter().map(|c| c.y).fold((i32::MAX, i32::MIN), |a, v| (a.0.min(v), a.1.max(v)));
    if max_s - min_s < 1 || max_y - min_y < 2 || max_s - min_s >= MAX_PORTAL || max_y - min_y >= MAX_PORTAL {
        return None;
    }
    Some(cells)
}

/// A 4x5 obsidian frame with a lit portal, standing on `base` (bottom-left of the inside),
/// with a platform and room to step out.
pub fn build_portal(world: &mut World, base: IVec3) {
    for dx in -1..=2 {
        for dz in -1..=1 {
            // A floor to stand on, and clear air around it.
            let floor = base + IVec3::new(dx, -1, dz);
            if !is_solid(world.get_v(floor)) || is_liquid(world.get_v(floor)) {
                world.set_v(floor, OBSIDIAN);
            }
            for dy in 0..4 {
                let p = base + IVec3::new(dx, dy, dz);
                if dz != 0 && world.get_v(p) != AIR {
                    world.set_v(p, AIR);
                }
            }
        }
    }
    for dx in -1..=2 {
        for dy in -1..=3 {
            let p = base + IVec3::new(dx, dy, 0);
            let frame = dx == -1 || dx == 2 || dy == -1 || dy == 3;
            world.set_v(p, if frame { OBSIDIAN } else { PORTAL_X });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destinations_scale_by_eight() {
        let d = destination(IVec3::new(800, 70, -160));
        assert_eq!((d.x, d.z), (SCORCH_ORIGIN + 100, -20));
        assert!(in_scorch(d.x as f32));
        let back = destination(IVec3::new(SCORCH_ORIGIN + 100, 40, -20));
        assert_eq!((back.x, back.z), (800, -160));
        assert!(!in_scorch(back.x as f32));
        // Far out in the ordinary world still lands inside the Scorchlands.
        assert!(destination(IVec3::new(1_000_000, 70, 0)).x > SCORCH_X + 16);
        assert!(in_wall(SCORCH_X - 5) && !in_wall(SCORCH_X - WALL - 1));
    }

    #[test]
    fn the_scorchlands_are_hot_and_enclosed() {
        let g = Generator::new(77);
        let b = g.generate(SCORCH_ORIGIN / CW, 0);
        let count = |id: Id| b.iter().filter(|&&x| x == id).count();
        assert!(count(SCORCHROCK) > 1000 && count(LAVA) > 50 && count(AIR) > 1000);
        // A bedrock sky, and bedrock in the band before it.
        assert!((0..CW).all(|x| b[crate::world::idx(x, CH - 1, 0)] == BEDROCK));
        let wall = g.generate((SCORCH_X - 100) / CW, 0);
        assert!(wall.iter().all(|&x| x == BEDROCK));
    }
}
