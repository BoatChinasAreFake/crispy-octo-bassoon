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
    adv("benchmarking", "Benchmarking", "Craft something big at a Crafting Table. It has a job now."),
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
    adv("follow_the_fin", "Follow the Fin", "Feed a Dolphin a fish and let it show you the way."),
    adv("llama_drama", "Llama Drama", "Tame a Llama. It still might spit."),
    adv("zombie_doctor", "Zombie Doctor", "Cure a Zombie Hmmer with a Golden Chop."),
    adv("passing_trade", "Passing Trade", "Buy something from a Wanderer before it wanders off."),
    adv("riptide", "Spear Express", "Throw a Riptide spear in the rain or the sea, and go with it."),
    adv("ahoy", "Ahoy!", "Open a chest in a shipwreck. Finders keepers."),
    adv("x_marks_the_spot", "X Marks the Spot", "Dig up buried treasure. The map wasn't lying."),
    adv("hot_stuff", "Hot Stuff", "Get a bucket of lava. Or get into lava. One of those is a good idea."),
    adv("good_boy", "Who's a Good Boy?", "Tame a Woofer with a bone. It's you. You're the good boy now."),
    adv("the_birds_and_the_bees", "The Birds and the Bees", "Breed two animals. We won't ask how."),
    adv("its_alive", "It's Alive!", "Light a Zappy Lamp with a switch. Electricity: now legally distinct."),
    adv("what_a_deal", "What a Deal", "Trade with a Hmmer. Gold is finally good for something."),
    adv("not_today", "Not Today", "Block a hit with a shield. It's a door you can carry."),
    adv("portal_open", "Portal Opener", "Light an obsidian frame with a Sparker. Shimmery!"),
    adv("hotter", "We Need to Go Hotter", "Step into the Scorchlands. Bring a snack. And a bucket of regret."),
    adv("enchanter", "Enchanter", "Enchant something. The table knows words you don't."),
    adv("good_as_new", "Good as New(ish)", "Repair something at an anvil. Hit it until it's fixed."),
    adv("ominous", "Drops Ominously", "Use an anvil until it crumbles. It warned you."),
    adv("level_30", "Experienced", "Reach level 30. Nothing to spend it on but anvils. For now."),
    adv("eye_spy", "Eye Spy", "Fill a Crypt's ring of Eye Frames. Something opened."),
    adv("hollow", "The Hollow", "Step into the Hollow. It's very quiet. Too quiet."),
    adv("wyrm_slayer", "Wyrm Slayer", "Beat the Hollow Wyrm. Roll credits (there are none)."),
    adv("giddy_up", "Giddy Up", "Tame a Galloper. It only took several hundred attempts."),
    adv("hello_my_name_is", "Hello, My Name Is", "Name a mob with a Name Tag. It won't answer to it."),
    adv("brewmaster", "Local Brewery", "Drink a potion you brewed. Or found. We don't judge."),
    adv("lumberjack_reforms", "Reformed Lumberjack", "Grow a tree from a sapling with Bone Dust. Balance restored."),
    adv("welcome_to_the_jungle", "Welcome to the Jungle", "Visit a jungle. It's got fun and games (and melons)."),
    adv("swamp_thing", "Swamp Thing", "Visit a swamp. Mind the Bloops."),
    adv("stripy", "Earn Your Stripes", "Visit the badlands. Admire the terracotta. Don't lick it."),
    adv("needles", "Needle in a Haystack", "Visit a taiga. Spruce yourself up."),
    adv("village_people", "Village People", "Find a village. An actual one, this time."),
    adv("clank_you", "Clank You Very Much", "Watch a Clanker flatten a monster. It's on your side. Probably."),
    adv("pretty_polly", "Pretty Polly", "Feed a Squawker some seeds. It will tell everyone."),
    adv("melon_baller", "Melon Baller", "Eat a Melon Slice. Mostly water, entirely delicious."),
    adv("fire_starter", "Fire Starter", "Light a fire with a Sparker. Twisted, apparently."),
    adv("pushy", "Pushy", "Watch a piston push something. Personal space is a myth."),
    adv("freight", "Freight Train", "Load a Minecart with Chest. Choo choo, cargo."),
    adv("tattletale", "Tattletale", "Ride over a Detector Rail. It told everyone."),
    adv("beaconator", "Beaconator", "Stand in a beacon's light. The Wyrm's egg, finally useful."),
    adv("wings", "Look Ma, No Hands", "Glide with a Glider. Landing is a separate skill."),
    adv("rocket_man", "Rocket Man", "Boost a glide with a Boom Rocket. Burning out your fuse up here alone."),
    adv("boxed_in", "Bigger on the Inside", "Pick up a Hollow Box with things still in it."),
    adv("patina", "Statue Chic", "Watch copper turn fully green. Took its time."),
    adv("waxed", "Freeze Frame", "Wax some copper with Goo. It'll stay shiny forever. Probably."),
    adv("bamboozled", "Bamboozled", "Get some Bamboo. It grew while you read this."),
    adv("reef_madness", "Reef Madness", "Mine a block of coral. It was so colourful."),
    adv("soggy", "Soggy Bottom", "Defeat a Soggy Groaner. It was having a bad day anyway."),
    adv("spear_it", "Spear It Out", "Throw a Soggy Spear. Go and get it back."),
    adv("fishy_business", "Fishy Business", "Catch a Fishy with your bare hands (well, hit one)."),
    adv("sweet_success", "Sweet Success", "Harvest honey or honeycomb from a hive."),
    adv("ancient_honey", "Older Than Sweet", "Bottle Ancient honey (bees + Torchflowers)."),
    adv("new_colony", "Long Live the Queen", "Put a Queen Bee in an empty Beehive."),
    adv("swarm_catcher", "Swarm Catcher", "Have a swarm move into one of your empty hives."),
    adv("bee_careful", "Bee Careful", "Get stung. The smoker was right there."),
    adv("careful_hands", "Careful Hands", "Brush something out of Suspicious Sand or Gravel."),
    adv("pristine", "Not a Scratch", "Uncover a Pristine relic."),
    adv("restorer", "Elbow Grease", "Clean an Encrusted Relic at a Restoration Bench."),
    adv("collection_complete", "Museum Piece", "Complete a culture's collection in your Field Journal."),
    adv("curator", "The Curator", "Every relic, every culture, all Pristine. Museums weep."),
    adv("it_listens", "It Listens", "Wake The Hush. Bold choice."),
    adv("silence", "Silence", "Defeat The Hush. Shh."),
    adv("clean_slate", "Clean Slate", "Grind the enchantments off something."),
    adv("scorchite", "Hot Upgrade", "Upgrade Dimond gear to Scorchite."),
    adv("sly_friend", "Sly Friend", "Earn a Sneaker's trust with Cluckets."),
    adv("special_delivery", "Special Delivery", "Get a present from a trusting Sneaker."),
    adv("tongue_tied", "Tongue Tied", "Watch a Ribbit eat a Bloop."),
    adv("scute_cute", "Scute Cute", "Brush a Rollo for a scute."),
    adv("armoured_pup", "Very Good Boy", "Put Woofer Armour on your Woofer."),
    adv("plinky", "Plinky Plonky", "Tune a Note Block. Perfect pitch not required."),
    adv("now_playing", "Now Playing", "Put a Music Disc in a Jukebox. Turn it up."),
    adv("too_hot", "Too Hot to Handle", "Defeat a Sizzler. It was getting heated."),
    adv("dry_your_eyes", "Dry Your Eyes", "Defeat a Weeper. It's in a better place."),
    adv("return_to_sender", "Return to Sender", "Defeat a Weeper with its own fireball."),
    adv("fair_trade", "Fair Trade", "Barter with a Snout. Gold for... whatever that is."),
    adv("oinkstep", "Oinkstep", "Get the Oinkstep disc from a Snout. Bangers only."),
    adv("bad_omen", "Bad Omen", "Defeat a patrol captain. Villages are going to love you."),
    adv("hero_village", "Hero of the Village", "See off a raid. The Hmmers are very grateful (and give discounts)."),
    adv("rampage_over", "Rampage Over", "Defeat a Rampager. Large and in charge, until now."),
    adv("totem_saved", "Postponed", "Cheat death with a Totem of Not Dying."),
    adv("breeze_through", "Breeze Through", "Defeat a Breeze. It was full of hot air."),
    adv("under_lock", "Under Lock and Key", "Open a Vault with a Trial Key."),
    adv("wind_jump", "Up, Up and Away", "Launch yourself with a Wind Charge."),
    adv("toot_toot", "Toot Toot", "Get a Goat Horn the hard way: from a Goat that missed."),
    adv("bird_plane", "Is It a Bird?", "Look through a Spyglass."),
    adv("light_show", "Light Show", "Set off a firework from the ground."),
    adv("dressed_up", "Crafting a New Look", "Put a trim on some armour at a Smithing Table."),
    adv("smash", "Smash Hit", "Land a Mace blow on the way down. Gravity did most of the work."),
    adv("ominous_vault", "Ominous Outcome", "Open an Ominous Vault with an Ominous Trial Key."),
    adv("heartbreak", "Heartbreaker", "Break a Creaking Heart and see its Creaking crumble."),
    adv("ancient_seeds", "Planting the Past", "Plant a seed a Sniffer dug up."),
    adv("published", "Published Author", "Sign a book you wrote. A literary career begins."),
    adv("loomed", "Flying the Flag", "Pattern a banner at a Loom."),
    adv("homing_in", "Country Lode, Take Me Home", "Point a compass at a Lodestone."),
    adv("copper_golem", "Some Assembly Required", "Build a Copper Golem: a pumpkin on a block of copper."),
    adv("sorted", "Sorted", "Watch a Copper Golem put something away. Neatly."),
    adv("floaty_born", "Just Add Water", "Wake a Dried Floaty by putting it next to water."),
    adv("harnessed", "Strapped In", "Put a Harness on a grown-up Floaty."),
    adv("full_flight", "Full Flight", "Fill all four seats on a Floaty."),
    adv("resin_up", "Sticky Situation", "Knock some Resin out of a Creaking's heart."),
    adv("jousting", "Jousting Champion", "Hit something with a spear from a galloping mount."),
    adv("the_usual", "The Usual, Please", "Trade fifteen times with the same Hmmer. They know your order now."),
    adv("trading_hall", "Trading Hall", "Trade with a Farmer, a Librarian, a Smith and a Fisher."),
    adv("glow_up", "Glow Up", "Pick Glow Berries off a cave vine. Snack and nightlight in one."),
    adv("mind_your_head", "Mind Your Head", "Get hit by a falling Pointy Rock. It was on the label."),
    adv("crystal_clear", "Crystal Clear", "Get an Amethyst Shard out of a geode."),
    adv("stalac_tight", "Stalac-tight", "Find a dripstone cave. Look up. Then move."),
    adv("lush_life", "Lush Life", "Find a lush cave, where the moss is greener."),
    adv("pyramid_scheme", "Pyramid Scheme", "Find a desert pyramid. Watch your step."),
    adv("temple_run", "Temple Run", "Find a jungle temple. Watch your feet."),
    adv("off_the_rails", "Off the Rails", "Find an abandoned mineshaft. Watch out for webs."),
    adv("cold_feet", "Cold Feet", "Find an igloo. Watch the floor."),
    adv("monumental", "Monumental", "Find an Ocean Monument. Hold your breath."),
    adv("guardian_down", "Eye Contact", "Defeat a Guardian. It saw you first."),
    adv("elder_statesman", "Elder Statesman", "Defeat an Elder Guardian. Your arms will thank you."),
    adv("cursed", "Heavy Arms", "Get cursed with Mining Fatigue by an Elder Guardian."),
    adv("conduit_power", "Breathe Easy", "Feel a Conduit's power underwater."),
    adv("which_witch", "Which Witch?", "Defeat a Witch. Mind the bottles."),
    adv("local_flavour", "Local Flavour", "Defeat a desert Groaner or a snowy Rattler."),
    adv("fetch", "Fetch!", "Give an Allay something to fetch."),
    adv("char_broiled", "Char-Broiled", "Defeat a Charred Rattler."),
    adv("wilter_built", "Wilting Heights", "Build the Wilter. Why would you do that?"),
    adv("wilt_under_pressure", "Wilt Under Pressure", "Defeat the Wilter."),
    adv("star_power", "Star Power", "Set a Wilter Star in a beacon."),
    adv("propped_up", "Propped Up", "Stop a creaking ceiling from coming down."),
    adv("sinking_feeling", "That Sinking Feeling", "Be under a ceiling when it comes down."),
    adv("rope_a_dope", "Rope-a-Dope", "Throw a Spelunker's Rope down a hole."),
    adv("branching_out", "Branching Out", "Carry acacia, birch and dark oak logs at once."),
    adv("flat_tops", "Flat Tops", "Find a savanna, where the trees had a haircut."),
    adv("birch_please", "Birch, Please", "Find a birch forest."),
    adv("lights_out", "Lights Out", "Find a dark forest. Bring a torch."),
    adv("fungi_to_be_with", "Fungi to Be With", "Find a mushroom island."),
    adv("point_taken", "Point Taken", "Find the ice spikes."),
    adv("hay_fever", "Hay Fever", "Find a meadow."),
    adv("peak_performance", "Peak Performance", "Stand on a stony peak."),
    adv("fungus_amongus", "Fungus Among Us", "Shear the mushrooms off a Mushmooer."),
    adv("warm_welcome", "Warm Welcome", "Swim in a warm ocean."),
    adv("brr", "Brr", "Find a frozen ocean, icebergs and all."),
    adv("kelp_me", "Kelp Me", "Bring down a column of kelp."),
    adv("seeing_red", "Seeing Red", "Find a Crimson Forest down in the Scorchlands."),
    adv("teal_appeal", "Teal Appeal", "Find a Teal Forest. It's oddly calming."),
    adv("delta_force", "Delta Force", "Find the Basalt Deltas. Mind the ash."),
    adv("soul_searching", "Soul Searching", "Find a Soul Sand Valley."),
    adv("hot_foot", "Hot Foot", "Stand on a Magma Block without sneaking. Ow."),
    adv("fungal_growth", "Fungal Growth", "Grow a huge fungus with Bone Dust."),
    adv("pork_barrel", "Pork Barrel", "Defeat a Tusker."),
    adv("raising_hell", "Raising Hell", "Breed two Tuskers with Crimson Fungus."),
    adv("spore_loser", "Spore Loser", "Get a faceful of a Sporeling's spores."),
    adv("magma_carta", "Magma Carta", "Defeat a Magma Bloop."),
    adv("snuffed_out", "Snuffed Out", "Put out a Wisp for good."),
    adv("brute_force", "Brute Force", "Defeat a Snout Brute."),
    adv("gilded_age", "Gilded Age", "Open the treasure chest in a Snout Bastion."),
    adv("dye_job", "Dye Job", "Dip woolly armour in a cauldron of dye."),
    adv("compost_happens", "Compost Happens", "Get Compost out of a Composter."),
    adv("grounded", "Grounded", "Be near a Lightning Rod when it takes a strike."),
    adv("knight_errant", "Knight Errant", "Put armour on your Galloper."),
];

/// The advancements screen's tabs (everything not listed is an Adventure).
pub const TABS: [&str; 5] = ["All", "Getting Started", "Creatures", "Home and Craft", "Adventure"];
const STARTED: &[&str] = &[
    "getting_wood", "benchmarking", "stone_age", "tool_time", "iron_will", "dimonds", "fools_gold", "golden_boy", "boing", "ouch", "zoomies", "sweet_dreams", "cake", "spooky", "thirsty",
    "kaboom", "centurion", "why_cross", "suit_up", "cover_me", "open_door_policy", "butterfingers", "hot_stuff", "not_today",
    "branching_out",
];
const CREATURES: &[&str] = &[
    "bacon", "hiss_tory", "groan_up", "fluffed", "dont_blink", "staring_champ", "rude_teleport", "udderly", "bone_zone", "arachno", "split_decision", "robin_hood", "good_boy",
    "the_birds_and_the_bees", "giddy_up", "hello_my_name_is", "pretty_polly", "soggy", "spear_it", "fishy_business", "sly_friend", "special_delivery", "tongue_tied", "scute_cute",
    "armoured_pup", "too_hot", "guardian_down", "elder_statesman", "dry_your_eyes", "rampage_over", "breeze_through", "toot_toot", "heartbreak", "clank_you", "copper_golem", "sorted", "floaty_born", "harnessed",
    "full_flight", "jousting", "resin_up", "which_witch", "local_flavour", "fetch", "char_broiled", "fungus_amongus",
    "pork_barrel", "raising_hell", "spore_loser", "magma_carta", "snuffed_out", "knight_errant",
];
const HOME: &[&str] = &[
    "green_thumb", "crop_rotation", "soil_scientist", "weed_whacker", "hay_there", "gone_fishin", "one_that_got_away", "bootiful", "sunken_treasure", "big_bob", "its_alive",
    "what_a_deal", "brewmaster", "lumberjack_reforms", "melon_baller", "fire_starter", "pushy", "freight", "tattletale", "beaconator", "patina", "waxed", "bamboozled",
    "sweet_success", "ancient_honey", "new_colony", "swarm_catcher", "bee_careful", "plinky", "now_playing", "enchanter", "good_as_new", "published", "loomed", "homing_in",
    "boxed_in", "dressed_up", "the_usual", "trading_hall", "fair_trade", "ancient_seeds", "light_show", "bird_plane", "glow_up", "star_power", "kelp_me", "fungal_growth",
    "dye_job", "compost_happens", "grounded",
];

/// Which tab (1..) an advancement is on.
pub fn tab_of(key: &str) -> usize {
    if STARTED.contains(&key) {
        1
    } else if CREATURES.contains(&key) {
        2
    } else if HOME.contains(&key) {
        3
    } else {
        4
    }
}

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
    fn every_tab_names_real_advancements_and_each_has_one_tab() {
        for key in STARTED.iter().chain(CREATURES).chain(HOME) {
            assert!(find(key).is_some(), "no advancement {key}");
        }
        for a in ALL {
            let n = [STARTED, CREATURES, HOME].iter().filter(|l| l.contains(&a.key)).count();
            assert!(n <= 1, "{} is on two tabs", a.key);
        }
        assert!(ALL.iter().any(|a| tab_of(a.key) == 4), "some adventures");
    }

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

    #[test]
    fn every_advancement_the_game_awards_exists() {
        // Scan the source for advance("key") and advance_for(who, "key"): a typo would
        // otherwise just never award anything.
        // (Every .rs file under src/, folders and all.)
        let mut files = Vec::new();
        let mut dirs = vec![std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
        while let Some(d) = dirs.pop() {
            for entry in std::fs::read_dir(d).unwrap().flatten() {
                let p = entry.path();
                if p.is_dir() {
                    dirs.push(p);
                } else if p.extension().is_some_and(|e| e == "rs") {
                    files.push(p);
                }
            }
        }
        let mut used = Vec::new();
        for path in files {
            let text = std::fs::read_to_string(path).unwrap_or_default();
            for pat in ["advance(\"", "advance_for(who, \""] {
                for (at, _) in text.match_indices(pat) {
                    let rest = &text[at + pat.len()..];
                    if let Some(end) = rest.find('"') {
                        used.push(rest[..end].to_string());
                    }
                }
            }
        }
        assert!(used.len() > 40, "found only {} uses", used.len());
        for key in used.into_iter().filter(|k| k != "key") {
            assert!(find(&key).is_some(), "advancement {key:?} is awarded but not defined");
        }
        let mut keys: Vec<&str> = ALL.iter().map(|a| a.key).collect();
        keys.sort_unstable();
        let n = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), n, "duplicate keys");
    }
}
