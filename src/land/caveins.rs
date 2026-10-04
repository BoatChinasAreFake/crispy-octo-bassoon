//! Cave-ins: dig out too big a room underground and the ceiling comes down.
//!
//! Every cell a player digs out underground is remembered (natural caves
//! have had ages to settle; your digging hasn't). After each dig, any dug
//! cell whose ceiling is more than **5 blocks** from something holding it
//! up is trouble. What holds a ceiling up: a wall or a pillar (anything
//! solid at the same height), or a wooden beam (a log in the ceiling) within
//! 3 blocks. So a room up to about nine blocks across is fine; bigger needs
//! pillars or beams.
//!
//! Trouble starts with a creak and dust falling for a few seconds: time to
//! put a pillar or a beam in, or to leave. Then whatever's still unsupported
//! comes down as rubble, on whoever is under it.
//!
//! The **Support Gauge** reads the room you're standing in: how far its
//! ceiling is from support, as a percentage (100% is snug; 0% is coming
//! down if you dug it). It works from the blocks around you, so it reads
//! natural caves too, which only ever stay up because they're old.
//!
//! Cave-ins happen where the world lives (the falling rubble is synced like
//! any falling block); the creaking comes to everyone nearby as a message.

use crate::block::*;
use crate::game::Game;
use crate::sound::Sfx;
use crate::world::World;
use macroquad::math::{IVec3, Vec3};

/// The furthest a ceiling can be from support and stay up.
pub const MAX_SPAN: i32 = 5;
/// How far a beam (a log in the ceiling) holds a ceiling up around it.
pub const BEAM_REACH: i32 = 3;
/// How far to look for a wall.
const LOOK: i32 = 8;
/// Seconds of creaking before it comes down.
pub const WARN_SECS: f32 = 4.0;
/// How far round a dig to look for trouble.
const ROOM: i32 = 10;
/// The most dug cells remembered.
const DUG_CAP: usize = 60_000;
/// Damage to someone under a falling ceiling.
const CRUSH: f32 = 6.0;

/// A creaking ceiling.
#[derive(Clone, Debug)]
pub struct CaveIn {
    /// The dug cells under the unsupported ceiling.
    pub cells: Vec<IVec3>,
    pub t: f32,
    dust: f32,
}

/// Is `p` underground: under a few blocks of rock?
pub fn underground(world: &World, p: IVec3) -> bool {
    (1..=16).filter(|&k| is_opaque(world.get_v(p + IVec3::Y * k))).count() >= 3 && (1..=3).any(|k| is_solid(world.get_v(p + IVec3::Y * k)))
}

/// The ceiling over an open cell (within 3 above), if there is one.
pub fn ceiling(world: &World, c: IVec3) -> Option<IVec3> {
    (1..=3).map(|k| c + IVec3::Y * k).find(|&q| is_solid(world.get_v(q)))
}

/// How far the ceiling over `c` is from something holding it up (None: no ceiling over it).
pub fn span(world: &World, c: IVec3) -> Option<i32> {
    let roof = ceiling(world, c)?;
    for r in 0..=LOOK {
        for dz in -r..=r {
            for dx in -r..=r {
                if dx.abs().max(dz.abs()) != r {
                    continue;
                }
                let q = IVec3::new(c.x + dx, c.y, c.z + dz);
                if r > 0 && is_solid(world.get_v(q)) {
                    return Some(r);
                }
                if r <= BEAM_REACH && is_log(world.get_v(IVec3::new(q.x, roof.y, q.z))) {
                    return Some(r);
                }
            }
        }
    }
    Some(LOOK + 1)
}

/// The dug cells round `p` whose ceilings are too far from support.
pub fn unsupported(world: &World, p: IVec3) -> Vec<IVec3> {
    let mut out = Vec::new();
    for dy in -3..=3 {
        for dz in -ROOM..=ROOM {
            for dx in -ROOM..=ROOM {
                let c = p + IVec3::new(dx, dy, dz);
                if world.dug.contains(&c) && world.get_v(c) == AIR && span(world, c).is_some_and(|s| s > MAX_SPAN) {
                    out.push(c);
                }
            }
        }
    }
    out
}

/// The Support Gauge's reading at `feet`: the worst span among the open
/// cells round you, as 0 to 100 (None: open sky, or no ceiling to speak of).
pub fn gauge(world: &World, feet: IVec3) -> Option<i32> {
    let mut worst: Option<i32> = None;
    for dz in -5..=5 {
        for dx in -5..=5 {
            let c = feet + IVec3::new(dx, 0, dz);
            if world.get_v(c) != AIR {
                continue;
            }
            if let Some(s) = span(world, c) {
                worst = Some(worst.map_or(s, |w| w.max(s)));
            }
        }
    }
    worst.map(|w| ((MAX_SPAN + 1 - w).max(0) * 100 / MAX_SPAN).min(100))
}

/// What the gauge says about a reading.
pub fn gauge_words(pct: i32) -> &'static str {
    match pct {
        60.. => "solid",
        20.. => "shaky",
        _ => "too wide: prop it up",
    }
}

/// What a ceiling block comes down as.
fn rubble(id: Id) -> Id {
    match id {
        STONE => COBBLE,
        _ => id,
    }
}

impl Game {
    /// Where the world lives: a block was dug out at `p` (checked next tick, once it's gone).
    pub fn dug_out(&mut self, p: IVec3) {
        if !self.is_client() {
            self.dig_queue.push(p);
        }
    }

    /// Remember fresh digs, creak, and bring down what isn't held up.
    pub fn caveins_tick(&mut self, dt: f32) {
        if self.is_client() {
            return;
        }
        // Remember them all first, then look round each for trouble.
        let digs: Vec<IVec3> = std::mem::take(&mut self.dig_queue).into_iter().filter(|&p| self.world.get_v(p) == AIR && underground(&self.world, p)).collect();
        for &p in &digs {
            if self.world.dug.len() < DUG_CAP {
                self.world.dug.insert(p);
            }
        }
        let mut looked: Vec<IVec3> = Vec::new();
        for p in digs {
            // (One look covers a whole room.)
            if looked.iter().any(|q| (*q - p).abs().max_element() <= ROOM / 2) {
                continue;
            }
            looked.push(p);
            let cells = unsupported(&self.world, p);
            if cells.is_empty() {
                continue;
            }
            // Already creaking nearby: the trouble just got bigger.
            if let Some(c) = self.cave_ins.iter_mut().find(|c| c.cells.iter().any(|q| (*q - p).abs().max_element() <= ROOM)) {
                for q in cells {
                    if !c.cells.contains(&q) {
                        c.cells.push(q);
                    }
                }
                continue;
            }
            let at = cells[0].as_vec3();
            self.sfx(Sfx::Thud, Some(at));
            self.tell_near(at, "The ceiling creaks... (prop it up with a pillar or a beam, or get out)");
            self.cave_ins.push(CaveIn { cells, t: 0.0, dust: 0.0 });
        }
        self.settle_check();
        let mut i = 0;
        while i < self.cave_ins.len() {
            let c = &mut self.cave_ins[i];
            c.t += dt;
            c.dust -= dt;
            let dusty = c.dust <= 0.0;
            if dusty {
                c.dust = 0.4;
            }
            if c.t < WARN_SECS {
                if dusty {
                    let k = self.rng.int(0, self.cave_ins[i].cells.len() as i32 - 1) as usize;
                    let cell = self.cave_ins[i].cells[k];
                    if let Some(roof) = ceiling(&self.world, cell) {
                        self.block_particles(roof, 3);
                        if self.rng.chance(0.3) {
                            self.sfx(Sfx::Thud, Some(roof.as_vec3()));
                        }
                    }
                }
                i += 1;
                continue;
            }
            let c = self.cave_ins.swap_remove(i);
            self.collapse(c);
        }
    }

    /// Time's up: whatever is still unsupported comes down.
    fn collapse(&mut self, c: CaveIn) {
        let still: Vec<IVec3> = c.cells.into_iter().filter(|&q| self.world.get_v(q) == AIR && span(&self.world, q).is_some_and(|s| s > MAX_SPAN)).collect();
        let Some(&first) = still.first() else { return };
        let at = first.as_vec3() + Vec3::splat(0.5);
        for &q in &still {
            self.world.dug.remove(&q);
            let Some(roof) = ceiling(&self.world, q) else { continue };
            // The ceiling block, and the one over it if that's loose now too.
            for k in 0..2 {
                let r = roof + IVec3::Y * k;
                let id = self.world.get_v(r);
                if !is_solid(id) || block(id).hardness < 0.0 || crate::fortress::is_cage(id) {
                    break;
                }
                self.world.set_v(r, AIR);
                self.falling.push(crate::falling::FallingBlock { pos: r.as_vec3(), vel: 0.0, id: rubble(id), from: r.y as f32 });
            }
        }
        self.sfx(Sfx::Explode, Some(at));
        // Anyone under it.
        let under = |p: Vec3| still.iter().any(|q| (p.x - (q.x as f32 + 0.5)).abs() < 1.0 && (p.z - (q.z as f32 + 0.5)).abs() < 1.0 && (p.y - q.y as f32).abs() < 3.0);
        let cause = "was buried in a cave-in. Should have propped it up";
        if !self.dedicated && self.dead.is_none() && under(self.player.body.pos) {
            self.player.hurt = 0.0;
            self.hurt_player(CRUSH, cause);
            self.advance("sinking_feeling");
        }
        let hit: Vec<u32> = self.peers.iter().filter(|(_, p)| p.alive() && under(p.target)).map(|(&id, _)| id).collect();
        for id in hit {
            self.hurt_peer(id, CRUSH, cause, Vec3::ZERO);
        }
        self.tell_near(at, "The ceiling came down!");
    }

    /// A message to everyone within earshot of `at` (the local player, and joined players).
    fn tell_near(&mut self, at: Vec3, text: &str) {
        if !self.dedicated && self.player.body.pos.distance(at) < 24.0 {
            self.msg(text.to_string());
        }
        let near: Vec<u32> = self.peers.iter().filter(|(_, p)| p.target.distance(at) < 24.0).map(|(&id, _)| id).collect();
        for id in near {
            self.system_message(Some(id), text);
        }
    }

    /// Creaking ceilings that have been propped up (every cell filled in or
    /// held up again) settle, and stop creaking.
    pub fn settle_check(&mut self) {
        let world = &self.world;
        let held = |c: &CaveIn| c.cells.iter().all(|&q| world.get_v(q) != AIR || span(world, q).is_none_or(|s| s <= MAX_SPAN));
        let settled: Vec<Vec3> = self.cave_ins.iter().filter(|c| held(c)).map(|c| c.cells[0].as_vec3()).collect();
        if settled.is_empty() {
            return;
        }
        self.cave_ins.retain(|c| !held(c));
        for at in settled {
            self.tell_near(at, "The ceiling settles. That'll hold.");
            if !self.dedicated && self.player.body.pos.distance(at) < 24.0 {
                self.advance("propped_up");
            }
        }
    }

    /// The Support Gauge in hand: what it reads here (refreshed now and then).
    pub fn gauge_tick(&mut self, dt: f32) {
        if self.inv.held() != SUPPORT_GAUGE {
            self.gauge_reading = None;
            return;
        }
        self.gauge_acc -= dt;
        if self.gauge_acc > 0.0 && self.gauge_reading.is_some() {
            return;
        }
        self.gauge_acc = 0.5;
        let feet = self.player.body.pos.floor().as_ivec3();
        self.gauge_reading = Some(gauge(&self.world, feet).map_or(-1, |p| p));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use macroquad::math::ivec3;

    /// Solid rock from y 40 to 60 over the arena (so it's underground), and
    /// a dug room `w` wide and 3 tall at y 45, its cells remembered.
    fn room(g: &mut Game, w: i32) -> Vec<IVec3> {
        for y in 40..=60 {
            for z in -12..12 {
                for x in -12..12 {
                    g.world.set_v(ivec3(x, y, z), STONE);
                }
            }
        }
        let mut cells = Vec::new();
        for y in 45..48 {
            for z in 0..w {
                for x in 0..w {
                    let p = ivec3(x - w / 2, y, z - w / 2);
                    g.world.set_v(p, AIR);
                    cells.push(p);
                }
            }
        }
        cells
    }

    #[test]
    fn a_small_room_holds_and_a_big_one_comes_down() {
        let mut g = crate::game::tests::arena(501);
        let cells = room(&mut g, 7);
        for &p in &cells {
            g.dug_out(p);
        }
        g.caveins_tick(0.05);
        assert!(g.cave_ins.is_empty(), "seven across holds");
        assert!(gauge(&g.world, ivec3(0, 45, 0)).unwrap() >= 40);

        let mut g = crate::game::tests::arena(502);
        let cells = room(&mut g, 15);
        for &p in &cells {
            g.dug_out(p);
        }
        g.caveins_tick(0.05);
        assert_eq!(g.cave_ins.len(), 1, "fifteen across creaks");
        assert_eq!(gauge(&g.world, ivec3(0, 45, 0)), Some(0));
        let roof = ivec3(0, 48, 0);
        assert_eq!(g.world.get_v(roof), STONE);
        g.player.body.pos = Vec3::new(0.5, 45.0, 0.5);
        let hp = g.player.health;
        for _ in 0..((WARN_SECS + 0.2) / 0.05) as usize {
            g.caveins_tick(0.05);
        }
        assert_eq!(g.world.get_v(roof), AIR, "the middle came down");
        assert!(g.falling.iter().any(|f| f.id == COBBLE), "as rubble");
        assert!(g.player.health < hp, "on whoever was under it");
        assert!(g.cave_ins.is_empty());
    }

    #[test]
    fn pillars_and_beams_hold_it_up() {
        let mut g = crate::game::tests::arena(503);
        let cells = room(&mut g, 15);
        for &p in &cells {
            g.dug_out(p);
        }
        g.caveins_tick(0.05);
        assert_eq!(g.cave_ins.len(), 1);
        // Pillars every few blocks, and a beam across the middle.
        for &(x, z) in &[(-4, -4), (4, -4), (-4, 4), (4, 4), (0, -4), (0, 4), (-4, 0), (4, 0)] {
            for y in 45..48 {
                g.world.set_v(ivec3(x, y, z), COBBLE);
            }
        }
        for x in -7..=7 {
            g.world.set_v(ivec3(x, 48, 0), LOG);
        }
        g.settle_check();
        assert!(g.cave_ins.is_empty(), "it settled");
        assert_eq!(g.world.get_v(ivec3(2, 48, 2)), STONE);
        assert!(gauge(&g.world, ivec3(2, 45, 2)).unwrap() >= 40);
    }

    #[test]
    fn natural_caves_never_fall_and_the_surface_isnt_underground() {
        let mut g = crate::game::tests::arena(504);
        room(&mut g, 15);
        // Nobody dug it: nothing to worry about.
        g.dug_out(ivec3(20, 45, 20));
        g.caveins_tick(0.05);
        assert!(g.cave_ins.is_empty());
        assert!(!underground(&crate::game::tests::arena(505).world, ivec3(0, 50, 0)));
    }
}
