//! Raids, and the illagers who make them.
//!
//! - **Pilferers** (crossbows) camp around **Pilferer Outposts**, dark wooden
//!   towers with a chest at the top, and wander the countryside in
//!   **patrols** of a few, led by a **captain** with an Ominous Banner on its
//!   back.
//! - Defeat a captain and you're left with **Bad Omen** for twenty minutes.
//!   Walk into a village while it lasts and a **raid** begins: the village
//!   is warned, the Hmmers hurry home, and waves of raiders march on the
//!   square from outside the village. Easy has three waves, Normal five and
//!   Hard seven, each bigger than the last: Pilferers first, then
//!   **Hacklers** with axes, then **Invoicers** (who summon little flying
//!   **Fees** that go through walls, and send **Late Fees** snapping up out of
//!   the ground at you) and **Rampagers**, who are large.
//! - A bar at the top of the screen shows the wave and how many raiders are
//!   left. See off every wave and the raid is won: everyone there becomes a
//!   **Hero of the Village** (Hmmers give a third off) for twenty minutes.
//!   If every Hmmer in the village is lost, or nobody stays to defend it,
//!   the raid is lost.
//! - Ring a village's **Bell (Ding Dong)** to make every raider nearby glow
//!   for a few seconds and send the Hmmers indoors.
//! - Invoicers carry a **Totem of Not Dying (Once)**. Keep it in your hotbar
//!   and the next blow that would finish you doesn't: you're back up with a
//!   couple of hearts, Regeneration and Fire Resistance, and the totem's gone.
//!
//! Raids run where the world lives; joined players are sent the raid bar
//! (`Msg::Raid`) and their Bad Omen and Hero (`Msg::TimedEffect`).

use crate::block::*;
use crate::entity::{Mob, MobKind};
use crate::game::Game;
use crate::net::Msg;
use crate::potions::Potion;
use crate::sound::Sfx;
use crate::structures::{Kind, Site};
use macroquad::math::{ivec3, IVec3, Vec3};

/// How long Bad Omen and Hero of the Village last.
pub const OMEN_SECS: f32 = 1200.0;
pub const HERO_SECS: f32 = 1200.0;
/// Seconds between waves.
pub const WAVE_GAP: f32 = 12.0;
/// How close to a village's square counts as being in the village.
pub const VILLAGE_RANGE: f32 = 48.0;
/// Seconds with nobody defending before the raid is lost.
const LOST_AFTER: f32 = 240.0;
/// A Hero's prices: this much off.
pub const HERO_DISCOUNT: f32 = 0.33;
/// How long the Bell makes raiders glow.
pub const GLOW_SECS: f32 = 4.0;

#[derive(Clone, Debug)]
pub struct Raid {
    pub centre: Vec3,
    pub wave: u8,
    pub waves: u8,
    /// Seconds until the next wave marches (while the last one's gone).
    pub between: f32,
    /// Seconds nobody has been around to defend it.
    pub idle: f32,
    /// This wave's raiders (mob ids), and how many there were.
    pub raiders: Vec<u32>,
    pub size: u16,
    /// Hmmers at the start (none left: lost).
    pub hmmers: usize,
}

/// Who marches in wave `wave` (1-based) of `waves`, with `players` defending.
pub fn wave_mix(wave: u8, waves: u8, players: usize) -> Vec<MobKind> {
    let mut v = Vec::new();
    let w = wave as usize;
    let extra = players.saturating_sub(1);
    for _ in 0..(2 + w / 2 + extra) {
        v.push(MobKind::Pilferer);
    }
    for _ in 0..(w.saturating_sub(1) / 2 * (1 + extra / 2) + (w >= 2) as usize) {
        v.push(MobKind::Hackler);
    }
    if w >= 3 {
        v.push(MobKind::Invoicer);
    }
    if w >= 5 {
        v.push(MobKind::Invoicer);
    }
    if w >= 3 && (w % 2 == 1 || wave == waves) {
        v.push(MobKind::Rampager);
    }
    v
}

/// A trade at a Hero's prices: a third off what you give (never below one).
pub fn hero_price(mut t: crate::villagers::Trade) -> crate::villagers::Trade {
    for g in t.give.iter_mut() {
        if g.0 != AIR && g.1 > 1 {
            g.1 = ((g.1 as f32 * (1.0 - HERO_DISCOUNT)).round() as u8).max(1);
        }
    }
    t
}

/// A Pilferer outpost: a dark wooden tower, a lookout, and a chest at the top.
pub fn outpost_blocks(site: &Site) -> Vec<(IVec3, Id)> {
    let mut out = Vec::new();
    let o = site.origin;
    let mut put = |x: i32, y: i32, z: i32, id: Id| out.push((o + ivec3(x, y, z), id));
    for x in -3..=3i32 {
        for z in -3..=3i32 {
            // Footings into the ground, and room cleared all the way up.
            for y in -3..=0 {
                put(x, y, z, COBBLE);
            }
            for y in 1..=16 {
                let corner = x.abs() == 3 && z.abs() == 3;
                let floor = y == 6 || y == 11;
                let edge = x.abs() == 3 || z.abs() == 3;
                let id = if corner && y <= 15 {
                    SPRUCE_LOG
                } else if (floor && !(x == 2 && z == 2)) || (y == 12 && edge && (x + z) % 2 == 0) || y == 16 {
                    PLANKS
                } else {
                    AIR
                };
                put(x, y, z, id);
            }
        }
    }
    // A ladder up the inside, through the gaps in the floors.
    for y in 1..=11 {
        put(2, y, 2, LADDER_FIRST + 2);
    }
    put(2, 0, 3, PLANKS);
    put(0, 12, 0, CHEST);
    out
}

impl Game {
    /// The village square near `at`, if `at` is in a village.
    pub fn village_at(&self, at: Vec3) -> Option<Vec3> {
        if crate::scorch::in_scorch(at.x) || crate::hollow::in_hollow(at.x) {
            return None;
        }
        let o = self.world.generator.nearest_site(Kind::Village, at, 5)?;
        let centre = o.as_vec3() + Vec3::new(0.5, 1.0, 0.5);
        (Vec3::new(centre.x - at.x, 0.0, centre.z - at.z).length() < VILLAGE_RANGE).then_some(centre)
    }

    /// `who` (a player id; ours for the local player) gets Bad Omen.
    pub fn give_effect_to(&mut self, who: u32, effect: Potion, secs: f32) {
        if who == self.my_id && !self.dedicated {
            self.timed_effect(effect, secs);
        } else if self.peers.contains_key(&who) {
            self.send_timed_effect(who, effect, secs, 0);
        }
        match effect {
            Potion::BadOmen => {
                self.omens.insert(who, secs);
            }
            Potion::Hero => {
                self.heroes.insert(who, secs);
            }
            _ => {}
        }
    }

    /// A raider was defeated: a patrol captain leaves its killer with Bad Omen.
    pub fn raider_died(&mut self, m: &Mob) {
        if m.kind == MobKind::Pilferer && m.seed == 1 && self.raid.as_ref().is_none_or(|r| !r.raiders.contains(&m.id)) {
            let who = m.last_attacker;
            if who == self.my_id || self.peers.contains_key(&who) {
                self.give_effect_to(who, Potion::BadOmen, OMEN_SECS);
                let name = if who == self.my_id { self.player_name.clone() } else { self.peer_name(who) };
                self.tell(&name, "You feel a Bad Omen. Villages might not be safe with you around.");
                self.advance_for(&name, "bad_omen");
            }
        }
    }

    /// Where the world lives: omens, patrols, the raid itself.
    pub fn raids_tick(&mut self, dt: f32) {
        if let Some(h) = self.raid_hud.as_mut() {
            h.4 -= dt;
        }
        self.raid_hud = self.raid_hud.filter(|h| h.4 > 0.0);
        self.bell_glow = (self.bell_glow - dt).max(0.0);
        if self.is_client() {
            return;
        }
        for t in self.omens.values_mut().chain(self.heroes.values_mut()) {
            *t -= dt;
        }
        for (seconds, _) in self.strong.values_mut() {
            *seconds -= dt;
        }
        self.omens.retain(|_, t| *t > 0.0);
        self.heroes.retain(|_, t| *t > 0.0);
        self.strong.retain(|_, (seconds, _)| *seconds > 0.0);
        self.raid_clock -= dt;
        if self.raid_clock > 0.0 {
            return;
        }
        // Once a second is plenty for the rest.
        let step = 1.0;
        self.raid_clock = step;
        self.patrols(step);
        self.outpost_guards();
        // Bad Omen in a village: here they come.
        if self.raid.is_none() && self.rules.difficulty.monsters() {
            let players = self.player_spots_by_id();
            for (id, at) in players {
                if !self.omens.contains_key(&id) {
                    continue;
                }
                if let Some(centre) = self.village_at(at) {
                    self.omens.remove(&id);
                    if id == self.my_id {
                        self.effects.retain(|effect| effect.kind != Potion::BadOmen);
                    } else {
                        self.net_send_to(id, Msg::TimedEffect { effect: Potion::BadOmen.effect_index(), secs: 0.0, amplifier: 0 });
                    }
                    self.start_raid(centre);
                    break;
                }
            }
        }
        if self.raid.is_some() {
            self.raid_step(step);
        }
    }

    pub fn start_raid(&mut self, centre: Vec3) {
        let waves = match self.rules.difficulty {
            crate::rules::Difficulty::Peaceful => return,
            crate::rules::Difficulty::Easy => 3,
            crate::rules::Difficulty::Normal => 5,
            crate::rules::Difficulty::Hard => 7,
        };
        let hmmers = self.mobs.iter().filter(|m| m.kind == MobKind::Hmmer && m.body.pos.distance(centre) < VILLAGE_RANGE).count();
        self.raid = Some(Raid { centre, wave: 0, waves, between: 6.0, idle: 0.0, raiders: Vec::new(), size: 0, hmmers });
        self.system_message(None, "A raid has begun! Defend the village.");
        self.sfx(Sfx::Thunder, Some(centre));
        self.ring_bell_at(centre);
    }

    fn raid_step(&mut self, dt: f32) {
        let Some(mut r) = self.raid.take() else { return };
        // Raiders still standing.
        r.raiders.retain(|id| self.mobs.iter().any(|m| m.id == *id && m.health > 0.0));
        // Anyone defending?
        let defenders = self.player_spots().iter().filter(|(_, at, _)| at.distance(r.centre) < VILLAGE_RANGE * 1.5).count();
        r.idle = if defenders == 0 { r.idle + dt } else { 0.0 };
        let hmmers_left = self.mobs.iter().filter(|m| m.kind == MobKind::Hmmer && m.body.pos.distance(r.centre) < VILLAGE_RANGE * 1.5).count();
        if r.idle > LOST_AFTER || (r.hmmers > 0 && hmmers_left == 0) {
            self.end_raid(r, false);
            return;
        }
        // Hmmers stay indoors while it's on.
        for m in self.mobs.iter_mut().filter(|m| m.kind == MobKind::Hmmer && m.body.pos.distance(r.centre) < VILLAGE_RANGE * 1.5) {
            if let Some(h) = m.home {
                m.goal = Some(h);
            }
        }
        // Raiders go for Hmmers in their way.
        let mut hits: Vec<(usize, f32, Vec3)> = Vec::new();
        for (i, h) in self.mobs.iter().enumerate().filter(|(_, m)| m.kind == MobKind::Hmmer) {
            if let Some(raider) = self.mobs.iter().find(|m| m.kind.raider() && m.kind != MobKind::Pilferer && m.body.pos.distance(h.body.pos) < 1.8 && m.attack_cd <= 0.0) {
                hits.push((i, if raider.kind == MobKind::Rampager { 10.0 } else { 6.0 }, raider.body.pos));
            }
        }
        for (i, dmg, from) in hits {
            self.mobs[i].damage(dmg, from);
        }
        if r.raiders.is_empty() {
            if r.wave >= r.waves {
                self.end_raid(r, true);
                return;
            }
            r.between -= dt;
            if r.between <= 0.0 {
                r.wave += 1;
                r.between = WAVE_GAP;
                let mix = wave_mix(r.wave, r.waves, self.peers.len() + (!self.dedicated) as usize);
                let a = self.rng.range(0.0, std::f32::consts::TAU);
                let edge = r.centre + Vec3::new(a.cos(), 0.0, a.sin()) * (VILLAGE_RANGE * 0.7);
                for (k, kind) in mix.into_iter().enumerate() {
                    let off = Vec3::new(self.rng.range(-4.0, 4.0), 0.0, self.rng.range(-4.0, 4.0));
                    let (x, z) = ((edge.x + off.x).floor() as i32, (edge.z + off.z).floor() as i32);
                    if !self.world.is_loaded(x, z) {
                        continue;
                    }
                    let y = self.world.surface_y(x, z) + 1;
                    let id = self.alloc_mob(kind, Vec3::new(x as f32 + 0.5, y as f32, z as f32 + 0.5));
                    if let Some(m) = self.mobs.iter_mut().find(|m| m.id == id) {
                        m.home = Some(r.centre);
                        m.persistent = true;
                        // The wave's first Pilferer carries the banner.
                        m.seed = (k == 0 && kind == MobKind::Pilferer) as u32;
                        r.raiders.push(id);
                    }
                }
                r.size = r.raiders.len() as u16;
                self.system_message(None, &format!("Wave {} of {}!", r.wave, r.waves));
                self.sfx(Sfx::Thunder, Some(edge));
            }
        }
        let state = Msg::Raid { state: 1, wave: r.wave.max(1), waves: r.waves, left: r.raiders.len() as u16 };
        self.raid_hud = Some((1, r.wave.max(1), r.waves, r.raiders.len() as u16, 2.0));
        self.send_raid_bar(&r, state);
        self.raid = Some(r);
    }

    fn send_raid_bar(&mut self, r: &Raid, state: Msg) {
        let near: Vec<u32> = self.peers.iter().filter(|(_, p)| p.target.distance(r.centre) < VILLAGE_RANGE * 2.0).map(|(&id, _)| id).collect();
        for id in near {
            self.net_send_to(id, state.clone());
        }
    }

    fn end_raid(&mut self, r: Raid, won: bool) {
        let state = Msg::Raid { state: if won { 2 } else { 3 }, wave: r.wave, waves: r.waves, left: 0 };
        self.send_raid_bar(&r, state);
        self.raid_hud = Some((if won { 2 } else { 3 }, r.wave, r.waves, 0, 6.0));
        if won {
            self.system_message(None, "Victory! The village is saved.");
            self.sfx(Sfx::Fanfare, Some(r.centre));
            for k in 0..12 {
                let a = k as f32 * 0.52;
                self.smoke(r.centre + Vec3::new(a.cos() * 6.0, 8.0 + (k % 3) as f32 * 2.0, a.sin() * 6.0), 10, 0.8);
            }
            for (id, at) in self.player_spots_by_id() {
                if at.distance(r.centre) < VILLAGE_RANGE * 1.5 {
                    self.give_effect_to(id, Potion::Hero, HERO_SECS);
                    let name = if id == self.my_id { self.player_name.clone() } else { self.peer_name(id) };
                    self.advance_for(&name, "hero_village");
                }
            }
        } else {
            self.system_message(None, "The raid was lost. The village will remember this.");
            // The raiders go home.
            for m in self.mobs.iter_mut().filter(|m| r.raiders.contains(&m.id)) {
                m.persistent = false;
                m.home = None;
            }
        }
    }

    /// Now and then, by day, a patrol wanders out near someone (not in the other worlds).
    fn patrols(&mut self, dt: f32) {
        self.patrol_timer -= dt;
        if self.patrol_timer > 0.0 {
            return;
        }
        self.patrol_timer = self.rng.range(600.0, 1200.0);
        if self.is_night() || !self.rules.difficulty.monsters() || self.mobs.iter().filter(|m| m.kind == MobKind::Pilferer).count() >= 8 {
            return;
        }
        let spots = self.player_spots();
        if spots.is_empty() {
            return;
        }
        let at = spots[self.rng.int(0, spots.len() as i32 - 1) as usize].1;
        if crate::scorch::in_scorch(at.x) || crate::hollow::in_hollow(at.x) || self.village_at(at).is_some() {
            return;
        }
        let a = self.rng.range(0.0, std::f32::consts::TAU);
        let d = self.rng.range(24.0, 40.0);
        let (x, z) = ((at.x + a.cos() * d).floor() as i32, (at.z + a.sin() * d).floor() as i32);
        if !self.world.is_loaded(x, z) {
            return;
        }
        let y = self.world.surface_y(x, z) + 1;
        if is_liquid(self.world.get(x, y - 1, z)) {
            return;
        }
        for k in 0..self.rng.int(2, 5) {
            let id = self.alloc_mob(MobKind::Pilferer, Vec3::new(x as f32 + 0.5 + k as f32 * 0.8, y as f32, z as f32 + 0.5));
            if k == 0
                && let Some(m) = self.mobs.iter_mut().find(|m| m.id == id)
            {
                // The captain.
                m.seed = 1;
            }
        }
    }

    /// Outposts keep a few Pilferers about while someone's near.
    fn outpost_guards(&mut self) {
        if !self.rules.difficulty.monsters() || self.rng.f32() > 0.15 {
            return;
        }
        for (_, at, _) in self.player_spots() {
            if crate::scorch::in_scorch(at.x) || crate::hollow::in_hollow(at.x) {
                continue;
            }
            let Some(o) = self.world.generator.nearest_site(Kind::Outpost, at, 3) else { continue };
            let base = o.as_vec3() + Vec3::new(0.5, 1.0, 0.5);
            if base.distance(at) > 48.0 || !self.world.is_loaded(o.x, o.z) {
                continue;
            }
            let guards = self.mobs.iter().filter(|m| m.kind == MobKind::Pilferer && m.body.pos.distance(base) < 24.0).count();
            if guards < 4 {
                let a = self.rng.range(0.0, std::f32::consts::TAU);
                let (x, z) = ((base.x + a.cos() * 6.0).floor() as i32, (base.z + a.sin() * 6.0).floor() as i32);
                if self.world.is_loaded(x, z) {
                    let y = self.world.surface_y(x, z) + 1;
                    self.alloc_mob(MobKind::Pilferer, Vec3::new(x as f32 + 0.5, y as f32, z as f32 + 0.5));
                }
            }
        }
    }

    /// Right-click a Bell: ding dong. Raiders nearby glow, Hmmers go home.
    pub fn ring_bell(&mut self, pos: IVec3) {
        self.player.swing = 1.0;
        let centre = pos.as_vec3() + Vec3::splat(0.5);
        self.bell_glow = GLOW_SECS;
        if self.is_client() {
            self.sfx(Sfx::Chime, Some(centre));
            self.net_send_msg(Msg::Interact { x: pos.x, y: pos.y, z: pos.z, item: AIR });
            return;
        }
        self.ring_bell_at(centre);
    }

    /// Where the world lives: the bell's sound, and its effect on the village.
    pub fn ring_bell_at(&mut self, centre: Vec3) {
        self.sfx(Sfx::Chime, Some(centre));
        for m in self.mobs.iter_mut().filter(|m| m.kind == MobKind::Hmmer && m.body.pos.distance(centre) < 32.0) {
            if let Some(h) = m.home {
                m.goal = Some(h);
            }
        }
    }

    /// An Invoicer calls up three Fees (no more than eight about).
    pub fn summon_fees(&mut self, at: Vec3) {
        let near = self.mobs.iter().filter(|m| m.kind == MobKind::Fee && m.body.pos.distance(at) < 24.0).count();
        if near >= 8 {
            return;
        }
        for k in 0..3 {
            let a = k as f32 * 2.1;
            self.alloc_mob(MobKind::Fee, at + Vec3::new(a.cos(), 0.5, a.sin()));
        }
        self.smoke(at, 12, 0.6);
        self.sfx(Sfx::Warp, Some(at));
    }

    /// Late Fees: a line of snapping jaws up out of the ground toward `to`.
    pub fn late_fees(&mut self, from: Vec3, to: Vec3) {
        let flat = Vec3::new(to.x - from.x, 0.0, to.z - from.z);
        let dir = flat.normalize_or_zero();
        let steps = (flat.length().ceil() as i32 + 2).min(14);
        let me = !self.dedicated && self.dead.is_none() && !self.creative;
        let mut hit_me = false;
        let mut hit_peers: Vec<u32> = Vec::new();
        for k in 1..=steps {
            let p = from + dir * k as f32;
            let (x, z) = (p.x.floor() as i32, p.z.floor() as i32);
            // The ground at that spot (within a few blocks of the caster's feet).
            let mut y = from.y.floor() as i32;
            for dy in [0, 1, -1, 2, -2, 3, -3] {
                if is_solid(self.world.get(x, y + dy - 1, z)) && !is_solid(self.world.get(x, y + dy, z)) {
                    y += dy;
                    break;
                }
            }
            let jaw = Vec3::new(x as f32 + 0.5, y as f32, z as f32 + 0.5);
            self.block_particles_tile(IVec3::new(x, y, z), crate::texture::T_BONE, 3);
            if me && Vec3::new(self.player.body.pos.x - jaw.x, 0.0, self.player.body.pos.z - jaw.z).length() < 0.9 && (self.player.body.pos.y - jaw.y).abs() < 1.5 {
                hit_me = true;
            }
            for (&id, q) in self.peers.iter() {
                if q.alive() && Vec3::new(q.target.x - jaw.x, 0.0, q.target.z - jaw.z).length() < 0.9 && (q.target.y - jaw.y).abs() < 1.5 && !hit_peers.contains(&id) {
                    hit_peers.push(id);
                }
            }
            for m in self.mobs.iter_mut().filter(|m| !m.kind.raider() && m.kind != MobKind::Fee) {
                if Vec3::new(m.body.pos.x - jaw.x, 0.0, m.body.pos.z - jaw.z).length() < 0.9 && (m.body.pos.y - jaw.y).abs() < 1.5 {
                    m.damage(6.0, jaw);
                }
            }
        }
        self.sfx(Sfx::Snip, Some(from + dir * 3.0));
        if hit_me {
            self.hurt_player_from(6.0, "was charged Late Fees", Some(from), false);
        }
        for id in hit_peers {
            self.hurt_peer(id, 6.0, "was charged Late Fees", Vec3::Y * 2.0);
        }
    }

    /// About to die with a Totem in the hotbar? Not today. True if it saved you.
    pub fn use_totem(&mut self) -> bool {
        let Some(slot) = (0..9).find(|&i| self.inv.slots[i].is_some_and(|s| s.0 == TOTEM)) else { return false };
        self.inv.slots[slot] = None;
        if self.is_client() {
            self.net_send_msg(Msg::Consume { item: TOTEM, n: 1 });
        }
        self.player.health = 4.0;
        self.timed_effect(Potion::Regeneration, 45.0);
        self.timed_effect(Potion::FireResistance, 40.0);
        let at = self.player.body.pos + Vec3::Y;
        for _ in 0..3 {
            self.smoke(at, 10, 0.6);
        }
        self.sfx(Sfx::Fanfare, None);
        self.msg("The Totem of Not Dying crumbles. Not today!");
        self.advance("totem_saved");
        true
    }

    /// Players where the world lives, by id (ours too), and where they are.
    pub fn player_spots_by_id(&self) -> Vec<(u32, Vec3)> {
        let mut v: Vec<(u32, Vec3)> = self.peers.iter().filter(|(_, p)| p.alive()).map(|(&id, p)| (id, p.target)).collect();
        if !self.dedicated && self.dead.is_none() {
            v.push((self.my_id, self.player.body.pos));
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waves_grow_and_bring_the_big_ones_later() {
        let first = wave_mix(1, 5, 1);
        assert!(first.iter().all(|k| *k == MobKind::Pilferer), "the first wave is just Pilferers");
        let third = wave_mix(3, 5, 1);
        assert!(third.contains(&MobKind::Invoicer) && third.contains(&MobKind::Rampager));
        assert!(wave_mix(5, 5, 1).len() > first.len());
        assert!(wave_mix(2, 5, 4).len() > wave_mix(2, 5, 1).len(), "more defenders, more raiders");
    }

    #[test]
    fn heroes_pay_less() {
        let t = crate::villagers::Trade { give: [(GOLD_INGOT, 9), (BOOK, 1)], get: (DIAMOND, 1), wear: 0 };
        let h = hero_price(t);
        assert_eq!(h.give[0], (GOLD_INGOT, 6));
        assert_eq!(h.give[1], (BOOK, 1));
    }

    #[test]
    fn a_captain_leaves_bad_omen_and_a_totem_saves_you() {
        let mut g = crate::game::tests::arena(97);
        let mut rng = crate::noise::Rng::new(4);
        let mut cap = Mob::new(MobKind::Pilferer, Vec3::new(3.0, 50.0, 3.0), &mut rng);
        cap.seed = 1;
        cap.last_attacker = g.my_id;
        g.raider_died(&cap);
        assert!(g.has_effect(Potion::BadOmen));
        assert!(g.omens.contains_key(&g.my_id));

        g.creative = false;
        g.inv.slots[3] = Some((TOTEM, 1));
        g.player.hurt = 0.0;
        g.hurt_player(100.0, "fell over in a test");
        assert!(g.dead.is_none(), "the totem saved them");
        assert!(g.player.health > 0.0);
        assert_eq!(g.inv.count(TOTEM), 0);
        assert!(g.has_effect(Potion::Regeneration));
    }

    #[test]
    fn a_raid_comes_in_waves_and_heroes_win_it() {
        let mut g = crate::game::tests::arena(98);
        g.rules.difficulty = crate::rules::Difficulty::Easy;
        let centre = g.player.body.pos;
        g.start_raid(centre);
        let mut waves_seen = 0;
        for _ in 0..400 {
            g.raids_tick(1.0);
            let Some(r) = g.raid.as_ref() else { break };
            if !r.raiders.is_empty() {
                waves_seen = waves_seen.max(r.wave);
                assert!(g.mobs.iter().filter(|m| r.raiders.contains(&m.id)).all(|m| m.home.is_some()), "raiders head for the square");
                // The defenders do their job.
                let ids = r.raiders.clone();
                g.mobs.retain(|m| !ids.contains(&m.id));
            }
        }
        assert!(g.raid.is_none(), "the raid ended");
        assert_eq!(waves_seen, 3, "Easy has three waves");
        assert!(g.has_effect(Potion::Hero));
        assert_eq!(g.raid_hud.map(|h| h.0), Some(2), "victory on the bar");
    }

    #[test]
    fn outposts_have_a_lookout_with_a_chest() {
        let site = Site { kind: Kind::Outpost, origin: ivec3(10, 60, 10), facing: 0, seed: 3 };
        let b = outpost_blocks(&site);
        assert!(b.iter().any(|x| x.1 == CHEST && x.0.y == 72));
        assert!(b.iter().filter(|x| crate::carpentry::is_ladder(x.1)).count() >= 10);
    }
}
