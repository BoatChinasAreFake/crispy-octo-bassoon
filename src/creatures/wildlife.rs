//! **Turtles**, **Dolphins**, **Pandas**, **Polar Bears** and **Llamas**.
//!
//! Turtles paddle about near beaches and come ashore slowly; fed Lily Pads,
//! a pair lays Turtle Eggs on the nearest sand, which hatch at night (sooner
//! under a full moon). A baby sheds a Turtle Scute as it grows up; five make
//! a Turtle Shell. Dolphins swim fast in the open sea and leap now and then;
//! feed one a fish and it leads you to the nearest shipwreck. Hit one and it
//! bites back. Pandas sit about in jungles and live on bamboo. Polar Bears
//! roam the snow and leave you alone, unless you go near a cub or hit one.
//!
//! Llamas wander the plains. Feed a wild one hay (or wheat) a few times and
//! it's yours: it follows you about (empty-handed and sneaking, tell it to
//! stay or come along). Give a tame one a chest and it carries a chest's
//! worth of things; right-click it empty-handed to open its pack (only its
//! owner can). The pack is kept where the world lives in `World::packs`,
//! by the llama's id while the world runs and by its seed in the save.

use crate::animals::Interaction;
use crate::block::*;
use crate::containers::Container;
use crate::entity::{angle_diff, Mob, MobEvent, MobKind};
use crate::game::Game;
use crate::noise::Rng;
use crate::world::World;
use macroquad::math::{ivec3, IVec3, Vec3};

/// Hay (3) or wheat (1) a wild Llama needs before it's yours.
pub const TAME_AT: u8 = 6;
/// How long a fed Dolphin leads the way, and how far it looks for a wreck (chunks).
pub const LEAD_SECS: f32 = 90.0;
const WRECK_RANGE: i32 = 20;
/// Polar Bears mind anyone this close to a cub.
const CUB_GUARD: f32 = 8.0;

/// A Llama's pack is found by a container key made from its id (like a cart's; see containers.rs).
const PACK_X: i32 = i32::MIN + 4;
const PACK_Y: i32 = -4097;

pub fn pack_key(mob: u32) -> IVec3 {
    IVec3::new(PACK_X, PACK_Y, mob as i32)
}

pub fn pack_of_key(p: IVec3) -> Option<u32> {
    (p.x == PACK_X && p.y == PACK_Y).then_some(p.z as u32)
}

fn ahead_is_water(m: &Mob, world: &World) -> bool {
    let ahead = m.body.pos + Vec3::new(m.yaw.sin(), 0.2, -m.yaw.cos()) * 0.9;
    is_wet(world.get(ahead.x.floor() as i32, ahead.y.floor() as i32, ahead.z.floor() as i32))
}

/// Drift about in the water without leaving it.
fn paddle(m: &mut Mob, world: &World, rng: &mut Rng, swim_vy: &mut Option<f32>) {
    if !ahead_is_water(m, world) {
        m.wander_dir = Some(m.yaw + std::f32::consts::PI + rng.range(-0.8, 0.8));
        m.wander_t = rng.range(1.0, 3.0);
    }
    let above = world.get(m.body.pos.x.floor() as i32, (m.body.pos.y + 0.8).floor() as i32, m.body.pos.z.floor() as i32);
    let drift = (m.wander_t * 1.5 + m.id as f32).sin() * 0.7;
    *swim_vy = Some(if is_water(above) { drift } else { drift.min(0.0) - 0.3 });
}

/// How these creatures move (from `Mob::update`).
#[allow(clippy::too_many_arguments)]
pub fn update(m: &mut Mob, dt: f32, world: &World, player: Vec3, visible: bool, rng: &mut Rng, want: &mut Option<(f32, f32)>, swim_vy: &mut Option<f32>, may_wander: &mut bool, ev: &mut Vec<MobEvent>) {
    let to = player - m.body.pos;
    let flat = Vec3::new(to.x, 0.0, to.z);
    let face = flat.x.atan2(-flat.z);
    let away = (-flat.x).atan2(flat.z);
    let toward = |g: Vec3, from: Vec3| (g.x - from.x).atan2(-(g.z - from.z));
    match m.kind {
        MobKind::Turtle => {
            if m.body.in_water {
                if m.flee > 0.0 {
                    *want = Some((away, 3.0));
                } else if let Some(g) = m.goal {
                    *want = Some((toward(g, m.body.pos), 2.4));
                    *swim_vy = Some(((g.y - m.body.pos.y) * 2.0).clamp(-2.0, 2.0));
                }
                if swim_vy.is_none() {
                    paddle(m, world, rng, swim_vy);
                }
            } else if m.flee > 0.0 {
                *want = Some((away, 1.6));
            } else if let Some(g) = m.goal {
                *want = Some((toward(g, m.body.pos), 1.0));
            } else if !m.sitting {
                // A slow plod back toward the water.
                *want = Some((m.yaw, 0.7));
            }
        }
        MobKind::Dolphin => {
            if !m.body.in_water {
                // Stranded: flop about and dry out.
                *may_wander = false;
                m.health -= dt * 0.3;
                if m.body.on_ground && rng.chance(dt * 1.5) {
                    m.body.vel.y = 4.0;
                    m.yaw = rng.range(0.0, std::f32::consts::TAU);
                }
                return;
            }
            if !visible || flat.length() > 24.0 {
                m.angry = false;
            }
            if m.angry && visible {
                *want = Some((face, 5.0));
                *swim_vy = Some((to.y * 2.0).clamp(-3.0, 3.0));
                if flat.length() < 1.4 && to.y.abs() < 1.5 && m.attack_cd <= 0.0 {
                    ev.push(MobEvent::HurtPlayer(3.0, "was nipped by a Dolphin. Fair enough"));
                    m.attack_cd = 1.0;
                }
            } else if let Some(g) = m.goal {
                // Leading the way: well ahead, but not out of sight.
                let wait = flat.length() > 14.0 && visible;
                if wait {
                    m.yaw += angle_diff(face, m.yaw).clamp(-4.0 * dt, 4.0 * dt);
                    *may_wander = false;
                } else {
                    *want = Some((toward(g, m.body.pos), 5.0));
                }
                *swim_vy = Some(((g.y + 2.0 - m.body.pos.y) * 1.5).clamp(-3.0, 3.0));
            } else {
                *want = Some((m.wander_dir.unwrap_or(m.yaw), 3.5));
                paddle(m, world, rng, swim_vy);
                // A leap out of the water now and then.
                let surface = !is_water(world.get(m.body.pos.x.floor() as i32, (m.body.pos.y + 1.0).floor() as i32, m.body.pos.z.floor() as i32));
                if surface && rng.chance(dt * 0.4) {
                    m.body.vel.y = 7.0;
                    *swim_vy = None;
                }
            }
        }
        MobKind::PolarBear => {
            if !visible || flat.length() > 24.0 {
                m.angry = false;
            }
            if m.angry && visible && m.baby <= 0.0 {
                *want = Some((face, 4.2));
                if flat.length() < 1.7 && to.y.abs() < 1.8 && m.attack_cd <= 0.0 {
                    ev.push(MobEvent::HurtPlayer(6.0, "got too close to a Polar Bear's cub"));
                    m.attack_cd = 1.2;
                }
            } else if m.flee > 0.0 && m.baby > 0.0 {
                *want = Some((away, 3.0));
            } else if let Some(g) = m.goal
                && Vec3::new(g.x - m.body.pos.x, 0.0, g.z - m.body.pos.z).length() > 1.5
            {
                *want = Some((toward(g, m.body.pos), 1.8));
            }
            if m.body.in_water {
                *swim_vy = Some(0.6);
            }
        }
        MobKind::Llama => {
            if !visible || flat.length() > 16.0 {
                m.angry = false;
            }
            if m.sitting {
                *may_wander = false;
            } else if m.angry && visible {
                // Spits from where it stands.
                m.yaw += angle_diff(face, m.yaw).clamp(-6.0 * dt, 6.0 * dt);
                *may_wander = false;
                if flat.length() < 10.0 && m.attack_cd <= 0.0 {
                    ev.push(MobEvent::HurtPlayer(1.0, "was spat at by a Llama. Rude"));
                    ev.push(MobEvent::Smoke(m.body.pos + Vec3::Y * 1.7 + flat.normalize_or_zero() * 0.8));
                    m.attack_cd = 2.5;
                }
            } else if m.flee > 0.0 {
                *want = Some((away, 3.0));
            } else if let Some(g) = m.goal {
                let d = Vec3::new(g.x - m.body.pos.x, 0.0, g.z - m.body.pos.z).length();
                if d > 2.5 {
                    *want = Some((toward(g, m.body.pos), if d > 8.0 { 4.0 } else { 2.2 }));
                } else {
                    *may_wander = false;
                }
            }
        }
        _ => {}
    }
}

/// The nearest sand with room for an egg on top, within `r` blocks of `at`.
fn nest_spot(world: &World, at: Vec3, r: i32) -> Option<IVec3> {
    let c = at.floor().as_ivec3();
    let mut best: Option<(i32, IVec3)> = None;
    for dz in -r..=r {
        for dx in -r..=r {
            for dy in -3..=3 {
                let p = c + ivec3(dx, dy, dz);
                if world.get_v(p) == SAND && world.get_v(p + IVec3::Y) == AIR {
                    let d = dx * dx + dz * dz + dy * dy;
                    if best.is_none_or(|(b, _)| d < b) {
                        best = Some((d, p + IVec3::Y));
                    }
                }
            }
        }
    }
    best.map(|b| b.1)
}

impl Game {
    /// Where the world lives, after `animals_tick` (which sets every goal):
    /// Llamas follow their owners, Dolphins lead the way, bears guard cubs.
    pub fn wildlife_tick(&mut self, dt: f32) {
        let spots = self.player_spots();
        let cubs: Vec<Vec3> = self.mobs.iter().filter(|m| m.kind == MobKind::PolarBear && m.baby > 0.0).map(|m| m.body.pos).collect();
        let mothers: Vec<(u32, Vec3)> = self.mobs.iter().filter(|m| m.kind == MobKind::PolarBear && m.baby <= 0.0).map(|m| (m.id, m.body.pos)).collect();
        for m in self.mobs.iter_mut() {
            match m.kind {
                MobKind::Llama => {
                    let Some(owner) = m.owner.as_deref() else { continue };
                    if let Some((_, at, _)) = spots.iter().find(|s| s.0 == owner) {
                        let d = at.distance(m.body.pos);
                        m.goal = (d > 3.0 && d < 40.0 && !m.sitting).then_some(*at);
                    }
                }
                MobKind::Dolphin => {
                    if let Some(home) = m.home {
                        m.warp_cd -= dt;
                        if m.warp_cd <= 0.0 || home.distance(m.body.pos) < 4.0 {
                            m.home = None;
                        } else {
                            m.goal = Some(home);
                        }
                    }
                }
                MobKind::PolarBear if m.baby > 0.0 => {
                    // Cubs keep close to the nearest grown bear.
                    m.goal = mothers.iter().filter(|(id, _)| *id != m.id).map(|(_, p)| *p).min_by(|a, b| a.distance(m.body.pos).total_cmp(&b.distance(m.body.pos)));
                }
                MobKind::PolarBear => {
                    if cubs.iter().any(|c| c.distance(m.body.pos) < 16.0 && spots.iter().any(|s| s.1.distance(*c) < CUB_GUARD)) {
                        m.angry = true;
                    }
                }
                _ => {}
            }
        }
    }

    /// Right-clicking one of these (None: the usual rules apply).
    pub fn wildlife_interact(&mut self, who: &str, i: usize, item: Id) -> Option<Interaction> {
        let pos = self.mobs[i].body.pos + Vec3::Y * self.mobs[i].body.height;
        match self.mobs[i].kind {
            MobKind::Dolphin if matches!(item, COD | SALMON | COOKED_COD | COOKED_SALMON) => {
                let wreck = self.world.generator.nearest_site(crate::structures::Kind::Shipwreck, self.mobs[i].body.pos, WRECK_RANGE);
                self.mobs[i].angry = false;
                self.hearts(pos, 4);
                match wreck {
                    Some(w) => {
                        self.mobs[i].home = Some(w.as_vec3() + Vec3::new(0.5, 1.0, 0.5));
                        self.mobs[i].warp_cd = LEAD_SECS;
                        self.tell(who, "The Dolphin swims off, looking back at you. Follow it!");
                        self.advance_for(who, "follow_the_fin");
                    }
                    None => self.tell(who, "The Dolphin enjoys the fish. It doesn't know any wrecks round here."),
                }
                Some(Interaction::Ate)
            }
            MobKind::Llama => {
                let m = &mut self.mobs[i];
                let mine = m.owner.as_deref() == Some(who);
                if m.owner.is_none() && matches!(item, HAY | WHEAT) {
                    m.temper = m.temper.saturating_add(if item == HAY { 3 } else { 1 });
                    m.persistent = true;
                    if m.temper >= TAME_AT {
                        m.owner = Some(who.to_string());
                        m.angry = false;
                        self.hearts(pos, 7);
                        self.tell(who, "The Llama is yours now. It'll follow you; give it a chest to carry things.");
                        self.advance_for(who, "llama_drama");
                    } else {
                        self.smoke(pos, 3, 0.2);
                    }
                    return Some(Interaction::Ate);
                }
                if mine && item == CHEST && !m.saddled {
                    m.saddled = true;
                    let id = m.id;
                    self.world.packs.insert(id, Container::for_block(CHEST));
                    self.sfx(crate::sound::Sfx::Place(crate::sound::Mat::Wood), Some(pos));
                    return Some(Interaction::Ate);
                }
                if mine && item == AIR {
                    m.sitting = !m.sitting;
                    m.goal = None;
                    return Some(Interaction::Toggled);
                }
                None
            }
            _ => None,
        }
    }

    /// The local player: right-clicked their own Llama with a pack, empty-handed (not sneaking).
    pub fn wants_pack(&self, i: usize) -> bool {
        let m = &self.mobs[i];
        let mine = self.is_client() || m.owner.as_deref() == Some(crate::players::record_key(&self.player_name).as_str());
        m.kind == MobKind::Llama && m.saddled && mine && self.inv.held() == AIR && !self.player.sneaking
    }

    /// Is a Llama's pack in reach of `at` (and theirs, by record key)?
    pub fn pack_near(&self, mob: u32, at: Vec3, who: Option<&str>) -> bool {
        self.mobs.iter().any(|m| m.id == mob && m.kind == MobKind::Llama && m.saddled && (m.body.pos + Vec3::Y).distance(at) < 6.0 && who.is_none_or(|w| m.owner.is_none() || m.owner.as_deref() == Some(w)))
    }

    /// A Llama died: its pack spills, chest and all.
    pub fn spill_pack(&mut self, mob: u32, at: Vec3) {
        if let Some(c) = self.world.packs.remove(&mob) {
            for (item, n, wear) in c.contents() {
                self.pop_drop_worn(at, item, n, wear);
            }
            self.pop_drop(at, CHEST, 1);
        }
    }

    /// Lay Turtle Eggs on the nearest sand (instead of a baby; see animals.rs). False if there's none near.
    pub fn lay_turtle_eggs(&mut self, at: Vec3) -> bool {
        let Some(p) = nest_spot(&self.world, at, 12) else { return false };
        self.world.set_v(p, TURTLE_EGG);
        self.sfx(crate::sound::Sfx::Place(crate::sound::Mat::Sand), Some(p.as_vec3()));
        true
    }

    /// A Turtle Egg's random tick: at night (more so under a full moon) it hatches.
    pub fn turtle_egg_tick(&mut self, p: IVec3) {
        let chance = 0.05 * crate::skies::hatch_scale(self.moon_phase());
        if !self.is_night() || !self.rng.chance(chance) {
            return;
        }
        self.world.set_v(p, AIR);
        let at = p.as_vec3() + Vec3::new(0.5, 0.0, 0.5);
        for _ in 0..self.rng.int(1, 2) {
            let id = self.alloc_mob(MobKind::Turtle, at);
            if let Some(m) = self.mobs.iter_mut().find(|m| m.id == id) {
                m.set_baby(crate::animals::GROW_SECS);
                m.persistent = true;
            }
        }
        self.sfx(crate::sound::Sfx::Break(crate::sound::Mat::Glass), Some(at));
    }

    /// Save: packs by their llama's seed (ids change between runs).
    pub fn encode_packs(&self) -> Vec<u8> {
        let by_seed: std::collections::HashMap<i32, Container> = self.world.packs.iter().filter_map(|(id, c)| self.mobs.iter().find(|m| m.id == *id).map(|m| (m.seed as i32, c.clone()))).collect();
        crate::stash::encode(&by_seed)
    }

    /// Load: give each pack back to the llama with its seed.
    pub fn decode_packs(&mut self, b: &[u8]) {
        let by_seed = crate::stash::decode(b);
        self.world.packs = self.mobs.iter().filter(|m| m.kind == MobKind::Llama && m.saddled).filter_map(|m| by_seed.get(&(m.seed as i32)).map(|c| (m.id, c.clone()))).collect();
    }
}

/// Where Turtles might come ashore, Pandas sit, Polar Bears roam and Llamas graze (see `Game::spawn_tick`).
pub fn spawn_kind(biome: crate::world::Biome, top: Id, rng: &mut Rng) -> Option<MobKind> {
    use crate::world::Biome;
    match (biome, top) {
        (_, SAND) if biome != Biome::Desert && rng.chance(0.3) => Some(MobKind::Turtle),
        (Biome::Jungle, _) if rng.chance(0.25) => Some(MobKind::Panda),
        (Biome::Snowy | Biome::IceSpikes, _) if rng.chance(0.2) => Some(MobKind::PolarBear),
        (Biome::Plains | Biome::Taiga, _) if rng.chance(0.06) => Some(MobKind::Llama),
        // Llamas like it high and dry as well.
        (Biome::Savanna | Biome::StonyPeaks, _) if rng.chance(0.2) => Some(MobKind::Llama),
        _ => None,
    }
}

#[cfg(test)]
mod mushmooer_tests {
    use crate::block::*;
    use crate::entity::MobKind;
    use macroquad::math::Vec3;

    #[test]
    fn shearing_a_mushmooer_gives_mushrooms_and_leaves_a_mooer() {
        let mut g = crate::game::tests::arena(161);
        let me = crate::players::record_key(&g.player_name);
        let at = Vec3::new(2.5, 50.0, 0.5);
        let id = g.alloc_mob(MobKind::Mushmooer, at);
        assert_eq!(g.interact_mob(&me, g.player.body.pos, id, SHEARS), crate::animals::Interaction::Sheared);
        let m = g.mobs.iter().find(|m| m.id == id).unwrap();
        assert_eq!(m.kind, MobKind::Mooer);
        assert_eq!(g.drops.iter().filter(|d| d.item == MUSHROOM).map(|d| d.n as u32).sum::<u32>(), 5);
        // Shears do nothing more to it now.
        assert_eq!(g.interact_mob(&me, g.player.body.pos, id, SHEARS), crate::animals::Interaction::Nothing);
    }
}

/// Anyone's Llama seed: never 0 (so a pack always has a key in the save).
pub fn llama_seed(rng: &mut Rng) -> u32 {
    rng.int(1, 0xFF_FFFF) as u32
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::players::record_key;

    #[test]
    fn a_llama_is_tamed_with_hay_carries_a_chest_and_keeps_it_through_a_save() {
        let mut g = crate::game::tests::arena(151);
        let me = record_key(&g.player_name);
        g.player.body.pos = Vec3::new(0.5, 50.0, 0.5);
        let id = g.alloc_mob(MobKind::Llama, Vec3::new(2.5, 50.0, 0.5));
        let at = g.player.body.pos;
        assert_eq!(g.interact_mob(&me, at, id, CHEST), Interaction::Nothing, "not yours yet");
        for _ in 0..2 {
            assert_eq!(g.interact_mob(&me, at, id, HAY), Interaction::Ate);
        }
        let llama = g.mobs.iter().find(|m| m.id == id).unwrap();
        assert_eq!(llama.owner.as_deref(), Some(me.as_str()));
        assert_ne!(llama.seed, 0);
        assert_eq!(g.interact_mob(&me, at, id, CHEST), Interaction::Ate);
        assert!(g.world.packs.contains_key(&id));
        // Pack things into it like a chest.
        let key = pack_key(id);
        assert_eq!(crate::containers::store_kind(&g.world, &g.vehicles, key), CHEST);
        crate::containers::store(&mut g.world, &mut g.vehicles, key).unwrap().slots[4] = Some((DIAMOND, 5));
        assert!(g.pack_near(id, at + Vec3::Y, Some(&me)));
        assert!(!g.pack_near(id, at + Vec3::Y, Some("someone else")));
        // Saved by seed, and back with the llama under its new id.
        let bytes = g.encode_packs();
        let mut h = crate::game::tests::arena(152);
        let mut copy = Mob::new(MobKind::Llama, Vec3::new(5.0, 50.0, 5.0), &mut h.rng);
        copy.seed = g.mobs.iter().find(|m| m.id == id).unwrap().seed;
        copy.saddled = true;
        copy.id = 77;
        h.mobs.push(copy);
        h.decode_packs(&bytes);
        assert_eq!(h.world.packs[&77].slots[4], Some((DIAMOND, 5)));
        // It dies: everything spills, chest too.
        h.spill_pack(77, Vec3::new(5.0, 50.5, 5.0));
        assert!(h.drops.iter().any(|d| d.item == DIAMOND && d.n == 5) && h.drops.iter().any(|d| d.item == CHEST));
    }

    #[test]
    fn turtles_lay_eggs_on_sand_which_hatch_at_night_into_babies_that_shed_scutes() {
        let mut g = crate::game::tests::arena(153);
        for x in 4..8 {
            g.world.set_v(ivec3(x, 49, 0), SAND);
        }
        assert!(g.lay_turtle_eggs(Vec3::new(1.5, 50.0, 0.5)));
        let egg = ivec3(4, 50, 0);
        assert_eq!(g.world.get_v(egg), TURTLE_EGG, "on the nearest sand");
        g.time = 0.25;
        for _ in 0..200 {
            g.turtle_egg_tick(egg);
        }
        assert_eq!(g.world.get_v(egg), TURTLE_EGG, "not by day");
        g.time = 0.75;
        for _ in 0..2000 {
            g.turtle_egg_tick(egg);
        }
        assert_eq!(g.world.get_v(egg), AIR);
        let baby = g.mobs.iter_mut().find(|m| m.kind == MobKind::Turtle).expect("hatched");
        assert!(baby.baby > 0.0);
        baby.baby = 0.01;
        let ev = baby.update(0.05, &g.world, Vec3::new(20.0, 50.0, 20.0), false, 1.0, &mut Rng::new(1));
        assert!(ev.iter().any(|e| matches!(e, MobEvent::DropItem(_, TURTLE_SCUTE))), "a scute as it grows up");
    }

    #[test]
    fn a_fed_dolphin_leads_to_a_wreck_and_bears_guard_their_cubs() {
        let mut g = crate::game::tests::arena(154);
        let me = record_key(&g.player_name);
        g.player.body.pos = Vec3::new(0.5, 50.0, 0.5);
        let d = g.alloc_mob(MobKind::Dolphin, Vec3::new(1.5, 50.0, 0.5));
        let wreck = g.world.generator.nearest_site(crate::structures::Kind::Shipwreck, Vec3::new(1.5, 50.0, 0.5), WRECK_RANGE);
        assert_eq!(g.interact_mob(&me, g.player.body.pos, d, COD), Interaction::Ate);
        let dolphin = g.mobs.iter().find(|m| m.id == d).unwrap();
        assert_eq!(dolphin.home.is_some(), wreck.is_some(), "leads the way if there's a wreck in range");
        // A cub, its mother, and you too close.
        let cub = g.alloc_mob(MobKind::PolarBear, Vec3::new(3.5, 50.0, 0.5));
        g.mobs.iter_mut().find(|m| m.id == cub).unwrap().set_baby(100.0);
        let mum = g.alloc_mob(MobKind::PolarBear, Vec3::new(8.5, 50.0, 0.5));
        g.wildlife_tick(0.05);
        assert!(g.mobs.iter().find(|m| m.id == mum).unwrap().angry);
        assert!(!g.mobs.iter().find(|m| m.id == cub).unwrap().angry);
    }
}
