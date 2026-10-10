//! Home and craft: cauldrons, composters, mob heads, Galloper armour and
//! lightning rods.
//!
//! - A **Cauldron** (seven iron chunks) holds three levels of water: a water
//!   bucket fills it, an empty bucket takes a full one back, and bottles take
//!   or give a level at a time. Drop a dye in and the water takes its colour;
//!   dip woolly armour in it (right-click) and the armour comes out that
//!   colour, for a level of water. Plain water washes the dye out of woolly
//!   armour, and the top pattern off a banner.
//! - A **Composter** (seven planks) takes plant scraps: each one might raise
//!   its level (the likelier the more filling the scrap), and at the top it
//!   gives back Compost.
//! - **Mob heads**: Groaners, Rattlers and Hissers drop their heads now and
//!   then, to put on a shelf and look at.
//! - **Galloper armour** (iron, gold or dimond, found in chests) goes on a
//!   Galloper you've tamed and soaks up part of every hit. Shears take it off.
//! - A **Lightning Rod** (three copper ingots) draws in any lightning that
//!   strikes within 32 blocks (the tallest rod wins), sparing what's round
//!   about, and gives off Zappy power for a moment.
//!
//! The world's owner does everything here; a joined player's game does the
//! inventory side straight away and sends a `Msg::Interact`, and the host
//! checks it against what they hold before changing the block.

use crate::block::*;
use crate::game::Game;
use crate::inventory::Wear;
use crate::net::Msg;
use crate::sound::Sfx;
use crate::texture::*;
use macroquad::math::{IVec3, Vec3};

/// Water levels a cauldron holds.
pub const LEVELS: u8 = 3;
/// How far a lightning rod reaches for a strike.
pub const ROD_REACH: f32 = 32.0;
/// How long a struck rod stays powered, in seconds.
pub const ROD_SECS: f32 = 0.8;
/// The chance a Groaner, Rattler or Hisser leaves its head behind.
pub const HEAD_CHANCE: f32 = 0.025;
/// Galloper armour: the share of each hit it soaks up (none, iron, gold, dimond).
pub const BARDING_SOAK: [f32; 4] = [0.0, 0.2, 0.28, 0.44];

pub fn is_cauldron(id: Id) -> bool {
    (CAULDRON..CAULDRON_DYED + 24).contains(&id)
}

pub fn is_composter(id: Id) -> bool {
    (COMPOSTER..COMPOSTER + 8).contains(&id)
}

pub fn is_head(id: Id) -> bool {
    matches!(id, GROANER_HEAD | RATTLER_SKULL | HISSER_HEAD)
}

pub fn is_rod(id: Id) -> bool {
    id == LIGHTNING_ROD || id == LIGHTNING_ROD_ON
}

/// A cauldron's water level (0 to 3) and the dye in it, if any.
pub fn cauldron_state(id: Id) -> (u8, Option<usize>) {
    if (CAULDRON_WATER..CAULDRON_WATER + 3).contains(&id) {
        ((id - CAULDRON_WATER) as u8 + 1, None)
    } else if (CAULDRON_DYED..CAULDRON_DYED + 24).contains(&id) {
        let k = id - CAULDRON_DYED;
        ((k % 3) as u8 + 1, Some((k / 3) as usize))
    } else {
        (0, None)
    }
}

/// The cauldron block with this much water (and dye) in it.
pub fn cauldron_block(level: u8, dye: Option<usize>) -> Id {
    match (level.min(LEVELS), dye) {
        (0, _) => CAULDRON,
        (l, None) => CAULDRON_WATER + l as Id - 1,
        (l, Some(c)) => CAULDRON_DYED + (c % 8) as Id * 3 + l as Id - 1,
    }
}

/// What using a held item on a cauldron does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CauldronUse {
    /// The cauldron afterwards.
    pub block: Id,
    /// One of the held item is used up.
    pub take: bool,
    /// Something comes back (an empty bucket, a filled bottle).
    pub give: Option<Id>,
    /// The held item's new wear (dyed or washed armour, a washed banner).
    pub wear: Option<Wear>,
}

/// Using `held` (with `wear`) on cauldron `block`. None: nothing happens.
pub fn cauldron_use(block: Id, held: Id, wear: Wear) -> Option<CauldronUse> {
    let (level, dye) = cauldron_state(block);
    let plain = CauldronUse { block, take: false, give: None, wear: None };
    match held {
        // A bucket of water fills it right up (and clears any dye).
        WATER_BUCKET if level < LEVELS || dye.is_some() => Some(CauldronUse { block: cauldron_block(LEVELS, None), take: true, give: Some(BUCKET), ..plain }),
        BUCKET if level == LEVELS && dye.is_none() => Some(CauldronUse { block: CAULDRON, take: true, give: Some(WATER_BUCKET), ..plain }),
        GLASS_BOTTLE if level > 0 && dye.is_none() => Some(CauldronUse { block: cauldron_block(level - 1, None), take: true, give: Some(WATER_BOTTLE), ..plain }),
        WATER_BOTTLE if level < LEVELS && dye.is_none() => Some(CauldronUse { block: cauldron_block(level + 1, None), take: true, give: Some(GLASS_BOTTLE), ..plain }),
        d if (DYE_FIRST..DYE_FIRST + 8).contains(&d) && level > 0 => {
            let c = (d - DYE_FIRST) as usize;
            (dye != Some(c)).then_some(CauldronUse { block: cauldron_block(level, Some(c)), take: true, ..plain })
        }
        // Woolly armour: dyed in dyed water, washed in plain.
        a if crate::trims::is_woolly(a) && level > 0 => {
            let new = crate::trims::with_dye(wear, dye);
            (crate::trims::dye_of(new) != crate::trims::dye_of(wear)).then_some(CauldronUse { block: cauldron_block(level - 1, dye), wear: Some(new), ..plain })
        }
        // A banner loses its top pattern in plain water.
        BANNER if level > 0 && dye.is_none() => {
            let d = crate::banners::Design::from_bits(crate::enchant::enchants(wear));
            let washed = washed_banner(d)?;
            Some(CauldronUse { block: cauldron_block(level - 1, None), wear: Some((wear & 0xFFFF) | (washed.bits() as Wear) << 16), ..plain })
        }
        _ => None,
    }
}

/// A banner with its top pattern washed off (None: there's nothing to wash).
pub fn washed_banner(mut d: crate::banners::Design) -> Option<crate::banners::Design> {
    if d.layers[1].0 != 0 {
        d.layers[1] = (0, 0);
    } else if d.layers[0].0 != 0 {
        d.layers[0] = (0, 0);
    } else {
        return None;
    }
    Some(d)
}

/// How likely a scrap is to raise a composter's level (None: it won't go in).
pub fn compost_chance(id: Id) -> Option<f32> {
    Some(match id {
        WHEAT_SEEDS | KELP | DRIED_KELP | SEAGRASS | TALL_GRASS | SAPLING | GLOW_BERRIES => 0.3,
        _ if is_leaves(id) => 0.3,
        MELON_SLICE | CACTUS | BAMBOO | LILY_PAD => 0.5,
        WHEAT | CARROT | POTATO | APPLE | PUMPKIN | MOSS_BLOCK => 0.65,
        _ if crate::bees::is_flower(id) || id == FLOWER => 0.65,
        BAKED_POTATO | BREAD | MELON | HAY => 0.85,
        _ => return None,
    })
}

/// Now and then, a mob's head.
pub fn head_drop(kind: crate::entity::MobKind, rng: &mut crate::noise::Rng) -> Option<(Id, u8)> {
    use crate::entity::MobKind;
    let head = match kind {
        MobKind::Groaner | MobKind::DesertGroaner => GROANER_HEAD,
        MobKind::Rattler | MobKind::SnowyRattler => RATTLER_SKULL,
        MobKind::Hisser => HISSER_HEAD,
        _ => return None,
    };
    rng.chance(HEAD_CHANCE).then_some((head, 1))
}

/// Galloper armour's level (1 iron, 2 gold, 3 dimond), or 0.
pub fn barding_of(item: Id) -> u8 {
    match item {
        HORSE_ARMOR_IRON => 1,
        HORSE_ARMOR_GOLD => 2,
        HORSE_ARMOR_DIAMOND => 3,
        _ => 0,
    }
}

pub fn barding_item(level: u8) -> Option<Id> {
    match level {
        1 => Some(HORSE_ARMOR_IRON),
        2 => Some(HORSE_ARMOR_GOLD),
        3 => Some(HORSE_ARMOR_DIAMOND),
        _ => None,
    }
}

/// Block definitions (appended to the registry, in id order from CAULDRON).
pub fn defs() -> Vec<BlockDef> {
    use Model::Shaped;
    let leak = |s: String| -> &'static str { Box::leak(s.into_boxed_str()) };
    let mut v = Vec::new();
    let cauldron = |key: &'static str, name: &'static str| {
        let mut d = def(key, name, Shaped, true, false, [T_CAULDRON_TOP, T_CAULDRON_SIDE, T_CAULDRON_INNER], 2.0, 1, true, CAULDRON, 0.0, S_STONE);
        d.creative = key == "cauldron";
        d
    };
    v.push(cauldron("cauldron", "Cauldron (Double, Double)"));
    for l in 1..=3 {
        v.push(cauldron(leak(format!("water_cauldron_{l}")), "Cauldron of Water"));
    }
    for (key, name) in crate::carpentry::COLOURS {
        for l in 1..=3 {
            v.push(cauldron(leak(format!("{key}_dye_cauldron_{l}")), leak(format!("Cauldron of {name} Dye"))));
        }
    }
    for l in 0..8 {
        let mut d = def(leak(format!("composter_{l}")), "Composter (Smells Like Progress)", Shaped, true, false, [T_COMPOSTER_TOP, T_COMPOSTER_SIDE, T_COMPOSTER_SIDE], 0.6, 0, false, COMPOSTER, 0.0, S_WOOD);
        d.creative = l == 0;
        v.push(d);
    }
    for (id, key, name, side, face) in [
        (GROANER_HEAD, "groaner_head", "Groaner Head (Still Groaning, Faintly)", T_GROAN_SKIN, T_GROAN_FACE),
        (RATTLER_SKULL, "rattler_skull", "Rattler Skull (Rattles If Shaken)", T_BONE, T_RATTLER_FACE),
        (HISSER_HEAD, "hisser_head", "Hisser Head (Defused)", T_HISSER_SKIN, T_HISSER_FACE),
    ] {
        let mut d = def(key, name, Shaped, true, false, [side, face, side], 1.0, 0, false, id, 0.0, S_STONE);
        d.shape = Shape::Skull;
        v.push(d);
    }
    for (on, key, tile) in [(false, "lightning_rod", T_LIGHTNING_ROD), (true, "lightning_rod_on", T_LIGHTNING_ROD_ON)] {
        let mut d = def(key, "Lightning Rod (Come At Me, Sky)", Shaped, true, false, [tile; 3], 3.0, 1, true, LIGHTNING_ROD, if on { 8.0 } else { 0.0 }, S_STONE);
        d.shape = Shape::Chain;
        d.creative = !on;
        v.push(d);
    }
    v
}

pub fn items() -> Vec<ItemDef> {
    vec![
        ItemDef { stack: 1, ..item("iron_horse_armor", "Iron Galloper Armour (Clanky Hooves)", T_HORSE_ARMOR_ITEMS) },
        ItemDef { stack: 1, ..item("gold_horse_armor", "Gold Galloper Armour (Showy, Surprisingly Sturdy)", T_HORSE_ARMOR_ITEMS + 1) },
        ItemDef { stack: 1, ..item("diamond_horse_armor", "Dimond Galloper Armour (A Tank With a Mane)", T_HORSE_ARMOR_ITEMS + 2) },
    ]
}

pub fn recipes() -> Vec<Recipe> {
    let r = |inputs: &[(Id, u8)], output: (Id, u8)| Recipe { inputs: inputs.to_vec(), output };
    vec![r(&[(IRON, 7)], (CAULDRON, 1)), r(&[(PLANKS, 7)], (COMPOSTER, 1)), r(&[(COPPER_INGOT, 3)], (LIGHTNING_ROD, 1))]
}

/// The tallest lightning rod within reach of a strike at `at`, if any.
pub fn rod_for(rods: &std::collections::HashSet<IVec3>, at: Vec3) -> Option<IVec3> {
    rods.iter()
        .copied()
        .filter(|r| Vec3::new(r.x as f32 + 0.5 - at.x, 0.0, r.z as f32 + 0.5 - at.z).length() <= ROD_REACH)
        .max_by_key(|r| (r.y, -(r.x.abs() + r.z.abs()), r.x, r.z))
}

impl Game {
    /// Right-click on a cauldron with whatever's in hand. True if it did something.
    pub fn use_cauldron(&mut self, pos: IVec3) -> bool {
        let slot = self.inv.selected;
        let (held, wear) = (self.inv.held(), self.inv.wear[slot]);
        let Some(u) = cauldron_use(self.world.get_v(pos), held, wear) else { return false };
        self.player.swing = 1.0;
        if let Some(w) = u.wear {
            self.inv.wear[slot] = w;
            if crate::trims::dye_of(w).is_some() {
                self.advance("dye_job");
            }
        }
        if !self.creative {
            if u.take {
                self.inv.consume_held();
            }
            if let Some(g) = u.give {
                self.give(g, 1);
            }
        }
        if self.is_client() {
            self.net_send_msg(Msg::Interact { x: pos.x, y: pos.y, z: pos.z, item: held });
        } else {
            self.world.set_v(pos, u.block);
        }
        self.sfx(Sfx::Splash, Some(pos.as_vec3() + Vec3::splat(0.5)));
        true
    }

    /// A joined player used `item` on the cauldron at `pos`.
    pub fn host_cauldron(&mut self, from: u32, pos: IVec3, item: Id) {
        if !self.peer_near(from, pos) || self.verified_held(from) != item {
            return;
        }
        let ench = self.verified_ench(from);
        let block = self.world.get_v(pos);
        // The host can't see whether their armour is dyed: in plain water, take it that it is.
        let wear = if crate::trims::is_woolly(item) { crate::trims::with_dye(0, Some(0)) } else { (ench as Wear) << 16 };
        let Some(u) = cauldron_use(block, item, wear) else { return };
        if u.take && !self.peer_take(from, item, 1) {
            return;
        }
        if let Some(g) = u.give
            && !self.peer_free(from)
            && let Some(l) = self.ledger(from)
        {
            l.bag.add(g, 1);
        }
        if item == BANNER
            && let Some(w) = u.wear
        {
            let new = crate::enchant::enchants(w);
            if let Some(l) = self.ledger(from) {
                l.remove_enchanted(BANNER, ench);
                l.add_enchanted(BANNER, new, 1);
            }
            self.set_peer_held(from, BANNER, new);
        }
        self.world.set_v(pos, u.block);
        self.sfx(Sfx::Splash, Some(pos.as_vec3() + Vec3::splat(0.5)));
    }

    /// Right-click on a composter. True if it did something.
    pub fn use_composter(&mut self, pos: IVec3) -> bool {
        let id = self.world.get_v(pos);
        let held = self.inv.held();
        let full = id == COMPOSTER + 7;
        if !full && compost_chance(held).is_none() {
            return false;
        }
        self.player.swing = 1.0;
        if self.is_client() {
            if !full && !self.creative {
                self.inv.consume_held();
            }
            if full {
                self.advance("compost_happens");
            }
            self.net_send_msg(Msg::Interact { x: pos.x, y: pos.y, z: pos.z, item: held });
            return true;
        }
        if !full && !self.creative {
            self.inv.consume_held();
        }
        if let Some(out) = self.compost(pos, held) {
            self.give(out, 1);
            self.advance("compost_happens");
        }
        true
    }

    /// A joined player used `item` on the composter at `pos`.
    pub fn host_composter(&mut self, from: u32, pos: IVec3, item: Id) {
        if !self.peer_near(from, pos) {
            return;
        }
        let full = self.world.get_v(pos) == COMPOSTER + 7;
        if !full && (compost_chance(item).is_none() || !self.peer_take(from, item, 1)) {
            return;
        }
        if let Some(out) = self.compost(pos, item) {
            self.give_peer(from, out, 1);
        }
    }

    /// Put a scrap in (or take the compost out). Returns what comes out.
    pub fn compost(&mut self, pos: IVec3, item: Id) -> Option<Id> {
        let id = self.world.get_v(pos);
        if !is_composter(id) {
            return None;
        }
        let at = pos.as_vec3() + Vec3::new(0.5, 1.0, 0.5);
        if id == COMPOSTER + 7 {
            self.world.set_v(pos, COMPOSTER);
            self.sfx(Sfx::Hit(crate::sound::Mat::Grass), Some(at));
            return Some(COMPOST);
        }
        let chance = compost_chance(item)?;
        if self.rng.chance(chance) {
            self.world.set_v(pos, id + 1);
            self.smoke(at, 3, 0.15);
        }
        self.sfx(Sfx::Place(crate::sound::Mat::Grass), Some(at));
        None
    }

    /// Lightning about to strike `at`: the tallest rod in reach takes it
    /// instead (and powers up). Returns where it really strikes.
    pub fn rod_catch(&mut self, at: Vec3) -> Option<Vec3> {
        let rods: std::collections::HashSet<IVec3> = self.world.rods.iter().copied().filter(|p| self.world.is_loaded(p.x, p.z) && is_rod(self.world.get_v(*p))).collect();
        let rod = rod_for(&rods, at)?;
        self.world.set_v(rod, LIGHTNING_ROD_ON);
        self.buttons.insert(rod, ROD_SECS);
        if !self.away() && self.player.body.pos.distance(rod.as_vec3()) < ROD_REACH {
            self.advance("grounded");
        }
        Some(rod.as_vec3() + Vec3::new(0.5, 1.0, 0.5))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::tests::arena;

    #[test]
    fn cauldrons_fill_dye_and_wash() {
        assert_eq!(cauldron_state(CAULDRON), (0, None));
        for l in 1..=3 {
            assert_eq!(cauldron_state(cauldron_block(l, None)), (l, None));
            for c in 0..8 {
                assert_eq!(cauldron_state(cauldron_block(l, Some(c))), (l, Some(c)));
            }
        }
        assert!(is_cauldron(cauldron_block(3, Some(7))) && !is_cauldron(COMPOSTER));
        // The water shows, in its dye's colour.
        let top = |id: Id| crate::models::pieces(id).unwrap().iter().filter(|p| p.hide == !(1 << 2)).map(|p| p.tiles[2]).next();
        assert_eq!(top(CAULDRON), None);
        assert_eq!(top(cauldron_block(3, None)), Some(T_WATER));
        assert_eq!(top(cauldron_block(2, Some(2))), Some(T_DYED_WATER + 2));
        // Fill it, dye it, dip a helmet, wash it.
        let full = cauldron_use(CAULDRON, WATER_BUCKET, 0).expect("fills");
        assert_eq!((full.block, full.take, full.give), (cauldron_block(3, None), true, Some(BUCKET)));
        let red = cauldron_use(full.block, DYE_FIRST + 2, 0).expect("dyes");
        assert_eq!(red.block, cauldron_block(3, Some(2)));
        assert!(cauldron_use(red.block, DYE_FIRST + 2, 0).is_none(), "already red");
        let dip = cauldron_use(red.block, ARMOR_FIRST, crate::inventory::with_uses(0, 9)).expect("dips");
        let w = dip.wear.expect("dyed");
        assert_eq!((crate::trims::dye_of(w), crate::inventory::uses(w), dip.block, dip.take), (Some(2), 9, cauldron_block(2, Some(2)), false));
        assert!(cauldron_use(dip.block, ARMOR_FIRST, w).is_none(), "already that colour");
        assert!(cauldron_use(dip.block, ARMOR_FIRST + 4, 0).is_none(), "only woolly armour");
        let wash = cauldron_use(cauldron_block(1, None), ARMOR_FIRST, w).expect("washes");
        assert_eq!((wash.wear.map(crate::trims::dye_of), wash.block), (Some(None), CAULDRON));
        // Buckets and bottles.
        assert_eq!(cauldron_use(cauldron_block(3, None), BUCKET, 0).map(|u| (u.block, u.give)), Some((CAULDRON, Some(WATER_BUCKET))));
        assert!(cauldron_use(cauldron_block(2, None), BUCKET, 0).is_none(), "not full");
        assert_eq!(cauldron_use(cauldron_block(2, None), GLASS_BOTTLE, 0).map(|u| (u.block, u.give)), Some((cauldron_block(1, None), Some(WATER_BOTTLE))));
        assert_eq!(cauldron_use(CAULDRON, WATER_BOTTLE, 0).map(|u| u.block), Some(cauldron_block(1, None)));
        // A banner loses its top pattern.
        let d = crate::banners::Design::default().with(0, 2).with(1, 0).with(4, 1);
        let u = cauldron_use(cauldron_block(3, None), BANNER, (d.bits() as Wear) << 16).expect("washes");
        let back = crate::banners::Design::from_bits(crate::enchant::enchants(u.wear.unwrap()));
        assert_eq!(back, crate::banners::Design::default().with(0, 2).with(1, 0));
        assert!(cauldron_use(cauldron_block(3, None), BANNER, (crate::banners::Design::default().with(0, 2).bits() as Wear) << 16).is_none(), "nothing to wash");
    }

    #[test]
    fn using_a_cauldron_in_game() {
        let mut g = arena(901);
        let p = IVec3::new(2, 51, 2);
        g.world.set_v(p, CAULDRON);
        g.inv.slots[0] = Some((WATER_BUCKET, 1));
        g.inv.selected = 0;
        assert!(g.use_cauldron(p));
        assert_eq!(g.world.get_v(p), cauldron_block(3, None));
        assert_eq!(g.inv.count(BUCKET), 1);
        g.inv.slots[1] = Some((DYE_FIRST + 6, 2));
        g.inv.selected = 1;
        assert!(g.use_cauldron(p));
        assert_eq!((g.world.get_v(p), g.inv.count(DYE_FIRST + 6)), (cauldron_block(3, Some(6)), 1));
        g.inv.slots[2] = Some((ARMOR_FIRST + 1, 1));
        g.inv.selected = 2;
        assert!(g.use_cauldron(p));
        assert_eq!(crate::trims::dye_of(g.inv.wear[2]), Some(6));
        assert_eq!(g.world.get_v(p), cauldron_block(2, Some(6)));
    }

    #[test]
    fn composters_fill_up_and_give_compost() {
        let mut g = arena(902);
        let p = IVec3::new(2, 51, 2);
        g.world.set_v(p, COMPOSTER);
        assert!(compost_chance(STONE).is_none() && compost_chance(BREAD) > compost_chance(WHEAT_SEEDS));
        let mut out = 0;
        for _ in 0..200 {
            if g.compost(p, BREAD) == Some(COMPOST) {
                out += 1;
            }
        }
        assert!(out >= 15, "{out} compost from 200 bread");
        g.world.set_v(p, COMPOSTER + 7);
        g.inv.slots[0] = None;
        g.inv.selected = 0;
        assert!(g.use_composter(p));
        assert_eq!((g.world.get_v(p), g.inv.count(COMPOST)), (COMPOSTER, 1));
        g.inv.slots[0] = Some((STONE, 1));
        assert!(!g.use_composter(p), "stone doesn't compost");
    }

    #[test]
    fn rods_catch_lightning() {
        let mut g = arena(903);
        let low = IVec3::new(4, 52, 4);
        let high = IVec3::new(-6, 55, 3);
        g.world.set_v(low, LIGHTNING_ROD);
        g.world.set_v(high, LIGHTNING_ROD);
        assert!(g.world.rods.contains(&low) && g.world.rods.contains(&high));
        let hit = g.rod_catch(Vec3::new(10.0, 51.0, 10.0)).expect("caught");
        assert_eq!(hit, high.as_vec3() + Vec3::new(0.5, 1.0, 0.5), "the tallest rod");
        assert_eq!(g.world.get_v(high), LIGHTNING_ROD_ON);
        assert!(crate::wiring::source_on(LIGHTNING_ROD_ON));
        assert!(g.rod_catch(Vec3::new(500.0, 51.0, 0.0)).is_none(), "out of reach");
        // It goes back to plain copper.
        for _ in 0..30 {
            g.zap_tick(0.05);
        }
        assert_eq!(g.world.get_v(high), LIGHTNING_ROD);
        g.world.set_v(high, AIR);
        assert!(!g.world.rods.contains(&high));
    }

    #[test]
    fn heads_and_barding() {
        let mut rng = crate::noise::Rng::new(5);
        let heads = (0..4000).filter(|_| head_drop(crate::entity::MobKind::Groaner, &mut rng).is_some()).count();
        assert!((50..170).contains(&heads), "{heads} heads in 4000");
        assert!(head_drop(crate::entity::MobKind::Oinker, &mut rng).is_none());
        for l in 1..=3 {
            assert_eq!(barding_of(barding_item(l).unwrap()), l);
        }
        assert_eq!(barding_of(SADDLE), 0);
        assert!(is_head(HISSER_HEAD) && block(HISSER_HEAD).shape == Shape::Skull);
    }
}
