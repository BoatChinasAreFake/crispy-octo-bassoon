//! Hunger, Minecraft style: a food bar (20 points), hidden saturation that
//! gets used up first, and exhaustion from running, jumping, fighting and
//! digging that slowly eats into both. A full stomach heals you; an empty one
//! hurts, but only down to half a heart (starving is dramatic, not fatal).

use crate::player::MAX_HEALTH;

pub const MAX_FOOD: f32 = 20.0;
/// Below this you're too hungry to sprint.
pub const SPRINT_FOOD: f32 = 6.0;
/// Exhaustion that costs one point of saturation (or food).
const EXHAUSTION_PER_POINT: f32 = 4.0;

// Exhaustion per action (Minecraft's numbers).
pub const SPRINT_PER_BLOCK: f32 = 0.1;
pub const SWIM_PER_BLOCK: f32 = 0.01;
pub const JUMP: f32 = 0.05;
pub const SPRINT_JUMP: f32 = 0.2;
pub const ATTACK: f32 = 0.1;
pub const HURT: f32 = 0.1;
pub const DIG: f32 = 0.005;
/// Healing a point of health costs this much.
const HEAL: f32 = 6.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Hunger {
    pub food: f32,
    pub saturation: f32,
    pub exhaustion: f32,
    timer: f32,
}

impl Default for Hunger {
    fn default() -> Self {
        Hunger { food: MAX_FOOD, saturation: 5.0, exhaustion: 0.0, timer: 0.0 }
    }
}

impl Hunger {
    pub fn new(food: f32, saturation: f32) -> Hunger {
        Hunger { food: food.clamp(0.0, MAX_FOOD), saturation: saturation.clamp(0.0, food.clamp(0.0, MAX_FOOD)), ..Hunger::default() }
    }

    pub fn exhaust(&mut self, amount: f32) {
        self.exhaustion = (self.exhaustion + amount).min(40.0);
    }

    /// Eat something worth `points` of food; `quality` scales the saturation
    /// (0.1 for junk, 0.3 raw, 0.6 plain, 0.8 cooked, 1.2 golden).
    pub fn eat(&mut self, points: f32, quality: f32) {
        self.food = (self.food + points).min(MAX_FOOD);
        self.saturation = (self.saturation + points * quality * 2.0).min(self.food);
    }

    pub fn full(&self) -> bool {
        self.food >= MAX_FOOD
    }

    pub fn can_sprint(&self) -> bool {
        self.food > SPRINT_FOOD
    }

    /// Advance by `dt` seconds. Returns the change in health: healing when well
    /// fed, a point of starvation damage every few seconds when empty.
    pub fn tick(&mut self, dt: f32, health: f32) -> f32 {
        while self.exhaustion >= EXHAUSTION_PER_POINT {
            self.exhaustion -= EXHAUSTION_PER_POINT;
            if self.saturation > 0.0 {
                self.saturation = (self.saturation - 1.0).max(0.0);
            } else {
                self.food = (self.food - 1.0).max(0.0);
            }
        }
        let hurt = health < MAX_HEALTH && health > 0.0;
        self.timer += dt;
        if self.food >= MAX_FOOD && self.saturation > 0.0 && hurt {
            // Well fed: quick healing, paid for out of saturation.
            if self.timer >= 0.5 {
                self.timer = 0.0;
                let heal = self.saturation.min(HEAL) / HEAL;
                self.exhaust(heal * HEAL);
                return heal;
            }
        } else if self.food >= 18.0 && hurt {
            if self.timer >= 4.0 {
                self.timer = 0.0;
                self.exhaust(HEAL);
                return 1.0;
            }
        } else if self.food <= 0.0 {
            if self.timer >= 4.0 {
                self.timer = 0.0;
                if health > 1.0 {
                    return -1.0;
                }
            }
        } else {
            self.timer = 0.0;
        }
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hunger_heals_drains_and_starves() {
        let mut h = Hunger::default();
        // Full and hurt: quick healing, which costs saturation.
        let healed: f32 = (0..20).map(|_| h.tick(0.5, 10.0)).sum();
        assert!(healed > 1.0, "healed {healed}");
        assert!(h.saturation < 5.0);
        // Running around empties the bar: saturation goes first, then food.
        let mut h = Hunger::default();
        h.exhaust(4.0 * 5.0);
        h.tick(0.1, MAX_HEALTH);
        assert_eq!((h.food, h.saturation), (MAX_FOOD, 0.0));
        for _ in 0..10 {
            h.exhaust(4.0);
            h.tick(0.1, MAX_HEALTH);
        }
        assert_eq!(h.food, 10.0);
        // No sprinting on an empty stomach, and no healing below 18.
        h.food = 6.0;
        assert!(!h.can_sprint());
        assert_eq!((0..10).map(|_| h.tick(1.0, 10.0)).sum::<f32>(), 0.0);
        // Starving hurts every four seconds, but never below half a heart.
        h.food = 0.0;
        assert_eq!((0..8).map(|_| h.tick(1.0, 10.0)).sum::<f32>(), -2.0);
        assert_eq!((0..8).map(|_| h.tick(1.0, 1.0)).sum::<f32>(), 0.0);
        // Eating: points and saturation, capped.
        h.eat(8.0, 0.8);
        assert_eq!(h.food, 8.0);
        assert!((h.saturation - 8.0).abs() < 1e-4, "saturation can't pass food");
        h.eat(20.0, 0.3);
        assert!(h.full());
    }
}
