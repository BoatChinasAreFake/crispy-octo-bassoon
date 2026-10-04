//! Night threats, and some small creatures.
//!
//! - **Witches** come out at night (most of all in swamps). They keep their
//!   distance and throw splash potions: Slowness if you're far off, Poison
//!   or Weakness up close. Hurt, they drink a healing potion; on fire, they
//!   drink one against that.
//! - **Desert Groaners** are the Groaners of the desert: the sun doesn't
//!   bother them, and their bite leaves you hungry (Hunger).
//! - **Snowy Rattlers** are the Rattlers of the snow: their arrows are tipped
//!   with Slowness.
//! - **Glow Squid** drift in dark water, glowing. Hurt one and it squirts a
//!   cloud of ink. They drop **Glow Ink Sacs**: use one on a sign and its
//!   words glow.
//! - **Bats** flutter about in dark caves, and hang upside down from the
//!   ceiling for a rest.
//! - **Allays** are little blue spirits, kept in cages by Pilferer outposts.
//!   Give one an item (right-click) and it follows you about, fetching
//!   matching items it sees lying around and dropping them at your feet.
//!   Right-click it empty-handed to take the item back.

use crate::block::*;
use crate::entity::{Mob, MobEvent, MobKind};
use crate::game::Game;
use crate::noise::Rng;
use crate::potions::Potion;
use crate::world::World;
use macroquad::math::Vec3;

/// How far an Allay looks for things to fetch, and how far it strays from whoever gave it the item.
pub const FETCH_RANGE: f32 = 24.0;
const LEASH: f32 = 32.0;
/// Most an Allay carries at once.
pub const ALLAY_CARRY: u8 = 16;

/// How the night's newcomers and small creatures move (from `Mob::update`).
#[allow(clippy::too_many_arguments)]
pub fn update(m: &mut Mob, dt: f32, world: &World, player: Vec3, visible: bool, rng: &mut Rng, want: &mut Option<(f32, f32)>, swim_vy: &mut Option<f32>, fly_vy: &mut Option<f32>, may_wander: &mut bool, ev: &mut Vec<MobEvent>) {
    let to = player - m.body.pos;
    let flat = Vec3::new(to.x, 0.0, to.z);
    let fd = flat.length();
    let face = flat.x.atan2(-flat.z);
    m.warp_cd = (m.warp_cd - dt).max(0.0);
    match m.kind {
        MobKind::Witch => {
            // A drink first, if it needs one.
            let max = m.kind.max_health();
            if m.warp_cd <= 0.0 && (m.health < max * 0.5 || m.on_fire > 0.0) {
                if m.on_fire > 0.0 {
                    m.on_fire = 0.0;
                } else {
                    m.health = (m.health + 8.0).min(max);
                }
                m.warp_cd = 12.0;
                m.attack_cd = m.attack_cd.max(1.5);
                ev.push(MobEvent::Smoke(m.eye()));
                return;
            }
            if visible && to.length() < 20.0 {
                if fd > 10.0 {
                    *want = Some((face, 2.0));
                } else if fd < 5.0 {
                    *want = Some((face + std::f32::consts::PI, 1.8));
                } else {
                    m.yaw = face;
                    *may_wander = false;
                }
                let eye = m.eye();
                let aim = player + Vec3::Y * 0.9 - eye;
                let clear = world.raycast(eye, aim.normalize_or_zero(), aim.length()).is_none();
                if m.attack_cd <= 0.0 && fd < 13.0 && clear {
                    let p = if fd > 8.0 {
                        Potion::Slowness
                    } else if rng.chance(0.3) {
                        Potion::Weakness
                    } else {
                        Potion::Poison
                    };
                    // A lob: a little higher the further it has to go.
                    let vel = aim.normalize_or_zero() * 11.0 + Vec3::Y * aim.length() * 0.45;
                    ev.push(MobEvent::ThrowPotion(eye + aim.normalize_or_zero() * 0.4, vel, p));
                    m.attack_cd = rng.range(2.5, 3.5);
                }
            }
        }
        MobKind::GlowSquid => {
            if m.body.in_water {
                // Drift, turning back from the edges of the water; flee when hurt.
                let ahead = m.body.pos + Vec3::new(m.yaw.sin(), 0.3, -m.yaw.cos()) * 0.9;
                if !is_wet(world.get(ahead.x.floor() as i32, ahead.y.floor() as i32, ahead.z.floor() as i32)) {
                    m.wander_dir = Some(m.yaw + std::f32::consts::PI + rng.range(-0.8, 0.8));
                    m.wander_t = rng.range(1.0, 3.0);
                }
                if m.flee > 0.0 {
                    *want = Some(((-flat.x).atan2(flat.z), 3.0));
                }
                let above = world.get(m.body.pos.x.floor() as i32, (m.body.pos.y + 1.0).floor() as i32, m.body.pos.z.floor() as i32);
                let drift = (m.wander_t * 1.1 + m.id as f32).sin() * 0.5;
                *swim_vy = Some(if is_water(above) { drift } else { drift.min(0.0) - 0.3 });
                // Ink, now and then while it's fleeing.
                if m.flee > 0.0 && m.attack_cd <= 0.0 {
                    ev.push(MobEvent::Ink(m.body.pos + Vec3::Y * 0.5));
                    m.attack_cd = 3.0;
                }
            } else {
                *may_wander = false;
                m.health -= dt * 0.5;
            }
        }
        MobKind::Bat => {
            *may_wander = false;
            let head = (m.body.pos + Vec3::Y * (m.body.height + 0.05)).floor().as_ivec3();
            if m.sitting {
                // Hanging from the ceiling until someone comes near (or the ceiling goes).
                m.body.vel = Vec3::ZERO;
                *fly_vy = Some(0.0);
                if (visible && to.length() < 4.0) || !is_solid(world.get_v(head)) || rng.chance(dt * 0.05) {
                    m.sitting = false;
                }
                return;
            }
            // Flutter: a new direction every so often, up and down at random.
            m.wander_t -= dt;
            if m.wander_t <= 0.0 {
                m.yaw = rng.range(0.0, std::f32::consts::TAU);
                m.temper = rng.int(0, 2) as u8;
                m.wander_t = rng.range(0.3, 1.2);
            }
            *want = Some((m.yaw, 3.5));
            *fly_vy = Some([-2.0, 0.6, 2.4][m.temper as usize % 3]);
            // A rest, upside down, if there's a ceiling just above.
            if is_solid(world.get_v(head)) && rng.chance(dt * 0.3) {
                m.sitting = true;
            }
        }
        MobKind::Allay => {
            *may_wander = false;
            // Fly to its goal (something to fetch, or whoever gave it the item), else hover nearby.
            let goal = m.goal.unwrap_or(m.home.unwrap_or(m.body.pos));
            let d = goal - m.body.pos;
            let flat = Vec3::new(d.x, 0.0, d.z);
            if flat.length() > 0.8 {
                *want = Some((flat.x.atan2(-flat.z), if flat.length() > 6.0 { 5.0 } else { 3.0 }));
            }
            let bob = (m.wander_t * 2.0 + m.id as f32).sin() * 0.3;
            m.wander_t += dt;
            *fly_vy = Some((d.y * 2.0 + bob).clamp(-3.0, 3.0));
        }
        _ => {}
    }
}

impl Game {
    /// A Witch's splash potion: a bottle that bursts where it lands, on everyone near.
    pub fn throw_witch_potion(&mut self, from: Vec3, vel: Vec3, p: Potion) {
        let mut a = crate::entity::Arrow::new(from, vel, None, 0.0);
        a.appearance = ProjectileAppearance { model: ProjectileModel::Billboard, tile: Some(item_tile(crate::potions::potion_item(Potion::Healing, true))), scale: 0.6 };
        a.effect = Some(ProjectileEffect { kind: p, duration: if p == Potion::Poison { 12.0 } else { 30.0 }, amplifier: 0 });
        a.splash = true;
        self.arrows.push(a);
        self.sfx(crate::sound::Sfx::Hmm, Some(from));
    }

    /// A splash potion (or a Witch's) bursts at `at`: glass, a puff, and the effect on everyone near.
    pub fn potion_splash(&mut self, at: Vec3, effect: Option<ProjectileEffect>) {
        self.sfx(crate::sound::Sfx::Break(crate::sound::Mat::Glass), Some(at));
        self.smoke(at, 12, 0.6);
        let Some(e) = effect else { return };
        let r = crate::potions::SPLASH_RADIUS * 0.75;
        let me = self.player.body.pos + Vec3::Y * 0.9;
        if !self.dedicated && !self.spectator && self.dead.is_none() && !self.creative && me.distance(at) <= r {
            self.timed_effect_amplified(e.kind, e.duration, e.amplifier);
        }
        let hit: Vec<u32> = self.peers.iter().filter(|(_, p)| p.alive() && (p.target + Vec3::Y * 0.9).distance(at) <= r).map(|(&id, _)| id).collect();
        for id in hit {
            self.send_timed_effect(id, e.kind, e.duration, e.amplifier);
        }
    }

    /// A Glow Squid's ink: a dark, glinting cloud.
    pub fn ink_cloud(&mut self, at: Vec3) {
        if self.dedicated {
            return;
        }
        for _ in 0..14 {
            let r = &mut self.rng;
            self.particles.push(crate::entity::Particle {
                pos: at + Vec3::new(r.range(-0.4, 0.4), r.range(-0.3, 0.3), r.range(-0.4, 0.4)),
                vel: Vec3::new(r.range(-1.2, 1.2), r.range(-0.4, 0.6), r.range(-1.2, 1.2)),
                life: r.range(1.0, 2.2),
                tile: crate::texture::T_GLOW_SQUID,
                uv: [0.2, 0.2],
                size: r.range(0.2, 0.45),
                gravity: 0.0,
            });
        }
    }

    /// Right-clicking an Allay: give it the held item (it fetches more of
    /// them for `who`), or, empty-handed, take its item back.
    pub fn allay_interact(&mut self, who: &str, at: Vec3, i: usize, item: Id) -> crate::animals::Interaction {
        use crate::animals::Interaction;
        let m = &mut self.mobs[i];
        if item != AIR && m.seed == 0 {
            m.seed = item as u32;
            m.temper = 0;
            m.owner = Some(who.to_string());
            m.persistent = true;
            let pos = m.body.pos;
            self.sfx(crate::sound::Sfx::Chime, Some(pos));
            if !self.dedicated && who == crate::players::record_key(&self.player_name) {
                self.advance("fetch");
            }
            return Interaction::Ate;
        }
        if item == AIR && m.seed != 0 {
            let (held, n) = (m.seed as Id, 1 + m.temper);
            m.seed = 0;
            m.temper = 0;
            m.owner = None;
            self.pop_drop(at + Vec3::Y * 0.5, held, n);
            return Interaction::Toggled;
        }
        Interaction::Nothing
    }

    /// Allays fetch: off to the nearest matching item lying about, pick it
    /// up, and back to drop it at their person's feet. (Where the world lives.)
    pub fn allays_tick(&mut self) {
        if self.is_client() {
            return;
        }
        let mut people: Vec<(String, Vec3)> = self.peers.values().filter(|p| p.alive()).map(|p| (crate::players::record_key(&p.name), p.target)).collect();
        if !self.dedicated && self.dead.is_none() {
            people.push((crate::players::record_key(&self.player_name), self.player.body.pos));
        }
        for i in 0..self.mobs.len() {
            if self.mobs[i].kind != MobKind::Allay {
                continue;
            }
            let m = &self.mobs[i];
            let (item, pos) = (m.seed as Id, m.body.pos);
            let owner = m.owner.as_ref().and_then(|o| people.iter().find(|(k, _)| k == o).map(|(_, p)| *p));
            let Some(owner) = owner.filter(|_| item != AIR) else {
                self.mobs[i].goal = None;
                continue;
            };
            let carrying = self.mobs[i].temper;
            // Full (or nothing left to fetch): back to its person, and drop the lot there.
            let target = (carrying < ALLAY_CARRY)
                .then(|| {
                    self.drops.iter().enumerate().filter(|(_, d)| d.item == item && d.body.pos.distance(pos) < FETCH_RANGE && d.body.pos.distance(owner) < LEASH && d.body.pos.distance(owner) > 2.5).min_by(|a, b| a.1.body.pos.distance(pos).total_cmp(&b.1.body.pos.distance(pos)))
                })
                .flatten()
                .map(|(j, d)| (j, d.body.pos));
            match target {
                Some((j, at)) if at.distance(pos) < 1.2 => {
                    let take = self.drops[j].n.min(ALLAY_CARRY - carrying);
                    self.drops[j].n -= take;
                    if self.drops[j].n == 0 {
                        self.drops.remove(j);
                    }
                    self.mobs[i].temper += take;
                    self.sfx(crate::sound::Sfx::Pop, Some(at));
                }
                Some((_, at)) => self.mobs[i].goal = Some(at + Vec3::Y * 0.3),
                None => {
                    self.mobs[i].goal = Some(owner + Vec3::Y * 1.6);
                    if carrying > 0 && pos.distance(owner + Vec3::Y * 1.6) < 2.5 {
                        self.mobs[i].temper = 0;
                        self.pop_drop(owner + Vec3::Y * 0.5, item, carrying);
                        self.sfx(crate::sound::Sfx::Chime, Some(pos));
                    }
                }
            }
        }
    }

    /// Night threats, Bats and Glow Squid turning up (from `try_spawn`):
    /// true if something did.
    pub fn spawn_night_extras(&mut self, x: i32, z: i32, surface: i32) -> bool {
        // Bats in dark caves.
        if self.rng.chance(0.2) {
            let bats = self.mobs.iter().filter(|m| m.kind == MobKind::Bat).count();
            let cy = self.rng.int(8, (surface - 8).max(9));
            let dark = self.world.sky_light(x, cy, z) < 0.1 && self.world.block_level(x, cy, z) < 4;
            if bats < 6 && dark && (0..2).all(|h| self.world.get(x, cy + h, z) == AIR) {
                for k in 0..self.rng.int(1, 2) {
                    self.alloc_mob(MobKind::Bat, Vec3::new(x as f32 + 0.5 + k as f32 * 0.5, cy as f32 + 0.3, z as f32 + 0.5));
                }
                return true;
            }
        }
        // Glow Squid in dark water: underground, or the sea at night.
        if self.rng.chance(0.15) {
            let squid = self.mobs.iter().filter(|m| m.kind == MobKind::GlowSquid).count();
            let cy = self.rng.int(6, surface.max(7));
            let water = (0..2).all(|h| is_water(self.world.get(x, cy + h, z)));
            let dark = self.world.sky_light(x, cy, z) < 0.15 || (self.is_night() && cy < crate::world::SEA - 3);
            if squid < 4 && water && dark {
                self.alloc_mob(MobKind::GlowSquid, Vec3::new(x as f32 + 0.5, cy as f32 + 0.2, z as f32 + 0.5));
                return true;
            }
        }
        false
    }
}

/// Which night monster turns up in a biome, swapping in the local kinds:
/// desert Groaners, snowy Rattlers, swamp Witches.
pub fn local_kind(kind: MobKind, biome: crate::world::Biome, rng: &mut Rng) -> MobKind {
    use crate::world::Biome;
    match (kind, biome) {
        (MobKind::Groaner, Biome::Desert) if rng.chance(0.8) => MobKind::DesertGroaner,
        (MobKind::Rattler, Biome::Snowy) if rng.chance(0.8) => MobKind::SnowyRattler,
        (MobKind::Hisser | MobKind::Webber, Biome::Swamp) if rng.chance(0.3) => MobKind::Witch,
        _ => kind,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locals_turn_up_in_their_biomes() {
        use crate::world::Biome;
        let mut rng = Rng::new(5);
        let mut seen = Vec::new();
        for _ in 0..200 {
            seen.push(local_kind(MobKind::Groaner, Biome::Desert, &mut rng));
            seen.push(local_kind(MobKind::Rattler, Biome::Snowy, &mut rng));
            seen.push(local_kind(MobKind::Webber, Biome::Swamp, &mut rng));
        }
        assert!(seen.contains(&MobKind::DesertGroaner) && seen.contains(&MobKind::SnowyRattler) && seen.contains(&MobKind::Witch));
        assert_eq!(local_kind(MobKind::Groaner, Biome::Plains, &mut rng), MobKind::Groaner);
    }

    /// A walled arena: everything to fight in.
    fn pen(seed: u32) -> Game {
        let mut g = crate::game::tests::arena(seed);
        g.player.body.pos = Vec3::new(8.5, 50.0, 0.5);
        g
    }

    #[test]
    fn witches_throw_and_drink() {
        let mut g = pen(101);
        g.time = 0.75;
        let id = g.alloc_mob(MobKind::Witch, Vec3::new(0.5, 50.0, 0.5));
        for _ in 0..200 {
            g.update_entities(0.05);
            g.update_arrows(0.05);
            g.effects_tick(0.05);
        }
        let afflicted = [Potion::Slowness, Potion::Poison, Potion::Weakness].iter().any(|&p| g.has_effect(p));
        assert!(afflicted, "a potion landed");
        // Hurt badly, it drinks.
        let w = g.mobs.iter_mut().find(|m| m.id == id).unwrap();
        w.health = 5.0;
        w.warp_cd = 0.0;
        for _ in 0..4 {
            g.update_entities(0.05);
        }
        assert!(g.mobs.iter().find(|m| m.id == id).unwrap().health > 10.0, "it drank");
    }

    #[test]
    fn snowy_rattlers_slow_and_desert_groaners_starve() {
        let mut g = pen(102);
        g.time = 0.75;
        g.alloc_mob(MobKind::SnowyRattler, Vec3::new(-2.5, 50.0, 0.5));
        for _ in 0..300 {
            g.update_entities(0.05);
            g.update_arrows(0.05);
            if g.has_effect(Potion::Slowness) {
                break;
            }
        }
        assert!(g.has_effect(Potion::Slowness), "an arrow slowed us");
        let mut g = pen(103);
        g.time = 0.75;
        g.alloc_mob(MobKind::DesertGroaner, Vec3::new(7.5, 50.0, 0.5));
        for _ in 0..200 {
            g.update_entities(0.05);
            if g.has_effect(Potion::Hunger) {
                break;
            }
        }
        assert!(g.has_effect(Potion::Hunger), "bitten, and hungry");
        // And they don't burn by day.
        g.time = 0.25;
        let d = g.mobs.iter().find(|m| m.kind == MobKind::DesertGroaner).map(|m| m.id).unwrap();
        for _ in 0..40 {
            g.update_entities(0.05);
        }
        assert!(g.mobs.iter().find(|m| m.id == d).is_some_and(|m| !m.burning));
    }

    #[test]
    fn allays_fetch_what_you_give_them() {
        let mut g = pen(104);
        let me = crate::players::record_key(&g.player_name);
        let id = g.alloc_mob(MobKind::Allay, Vec3::new(4.5, 51.0, 0.5));
        let i = g.mobs.iter().position(|m| m.id == id).unwrap();
        let at = g.player.body.pos;
        assert_eq!(g.allay_interact(&me, at, i, DIAMOND), crate::animals::Interaction::Ate);
        // Some diamonds lying about, out of reach.
        g.pop_drop(Vec3::new(-8.5, 50.5, 4.5), DIAMOND, 3);
        g.pop_drop(Vec3::new(-8.5, 50.5, -4.5), COAL, 3);
        let near = |g: &Game, item: Id| g.drops.iter().filter(|d| d.item == item && d.body.pos.distance(g.player.body.pos) < 3.0).map(|d| d.n as u32).sum::<u32>();
        for _ in 0..600 {
            g.update_entities(0.05);
            g.allays_tick();
            if near(&g, DIAMOND) >= 3 {
                break;
            }
        }
        assert!(near(&g, DIAMOND) >= 3, "the diamonds were fetched");
        assert_eq!(near(&g, COAL), 0, "only what matches");
        // Empty-handed, take the item back.
        let i = g.mobs.iter().position(|m| m.id == id).unwrap();
        assert_eq!(g.allay_interact(&me, at, i, AIR), crate::animals::Interaction::Toggled);
        assert_eq!(g.mobs[i].seed, 0);
    }
}
