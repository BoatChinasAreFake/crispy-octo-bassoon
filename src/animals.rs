//! Farm animals and pets: feeding, breeding, shearing and taming.
//!
//! - **Breeding.** Feed an adult its favourite food (Oinkers carrots or
//!   potatoes, Fluffers and Mooers wheat, Clucksters seeds, tamed Woofers
//!   meat) and it falls in love for half a minute. Two in love find each
//!   other and make a baby, which grows up in four minutes. Parents rest for
//!   five before doing it again. Animals follow anyone holding their food.
//! - **Shearing.** Shears take 1–3 wool off a Fluffer; it grows back once the
//!   Fluffer has eaten some grass.
//! - **Taming.** Woofers live in forests and snowy places. Leave them alone
//!   and they leave you alone. Feed one bones and there's a one-in-three
//!   chance each time it becomes yours: it wears a collar, follows you,
//!   teleports to you when left behind, goes after whatever you hit (or
//!   whatever hits you), and sits or stands when you right-click it.
//!
//! Tamed, bred and fed animals stay put when everyone's far away, and are
//! saved with the world. Everything here happens where the world lives;
//! joined players ask the host with `Msg::MobInteract`.

use crate::block::*;
use crate::entity::{Mob, MobKind, Particle};
use crate::game::Game;
use crate::net::Msg;
use crate::players::record_key;
use crate::sound::Sfx;
use crate::texture::T_HEART;
use macroquad::math::Vec3;

/// Seconds in love, resting after breeding, and growing up.
pub const LOVE_SECS: f32 = 30.0;
pub const BREED_REST: f32 = 300.0;
pub const GROW_SECS: f32 = 240.0;
/// Chance a bone tames a Woofer.
pub const TAME_CHANCE: f32 = 1.0 / 3.0;
/// Tamed Woofers are hardier.
pub const TAMED_HEALTH: f32 = 20.0;
/// How far animals notice food, partners and owners.
const FOOD_RANGE: f32 = 8.0;
const LOVE_RANGE: f32 = 8.0;
/// Pets further than this from their owner teleport back to them.
const LEASH: f32 = 20.0;

/// What a right-click on a mob did (so the one clicking knows what it cost).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Interaction {
    Nothing,
    /// Used one of the held item up (feeding, taming).
    Ate,
    /// Wore the shears once.
    Sheared,
    /// A pet sat down or stood up.
    Toggled,
}

/// Would right-clicking this mob with `item` do anything? (Joined players
/// guess the same way before the host decides, to wear their shears.)
pub fn will_shear(m: &Mob, item: Id) -> bool {
    item == SHEARS && m.kind == MobKind::Fluffer && !m.sheared && m.baby <= 0.0
}

impl Game {
    /// The players' record keys and where they are (for owners and food).
    fn player_spots(&self) -> Vec<(String, Vec3, Id)> {
        let mut v = Vec::new();
        if !self.dedicated && self.dead.is_none() {
            v.push((record_key(&self.player_name), self.player.body.pos, self.inv.held()));
        }
        for p in self.peers.values().filter(|p| p.alive()) {
            v.push((record_key(&p.name), p.target, p.ledger.held));
        }
        v
    }

    /// Right-click on a mob by `who` (a record key) holding `item`.
    pub fn interact_mob(&mut self, who: &str, at: Vec3, mob_id: u32, item: Id) -> Interaction {
        let Some(i) = self.mobs.iter().position(|m| m.id == mob_id) else { return Interaction::Nothing };
        if self.mobs[i].body.pos.distance(at) > 6.0 {
            return Interaction::Nothing;
        }
        let pos = self.mobs[i].body.pos + Vec3::Y * self.mobs[i].body.height;
        let m = &mut self.mobs[i];
        // Shears.
        if will_shear(m, item) {
            m.sheared = true;
            m.persistent = true;
            let n = self.rng.int(1, 3) as u8;
            self.pop_drop(pos, WOOL, n);
            self.sfx(Sfx::Snip, Some(pos));
            return Interaction::Sheared;
        }
        let m = &mut self.mobs[i];
        match m.kind {
            MobKind::Woofer if m.owner.is_none() => {
                if item != BONE {
                    return Interaction::Nothing;
                }
                if self.rng.chance(TAME_CHANCE) {
                    m.owner = Some(who.to_string());
                    m.angry = false;
                    m.sitting = true;
                    m.persistent = true;
                    m.health = TAMED_HEALTH;
                    self.hearts(pos, 7);
                    self.sfx(Sfx::Woof, Some(pos));
                    self.tell(who, "The Woofer is yours now. It's sitting; right-click to call it along.");
                    self.advance_for(who, "good_boy");
                } else {
                    self.smoke(pos, 5, 0.2);
                }
                Interaction::Ate
            }
            MobKind::Woofer if m.owner.as_deref() == Some(who) => {
                if m.kind.breed_food().contains(&item) {
                    if m.health < TAMED_HEALTH {
                        m.health = (m.health + food_value(item).unwrap_or(2.0)).min(TAMED_HEALTH);
                        self.hearts(pos, 2);
                        return Interaction::Ate;
                    }
                    if m.ready_to_breed() {
                        m.love = LOVE_SECS;
                        self.hearts(pos, 4);
                        return Interaction::Ate;
                    }
                    return Interaction::Nothing;
                }
                m.sitting = !m.sitting;
                m.prey = None;
                Interaction::Toggled
            }
            k if k.passive() && k.breed_food().contains(&item) && m.ready_to_breed() => {
                m.love = LOVE_SECS;
                m.persistent = true;
                self.hearts(pos, 4);
                Interaction::Ate
            }
            _ => Interaction::Nothing,
        }
    }

    /// The local player right-clicked a mob.
    pub fn use_on_mob(&mut self, mob_index: usize) -> bool {
        let held = self.inv.held();
        let mob = &self.mobs[mob_index];
        let id = mob.id;
        if self.is_client() {
            // The host decides (and takes what was used); shears wear here and there alike.
            if will_shear(mob, held) {
                self.use_tool(1);
            }
            self.net_send_msg(Msg::MobInteract { mob: id, item: held });
            self.player.swing = 1.0;
            return true;
        }
        let me = record_key(&self.player_name);
        let at = self.player.body.pos;
        match self.interact_mob(&me, at, id, held) {
            Interaction::Nothing => false,
            Interaction::Ate => {
                if !self.creative {
                    self.use_up_held();
                }
                self.player.swing = 1.0;
                true
            }
            Interaction::Sheared => {
                self.use_tool(1);
                self.player.swing = 1.0;
                true
            }
            Interaction::Toggled => true,
        }
    }

    /// A joined player right-clicked a mob.
    pub fn host_mob_interact(&mut self, from: u32, mob: u32, item: Id) {
        let Some(p) = self.peers.get(&from) else { return };
        let (who, at) = (record_key(&p.name), p.target);
        if item != AIR && !self.peer_has(from, item) {
            return;
        }
        match self.interact_mob(&who, at, mob, item) {
            Interaction::Ate if !self.creative => self.take_peer(from, item, 1),
            Interaction::Sheared => self.host_wear(from, SHEARS, 1),
            _ => {}
        }
    }

    /// A message for one player (by record key).
    fn tell(&mut self, who: &str, text: &str) {
        if record_key(&self.player_name) == who && !self.dedicated {
            self.msg(text);
        } else if let Some(id) = self.peers.iter().find(|(_, p)| record_key(&p.name) == who).map(|(id, _)| *id) {
            self.system_message(Some(id), text);
        }
    }

    /// Advancements are only for the local player.
    fn advance_for(&mut self, who: &str, key: &str) {
        if record_key(&self.player_name) == who && !self.dedicated {
            self.advance(key);
        }
    }

    /// Little hearts floating up.
    pub fn hearts(&mut self, at: Vec3, n: usize) {
        if self.dedicated {
            return;
        }
        for _ in 0..n {
            let r = &mut self.rng;
            self.particles.push(Particle {
                pos: at + Vec3::new(r.range(-0.4, 0.4), r.range(0.0, 0.3), r.range(-0.4, 0.4)),
                vel: Vec3::new(r.range(-0.3, 0.3), r.range(0.6, 1.2), r.range(-0.3, 0.3)),
                life: r.range(0.8, 1.4),
                tile: T_HEART,
                uv: [0.375, 0.375],
                size: 0.16,
                gravity: -0.5,
            });
        }
    }

    /// Point a player's pets at a mob (they hit it, or it hit them).
    pub fn sic_pets(&mut self, owner: &str, target: u32) {
        let hostile_or_any = self.mobs.iter().find(|m| m.id == target).map(|m| m.owner.is_none() && m.kind != MobKind::Hisser).unwrap_or(false);
        if !hostile_or_any {
            return;
        }
        for m in self.mobs.iter_mut().filter(|m| m.owner.as_deref() == Some(owner) && !m.sitting && m.id != target) {
            m.prey = Some(target);
        }
    }

    /// Where the world lives: give animals somewhere to go, breed, bite, regrow.
    pub fn animals_tick(&mut self, dt: f32) {
        let players = self.player_spots();
        let n = self.mobs.len();
        let mut babies: Vec<(MobKind, Vec3, Option<String>)> = Vec::new();
        let mut bites: Vec<(usize, u32)> = Vec::new();
        for i in 0..n {
            let kind = self.mobs[i].kind;
            let pos = self.mobs[i].body.pos;
            let mut goal = None;
            // Tamed Woofers: prey first, else their owner.
            if let Some(owner) = self.mobs[i].owner.clone() {
                let owner_at = players.iter().find(|p| p.0 == owner).map(|p| p.1);
                if owner_at.is_none() {
                    // Owner's away: wait for them.
                    self.mobs[i].prey = None;
                }
                if let Some(prey) = self.mobs[i].prey {
                    match self.mobs.iter().find(|m| m.id == prey && m.health > 0.0).map(|m| m.body.pos) {
                        Some(p) if p.distance(pos) < 24.0 => {
                            goal = Some(p);
                            if p.distance(pos) < 1.8 {
                                bites.push((i, prey));
                            }
                        }
                        _ => self.mobs[i].prey = None,
                    }
                }
                if goal.is_none() && !self.mobs[i].sitting {
                    if let Some(o) = owner_at {
                        goal = Some(o);
                        // Left behind: catch up.
                        if o.distance(pos) > LEASH && self.world.is_loaded(o.x.floor() as i32, o.z.floor() as i32) {
                            if let Some(spot) = crate::entity::warp_spot(&self.world, o, 2.5, &mut self.rng) {
                                self.mobs[i].body.pos = spot;
                                self.mobs[i].body.vel = Vec3::ZERO;
                            }
                        }
                    }
                }
            }
            // In love: find a partner.
            if goal.is_none() && self.mobs[i].love > 0.0 {
                let me = &self.mobs[i];
                let partner = (0..n)
                    .filter(|&j| j != i)
                    .filter(|&j| {
                        let o = &self.mobs[j];
                        o.kind == kind && o.love > 0.0 && o.baby <= 0.0 && o.body.pos.distance(pos) < LOVE_RANGE && (kind != MobKind::Woofer || o.owner.is_some())
                    })
                    .min_by(|&a, &b| self.mobs[a].body.pos.distance(pos).total_cmp(&self.mobs[b].body.pos.distance(pos)));
                if let Some(j) = partner {
                    let other = self.mobs[j].body.pos;
                    goal = Some(other);
                    if other.distance(pos) < 1.6 && i < j {
                        babies.push((kind, (pos + other) * 0.5, me.owner.clone()));
                        for k in [i, j] {
                            let m = &mut self.mobs[k];
                            m.love = 0.0;
                            m.breed_cd = BREED_REST;
                        }
                    }
                }
            }
            // Otherwise follow whoever is holding something tasty.
            if goal.is_none() && self.mobs[i].baby <= 0.0 && (kind.passive() || (kind == MobKind::Woofer && self.mobs[i].owner.is_none())) {
                let food = if kind == MobKind::Woofer { &[BONE][..] } else { kind.breed_food() };
                goal = players.iter().filter(|p| food.contains(&p.2) && p.1.distance(pos) < FOOD_RANGE).map(|p| p.1).next();
            }
            // Babies trail after a grown-up of their kind.
            if goal.is_none() && self.mobs[i].baby > 0.0 {
                goal = (0..n).filter(|&j| self.mobs[j].kind == kind && self.mobs[j].baby <= 0.0 && self.mobs[j].body.pos.distance(pos) < 12.0).map(|j| self.mobs[j].body.pos).next();
            }
            self.mobs[i].goal = goal;
            // Sheared Fluffers grow their wool back by eating grass.
            let m = &self.mobs[i];
            if m.kind == MobKind::Fluffer && m.sheared && m.body.on_ground && self.rng.chance(dt / 30.0) {
                let under = macroquad::math::IVec3::new(pos.x.floor() as i32, (pos.y - 0.1).floor() as i32, pos.z.floor() as i32);
                if self.world.get_v(under) == GRASS {
                    self.world.set_v(under, DIRT);
                    self.mobs[i].sheared = false;
                    self.sfx(Sfx::Baa, Some(pos));
                }
            }
            if self.mobs[i].love > 0.0 && self.rng.chance(dt * 2.0) {
                let h = self.mobs[i].body.height;
                self.hearts(pos + Vec3::Y * h, 1);
            }
        }
        // Pets bite.
        for (i, prey) in bites {
            if self.mobs[i].attack_cd > 0.0 {
                continue;
            }
            self.mobs[i].attack_cd = 1.0;
            let from = self.mobs[i].body.pos;
            if let Some(m) = self.mobs.iter_mut().find(|m| m.id == prey) {
                m.hurt = 0.0;
                m.damage(4.0, from);
                let (kind, at) = (m.kind, m.body.pos);
                self.sfx(Sfx::hurt_of(kind), Some(at));
                self.sfx(Sfx::Woof, Some(from));
            }
        }
        for (kind, at, owner) in babies {
            let mut b = Mob::new(kind, at, &mut self.rng);
            b.id = self.next_mob_id;
            self.next_mob_id += 1;
            b.set_baby(GROW_SECS);
            b.persistent = true;
            b.breed_cd = GROW_SECS;
            if kind == MobKind::Woofer {
                b.owner = owner;
                b.health = TAMED_HEALTH;
            }
            self.hearts(at + Vec3::Y * 0.5, 6);
            let points = self.rng.int(1, 7) as u32;
            self.spawn_orbs(at + Vec3::Y * 0.5, points);
            self.mobs.push(b);
            if self.player.body.pos.distance(at) < 16.0 {
                self.advance("the_birds_and_the_bees");
            }
        }
    }
}

// ------------------------------------------------------------------ saving

/// A mob kept in the save file (only the ones worth keeping: see `Mob::persistent`).
pub fn encode_mobs(mobs: &[Mob]) -> Vec<u8> {
    let mut out = Vec::new();
    let keep: Vec<&Mob> = mobs.iter().filter(|m| m.persistent && m.health > 0.0).collect();
    out.extend_from_slice(&(keep.len() as u32).to_le_bytes());
    for m in keep {
        out.push(m.kind.index());
        for v in [m.body.pos.x, m.body.pos.y, m.body.pos.z, m.yaw, m.health, m.baby.max(0.0), m.breed_cd] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.push(m.size as u8);
        out.push(m.sheared as u8 | (m.sitting as u8) << 1);
        let owner = m.owner.as_deref().unwrap_or("");
        let owner = &owner.as_bytes()[..owner.len().min(64)];
        out.push(owner.len() as u8);
        out.extend_from_slice(owner);
    }
    out
}

/// Unpack `encode_mobs` (stops at anything malformed). Ids are handed out by the caller.
pub fn decode_mobs(b: &[u8], rng: &mut crate::noise::Rng) -> Vec<Mob> {
    let mut v = Vec::new();
    let mut i = 0usize;
    let mut take = |n: usize| -> Option<&[u8]> {
        let s = b.get(i..i + n)?;
        i += n;
        Some(s)
    };
    let Some(count) = take(4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]])) else { return v };
    for _ in 0..count.min(4096) {
        let Some(kind) = take(1).and_then(|s| MobKind::from_index(s[0])) else { break };
        let Some(f) = take(28).map(|s| (0..7).map(|k| f32::from_le_bytes([s[k * 4], s[k * 4 + 1], s[k * 4 + 2], s[k * 4 + 3]])).collect::<Vec<f32>>()) else { break };
        let Some(&[size, flags]) = take(2) else { break };
        let Some(len) = take(1).map(|s| s[0] as usize) else { break };
        let Some(owner) = take(len).map(|s| String::from_utf8_lossy(s).into_owned()) else { break };
        if !f.iter().all(|x| x.is_finite()) {
            continue;
        }
        let mut m = Mob::new(kind, Vec3::new(f[0], f[1], f[2]), rng).with_size(size.max(1));
        m.yaw = f[3];
        m.health = f[4].clamp(1.0, 100.0);
        m.set_baby(f[5].clamp(0.0, GROW_SECS));
        m.breed_cd = f[6].clamp(0.0, BREED_REST);
        m.sheared = flags & 1 != 0;
        m.sitting = flags & 2 != 0;
        m.owner = (!owner.is_empty()).then_some(owner);
        m.persistent = true;
        v.push(m);
    }
    v
}
