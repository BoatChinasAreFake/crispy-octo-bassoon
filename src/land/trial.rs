//! Trial Chambers: copper-and-tuff halls deep underground.
//!
//! - **Trial Spawners** wake when someone comes within 14 blocks and send
//!   out a wave of monsters (a couple at a time; more for more players).
//!   Beat the whole wave and it pays out a **Trial Key** and a little loot,
//!   then goes dark for twenty minutes.
//! - **Vaults** open for a Trial Key: they spit out their treasure (Armour
//!   Trim templates among it) and stay open.
//! - **Breezes** bounce about keeping their distance and throw **Wind
//!   Charges**: a burst of wind that knocks everyone back without breaking
//!   anything. Their **Breeze Rods** craft into Wind Charges of your own;
//!   throw one at your feet to launch yourself (wind-jumping).
//!
//! Spawners, vaults and wind all run where the world lives; joined players
//! see the wind charges with the arrows and ask the host to open vaults.

use crate::block::*;
use crate::entity::{Arrow, MobKind};
use crate::game::Game;
use crate::net::Msg;
use crate::sound::Sfx;
use crate::world::{CH, CW};
use crate::noise::{hash2, hash3};
use macroquad::math::{IVec3, Vec3, ivec3};

/// How near someone must be to wake a Trial Spawner.
pub const WAKE_RANGE: f32 = 14.0;
/// Most of a spawner's monsters out at once.
const AT_ONCE: usize = 2;
/// Seconds a spent spawner rests before it can be challenged again.
pub const REST_SECS: f32 = 1200.0;
/// How far a wind burst reaches.
pub const WIND_RADIUS: f32 = 2.6;
/// How fast thrown wind charges fly.
pub const WIND_SPEED: f32 = 18.0;

/// A Trial Spawner's fight in progress (or its rest afterwards).
#[derive(Clone, Debug, Default)]
pub struct Trial {
    /// Monsters still to come in this wave.
    pub to_come: u8,
    /// The ones out now (mob ids).
    pub out: Vec<u32>,
    /// Seconds until the next one comes out.
    pub next: f32,
    /// Resting after a win: seconds until it wakes again.
    pub rest: f32,
    /// Woken by someone with Bad Omen: tougher, with an Ominous Trial Key at the end.
    pub ominous: bool,
}

pub fn is_trial_spawner(id: Id) -> bool {
    matches!(id, TRIAL_SPAWNER | TRIAL_SPAWNER_SPENT | TRIAL_SPAWNER_OMINOUS)
}

/// Bad Omen turns a Trial Spawner ominous when it wakes; Ominous Vaults
/// within this distance of it turn too.
pub const OMINOUS_REACH: i32 = 24;
/// Ominous monsters are tougher by this much.
pub const OMINOUS_TOUGHNESS: f32 = 1.5;

/// What tumbles out of an Ominous Vault: better, and sometimes a Heavy Core (for a Mace).
pub fn ominous_loot(rng: &mut crate::noise::Rng) -> Vec<(Id, u8)> {
    let mut v = vec![if rng.chance(0.35) { (HEAVY_CORE, 1) } else { (DIAMOND, rng.int(2, 3) as u8) }];
    for _ in 0..rng.int(2, 3) {
        v.push(match rng.int(0, 5) {
            0 => (OMINOUS_BOTTLE, 1),
            1 => (GOLDEN_CHOP, 1),
            2 => (WIND_CHARGE, rng.int(4, 8) as u8),
            3 => (TRIM_FIRST + rng.int(0, TRIMS as i32 - 1) as Id, 1),
            4 => (DIAMOND, 1),
            _ => (BREEZE_ROD, rng.int(1, 2) as u8),
        });
    }
    v
}

/// What a Trial Spawner sends out: fixed per spawner.
pub fn trial_kind(p: IVec3) -> MobKind {
    let h = (p.x.wrapping_mul(73_856_093) ^ p.y.wrapping_mul(19_349_663) ^ p.z.wrapping_mul(83_492_791)).unsigned_abs();
    [MobKind::Breeze, MobKind::Groaner, MobKind::Rattler, MobKind::Webber, MobKind::Breeze, MobKind::Bloop][h as usize % 6]
}

/// A spawner's reward for beating its wave (besides the key).
fn spoils(rng: &mut crate::noise::Rng) -> (Id, u8) {
    match rng.int(0, 5) {
        0 => (BREAD, rng.int(2, 4) as u8),
        1 => (ARROW, rng.int(4, 8) as u8),
        2 => (IRON, rng.int(1, 3) as u8),
        3 => (WIND_CHARGE, rng.int(2, 4) as u8),
        4 => (COOKED_CHOP, rng.int(1, 3) as u8),
        _ => (GOLD_INGOT, rng.int(1, 2) as u8),
    }
}

/// What tumbles out of a Vault.
pub fn vault_loot(rng: &mut crate::noise::Rng) -> Vec<(Id, u8)> {
    let mut v = vec![[(DIAMOND, rng.int(1, 2) as u8), (TRIM_FIRST + rng.int(0, TRIMS as i32 - 1) as Id, 1), (GOLDEN_CHOP, 1), (CROSSBOW, 1)][rng.int(0, 3) as usize]];
    for _ in 0..rng.int(1, 3) {
        v.push(match rng.int(0, 6) {
            0 => (GOLD_INGOT, rng.int(2, 5) as u8),
            1 => (WIND_CHARGE, rng.int(3, 6) as u8),
            2 => (IRON, rng.int(2, 4) as u8),
            3 => (ARROW, rng.int(4, 10) as u8),
            4 => (TRIM_FIRST + rng.int(0, TRIMS as i32 - 1) as Id, 1),
            5 => (BREEZE_ROD, 1),
            _ => (DIAMOND, 1),
        });
    }
    v
}

/// Every block of a Trial Chamber with its hall's floor at `o`: a big
/// central hall, corridors out four ways, and a room at the end of each.
pub fn chamber_blocks(o: IVec3, seed: u32) -> Vec<(IVec3, Id)> {
    let mut out = Vec::new();
    let r = |x: i32, y: i32, z: i32| hash3(seed, x, y, z);
    let wall = |x: i32, y: i32, z: i32| -> Id {
        if y == 3 {
            CHISELED_TUFF
        } else if r(x, y, z) < 0.12 {
            COPPER_GRATE
        } else {
            TUFF_BRICKS
        }
    };
    // A hollow box: walls, floor and ceiling; air inside.
    let room = |out: &mut Vec<(IVec3, Id)>, cx: i32, cz: i32, hx: i32, hz: i32, h: i32| {
        for x in cx - hx..=cx + hx {
            for z in cz - hz..=cz + hz {
                for y in 0..=h {
                    let shell = x == cx - hx || x == cx + hx || z == cz - hz || z == cz + hz || y == 0 || y == h;
                    let id = if !shell {
                        AIR
                    } else if y == 0 {
                        // A copper checkerboard floor.
                        if (x + z).rem_euclid(2) == 0 { COPPER_FIRST } else { TUFF_BRICKS }
                    } else if y == h {
                        TUFF_BRICKS
                    } else {
                        wall(x, y, z)
                    };
                    out.push((o + ivec3(x, y, z), id));
                }
            }
        }
    };
    room(&mut out, 0, 0, 7, 7, 7);
    // Corridors, then the side rooms they lead to.
    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        for k in 7..=12 {
            for w in -2..=2i32 {
                for y in 0..=4 {
                    let (x, z) = (dx * k + dz * w, dz * k + dx * w);
                    let shell = w.abs() == 2 || y == 0 || y == 4;
                    out.push((o + ivec3(x, y, z), if !shell { AIR } else if y == 0 { TUFF_BRICKS } else { wall(x, y, z) }));
                }
            }
        }
    }
    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        let (cx, cz) = (dx * 17, dz * 17);
        room(&mut out, cx, cz, 5, 5, 5);
        // Doorway back to the corridor.
        for w in -1..=1i32 {
            for y in 1..=3 {
                out.push((o + ivec3(dx * 12 + dz * w, y, dz * 12 + dx * w), AIR));
            }
        }
        out.push((o + ivec3(cx, 1, cz), TRIAL_SPAWNER));
        out.push((o + ivec3(cx + dz * 3 + dx * 3, 1, cz + dx * 3 + dz * 3), CHEST));
        out.push((o + ivec3(cx - dz * 3 + dx * 3, 4, cz - dx * 3 + dz * 3), COPPER_BULB_ON));
    }
    // The doorways out of the hall.
    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        for w in -1..=1i32 {
            for y in 1..=3 {
                out.push((o + ivec3(dx * 7 + dz * w, y, dz * 7 + dx * w), AIR));
            }
        }
    }
    // The hall: a spawner in the middle, vaults by the walls, lights above.
    out.push((o + ivec3(0, 1, 0), TRIAL_SPAWNER));
    out.push((o + ivec3(-5, 1, -5), VAULT));
    out.push((o + ivec3(5, 1, 5), VAULT));
    out.push((o + ivec3(5, 1, -5), CHEST));
    for (x, z) in [(-4, -4), (4, 4), (-4, 4), (4, -4)] {
        out.push((o + ivec3(x, 6, z), COPPER_BULB_ON));
    }
    out.retain(|(p, _)| (1..CH - 1).contains(&p.y));
    out
}

/// The floor of a Trial Chamber whose chunk rolled one, if any.
pub fn chamber_y(seed: u32, cx: i32, cz: i32) -> i32 {
    18 + (hash2(seed ^ 0x7A15, cx, cz) * 10.0) as i32
}

impl Game {
    /// Throw a held Wind Charge (the host owns projectiles, so clients ask it to).
    pub fn throw_wind_charge(&mut self) {
        self.player.swing = 1.0;
        self.use_cd = 0.5;
        if self.is_client() {
            // (The host heard about it with the rest of our item use.)
            self.inv.consume_held();
            self.sfx(Sfx::Gust, None);
            return;
        }
        if !self.creative {
            self.inv.consume_held();
        }
        let dir = self.player.look_dir();
        let me = self.my_id;
        self.spawn_wind_charge(self.player.eye() + dir * 0.4, dir * WIND_SPEED, Some(me));
    }

    /// Sound a Goat Horn (everyone hears it, a long way off).
    pub fn blow_horn(&mut self) {
        self.use_cd = 5.0;
        self.player.swing = 1.0;
        let at = self.player.eye();
        self.sfx(Sfx::Horn, Some(at));
    }

    /// A joined player threw a wind charge (host side).
    pub fn host_throw_wind_charge(&mut self, from: u32) {
        let Some(p) = self.peers.get(&from) else { return };
        let dir = Vec3::new(p.yaw.sin() * p.pitch.cos(), p.pitch.sin(), -p.yaw.cos() * p.pitch.cos());
        let eye = p.target + Vec3::Y * crate::player::EYE;
        if self.peer_take(from, WIND_CHARGE, 1) {
            self.spawn_wind_charge(eye + dir * 0.4, dir * WIND_SPEED, Some(from));
        }
    }

    /// Set a wind charge flying: a Breeze's (`shooter` None) or a player's.
    pub fn spawn_wind_charge(&mut self, from: Vec3, vel: Vec3, shooter: Option<u32>) {
        let mut a = Arrow::new(from, vel, shooter, 0.0);
        a.wind = true;
        a.life = 4.0;
        a.appearance = ProjectileAppearance { model: ProjectileModel::Billboard, tile: Some(crate::texture::T_WIND_CHARGE), scale: 0.6 };
        self.arrows.push(a);
        self.sfx(Sfx::Gust, Some(from));
    }

    /// A wind charge bursts: everyone near is flung away (and up). Nothing breaks.
    /// A Breeze's charge stings a little on a direct hit. Host side.
    pub fn wind_burst(&mut self, at: Vec3, shooter: Option<u32>) {
        self.sfx(Sfx::Gust, Some(at));
        self.smoke(at, 10, 0.6);
        let fling = |pos: Vec3| -> Option<Vec3> {
            let d = pos.distance(at);
            (d <= WIND_RADIUS).then(|| {
                let k = 1.0 - d / WIND_RADIUS * 0.6;
                let away = (pos - at).normalize_or(Vec3::Y);
                (away * 9.0 + Vec3::Y * 7.0) * k
            })
        };
        let sting = |pos: Vec3| if shooter.is_none() && pos.distance(at) < 1.2 { 1.0 } else { 0.0 };
        let me = self.player.body.pos + Vec3::Y * 0.9;
        if !self.away() && !self.spectator && self.dead.is_none()
            && let Some(push) = fling(me)
        {
            let dmg = sting(me);
            if dmg > 0.0 {
                self.player.hurt = 0.0;
                self.hurt_player_from(dmg, "was blown away by a Breeze", Some(at), false);
            }
            if shooter == Some(self.my_id) {
                // Your own: a wind jump (and a soft landing from it).
                self.player.body.vel += push * 1.3;
                self.player.fall_start = self.player.body.pos.y;
                self.advance("wind_jump");
            } else {
                self.player.body.vel += self.steadied(push);
            }
        }
        let peers: Vec<(u32, Vec3)> = self.peers.iter().filter(|(_, p)| p.alive()).map(|(&id, p)| (id, p.target + Vec3::Y * 0.9)).collect();
        for (id, pos) in peers {
            if let Some(push) = fling(pos) {
                let push = if shooter == Some(id) { push * 1.3 } else { push };
                self.hurt_peer(id, sting(pos), "was blown away by a Breeze", push);
                if shooter == Some(id) {
                    let name = self.peer_name(id);
                    self.advance_for(&crate::players::record_key(&name), "wind_jump");
                }
            }
        }
        for m in self.mobs.iter_mut() {
            if m.kind == MobKind::Breeze || m.rider != 0 {
                continue;
            }
            if let Some(push) = fling(m.body.pos + Vec3::Y * m.body.height * 0.5) {
                m.body.vel += push * 0.8;
                if let Some(who) = shooter {
                    m.last_attacker = who;
                }
            }
        }
    }

    /// Trial Spawners near players run their waves; spent ones rest (where the world lives).
    pub fn trials_tick(&mut self, dt: f32) {
        if self.is_client() {
            return;
        }
        self.trial_timer -= dt;
        if self.trial_timer > 0.0 {
            return;
        }
        let dt = 0.5 - self.trial_timer;
        self.trial_timer = 0.5;
        let players = self.player_spots();
        let spawners: Vec<IVec3> = self.world.cages.iter().copied().filter(|p| is_trial_spawner(self.world.get_v(*p))).collect();
        // Fights whose spawners are gone (broken, or out of the world) are forgotten.
        self.trials.retain(|p, _| spawners.contains(p));
        for p in spawners {
            let id = self.world.get_v(p);
            let centre = p.as_vec3() + Vec3::new(0.5, 1.0, 0.5);
            let state = self.trials.entry(p).or_default();
            if id == TRIAL_SPAWNER_SPENT {
                if state.rest <= 0.0 {
                    // Found spent (say, after loading): it rests from now.
                    state.rest = REST_SECS;
                }
                state.rest -= dt;
                if state.rest <= 0.0 {
                    self.trials.remove(&p);
                    self.world.set_v(p, TRIAL_SPAWNER);
                }
                continue;
            }
            let near = players.iter().filter(|(_, at, _)| at.distance(centre) < WAKE_RANGE).count();
            if state.to_come == 0 && state.out.is_empty() {
                if near == 0 || !self.rules.difficulty.monsters() {
                    continue;
                }
                // A fresh challenge: bigger for more people.
                state.to_come = (4 + 2 * (near as u8 - 1)).min(10);
                state.next = 1.0;
                // Someone brought a Bad Omen: it's an Ominous Trial now.
                if self.take_omen_near(centre) {
                    self.make_ominous(p);
                }
            }
            let state = self.trials.get_mut(&p).expect("just made");
            let alive: Vec<u32> = state.out.iter().copied().filter(|id| self.mobs.iter().any(|m| m.id == *id && m.health > 0.0)).collect();
            let state = self.trials.get_mut(&p).expect("just made");
            state.out = alive;
            state.next -= dt;
            if state.to_come > 0 && state.out.len() < AT_ONCE && state.next <= 0.0 {
                state.next = 2.0;
                let spot = centre + Vec3::new(self.rng.range(-2.5, 2.5), 0.0, self.rng.range(-2.5, 2.5));
                let clear = (0..2).all(|h| self.world.get_v((spot + Vec3::Y * h as f32).floor().as_ivec3()) == AIR);
                if clear {
                    let ominous = state.ominous;
                    let mob = self.alloc_mob(trial_kind(p), spot);
                    if let Some(m) = self.mobs.iter_mut().find(|m| m.id == mob) {
                        m.persistent = true;
                        if ominous {
                            m.health *= OMINOUS_TOUGHNESS;
                        }
                    }
                    let state = self.trials.get_mut(&p).expect("still there");
                    state.out.push(mob);
                    state.to_come -= 1;
                    self.smoke(spot + Vec3::Y, 8, 0.4);
                    self.sfx(Sfx::Warp, Some(spot));
                }
                continue;
            }
            if state.to_come == 0 && state.out.is_empty() {
                // Wave beaten: pay out, then rest.
                state.rest = REST_SECS;
                let ominous = std::mem::take(&mut state.ominous);
                self.world.set_v(p, TRIAL_SPAWNER_SPENT);
                self.pop_drop(centre, if ominous { OMINOUS_TRIAL_KEY } else { TRIAL_KEY }, 1);
                let (item, n) = spoils(&mut self.rng);
                self.pop_drop(centre, item, n);
                if ominous {
                    let (item, n) = spoils(&mut self.rng);
                    self.pop_drop(centre, item, n);
                    if self.rng.chance(0.3) {
                        self.pop_drop(centre, OMINOUS_BOTTLE, 1);
                    }
                }
                self.sfx(Sfx::Fanfare, Some(centre));
            }
        }
    }

    /// A player near `at` has Bad Omen: it's used up (true). Where the world lives.
    pub fn take_omen_near(&mut self, at: Vec3) -> bool {
        let who = self.player_spots_by_id().into_iter().find(|(id, pos)| pos.distance(at) < WAKE_RANGE && self.omens.contains_key(id)).map(|(id, _)| id);
        let Some(id) = who else { return false };
        self.omens.remove(&id);
        if id == self.my_id {
            self.effects.retain(|e| e.kind != crate::potions::Potion::BadOmen);
        } else {
            self.net_send_to(id, Msg::TimedEffect { effect: crate::potions::Potion::BadOmen.effect_index(), secs: 0.0, amplifier: 0 });
        }
        let name = if id == self.my_id { self.player_name.clone() } else { self.peer_name(id) };
        self.tell(&crate::players::record_key(&name), "The Bad Omen settles on the Trial Spawner. This one's going to be ominous.");
        true
    }

    /// Turn a waking spawner (and the vaults around it) ominous.
    pub fn make_ominous(&mut self, p: IVec3) {
        if let Some(t) = self.trials.get_mut(&p) {
            t.ominous = true;
            t.to_come = (t.to_come as f32 * 1.5).ceil() as u8;
        }
        self.world.set_v(p, TRIAL_SPAWNER_OMINOUS);
        let r = OMINOUS_REACH;
        for dz in -r..=r {
            for dy in -4..=4 {
                for dx in -r..=r {
                    let q = p + IVec3::new(dx, dy, dz);
                    if self.world.get_v(q) == VAULT {
                        self.world.set_v(q, VAULT_OMINOUS);
                    }
                }
            }
        }
        self.sfx(Sfx::Shriek, Some(p.as_vec3() + Vec3::splat(0.5)));
    }

    /// Open a Vault with a held Trial Key (clients ask the host).
    pub fn use_vault(&mut self, p: IVec3) {
        self.player.swing = 1.0;
        let key = if self.world.get_v(p) == VAULT_OMINOUS { OMINOUS_TRIAL_KEY } else { TRIAL_KEY };
        if self.inv.held() != key {
            self.msg(if key == TRIAL_KEY { "The Vault is locked. It wants a Trial Key." } else { "This Vault wants an Ominous Trial Key." });
            return;
        }
        if self.is_client() {
            self.inv.consume_held();
            self.net_send_msg(Msg::Interact { x: p.x, y: p.y, z: p.z, item: key });
            self.advance(if key == OMINOUS_TRIAL_KEY { "ominous_vault" } else { "under_lock" });
            return;
        }
        if self.open_vault(p) {
            if !self.creative {
                self.inv.consume_held();
            }
            self.advance(if key == OMINOUS_TRIAL_KEY { "ominous_vault" } else { "under_lock" });
        }
    }

    /// A joined player turns a key in a Vault.
    pub fn host_use_vault(&mut self, from: u32, p: IVec3) {
        let key = match self.world.get_v(p) {
            VAULT => TRIAL_KEY,
            VAULT_OMINOUS => OMINOUS_TRIAL_KEY,
            _ => return,
        };
        if self.peer_take(from, key, 1) {
            self.open_vault(p);
        }
    }

    /// Where the world lives: the Vault opens and its treasure tumbles out.
    pub fn open_vault(&mut self, p: IVec3) -> bool {
        let ominous = match self.world.get_v(p) {
            VAULT => false,
            VAULT_OMINOUS => true,
            _ => return false,
        };
        self.world.set_v(p, VAULT_OPEN);
        let at = p.as_vec3() + Vec3::new(0.5, 1.1, 0.5);
        let loot = if ominous { ominous_loot(&mut self.rng) } else { vault_loot(&mut self.rng) };
        for (item, n) in loot {
            self.pop_drop(at, item, n);
        }
        self.sfx(Sfx::Fanfare, Some(at));
        true
    }
}

/// Spots whose spawners sit at the corners of a chunk-sized grid can't
/// overlap: a chamber reaches 23 blocks from its middle.
pub const REACH: i32 = 23;
const _: () = assert!(REACH < 2 * CW);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::tests::arena;

    #[test]
    fn chambers_have_spawners_vaults_and_chests() {
        let b = chamber_blocks(ivec3(100, 20, 100), 7);
        assert_eq!(b.iter().filter(|x| x.1 == TRIAL_SPAWNER).count(), 5);
        assert_eq!(b.iter().filter(|x| x.1 == VAULT).count(), 2);
        assert!(b.iter().filter(|x| x.1 == CHEST).count() >= 5);
        assert!(b.iter().all(|(p, _)| (p.x - 100).abs() <= REACH && (p.z - 100).abs() <= REACH));
        // Later blocks win: the hall's middle is open, with the spawner on the floor.
        let last = |q: IVec3| b.iter().rev().find(|x| x.0 == q).map(|x| x.1);
        assert_eq!(last(ivec3(100, 21, 100)), Some(TRIAL_SPAWNER));
        assert_eq!(last(ivec3(100, 23, 100)), Some(AIR));
        assert_eq!(last(ivec3(107, 22, 100)), Some(AIR), "a doorway east");
    }

    #[test]
    fn a_beaten_wave_pays_a_key_and_the_vault_opens_for_it() {
        let mut g = arena(31);
        let base = g.player.body.pos.floor().as_ivec3();
        let sp = base + ivec3(4, 0, 0);
        g.world.set_v(sp, TRIAL_SPAWNER);
        assert!(g.world.cages.contains(&sp));
        for _ in 0..20 {
            g.trials_tick(0.6);
        }
        let out = g.trials.get(&sp).map(|t| t.out.clone()).unwrap_or_default();
        assert!(!out.is_empty(), "the trial began");
        // Beat everything it sends.
        for _ in 0..60 {
            for m in g.mobs.iter_mut() {
                m.health = -1.0;
            }
            g.mobs.clear();
            g.trials_tick(2.1);
            if g.world.get_v(sp) == TRIAL_SPAWNER_SPENT {
                break;
            }
        }
        assert_eq!(g.world.get_v(sp), TRIAL_SPAWNER_SPENT);
        assert!(g.drops.iter().any(|d| d.item == TRIAL_KEY), "a key for the winner");
        let vault = base + ivec3(-3, 0, 0);
        g.world.set_v(vault, VAULT);
        g.inv.slots[g.inv.selected] = Some((TRIAL_KEY, 1));
        let before = g.drops.len();
        g.use_vault(vault);
        assert_eq!(g.world.get_v(vault), VAULT_OPEN);
        assert!(g.drops.len() > before, "treasure");
        assert_eq!(g.inv.held(), AIR, "the key is used up");
        // Spent spawners wake again after their rest.
        g.trials_tick(REST_SECS + 1.0);
        g.trials_tick(1.0);
        assert_eq!(g.world.get_v(sp), TRIAL_SPAWNER);
    }

    #[test]
    fn a_bad_omen_makes_the_trial_ominous() {
        let mut g = arena(33);
        let base = g.player.body.pos.floor().as_ivec3();
        let sp = base + ivec3(4, 0, 0);
        let vault = base + ivec3(-3, 0, 0);
        g.world.set_v(sp, TRIAL_SPAWNER);
        g.world.set_v(vault, VAULT);
        let me = g.my_id;
        g.give_effect_to(me, crate::potions::Potion::BadOmen, 600.0);
        g.trials_tick(0.6);
        assert_eq!(g.world.get_v(sp), TRIAL_SPAWNER_OMINOUS);
        assert_eq!(g.world.get_v(vault), VAULT_OMINOUS, "the vaults nearby turn too");
        assert!(!g.has_effect(crate::potions::Potion::BadOmen), "the omen is used up");
        for _ in 0..80 {
            g.mobs.clear();
            g.trials_tick(2.1);
            if g.world.get_v(sp) == TRIAL_SPAWNER_SPENT {
                break;
            }
        }
        assert!(g.drops.iter().any(|d| d.item == OMINOUS_TRIAL_KEY));
        // An ordinary key won't do; the ominous one opens it.
        g.inv.slots[g.inv.selected] = Some((TRIAL_KEY, 1));
        g.use_vault(vault);
        assert_eq!(g.world.get_v(vault), VAULT_OMINOUS);
        g.inv.slots[g.inv.selected] = Some((OMINOUS_TRIAL_KEY, 1));
        g.use_vault(vault);
        assert_eq!(g.world.get_v(vault), VAULT_OPEN);
        // Heavy Cores turn up in ominous loot.
        let mut rng = crate::noise::Rng::new(3);
        assert!((0..200).any(|_| ominous_loot(&mut rng).iter().any(|l| l.0 == HEAVY_CORE)));
    }

    #[test]
    fn maces_hit_harder_the_further_you_fell() {
        use crate::combat::smash_bonus;
        assert_eq!(smash_bonus(0.0), 0.0);
        assert_eq!(smash_bonus(3.0), 12.0);
        assert_eq!(smash_bonus(8.0), 22.0);
        assert_eq!(smash_bonus(10.0), 24.0);
    }

    #[test]
    fn wind_charges_fling_without_breaking_anything() {
        let mut g = arena(32);
        let me = g.my_id;
        let feet = g.player.body.pos;
        let below = feet.floor().as_ivec3() - IVec3::Y;
        let ground = g.world.get_v(below);
        g.player.body.vel = Vec3::ZERO;
        g.wind_burst(feet + Vec3::new(0.0, 0.1, 0.3), Some(me));
        assert!(g.player.body.vel.y > 5.0, "a wind jump");
        assert_eq!(g.world.get_v(below), ground, "nothing breaks");
        // A Breeze's charge flies straight and bursts on the player.
        g.player.body.vel = Vec3::ZERO;
        let hp = g.player.health;
        g.spawn_wind_charge(feet + Vec3::new(0.0, 1.0, -4.0), Vec3::new(0.0, 0.0, WIND_SPEED), None);
        for _ in 0..20 {
            g.update_arrows(0.05);
        }
        assert!(g.arrows.is_empty(), "it burst");
        assert!(g.player.health < hp, "a direct hit stings");
        assert!(g.player.body.vel.length() > 1.0);
    }
}
