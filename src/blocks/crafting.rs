//! Crafting tables, finally good for something, and the recipe book.
//!
//! Small recipes (four ingredients or fewer, the ones that would fit in
//! Minecraft's 2x2 pocket grid) work anywhere. Everything bigger needs a
//! Crafting Table within a few blocks: right-click one, or just open your
//! inventory while standing near it. Joined players' hosts check the table
//! too.
//!
//! The recipe book lists every recipe you've discovered (one is discovered as
//! soon as you've held any of its ingredients, or what it makes), with a
//! search box, tabs, a "craftable now" filter, and a pin that keeps one
//! recipe's shopping list on screen while you go and gather it.

use crate::block::*;
use crate::world::World;
use macroquad::math::{IVec3, Vec3};
use std::collections::BTreeSet;

/// Ingredients that fit in your pockets.
pub const POCKET_LIMIT: u32 = 4;
/// How close (in blocks, any direction) a table has to be.
pub const TABLE_REACH: i32 = 4;

/// Does it need a crafting table?
pub fn needs_table(r: &Recipe) -> bool {
    r.inputs.iter().map(|&(_, n)| n as u32).sum::<u32>() > POCKET_LIMIT
}

/// The nearest crafting table within `reach` of `at`.
pub fn table_near(world: &World, at: Vec3, reach: i32) -> Option<IVec3> {
    let c = at.floor().as_ivec3();
    let mut best: Option<(i32, IVec3)> = None;
    for dy in -reach..=reach {
        for dz in -reach..=reach {
            for dx in -reach..=reach {
                let p = c + IVec3::new(dx, dy, dz);
                if world.get_v(p) == TABLE {
                    let d = dx * dx + dy * dy + dz * dz;
                    if best.is_none_or(|(b, _)| d < b) {
                        best = Some((d, p));
                    }
                }
            }
        }
    }
    best.map(|(_, p)| p)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    All,
    Building,
    Tools,
    Food,
    Zappy,
    Odds,
}

impl Tab {
    pub const ALL: [Tab; 6] = [Tab::All, Tab::Building, Tab::Tools, Tab::Food, Tab::Zappy, Tab::Odds];

    pub fn name(self) -> &'static str {
        match self {
            Tab::All => "All",
            Tab::Building => "Blocks",
            Tab::Tools => "Gear",
            Tab::Food => "Food",
            Tab::Zappy => "Zappy",
            Tab::Odds => "Odds",
        }
    }
}

/// Which tab a recipe lives under.
pub fn tab_of(r: &Recipe) -> Tab {
    let out = r.output.0;
    let zappy = r.inputs.iter().any(|&(i, _)| matches!(i, ZAP_DUST | ZTORCH_ON | ZAP_BLOCK)) || out == ZAP_DUST || out == LEVER || out == BUTTON || out == PLATE;
    if zappy {
        Tab::Zappy
    } else if food_value(out).is_some() || matches!(out, CAKE | STEW) {
        Tab::Food
    } else if durability(out).is_some() || matches!(out, ARROW | ROCKET | BUCKET | COMPASS | MAP) {
        Tab::Tools
    } else if is_block_item(out) {
        Tab::Building
    } else {
        Tab::Odds
    }
}

/// Does a recipe match what was typed in the search box? Searches what it
/// makes and what goes in, ignoring case.
pub fn matches(r: &Recipe, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return true;
    }
    let hit = |id: Id| item_name(id).to_lowercase().contains(&q) || reg().key_of(id).replace('_', " ").contains(&q);
    hit(r.output.0) || r.inputs.iter().any(|&(i, _)| hit(i))
}

/// Has the player come across anything to do with this recipe?
pub fn discovered(r: &Recipe, known: &BTreeSet<Id>) -> bool {
    known.contains(&r.output.0) || r.inputs.iter().any(|(i, _)| known.contains(i))
}

/// Saved as item keys, so mods coming and going don't scramble it.
pub fn encode_known(known: &BTreeSet<Id>) -> String {
    known.iter().map(|&i| reg().key_of(i)).filter(|k| !k.is_empty()).collect::<Vec<_>>().join(",")
}

pub fn decode_known(s: &str) -> BTreeSet<Id> {
    s.split(',').filter_map(|k| reg().lookup(k.trim())).collect()
}

// ------------------------------------------------------------------ in the game

use crate::game::Game;

impl Game {
    /// A crafting table close enough to use.
    pub fn table_nearby(&self) -> bool {
        self.creative || table_near(&self.world, self.player.eye(), TABLE_REACH).is_some()
    }

    /// Note everything in the inventory as seen (for the recipe book).
    pub fn learn_inventory(&mut self) {
        let before = self.known.len();
        for (id, _) in self.inv.slots.iter().chain(self.inv.armor.iter()).chain(std::iter::once(&self.inv.cursor)).flatten() {
            self.known.insert(*id);
        }
        if self.known.len() > before && before > 0 {
            let new = recipes().iter().filter(|r| discovered(r, &self.known)).count();
            if new > self.recipes_seen {
                self.recipe_news = 4.0;
            }
        }
        self.recipes_seen = recipes().iter().filter(|r| discovered(r, &self.known)).count();
    }

    /// Joined player crafting a table recipe: are they really near one?
    pub fn peer_at_table(&self, from: u32) -> bool {
        // A little slack over the local check, for lag.
        self.peers.get(&from).is_some_and(|p| table_near(&self.world, p.target + Vec3::Y * 1.6, TABLE_REACH + 2).is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find(out: Id) -> &'static Recipe {
        recipes().iter().find(|r| r.output.0 == out).unwrap()
    }

    #[test]
    fn small_things_in_pockets_big_things_at_tables() {
        assert!(!needs_table(find(PLANKS)), "planks from a log");
        assert!(!needs_table(find(STICK)));
        assert!(!needs_table(find(TABLE)), "you can always make a table");
        assert!(!needs_table(find(TORCH)));
        assert!(needs_table(find(PICK_WOOD)));
        assert!(needs_table(find(CHEST)));
        assert!(needs_table(find(FURNACE)));
        let pocket = recipes().iter().filter(|r| !needs_table(r)).count();
        assert!(pocket > 20 && pocket < recipes().len() - 40, "{pocket} pocket recipes");
    }

    #[test]
    fn search_tabs_and_discovery() {
        let pick = find(PICK_IRON);
        assert!(matches(pick, "pick"));
        assert!(matches(pick, "IRON"), "ingredients count");
        assert!(!matches(pick, "cake"));
        assert_eq!(tab_of(pick), Tab::Tools);
        assert_eq!(tab_of(find(BREAD)), Tab::Food);
        assert_eq!(tab_of(find(LAMP)), Tab::Zappy);
        assert_eq!(tab_of(find(STONE_BRICKS)), Tab::Building);
        let mut known = BTreeSet::new();
        assert!(!discovered(pick, &known));
        known.insert(IRON);
        assert!(discovered(pick, &known));
        let back = decode_known(&encode_known(&known));
        assert_eq!(back, known);
    }

    #[test]
    fn tables_are_found_nearby() {
        let mut w = World::new(1);
        let start = std::time::Instant::now();
        while !w.chunks.contains_key(&(0, 0)) && start.elapsed().as_secs() < 20 {
            w.stream(&[(Vec3::ZERO, 1)]);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let p = IVec3::new(2, 70, 2);
        w.set_v(p, TABLE);
        assert_eq!(table_near(&w, Vec3::new(0.5, 70.5, 0.5), TABLE_REACH), Some(p));
        assert_eq!(table_near(&w, Vec3::new(20.5, 70.5, 0.5), TABLE_REACH), None);
    }
}
