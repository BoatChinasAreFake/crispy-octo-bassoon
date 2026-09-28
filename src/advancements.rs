//! Advancements: little "you did a thing!" toasts, saved per world.
//! Any resemblance to another game's achievement system is entirely deliberate.

pub struct Advancement {
    /// Stable key written to saves.
    pub key: &'static str,
    pub title: &'static str,
    pub desc: &'static str,
}

const fn adv(key: &'static str, title: &'static str, desc: &'static str) -> Advancement {
    Advancement { key, title, desc }
}

pub const ALL: &[Advancement] = &[
    adv("getting_wood", "Getting Wood", "Punch a tree. It had it coming."),
    adv("benchmarking", "Benchmarking", "Craft a Decorative Crafting Table. You know it's decorative, right?"),
    adv("stone_age", "Stone Age", "Get Cobblestun. Civilisation begins."),
    adv("tool_time", "Tool Time", "Craft a pickaxe. Hitting rocks with rocks, but fancier."),
    adv("iron_will", "Iron Will", "Mine Iron Ore. Heavy metal!"),
    adv("dimonds", "DIMONDS!", "Find a Dimond. Spelled that way for legal reasons."),
    adv("fools_gold", "Fool's Gold", "Mine Gold Ore. It's useless. You did it anyway."),
    adv("golden_boy", "Golden Boy", "Eat a Suspiciously Golden Oinkchop. Your doctor is concerned."),
    adv("bacon", "Bringing Home the Bacon", "Defeat an Oinker. They trusted you."),
    adv("hiss_tory", "Hiss-tory", "Defeat a Hisser before it defeats your house."),
    adv("groan_up", "Groan Up", "Defeat a Groaner. Or wait for sunrise, that also works."),
    adv("fluffed", "Ethically Sourced", "Get Wool from a Fluffer. It was consensual. Probably."),
    adv("dont_blink", "Don't Blink", "Make eye contact with a Starer. Bold."),
    adv("staring_champ", "Staring Champion", "Defeat a Starer and take its pearl."),
    adv("rude_teleport", "Personal Space", "Throw a Stare Pearl. Arrive rudely."),
    adv("boing", "Boing!", "Bounce on a Bouncy Goo Block. Physics weeps."),
    adv("ouch", "Hug Life", "Hug a Pokey Plant. We did say not to."),
    adv("zoomies", "Zoomies", "Sprint across Ice. Nature's floor wax."),
    adv("sweet_dreams", "Sweet Dreams", "Sleep in a bed and skip the night. Time is a construct."),
    adv("cake", "The Cake Is Not a Lie", "Eat cake. It was real all along."),
    adv("spooky", "Spooky Scary", "Place a Jack o'Lantern. It's always October somewhere."),
    adv("thirsty", "Thirsty", "Place a Sponge in water. Glug."),
    adv("kaboom", "Kaboom", "Blow something up with TNT. Totally Not Trouble."),
    adv("centurion", "Centurion", "Break 100 blocks. Your hands must be so tired."),
    adv("why_cross", "Why Did the Cluckster Cross the Road?", "Get a Feather. The answer is still unclear."),
    adv("udderly", "Udderly Ridiculous", "Get Raw Moo-steak from a Mooer."),
    adv("bone_zone", "Bone Zone", "Defeat a Rattler. It had a bone to pick with you."),
    adv("arachno", "Arachno-no-bia", "Defeat a Webber. Eight legs, zero chances."),
    adv("split_decision", "Split Decision", "Defeat a Bloop. Watch it become several Bloops."),
    adv("robin_hood", "Robin Hood (Legally Distinct)", "Hit a mob with a Pointy Stick from a bow."),
    adv("green_thumb", "Green Thumb", "Harvest a fully grown crop. Farming simulator unlocked."),
    adv("crop_rotation", "Crop Rotation Enthusiast", "Harvest a crop grown where a different one grew before."),
    adv("soil_scientist", "Soil Scientist", "Use a Soil Probe. Nitrogen, phosphorus, potassium, bafflement."),
    adv("weed_whacker", "Weed Whacker", "Pull some weeds. Satisfying."),
    adv("hay_there", "Hay There", "Survive a long fall by landing on a Hay Bale."),
    adv("gone_fishin", "Gone Fishin'", "Catch anything at all with a Fishing Stick."),
    adv("one_that_got_away", "The One That Got Away", "Snap your line fighting a big fish. It was THIS big."),
    adv("bootiful", "Boot-iful", "Fish up a Soggy Boot. Just the left one."),
    adv("sunken_treasure", "Sunken Treasure", "Fish up something valuable."),
    adv("big_bob", "Big Bob", "Catch the legendary Big Bob. Dawn or dusk, deep ocean. Good luck."),
    adv("suit_up", "Suit Up", "Put on a piece of armour. Even the socks count."),
    adv("cover_me", "Cover Me in Dimonds", "Wear a full set of Dimond armour. Subtle."),
    adv("open_door_policy", "Open Door Policy", "Open a door. Then close it. Then open it again."),
    adv("butterfingers", "Butterfingers", "Throw something on the floor with Q. On purpose, surely."),
    adv("good_as_new", "Good as New(ish)", "Repair something at an anvil. Hit it until it's fixed."),
    adv("ominous", "Drops Ominously", "Use an anvil until it crumbles. It warned you."),
    adv("level_30", "Experienced", "Reach level 30. Nothing to spend it on but anvils. For now."),
];

pub fn find(key: &str) -> Option<&'static Advancement> {
    ALL.iter().find(|a| a.key == key)
}

#[derive(Default)]
pub struct Progress {
    /// Keys in the order they were earned.
    pub earned: Vec<String>,
}

impl Progress {
    pub fn from_keys(keys: &[String]) -> Progress {
        let mut p = Progress::default();
        for k in keys {
            // Unknown keys (from a newer version) are kept so they survive a save.
            if !p.has(k) {
                p.earned.push(k.clone());
            }
        }
        p
    }

    pub fn has(&self, key: &str) -> bool {
        self.earned.iter().any(|k| k == key)
    }

    /// Mark earned. Returns the advancement if it's new.
    pub fn grant(&mut self, key: &str) -> Option<&'static Advancement> {
        let a = find(key)?;
        if self.has(key) {
            return None;
        }
        self.earned.push(key.to_string());
        Some(a)
    }

    /// How many of the known advancements are earned.
    pub fn count(&self) -> usize {
        ALL.iter().filter(|a| self.has(a.key)).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_unique_and_grants_once() {
        for (i, a) in ALL.iter().enumerate() {
            assert!(ALL[i + 1..].iter().all(|b| b.key != a.key), "duplicate key {}", a.key);
        }
        let mut p = Progress::from_keys(&["getting_wood".into(), "from_the_future".into()]);
        assert_eq!(p.count(), 1);
        assert!(p.grant("getting_wood").is_none());
        assert!(p.grant("no_such_thing").is_none());
        assert_eq!(p.grant("dimonds").map(|a| a.title), Some("DIMONDS!"));
        assert_eq!(p.count(), 2);
        assert!(p.earned.contains(&"from_the_future".to_string()), "unknown keys survive");
    }
}
