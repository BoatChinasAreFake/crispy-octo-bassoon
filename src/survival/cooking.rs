//! Cooking, over-engineered (like fishing and farming): a **Cooking Pot** on
//! a campfire.
//!
//! - Put a **Cooking Pot** (iron and a bucket) on top of a campfire, and drop
//!   ingredients in (right-click with them; up to eight). Meat, fish,
//!   vegetables, grain, fruit, mushrooms and honey each count as a kind of
//!   ingredient, and what kinds go in decides the dish: a **Hearty Stew**,
//!   **Fish Chowder**, **Garden Soup**, **Mushroom Risotto**, **Fruit
//!   Compote**, **Porridge**, **Hunter's Pie** or **Kelp Broth** (anything
//!   else is a **Hodgepodge**).
//! - It cooks while the fire's under it. Serve it into a **Bowl** whenever
//!   you like (right-click with one): each dish has its time, and too soon
//!   is underdone and too long is burnt. Right-click empty-handed to see how
//!   it's coming along.
//! - **Quality** is graded F to S, from four things: how well the
//!   ingredients suit the dish (and how many different ones went in), how
//!   **fresh** they were (raw beats leftovers, Goo is never fresh, and a pot
//!   left off the fire goes stale), the **seasoning** (a pinch of **Salt**,
//!   boiled out of a bottle of water, and some **Herbs**, from flowers; not
//!   too much, and no salt in the sweet things), and the **cooking time**.
//! - A good meal fills you up more and gives a small, short buff for its
//!   kind (Strength from a stew, Night Vision from chowder...), and it warms
//!   you up after a cold night (see springs.rs).
//! - The **Cookbook** fills in as you discover dishes, with the best grade
//!   you've managed; hold it to read it.
//!
//! The pot's contents live where the world lives (`Game::pots`, saved with
//! the dimension); a joined player's game sends a `Msg::Interact`, and the
//! host takes the ingredient or bowl and sends the dish.

use crate::block::*;
use crate::game::Game;
use crate::inventory::Wear;
use crate::net::Msg;
use crate::potions::Potion;
use crate::sound::Sfx;
use macroquad::math::{IVec3, Vec3};
use std::collections::HashMap;

/// The most a pot holds.
pub const POT_CAP: usize = 8;
/// How long ingredients take to go fully stale in a pot off the fire.
pub const STALE_SECS: f32 = 240.0;

pub fn is_pot(id: Id) -> bool {
    matches!(id, COOKING_POT | COOKING_POT_FULL | COOKING_POT_READY)
}

/// A kind of ingredient.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ing {
    Meat,
    Fish,
    Veg,
    Kelp,
    Grain,
    Fruit,
    Mushroom,
    Sweet,
    Salt,
    Herbs,
    Spoiled,
}

/// What kind of ingredient something is (None: it doesn't go in a pot).
pub fn ingredient(id: Id) -> Option<Ing> {
    Some(match id {
        PORKCHOP | MUTTON | CLUCKETS | MOO_STEAK | COOKED_CHOP | COOKED_MUTTON | COOKED_CLUCKETS | STEAK => Ing::Meat,
        COD | SALMON | TROPICAL | COOKED_COD | COOKED_SALMON => Ing::Fish,
        CARROT | POTATO | BAKED_POTATO | PUMPKIN => Ing::Veg,
        KELP | DRIED_KELP => Ing::Kelp,
        WHEAT | BREAD => Ing::Grain,
        APPLE | MELON_SLICE | GLOW_BERRIES => Ing::Fruit,
        MUSHROOM | CRIMSON_FUNGUS | TEAL_FUNGUS => Ing::Mushroom,
        h if (HONEY_FIRST..HONEY_FIRST + 5).contains(&h) => Ing::Sweet,
        SALT => Ing::Salt,
        HERBS => Ing::Herbs,
        GOO | COOKED_BOOT => Ing::Spoiled,
        _ => return None,
    })
}

/// How fresh an ingredient is as it goes in (leftovers less so; Goo never).
pub fn freshness(id: Id) -> f32 {
    match id {
        COOKED_CHOP | COOKED_MUTTON | COOKED_CLUCKETS | STEAK | COOKED_COD | COOKED_SALMON | BREAD | BAKED_POTATO | DRIED_KELP => 0.75,
        GOO | COOKED_BOOT => 0.1,
        _ => 1.0,
    }
}

/// The dishes, in Cookbook order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dish {
    HeartyStew,
    FishChowder,
    GardenSoup,
    Risotto,
    Compote,
    Porridge,
    HuntersPie,
    KelpBroth,
    Hodgepodge,
}

pub const DISHES: usize = 9;
pub const ALL_DISHES: [Dish; DISHES] = [Dish::HeartyStew, Dish::FishChowder, Dish::GardenSoup, Dish::Risotto, Dish::Compote, Dish::Porridge, Dish::HuntersPie, Dish::KelpBroth, Dish::Hodgepodge];
pub const GRADES: [&str; 6] = ["F", "D", "C", "B", "A", "S"];

impl Dish {
    pub fn index(self) -> usize {
        ALL_DISHES.iter().position(|d| *d == self).unwrap_or(0)
    }
    pub fn item(self) -> Id {
        DISH_FIRST + self.index() as Id
    }
    pub fn of_item(id: Id) -> Option<Dish> {
        (DISH_FIRST..DISH_FIRST + DISHES as Id).contains(&id).then(|| ALL_DISHES[(id - DISH_FIRST) as usize])
    }
    pub fn name(self) -> &'static str {
        ["Hearty Stew", "Fish Chowder", "Garden Soup", "Mushroom Risotto", "Fruit Compote", "Porridge", "Hunter's Pie", "Kelp Broth", "Hodgepodge"][self.index()]
    }
    /// What the Cookbook says before you've made one.
    pub fn hint(self) -> &'static str {
        ["meat and vegetables", "fish and vegetables", "vegetables, and nothing that swims or moos", "mushrooms and grain", "fruit, or fruit and honey", "grain, maybe sweetened", "meat, vegetables and grain", "kelp and fish", "whatever's left"][self.index()]
    }
    /// Seconds over the fire for it to be just right.
    pub fn ideal_secs(self) -> f32 {
        [30.0, 25.0, 20.0, 30.0, 15.0, 20.0, 45.0, 20.0, 25.0][self.index()]
    }
    /// Hunger it fills at its best.
    pub fn food(self) -> f32 {
        [8.0, 7.0, 6.0, 7.0, 5.0, 6.0, 10.0, 5.0, 4.0][self.index()]
    }
    /// The kinds that belong in it.
    fn fits(self, k: Ing) -> bool {
        use Ing::*;
        match self {
            Dish::HeartyStew => matches!(k, Meat | Veg | Mushroom),
            Dish::FishChowder => matches!(k, Fish | Veg | Kelp),
            Dish::GardenSoup => matches!(k, Veg | Kelp | Mushroom),
            Dish::Risotto => matches!(k, Mushroom | Grain | Veg),
            Dish::Compote => matches!(k, Fruit | Sweet),
            Dish::Porridge => matches!(k, Grain | Fruit | Sweet),
            Dish::HuntersPie => matches!(k, Meat | Veg | Grain | Mushroom),
            Dish::KelpBroth => matches!(k, Kelp | Fish),
            Dish::Hodgepodge => true,
        }
    }
    fn sweet(self) -> bool {
        matches!(self, Dish::Compote | Dish::Porridge)
    }
    /// The little buff a decent one gives: (effect, seconds at its best).
    pub fn buff(self) -> Option<(Potion, f32)> {
        match self {
            Dish::HeartyStew => Some((Potion::Strength, 60.0)),
            Dish::FishChowder => Some((Potion::NightVision, 120.0)),
            Dish::GardenSoup => Some((Potion::Regeneration, 10.0)),
            Dish::Risotto => Some((Potion::Speed, 60.0)),
            Dish::Compote => Some((Potion::Leaping, 60.0)),
            Dish::HuntersPie => Some((Potion::Regeneration, 20.0)),
            Dish::KelpBroth => Some((Potion::ConduitPower, 45.0)),
            Dish::Porridge | Dish::Hodgepodge => None,
        }
    }
}

/// What the ingredients make.
pub fn dish_for(items: &[(Id, f32)]) -> Dish {
    use Ing::*;
    let count = |k: Ing| items.iter().filter(|(id, _)| ingredient(*id) == Some(k)).count();
    let (meat, fish, veg, kelp, grain, fruit, mush, sweet) = (count(Meat), count(Fish), count(Veg), count(Kelp), count(Grain), count(Fruit), count(Mushroom), count(Sweet));
    if meat > 0 && veg > 0 && grain > 0 {
        Dish::HuntersPie
    } else if meat > 0 && veg > 0 {
        Dish::HeartyStew
    } else if fish > 0 && kelp > 0 && veg == 0 {
        Dish::KelpBroth
    } else if fish > 0 && (veg > 0 || kelp > 0) {
        Dish::FishChowder
    } else if mush > 0 && grain > 0 {
        Dish::Risotto
    } else if veg + kelp + mush >= 2 && meat + fish == 0 && grain + fruit + sweet == 0 {
        Dish::GardenSoup
    } else if grain > 0 && meat + fish + veg + kelp + mush == 0 {
        Dish::Porridge
    } else if fruit >= 2 || (fruit > 0 && sweet > 0) {
        Dish::Compote
    } else {
        Dish::Hodgepodge
    }
}

/// A finished dish's verdict: what it is, its quality (0..1), grade (0 F .. 5 S),
/// and what let it down most (None: nothing).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Verdict {
    pub dish: Dish,
    pub quality: f32,
    pub grade: u8,
    pub flaw: Option<&'static str>,
}

/// How well something cooked for `secs` came out, against its ideal (0..1).
pub fn doneness(secs: f32, ideal: f32) -> f32 {
    let r = secs / ideal;
    if r < 0.5 {
        0.0
    } else if r < 0.9 {
        (r - 0.5) / 0.4
    } else if r <= 1.3 {
        1.0
    } else if r < 2.0 {
        (2.0 - r) / 0.7
    } else {
        0.0
    }
}

/// Grade what's in a pot after `secs` on the fire.
pub fn judge(items: &[(Id, f32)], secs: f32) -> Verdict {
    let dish = dish_for(items);
    let food: Vec<&(Id, f32)> = items.iter().filter(|(id, _)| !matches!(ingredient(*id), Some(Ing::Salt | Ing::Herbs))).collect();
    // Fit: suited ingredients, and variety (up to four different things).
    let mut kinds: Vec<Id> = food.iter().map(|(id, _)| *id).collect();
    kinds.sort();
    kinds.dedup();
    let misfits = food.iter().filter(|(id, _)| ingredient(*id).is_some_and(|k| !dish.fits(k))).count();
    let fit = if dish == Dish::Hodgepodge { 0.3 } else { (0.5 + 0.12 * kinds.len().min(4) as f32 - 0.25 * misfits as f32).clamp(0.0, 1.0) };
    let fresh = if food.is_empty() { 0.0 } else { food.iter().map(|(_, f)| *f).sum::<f32>() / food.len() as f32 };
    let salt = items.iter().filter(|(id, _)| *id == SALT).count();
    let herbs = items.iter().filter(|(id, _)| *id == HERBS).count();
    let season = if salt + herbs > 3 {
        0.1
    } else if dish.sweet() && salt > 0 {
        0.2
    } else {
        0.4 + 0.3 * (salt > 0 || dish.sweet()) as u8 as f32 + 0.3 * (herbs > 0) as u8 as f32
    };
    let cook = doneness(secs, dish.ideal_secs());
    let quality = (0.3 * fit + 0.25 * fresh + 0.15 * season + 0.3 * cook).clamp(0.0, 1.0);
    let grade = match quality {
        q if q >= 0.92 => 5,
        q if q >= 0.8 => 4,
        q if q >= 0.65 => 3,
        q if q >= 0.5 => 2,
        q if q >= 0.35 => 1,
        _ => 0,
    };
    let r = secs / dish.ideal_secs();
    let flaw = [
        (cook, if r < 1.0 { "underdone" } else { "overcooked" }),
        (fresh, "not very fresh"),
        (fit, "an odd mix"),
        (season + 0.25, if salt + herbs > 3 || (dish.sweet() && salt > 0) { "over-seasoned" } else { "a bit bland" }),
    ]
    .into_iter()
    .filter(|(s, _)| *s < 0.85)
    .min_by(|a, b| a.0.total_cmp(&b.0))
    .map(|(_, w)| w);
    Verdict { dish, quality, grade, flaw }
}

/// What's in a pot.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pot {
    /// Each ingredient, with how fresh it still is.
    pub items: Vec<(Id, f32)>,
    /// Seconds on the fire so far.
    pub cooked: f32,
}

impl Pot {
    /// The block it shows as: empty, cooking, or ready (from just underdone on).
    pub fn block(&self) -> Id {
        if self.items.is_empty() {
            COOKING_POT
        } else if self.cooked >= dish_for(&self.items).ideal_secs() * 0.9 {
            COOKING_POT_READY
        } else {
            COOKING_POT_FULL
        }
    }

    /// How it's coming along, for a right-click.
    pub fn status(&self) -> String {
        if self.items.is_empty() {
            return "An empty pot. Put it on a campfire and add some ingredients.".into();
        }
        let d = dish_for(&self.items);
        let r = self.cooked / d.ideal_secs();
        let how = match r {
            0.0 => "not cooking yet (it wants a campfire under it)".to_string(),
            r if r < 0.9 => format!("{}% cooked", (r * 100.0) as i32),
            r if r <= 1.3 => "just right: serve it now".to_string(),
            r if r < 2.0 => "starting to catch".to_string(),
            _ => "burnt".to_string(),
        };
        format!("{} ({} in): {how}.", d.name(), self.items.len())
    }
}

/// A dish's wear: its grade.
pub fn dish_wear(grade: u8) -> Wear {
    grade.min(5) as Wear
}

pub fn grade_of(w: Wear) -> u8 {
    (w & 7).min(5) as u8
}

/// "Grade B", for a dish's tooltip.
pub fn describe(id: Id, w: Wear) -> Option<String> {
    Dish::of_item(id).map(|_| format!("Grade {}", GRADES[grade_of(w) as usize]))
}

/// Items, recipes and the save (the pots by position).
pub fn items() -> Vec<ItemDef> {
    use crate::texture::*;
    let mut v = vec![
        item("bowl", "Bowl (Empty, Hopeful)", T_BOWL),
        item("salt", "Salt (A Pinch)", T_SALT),
        item("herbs", "Herbs (Smell Nice)", T_HERBS),
        ItemDef { stack: 1, consume: false, ..item("cookbook", "Cookbook (Hold to Read)", T_COOKBOOK) },
    ];
    for (k, d) in ALL_DISHES.iter().enumerate() {
        let key = ["hearty_stew", "fish_chowder", "garden_soup", "mushroom_risotto", "fruit_compote", "porridge", "hunters_pie", "kelp_broth", "hodgepodge"][k];
        v.push(ItemDef { stack: 1, ..item(key, d.name(), T_DISH_FIRST + k as u16) });
    }
    v
}

pub fn recipes() -> Vec<Recipe> {
    let r = |inputs: &[(Id, u8)], output: (Id, u8)| Recipe { inputs: inputs.to_vec(), output };
    vec![r(&[(PLANKS, 3)], (BOWL, 4)), r(&[(FLOWER, 2)], (HERBS, 1)), r(&[(BOOK, 1), (WHEAT, 1)], (COOKBOOK, 1)), r(&[(IRON, 3), (BUCKET, 1)], (COOKING_POT, 1))]
}

/// The pot's blocks (in id order from COOKING_POT).
pub fn defs() -> Vec<BlockDef> {
    use crate::texture::*;
    ["cooking_pot", "cooking_pot_full", "cooking_pot_ready"]
        .into_iter()
        .enumerate()
        .map(|(k, key)| {
            let top = [T_COOKPOT_INNER, T_COOKPOT_STEW, T_COOKPOT_READY][k];
            let mut d = def(key, "Cooking Pot (Over-Engineered)", Model::Shaped, true, false, [top, T_COOKPOT_SIDE, T_COOKPOT_SIDE], 1.5, 1, true, COOKING_POT, 0.0, S_STONE);
            d.shape = Shape::Skull;
            d.creative = k == 0;
            d
        })
        .collect()
}

pub fn encode(pots: &HashMap<IVec3, Pot>) -> Vec<u8> {
    let mut out = (pots.len() as u32).to_le_bytes().to_vec();
    for (p, pot) in pots {
        for c in [p.x, p.y, p.z] {
            out.extend_from_slice(&c.to_le_bytes());
        }
        out.extend_from_slice(&pot.cooked.to_le_bytes());
        out.push(pot.items.len().min(POT_CAP) as u8);
        for (id, f) in pot.items.iter().take(POT_CAP) {
            out.extend_from_slice(&id.to_le_bytes());
            out.extend_from_slice(&f.to_le_bytes());
        }
    }
    out
}

pub fn decode(b: &[u8]) -> HashMap<IVec3, Pot> {
    let mut v = HashMap::new();
    let mut i = 0usize;
    let mut take = |n: usize| -> Option<&[u8]> {
        let s = b.get(i..i + n)?;
        i += n;
        Some(s)
    };
    let Some(n) = take(4).map(|s| u32::from_le_bytes(s.try_into().unwrap())) else { return v };
    for _ in 0..n.min(100_000) {
        let Some(s) = take(17) else { break };
        let c = |k: usize| i32::from_le_bytes(s[k * 4..k * 4 + 4].try_into().unwrap());
        let p = IVec3::new(c(0), c(1), c(2));
        let cooked = f32::from_le_bytes(s[12..16].try_into().unwrap());
        let mut pot = Pot { items: Vec::new(), cooked: if cooked.is_finite() { cooked.clamp(0.0, 3600.0) } else { 0.0 } };
        for _ in 0..(s[16] as usize).min(POT_CAP) {
            let Some(e) = take(6) else { break };
            let id = Id::from_le_bytes([e[0], e[1]]);
            let f = f32::from_le_bytes(e[2..6].try_into().unwrap());
            if ingredient(id).is_some() && f.is_finite() {
                pot.items.push((id, f.clamp(0.0, 1.0)));
            }
        }
        v.insert(p, pot);
    }
    v
}

/// The local player's Cookbook: per dish, the best grade made + 1 (0: not yet).
pub fn encode_book(book: &[u8; DISHES]) -> Vec<u8> {
    book.to_vec()
}

pub fn decode_book(b: &[u8]) -> [u8; DISHES] {
    let mut book = [0u8; DISHES];
    for (k, v) in b.iter().take(DISHES).enumerate() {
        book[k] = (*v).min(6);
    }
    book
}

impl Game {
    /// Right-click on a pot with whatever's in hand. True if it did something.
    pub fn use_cooking_pot(&mut self, pos: IVec3) -> bool {
        let held = self.inv.held();
        let adding = ingredient(held).is_some();
        if !adding && held != BOWL && held != AIR {
            return false;
        }
        self.player.swing = 1.0;
        if self.is_client() {
            if adding && !self.creative {
                self.inv.consume_held();
            }
            if held == BOWL && self.world.get_v(pos) != COOKING_POT && !self.creative {
                self.inv.consume_held();
            }
            self.net_send_msg(Msg::Interact { x: pos.x, y: pos.y, z: pos.z, item: held });
            return true;
        }
        match self.pot_use(pos, held) {
            PotUse::Added => {
                if !self.creative {
                    self.inv.consume_held();
                }
            }
            PotUse::Served(item, wear, text) => {
                if !self.creative {
                    self.inv.consume_held();
                }
                self.give_worn(item, 1, wear);
                self.note_dish(item, wear);
                self.msg(text);
            }
            PotUse::Said(text) => self.msg(text),
            PotUse::Nothing => return false,
        }
        true
    }

    /// A joined player used `item` on the pot at `pos`.
    pub fn host_cooking_pot(&mut self, from: u32, pos: IVec3, item: Id) {
        if !self.peer_near(from, pos) || (item != AIR && !self.peer_has(from, item)) {
            return;
        }
        let who = crate::players::record_key(&self.peer_name(from));
        match self.pot_use(pos, item) {
            PotUse::Added => {
                self.peer_take(from, item, 1);
            }
            PotUse::Served(dish, wear, text) => {
                self.peer_take(from, BOWL, 1);
                self.give_peer_worn(from, dish, 1, wear);
                self.system_message(Some(from), &text);
                if grade_of(wear) == 5 {
                    self.advance_for(&who, "chefs_kiss");
                }
            }
            PotUse::Said(text) => self.system_message(Some(from), &text),
            PotUse::Nothing => {}
        }
    }

    /// Add to, serve from, or look into a pot (where the world lives).
    fn pot_use(&mut self, pos: IVec3, held: Id) -> PotUse {
        if !is_pot(self.world.get_v(pos)) {
            return PotUse::Nothing;
        }
        let at = pos.as_vec3() + Vec3::new(0.5, 0.7, 0.5);
        let pot = self.pots.entry(pos).or_default();
        if ingredient(held).is_some() {
            if pot.items.len() >= POT_CAP {
                return PotUse::Said("The pot's full. Serve it before it boils over.".into());
            }
            pot.items.push((held, freshness(held)));
            let block = pot.block();
            self.world.set_v(pos, block);
            self.sfx(Sfx::Splash, Some(at));
            return PotUse::Added;
        }
        if held == BOWL && !pot.items.is_empty() {
            let pot = self.pots.remove(&pos).unwrap_or_default();
            let v = judge(&pot.items, pot.cooked);
            self.world.set_v(pos, COOKING_POT);
            self.sfx(Sfx::Eat, Some(at));
            let flaw = v.flaw.map(|f| format!(": {f}")).unwrap_or_else(|| ": perfect".into());
            let text = format!("You dish up a {}. Grade {}{flaw}.", v.dish.name(), GRADES[v.grade as usize]);
            return PotUse::Served(v.dish.item(), dish_wear(v.grade), text);
        }
        PotUse::Said(self.pots.get(&pos).map(|p| p.status()).unwrap_or_else(|| Pot::default().status()))
    }

    /// Where the world lives: pots on a fire cook, pots off it go stale.
    pub fn pots_tick(&mut self, dt: f32) {
        if self.is_client() || self.pots.is_empty() {
            return;
        }
        let mut changed = Vec::new();
        let mut gone = Vec::new();
        for (p, pot) in self.pots.iter_mut() {
            if !self.world.is_loaded(p.x, p.z) {
                continue;
            }
            if !is_pot(self.world.get_v(*p)) {
                gone.push(*p);
                continue;
            }
            if pot.items.is_empty() {
                continue;
            }
            if self.world.get_v(*p - IVec3::Y) == CAMPFIRE {
                pot.cooked = (pot.cooked + dt).min(3600.0);
            } else {
                for (_, f) in pot.items.iter_mut() {
                    *f = (*f - dt / STALE_SECS).max(0.0);
                }
            }
            let b = pot.block();
            if self.world.get_v(*p) != b {
                changed.push((*p, b));
            }
        }
        for p in gone {
            // Broken: what was in it spills.
            if let Some(pot) = self.pots.remove(&p) {
                for (id, _) in pot.items {
                    self.pop_drop(p.as_vec3() + Vec3::new(0.5, 0.5, 0.5), id, 1);
                }
            }
        }
        for (p, b) in changed {
            self.world.set_v(p, b);
        }
    }

    /// Steam off pots cooking near `eye` (everyone draws it from the blocks).
    pub fn pot_steam(&mut self, dt: f32) {
        if self.away() {
            return;
        }
        let eye = self.player.eye();
        let near: Vec<IVec3> = self.world.cook_pots.iter().copied().filter(|p| p.as_vec3().distance(eye) < 32.0).collect();
        for p in near {
            let id = self.world.get_v(p);
            if matches!(id, COOKING_POT_FULL | COOKING_POT_READY) && self.world.get_v(p - IVec3::Y) == CAMPFIRE && self.rng.chance(dt * if id == COOKING_POT_READY { 6.0 } else { 3.0 }) {
                self.smoke(p.as_vec3() + Vec3::new(0.5, 0.7, 0.5), 1, 0.15);
            }
        }
    }

    /// A dish came our way: into the Cookbook it goes.
    pub fn note_dish(&mut self, item: Id, wear: Wear) {
        let Some(d) = Dish::of_item(item) else { return };
        let g = grade_of(wear) + 1;
        let k = d.index();
        let new = self.cookbook[k] == 0;
        self.cookbook[k] = self.cookbook[k].max(g);
        if new {
            self.msg(format!("New in your Cookbook: {}.", d.name()));
            self.advance("first_course");
        }
        if g == 6 {
            self.advance("chefs_kiss");
        }
        if self.cookbook.iter().all(|&b| b > 0) {
            self.advance("cordon_bleu");
        }
    }

    /// Eat a dish: more filling the better it is, a buff if it's decent.
    pub fn eat_dish(&mut self, item: Id, wear: Wear) -> bool {
        let Some(d) = Dish::of_item(item) else { return false };
        let g = grade_of(wear);
        let q = g as f32 / 5.0;
        if self.player.hunger.full() && !self.creative {
            return false;
        }
        self.player.hunger.eat((d.food() * (0.5 + 0.5 * q)).round(), 0.4 + 0.6 * q);
        self.stats.eaten += 1;
        if g >= 2
            && let Some((p, secs)) = d.buff()
        {
            self.timed_effect(p, secs * (0.4 + 0.6 * q));
        }
        if !self.creative {
            self.use_up_held();
        }
        self.warm_up(20.0 + 40.0 * q);
        self.sfx(Sfx::Eat, None);
        self.msg(format!("{} (Grade {}). {}", d.name(), GRADES[g as usize], ["Edible. Technically.", "Not bad.", "Rather good, actually.", "Delicious.", "Wonderful.", "Perfect. A tear rolls down your blocky cheek."][g as usize]));
        true
    }
}

/// What a click on a pot did.
enum PotUse {
    Added,
    Served(Id, Wear, String),
    Said(String),
    Nothing,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::tests::arena;

    #[test]
    fn ingredients_decide_the_dish() {
        let f = |ids: &[Id]| dish_for(&ids.iter().map(|&i| (i, 1.0)).collect::<Vec<_>>());
        assert_eq!(f(&[PORKCHOP, CARROT]), Dish::HeartyStew);
        assert_eq!(f(&[PORKCHOP, CARROT, WHEAT]), Dish::HuntersPie);
        assert_eq!(f(&[COD, POTATO]), Dish::FishChowder);
        assert_eq!(f(&[COD, KELP]), Dish::KelpBroth);
        assert_eq!(f(&[CARROT, POTATO]), Dish::GardenSoup);
        assert_eq!(f(&[MUSHROOM, WHEAT]), Dish::Risotto);
        assert_eq!(f(&[APPLE, MELON_SLICE]), Dish::Compote);
        assert_eq!(f(&[WHEAT, HONEY_FIRST]), Dish::Porridge);
        assert_eq!(f(&[GOO, BREAD, COD, APPLE]), Dish::Hodgepodge);
        for d in ALL_DISHES {
            assert_eq!(Dish::of_item(d.item()), Some(d));
        }
    }

    #[test]
    fn quality_rewards_fresh_seasoned_well_timed_cooking() {
        let fresh = [(PORKCHOP, 1.0), (CARROT, 1.0), (POTATO, 1.0), (MUSHROOM, 1.0), (SALT, 1.0), (HERBS, 1.0)];
        let best = judge(&fresh, 32.0);
        assert_eq!((best.dish, best.grade), (Dish::HeartyStew, 5), "{best:?}");
        assert_eq!(best.flaw, None);
        // Underdone, burnt, stale, bland, over-salted: each costs.
        let raw = judge(&fresh, 12.0);
        assert!(raw.grade < best.grade && raw.flaw == Some("underdone"), "{raw:?}");
        let burnt = judge(&fresh, 70.0);
        assert!(burnt.grade < best.grade && burnt.flaw == Some("overcooked"), "{burnt:?}");
        let stale: Vec<(Id, f32)> = fresh.iter().map(|&(id, _)| (id, 0.1)).collect();
        assert!(judge(&stale, 32.0).quality < best.quality);
        let bland = judge(&fresh[..4], 32.0);
        assert!(bland.quality < best.quality && bland.flaw == Some("a bit bland"), "{bland:?}");
        let salty = [(PORKCHOP, 1.0), (CARROT, 1.0), (SALT, 1.0), (SALT, 1.0), (SALT, 1.0), (SALT, 1.0)];
        assert_eq!(judge(&salty, 32.0).flaw, Some("over-seasoned"));
        // Sweet things don't want salt.
        assert!(judge(&[(APPLE, 1.0), (MELON_SLICE, 1.0), (SALT, 1.0)], 15.0).quality < judge(&[(APPLE, 1.0), (MELON_SLICE, 1.0)], 15.0).quality);
        assert_eq!(doneness(0.0, 30.0), 0.0);
        assert_eq!(doneness(30.0, 30.0), 1.0);
        assert_eq!(doneness(90.0, 30.0), 0.0);
    }

    #[test]
    fn a_pot_on_a_campfire_cooks_and_serves() {
        let mut g = arena(921);
        let fire = IVec3::new(2, 51, 2);
        let pot = fire + IVec3::Y;
        g.world.set_v(fire, CAMPFIRE);
        g.world.set_v(pot, COOKING_POT);
        g.inv.selected = 0;
        for id in [MOO_STEAK, CARROT, POTATO, SALT, HERBS] {
            g.inv.slots[0] = Some((id, 1));
            assert!(g.use_cooking_pot(pot));
        }
        assert_eq!(g.world.get_v(pot), COOKING_POT_FULL);
        for _ in 0..300 {
            g.pots_tick(0.1);
        }
        assert_eq!(g.world.get_v(pot), COOKING_POT_READY);
        g.inv.slots[0] = Some((BOWL, 1));
        assert!(g.use_cooking_pot(pot));
        assert_eq!(g.world.get_v(pot), COOKING_POT);
        let slot = g.inv.slots.iter().position(|s| s.is_some_and(|s| s.0 == Dish::HeartyStew.item())).expect("a stew");
        assert!(grade_of(g.inv.wear[slot]) >= 4, "grade {}", grade_of(g.inv.wear[slot]));
        assert!(g.cookbook[Dish::HeartyStew.index()] > 0, "in the Cookbook");
        // Eating it: food, Strength, and the bowl back.
        g.player.hunger.food = 4.0;
        g.inv.selected = slot;
        let wear = g.inv.wear[slot];
        assert!(g.eat_dish(Dish::HeartyStew.item(), wear));
        assert!(g.player.hunger.food > 9.0);
        assert!(g.has_effect(Potion::Strength));
        // Off the fire, a pot goes stale.
        g.world.set_v(fire, STONE);
        g.inv.slots[0] = Some((APPLE, 1));
        g.inv.selected = 0;
        g.use_cooking_pot(pot);
        for _ in 0..100 {
            g.pots_tick(1.0);
        }
        assert!(g.pots[&pot].items[0].1 < 0.7);
        assert_eq!(g.pots[&pot].cooked, 0.0, "no fire, no cooking");
        // And it's saved.
        assert_eq!(decode(&encode(&g.pots)), g.pots);
        assert_eq!(decode_book(&encode_book(&g.cookbook)), g.cookbook);
    }
}
