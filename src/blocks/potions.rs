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
    /// Sizzle Powder (see fortress.rs): hit harder.
    Strength,
    /// A Weeper's Tear: health creeps back.
    Regeneration,
    /// Not a potion: what a patrol captain leaves you with (see raids.rs).
    BadOmen,
    /// Not a potion: a village's thanks for seeing off a raid (better prices).
    Hero,
    /// Not a potion: an Elder Guardian's curse (digging is slow; see monument.rs).
    MiningFatigue,
    /// Not a potion: a working Conduit nearby (you can breathe underwater).
    ConduitPower,
    /// From Witches and snowy Rattlers' arrows: you walk slower.
    Slowness,
    /// From Witches: your blows land softer.
    Weakness,
    /// From Witches: health drains (but never below half a heart).
    Poison,
    /// From desert Groaners: you get hungry faster.
    Hunger,
    /// From the Wilter and Charred Rattlers: health drains, all the way down.
    Wilting,
}

/// The first five, which beacons give (and whose items come first).
pub const ALL: [Potion; 5] = [Potion::Healing, Potion::Speed, Potion::FireResistance, Potion::NightVision, Potion::Leaping];
/// Everything a brewing stand makes.
pub const BREWABLE: [Potion; 7] = [Potion::Healing, Potion::Speed, Potion::FireResistance, Potion::NightVision, Potion::Leaping, Potion::Strength, Potion::Regeneration];
/// Every effect, in wire order.
pub const EFFECTS: [Potion; 16] = [
    Potion::Healing,
    Potion::Speed,
    Potion::FireResistance,
    Potion::NightVision,
    Potion::Leaping,
    Potion::Strength,
    Potion::Regeneration,
    Potion::BadOmen,
    Potion::Hero,
    Potion::MiningFatigue,
    Potion::ConduitPower,
    Potion::Slowness,
    Potion::Weakness,
    Potion::Poison,
    Potion::Hunger,
    Potion::Wilting,
];
/// Extra melee damage with Strength, and seconds per heart-half with Regeneration.
pub const STRENGTH_BONUS: f32 = 3.0;
pub const REGEN_EVERY: f32 = 2.5;
/// Seconds per half-heart lost to Poison, damage taken off by Weakness, and
/// exhaustion a second from Hunger (per level).
pub const POISON_EVERY: f32 = 1.25;
/// Seconds per half-heart lost to Wilting (per level).
pub const WILT_EVERY: f32 = 2.0;
pub const WEAKNESS_PENALTY: f32 = 3.0;
pub const HUNGER_PER_SEC: f32 = 0.5;

/// One canonical timed effect on a player.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ActiveEffect {
    pub kind: Potion,
    pub seconds: f32,
    pub amplifier: u8,
}

/// Add or refresh an effect without allowing a weaker refresh to downgrade it.
pub fn apply_timed_effect(effects: &mut Vec<ActiveEffect>, kind: Potion, seconds: f32, amplifier: u8) {
    let amplifier = amplifier.min(3);
    match effects.iter_mut().find(|effect| effect.kind == kind) {
        Some(effect) if amplifier > effect.amplifier => {
            effect.seconds = seconds;
            effect.amplifier = amplifier;
        }
        Some(effect) => effect.seconds = effect.seconds.max(seconds),
        None => effects.push(ActiveEffect { kind, seconds, amplifier }),
    }
}

pub fn effect_amplifier(effects: &[ActiveEffect], kind: Potion) -> Option<u8> {
    effects.iter().find(|effect| effect.kind == kind).map(|effect| effect.amplifier)
}

pub fn effect_level(effects: &[ActiveEffect], kind: Potion) -> u8 {
    effect_amplifier(effects, kind).map_or(0, |amplifier| amplifier + 1)
}

pub fn strength_bonus(amplifier: u8) -> f32 {
    STRENGTH_BONUS * (amplifier.min(3) as f32 + 1.0)
}

impl Potion {
    pub fn index(self) -> usize {
        BREWABLE.iter().position(|&p| p == self).unwrap_or(0)
    }
    pub fn effect_index(self) -> u8 {
        EFFECTS.iter().position(|&p| p == self).unwrap_or(0) as u8
    }
    pub fn name(self) -> &'static str {
        match self {
            Potion::Healing => "Healing",
            Potion::Speed => "Speed",
            Potion::FireResistance => "Fire Resistance",
            Potion::NightVision => "Night Vision",
            Potion::Leaping => "Leaping",
            Potion::Strength => "Strength",
            Potion::Regeneration => "Regeneration",
            Potion::BadOmen => "Bad Omen",
            Potion::Hero => "Hero of the Village",
            Potion::MiningFatigue => "Mining Fatigue",
            Potion::ConduitPower => "Conduit Power",
            Potion::Slowness => "Slowness",
            Potion::Weakness => "Weakness",
            Potion::Poison => "Poison",
            Potion::Hunger => "Hunger",
            Potion::Wilting => "Wilting",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Potion::Healing => "healing",
            Potion::Speed => "speed",
            Potion::FireResistance => "fire_resistance",
            Potion::NightVision => "night_vision",
            Potion::Leaping => "leaping",
            Potion::Strength => "strength",
            Potion::Regeneration => "regeneration",
            Potion::BadOmen => "bad_omen",
            Potion::Hero => "hero_of_the_village",
            Potion::MiningFatigue => "mining_fatigue",
            Potion::ConduitPower => "conduit_power",
            Potion::Slowness => "slowness",
            Potion::Weakness => "weakness",
            Potion::Poison => "poison",
            Potion::Hunger => "hunger",
            Potion::Wilting => "wilting",
        }
    }
    pub fn colour(self) -> [u8; 3] {
        match self {
            Potion::Healing => [230, 60, 90],
            Potion::Speed => [120, 200, 255],
            Potion::FireResistance => [240, 150, 40],
            Potion::NightVision => [40, 60, 200],
            Potion::Leaping => [120, 230, 90],
            Potion::Strength => [150, 40, 30],
            Potion::Regeneration => [230, 120, 200],
            Potion::BadOmen => [40, 70, 50],
            Potion::Hero => [90, 220, 90],
            Potion::MiningFatigue => [90, 80, 40],
            Potion::ConduitPower => [80, 190, 230],
            Potion::Slowness => [90, 110, 140],
            Potion::Weakness => [70, 70, 80],
            Potion::Poison => [80, 150, 40],
            Potion::Hunger => [90, 110, 60],
            Potion::Wilting => [55, 40, 45],
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
            Potion::Strength => SIZZLE_POWDER,
            Potion::Regeneration => WEEPER_TEAR,
            Potion::BadOmen | Potion::Hero | Potion::MiningFatigue | Potion::ConduitPower | Potion::Slowness | Potion::Weakness | Potion::Poison | Potion::Hunger | Potion::Wilting => AIR,
        }
    }
    /// Brewed from this ingredient? (Magma Cream makes Fire Resistance too.)
    pub fn brews_from(self, item: Id) -> bool {
        self.ingredient() == item || (self == Potion::FireResistance && item == MAGMA_CREAM)
    }
    /// Over at once (healing) rather than lasting.
    pub fn instant(self) -> bool {
        self == Potion::Healing
    }
}

/// A potion item: which, and whether it's a splash one.
pub fn potion_of(id: Id) -> Option<(Potion, bool)> {
    let extra = BREWABLE.len() as Id - 5;
    if (POTION_FIRST..POTION_FIRST + 5).contains(&id) {
        Some((ALL[(id - POTION_FIRST) as usize], false))
    } else if (SPLASH_FIRST..SPLASH_FIRST + 5).contains(&id) {
        Some((ALL[(id - SPLASH_FIRST) as usize], true))
    } else if (POTION_EXTRA_FIRST..POTION_EXTRA_FIRST + extra).contains(&id) {
        Some((BREWABLE[5 + (id - POTION_EXTRA_FIRST) as usize], false))
    } else if (SPLASH_EXTRA_FIRST..SPLASH_EXTRA_FIRST + extra).contains(&id) {
        Some((BREWABLE[5 + (id - SPLASH_EXTRA_FIRST) as usize], true))
    } else {
        None
    }
}

pub fn potion_item(p: Potion, splash: bool) -> Id {
    let i = p.index() as Id;
    match (i < 5, splash) {
        (true, false) => POTION_FIRST + i,
        (true, true) => SPLASH_FIRST + i,
        (false, false) => POTION_EXTRA_FIRST + i - 5,
        (false, true) => SPLASH_EXTRA_FIRST + i - 5,
    }
}

/// What brewing `ingredient` into `bottle` makes.
pub fn brew(bottle: Id, ingredient: Id) -> Option<Id> {
    if bottle == WATER_BOTTLE {
        return BREWABLE.iter().find(|p| p.brews_from(ingredient)).map(|&p| potion_item(p, false));
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
        self.timed_effect_amplified(p, EFFECT_SECS, 0);
        self.msg(format!("You feel... {}.", p.name().to_lowercase()));
    }

    /// An effect for `secs` (from food rather than a potion), quietly.
    pub fn timed_effect(&mut self, p: Potion, secs: f32) {
        self.timed_effect_amplified(p, secs, 0);
    }

    pub fn timed_effect_amplified(&mut self, p: Potion, secs: f32, amplifier: u8) {
        apply_timed_effect(&mut self.effects, p, secs, amplifier);
    }

    pub fn effect_amplifier(&self, p: Potion) -> Option<u8> {
        effect_amplifier(&self.effects, p)
    }

    pub fn effect_level(&self, p: Potion) -> u8 {
        effect_level(&self.effects, p)
    }

    pub fn has_effect(&self, p: Potion) -> bool {
        self.effect_amplifier(p).is_some()
    }

    pub fn track_peer_strength(&mut self, id: u32, secs: f32, amplifier: u8) {
        let amplifier = amplifier.min(3);
        match self.strong.get_mut(&id) {
            Some((active_secs, active_amplifier)) if amplifier > *active_amplifier => {
                *active_secs = secs;
                *active_amplifier = amplifier;
            }
            Some((active_secs, _)) => *active_secs = (*active_secs).max(secs),
            None => {
                self.strong.insert(id, (secs, amplifier));
            }
        }
    }

    /// Send a host-authored timed effect to a joined player and retain any
    /// Strength level needed to validate that player's later attacks.
    pub fn send_timed_effect(&mut self, id: u32, p: Potion, secs: f32, amplifier: u8) {
        let amplifier = amplifier.min(3);
        self.net_send_to(id, Msg::TimedEffect { effect: p.effect_index(), secs, amplifier });
        if p == Potion::Strength {
            self.track_peer_strength(id, secs, amplifier);
        }
    }

    /// Effects wear off.
    pub fn effects_tick(&mut self, dt: f32) {
        for effect in self.effects.iter_mut() {
            effect.seconds -= dt;
        }
        let ended: Vec<Potion> = self.effects.iter().filter(|effect| effect.seconds <= 0.0).map(|effect| effect.kind).collect();
        self.effects.retain(|effect| effect.seconds > 0.0);
        for p in ended {
            self.msg(format!("{} wore off.", p.name()));
        }
        // What the effects do to the player's body.
        let speed_level = self.effect_level(Potion::Speed) as f32;
        let slow_level = self.effect_level(Potion::Slowness) as f32;
        self.player.speed_boost = (1.0 + 0.35 * speed_level) * (1.0 - 0.15 * slow_level).max(0.3);
        // Poison drains health (never below half a heart); Hunger, food.
        let poison = self.effect_level(Potion::Poison) as f32;
        if poison > 0.0 && self.dead.is_none() {
            self.poison_clock += dt;
            if self.poison_clock >= POISON_EVERY / poison {
                self.poison_clock = 0.0;
                if self.player.health > 1.0 {
                    self.player.health = (self.player.health - 1.0).max(1.0);
                    self.player.hurt = 0.3;
                    self.sfx(Sfx::Hurt, None);
                }
            }
        }
        // Wilting drains health too, and doesn't stop.
        let wilting = self.effect_level(Potion::Wilting) as f32;
        if wilting > 0.0 && self.dead.is_none() {
            self.wilt_clock += dt;
            if self.wilt_clock >= WILT_EVERY / wilting {
                self.wilt_clock = 0.0;
                if self.player.health > 1.0 {
                    self.player.health -= 1.0;
                    self.player.hurt = 0.3;
                    self.sfx(Sfx::Hurt, None);
                } else {
                    // The last of it: this one's through the usual door.
                    self.player.hurt = 0.0;
                    self.hurt_player(1.0, "wilted away");
                }
            }
        }
        let hunger = self.effect_level(Potion::Hunger) as f32;
        if hunger > 0.0 && !self.creative {
            self.player.hunger.exhaust(HUNGER_PER_SEC * hunger * dt);
        }
        self.player.leaping = self.effect_level(Potion::Leaping);
        let regeneration_level = self.effect_level(Potion::Regeneration) as f32;
        if regeneration_level > 0.0 && self.dead.is_none() {
            self.regen_clock += dt;
            if self.regen_clock >= REGEN_EVERY / regeneration_level {
                self.regen_clock = 0.0;
                self.player.health = (self.player.health + 1.0).min(20.0);
            }
        }
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
            if p == Potion::Strength {
                self.track_peer_strength(id, EFFECT_SECS, 0);
            }
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
        if near_water && self.peer_take(from, GLASS_BOTTLE, 1)
            && let Some(l) = self.ledger(from) {
                l.bag.add(WATER_BOTTLE, 1);
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
    fn amplified_timed_effects_use_canonical_semantics() {
        let mut g = crate::game::tests::arena(62);
        g.timed_effect_amplified(Potion::Speed, 20.0, 2);
        g.timed_effect_amplified(Potion::Leaping, 20.0, 3);
        g.effects_tick(0.01);
        assert_eq!(g.effect_level(Potion::Speed), 3);
        assert!((g.player.speed_boost - 2.05).abs() < 1e-6);
        assert_eq!(g.player.leaping, 4);
        assert_eq!(strength_bonus(2), 9.0);

        g.timed_effect_amplified(Potion::Strength, 100.0, 3);
        g.timed_effect_amplified(Potion::Strength, 200.0, 0);
        let strength = g.effects.iter().find(|effect| effect.kind == Potion::Strength).unwrap();
        assert_eq!((strength.amplifier, strength.seconds), (3, 200.0));

        g.send_timed_effect(7, Potion::Strength, 30.0, 2);
        g.send_timed_effect(7, Potion::Strength, 40.0, 0);
        assert_eq!(g.strong.get(&7), Some(&(40.0, 2)), "host retains joined-player Strength amplifier");

        g.player.health = 10.0;
        g.timed_effect_amplified(Potion::Regeneration, 20.0, 1);
        g.effects_tick(REGEN_EVERY / 2.0 - 0.02);
        assert_eq!(g.player.health, 10.0);
        g.effects_tick(0.03);
        assert_eq!(g.player.health, 11.0);

        g.timed_effect(Potion::FireResistance, 10.0);
        g.timed_effect(Potion::NightVision, 10.0);
        assert_eq!(g.effect_amplifier(Potion::FireResistance), Some(0));
        assert_eq!(g.effect_amplifier(Potion::NightVision), Some(0));
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
