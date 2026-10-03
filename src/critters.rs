//! Smaller wildlife with habits of its own.
//!
//! - **Sneakers** (foxes) live in taiga and snowy forests. They nap through
//!   the day, hunt Clucksters at night, and pick up anything left lying
//!   about (they drop it if they're defeated). Wild ones keep their
//!   distance; feed one Cluckets and it trusts you: it follows you around,
//!   and brings you whatever it finds.
//! - **Ribbits** (frogs) hop about swamps and swim well. A Ribbit that
//!   catches a small Bloop with its tongue leaves a **Froglight**, its colour
//!   set by the climate: Ochre where it's mild, Verdant where it's cold,
//!   Pearlescent where it's hot. Feed them Groaner Goo to breed them, and
//!   take the babies travelling.
//! - **Rollos** (armadillos) potter about deserts and badlands. Get close
//!   (or hit one) and it rolls into an armoured ball that shrugs off three
//!   quarters of every hit. They shed **scutes** now and then, and give one
//!   up when brushed (a Brush, from archaeology; once every few minutes).
//!   Six scutes make **Woofer Armour**: right-click your tame Woofer with it
//!   and it soaks up hits until it wears through (shears take it off again).
//! - **Squawkers** (parrots) were already here: feed them seeds.

use crate::block::*;
use crate::entity::{Mob, MobKind};
use crate::game::Game;
use crate::sound::Sfx;
use crate::world::Biome;
use macroquad::math::Vec3;

/// Hit points of a fresh set of Woofer armour.
pub const WOLF_ARMOUR_HP: u8 = 64;
/// Seconds between scutes from brushing the same Rollo.
pub const BRUSH_COOLDOWN: f32 = 300.0;

/// Woofer armour takes the hit first (its hit points live in `temper`). Returns what gets through.
pub fn armour_soak(m: &mut Mob, amount: f32) -> f32 {
    let hp = m.temper as f32;
    let soaked = amount.min(hp);
    m.temper = (hp - soaked).round().max(0.0) as u8;
    if m.temper == 0 {
        m.saddled = false;
    }
    amount - soaked
}

/// Which Froglight a Ribbit makes, by the climate where it ate.
pub fn froglight_for(b: Biome) -> Id {
    match b {
        Biome::Snowy | Biome::Taiga => FROGLIGHT_FIRST + 1,
        Biome::Desert | Biome::Jungle | Biome::Badlands => FROGLIGHT_FIRST + 2,
        _ => FROGLIGHT_FIRST,
    }
}

impl Game {
    /// Where the world lives, after `animals_tick`: hunting, thieving, tongues and scutes.
    pub fn critters_tick(&mut self, dt: f32) {
        if self.is_client() {
            return;
        }
        let night = self.is_night();
        let players = self.player_spots();
        let n = self.mobs.len();
        let mut kills: Vec<(usize, usize)> = Vec::new();
        let mut eaten: Vec<(usize, usize)> = Vec::new();
        let mut grabs: Vec<(usize, usize)> = Vec::new();
        let mut gifts: Vec<(usize, Vec3)> = Vec::new();
        let mut scutes: Vec<Vec3> = Vec::new();
        let mut nips: Vec<(usize, usize)> = Vec::new();
        for i in 0..n {
            let (kind, pos) = (self.mobs[i].kind, self.mobs[i].body.pos);
            match kind {
                MobKind::Sneaker => {
                    let m = &self.mobs[i];
                    // A trusting Sneaker with something in its mouth brings it to you.
                    if m.seed != 0
                        && let Some(owner) = &m.owner
                        && let Some(at) = players.iter().find(|p| &p.0 == owner).map(|p| p.1)
                        && at.distance(pos) < 2.2
                    {
                        gifts.push((i, at));
                        continue;
                    }
                    // Anything on the ground nearby is fair game (when its mouth is free).
                    if m.seed == 0
                        && m.goal.is_none()
                        && let Some((d, _)) = self.drops.iter().enumerate().filter(|(_, d)| d.age > d.delay + 2.0 && d.body.pos.distance(pos) < 8.0).min_by(|a, b| a.1.body.pos.distance(pos).total_cmp(&b.1.body.pos.distance(pos)))
                    {
                        let at = self.drops[d].body.pos;
                        if at.distance(pos) < 1.3 {
                            grabs.push((i, d));
                        } else {
                            self.mobs[i].goal = Some(at);
                        }
                        continue;
                    }
                    // At night: Cluckster hunting.
                    if night && self.mobs[i].goal.is_none() {
                        let prey = self.mobs.iter().enumerate().filter(|(_, c)| c.kind == MobKind::Cluckster && c.health > 0.0 && c.body.pos.distance(pos) < 12.0).min_by(|a, b| a.1.body.pos.distance(pos).total_cmp(&b.1.body.pos.distance(pos))).map(|(j, c)| (j, c.id, c.body.pos));
                        if let Some((j, id, at)) = prey {
                            self.mobs[i].prey = Some(id);
                            self.mobs[i].goal = Some(at);
                            if at.distance(pos) < 1.3 && self.mobs[i].attack_cd <= 0.0 {
                                kills.push((i, j));
                            }
                        } else {
                            self.mobs[i].prey = None;
                        }
                    }
                }
                MobKind::Ribbit => {
                    // Small Bloops look delicious.
                    let food = self.mobs.iter().enumerate().filter(|(_, b)| b.kind == MobKind::Bloop && b.size <= 1.0 && b.health > 0.0 && b.body.pos.distance(pos) < 8.0).min_by(|a, b| a.1.body.pos.distance(pos).total_cmp(&b.1.body.pos.distance(pos))).map(|(j, b)| (j, b.body.pos));
                    if let Some((j, at)) = food {
                        if self.mobs[i].goal.is_none() {
                            self.mobs[i].goal = Some(at);
                        }
                        if at.distance(pos) < 2.6 && self.mobs[i].attack_cd <= 0.0 {
                            eaten.push((i, j));
                        }
                    }
                }
                MobKind::Axolotl if self.mobs[i].body.in_water && self.mobs[i].baby <= 0.0 => {
                    // Hunts Soggy Groaners (and fish) in the water: help it out and it helps you.
                    let prey = self.mobs.iter().enumerate().filter(|(_, c)| matches!(c.kind, MobKind::Soggy | MobKind::Fishy) && c.health > 0.0 && c.body.in_water && c.body.pos.distance(pos) < 10.0).min_by(|a, b| a.1.body.pos.distance(pos).total_cmp(&b.1.body.pos.distance(pos))).map(|(j, c)| (j, c.id, c.body.pos + Vec3::Y * c.body.height * 0.4));
                    match prey {
                        Some((j, id, at)) => {
                            self.mobs[i].prey = Some(id);
                            self.mobs[i].goal = Some(at);
                            if at.distance(pos) < 1.6 && self.mobs[i].attack_cd <= 0.0 {
                                nips.push((i, j));
                            }
                        }
                        None => self.mobs[i].prey = None,
                    }
                }
                MobKind::Rollo => {
                    let m = &mut self.mobs[i];
                    m.warp_cd = (m.warp_cd - dt).max(0.0);
                    m.restock -= dt;
                    if m.restock <= 0.0 {
                        m.restock = self.rng.range(400.0, 800.0);
                        scutes.push(pos);
                    }
                }
                _ => {}
            }
        }
        for (i, j) in kills {
            let at = self.mobs[i].body.pos;
            self.mobs[i].attack_cd = 1.0;
            self.mobs[j].damage(3.0, at);
            self.sfx(Sfx::Yip, Some(at));
        }
        for (i, j) in eaten {
            let at = self.mobs[j].body.pos;
            self.mobs[i].attack_cd = 3.0;
            self.mobs[i].goal = None;
            // Gone in one gulp (no Goo, no experience).
            self.mobs[j].health = -100.0;
            let (_, biome) = self.world.generator.column(at.x.floor() as i32, at.z.floor() as i32);
            self.pop_drop(at + Vec3::Y * 0.3, froglight_for(biome), 1);
            self.sfx(Sfx::Croak, Some(at));
            if players.iter().any(|p| p.1.distance(at) < 16.0) {
                self.advance("tongue_tied");
            }
        }
        // Grab in reverse order so indices stay valid.
        grabs.sort_by(|a, b| b.1.cmp(&a.1));
        for (i, d) in grabs {
            if d >= self.drops.len() || self.mobs[i].seed != 0 {
                continue;
            }
            let item = self.drops[d].item;
            if self.drops[d].n > 1 {
                self.drops[d].n -= 1;
            } else {
                self.drops.remove(d);
            }
            self.mobs[i].seed = item as u32;
            self.mobs[i].goal = None;
            self.sfx(Sfx::Yip, Some(self.mobs[i].body.pos));
        }
        for (i, at) in gifts {
            let item = std::mem::take(&mut self.mobs[i].seed) as Id;
            let from = self.mobs[i].body.pos + Vec3::Y * 0.5;
            self.fling_to(from, at, item, 1);
            self.hearts(from, 2);
            self.advance("special_delivery");
        }
        for at in scutes {
            self.pop_drop(at + Vec3::Y * 0.3, SCUTE, 1);
        }
        for (i, j) in nips {
            let at = self.mobs[i].body.pos;
            self.mobs[i].attack_cd = 1.0;
            let was = self.mobs[j].health;
            self.mobs[j].damage(2.0, at);
            // A win: everyone fighting alongside is patched up a bit.
            if was > 0.0 && self.mobs[j].health <= 0.0 {
                self.axolotl_win(at);
            }
        }
    }
}

impl Game {
    /// An Axolotl's prey is beaten: players nearby get a little Regeneration.
    pub fn axolotl_win(&mut self, at: Vec3) {
        if !self.dedicated && self.dead.is_none() && self.player.body.pos.distance(at) < 12.0 {
            self.timed_effect(crate::potions::Potion::Regeneration, 6.0);
        }
        let near: Vec<u32> = self.peers.iter().filter(|(_, p)| p.alive() && p.target.distance(at) < 12.0).map(|(&id, _)| id).collect();
        for id in near {
            self.send_timed_effect(id, crate::potions::Potion::Regeneration, 6.0, 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::noise::Rng;

    #[test]
    fn woofer_armour_soaks_hits() {
        let mut rng = Rng::new(1);
        let mut w = Mob::new(MobKind::Woofer, Vec3::ZERO, &mut rng);
        w.saddled = true;
        w.temper = 10;
        assert_eq!(armour_soak(&mut w, 4.0), 0.0);
        assert_eq!(w.temper, 6);
        assert_eq!(armour_soak(&mut w, 10.0), 4.0, "worn through: the rest gets through");
        assert!(!w.saddled);
        assert_eq!(froglight_for(Biome::Snowy), FROGLIGHT_FIRST + 1);
        assert_eq!(froglight_for(Biome::Swamp), FROGLIGHT_FIRST);
    }

    #[test]
    fn rollos_curl_up_and_shrug_off_hits() {
        let mut rng = Rng::new(2);
        let mut r = Mob::new(MobKind::Rollo, Vec3::ZERO, &mut rng);
        r.damage(4.0, Vec3::X);
        assert!(r.fuse > 0.0, "curled up");
        let h = r.health;
        r.hurt = 0.0;
        r.damage(4.0, Vec3::X);
        assert!((h - r.health - 1.0).abs() < 1e-4, "a quarter gets through");
    }
}
