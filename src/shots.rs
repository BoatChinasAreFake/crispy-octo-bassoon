//! The screenshot harness: `--screenshot out.png --mode M` sets up a scene
//! (a world, a screen, mobs or blocks to look at), holds the camera on it,
//! and saves a picture after `--frames` frames. Used to check how things
//! look (and for the README pictures); the playtest bots are in playtest.rs.

use crate::*;

/// Headless-ish verification helper: `--screenshot out.png [--mode title|survival|creative|inventory|night|options] [--frames N]`.
pub(crate) struct ShotArgs {
    pub(crate) path: String,
    pub(crate) mode: String,
    pub(crate) frames: u32,
    pub(crate) time: Option<f32>,
    pub(crate) yaw: f32,
    pub(crate) pitch: f32,
    pub(crate) pos: Option<Vec3>,
    /// Render distance in chunks (the default setting when absent).
    pub(crate) distance: Option<i32>,
    pub(crate) addr: String,
    pub(crate) password: String,
    pub(crate) chat: Vec<String>,
}

pub(crate) fn parse_args() -> Option<ShotArgs> {
    let args: Vec<String> = std::env::args().collect();
    let get = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    get("--screenshot").map(|path| ShotArgs {
        path,
        mode: get("--mode").unwrap_or_else(|| "title".into()),
        frames: get("--frames").and_then(|f| f.parse().ok()).unwrap_or(240),
        time: get("--time").and_then(|f| f.parse().ok()),
        distance: get("--distance").and_then(|f| f.parse().ok()),
        addr: get("--addr").unwrap_or_else(|| "127.0.0.1".into()),
        password: get("--password").unwrap_or_default(),
        chat: args.windows(2).filter(|w| w[0] == "--chat").map(|w| w[1].clone()).collect(),
        yaw: get("--yaw").and_then(|f| f.parse().ok()).unwrap_or(2.4),
        pitch: get("--pitch").and_then(|f| f.parse().ok()).unwrap_or(-0.25),
        pos: get("--pos").and_then(|p| {
            let v: Vec<f32> = p.split(',').filter_map(|x| x.parse().ok()).collect();
            (v.len() == 3).then(|| Vec3::new(v[0], v[1], v[2]))
        }),
    })
}


/// Scenes down in the Scorchlands (`scenic_view` finds them in generator
/// coordinates; see dims.rs).
fn scorch_scene(mode: &str) -> bool {
    matches!(mode, "fortress" | "camp" | "bastion" | "crimson" | "teal" | "basalt" | "soulvalley")
}

pub(crate) fn scenic_view(g: &Game, mode: &str) -> Option<(Vec3, f32, f32)> {
    use structures::Kind;
    let scorch = scorch_scene(mode).then(|| world::Generator::with_dim(g.world.seed(), g.world.generator.opts, crate::dims::Dim::Scorch));
    let generator = scorch.as_ref().unwrap_or(&*g.world.generator);
    let (cx0, cz0) = ((g.spawn.x / 16.0).floor() as i32, (g.spawn.z / 16.0).floor() as i32);
    let ring = |r: i32| (-r..=r).flat_map(move |dz| (-r..=r).map(move |dx| (dx, dz))).filter(move |(dx, dz)| dx.abs().max(dz.abs()) == r);
    let look = |from: Vec3, to: Vec3| {
        let d = to - from;
        (from, d.x.atan2(-d.z), d.y.atan2(Vec2::new(d.x, d.z).length()))
    };
    // The Scorchlands' structures are a long way east.
    let (cx0, cz0) = if matches!(mode, "fortress" | "camp" | "bastion") { (crate::scorch::SCORCH_ORIGIN / 16, 0) } else { (cx0, cz0) };
    let kind = match mode {
        "outpost" => Kind::Outpost,
        "trials" => Kind::TrialChambers,
        "fortress" => Kind::Fortress,
        "camp" => Kind::SnoutCamp,
        "bastion" => Kind::Bastion,
        "raid" => Kind::Village,
        "hut" => Kind::Hut,
        "tower" => Kind::Tower,
        "well" => Kind::Well,
        "dungeon" => Kind::Dungeon,
        "village" => Kind::Village,
        "city" => Kind::HushedCity,
        "ruins" => Kind::DesertRuins,
        "trailruins" => Kind::TrailRuins,
        "oceanruins" => Kind::OceanRuins,
        "shipwreck" => Kind::Shipwreck,
        "pyramid" => Kind::DesertPyramid,
        "jungletemple" => Kind::JungleTemple,
        "mineshaft" => Kind::Mineshaft,
        "igloo" => Kind::Igloo,
        "monument" => Kind::Monument,
        "dripstone" | "lush" => {
            // Standing in a roomy cave in that cave biome, looking along it.
            let (want, ground) = if mode == "dripstone" { (caves::CaveBiome::Dripstone, DRIPSTONE_BLOCK) } else { (caves::CaveBiome::Lush, MOSS_BLOCK) };
            for r in 0..120 {
                for (dx, dz) in ring(r) {
                    let (cx, cz) = (cx0 + dx, cz0 + dz);
                    let inside = (-1..=1).all(|i| (-1..=1).all(|j| generator.cave_biome(cx * 16 + 8 + i * 28, cz * 16 + 8 + j * 28) == want));
                    if !inside {
                        continue;
                    }
                    let b = generator.generate(cx, cz);
                    for lz in 4..12 {
                        for lx in 2..9 {
                            let h = generator.column(cx * 16 + lx, cz * 16 + lz).0;
                            let at = |x: i32, y: i32, z: i32| b[world::idx(x, y, z)];
                            for y in (10..h - 10).rev() {
                                let roomy = (0..4).all(|k| (0..6).all(|d| at(lx + d, y + k, lz) == AIR));
                                if roomy && at(lx, y - 1, lz) == ground {
                                    return Some((Vec3::new((cx * 16 + lx) as f32 + 0.5, y as f32 + 1.6, (cz * 16 + lz) as f32 + 0.5), 1.57, -0.05));
                                }
                            }
                        }
                    }
                }
            }
            return None;
        }
        "crimson" | "teal" | "basalt" | "soulvalley" => {
            // Down in the Scorchlands, well inside the biome, on a floor with room to look about.
            let want = match mode {
                "crimson" => wilds::ScorchBiome::CrimsonForest,
                "teal" => wilds::ScorchBiome::TealForest,
                "basalt" => wilds::ScorchBiome::BasaltDeltas,
                _ => wilds::ScorchBiome::SoulValley,
            };
            let (ox, oz) = (crate::scorch::SCORCH_ORIGIN, 0);
            for r in 0..300 {
                for (dx, dz) in ring(r) {
                    let (x, z) = (ox + dx * 16 + 8, oz + dz * 16 + 8);
                    let inside = (-1..=1).all(|i| (-1..=1).all(|j| generator.scorch_biome(x + i * 20, z + j * 20) == want));
                    if !inside {
                        continue;
                    }
                    // A wide floor: the same floor a few blocks either way along the view.
                    let Some(y) = generator.scorch_floor(x, z, crate::scorch::LAVA_SEA + 4, 9) else { continue };
                    let wide = (2..=8).step_by(3).all(|d| generator.scorch_floor(x - d, z + d, y - 2, 6).is_some_and(|y2| (y2 - y).abs() <= 3));
                    // (Not standing in a huge fungus.)
                    let clear = (-4..=4).all(|i| (-4..=4).all(|j| generator.fungus_at(x + i, z + j).is_none()));
                    if wide && clear {
                        return Some((Vec3::new(x as f32 + 0.5, y as f32 + 2.5, z as f32 + 0.5), -0.785, -0.12));
                    }
                }
            }
            return None;
        }
        "kelp" => {
            // Under the sea in a kelp forest, looking across the floor.
            for r in 0..200 {
                for (dx, dz) in ring(r) {
                    let (x, z) = ((cx0 + dx) * 16 + 8, (cz0 + dz) * 16 + 8);
                    let (h, biome) = generator.column(x, z);
                    let deep = generator.sea() - h;
                    let kelpy = (-2..=2).filter(|k| crate::seas::floor_plant(generator, biome, x + k * 3, z, deep).is_some_and(|p| p.0 == block::KELP)).count();
                    if biome.is_ocean() && deep >= 8 && kelpy >= 2 {
                        return Some((Vec3::new(x as f32 + 0.5, h as f32 + 3.0, z as f32 + 0.5), 0.7, 0.05));
                    }
                }
            }
            return None;
        }
        "geode" => {
            // Inside the nearest geode, looking at its wall.
            let (rx0, rz0) = (cx0 * 16 / 56, cz0 * 16 / 56);
            for r in 0..40 {
                for (dx, dz) in ring(r) {
                    if let Some(geode) = generator.geode_in(rx0 + dx, rz0 + dz) {
                        let c = geode.centre.as_vec3() + Vec3::new(0.5, 0.0, 0.5);
                        return Some((c, 0.6, -0.25));
                    }
                }
            }
            return None;
        }
        "deepdark" => {
            // Inside a Deep Dark cavern, standing on its floor.
            for r in 0..200 {
                for (dx, dz) in ring(r) {
                    let (x, z) = ((cx0 + dx) * 16 + 8, (cz0 + dz) * 16 + 8);
                    if !generator.deep_dark(x, z) || generator.column(x, z).0 < deepdark::DEEP_TOP + 8 {
                        continue;
                    }
                    for y in 5..deepdark::DEEP_TOP - 6 {
                        let open = (0..4).all(|k| generator.deep_cavern(x, y + k, z)) && !generator.deep_cavern(x, y - 1, z);
                        if open {
                            return Some((Vec3::new(x as f32 + 0.5, y as f32 + 1.6, z as f32 + 0.5), 0.7, -0.15));
                        }
                    }
                }
            }
            return None;
        }
        "beenest" => {
            // An oak with a nest on it (the same rule the generator uses).
            for r in 0..120 {
                for (dx, dz) in ring(r) {
                    for k in 0..256 {
                        let (tx, tz) = ((cx0 + dx) * 16 + k % 16, (cz0 + dz) * 16 + k / 16);
                        let Some((h, trunk, kind)) = generator.tree_at(tx, tz) else { continue };
                        let biome = generator.column(tx, tz).1;
                        if kind == world::TreeKind::Oak && noise::hash2(generator.seed ^ 0xBEE, tx, tz) < 0.07 && matches!(biome, world::Biome::Plains | world::Biome::Forest) {
                            let nest = Vec3::new(tx as f32 + 0.5, (h + (trunk - 3).max(1)) as f32 + 0.5, tz as f32 + 0.5);
                            return Some(look(nest + Vec3::new(4.5, 0.5, 4.5), nest));
                        }
                    }
                }
            }
            return None;
        }
        "swamp" | "jungle" | "badlands" | "taiga" | "cherry" | "mangrove" | "palegarden" | "aurora" | "autumn" | "rainbow" | "savanna" | "birchforest" | "darkforest" | "mushroomisland" | "icespikes" | "meadow" | "stonypeaks" | "warmocean" | "frozenocean" => {
            let want = match mode {
                "warmocean" => world::Biome::WarmOcean,
                "frozenocean" => world::Biome::FrozenOcean,
                "savanna" => world::Biome::Savanna,
                "birchforest" => world::Biome::BirchForest,
                "darkforest" => world::Biome::DarkForest,
                "mushroomisland" => world::Biome::MushroomIslands,
                "icespikes" => world::Biome::IceSpikes,
                "meadow" => world::Biome::Meadow,
                "stonypeaks" => world::Biome::StonyPeaks,
                "aurora" => world::Biome::Snowy,
                "autumn" => world::Biome::Forest,
                "rainbow" => world::Biome::Plains,
                "swamp" => world::Biome::Swamp,
                "jungle" => world::Biome::Jungle,
                "badlands" => world::Biome::Badlands,
                "cherry" => world::Biome::Cherry,
                "palegarden" => world::Biome::PaleGarden,
                "mangrove" => world::Biome::Mangrove,
                _ => world::Biome::Taiga,
            };
            // Somewhere well inside the biome (all nine columns around agree), looking across it.
            // (Islands are small and dark forests patchy: closer in will do for them.)
            let spread = if matches!(want, world::Biome::MushroomIslands | world::Biome::DarkForest) { 10 } else { 24 };
            let lift = if want == world::Biome::StonyPeaks { 4.0 } else { 12.0 };
            for r in 0..400 {
                for (dx, dz) in ring(r) {
                    let (x, z) = ((cx0 + dx) * 16 + 8, (cz0 + dz) * 16 + 8);
                    let inside = (-1..=1).all(|i| (-1..=1).all(|j| generator.column(x + i * spread, z + j * spread).1 == want));
                    if inside {
                        let h = generator.column(x, z).0.max(generator.sea());
                        return Some((Vec3::new(x as f32 + 0.5, h as f32 + lift, z as f32 + 0.5), 0.8, -0.35));
                    }
                }
            }
            return None;
        }
        "ocean" => {
            // Above deep-ish open sea.
            for r in 0..160 {
                for (dx, dz) in ring(r) {
                    let (x, z) = ((cx0 + dx) * 16 + 8, (cz0 + dz) * 16 + 8);
                    let (h, biome) = generator.column(x, z);
                    if biome.is_ocean() && h < generator.sea() - 6 && !generator.cold(x, z) {
                        return Some((Vec3::new(x as f32 + 0.5, generator.sea() as f32 - 3.0, z as f32 + 0.5), 0.8, -0.4));
                    }
                }
            }
            return None;
        }
        "ravine" | "snow" | "rain" | "thunder" => {
            for r in 0..60 {
                for (dx, dz) in ring(r) {
                    let (x, z) = ((cx0 + dx) * 16 + 8, (cz0 + dz) * 16 + 8);
                    let (h, biome) = generator.column(x, z);
                    let open = generator.site(cx0 + dx, cz0 + dz).is_none() && h > generator.sea() + 2;
                    if mode == "snow" {
                        if biome == world::Biome::Snowy && open {
                            return Some((Vec3::new(x as f32 + 0.5, h as f32 + 8.0, z as f32 + 0.5), 2.4, -0.35));
                        }
                    } else if mode != "ravine" {
                        let clear = (-3..=3).all(|i| (-3..=3).all(|j| generator.tree_at(x + i, z + j).is_none()));
                        if biome == world::Biome::Plains && open && clear {
                            return Some((Vec3::new(x as f32 + 0.5, h as f32 + 3.0, z as f32 + 0.5), 2.4, -0.1));
                        }
                    } else if generator.ravine_floor(x, z).is_some() && h > generator.sea() + 4 {
                        return Some((Vec3::new(x as f32 + 0.5, h as f32 + 14.0, z as f32 + 0.5), 2.4, -1.1));
                    }
                }
            }
            return None;
        }
        _ => return None,
    };
    for r in 0..60 {
        for (dx, dz) in ring(r) {
            let Some(site) = generator.site(cx0 + dx, cz0 + dz).filter(|s| s.kind == kind) else { continue };
            let o = site.origin.as_vec3() + Vec3::new(0.5, 0.0, 0.5);
            return Some(match kind {
                Kind::Dungeon => look(o + Vec3::new(2.6, 2.4, 2.6), o + Vec3::new(-3.0, 0.5, -1.0)),
                Kind::Tower => look(o + Vec3::new(-11.0, 9.0, -11.0), o + Vec3::Y * 4.0),
                Kind::Village => look(o + Vec3::new(-20.0, 18.0, -20.0), o + Vec3::Y * 2.0),
                Kind::HushedCity => look(o + Vec3::new(-7.0, 4.5, -9.0), o + Vec3::new(0.0, 3.0, 0.0)),
                Kind::Fortress => look(o + Vec3::new(1.5, 2.6, -21.5), o + Vec3::new(0.5, 4.0, 0.5)),
                Kind::SnoutCamp => look(o + Vec3::new(-9.0, 6.0, -10.0), o + Vec3::Y * 1.5),
                // From the ramparts' corner, over the courtyard to the treasure room.
                Kind::Bastion => look(o + Vec3::new(13.5, 9.5, 13.5), o + Vec3::new(0.0, 2.0, -4.0)),
                Kind::Outpost => look(o + Vec3::new(-14.0, 10.0, -14.0), o + Vec3::Y * 7.0),
                Kind::TrialChambers => look(o + Vec3::new(-5.5, 5.5, -5.5), o + Vec3::new(2.0, 1.0, 2.0)),
                Kind::DesertRuins | Kind::TrailRuins | Kind::OceanRuins => look(o + Vec3::new(-7.0, 7.0, -7.0), o + Vec3::new(1.0, 0.0, 0.0)),
                Kind::Shipwreck => look(o + Vec3::new(-8.0, 3.0, -6.0), o + Vec3::Y * 1.0),
                Kind::DesertPyramid => look(o + Vec3::new(-15.0, 9.0, 17.0), o + Vec3::Y * 3.0),
                Kind::JungleTemple => look(o + Vec3::new(-6.0, 15.0, 11.0), o + Vec3::Y * 3.0),
                Kind::Mineshaft => look(o + Vec3::new(0.5, 2.6, 0.5), o + Vec3::new(0.5, 2.2, -14.0)),
                Kind::Igloo => look(o + Vec3::new(-7.0, 5.0, 9.0), o + Vec3::Y * 2.0),
                Kind::Monument => look(o + Vec3::new(-11.0, 10.0, 18.0), o + Vec3::Y * 5.0),
                _ => look(o + Vec3::new(-8.0, 6.0, -8.0), o + Vec3::Y * 1.5),
            });
        }
    }
    None
}


impl App {
    /// Set up the scene for screenshot mode `s.mode`.
    pub(crate) fn shot_setup(&mut self, s: &mut ShotArgs) {
        let app = self;
        match s.mode.as_str() {
            "survival" | "creative" | "inventory" | "night" | "death" => {
                // --seed N shows someone else's world (to look at a reported bug).
                let seed = std::env::args().collect::<Vec<_>>().windows(2).find(|w| w[0] == "--seed").and_then(|w| w[1].parse().ok()).unwrap_or(424242);
                let mut g = shot_game(seed, s.mode == "creative", false);
                if s.mode == "inventory" {
                    for (item, n) in [(LOG, 12), (COBBLE, 20), (COAL, 5), (IRON, 3), (DIAMOND, 2), (GUNPOWDER, 5), (SAND, 9), (PORKCHOP, 3), (block::stairs(1, 0), 8), (block::slab(0, false), 12), (block::DOOR, 2)] {
                        g.inv.add(item, n);
                    }
                    g.inv.add(block::ARMOR_FIRST + 8, 1);
                    g.inv.armor[1] = Some((block::ARMOR_FIRST + 4 + 1, 1));
                    g.inv.armor[3] = Some((block::ARMOR_FIRST + 3, 1));
                    g.inv.armor_wear[1] = 150;
                    // Some well-used tools.
                    for (item, wear) in [(block::PICK_IRON, 60), (block::SWORD_STONE, 110), (block::BOW, 20)] {
                        g.inv.add(item, 1);
                        let i = g.inv.slots.iter().position(|s| *s == Some((item, 1))).unwrap();
                        g.inv.wear[i] = wear;
                    }
                    g.player.hunger.food = 13.0;
                    g.inv.offhand = Some((block::TORCH, 16));
                }
                if let Some(t) = s.time {
                    g.time = t;
                }
                g.start_scripts();
                app.start_game(g);
                app.show_debug = true;
                if s.mode == "inventory" {
                    app.set_screen(Screen::Inventory);
                }
            }
            "host" => {
                app.start_game(shot_game(424242, true, false));
                app.game.open_lan("Hosty", None).expect("open to LAN");
                app.show_debug = true;
            }
            "showcase" => {
                app.start_game(shot_game(424242, true, false));
                app.game.open_lan("Hosty", None).expect("open to LAN");
                app.show_debug = false;
            }
            "parody" => {
                app.start_game(shot_game(424242, true, false));
                app.show_debug = false;
            }
            "farm" | "fish" | "kitchen" | "chest" | "chests" | "bigchest" | "furnace" | "building" | "armour" | "anvil" | "rules" | "xp" | "enchant" | "table" | "liquids" | "zappy" | "trade" | "vehicles" | "decor" | "carpentry" | "brewing" | "contraptions" | "machines" | "underworks" | "woods" | "models" | "newblocks" | "glider" | "homecraft" => {
                let mut g = shot_game(424242, matches!(s.mode.as_str(), "farm" | "newblocks"), false);
                g.time = s.time.unwrap_or(0.2);
                if s.mode == "fish" {
                    g.inv.slots[0] = Some((block::ROD, 1));
                }
                app.start_game(g);
                app.show_debug = false;
            }
            "hut" | "tower" | "well" | "dungeon" | "village" | "ravine" | "rain" | "thunder" | "snow" | "swamp" | "jungle" | "badlands" | "taiga" | "cherry" | "mangrove" | "palegarden" | "city" | "ruins" | "trailruins" | "oceanruins" | "shipwreck" | "deepdark" | "dripstone" | "lush" | "geode" | "pyramid" | "jungletemple" | "mineshaft" | "igloo" | "monument" | "beenest" | "outpost" | "fortress" | "camp" | "raid" | "trials" | "aurora" | "autumn" | "rainbow" | "savanna" | "birchforest" | "darkforest" | "mushroomisland" | "icespikes" | "meadow" | "stonypeaks" | "warmocean" | "frozenocean" | "kelp" | "crimson" | "teal" | "basalt" | "soulvalley" | "bastion" => {
                // Somewhere the generator built something (or the sky is doing something).
                let mut g = shot_game(424242, true, false);
                g.time = s.time.unwrap_or(if s.mode == "aurora" { 0.8 } else { 0.3 });
                if s.mode == "aurora" {
                    // Already out (it fades in over a while otherwise).
                    g.aurora = 1.0;
                }
                match s.mode.as_str() {
                    "autumn" => {
                        g.rules.seasons = true;
                        g.day = seasons::SEASON_DAYS * 2 + 3;
                    }
                    "rainbow" => {
                        // (A little while after the rain, so it's fully in.)
                        g.rainbow = skies::RAINBOW_SECS - 20.0;
                        g.time = s.time.unwrap_or(0.38);
                    }
                    // A Turtle Shell, to see further underwater.
                    "monument" => g.inv.armor[0] = Some((block::TURTLE_SHELL, 1)),
                    _ => {}
                }
                let kind = match s.mode.as_str() {
                    "rain" | "snow" => Some(weather::Weather::Rain),
                    "thunder" => Some(weather::Weather::Thunder),
                    _ => None,
                };
                if let Some(k) = kind {
                    g.weather.kind = k;
                    g.weather.strength = 1.0;
                    g.rules.weather_cycle = false;
                }
                if let Some((mut pos, yaw, pitch)) = scenic_view(&g, &s.mode) {
                    if scorch_scene(&s.mode) {
                        pos.x -= crate::scorch::SCORCH_ORIGIN as f32;
                        g.move_local_player(crate::dims::Dim::Scorch, pos);
                    }
                    (s.pos, s.yaw, s.pitch) = (Some(pos), yaw, pitch);
                }
                // Look up at the sky: north for an aurora, away from the sun for a rainbow.
                if s.mode == "aurora" {
                    (s.yaw, s.pitch) = (0.0, 0.45);
                } else if s.mode == "rainbow" {
                    let a = g.sun_angle();
                    let anti = -Vec3::new(a.cos(), a.sin(), 0.25);
                    (s.yaw, s.pitch) = (anti.x.atan2(-anti.z), 0.2);
                }
                app.start_game(g);
                app.show_debug = false;
            }
            "cave" | "caverain" => {
                // Standing in a roomy cave pocket near spawn, well below the surface.
                let mut g = shot_game(424242, true, false);
                g.time = s.time.unwrap_or(0.3);
                if s.mode == "caverain" {
                    // A storm overhead must stay overhead.
                    g.weather.kind = weather::Weather::Rain;
                    g.weather.strength = 1.0;
                    g.rules.weather_cycle = false;
                }
                let (cx0, cz0) = ((g.spawn.x / 16.0).floor() as i32, (g.spawn.z / 16.0).floor() as i32);
                for cz in cz0 - 4..=cz0 + 4 {
                    for cx in cx0 - 4..=cx0 + 4 {
                        g.world.load_now(cx, cz);
                    }
                }
                let air = |g: &Game, x: i32, y: i32, z: i32| g.world.get(x, y, z) == AIR;
                let mut found = None;
                'search: for r in 0i32..56 {
                    for dz in -r..=r {
                        for dx in -r..=r {
                            if dx.abs().max(dz.abs()) != r {
                                continue;
                            }
                            let (x, z) = (cx0 * 16 + 8 + dx, cz0 * 16 + 8 + dz);
                            let top = (0..world::CH).rev().find(|&y| !air(&g, x, y, z)).unwrap_or(0);
                            for y in (14..top - 8).rev() {
                                let roomy = (-1..=1).all(|i| (-1..=1).all(|k| (0..3).all(|h| air(&g, x + i, y + h, z + k))));
                                let floor = is_solid(g.world.get(x, y - 1, z)) && (0..=8).all(|i| !is_liquid(g.world.get(x + i, y - 1, z)));
                                let far = (3..9).filter(|d| air(&g, x + d, y + 1, z)).count() >= 4;
                                if roomy && floor && far {
                                    found = Some(Vec3::new(x as f32 + 0.5, y as f32 + 1.6, z as f32 + 0.5));
                                    break 'search;
                                }
                            }
                        }
                    }
                }
                if s.pos.is_none() {
                    s.pos = found;
                    s.yaw = std::f32::consts::FRAC_PI_2;
                    s.pitch = -0.2;
                }
                app.start_game(g);
                app.show_debug = false;
            }
            "scorch" | "portal" => {
                // Through a portal (built on the spot), or looking at one.
                let mut g = shot_game(424242, s.mode == "scorch", false);
                g.time = s.time.unwrap_or(0.3);
                let spawn = g.spawn;
                let here = IVec3::new(spawn.x.floor() as i32, spawn.y.floor() as i32, spawn.z.floor() as i32);
                if s.mode == "scorch" {
                    let (dim, to) = g.travel(here);
                    g.move_local_player(dim, to);
                    if s.pos.is_none() {
                        s.pos = Some(to + Vec3::new(0.0, 0.0, 1.0));
                        s.yaw = std::f32::consts::PI;
                        s.pitch = -0.15;
                    }
                } else {
                    let (cx, cz) = (here.x.div_euclid(16), here.z.div_euclid(16));
                    for dz in -1..=1 {
                        for dx in -1..=1 {
                            g.world.load_now(cx + dx, cz + dz);
                        }
                    }
                    let base = here + IVec3::new(0, 0, -6);
                    let base = IVec3::new(base.x, g.world.surface_y(base.x, base.z) + 1, base.z);
                    scorch::build_portal(&mut g.world, base);
                    if s.pos.is_none() {
                        s.pos = Some(base.as_vec3() + Vec3::new(1.0, 1.2, 6.0));
                        s.yaw = 0.0;
                        s.pitch = -0.05;
                    }
                }
                app.start_game(g);
                app.show_debug = false;
            }
            "hollow" => {
                // On the island's edge, looking in at the pillars (and whatever circles them).
                let mut g = shot_game(424242, true, false);
                let (dim, to) = g.hollow_destination(IVec3::ZERO);
                g.move_local_player(dim, to);
                let o = hollow::ORIGIN.as_vec3();
                g.world.load_now(hollow::ORIGIN.x.div_euclid(16), 0);
                g.alloc_mob(entity::MobKind::Wyrm, o + Vec3::new(20.0, 24.0, -10.0));
                if s.pos.is_none() {
                    s.pos = Some(to + Vec3::Y * 2.0);
                    s.yaw = -std::f32::consts::FRAC_PI_2;
                    s.pitch = 0.12;
                }
                app.start_game(g);
                app.show_debug = false;
            }
            "zoo" | "animals" | "newmobs" | "music" | "modzoo" | "banners" | "golems" | "homestead" | "chat" => {
                // Every mob in two rows, in daylight unless --time says otherwise, in creative (so nobody attacks).
                let mut g = shot_game(424242, true, false);
                g.time = s.time.unwrap_or(0.2);
                app.start_game(g);
                app.show_debug = false;
            }
            "advancements" => {
                let mut g = shot_game(424242, false, false);
                for a in advancements::ALL.iter().step_by(3) {
                    g.advancements.grant(a.key);
                }
                app.start_game(g);
                app.set_screen(Screen::Advancements);
            }
            "portrait" | "beds" => {
                // Mobs (--mob a,b,c) on a plain stone floor, close up, for checking their looks.
                let mut g = shot_game(424242, true, false);
                g.time = s.time.unwrap_or(0.25);
                let base = g.spawn.floor().as_ivec3();
                for dz in -1..=1 {
                    for dx in -1..=1 {
                        g.world.load_now(base.x.div_euclid(16) + dx, base.z.div_euclid(16) + dz);
                    }
                }
                let ground = g.world.surface_y(base.x, base.z);
                for z in -14..=14 {
                    for x in -14..=14 {
                        for y in ground + 1..ground + 14 {
                            g.world.set(base.x + x, y, base.z + z, AIR);
                        }
                        g.world.set(base.x + x, ground, base.z + z, STONE);
                    }
                }
                g.mobs.clear();
                let o = Vec3::new(base.x as f32 + 0.5, ground as f32 + 1.0, base.z as f32 + 0.5);
                let back = std::env::args().collect::<Vec<_>>().windows(2).find(|w| w[0] == "--back").and_then(|w| w[1].parse::<f32>().ok()).unwrap_or(5.0);
                // (`from` is where the feet go; the eye is 1.62 above.)
                let from = o + Vec3::new(0.3 * back, 0.3 * back - 1.62, -back);
                let d = o + Vec3::Y * (0.5 + 0.1 * back) - (from + Vec3::Y * 1.62);
                (s.pos, s.yaw, s.pitch) = (Some(from), d.x.atan2(-d.z), d.y.atan2(Vec2::new(d.x, d.z).length()));
                g.player.flying = true;
                app.start_game(g);
                app.show_debug = false;
            }
            "shadowtest" => {
                // A floating platform over flat ground: its shadow should land just beside it.
                let mut g = shot_game(424242, true, false);
                g.time = s.time.unwrap_or(0.1);
                let base = g.spawn.floor().as_ivec3();
                for dz in -1..=1 {
                    for dx in -1..=1 {
                        g.world.load_now(base.x.div_euclid(16) + dx, base.z.div_euclid(16) + dz);
                    }
                }
                let ground = g.world.surface_y(base.x, base.z);
                for z in -12..=12 {
                    for x in -12..=12 {
                        for y in ground + 1..ground + 12 {
                            g.world.set(base.x + x, y, base.z + z, AIR);
                        }
                        g.world.set(base.x + x, ground, base.z + z, STONE);
                    }
                }
                for z in -2..=2 {
                    for x in -2..=2 {
                        g.world.set(base.x + x, ground + 5, base.z + z, GOLD_ORE);
                    }
                }
                let o = Vec3::new(base.x as f32 + 0.5, ground as f32 + 1.0, base.z as f32 + 0.5);
                let from = o + Vec3::new(-9.0, 10.0, -9.0);
                let d = o - from;
                (s.pos, s.yaw, s.pitch) = (Some(from), d.x.atan2(-d.z), d.y.atan2(Vec2::new(d.x, d.z).length()));
                g.player.flying = true;
                app.start_game(g);
                app.show_debug = false;
            }
            "recipes" => {
                let mut g = shot_game(424242, false, false);
                for (item, n) in [(LOG, 12), (PLANKS, 20), (COBBLE, 20), (IRON, 4), (STICK, 6), (COAL, 5), (WHEAT, 6), (HONEYCOMB, 3), (FEATHER, 2), (COPPER_INGOT, 2)] {
                    g.inv.add(item, n);
                }
                g.learn_inventory();
                g.pinned = recipes().iter().position(|r| r.output.0 == BEEHIVE);
                app.start_game(g);
                app.set_screen(Screen::Inventory);
                app.book_search = "pick".into();
            }
            "journal" | "beelog" => {
                let mut g = shot_game(424242, false, false);
                for (i, c) in [(0, 0), (1, 2), (2, 1), (5, 0), (7, 3), (8, 1), (11, 0)] {
                    g.journal.note(SHARD_FIRST + i, c);
                }
                for (i, c) in [(0, 0), (2, 1), (3, 2), (9, 0), (10, 0), (11, 1)] {
                    g.journal.note(RELIC_FIRST + i, c);
                }
                for i in archaeology::Culture::Hushed.shards() {
                    g.journal.note(SHARD_FIRST + *i as Id, 0);
                }
                g.journal.xp = 140;
                g.journal.digs = 31;
                g.journal.cracked = 9;
                g.journal.shattered = 2;
                g.journal.restored = 3;
                g.bee_log = bees::BeeLog { xp: 95, bottles: [14, 6, 3, 8, 1], comb: 21, stings: 4, swarms_caught: 2, swarms_lost: 1, queens: 3, colonies_started: 2 };
                app.start_game(g);
                app.journal_back = Screen::Paused;
                app.set_screen(if s.mode == "journal" { Screen::Journal } else { Screen::BeeLog });
            }
            "bench" | "grindstone" => {
                let mut g = shot_game(424242, false, false);
                for (item, n) in [(UPGRADE_TEMPLATE, 2), (SCORCHITE_INGOT, 3), (PICK_DIAMOND, 1), (SWORD_DIAMOND, 1), (ARMOR_FIRST + 13, 1)] {
                    g.inv.add(item, n);
                }
                let bench = if s.mode == "bench" { smithing::Bench::Smithing } else { smithing::Bench::Grindstone };
                // A real one to stand at (the screen closes if it isn't there).
                let at = g.spawn.floor().as_ivec3() + IVec3::new(2, 0, 0);
                g.world.load_now(at.x.div_euclid(16), at.z.div_euclid(16));
                g.world.set_v(at, bench.block());
                app.start_game(g);
                app.game.open_bench(at, bench);
                if let Some(b) = &mut app.game.bench {
                    if bench == smithing::Bench::Smithing {
                        b.slots = [Some((UPGRADE_TEMPLATE, 1)), Some((PICK_DIAMOND, 1)), Some((SCORCHITE_INGOT, 1))];
                        b.wear[1] = enchant::with_level(120, enchant::Enchant::Efficiency, 4);
                    } else {
                        b.slots[0] = Some((SWORD_IRON, 1));
                        b.wear[0] = enchant::with_level(enchant::with_level(30, enchant::Enchant::Sharpness, 3), enchant::Enchant::Unbreaking, 2);
                    }
                }
                app.screen = Screen::Bench;
            }
            "stats" => {
                let mut g = shot_game(424242, false, false);
                g.stats = stats::Stats { mined: 1843, placed: 1207, crafted: 311, kills: 58, deaths: 3, damage_dealt: 402.0, damage_taken: 131.0, walked: 18_420.0, swum: 960.0, flown: 0.0, ridden: 2_310.0, jumps: 4_107, played: 3.0 * 3600.0 + 1260.0, fish: 12, eaten: 96 };
                app.start_game(g);
                app.set_screen(Screen::Stats);
            }
            "mods" => {
                app.game = shot_game(424242, true, true);
                app.set_screen(Screen::Mods);
            }
            "worlds" => {
                app.game = shot_game(424242, true, true);
                app.open_worlds();
            }
            "backups" => {
                // A saved world with a few backups made over the last days.
                let (root, id) = (save::saves_dir(), "castle-town");
                let _ = save::write_name(&root, id, "Castle Town");
                app.game = shot_game(424242, false, false);
                app.current_world = Some(id.into());
                let _ = app.write_current_world();
                app.game = shot_game(424242, true, true);
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                for hours in [50, 26, 3] {
                    let _ = backups::back_up(&root, &backups::backups_dir(), id, now - hours * 3600);
                }
                app.open_worlds();
                app.world_sel = app.worlds.iter().position(|w| w.id == id);
                app.backup_list = backups::list(&backups::backups_dir(), id);
                app.backup_sel = Some(0);
                app.set_screen(Screen::Backups);
            }
            "createform" => {
                app.game = shot_game(424242, true, true);
                app.form_name = "Cheese Kingdom".into();
                app.form_seed = "cheese".into();
                app.form_focus = 1;
                app.set_screen(Screen::CreateWorld);
            }
            "newworld" => {
                // Goes through the real "Create World" path.
                app.form_name = "Harness Test World".into();
                app.form_seed = "cheese".into();
                app.form_creative = true;
                app.create_world();
                app.show_debug = true;
            }
            "palette" => {
                app.start_game(shot_game(424242, true, false));
                app.set_screen(Screen::Inventory);
            }
            "internet" => {
                app.start_game(shot_game(424242, true, false));
                app.mp_password = "sekrit".into();
                app.open_to_internet();
                app.set_screen(Screen::Paused);
                app.pause_page = crate::PausePage::Share;
            }
            "pause" => {
                app.start_game(shot_game(424242, true, false));
                app.set_screen(Screen::Paused);
            }
            "join" => {
                app.mp_name = "Joiny".into();
                app.mp_password = s.password.clone();
                app.connect_next = Some(s.addr.clone());
                app.set_screen(Screen::Multiplayer);
            }
            "options" => {
                app.game = shot_game(424242, true, true);
                app.set_screen(Screen::Options { from_title: true });
            }
            "video" => {
                app.game = shot_game(424242, true, true);
                app.settings.max_fps = 144;
                app.settings.particles = 1;
                app.settings.shadow_quality = 3;
                app.set_screen(Screen::Video { from_title: true });
            }
            "reef" => {
                // Under a warm sea, somewhere with coral (found once the chunks are in).
                let mut g = shot_game(424242, true, false);
                g.time = s.time.unwrap_or(0.25);
                if let Some((pos, yaw, pitch)) = scenic_view(&g, "ocean") {
                    (s.pos, s.yaw, s.pitch) = (Some(pos), yaw, pitch);
                }
                app.start_game(g);
                app.show_debug = false;
            }
            "spire" => {
                // One of the Hollow's outer islands, from a little way off.
                let mut g = shot_game(424242, true, false);
                g.time = 0.25;
                let seed = g.world.seed();
                let spire = (1..12).flat_map(|r: i32| (-r..=r).flat_map(move |i| [(i, -r), (i, r), (-r, i), (r, i)])).find_map(|(gx, gz)| hollow::outer_island(seed, gx, gz).filter(|i| i.2));
                if let Some((c, _, _)) = spire {
                    // (The island is in generator coordinates; the Hollow's own are shifted; see dims.rs.)
                    let c = c - IVec3::new(hollow::GEN_ORIGIN.x, 0, 0);
                    let tall = hollow::spire_height(g.world.generator.opts.version >= 3) as f32;
                    let from = c.as_vec3() + Vec3::new(18.0, tall * 0.6, 18.0);
                    let d = c.as_vec3() + Vec3::new(0.5, tall * 0.45, 0.5) - from;
                    g.move_local_player(crate::dims::Dim::Hollow, from);
                    (s.pos, s.yaw, s.pitch) = (Some(from), d.x.atan2(-d.z), d.y.atan2(Vec2::new(d.x, d.z).length()));
                }
                g.player.flying = true;
                app.start_game(g);
                app.show_debug = false;
            }
            "controls" => {
                app.game = shot_game(424242, true, true);
                // Show a changed binding and one waiting for a key.
                app.settings.binds.set(keybinds::Action::Sprint, true, keybinds::Bind::parse("F"));
                app.set_screen(Screen::Controls { from_title: true });
                app.rebinding = Some((keybinds::Action::Drop, true));
            }
            _ => {
                app.game = shot_game(424242, true, true);
            }
        }
    }

    /// Each frame of a screenshot run: keep the camera put, and stage whatever the mode needs.
    pub(crate) fn shot_frame(&mut self, s: &ShotArgs, frames: u32) {
        let app = self;
            if !matches!(s.mode.as_str(), "title" | "inventory" | "join" | "internet" | "pause" | "mods" | "palette" | "worlds" | "newworld" | "createform") || (s.mode == "join" && app.game.is_client()) {
                // Keep the demo camera looking at something interesting.
                app.game.player.pitch = s.pitch;
                app.game.player.yaw = s.yaw;
                if let Some(p) = s.pos {
                    app.game.player.body.pos = p;
                    app.game.player.body.vel = Vec3::ZERO;
                    app.game.player.flying = true;
                }
            }
            if frames >= 150 && frames < 150 + s.chat.len() as u32 {
                // Type the scripted chat lines one per frame.
                let line = s.chat[(frames - 150) as usize].clone();
                app.game.send_chat(&line);
            }
            if (s.mode == "showcase" || s.mode == "parody") && frames == 120 {
                // A little display of every mod block (or the newest base ones), on stone plinths in front of the player.
                let p = app.game.player.body.pos;
                let (fx, fz) = (2.4f32.sin(), -2.4f32.cos());
                let r = block::reg();
                let ids = if s.mode == "parody" { block::GOLD_ORE..block::NUM_BLOCKS } else { block::NUM_BLOCKS..r.blocks.len() as block::Id };
                let n = ids.len() as f32;
                for (i, id) in ids.enumerate() {
                    let side = if s.mode == "parody" { (i as f32 - (n - 1.0) / 2.0) * 1.3 } else { i as f32 * 1.6 - 3.0 };
                    let at = p + Vec3::new(fx * 5.0 - fz * side, 0.0, fz * 5.0 + fx * side);
                    let (x, y, z) = (at.x.floor() as i32, at.y.floor() as i32, at.z.floor() as i32);
                    app.game.world.set(x, y, z, block::STONE);
                    app.game.world.set(x, y + 1, z, id);
                }
            }
            if s.mode == "parody" && frames == 140 {
                app.game.advance("dimonds");
            }
            if matches!(s.mode.as_str(), "zoo" | "newmobs" | "modzoo" | "music" | "animals" | "banners" | "golems" | "homestead" | "farm" | "fish" | "kitchen" | "chest" | "chests" | "bigchest" | "furnace" | "building" | "armour" | "anvil" | "rules" | "xp" | "enchant" | "table" | "liquids" | "zappy" | "trade" | "vehicles" | "decor" | "carpentry" | "brewing" | "contraptions" | "machines" | "underworks" | "woods" | "models" | "newblocks" | "homecraft") && frames == 120 {
                // A flat, clear stone floor in front of the camera.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let y0 = p.y.floor() as i32 - 1;
                for x in p.x as i32 - 26..p.x as i32 + 26 {
                    for z in p.z as i32 - 26..p.z as i32 + 26 {
                        let d = Vec3::new(x as f32 + 0.5, p.y, z as f32 + 0.5) - p;
                        if !(1.0..22.0).contains(&d.dot(fwd)) || d.dot(right).abs() > 15.0 {
                            continue;
                        }
                        app.game.world.set(x, y0, z, block::STONE);
                        for y in y0 + 1..y0 + 14 {
                            app.game.world.set(x, y, z, block::AIR);
                        }
                    }
                }
            }
            if s.mode == "banners" && frames == 125 {
                // Three banners up in front, a lectern and a loom, and a painted shield in hand.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let y0 = p.y.floor() as i32;
                let at = |f: f32, r: f32| {
                    let v = p + fwd * f + right * r;
                    IVec3::new(v.x.floor() as i32, y0, v.z.floor() as i32)
                };
                let designs = [
                    banners::Design::default().with(0, 2).with(1, 0).with(4, 4),
                    banners::Design::default().with(0, 6).with(3, 0),
                    banners::Design::default().with(0, 1).with(5, 3).with(6, 4),
                ];
                let facing = banners::facing_from_yaw(s.yaw);
                for (k, d) in designs.iter().enumerate() {
                    let q = at(5.0, (k as f32 - 1.0) * 2.0);
                    app.game.world.set_v(q, block::BANNER);
                    app.game.put_up_banner(q, d.bits(), facing);
                }
                app.game.world.set_v(at(3.0, -2.5), block::LECTERN_BOOK);
                app.game.world.set_v(at(3.0, 2.5), block::LOOM);
                app.game.inv.slots[0] = Some((block::SHIELD, 1));
                app.game.inv.selected = 0;
                app.game.shield_banner = designs[1].bits();
                // A wall of framed maps behind them.
                let f = if fwd.x.abs() > fwd.z.abs() { IVec3::new(fwd.x.signum() as i32, 0, 0) } else { IVec3::new(0, 0, fwd.z.signum() as i32) };
                if let Some(facing) = decor::frame_facing(-f) {
                    for (k, up) in [(-1, 1), (0, 1), (-1, 2), (0, 2)] {
                        let wall = at(8.0, k as f32) + IVec3::Y * up;
                        app.game.world.set_v(wall, block::STONE_BRICKS);
                        let frame = wall - f;
                        app.game.world.set_v(frame, block::FRAME_FIRST + facing as u16);
                        app.game.world.frames.insert(frame, (block::MAP, 0));
                    }
                }
            }
            if s.mode == "homestead" && frames == 125 {
                // The v0.1.20 home blocks: a campfire cooking, a smoker and a blast
                // furnace going, a barrel, a dressed armour stand and paintings on a wall.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let y0 = p.y.floor() as i32;
                let at = |f: f32, r: f32| {
                    let v = p + fwd * f + right * r;
                    IVec3::new(v.x.floor() as i32, y0, v.z.floor() as i32)
                };
                let fire = at(4.0, 0.0);
                app.game.world.set_v(fire, block::CAMPFIRE);
                app.game.put_on_campfire(fire, block::PORKCHOP);
                app.game.put_on_campfire(fire, block::COD);
                app.game.world.set_v(at(6.0, -3.0), block::SMOKER_LIT);
                app.game.world.set_v(at(6.0, -2.0), block::BLAST_FURNACE_LIT);
                app.game.world.set_v(at(6.0, 2.0), block::BARREL);
                app.game.world.set_v(at(6.0, 2.0) + IVec3::Y, block::BARREL);
                let facing = (((s.yaw / std::f32::consts::FRAC_PI_2).round() as i32 + 2).rem_euclid(4)) as u16;
                let stand = at(3.5, 2.5);
                app.game.world.set_v(stand, block::ARMOUR_STAND_FIRST + facing);
                let mut c = containers::Container::for_block(block::ARMOUR_STAND_FIRST);
                c.slots[0] = Some((block::TURTLE_SHELL, 1));
                c.slots[1] = Some((block::ARMOR_FIRST + 12 + block::CHESTPLATE as u16, 1));
                c.slots[2] = Some((block::ARMOR_FIRST + 8 + block::LEGGINGS as u16, 1));
                c.slots[3] = Some((block::ARMOR_FIRST + 4 + block::BOOTS as u16, 1));
                app.game.world.containers.insert(stand, c);
                // A wall at the back with three paintings on it.
                let f = if fwd.x.abs() > fwd.z.abs() { IVec3::new(fwd.x.signum() as i32, 0, 0) } else { IVec3::new(0, 0, fwd.z.signum() as i32) };
                if let Some(facing) = decor::frame_facing(-f) {
                    for r in -4..=4 {
                        for up in 0..4 {
                            app.game.world.set_v(at(10.0, r as f32) + IVec3::Y * up, block::PLANKS);
                        }
                    }
                    for r in [-3, 0, 3] {
                        app.game.world.set_v(at(10.0, r as f32) + IVec3::Y * 2 - f, block::PAINTING_FIRST + facing as u16);
                    }
                    // Torches on the wall between the paintings.
                    for r in [-1.5, 1.5] {
                        app.game.world.set_v(at(10.0, r) + IVec3::Y * 2 - f, block::WALL_TORCH_FIRST + facing as u16);
                    }
                }
                // v0.1.21's building blocks along the left.
                for c in 0..4u16 {
                    app.game.world.set_v(at(3.0 + c as f32, -8.0), block::CONCRETE_FIRST + c * 2);
                    app.game.world.set_v(at(3.0 + c as f32, -8.0) + IVec3::Y, block::GLAZED_FIRST + c * 2 + 1);
                }
                app.game.world.set_v(at(2.5, -4.5), block::CANDLE_LIT);
                app.game.world.set_v(at(2.5, -3.5), block::CANDLE);
                for up in 0..3 {
                    app.game.world.set_v(at(8.0, -8.0) + IVec3::Y * up, block::SCAFFOLDING);
                }
                app.game.world.set_v(at(9.5, 1.0) + IVec3::Y * 3, block::CHAIN);
                app.game.world.set_v(at(9.5, 1.0) + IVec3::Y * 2, block::LANTERN_HANGING);
                app.game.inv.slots[0] = Some((block::TREASURE_MAP, 1));
                app.game.inv.wear[0] = treasure::mark(at(40.0, 10.0));
                app.game.inv.selected = 0;
            }
            if s.mode == "beds" && frames >= 60 {
                // Four beds, every way round, and us asleep in one (seen from outside).
                let back = 5.0;
                let o = s.pos.unwrap_or_default() - Vec3::new(0.3 * back, 0.3 * back - 1.62, -back);
                let base = o.floor().as_ivec3();
                for f in 0..4u8 {
                    app.game.world.set_v(base + IVec3::new(f as i32 * 2 - 3, 0, 0), beds::bed(f));
                }
                let b = base + IVec3::new(-1, 0, 0);
                app.game.sleeping = Some(beds::Sleep { bed: b, secs: 0.5 });
                app.game.third_person = true;
            }
            if s.mode == "portrait" && frames + 3 == s.frames {
                // The mobs, side by side, facing the camera.
                let names = std::env::args().collect::<Vec<_>>().windows(2).find(|w| w[0] == "--mob").map(|w| w[1].clone()).unwrap_or_else(|| "hmmer".into());
                let kinds: Vec<entity::MobKind> = names.split(',').filter_map(entity::MobKind::from_name).collect();
                let back = std::env::args().collect::<Vec<_>>().windows(2).find(|w| w[0] == "--back").and_then(|w| w[1].parse::<f32>().ok()).unwrap_or(5.0);
                let o = s.pos.unwrap_or_default() - Vec3::new(0.3 * back, 0.3 * back - 1.62, -back);
                let mut rng = noise::Rng::new(5);
                for (i, kind) in kinds.iter().enumerate() {
                    let x = (i as f32 - (kinds.len() as f32 - 1.0) / 2.0) * 2.2;
                    let mut m = entity::Mob::new(*kind, o + Vec3::new(x, 0.0, 0.0), &mut rng);
                    m.id = 3000 + i as u32;
                    m.yaw = -0.8;
                    if let Some(v) = std::env::args().collect::<Vec<_>>().windows(2).find(|w| w[0] == "--variant").and_then(|w| w[1].parse::<u8>().ok()) {
                        m.variant = v;
                    }
                    app.game.mobs.push(m);
                }
            }
            if s.mode == "chat" && frames == 60 {
                // A long chat, open and scrolled back a little.
                for i in 0..40 {
                    app.game.msg(format!("<Player{}> chat line number {i}", i % 3));
                }
                app.game.msg("<Player1> and here's a much longer message, the kind somebody types when they're explaining exactly where they left the diamonds, which wraps onto the next line instead of running off the edge of the screen".to_string());
                app.chat = Some("/g".into());
                app.chat_options = Some((app.game.complete_command("/g"), usize::MAX));
                app.chat_scroll = 0;
            }
            if s.mode == "homestead" && frames == 150 {
                use entity::MobKind as K;
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let mut rng = noise::Rng::new(12);
                for (i, (kind, f, r)) in [(K::Llama, 7.0, -6.0), (K::Panda, 8.0, -2.0), (K::PolarBear, 8.5, 1.5), (K::Turtle, 4.5, -3.0), (K::Dolphin, 9.0, 0.0), (K::Wanderer, 7.0, 4.0), (K::ZombieHmmer, 6.5, -4.0)].into_iter().enumerate() {
                    let mut m = entity::Mob::new(kind, p + fwd * f + right * r, &mut rng);
                    m.id = 2200 + i as u32;
                    m.saddled = kind == K::Llama;
                    m.persistent = true;
                    app.game.mobs.push(m);
                }
            }
            if s.mode == "golems" && frames == 125 {
                // A Copper Golem between its chests, a harnessed Floaty, a saddled
                // Rotsteed, and the new ground covers, with an iron spear in hand.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let y0 = p.y.floor() as i32;
                let at = |f: f32, r: f32| {
                    let v = p + fwd * f + right * r;
                    IVec3::new(v.x.floor() as i32, y0, v.z.floor() as i32)
                };
                app.game.world.set_v(at(4.0, -3.0), block::COPPER_CHEST);
                app.game.world.set_v(at(4.0, 3.0), block::CHEST);
                app.game.world.set_v(at(3.0, -5.5), block::FIREFLY_BUSH);
                app.game.world.set_v(at(3.0, 5.5), block::EYEBLOSSOM_OPEN);
                app.game.world.set_v(at(9.0, -6.0), block::RESIN_BRICKS);
                app.game.world.set_v(at(9.0, -6.0) + IVec3::Y, block::RESIN_BLOCK);
                for f in 5..8 {
                    for r in -6..-2 {
                        app.game.world.set_v(at(f as f32, r as f32), block::LEAF_LITTER);
                    }
                    for r in 3..7 {
                        app.game.world.set_v(at(f as f32, r as f32), block::WILDFLOWERS);
                    }
                }
                app.game.inv.slots[0] = Some((block::SPEAR_FIRST + 2, 1));
                app.game.inv.selected = 0;
            }
            if s.mode == "golems" && frames == 150 {
                use entity::MobKind as K;
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let mut rng = noise::Rng::new(11);
                for (i, (kind, f, r, up)) in [(K::CopperGolem, 4.0, 0.0, 0.0), (K::Rotsteed, 7.0, -2.5, 0.0), (K::Floaty, 13.0, 2.0, 2.5)].into_iter().enumerate() {
                    let mut m = entity::Mob::new(kind, p + fwd * f + right * r + Vec3::Y * up, &mut rng);
                    m.yaw = s.yaw + std::f32::consts::PI + if kind == K::Rotsteed { 1.2 } else { 0.0 };
                    m.id = 2100 + i as u32;
                    m.saddled = kind != K::CopperGolem;
                    if kind == K::CopperGolem {
                        m.seed = block::RESIN_CLUMP as u32 | 5 << 16;
                    }
                    m.persistent = true;
                    app.game.mobs.push(m);
                }
            }
            if s.mode == "farm" && frames == 125 {
                // A little farm: three crops at every stage, watered down the middle,
                // with a Scarecrow, and the new decorative blocks along the back.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let y0 = p.y.floor() as i32 - 1;
                let at = |f: f32, r: f32| {
                    let v = p + fwd * f + right * r;
                    (v.x.floor() as i32, v.z.floor() as i32)
                };
                let crops = [block::WHEAT_0, block::CARROT_0, block::POTATO_0];
                for (row, base) in crops.iter().enumerate() {
                    for col in 0..8 {
                        let (x, z) = at(4.0 + row as f32, col as f32 - 3.5);
                        let water = col == 4;
                        app.game.world.set(x, y0, z, if water { block::WATER } else { block::FARMLAND_WET });
                        if !water {
                            let stage = (col.min(7) as u16 / 2).min(3);
                            app.game.world.set(x, y0 + 1, z, base + stage);
                        }
                    }
                }
                let (x, z) = at(3.0, 5.5);
                app.game.world.set(x, y0 + 1, z, block::SCARECROW);
                let showcase = [block::SANDSTONE, block::STONE_BRICKS, block::MOSSY_COBBLE, block::HAY, block::BOOKSHELF, block::LANTERN, block::MUSHROOM, block::WEEDS];
                for (i, id) in showcase.into_iter().enumerate() {
                    let (x, z) = at(9.0, i as f32 * 1.2 - 4.2);
                    if matches!(id, block::MUSHROOM | block::WEEDS) {
                        app.game.world.set(x, y0, z, block::GRASS);
                    }
                    app.game.world.set(x, y0 + 1, z, id);
                }
            }
            if s.mode == "fish" && frames == 125 {
                // A pond, and a big one on the line.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let y0 = p.y.floor() as i32 - 1;
                for x in p.x as i32 - 14..p.x as i32 + 14 {
                    for z in p.z as i32 - 14..p.z as i32 + 14 {
                        let d = Vec3::new(x as f32 + 0.5, p.y, z as f32 + 0.5) - p;
                        if (3.0..12.0).contains(&d.dot(fwd)) && d.dot(right).abs() < 4.5 {
                            for y in y0 - 2..=y0 {
                                app.game.world.set(x, y, z, block::WATER);
                            }
                        }
                    }
                }
            }
            if s.mode == "fish" && frames >= 160 {
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let spot = Vec3::new(p.x, p.y.floor() - 0.1, p.z) + fwd * 8.0;
                app.game.bobber = Some(fishing::Bobber {
                    pos: spot,
                    vel: Vec3::ZERO,
                    state: fishing::BobberState::Biting { window: 1.0 },
                    fight: Some(fishing::Fight::new(0.55, 0.8)),
                    since_nibble: 9.0,
                    bait: false,
                });
            }
            if s.mode == "zoo" && frames == 150 {
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let mut rng = noise::Rng::new(9);
                for (i, kind) in entity::MobKind::ALL.into_iter().enumerate() {
                    // The six newest in front, the originals behind (offset so they show between).
                    let (row, col) = (1 - (i / 6).min(1), i % 6);
                    let off = if row == 1 { 0.5 } else { 0.0 };
                    let at = p + fwd * (6.0 + row as f32 * 5.0) + right * ((col as f32 - 2.5 + off) * 2.6) + Vec3::Y * 2.0;
                    let mut m = entity::Mob::new(kind, at, &mut rng).with_size(if kind == entity::MobKind::Bloop { 2 } else { 1 });
                    m.yaw = s.yaw + std::f32::consts::PI;
                    m.id = 1000 + i as u32;
                    app.game.mobs.push(m);
                }
            }
            if matches!(s.mode.as_str(), "chests" | "bigchest") && frames == 125 {
                // Every kind of chest in a row, the diamond one open.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let y = p.y.floor() as i32;
                let at = |f: f32, r: f32| {
                    let v = p + fwd * f + right * r;
                    IVec3::new(v.x.floor() as i32, y, v.z.floor() as i32)
                };
                let row = [block::CHEST, block::COPPER_CHEST, block::IRON_CHEST, block::GOLD_CHEST, block::DIAMOND_CHEST];
                for (i, &id) in row.iter().enumerate() {
                    app.game.world.set_v(at(3.5, i as f32 * 1.2 - 2.4), id);
                }
                let big = at(3.5, 4.0 * 1.2 - 2.4);
                if let Some(c) = app.game.world.containers.get_mut(&big) {
                    for (i, item) in [block::DIAMOND, block::GOLD_INGOT, block::IRON, block::BREAD, block::COAL, block::LOG, block::COBBLE].into_iter().enumerate() {
                        c.slots[i * 9 + 2] = Some((item, 32 + i as u8 * 4));
                    }
                }
                if s.mode == "bigchest" {
                    app.game.open = Some(big);
                    app.set_screen(Screen::Container);
                }
            }
            if s.mode == "chests" && frames >= 126 {
                // Hold the diamond chest's lid open (nobody's using it).
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let v = p + fwd * 3.5 + right * (4.0 * 1.2 - 2.4);
                let big = IVec3::new(v.x.floor() as i32, p.y.floor() as i32, v.z.floor() as i32);
                if !app.game.lids.contains_key(&big) {
                    chests::set_lifted(big, true);
                    app.game.world.dirty.insert((big.x.div_euclid(16), big.z.div_euclid(16)));
                }
                app.game.lids.insert(big, chests::Lid { open: 1.0, yaw: s.yaw + std::f32::consts::PI });
            }
            if matches!(s.mode.as_str(), "kitchen" | "chest" | "furnace") && frames == 125 {
                // A chest, a table and a row of furnaces (the middle one busy), a few blocks ahead.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let y = p.y.floor() as i32;
                let at = |f: f32, r: f32| {
                    let v = p + fwd * f + right * r;
                    IVec3::new(v.x.floor() as i32, y, v.z.floor() as i32)
                };
                let row = [block::CHEST, block::TABLE, block::FURNACE, block::FURNACE, block::FURNACE, block::CHEST];
                for (i, &id) in row.iter().enumerate() {
                    app.game.world.set_v(at(4.0, i as f32 - 2.5), id);
                }
                let busy = at(4.0, 3.0 - 2.5);
                if let Some(c) = app.game.world.containers.get_mut(&busy) {
                    c.slots[containers::INPUT] = Some((block::PORKCHOP, 5));
                    c.slots[containers::FUEL] = Some((block::COAL, 3));
                    c.slots[containers::OUTPUT] = Some((block::COOKED_CHOP, 2));
                }
                let chest = at(4.0, -2.5);
                if let Some(c) = app.game.world.containers.get_mut(&chest) {
                    let loot = [(block::DIAMOND, 7), (block::COBBLE, 64), (block::LOG, 23), (block::BREAD, 4), (block::BOOT, 1), (block::COAL, 18), (block::GOLD_INGOT, 3), (block::WHEAT_SEEDS, 12)];
                    for (i, (item, n)) in loot.into_iter().enumerate() {
                        c.slots[i * 3 + i / 3] = Some((item, n));
                    }
                }
                for (item, n) in [(block::PORKCHOP, 3), (block::SAND, 16), (block::PLANKS, 20), (block::MUTTON, 2)] {
                    app.game.give(item, n);
                }
                match s.mode.as_str() {
                    "chest" => app.game.open_container(chest),
                    "furnace" => app.game.open_container(busy),
                    _ => {}
                }
            }
            if s.mode == "newblocks" && frames == 125 {
                // Copper as it ages (and waxed), bamboo things, coral, a Hollow Box.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let y = p.y.floor() as i32;
                let at = |f: f32, r: f32, up: i32| {
                    let v = p + fwd * f + right * r;
                    IVec3::new(v.x.floor() as i32, y + up, v.z.floor() as i32)
                };
                let w = &mut app.game.world;
                for i in 0..4 {
                    w.set_v(at(6.0, i as f32 - 5.0, 0), block::COPPER_FIRST + i);
                    w.set_v(at(6.0, i as f32 - 5.0, 1), block::WAXED_COPPER_FIRST + i);
                    w.set_v(at(8.0, i as f32 - 5.0, 0), block::CORAL_FIRST + i);
                }
                w.set_v(at(8.0, -1.0, 0), block::DEAD_CORAL);
                w.set_v(at(6.0, 0.0, 0), block::COPPER_ORE);
                w.set_v(at(6.0, 1.0, 0), block::BAMBOO_BLOCK);
                w.set_v(at(6.0, 2.0, 0), block::BAMBOO_PLANKS);
                w.set_v(at(6.0, 3.0, 0), block::BAMBOO_MOSAIC);
                w.set_v(at(5.0, 1.0, 0), block::BAMBOO_SLAB);
                w.set_v(at(5.0, 2.0, 0), block::BAMBOO_STAIRS);
                w.set_v(at(5.0, 3.0, 0), block::HOLLOW_BOX);
                for up in 0..6 {
                    w.set_v(at(8.0, 3.0, up), block::BAMBOO);
                    w.set_v(at(9.0, 2.0, up.min(3)), block::BAMBOO);
                }
                app.game.inv.slots[0] = Some((block::GLIDER, 1));
                app.game.inv.slots[1] = Some((block::ROCKET, 16));
                app.game.inv.slots[2] = Some((block::SPEAR, 1));
                app.game.inv.slots[3] = Some((block::COPPER_INGOT, 12));
                app.game.inv.slots[4] = Some((block::HOLLOW_BOX, 1));
                app.game.inv.slots[5] = Some((block::BAMBOO, 32));
                app.game.inv.slots[6] = Some((block::AXE_FIRST + 2, 1));
                app.game.inv.slots[7] = Some((block::SHOVEL_FIRST + 4, 1));
                app.game.inv.slots[8] = Some((block::PICK_COPPER, 1));
                app.game.inv.selected = 6;
            }
            if s.mode == "glider" && frames == 125 {
                // Soaring: worn Glider, seen from behind.
                app.game.inv.armor[block::CHESTPLATE] = Some((block::GLIDER, 1));
                app.game.player.body.pos += Vec3::Y * 40.0;
                app.game.player.body.vel = Vec3::new(s.yaw.sin(), -0.2, -s.yaw.cos()) * 18.0;
                app.game.player.body.on_ground = false;
                app.game.player.gliding = true;
                app.game.third_person = true;
            }
            if s.mode == "reef" && frames == 150 {
                // Some sea life in front of the camera.
                let eye = app.game.player.eye();
                let fwd = Vec3::new(s.yaw.sin() * s.pitch.cos(), s.pitch.sin(), -s.yaw.cos() * s.pitch.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let mut rng = noise::Rng::new(3);
                for i in 0..5 {
                    let at = eye + fwd * (4.0 + (i % 2) as f32) + right * (i as f32 * 0.7 - 1.4) + Vec3::Y * ((i % 3) as f32 * 0.4 - 0.4);
                    let mut m = entity::Mob::new(entity::MobKind::Fishy, at, &mut rng);
                    m.id = 5000 + i;
                    m.yaw = s.yaw + 1.2;
                    app.game.mobs.push(m);
                }
                let mut m = entity::Mob::new(entity::MobKind::Soggy, eye + fwd * 8.0 - right * 3.0 - Vec3::Y * 1.0, &mut rng);
                m.seed = 1;
                m.id = 5100;
                m.yaw = s.yaw + std::f32::consts::PI;
                app.game.mobs.push(m);
            }
            if matches!(s.mode.as_str(), "building" | "armour") && frames == 125 {
                // A little staircase, slabs, a pair of doors and some things on the floor.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let y = p.y.floor() as i32;
                let at = |f: f32, r: f32, up: i32| {
                    let v = p + fwd * f + right * r;
                    IVec3::new(v.x.floor() as i32, y + up, v.z.floor() as i32)
                };
                // Which way is "away" from the camera, as a facing.
                let away = if fwd.x.abs() > fwd.z.abs() { if fwd.x > 0.0 { 1 } else { 3 } } else if fwd.z > 0.0 { 2 } else { 0 };
                let side = (away + 1) % 4;
                for (i, m) in [0usize, 1, 2].into_iter().enumerate() {
                    let r = i as f32 * 1.0 - 5.0;
                    app.game.world.set_v(at(5.0, r, 0), block::stairs(m, away));
                    app.game.world.set_v(at(6.0, r, 0), block::MATERIALS[m].0);
                    app.game.world.set_v(at(6.0, r, 1), block::stairs(m, away));
                    app.game.world.set_v(at(7.0, r, 0), block::MATERIALS[m].0);
                    app.game.world.set_v(at(7.0, r, 1), block::MATERIALS[m].0);
                    app.game.world.set_v(at(4.0, r + 4.0, 0), block::slab(m, false));
                    app.game.world.set_v(at(5.0, r + 4.0, 1), block::slab(m, true));
                    app.game.world.set_v(at(5.0, r + 4.0, 0), block::STONE);
                }
                for (r, open) in [(3.0, false), (4.0, true)] {
                    app.game.world.set_v(at(6.0, r, 0), block::door(away, open, false));
                    app.game.world.set_v(at(6.0, r, 1), block::door(away, open, true));
                }
                for r in [2.0, 5.0] {
                    for up in 0..3 {
                        app.game.world.set_v(at(6.0, r, up), block::PLANKS);
                    }
                }
                app.game.world.set_v(at(6.0, 3.0, 2), block::PLANKS);
                app.game.world.set_v(at(6.0, 4.0, 2), block::PLANKS);
                let _ = side;
                let loot = [(block::DIAMOND, 3), (block::COBBLE, 12), (block::PORKCHOP, 1), (block::stairs(0, 0), 4), (block::ARMOR_FIRST + 4, 1), (block::DOOR, 1), (block::TORCH, 5)];
                for (i, (item, n)) in loot.into_iter().enumerate() {
                    let v = p + fwd * (2.5 + (i % 3) as f32 * 0.7) + right * ((i as f32 - 3.0) * 0.6);
                    app.game.spawn_drop(Vec3::new(v.x, y as f32, v.z), item, n, 0, Vec3::ZERO, 60.0);
                }
                if s.mode == "armour" {
                    for slot in 0..4 {
                        let tier = [3, 1, 2, 0][slot];
                        app.game.inv.armor[slot] = Some((block::ARMOR_FIRST + tier * 4 + slot as u16, 1));
                        // Each piece trimmed differently (see trims.rs).
                        app.game.inv.armor_wear[slot] = trims::with_trim(0, slot, [1, 2, 3, 0][slot]);
                    }
                    app.game.third_person = true;
                    app.game.player.health = 15.0;
                }
            }
            if s.mode == "contraptions" && frames == 125 {
                // Lever -> dust -> repeater -> lamp; a torch inverter; pistons out; a dispenser.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let f = if fwd.x.abs() > fwd.z.abs() { IVec3::new(fwd.x.signum() as i32, 0, 0) } else { IVec3::new(0, 0, fwd.z.signum() as i32) };
                let r = IVec3::new(-f.z, 0, f.x);
                let base = IVec3::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
                let at = |fo: i32, ro: i32, up: i32| base + f * fo + r * ro + IVec3::Y * up;
                app.game.world.set_v(at(3, -3, 0), block::LEVER_ON);
                for k in -2..0 {
                    app.game.world.set_v(at(3, k, 0), block::WIRE);
                }
                app.game.world.set_v(at(3, 0, 0), contraptions::repeater(contraptions::facing_of(r), false));
                app.game.world.set_v(at(3, 1, 0), block::LAMP);
                app.game.world.set_v(at(5, -2, 0), block::STONE);
                app.game.world.set_v(at(5, -2, 1), block::ZTORCH_ON);
                app.game.world.set_v(at(5, -1, 1), block::LAMP);
                for k in 0..3 {
                    app.game.world.set_v(at(6, k, 0), contraptions::piston(contraptions::facing_of(f), false, k == 1));
                    app.game.world.set_v(at(7, k, 0), block::PLANKS);
                    app.game.world.set_v(at(6, k, 1), block::ZAP_BLOCK);
                }
                app.game.world.set_v(at(4, 3, 0), block::DISPENSER_FIRST + contraptions::facing_of(-f) as block::Id);
                // A chest feeding a furnace through a hopper.
                app.game.world.set_v(at(3, -5, 0), block::FURNACE);
                app.game.world.set_v(at(3, -5, 1), block::HOPPER_FIRST);
                app.game.world.set_v(at(3, -5, 2), block::CHEST);
                app.game.world.set_v(at(4, -5, 1), block::HOPPER_FIRST + 1 + contraptions::facing_of(-f) as block::Id);
            }
            if s.mode == "models" && frames == 125 {
                // Two rows of shaped blocks to look at closely (the bug list's models).
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let f = if fwd.x.abs() > fwd.z.abs() { IVec3::new(fwd.x.signum() as i32, 0, 0) } else { IVec3::new(0, 0, fwd.z.signum() as i32) };
                let r = IVec3::new(-f.z, 0, f.x);
                let base = IVec3::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
                let at = |fo: i32, ro: i32, up: i32| base + f * fo + r * ro + IVec3::Y * up;
                let near = [block::BED_FACING_FIRST, block::SOUL_LANTERN, block::LANTERN, block::ENCHANTING_TABLE, block::DETECTOR_RAIL, block::BELL, block::SNOW_GRASS, block::GRINDSTONE];
                let far = [block::POT_FIRST, block::SMITHING_TABLE, block::BOOKSHELF, block::SCAFFOLDING, block::CHAIN, block::CHARRED_SKULL, block::EYE_FRAME_FULL, block::BREWING_STAND];
                // `--row 1..3` shows a few of them up close instead.
                let row = std::env::args().collect::<Vec<_>>().windows(2).find(|w| w[0] == "--row").and_then(|w| w[1].parse::<usize>().ok());
                let all: Vec<block::Id> = near.into_iter().chain(far).collect();
                let rows: Vec<(i32, Vec<block::Id>)> = match row {
                    Some(k) => vec![(4, all.iter().copied().skip((k - 1) * 6).take(6).collect())],
                    None => vec![(7, near.to_vec()), (11, far.to_vec())],
                };
                let gap = if row.is_some() { 1 } else { 2 };
                for (row, ids) in rows {
                    for (i, id) in ids.into_iter().enumerate() {
                        let ro = i as i32 * gap - if gap == 1 { 3 } else { 7 };
                        app.game.world.set_v(at(row, ro, 0), id);
                        if id == block::CHAIN {
                            app.game.world.set_v(at(row, ro, 1), id);
                        }
                    }
                }
                // A lit portal in a frame, off to one side.
                for dy in -1..=3 {
                    for dx in -1..=2 {
                        let frame = dx == -1 || dx == 2 || dy == -1 || dy == 3;
                        let c = at(14, dx, dy + 1);
                        app.game.world.set_v(c, if frame { block::OBSIDIAN } else if f.z != 0 { block::PORTAL_X } else { block::PORTAL_Z });
                    }
                }
                for (k, id) in [block::BELL, block::GRINDSTONE, block::BREWING_STAND, block::POT_FIRST, block::SCAFFOLDING, block::CHAIN, block::SOUL_LANTERN, block::CHARRED_SKULL, block::ENCHANTING_TABLE].into_iter().enumerate() {
                    app.game.inv.slots[k] = Some((id, 1));
                }
            }
            if s.mode == "homecraft" && frames == 125 {
                // v0.3's home and craft: cauldrons, composters, heads and a rod; an armoured Galloper.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let f = if fwd.x.abs() > fwd.z.abs() { IVec3::new(fwd.x.signum() as i32, 0, 0) } else { IVec3::new(0, 0, fwd.z.signum() as i32) };
                let r = IVec3::new(-f.z, 0, f.x);
                let base = IVec3::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
                let at = |fo: i32, ro: i32, up: i32| base + f * fo + r * ro + IVec3::Y * up;
                use crate::homecraft::cauldron_block;
                let near = [block::CAULDRON, cauldron_block(3, None), cauldron_block(3, Some(2)), cauldron_block(1, Some(6)), block::COMPOSTER, block::COMPOSTER + 4, block::COMPOSTER + 7];
                let far = [block::GROANER_HEAD, block::RATTLER_SKULL, block::HISSER_HEAD, block::LIGHTNING_ROD];
                for (i, id) in near.into_iter().enumerate() {
                    app.game.world.set_v(at(4, i as i32 - 3, 0), id);
                }
                for (i, id) in far.into_iter().enumerate() {
                    app.game.world.set_v(at(6, i as i32 * 2 - 3, 0), id);
                }
                let mut rng = crate::noise::Rng::new(5);
                let mut m = entity::Mob::new(entity::MobKind::Galloper, (base + f * 8 + r * 2).as_vec3() + Vec3::new(0.5, 0.0, 0.5), &mut rng);
                m.owner = Some("you".into());
                m.saddled = true;
                m.barding = 3;
                m.yaw = s.yaw + 1.2;
                m.id = 2300;
                m.persistent = true;
                app.game.mobs.push(m);
                // Pack animals, a Snow Golem, and an Oinker tied to a fence.
                for (k, (kind, fo, ro)) in [(entity::MobKind::Donkey, 8, -2), (entity::MobKind::Mule, 9, -5), (entity::MobKind::SnowGolem, 7, 5)].into_iter().enumerate() {
                    let mut m = entity::Mob::new(kind, (base + f * fo + r * ro).as_vec3() + Vec3::new(0.5, 0.0, 0.5), &mut rng);
                    m.id = 2301 + k as u32;
                    m.yaw = s.yaw + 1.6;
                    m.persistent = true;
                    m.pack = kind != entity::MobKind::SnowGolem;
                    m.saddled = kind == entity::MobKind::Donkey;
                    app.game.mobs.push(m);
                }
                let post = at(2, 1, 0);
                app.game.world.set_v(post, block::FENCE_FIRST);
                let mut pig = entity::Mob::new(entity::MobKind::Oinker, (post - r * 3).as_vec3() + Vec3::new(0.5, 0.0, 0.5), &mut rng);
                pig.id = 2310;
                pig.persistent = true;
                pig.leash = Some(leads::Leash::Fence(post));
                app.game.mobs.push(pig);
                for (k, (id, wear)) in [(block::ARMOR_FIRST, trims::with_dye(0, Some(2))), (block::ARMOR_FIRST + 1, trims::with_dye(0, Some(6))), (block::ARMOR_FIRST + 2, trims::with_dye(0, Some(4))), (block::CAULDRON, 0), (block::COMPOSTER, 0), (block::LIGHTNING_ROD, 0), (block::HORSE_ARMOR_IRON, 0), (block::HORSE_ARMOR_GOLD, 0), (block::HORSE_ARMOR_DIAMOND, 0)].into_iter().enumerate() {
                    app.game.inv.slots[k] = Some((id, 1));
                    app.game.inv.wear[k] = wear;
                }
            }
            if s.mode == "woods" && frames == 125 {
                // v0.3's woods, one row each: log, leaves, planks, slab, stairs, fence and gate, door; a boat.
                use crate::woods::{id, part};
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let f = if fwd.x.abs() > fwd.z.abs() { IVec3::new(fwd.x.signum() as i32, 0, 0) } else { IVec3::new(0, 0, fwd.z.signum() as i32) };
                let r = IVec3::new(-f.z, 0, f.x);
                let base = IVec3::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
                let at = |fo: i32, ro: i32, up: i32| base + f * fo + r * ro + IVec3::Y * up;
                for w in 0..3usize {
                    let fo = 5 + w as i32 * 3;
                    for up in 0..3 {
                        app.game.world.set_v(at(fo, -5, up), id(w, part::LOG));
                    }
                    app.game.world.set_v(at(fo, -5, 3), id(w, part::LEAVES));
                    app.game.world.set_v(at(fo, -4, 0), id(w, part::PLANKS));
                    app.game.world.set_v(at(fo, -3, 0), id(w, part::SLAB));
                    app.game.world.set_v(at(fo, -2, 0), id(w, part::STAIRS));
                    for k in -1..=1 {
                        app.game.world.set_v(at(fo, k, 0), id(w, part::FENCE));
                    }
                    app.game.world.set_v(at(fo, 2, 0), crate::carpentry::gate_of(id(w, part::GATE), f.z != 0, false));
                    app.game.world.set_v(at(fo, 3, 0), id(w, part::FENCE));
                    let away = if fwd.x.abs() > fwd.z.abs() { if fwd.x > 0.0 { 1 } else { 3 } } else if fwd.z > 0.0 { 2 } else { 0 };
                    app.game.world.set_v(at(fo, 5, 0), block::door_of(id(w, part::DOOR), away, false, false));
                    app.game.world.set_v(at(fo, 5, 1), block::door_of(id(w, part::DOOR), away, false, true));
                    app.game.spawn_vehicle(vehicles::WOOD_BOAT_KIND + w as u8, at(fo - 1, 7, 0).as_vec3() + Vec3::new(0.5, 0.0, 0.5), 0.3);
                }
            }
            if s.mode == "underworks" && frames == 125 {
                // v0.2 part 2: a Wilter's T (one skull short), a Starred Beacon, a rope down a hole.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let f = if fwd.x.abs() > fwd.z.abs() { IVec3::new(fwd.x.signum() as i32, 0, 0) } else { IVec3::new(0, 0, fwd.z.signum() as i32) };
                let r = IVec3::new(-f.z, 0, f.x);
                let base = IVec3::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
                let at = |fo: i32, ro: i32, up: i32| base + f * fo + r * ro + IVec3::Y * up;
                for k in -1..=1 {
                    app.game.world.set_v(at(7, k - 2, 1), block::SORROW_SAND);
                }
                app.game.world.set_v(at(7, -2, 0), block::SORROW_SAND);
                for k in [-1, 0] {
                    app.game.world.set_v(at(7, k - 2, 2), block::CHARRED_SKULL);
                }
                let top = at(12, -7, 3);
                for k in 1..=3 {
                    for dx in -k..=k {
                        for dz in -k..=k {
                            app.game.world.set_v(top + IVec3::new(dx, -k, dz), block::OBSIDIAN);
                        }
                    }
                }
                app.game.world.set_v(top, block::STARRED_BEACON_FIRST + 1);
                // A hole with a rope down it, and one hanging off a post.
                for d in 0..6 {
                    app.game.world.set_v(at(4, -3, -d - 1), block::AIR);
                }
                app.game.world.set_v(at(4, -3, 0), block::ROPE);
                app.game.unroll(at(4, -3, 0));
                for d in 0..4 {
                    app.game.world.set_v(at(5, -4, d), block::STONE);
                }
                app.game.world.set_v(at(4, -4, 3), block::ROPE);
                app.game.unroll(at(4, -4, 3));
                app.game.inv.slots[app.game.inv.selected] = Some((block::SUPPORT_GAUGE, 1));
            }
            if s.mode == "machines" && frames == 125 {
                // A beacon on its pyramid; a detector rail under a cart lighting a lamp;
                // chest and hopper carts; a comparator reading a chest.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let f = if fwd.x.abs() > fwd.z.abs() { IVec3::new(fwd.x.signum() as i32, 0, 0) } else { IVec3::new(0, 0, fwd.z.signum() as i32) };
                let r = IVec3::new(-f.z, 0, f.x);
                let base = IVec3::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
                let at = |fo: i32, ro: i32, up: i32| base + f * fo + r * ro + IVec3::Y * up;
                let top = at(12, 0, 2);
                for k in 1..=2 {
                    for dx in -k..=k {
                        for dz in -k..=k {
                            app.game.world.set_v(top + IVec3::new(dx, -k, dz), block::OBSIDIAN);
                        }
                    }
                }
                app.game.world.set_v(top, block::BEACON_FIRST + 1);
                // Track across the front, a detector in it, a lamp beside that.
                for k in -5..=2 {
                    app.game.world.set_v(at(5, k, 0), block::RAIL_FIRST);
                }
                app.game.world.set_v(at(5, -1, 0), block::DETECTOR_RAIL);
                app.game.world.set_v(at(6, -1, 0), block::LAMP);
                let spot = |fo: i32, ro: i32| at(fo, ro, 0).as_vec3() + Vec3::new(0.5, 0.06, 0.5);
                let yaw = (r.x as f32).atan2(-(r.z as f32));
                app.game.spawn_vehicle(vehicles::CART_KIND, spot(5, -1), yaw);
                let chest = app.game.spawn_vehicle(vehicles::CHEST_CART_KIND, spot(5, 1), yaw);
                app.game.spawn_vehicle(vehicles::HOPPER_CART_KIND, spot(5, -4), yaw);
                if let Some(c) = containers::store(&mut app.game.world, &mut app.game.vehicles, vehicles::cart_key(chest)) {
                    c.slots[0] = Some((block::DIAMOND, 3));
                }
                // Chest -> comparator -> lamp.
                app.game.world.set_v(at(3, -3, 0), block::CHEST);
                if let Some(c) = app.game.world.containers.get_mut(&at(3, -3, 0)) {
                    c.slots[0] = Some((block::COBBLE, 5));
                }
                app.game.world.set_v(at(3, -2, 0), contraptions::comparator(contraptions::facing_of(r), false, false));
                app.game.world.set_v(at(3, -1, 0), block::LAMP);
            }
            if s.mode == "brewing" && frames == 125 {
                // A brewing stand mid-brew, potions in hand, and a couple of effects on.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let at = (p + fwd * 2.5).floor().as_ivec3();
                app.game.world.set_v(at, block::BREWING_STAND);
                if let Some(c) = app.game.world.containers.get_mut(&at) {
                    c.slots[containers::INPUT] = Some((block::EMBER_SHROOM, 3));
                    c.slots[containers::FUEL] = Some((block::WATER_BOTTLE, 1));
                }
                app.game.world.set_v(at + IVec3::X, block::EMBER_SHROOM);
                for (i, p) in potions::ALL.iter().enumerate() {
                    app.game.inv.slots[i] = Some((potions::potion_item(*p, i % 2 == 1), 1));
                }
                app.game.inv.slots[5] = Some((block::GLASS_BOTTLE, 3));
                app.game.inv.slots[6] = Some((block::WATER_BOTTLE, 2));
                app.game.inv.slots[7] = Some((block::GRUMBLER_TUSK, 1));
                app.game.apply_potion(potions::Potion::Speed);
                app.game.apply_potion(potions::Potion::NightVision);
            }
            if s.mode == "carpentry" && frames == 125 {
                // A fenced pen with a gate, a wall with a ladder, panes and colours.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let f = if fwd.x.abs() > fwd.z.abs() { IVec3::new(fwd.x.signum() as i32, 0, 0) } else { IVec3::new(0, 0, fwd.z.signum() as i32) };
                let r = IVec3::new(-f.z, 0, f.x);
                let base = IVec3::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
                let at = |fo: i32, ro: i32, up: i32| base + f * fo + r * ro + IVec3::Y * up;
                for k in -3..=3 {
                    app.game.world.set_v(at(3, k, 0), if k == 0 { carpentry::gate(f.x != 0, false) } else { block::FENCE_FIRST });
                    app.game.world.set_v(at(7, k, 0), block::FENCE_FIRST);
                }
                for fo in 4..7 {
                    app.game.world.set_v(at(fo, -3, 0), block::FENCE_FIRST);
                    app.game.world.set_v(at(fo, 3, 0), block::FENCE_FIRST);
                }
                app.game.alloc_mob(entity::MobKind::Oinker, at(5, 0, 0).as_vec3() + Vec3::new(0.5, 0.0, 0.5));
                // A wall of coloured wool and stained glass panes, with a ladder.
                for k in 4..12 {
                    for up in 0..4 {
                        let c = (k + up) as u16 % 8;
                        let id = if up == 2 && k % 2 == 0 { block::PANE_FIRST } else if up == 3 { block::STAINED_GLASS + c } else if c == 0 { block::WOOL } else { block::DYED_WOOL + c - 1 };
                        app.game.world.set_v(at(9, k - 8, up), id);
                    }
                }
                for up in 0..4 {
                    app.game.world.set_v(at(8, -4, up), block::LADDER_FIRST + building_facing(-f));
                }
                app.game.world.set_v(at(2, 3, 0), carpentry::trapdoor(0, true));
                app.game.world.set_v(at(2, 4, 0), carpentry::trapdoor(0, false));
                // A campfire of sorts: logs, alight.
                app.game.world.set_v(at(2, -3, 0), block::LOG);
                app.game.world.set_v(at(2, -3, 1), block::FIRE);
                app.game.world.set_v(at(2, -2, 0), block::FIRE);
                // A plank post on fire up one side (the flames cling to it).
                for up in 0..3 {
                    app.game.world.set_v(at(4, -3, up), block::PLANKS);
                    app.game.world.set_v(at(4, -2, up), block::FIRE);
                }
            }
            if s.mode == "decor" && frames == 125 {
                // A signpost, a framed sword on a wall, and a map in hand.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let f = if fwd.x.abs() > fwd.z.abs() { IVec3::new(fwd.x.signum() as i32, 0, 0) } else { IVec3::new(0, 0, fwd.z.signum() as i32) };
                let r = IVec3::new(-f.z, 0, f.x);
                let base = IVec3::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
                let facing = if f.z < 0 { 0u16 } else if f.x > 0 { 1 } else if f.z > 0 { 2 } else { 3 };
                let sign = base + f * 3 - r;
                app.game.world.set_v(sign, block::SIGN_FIRST + facing);
                app.game.world.signs.insert(sign, ["Welcome to".into(), "STOVEVILLE".into(), "pop. 1".into(), "(it's you)".into()]);
                for k in -1..=2 {
                    for up in 0..3 {
                        app.game.world.set_v(base + f * 6 + r * k + IVec3::Y * up, block::STONE_BRICKS);
                    }
                }
                // Frames hang on the wall's near face: their facing is the far side of their cell.
                let frame_facing = facing as block::Id;
                for (k, item) in [(0, block::SWORD_DIAMOND), (1, block::DIAMOND)] {
                    let at = base + f * 5 + r * k + IVec3::Y;
                    app.game.world.set_v(at, block::FRAME_FIRST + frame_facing);
                    app.game.world.frames.insert(at, (item, 0));
                }
                app.game.inv.slots[app.game.inv.selected] = Some((block::MAP, 1));
                app.game.inv.add(block::COMPASS, 1);
            }
            if s.mode == "vehicles" && frames == 125 {
                // A loop of rails with a cart, and a boat on a pond.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let f = if fwd.x.abs() > fwd.z.abs() { IVec3::new(fwd.x.signum() as i32, 0, 0) } else { IVec3::new(0, 0, fwd.z.signum() as i32) };
                let r = IVec3::new(-f.z, 0, f.x);
                let base = IVec3::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
                let at = |fo: i32, ro: i32| base + f * fo + r * ro;
                for k in 0..7 {
                    for (a, b) in [(3 + k, -4), (3 + k, 1), (3, -4 + k.min(5)), (9, -4 + k.min(5))] {
                        app.game.world.set_v(at(a, b), block::RAIL_FIRST);
                    }
                }
                app.game.world.set_v(at(3, -1), block::POWERED_RAIL);
                app.game.world.set_v(at(3, -1) - r, block::LEVER_ON);
                for a in 4..9 {
                    for b in 3..7 {
                        app.game.world.set_v(at(a, b) - IVec3::Y, block::WATER);
                    }
                }
                let cart_at = at(6, -4).as_vec3() + Vec3::new(0.5, 1.0 / 16.0, 0.5);
                app.game.spawn_vehicle(vehicles::CART_KIND, cart_at, 0.0);
                let boat_at = at(6, 5).as_vec3() + Vec3::new(0.5, -0.15, 0.5);
                app.game.spawn_vehicle(vehicles::BOAT_KIND, boat_at, 0.7);
                app.game.inv.add(block::MINECART, 1);
                app.game.inv.add(block::BOAT, 1);
                app.game.inv.add(block::RAIL_FIRST, 32);
            }
            if s.mode == "trade" && frames == 125 {
                // A Librarian and a purse of gold.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let seed = (0..64u32).find(|&s| villagers::Job::of(s) == villagers::Job::Librarian).unwrap_or(1);
                app.game.world.new_huts.push((p + fwd * 3.0, seed));
                app.game.house_hmmers();
                let id = app.game.mobs.last().map(|m| m.id).unwrap_or(0);
                if let Some(m) = app.game.mobs.last_mut() {
                    m.yaw = s.yaw + std::f32::consts::PI;
                }
                app.game.inv.add(block::GOLD_INGOT, 23);
                app.game.inv.add(block::FEATHER, 30);
                app.game.inv.add(block::BOOK, 3);
                app.game.open_trade(id);
            }
            if s.mode == "zappy" && frames == 125 {
                // A lever wired to a row of lamps and a door; a button and a plate beside.
                // Laid out on the grid (dust only links to its neighbours).
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let f = if fwd.x.abs() > fwd.z.abs() { IVec3::new(fwd.x.signum() as i32, 0, 0) } else { IVec3::new(0, 0, fwd.z.signum() as i32) };
                let r = IVec3::new(-f.z, 0, f.x);
                let base = IVec3::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
                let at = |fo: f32, ro: f32, up: i32| base + f * fo.round() as i32 + r * ro.round() as i32 + IVec3::Y * up;
                let w = &mut app.game.world;
                w.set_v(at(4.0, -4.0, 0), block::LEVER_ON);
                for k in 0..9 {
                    w.set_v(at(4.0, -3.0 + k as f32, 0), block::WIRE);
                }
                for k in 0..4 {
                    w.set_v(at(5.0 + k as f32, 5.0, 0), block::WIRE);
                }
                for k in 0..3 {
                    w.set_v(at(5.0, -2.0 + k as f32 * 3.0, 0), block::LAMP);
                    w.set_v(at(5.0, -2.0 + k as f32 * 3.0, 1), block::LAMP);
                }
                let d = at(9.0, 5.0, 0);
                w.set_v(d, block::door(0, false, false));
                w.set_v(d + IVec3::Y, block::door(0, false, true));
                w.set_v(at(3.0, -1.5, 0), block::BUTTON);
                w.set_v(at(2.5, 1.0, 0), block::PLATE);
                w.set_v(at(8.0, -4.0, 0), block::ZAP_BLOCK);
                w.set_v(at(8.0, -3.0, 0), block::LAMP);
                app.game.inv.add(block::ZAP_DUST, 32);
                app.game.inv.add(block::LEVER, 2);
                app.game.inv.add(block::LAMP, 4);
            }
            if s.mode == "liquids" && frames == 125 {
                // A waterfall off a pillar, a lava pool, and where they met.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let at = |f: f32, r: f32, up: i32| {
                    let v = p + fwd * f + right * r;
                    IVec3::new(v.x.floor() as i32, p.y.floor() as i32 + up, v.z.floor() as i32)
                };
                for up in 0..4 {
                    app.game.world.set_v(at(9.0, -3.0, up), block::STONE_BRICKS);
                }
                app.game.world.set_v(at(9.0, -3.0, 4), block::WATER);
                for (f, r) in [(6.0, 3.0), (6.0, 4.0), (7.0, 3.0), (7.0, 4.0)] {
                    app.game.world.set_v(at(f, r, -1), block::LAVA);
                }
                app.game.world.set_v(at(8.0, 3.5, 0), block::OBSIDIAN);
                app.game.world.set_v(at(8.0, 3.5, 1), block::OBSIDIAN);
                app.game.inv.add(block::WATER_BUCKET, 1);
                app.game.inv.add(block::LAVA_BUCKET, 1);
                app.game.inv.add(block::BUCKET, 3);
            }
            if matches!(s.mode.as_str(), "enchant" | "table") && frames == 125 {
                // An enchanting table in a ring of bookshelves, and something to enchant.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let v = p + fwd * 4.5;
                let t = IVec3::new(v.x.floor() as i32, p.y.floor() as i32, v.z.floor() as i32);
                app.game.world.set_v(t, block::ENCHANTING_TABLE);
                for dx in -2..=2i32 {
                    for dz in -2..=2i32 {
                        // Leave a gap on the near side to walk in.
                        let near = IVec3::new(dx, 0, dz).as_vec3().dot(fwd) < -1.0;
                        if dx.abs().max(dz.abs()) == 2 && !near {
                            app.game.world.set_v(t + IVec3::new(dx, 0, dz), block::BOOKSHELF);
                            app.game.world.set_v(t + IVec3::new(dx, 1, dz), block::BOOKSHELF);
                        }
                    }
                }
                app.game.xp = xp::points_for_level(30) + 40;
                for (item, n, wear) in [(block::PICK_DIAMOND, 1, 12), (block::GOLD_INGOT, 9, 0), (block::SWORD_IRON, 1, enchant::with_level(enchant::with_level(40, enchant::Enchant::Sharpness, 3), enchant::Enchant::Unbreaking, 1)), (block::BREAD, 6, 0)] {
                    app.game.inv.add(item, n);
                    let i = app.game.inv.slots.iter().position(|s| *s == Some((item, n))).unwrap();
                    app.game.inv.wear[i] = wear;
                }
                if s.mode == "enchant" {
                    app.game.open_enchanting(t);
                    if let Some(ui) = &mut app.game.enchanting {
                        ui.item = Some((block::PICK_DIAMOND, 1));
                        ui.wear = 12;
                        ui.gold = Some((block::GOLD_INGOT, 3));
                    }
                    app.game.inv.remove(block::PICK_DIAMOND, 1);
                    app.game.inv.remove(block::GOLD_INGOT, 3);
                } else {
                    app.game.inv.selected = app.game.inv.slots.iter().position(|s| s.is_some_and(|s| s.0 == block::SWORD_IRON)).unwrap_or(0);
                }
            }
            if matches!(s.mode.as_str(), "anvil" | "rules" | "xp") && frames == 125 {
                // An anvil (and a chipped one) ahead, a worn pickaxe and some iron, and a few levels.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let at = |f: f32, r: f32| {
                    let v = p + fwd * f + right * r;
                    IVec3::new(v.x.floor() as i32, p.y.floor() as i32, v.z.floor() as i32)
                };
                let (a, b) = (at(3.5, -0.8), at(3.5, 1.2));
                app.game.world.set_v(a, block::ANVIL);
                app.game.world.set_v(b, block::ANVIL_DAMAGED);
                app.game.xp = xp::points_for_level(7) + 20;
                for (item, n, wear) in [(block::PICK_IRON, 1, 190), (block::IRON, 5, 0), (block::SWORD_DIAMOND, 1, 800), (block::BREAD, 6, 0)] {
                    app.game.inv.add(item, n);
                    let i = app.game.inv.slots.iter().position(|s| *s == Some((item, n))).unwrap();
                    app.game.inv.wear[i] = wear;
                }
                match s.mode.as_str() {
                    "anvil" => {
                        app.game.open_anvil(a);
                        if let Some(ui) = &mut app.game.anvil {
                            ui.slots = [Some((block::PICK_IRON, 1)), Some((block::IRON, 3))];
                            ui.wear = [190, 0];
                        }
                        app.game.inv.remove(block::PICK_IRON, 1);
                        app.game.inv.remove(block::IRON, 3);
                    }
                    "rules" => app.set_screen(Screen::WorldSettings),
                    _ => {
                        for i in 0..6 {
                            let v = p + fwd * (2.0 + i as f32 * 0.6) + right * ((i as f32 - 2.5) * 0.7) + Vec3::Y * 0.8;
                            app.game.spawn_orbs(v, [3, 7, 17, 1, 37, 7][i]);
                        }
                        for o in app.game.orbs.iter_mut() {
                            o.body.vel = Vec3::ZERO;
                        }
                    }
                }
            }
            if s.mode == "xp" && frames > 125 {
                // Hold the orbs still for the photo.
                for o in app.game.orbs.iter_mut() {
                    o.body.vel = Vec3::ZERO;
                    o.age = 0.0;
                }
            }
            if s.mode == "thunder" && frames + 2 == s.frames {
                // A bolt in the distance for the photo.
                let p = app.game.player.body.pos;
                let (x, z) = ((p.x + s.yaw.sin() * 14.0).floor() as i32, (p.z - s.yaw.cos() * 14.0).floor() as i32);
                let y = app.game.world.surface_y(x, z) + 1;
                app.game.lightning_effects(Vec3::new(x as f32 + 0.5, y as f32, z as f32 + 0.5));
            }
            if s.mode == "death" && frames == 150 {
                app.game.inv.add(block::DIAMOND, 3);
                app.game.player.hurt = 0.0;
                app.game.hurt_player(100.0, "was defeated by a screenshot");
            }
            if s.mode == "animals" && frames == 150 {
                // A family of Mooers, a shorn Fluffer and a lamb, a pet Woofer sitting nicely.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let mut rng = noise::Rng::new(4);
                let owner = players::record_key(&app.game.player_name);
                let herd: [(entity::MobKind, f32, f32, bool); 7] = [
                    (entity::MobKind::Mooer, 7.0, -3.0, false),
                    (entity::MobKind::Mooer, 7.5, -1.2, false),
                    (entity::MobKind::Mooer, 5.5, -2.0, true),
                    (entity::MobKind::Fluffer, 6.0, 1.5, false),
                    (entity::MobKind::Fluffer, 4.8, 2.6, true),
                    (entity::MobKind::Woofer, 3.2, 0.0, false),
                    (entity::MobKind::Oinker, 8.5, 3.2, false),
                ];
                for (i, (kind, f, r, baby)) in herd.into_iter().enumerate() {
                    let at = p + fwd * f + right * r + Vec3::Y * 1.0;
                    let mut m = entity::Mob::new(kind, at, &mut rng);
                    m.yaw = s.yaw + std::f32::consts::PI + (i as f32 - 3.0) * 0.3;
                    m.id = 2000 + i as u32;
                    if baby {
                        m.set_baby(100.0);
                    }
                    if kind == entity::MobKind::Fluffer && !baby {
                        m.sheared = true;
                    }
                    if kind == entity::MobKind::Woofer {
                        m.owner = Some(owner.clone());
                        m.sitting = true;
                    }
                    if i == 6 {
                        m.love = 20.0;
                    }
                    m.persistent = true;
                    app.game.mobs.push(m);
                }
                app.game.inv.add(block::SHEARS, 1);
                app.game.inv.add(block::WHEAT, 12);
                app.game.inv.add(block::BONE, 5);
            }
            if s.mode == "animals" && frames > 150 {
                for m in app.game.mobs.iter_mut() {
                    m.goal = None;
                    m.body.vel.x = 0.0;
                    m.body.vel.z = 0.0;
                }
            }
            if s.mode == "newmobs" && frames == 150 {
                // This batch's mobs (a Camel with both seats, an Axolotl in a puddle), under fireworks.
                use entity::MobKind as K;
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let mut rng = noise::Rng::new(9);
                let line = [(K::Axolotl, 4.0, -2.0), (K::Goat, 5.0, -0.3), (K::Breeze, 5.0, 2.0), (K::Camel, 8.5, -1.2)];
                for (x, y, z) in [(-8.0, 14.0, 22.0), (4.0, 17.0, 26.0), (12.0, 12.0, 20.0)] {
                    let colour = (x as i32).rem_euclid(8) as u8;
                    app.game.firework_sparks(p + right * x + Vec3::Y * y + fwd * z, colour);
                }
                for (i, (kind, f, r)) in line.into_iter().enumerate() {
                    let up = 0.0;
                    let mut m = entity::Mob::new(kind, p + fwd * f + right * r + Vec3::Y * (2.0 + up), &mut rng);
                    m.yaw = s.yaw + std::f32::consts::PI;
                    m.id = 2000 + i as u32;
                    // A captain with its banner, a Snout admiring gold, an Invoicer casting.
                    m.saddled = kind == K::Camel;
                    app.game.mobs.push(m);
                }
            }
            if s.mode == "modzoo" && frames == 150 {
                // Every mob the loaded mods define, in a row (fliers up in the air).
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let mut rng = noise::Rng::new(9);
                let n = block::reg().mobs.len();
                for i in 0..n {
                    let kind = entity::MobKind::Modded(i as u16);
                    // Rows of five, further back each row.
                    let (row, col) = (i / 5, i % 5);
                    let in_row = (n - row * 5).min(5);
                    let r = (col as f32 - (in_row as f32 - 1.0) * 0.5) * 2.2;
                    let up = if kind.flies() { 2.5 } else { 0.0 };
                    let mut m = entity::Mob::new(kind, p + fwd * (5.0 + row as f32 * 4.0) + right * r + Vec3::Y * (2.0 + up), &mut rng);
                    m.yaw = s.yaw + std::f32::consts::PI - 0.5;
                    m.id = 3000 + i as u32;
                    m.anim = 0.8;
                    app.game.mobs.push(m);
                }
            }
            if s.mode == "music" && frames == 125 {
                // Note blocks on each kind of block, a jukebox playing, a bell, gold.
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let y = p.y.floor() as i32;
                let at = |f: f32, r: f32| {
                    let v = p + fwd * f + right * r;
                    IVec3::new(v.x.floor() as i32, y, v.z.floor() as i32)
                };
                let under = [block::PLANKS, block::STONE, block::SAND, block::GLASS, block::GOLD_BLOCK, block::ICE, block::TERRACOTTA, block::WOOL, block::GRASS];
                for (i, &b) in under.iter().enumerate() {
                    let c = at(5.0, i as f32 * 1.0 - 4.0);
                    app.game.world.set_v(c, b);
                    app.game.world.set_v(c + IVec3::Y, block::NOTE_BLOCK + (i as u16 * 3) % 25);
                }
                app.game.world.set_v(at(3.0, -2.0), block::JUKEBOX_DISC_FIRST + 4);
                app.game.world.set_v(at(3.0, 0.0), block::JUKEBOX);
                app.game.world.set_v(at(3.0, 2.0) + IVec3::Y, block::BELL);
                app.game.world.set_v(at(3.0, 2.0), block::GOLD_BLOCK);
                for i in 0..6 {
                    app.game.give(block::DISC_FIRST + i, 1);
                }
                let note = at(5.0, 0.0) + IVec3::Y * 2;
                for k in 0..6 {
                    app.game.note_particle(note.as_vec3() + Vec3::new(0.5 + k as f32 * 0.2 - 0.5, 0.2 + k as f32 * 0.25, 0.5), (k * 4) as u8);
                }
            }
            if s.mode == "raid" && frames == 150 {
                // Wave two marching on the square.
                use entity::MobKind as K;
                let p = app.game.player.body.pos;
                let fwd = Vec3::new(s.yaw.sin(), 0.0, -s.yaw.cos());
                let right = Vec3::new(s.yaw.cos(), 0.0, s.yaw.sin());
                let mut rng = noise::Rng::new(5);
                for (i, kind) in [K::Pilferer, K::Pilferer, K::Hackler, K::Pilferer, K::Invoicer, K::Rampager, K::Hackler].into_iter().enumerate() {
                    let v = p + fwd * (16.0 + (i % 3) as f32 * 2.0) + right * ((i as f32 - 3.0) * 2.2);
                    let (x, z) = (v.x.floor() as i32, v.z.floor() as i32);
                    let ground = app.game.world.surface_y(x, z) + 1;
                    let mut m = entity::Mob::new(kind, Vec3::new(x as f32 + 0.5, ground as f32, z as f32 + 0.5), &mut rng);
                    m.yaw = s.yaw + std::f32::consts::PI;
                    m.id = 3000 + i as u32;
                    m.seed = (i == 0) as u32;
                    app.game.mobs.push(m);
                }
                app.game.raid_hud = Some((1, 2, 5, 7, 999.0));
                app.game.bell_glow = 999.0;
            }
            if matches!(s.mode.as_str(), "zoo" | "newmobs" | "raid" | "golems" | "homestead") && frames > 150 {
                // Hold still for the photo.
                for m in app.game.mobs.iter_mut() {
                    m.yaw = s.yaw + std::f32::consts::PI;
                    m.body.vel.x = 0.0;
                    m.body.vel.z = 0.0;
                }
            }
            if matches!(s.mode.as_str(), "survival" | "creative" | "parody") && frames == 150 {
                let p = app.game.player.body.pos;
                let mut rng = noise::Rng::new(9);
                let dist = if s.mode == "parody" { 9.0 } else { 6.0 };
                for (i, kind) in entity::MobKind::ALL.into_iter().filter(|k| *k != entity::MobKind::Wyrm).enumerate() {
                    let at = p + Vec3::new(2.4f32.sin() * dist + (i as f32 - 2.0) * 2.4, 2.0, -2.4f32.cos() * dist);
                    let m = entity::Mob::new(kind, at, &mut rng);
                    app.game.mobs.push(m);
                }
            }
    }
}

/// A game for a scene. `--gen 2` makes it a newer world (see `GenOptions`);
/// scenes are older worlds otherwise, so they keep looking as they always have.
fn shot_game(seed: u32, creative: bool, menu: bool) -> Game {
    let wanted = std::env::args().collect::<Vec<_>>().windows(2).find(|w| w[0] == "--gen").and_then(|w| w[1].parse::<u8>().ok());
    let opts = match wanted {
        Some(v) if v >= 1 => crate::world::GenOptions { version: v.min(3), ..crate::world::GenOptions::DEFAULT },
        _ => crate::world::GenOptions::LEGACY,
    };
    Game::new_with(seed, creative, menu, opts)
}
