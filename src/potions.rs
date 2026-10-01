//! Brewing and potions.
//!
//! - **Glass Bottles** (three glass make three) fill up at water with a
//!   right-click.
//! - A **Brewing Stand** (three cobblestone and a Grumbler Tusk, so a trip to
//!   the Scorchlands first) works like a furnace without fuel: a Water Bottle
//!   on the right and an ingredient on top become a potion in ten seconds.
//!   - Glowshroom: **Healing** (four hearts, at once)
//!   - Zappy Dust: **Speed**
//!   - Ember Shroom (from the Scorchlands): **Fire Resistance** (lava and fire don't hurt)
//!   - Carrot: **Night Vision**
//!   - Feather: **Leaping** (jump higher, fall softer)
//! - Brew a potion with Hisspowder for the **splash** version: right-click
//!   to throw it, and it bursts where you aim, on everyone (and everything)
//!   nearby.
//! - Drinking takes the potion and leaves the bottle. Effects last three
//!   minutes and show on screen.
//!
//! Stands brew where the world lives (like furnaces). Joined players drink
//! on their own; a splash is burst by the host, which checks they had it.

use crate::block::*;
use crate::game::Game;
use crate::net::Msg;
use crate::sound::Sfx;
use macroquad::math::Vec3;

/// Seconds to brew, how long effects last, and how far a splash reaches.
pub const BREW_SECS: f32 = 10.0;
pub const EFFECT_SECS: f32 = 180.0;
pub const SPLASH_RADIUS: f32 = 4.0;
/// How far a splash potion flies.
pub const THROW_RANGE: f32 = 12.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Potion {
    Healing,
    Speed,
    FireResistance,
    NightVision,
    Leaping,
}

pub const ALL: [Potion; 5] = [Potion::Healing, Potion::Speed, Potion::FireResistance, Potion::NightVision, Potion::Leaping];

impl Potion {
    pub fn index(self) -> usize {
        ALL.iter().position(|&p| p == self).unwrap_or(0)
    }
    pub fn name(self) -> &'static str {
        match self {
            Potion::Healing => "Healing",
            Potion::Speed => "Speed",
            Potion::FireResistance => "Fire Resistance",
            Potion::NightVision => "Night Vision",
            Potion::Leaping => "Leaping",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Potion::Healing => "healing",
            Potion::Speed => "speed",
            Potion::FireResistance => "fire_resistance",
            Potion::NightVision => "night_vision",
            Potion::Leaping => "leaping",
        }
    }
    pub fn colour(self) -> [u8; 3] {
        match self {
            Potion::Healing => [230, 60, 90],
            Potion::Speed => [120, 200, 255],
            Potion::FireResistance => [240, 150, 40],
            Potion::NightVision => [40, 60, 200],
            Potion::Leaping => [120, 230, 90],
        }
    }
    /// What goes in to make it.
    pub fn ingredient(self) -> Id {
        match self {
            Potion::Healing => GLOWSHROOM,
            Potion::Speed => ZAP_DUST,
            Potion::FireResistance => EMBER_SHROOM,
            Potion::NightVision => CARROT,
            Potion::Leaping => FEATHER,
        }
    }
    /// Over at once (healing) rather than lasting.
    pub fn instant(self) -> bool {
        self == Potion::Healing
    }
}

/// A potion item: which, and whether it's a splash one.
pub fn potion_of(id: Id) -> Option<(Potion, bool)> {
    if (POTION_FIRST..POTION_FIRST + 5).contains(&id) {
        Some((ALL[(id - POTION_FIRST) as usize], false))
    } else if (SPLASH_FIRST..SPLASH_FIRST + 5).contains(&id) {
        Some((ALL[(id - SPLASH_FIRST) as usize], true))
    } else {
        None
    }
}

pub fn potion_item(p: Potion, splash: bool) -> Id {
    (if splash { SPLASH_FIRST } else { POTION_FIRST }) + p.index() as Id
}

/// What brewing `ingredient` into `bottle` makes.
pub fn brew(bottle: Id, ingredient: Id) -> Option<Id> {
    if bottle == WATER_BOTTLE {
        return ALL.iter().find(|p| p.ingredient() == ingredient).map(|&p| potion_item(p, false));
    }
    match potion_of(bottle) {
        Some((p, false)) if ingredient == GUNPOWDER => Some(potion_item(p, true)),
        _ => None,
    }
}

/// Could this go in a brewing stand's bottle slot?
pub fn is_bottle(id: Id) -> bool {
    id == WATER_BOTTLE || potion_of(id).is_some_and(|(_, splash)| !splash)
}

impl Game {
    /// An effect starts (or starts over); healing just heals.
    pub fn apply_potion(&mut self, p: Potion) {
        if p.instant() {
            self.player.health = (self.player.health + 8.0).min(20.0);
            return;
        }
        self.effects.retain(|e| e.0 != p);
        self.effects.push((p, EFFECT_SECS));
        self.msg(format!("You feel... {}.", p.name().to_lowercase()));
    }

    /// An effect for `secs` (from food rather than a potion), quietly.
    pub fn timed_effect(&mut self, p: Potion, secs: f32) {
        self.effects.retain(|e| e.0 != p);
        self.effects.push((p, secs));
    }

    pub fn has_effect(&self, p: Potion) -> bool {
        self.effects.iter().any(|e| e.0 == p)
    }

    /// Effects wear off.
    pub fn effects_tick(&mut self, dt: f32) {
        for e in self.effects.iter_mut() {
            e.1 -= dt;
        }
        let ended: Vec<Potion> = self.effects.iter().filter(|e| e.1 <= 0.0).map(|e| e.0).collect();
        self.effects.retain(|e| e.1 > 0.0);
        for p in ended {
            self.msg(format!("{} wore off.", p.name()));
        }
        // What the effects do to the player's body.
        self.player.speed_boost = if self.has_effect(Potion::Speed) { 1.35 } else { 1.0 };
        self.player.leaping = self.has_effect(Potion::Leaping);
    }

    /// Right-click with a potion: drink it, or throw it if it's a splash one.
    /// Returns whether it did anything.
    pub fn use_potion(&mut self, held: Id) -> bool {
        if held == GLASS_BOTTLE {
            return self.fill_bottle();
        }
        let Some((p, splash)) = potion_of(held) else { return false };
        self.player.swing = 1.0;
        if splash {
            let eye = self.player.eye();
            let dir = self.player.look_dir();
            let at = match self.world.raycast(eye, dir, THROW_RANGE) {
                Some(h) => h.pos.as_vec3() + Vec3::splat(0.5) + h.normal.as_vec3() * 0.6,
                None => eye + dir * THROW_RANGE,
            };
            self.use_up_held();
            if self.is_client() {
                self.net_send_msg(Msg::Splash { item: held, at });
            } else {
                self.burst(p, at);
            }
            self.sfx(Sfx::Break(crate::sound::Mat::Glass), Some(at));
            return true;
        }
        if !self.creative {
            // Joined players' hosts hand the bottle back themselves (see `host_consume`).
            self.use_up_held();
            self.inv.add(GLASS_BOTTLE, 1);
        }
        self.sfx(Sfx::Eat, None);
        self.apply_potion(p);
        self.advance("brewmaster");
        true
    }

    /// A glass bottle dipped in water (within reach).
    fn fill_bottle(&mut self) -> bool {
        let eye = self.player.eye();
        let dir = self.player.look_dir();
        let Some(h) = self.world.raycast_liquid(eye, dir, 5.0) else { return false };
        if !is_water(self.world.get_v(h.pos)) {
            return false;
        }
        if !self.creative {
            // The host swaps them in its ledger when it hears about it (`host_fill_bottle`).
            self.inv.consume_held();
        }
        self.inv.add(WATER_BOTTLE, 1);
        self.sfx(Sfx::Splash, Some(h.pos.as_vec3()));
        self.player.swing = 1.0;
        true
    }

    /// Where the world lives: a splash potion bursts at `at`.
    pub fn burst(&mut self, p: Potion, at: Vec3) {
        self.smoke(at, 16, 0.5);
        if !self.dedicated && self.player.body.pos.distance(at) < SPLASH_RADIUS {
            self.apply_potion(p);
        }
        let hit: Vec<u32> = self.peers.iter().filter(|(_, q)| q.target.distance(at) < SPLASH_RADIUS).map(|(&id, _)| id).collect();
        for id in hit {
            self.net_send_to(id, Msg::PotionEffect { item: potion_item(p, false) });
        }
        if p == Potion::Healing {
            for m in self.mobs.iter_mut().filter(|m| m.body.pos.distance(at) < SPLASH_RADIUS) {
                m.health = (m.health + 8.0).min(m.kind.max_health());
            }
        }
    }

    /// A joined player threw a splash potion.
    pub fn host_splash(&mut self, from: u32, item: Id, at: Vec3) {
        let Some((p, true)) = potion_of(item) else { return };
        let Some(q) = self.peers.get(&from) else { return };
        let fair = at.is_finite() && at.distance(q.target + Vec3::Y * 1.6) <= THROW_RANGE + 1.0;
        if fair && self.peer_rate_ok(from, "splash", 0.3) && self.peer_take(from, item, 1) {
            self.burst(p, at);
        }
    }

    /// A joined player filled a bottle: check there's water near them.
    pub fn host_fill_bottle(&mut self, from: u32) {
        let Some(q) = self.peers.get(&from) else { return };
        let c = (q.target + Vec3::Y).floor().as_ivec3();
        let near_water = (-5..=5).any(|dy| (-5..=5).any(|dz| (-5..=5).any(|dx| is_water(self.world.get_v(c + macroquad::math::ivec3(dx, dy, dz))))));
        if near_water && self.peer_take(from, GLASS_BOTTLE, 1) {
            if let Some(l) = self.ledger(from) {
                l.bag.add(WATER_BOTTLE, 1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recipes() {
        assert_eq!(brew(WATER_BOTTLE, GLOWSHROOM), Some(potion_item(Potion::Healing, false)));
        assert_eq!(brew(potion_item(Potion::Speed, false), GUNPOWDER), Some(potion_item(Potion::Speed, true)));
        assert_eq!(brew(potion_item(Potion::Speed, true), GUNPOWDER), None);
        assert_eq!(brew(WATER_BOTTLE, DIAMOND), None);
        for p in ALL {
            assert_eq!(potion_of(potion_item(p, true)), Some((p, true)));
        }
    }

    #[test]
    fn a_brewing_stand_brews() {
        let mut c = crate::containers::Container::for_block(BREWING_STAND);
        c.slots[crate::containers::INPUT] = Some((EMBER_SHROOM, 2));
        c.slots[crate::containers::FUEL] = Some((WATER_BOTTLE, 1));
        for _ in 0..(BREW_SECS * 10.0) as i32 + 2 {
            c.brew_tick(0.1);
        }
        assert_eq!(c.slots[crate::containers::OUTPUT], Some((potion_item(Potion::FireResistance, false), 1)));
        assert_eq!(c.slots[crate::containers::INPUT], Some((EMBER_SHROOM, 1)));
        assert_eq!(c.slots[crate::containers::FUEL], None);
    }

    #[test]
    fn drinking_and_splashing() {
        let mut g = crate::game::tests::arena(61);
        g.player.health = 5.0;
        g.inv.slots[g.inv.selected] = Some((potion_item(Potion::Healing, false), 1));
        assert!(g.use_potion(potion_item(Potion::Healing, false)));
        assert_eq!(g.player.health, 13.0);
        assert_eq!(g.inv.count(GLASS_BOTTLE), 1);
        g.burst(Potion::Speed, g.player.body.pos);
        assert!(g.has_effect(Potion::Speed));
        g.effects_tick(0.1);
        assert!(g.player.speed_boost > 1.0);
        g.effects_tick(EFFECT_SECS);
        assert!(!g.has_effect(Potion::Speed));
    }
}
