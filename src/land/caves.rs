//! Cave biomes: some stretches of the underground are more than tunnels.
//!
//! - **Dripstone caves**: tan Dripstone Blocks round the walls, floors bristling
//!   with Pointy Rocks and ceilings hung with them. The ones hanging from
//!   dripstone are loose: stand under one and it may shake free and fall on
//!   you (it hurts, and shatters into an item). Anything that loses what it
//!   hangs from falls too, and landing on an upright Pointy Rock doubles your
//!   fall damage. Mind your head, as the name says.
//! - **Lush caves**: moss on the floors and ceilings, moss carpet, azaleas and
//!   grass, little pools, and **Cave Vines** hanging down. Some vines carry
//!   **Glow Berries**, which light the cave: right-click to pick them (eat
//!   them, or plant one under a ceiling for a new vine). Vines grow, grow new
//!   berries, and you can climb them like a ladder. Bone Dust on moss spreads it.
//! - **Amethyst geodes**: round pockets in the rock, smooth basalt outside,
//!   then calcite, then amethyst. **Budding Amethyst** grows buds above and
//!   below it, which grow into **Amethyst Clusters**; break a cluster with a
//!   pickaxe for **Amethyst Shards** (amethyst blocks, tinted glass).
//!
//! Generation is a pure function of the seed (in `Generator`); the rest
//! happens where the world lives, and joined players see the edits.

use crate::block::*;
use crate::falling::FallingBlock;
use crate::game::Game;
use crate::noise::{hash2, hash3};
use crate::sound::{Mat, Sfx};
use crate::world::{idx, Generator, CH, CW};
use macroquad::math::{ivec3, IVec3, Vec3};

/// What kind of cave a column's caves are.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CaveBiome {
    Plain,
    Dripstone,
    Lush,
    DeepDark,
}

/// Shared climate, then depth. Depth is measured in the world's existing
/// coordinates; no new carving noise or independent biome noise is needed.
pub fn pick_cave(p: &crate::land::shape::Place, y: i32) -> CaveBiome {
    if y < 3 || y >= p.h - 8 { return CaveBiome::Plain; }
    if y < crate::deepdark::DEEP_TOP && p.c > 0.05 && p.e < -0.1 && p.m < 0.22 {
        return CaveBiome::DeepDark;
    }
    if y >= 24 && p.m > 0.08 && p.t > -0.17 {
        return CaveBiome::Lush;
    }
    if (10..=48).contains(&y) && p.m <= 0.08 && p.t > -0.32 && p.c > -0.12 {
        return CaveBiome::Dripstone;
    }
    CaveBiome::Plain
}

/// Geodes: one may sit in each square this wide.
const GEODE_REGION: i32 = 56;
const GEODE_CHANCE: f32 = 0.3;
/// Longest a cave vine grows.
pub const VINE_MAX: i32 = 8;
/// Chance a second's look finds a loose Pointy Rock over someone shaking free.
const SHAKE_CHANCE: f32 = 0.05;
/// How far above someone a loose rock can be and still fall on them.
const SHAKE_REACH: i32 = 12;

/// An amethyst geode: its middle and outer radius (the basalt rind).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geode {
    pub centre: IVec3,
    pub r: f32,
}

impl Geode {
    /// What the geode puts at distance `d` from its middle (None: outside it).
    /// `roll` (0..1) picks budding amethyst among the amethyst.
    fn layer(&self, d: f32, roll: f32) -> Option<Id> {
        if d <= self.r - 2.2 {
            Some(AIR)
        } else if d <= self.r - 1.2 {
            Some(if roll < 0.14 { BUDDING_AMETHYST } else { AMETHYST_BLOCK })
        } else if d <= self.r - 0.4 {
            Some(CALCITE)
        } else if d <= self.r + 0.5 {
            Some(SMOOTH_BASALT)
        } else {
            None
        }
    }
}

impl Generator {
    /// Actual cave identity, including depth. Older worlds retain their
    /// column-wide selections and the original Deep Dark region noise.
    pub fn cave_biome_at(&self, x: i32, y: i32, z: i32) -> CaveBiome {
        if self.dim != crate::dims::Dim::Over { return CaveBiome::Plain; }
        if self.opts.version >= 8 {
            return pick_cave(&self.place_v7(x, z), y);
        }
        if y < crate::deepdark::DEEP_TOP && self.deep_dark(x, z) && self.column(x, z).0 > crate::deepdark::DEEP_TOP + 6 {
            CaveBiome::DeepDark
        } else {
            self.cave_biome(x, z)
        }
    }

    /// The cave biome under (x, z).
    pub fn cave_biome(&self, x: i32, z: i32) -> CaveBiome {
        let (fx, fz) = (x as f32 / 110.0, z as f32 / 110.0);
        if self.cavern.noise3(fx, 777.5, fz) > 0.2 {
            CaveBiome::Dripstone
        } else if self.cavern.noise3(fx, 1333.5, fz) > 0.2 {
            CaveBiome::Lush
        } else {
            CaveBiome::Plain
        }
    }

    /// The geode in a region, if it has one (well underground).
    pub fn geode_in(&self, rx: i32, rz: i32) -> Option<Geode> {
        let s = self.seed ^ 0x6E0DE;
        if hash2(s, rx, rz) >= GEODE_CHANCE {
            return None;
        }
        let r = 5.5 + hash2(s ^ 1, rx, rz) * 2.0;
        let x = rx * GEODE_REGION + 8 + (hash2(s ^ 2, rx, rz) * (GEODE_REGION - 16) as f32) as i32;
        let z = rz * GEODE_REGION + 8 + (hash2(s ^ 3, rx, rz) * (GEODE_REGION - 16) as f32) as i32;
        let y = 12 + (hash2(s ^ 4, rx, rz) * 30.0) as i32;
        // Keep it well under the ground (and out of the sea floor), and out of the Deep Dark.
        if self.column(x, z).0 < y + r as i32 + 8 || (self.deep_dark(x, z) && y - (r as i32) < crate::deepdark::DEEP_TOP + 2) {
            return None;
        }
        Some(Geode { centre: ivec3(x, y, z), r })
    }

    /// The geodes that reach into chunk (cx, cz).
    pub fn geodes_for_chunk(&self, cx: i32, cz: i32) -> Vec<Geode> {
        let (x0, z0) = (cx * CW, cz * CW);
        let mut out = Vec::new();
        for rz in (z0 - 8).div_euclid(GEODE_REGION)..=(z0 + CW + 8).div_euclid(GEODE_REGION) {
            for rx in (x0 - 8).div_euclid(GEODE_REGION)..=(x0 + CW + 8).div_euclid(GEODE_REGION) {
                if let Some(g) = self.geode_in(rx, rz) {
                    let reach = g.r as i32 + 2;
                    if g.centre.x + reach >= x0 && g.centre.x - reach < x0 + CW && g.centre.z + reach >= z0 && g.centre.z - reach < z0 + CW {
                        out.push(g);
                    }
                }
            }
        }
        out
    }

    /// Dress one column's caves for its cave biome. `lo` is the lowest cave
    /// height it touches (above the Deep Dark), `h` the ground.
    pub(crate) fn cave_column(&self, b: &mut [Id], (lx, lz): (i32, i32), (x, z): (i32, i32), h: i32, lo: i32) {
        if self.opts.version >= 8 {
            let p = self.place_v7(x, z);
            let hi = (h - 8).min(CH - 2);
            // The depth selector reserves Deep Dark itself. The old column
            // exclusion would otherwise leave heights 24 and 25 undressed.
            let mut start = 3;
            while start < hi {
                let biome = pick_cave(&p, start);
                let mut end = start + 1;
                while end < hi && pick_cave(&p, end) == biome { end += 1; }
                self.cave_band(b, (lx, lz), (x, z), start, end, biome);
                start = end;
            }
            return;
        }
        let biome = self.cave_biome(x, z);
        self.cave_band(b, (lx, lz), (x, z), lo, (h - 6).min(CH - 2), biome);
    }

    /// Existing decorators, bounded to one run of the selected cave biome.
    fn cave_band(&self, b: &mut [Id], (lx, lz): (i32, i32), (x, z): (i32, i32), lo: i32, hi: i32, biome: CaveBiome) {
        if biome == CaveBiome::Plain {
            return;
        }
        let s = self.seed ^ 0xCA5E;
        let stony = |id: Id| matches!(id, STONE | DEEPSLATE | GRAVEL | DIRT);
        match biome {
            CaveBiome::Dripstone => {
                // The rock near open space turns to dripstone, in streaks.
                for y in lo..hi {
                    let i = idx(lx, y, lz);
                    if b[i] != STONE {
                        continue;
                    }
                    let near_air = (1..=2).any(|d| b[idx(lx, y + d, lz)] == AIR || b[idx(lx, y - d, lz)] == AIR);
                    if (near_air && hash3(s, x, y, z) < 0.8) || hash3(s ^ 1, x >> 2, y >> 1, z >> 2) < 0.25 {
                        b[i] = DRIPSTONE_BLOCK;
                    }
                }
                // Pointy Rocks, up from the floors and down from the ceilings, one or two long.
                for y in lo.max(2)..hi {
                    if b[idx(lx, y, lz)] != AIR {
                        continue;
                    }
                    let (below, above) = (b[idx(lx, y - 1, lz)], b[idx(lx, y + 1, lz)]);
                    let r = hash3(s ^ 2, x, y, z);
                    if matches!(below, DRIPSTONE_BLOCK | STONE) && r < 0.14 {
                        b[idx(lx, y, lz)] = POINTY_ROCK;
                        if r < 0.05 && (self.opts.version < 8 || y + 2 < hi) && b[idx(lx, y + 1, lz)] == AIR && b[idx(lx, y + 2, lz)] == AIR {
                            b[idx(lx, y + 1, lz)] = POINTY_ROCK;
                        }
                    } else if matches!(above, DRIPSTONE_BLOCK | STONE) && r > 0.84 {
                        b[idx(lx, y, lz)] = POINTY_ROCK;
                        if r > 0.95 && (self.opts.version < 8 || y - 2 >= lo) && b[idx(lx, y - 1, lz)] == AIR && b[idx(lx, y - 2, lz)] == AIR {
                            b[idx(lx, y - 1, lz)] = POINTY_ROCK;
                        }
                    }
                }
            }
            CaveBiome::Lush => {
                for y in lo.max(2)..hi {
                    let i = idx(lx, y, lz);
                    if b[i] != AIR {
                        continue;
                    }
                    let (bi, ai) = (idx(lx, y - 1, lz), idx(lx, y + 1, lz));
                    let r = hash3(s ^ 3, x, y, z);
                    if stony(b[bi]) && (self.opts.version < 8 || y > lo) {
                        // A mossy floor, now and then a little pool.
                        b[bi] = if r < 0.04 && stony(b[idx(lx, y - 2, lz)]) { WATER } else { MOSS_BLOCK };
                        if b[bi] == MOSS_BLOCK {
                            b[i] = match hash3(s ^ 4, x, y, z) {
                                v if v < 0.22 => MOSS_CARPET,
                                v if v < 0.27 => AZALEA,
                                v if v < 0.4 => TALL_GRASS,
                                _ => AIR,
                            };
                        }
                    }
                    if stony(b[ai]) && (self.opts.version < 8 || y + 1 < hi) {
                        if r > 0.45 {
                            b[ai] = MOSS_BLOCK;
                        }
                        // Vines hang from the ceiling, some with berries on.
                        if hash3(s ^ 5, x, y, z) < 0.13 {
                            let len = 1 + (hash3(s ^ 6, x, y, z) * 4.0) as i32;
                            for k in 0..len {
                                let j = idx(lx, y - k, lz);
                                if y - k < lo || b[j] != AIR {
                                    break;
                                }
                                b[j] = if hash3(s ^ 7, x, y - k, z) < 0.3 { CAVE_VINES_LIT } else { CAVE_VINES };
                            }
                        }
                    }
                }
            }
            CaveBiome::Plain | CaveBiome::DeepDark => {}
        }
    }

    /// Cut this column's slice of any geodes, buds and all.
    pub(crate) fn geode_column(&self, b: &mut [Id], lx: i32, lz: i32, x: i32, z: i32, geodes: &[Geode]) {
        let s = self.seed ^ 0x6E0;
        for g in geodes {
            let (dx, dz) = ((x - g.centre.x) as f32, (z - g.centre.z) as f32);
            if dx * dx + dz * dz > (g.r + 1.0) * (g.r + 1.0) {
                continue;
            }
            let reach = g.r as i32 + 2;
            let (y0, y1) = ((g.centre.y - reach).max(1), (g.centre.y + reach).min(CH - 2));
            for y in y0..=y1 {
                let i = idx(lx, y, lz);
                if b[i] == BEDROCK {
                    continue;
                }
                // A little lumpy, so it isn't a perfect ball.
                let d = (dx * dx + ((y - g.centre.y) as f32 * 1.1).powi(2) + dz * dz).sqrt() + (hash3(s, x, y, z) - 0.5) * 0.5;
                match g.layer(d, hash3(s ^ 1, x, y, z)) {
                    Some(AIR) => b[i] = AIR,
                    // The rind only goes into rock, so a cave through it stays open.
                    Some(id) if b[i] != AIR && !is_liquid(b[i]) => b[i] = id,
                    _ => {}
                }
            }
            // Buds on the budding amethyst, standing on the floor and hanging from the roof.
            for y in y0.max(2)..y1 {
                let i = idx(lx, y, lz);
                if b[i] != AIR {
                    continue;
                }
                let r = hash3(s ^ 2, x, y, z);
                let on = b[idx(lx, y - 1, lz)] == BUDDING_AMETHYST || b[idx(lx, y + 1, lz)] == BUDDING_AMETHYST;
                if on && r < 0.7 {
                    b[i] = [AMETHYST_BUD_SMALL, AMETHYST_BUD_LARGE, AMETHYST_CLUSTER][(r * 4.3) as usize % 3];
                }
            }
        }
    }
}

/// Is this a bud or cluster?
pub fn is_amethyst_bud(id: Id) -> bool {
    (AMETHYST_BUD_SMALL..=AMETHYST_CLUSTER).contains(&id)
}

pub fn is_cave_vine(id: Id) -> bool {
    matches!(id, CAVE_VINES | CAVE_VINES_LIT)
}

/// Things drawn pointing down when they hang from something.
pub fn can_hang(id: Id) -> bool {
    id == POINTY_ROCK || is_amethyst_bud(id)
}

/// Which piece of its spike a Pointy Rock at height `y` is (the tile it's
/// drawn with): alone, a stack's tip, its base (against what holds it up) or
/// a middle. `hanging` says which way the spike points.
pub fn rock_piece(get: impl Fn(i32) -> Id, y: i32, hanging: bool) -> u16 {
    use crate::texture::*;
    let run = |step: i32| (1..6).take_while(|k| get(y + k * step) == POINTY_ROCK).count();
    let (below, above) = (run(-1), run(1));
    // Towards the point, and towards what it grows from.
    let (to_tip, to_base) = if hanging { (below, above) } else { (above, below) };
    match (to_tip, to_base) {
        (0, 0) => T_POINTY_ROCK,
        (0, _) => T_POINTY_TIP,
        (_, 0) => T_POINTY_BASE,
        _ => T_POINTY_MIDDLE,
    }
}

/// Does the Pointy Rock (or bud) at height `y` hang? `get` reads the column.
/// Follow a stack of them down: if it ends on something solid they stand,
/// otherwise they hang.
pub fn hangs(get: impl Fn(i32) -> Id, y: i32) -> bool {
    let id = get(y);
    let mut k = y - 1;
    while k > y - 6 && get(k) == id {
        k -= 1;
    }
    let below = get(k);
    !is_solid(below) && is_solid_or_same(get(y + 1), id, &get, y + 1)
}

/// Is what's above (at `y`) something to hang from: solid, or more of the same hanging from something solid?
fn is_solid_or_same(above: Id, id: Id, get: &impl Fn(i32) -> Id, y: i32) -> bool {
    if is_solid(above) {
        return true;
    }
    let mut k = y;
    while k < y + 5 && get(k) == id {
        k += 1;
    }
    get(k - 1) == id && is_solid(get(k))
}

/// Is a Pointy Rock at `p` held up (standing on something, or hanging from something)?
pub fn rock_supported(world: &crate::world::World, p: IVec3) -> bool {
    let get = |y: i32| world.get(p.x, y, p.z);
    let mut k = p.y - 1;
    while k > p.y - 8 && get(k) == POINTY_ROCK {
        k -= 1;
    }
    if is_solid(get(k)) {
        return true;
    }
    let mut k = p.y + 1;
    while k < p.y + 8 && get(k) == POINTY_ROCK {
        k += 1;
    }
    is_solid(get(k))
}

impl Game {
    /// Now and then, a loose Pointy Rock over someone's head in a dripstone
    /// cave shakes free and falls on them.
    pub fn stalactite_tick(&mut self, dt: f32) {
        if self.is_client() {
            return;
        }
        self.shake_acc += dt;
        if self.shake_acc < 1.0 {
            return;
        }
        self.shake_acc -= 1.0;
        let mut heads: Vec<Vec3> = self.peers.values().filter(|p| p.alive()).map(|p| p.target).collect();
        if !self.away() && self.dead.is_none() {
            heads.push(self.player.body.pos);
        }
        for at in heads {
            if !self.rng.chance(SHAKE_CHANCE) {
                continue;
            }
            let (x, z) = (at.x.floor() as i32, at.z.floor() as i32);
            let start = at.y.floor() as i32 + 2;
            for y in start..(start + SHAKE_REACH).min(CH - 1) {
                let id = self.world.get(x, y, z);
                if id == AIR {
                    continue;
                }
                if id == POINTY_ROCK && self.loose_rock(ivec3(x, y, z)) {
                    self.drop_rock_stack(ivec3(x, y, z));
                }
                break;
            }
        }
    }

    /// Is the Pointy Rock at `p` hanging from dripstone (so it can shake loose)?
    fn loose_rock(&self, p: IVec3) -> bool {
        let mut k = p.y;
        while k < p.y + 6 && self.world.get(p.x, k, p.z) == POINTY_ROCK {
            k += 1;
        }
        !is_solid(self.world.get(p.x, p.y - 1, p.z)) && self.world.get(p.x, k, p.z) == DRIPSTONE_BLOCK
    }

    /// The hanging stack of Pointy Rocks whose lowest is at `p` falls.
    fn drop_rock_stack(&mut self, p: IVec3) {
        let mut y = p.y;
        while self.world.get(p.x, y, p.z) == POINTY_ROCK {
            self.world.set(p.x, y, p.z, AIR);
            self.falling.push(FallingBlock { pos: ivec3(p.x, y, p.z).as_vec3(), vel: 0.0, id: POINTY_ROCK, from: y as f32 });
            y += 1;
        }
        self.sfx(Sfx::Break(Mat::Stone), Some(p.as_vec3() + Vec3::splat(0.5)));
    }

    /// A falling Pointy Rock landed at `at` having fallen `fell` blocks: it
    /// skewers whoever is under it, and shatters.
    pub fn rock_lands(&mut self, at: Vec3, fell: f32) {
        self.sfx(Sfx::Break(Mat::Stone), Some(at));
        self.pop_drop(at + Vec3::Y * 0.3, POINTY_ROCK, 1);
        let dmg = (2.0 + fell.max(0.0) * 1.5).min(20.0);
        if fell < 1.0 {
            return;
        }
        let under = |p: Vec3, h: f32| (p.x - at.x).abs() < 0.7 && (p.z - at.z).abs() < 0.7 && p.y <= at.y + 1.0 && p.y + h >= at.y - 0.2;
        for m in self.mobs.iter_mut().filter(|m| under(m.body.pos, m.body.height)) {
            m.damage(dmg, at + Vec3::Y);
        }
        const DEATH: &str = "was skewered by a falling Pointy Rock. It did say 'Mind Your Head'.";
        if !self.away() && under(self.player.body.pos, 1.8) {
            self.player.hurt = 0.0;
            self.hurt_player(dmg, DEATH);
            self.advance("mind_your_head");
        }
        let peers: Vec<u32> = self.peers.iter().filter(|(_, p)| p.alive() && under(p.target, 1.8)).map(|(&id, _)| id).collect();
        for id in peers {
            self.hurt_peer(id, dmg, DEATH, Vec3::ZERO);
        }
    }

    /// Right-click lit cave vines: pick the glow berries.
    pub fn pick_berries(&mut self, pos: IVec3) -> bool {
        if self.world.get_v(pos) != CAVE_VINES_LIT {
            return false;
        }
        self.world.set_v(pos, CAVE_VINES);
        self.player.swing = 1.0;
        self.sfx(Sfx::Place(Mat::Grass), Some(pos.as_vec3() + Vec3::splat(0.5)));
        // A joined player's berries come from the host (see ledger.rs).
        if !self.is_client() {
            self.pop_drop(pos.as_vec3() + Vec3::new(0.5, 0.2, 0.5), GLOW_BERRIES, 1);
        }
        self.advance("glow_up");
        true
    }

    /// Plant glow berries under a ceiling (or under the end of a vine): a new vine.
    pub fn plant_berries(&mut self, hit: IVec3, normal: IVec3, hit_id: Id) -> bool {
        if !(normal == IVec3::NEG_Y && (is_solid(hit_id) || is_cave_vine(hit_id))) {
            return false;
        }
        let at = hit - IVec3::Y;
        if at.y < 1 || !replaceable(self.world.get_v(at)) || is_liquid(self.world.get_v(at)) {
            return false;
        }
        self.world.set_v(at, CAVE_VINES);
        self.sfx(Sfx::Place(Mat::Grass), Some(at.as_vec3() + Vec3::splat(0.5)));
        self.player.swing = 1.0;
        if !self.creative {
            self.inv.consume_held();
        }
        true
    }

    /// Cave vines grow down and grow berries; budding amethyst grows buds,
    /// and buds grow into clusters. (Random ticks: see copper.rs.)
    pub fn cave_tick(&mut self, p: IVec3, id: Id) {
        if is_cave_vine(id) {
            if id == CAVE_VINES && self.rng.chance(0.03) {
                self.world.set_v(p, CAVE_VINES_LIT);
            }
            let below = p - IVec3::Y;
            if below.y > 1 && self.world.get_v(below) == AIR && self.rng.chance(0.04) {
                let mut len = 1;
                while len < VINE_MAX && is_cave_vine(self.world.get_v(p + IVec3::Y * len)) {
                    len += 1;
                }
                if len < VINE_MAX {
                    self.world.set_v(below, CAVE_VINES);
                }
            }
        } else if id == BUDDING_AMETHYST {
            for side in [IVec3::Y, IVec3::NEG_Y] {
                let q = p + side;
                if !(1..CH - 1).contains(&q.y) || !self.rng.chance(0.1) {
                    continue;
                }
                match self.world.get_v(q) {
                    AIR => self.world.set_v(q, AMETHYST_BUD_SMALL),
                    AMETHYST_BUD_SMALL => self.world.set_v(q, AMETHYST_BUD_LARGE),
                    AMETHYST_BUD_LARGE => self.world.set_v(q, AMETHYST_CLUSTER),
                    _ => {}
                }
            }
        }
    }

    /// Breaking a block takes down what hangs from it: vines drop (and their
    /// berries), buds break off. (Pointy Rocks fall: see falling.rs.)
    pub fn drop_hangers(&mut self, pos: IVec3) {
        let mut q = pos - IVec3::Y;
        let first = self.world.get_v(q);
        if is_amethyst_bud(first) && !is_solid(self.world.get_v(q - IVec3::Y)) {
            self.world.set_v(q, AIR);
            return;
        }
        while q.y > 0 && (is_cave_vine(self.world.get_v(q)) || self.world.get_v(q) == WEEPING_VINES) {
            if self.world.get_v(q) == CAVE_VINES_LIT && !self.creative && !self.is_client() {
                self.pop_drop(q.as_vec3() + Vec3::splat(0.5), GLOW_BERRIES, 1);
            }
            self.world.set_v(q, AIR);
            q -= IVec3::Y;
        }
    }

    /// Bone Dust on a moss block: moss spreads over the stone and dirt round
    /// it, with carpet, azaleas and grass on top. (Where the world lives:
    /// joined players ask the host; see farming.rs.)
    pub fn grow_moss(&mut self, pos: IVec3) {
        if self.world.get_v(pos) != MOSS_BLOCK {
            return;
        }
        for dz in -2..=2 {
            for dx in -2..=2 {
                for dy in -1..=1 {
                    let q = pos + ivec3(dx, dy, dz);
                    if !matches!(self.world.get_v(q), STONE | DIRT | GRASS | DEEPSLATE | MOSS_BLOCK) || self.world.get_v(q + IVec3::Y) != AIR || !self.rng.chance(0.7) {
                        continue;
                    }
                    self.world.set_v(q, MOSS_BLOCK);
                    let top = match self.rng.f32() {
                        v if v < 0.25 => MOSS_CARPET,
                        v if v < 0.3 => AZALEA,
                        v if v < 0.45 => TALL_GRASS,
                        _ => AIR,
                    };
                    if top != AIR {
                        self.world.set_v(q + IVec3::Y, top);
                    }
                }
            }
        }
        self.sfx(Sfx::Place(Mat::Grass), Some(pos.as_vec3() + Vec3::splat(0.5)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::GenOptions;

    #[test]
    fn review_depth_transition_has_no_legacy_exclusion_gap() {
        let _lock = crate::mods::registry_test_lock();
        let g = Generator::with(77, GenOptions::DEFAULT);
        let (x,z) = (-2400..2400).step_by(32).flat_map(|z| (-2400..2400).step_by(32).map(move |x| (x,z)))
            .find(|&(x,z)| g.cave_biome_at(x,10,z)==CaveBiome::DeepDark && g.cave_biome_at(x,25,z)==CaveBiome::Lush).unwrap();
        let mut b = vec![STONE; (CW*CW*CH) as usize];
        b[idx(0,23,0)] = BEDROCK; // No random little pool under the test floor.
        for y in 25..30 { b[idx(0,y,0)] = AIR; }
        g.cave_column(&mut b,(0,0),(x,z),g.column(x,z).0,crate::deepdark::DEEP_TOP+2);
        assert_eq!(b[idx(0,24,0)],MOSS_BLOCK,"the first lush floor above Deep Dark must be dressed");
    }

    #[test]
    fn cave_rules_share_climate_and_respect_depth() {
        let g = Generator::with(77, GenOptions::DEFAULT);
        let base = crate::land::shape::Place { h: 90, sea: 63, c: 0.4, e: -0.3, w: 0.2, pv: 0.0, t: 0.1, m: 0.0, river: false, wet_low: 0.0 };
        assert_eq!(pick_cave(&base, 10), CaveBiome::DeepDark);
        assert_eq!(pick_cave(&base, 23), CaveBiome::DeepDark);
        assert_eq!(pick_cave(&base, 24), CaveBiome::Dripstone);
        assert_eq!(pick_cave(&base, 48), CaveBiome::Dripstone);
        assert_eq!(pick_cave(&base, 49), CaveBiome::Plain);
        assert_eq!(pick_cave(&base, 2), CaveBiome::Plain);
        assert_eq!(pick_cave(&base, 82), CaveBiome::Plain);
        let wet = crate::land::shape::Place { m: 0.3, ..base };
        assert_eq!(pick_cave(&wet, 23), CaveBiome::Plain);
        assert_eq!(pick_cave(&wet, 24), CaveBiome::Lush);
        assert_eq!(pick_cave(&crate::land::shape::Place { t: -0.4, ..wet }, 40), CaveBiome::Plain);
        for z in (-1000..1000).step_by(80) {
            for x in (-1000..1000).step_by(80) {
                assert_eq!(g.deep_dark(x, z), g.cave_biome_at(x, crate::deepdark::CITY_Y, z) == CaveBiome::DeepDark);
                assert_eq!(g.cave_biome_at(x, 40, z), pick_cave(&g.place_v7(x, z), 40));
            }
        }
    }

    #[test]
    fn revision_eight_decorates_each_depth_without_crossing_bands() {
        let _lock = crate::mods::registry_test_lock();
        let g = Generator::with(77, GenOptions { structures: 0, ..GenOptions::DEFAULT });
        let mut found = std::collections::HashSet::new();
        for want in [CaveBiome::Dripstone, CaveBiome::Lush, CaveBiome::DeepDark] {
            let mut n = 0;
            'search: for z in (-2400..2400).step_by(64) {
                for x in (-2400..2400).step_by(64) {
                    let depth = if want == CaveBiome::DeepDark { 10 } else { 32 };
                    if g.cave_biome_at(x, depth, z) != want { continue; }
                    let (cx, cz) = (x.div_euclid(CW), z.div_euclid(CW));
                    let b = g.generate(cx, cz);
                    for lz in 0..CW {
                        for lx in 0..CW {
                            let (xx, zz) = (cx * CW + lx, cz * CW + lz);
                            for y in 3..g.column(xx, zz).0 - 8 {
                                let id = b[idx(lx, y, lz)];
                                if [DRIPSTONE_BLOCK, MOSS_BLOCK, CAVE_VINES, CAVE_VINES_LIT, SCULK].contains(&id) {
                                    found.insert(id);
                                    let expected = if id == DRIPSTONE_BLOCK { CaveBiome::Dripstone } else if id == SCULK { CaveBiome::DeepDark } else { CaveBiome::Lush };
                                    assert_eq!(g.cave_biome_at(xx, y, zz), expected, "{} at {xx},{y},{zz}", block(id).key);
                                }
                            }
                        }
                    }
                    n += 1;
                    if n == 12 { break 'search; }
                }
            }
            assert!(n > 0, "no {want:?} columns");
        }
        for id in [DRIPSTONE_BLOCK, MOSS_BLOCK, CAVE_VINES, CAVE_VINES_LIT, SCULK] { assert!(found.contains(&id), "no {}", block(id).key); }
    }

    #[test]
    fn long_spikes_are_drawn_as_one() {
        use crate::texture::*;
        // A stalagmite three long on the floor (y 1..=3), and a stalactite two long from the ceiling (y 8..=9).
        let col = |y: i32| match y {
            0 | 10 => STONE,
            1..=3 | 8 | 9 => POINTY_ROCK,
            _ => AIR,
        };
        let piece = |y: i32| rock_piece(col, y, hangs(col, y));
        assert_eq!([piece(1), piece(2), piece(3)], [T_POINTY_BASE, T_POINTY_MIDDLE, T_POINTY_TIP], "a stalagmite grows up to its point");
        assert_eq!([piece(9), piece(8)], [T_POINTY_BASE, T_POINTY_TIP], "a stalactite hangs down to its point");
        assert!(hangs(col, 8) && !hangs(col, 3));
        // On its own it's a whole little cone.
        let lone = |y: i32| if y == 0 { STONE } else if y == 1 { POINTY_ROCK } else { AIR };
        assert_eq!(rock_piece(lone, 1, false), T_POINTY_ROCK);
    }

    #[test]
    fn cave_biomes_and_geodes_turn_up() {
        let worldgen = Generator::with(7, GenOptions { version: 7, ..GenOptions::DEFAULT });
        let (mut drip, mut lush) = (0, 0);
        for z in (-2000..2000).step_by(40) {
            for x in (-2000..2000).step_by(40) {
                match worldgen.cave_biome(x, z) {
                    CaveBiome::Dripstone => drip += 1,
                    CaveBiome::Lush => lush += 1,
                    CaveBiome::Plain | CaveBiome::DeepDark => {}
                }
            }
        }
        // Each is a sizeable minority of the underground.
        assert!(drip > 300 && drip < 4000, "dripstone {drip}");
        assert!(lush > 300 && lush < 4000, "lush {lush}");
        let geodes = (-20..20).flat_map(|z| (-20..20).map(move |x| (x, z))).filter_map(|(x, z)| worldgen.geode_in(x, z)).count();
        assert!(geodes > 40, "geodes {geodes}");
    }

    #[test]
    fn generated_caves_have_their_blocks() {
        let worldgen = Generator::with(7, GenOptions { version: 7, ..GenOptions::DEFAULT });
        let mut seen = std::collections::HashSet::new();
        // Find a chunk with a geode in it, and chunks in each cave biome.
        let g = (0..40).flat_map(|z| (0..40).map(move |x| (x, z))).find_map(|(x, z)| worldgen.geode_in(x, z)).expect("a geode");
        for id in worldgen.generate(g.centre.x.div_euclid(CW), g.centre.z.div_euclid(CW)) {
            seen.insert(id);
        }
        for id in [CALCITE, SMOOTH_BASALT, AMETHYST_BLOCK, BUDDING_AMETHYST] {
            assert!(seen.contains(&id), "geode has {}", block(id).key);
        }
        let found = |want: CaveBiome, ids: &[Id]| {
            let mut all = std::collections::HashSet::new();
            let mut n = 0;
            'out: for z in (0..3000).step_by(48) {
                for x in (0..3000).step_by(48) {
                    if worldgen.cave_biome(x, z) == want {
                        all.extend(worldgen.generate(x.div_euclid(CW), z.div_euclid(CW)));
                        n += 1;
                        if n > 12 {
                            break 'out;
                        }
                    }
                }
            }
            for &id in ids {
                assert!(all.contains(&id), "{want:?} has {}", block(id).key);
            }
        };
        found(CaveBiome::Dripstone, &[DRIPSTONE_BLOCK, POINTY_ROCK]);
        found(CaveBiome::Lush, &[MOSS_BLOCK, CAVE_VINES, CAVE_VINES_LIT, MOSS_CARPET]);
    }

    #[test]
    fn hanging_and_standing_rocks() {
        // Floor at 10, ceiling at 20.
        let col = |rocks: &'static [i32]| move |y: i32| if y <= 10 || y >= 20 { STONE } else if rocks.contains(&y) { POINTY_ROCK } else { AIR };
        assert!(!hangs(col(&[11]), 11), "on the floor it stands");
        assert!(hangs(col(&[19]), 19), "from the ceiling it hangs");
        assert!(hangs(col(&[18, 19]), 18) && hangs(col(&[18, 19]), 19), "a hanging pair both hang");
        assert!(!hangs(col(&[11, 12]), 12), "a standing pair both stand");
    }

    #[test]
    fn loose_rocks_fall_and_hurt() {
        let mut g = crate::game::tests::arena(81);
        let base = ivec3(4, 50, 4);
        // A dripstone ceiling eight up, a rock hanging from it, and a Mooer underneath.
        g.world.set_v(base + IVec3::Y * 8, DRIPSTONE_BLOCK);
        g.world.set_v(base + IVec3::Y * 7, POINTY_ROCK);
        let mut rng = crate::noise::Rng::new(4);
        let mut cow = crate::entity::Mob::new(crate::entity::MobKind::Mooer, base.as_vec3() + Vec3::new(0.5, 0.0, 0.5), &mut rng);
        cow.persistent = true;
        let before = cow.health;
        g.mobs.push(cow);
        assert!(g.loose_rock(base + IVec3::Y * 7));
        g.drop_rock_stack(base + IVec3::Y * 7);
        for _ in 0..100 {
            g.falling_tick(0.05);
        }
        assert_eq!(g.world.get_v(base + IVec3::Y * 7), AIR);
        assert!(g.mobs[0].health < before, "skewered");
        assert!(g.drops.iter().any(|d| d.item == POINTY_ROCK), "it shattered into an item");

        // Take away what one hangs from: it falls.
        let up = ivec3(-4, 60, -4);
        g.world.set_v(up + IVec3::Y, STONE);
        g.world.set_v(up, POINTY_ROCK);
        g.falling_tick(0.05);
        assert_eq!(g.world.get_v(up), POINTY_ROCK, "held up");
        g.world.set_v(up + IVec3::Y, AIR);
        for _ in 0..200 {
            g.falling_tick(0.05);
        }
        assert_eq!(g.world.get_v(up), AIR, "it fell");
    }

    #[test]
    fn berries_pick_plant_and_vines_grow() {
        let mut g = crate::game::tests::arena(82);
        let roof = ivec3(2, 56, 2);
        g.world.set_v(roof, STONE);
        g.world.set_v(roof - IVec3::Y, CAVE_VINES_LIT);
        assert!(g.pick_berries(roof - IVec3::Y));
        assert_eq!(g.world.get_v(roof - IVec3::Y), CAVE_VINES);
        assert!(g.drops.iter().any(|d| d.item == GLOW_BERRIES));
        // Plant under another bit of roof.
        let roof2 = ivec3(6, 56, 2);
        g.world.set_v(roof2, STONE);
        g.inv.slots[g.inv.selected] = Some((GLOW_BERRIES, 3));
        assert!(g.plant_berries(roof2, IVec3::NEG_Y, STONE));
        assert_eq!(g.world.get_v(roof2 - IVec3::Y), CAVE_VINES);
        assert!(!g.plant_berries(roof2 + IVec3::Y * 3, IVec3::Y, STONE), "only under things");
        // Given time, vines grow down and grow berries.
        for _ in 0..400 {
            let p = roof2 - IVec3::Y;
            let id = g.world.get_v(p);
            g.cave_tick(p, id);
        }
        assert!(is_cave_vine(g.world.get_v(roof2 - IVec3::Y * 2)), "grew down");
        // Breaking the roof brings the vine down.
        g.drop_hangers(roof2);
        assert_eq!(g.world.get_v(roof2 - IVec3::Y), AIR);
    }

    #[test]
    fn budding_amethyst_grows_clusters() {
        let mut g = crate::game::tests::arena(83);
        let p = ivec3(-2, 55, 3);
        g.world.set_v(p, BUDDING_AMETHYST);
        for _ in 0..300 {
            g.cave_tick(p, BUDDING_AMETHYST);
        }
        assert_eq!(g.world.get_v(p + IVec3::Y), AMETHYST_CLUSTER);
        assert_eq!(g.world.get_v(p - IVec3::Y), AMETHYST_CLUSTER);
        assert_eq!(block(AMETHYST_CLUSTER).drop, AMETHYST_SHARD);
        // The cluster under it hangs (and is drawn pointing down).
        assert!(hangs(|y| g.world.get(p.x, y, p.z), p.y - 1));
        assert!(!hangs(|y| g.world.get(p.x, y, p.z), p.y + 1));
    }
}

