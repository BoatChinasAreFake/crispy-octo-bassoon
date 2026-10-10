# Notes for Claude

## Content rules

- **Never add Phantoms** (or any Phantom-like mob that harasses players for not sleeping), and never suggest them as an idea.

## Roadmap rules

The plan below runs from v0.2 to v1.0.0. Stick to it:

- **One version at a time, in order, released in parts.** Each version ships as a few parts (tagged `v0.2-part1`, `v0.2-part2`, ...), each a release of its own made of whole items from the list. The parts are listed under each version as they're planned. Don't start a version's items before the previous version's last part is released.
- **Don't wander.** Don't add features that aren't on the list for the version being worked on, and don't pull items forward from later versions. If something new seems worth doing, suggest it to the user and only add it to the list if they agree.
- **Between versions, only fixes.** Bug fixes and small requests from the user can go out as patch releases (v0.2.1, v0.2.2, ...), or ride along in the next part while a version is in progress. The user decides what counts as a small request.
- **Changing the plan.** Only the user changes this plan. When they do, update this file in the same session.
- **Every part ships with** (for what's in it): tests for the new rules, playtest-bot coverage of anything that syncs in multiplayer, advancements for the new content, screenshots checked, the README updated, 0 Clippy warnings, and a publish only when the user says "Yes".
- **Originals.** Each version from v0.2 to v0.8 also has two additions that aren't in Minecraft, listed under it as "Originals". They're as much a part of the version as the rest of its list.
- **When a part or a version is released,** mark it **(released)** here and leave its list in place so it's clear what each one contained.

## v0.2: Underground and ruins (released)

1. Cave biomes: dripstone caves (falling stalactites), lush caves (glow berries, moss), amethyst geodes.
2. Temples and mineshafts: desert pyramids (TNT trap), jungle temples (tripwires), abandoned mineshafts (rails, cobwebs, cart chests), igloos with a hidden basement.
3. Ocean Monument: Guardians and an Elder Guardian (mining fatigue), prismarine, and the Conduit (breathing underwater; drowning is already in).
4. Night threats: Witches (potion throwers), desert Groaners (husks) and snowy Rattlers (strays) that slow you.
5. Ocean and cave creatures: Glow Squid (ink), Bats, and the Allay (collects matching items).
8. A second boss: a Wither-like boss built from soul sand and skulls, dropping a beacon-upgrade star.
9. Deeper oceans (the user's request): the sea floor drops away offshore, and monuments sit on it.

Originals:
- Cave-ins: digging out a big hollow with no support makes the ceiling creak, drop dust, then collapse. Pillars and wooden beams hold it up, and a Support Gauge shows how stable a room is.
- Spelunker's Rope: throw it down a shaft and climb it. Ropes stay put and show on maps.

Parts:
- Part 1: items 1 to 5 and 9. **(released)**
- Part 2: item 8 and the two originals (this is the last part). **(released)**

## v0.3: A wider world

1. Surface biomes: savanna (acacia), birch forest, dark oak forest, mushroom islands (a red-spotted Mooer), ice spikes, meadows and stony peaks. Each new wood gets the full set (planks, slabs, stairs, fences, doors, boats).
2. Busier seas: kelp, seagrass, sea pickles; warm, lukewarm and frozen ocean variants.
3. Scorchlands biomes: a crimson and a teal fungus forest, basalt deltas and soul sand valleys, each with its own mob.
4. Snout Bastions: a big gilded ruin with a treasure room, tougher Snouts, and a boar-like beast that can be bred.
5. A woodland mansion in dark oak forests, full of Pilferers, Invoicers and secret rooms, found with a Cartographer's map.
6. Farm and pack animals: donkeys and mules (carry chests), a snow golem (throws snowballs), and leads (leash animals, tie them to fences).
7. Home and craft: cauldrons (dye leather armour, wash banners), a composter, mob heads, horse armour and a lightning rod.

Originals:
- Cooking, over-engineered (like fishing and farming): a cooking pot on a campfire. Ingredients combine into dishes with a quality grade; freshness, seasoning and cooking time matter; a Cookbook fills in as dishes are discovered; good meals give small buffs.
- Hot springs: steaming pools in snowy mountains. Bathing slowly heals and gets rid of the chill of a long night out.

Parts:
- Part 1: items 1 and 2. **(released)**
- Part 2: items 3 and 4. **(released)**
- Part 3: items 5, 6 and 7. **(released)**
- Part 4: the two originals (this is the last part).

## v0.4: Together and tinkering

1. Encrypted multiplayer: a key exchange and an encrypted stream for all traffic, using only the standard library (as now).
2. Finding servers: LAN games show up by themselves, and a saved server list with ping and player count.
3. Your own skins: load a 64×64 PNG, sent to everyone else by the server.
4. Bigger mods: mods can add biomes, structures (from a block template) and a whole new dimension with its own portal.
5. More Zappy parts: daylight sensor, target block, dropper, activator rail, trapped chest, calibrated sculk sensor.
6. Speed: chunk meshing on worker threads, compressed region files, a lower-memory option.
7. Languages: all text in a strings file, so translations can come as mods.
8. Accessibility: narrated menus, toggle (rather than hold) sneak and sprint, a high-contrast UI.

Originals:
- Land claims: a Survey Post marks out an area only you and the friends you name can build in (opt-in per server).
- Post boxes: send items and letters to another player, delivered even if they're offline, carried by a slightly put-upon Cluckster.

## v0.5: Villages and people

1. Villages that live: Hmmers claim beds and workstations, take their job from the station, have babies when there are free beds, and go indoors at night.
2. Village styles by biome: desert, savanna, taiga, snowy and plains villages each built differently, plus village-only blocks.
3. Reputation: helping a village (trading, curing, defending it) makes it like you; hurting a Hmmer or a Clanker makes it remember. Prices and the Clanker's mood follow.
4. Workstation trades by level: Novice to Master, unlocking better trades as a Hmmer is traded with.
5. Growing villages: the village builds a new Clanker when it has enough Hmmers, and adds houses over time as it grows (the Bell and the Wanderer already exist).
6. A notice board in each village with simple bounties ("bring 20 wheat", "defeat 5 Rattlers") paying gold and experience.

Originals:
- Your own shop stall: set prices, and Hmmers come by and buy what you stock. Demand rises and falls with what the village needs.
- The village paper: a daily newspaper reporting world events ("Local Woman Punches Tree", "Raid Repelled", "Hisser Seen Near Bakery").

## v0.6: The Hollow, filled in

1. Hollow Cities: tall towers on the outer islands, with a flying ship carrying the Glider.
2. A Shulker-like mob that hides in a shell, shoots homing bullets that make you float, and drops shells that make Hollow Boxes (replacing the current way of getting them).
3. Hollow plants: a chorus-fruit-like plant that teleports you a short way when eaten, and purpur-like blocks.
4. Gateways: beating the Wyrm opens small portals to the outer islands; the Wyrm can be summoned again with crystals.
5. A Hollow-themed advancement tab, and a parody "ending" poem when the Wyrm is first beaten.

Originals:
- Void fishing: cast off an island's edge into the void for strange catches (lost items, star fragments, the odd confused Fishie), tied into the fishing system and Angler levels.
- Gravity islands: small islands where down is a different direction.

## v0.7: Building and creative tools

1. Block sets: copper doors, trapdoors and grates; deepslate and tuff variants; walls for every stone; all slabs, stairs and walls for the v0.3 woods and stones.
2. Creative tools: `/fill`, `/clone`, a selection wand, and a structure block that saves and pastes builds (and can export them for mods).
3. Map art: maps record the colours of what they show, and can be locked and copied.
4. Armour dyeing for woolly (leather-like) armour, and coloured beds, candles and banners everywhere they're missing.
5. Build helpers: a symmetry option and a ghost preview of where a block will go (creative only, can be turned off).

Originals:
- Blueprints: copy a build at a Drafting Table, then Copper Golems build it from the materials in nearby chests, in survival.
- Paint roller: recolour placed wool, concrete, glass and terracotta without breaking them.

## v0.8: Atmosphere

1. Ambient sound: each biome has its own background (wind on peaks, birds in forests, drips in caves, bubbles at sea).
2. More music: biome and dimension pieces, a boss theme for each boss, and a title-screen theme.
3. Texture packs: a pack can replace any texture in the atlas (as a mod), with an option to keep the built-in look.
4. Visuals: animated textures (water, lava, portals, fire), better particles (falling leaves, dust in sunbeams, splashes), and volumetric-looking fog in caves and swamps.
5. Weather extras: windy days that sway more and push gliders, fog at dawn, and hail in the badlands (rare and silly).

Originals:
- Camera: take an in-game photo that becomes an item you can hang as a painting or put in a frame.
- Radio: a block that plays an endless procedurally composed station, with stations to tune between.

## v0.9: Feature freeze and polish

No new features. Only:

1. Balance pass: loot tables, mob health and damage, tool and armour numbers, food, enchantment costs, trade prices, spawn rates.
2. Onboarding: a short, skippable tutorial in a first world, and hints the first time something new is picked up.
3. Bug bash: run every screenshot scene and playtest at length, fix what turns up, and re-check all advancements can be earned.
4. Performance pass: profile the worst cases (big render distance, lots of mobs, busy servers) and fix the slowest parts.
5. Docs pass: README, MODDING.md and SCRIPTING.md complete and current.

## v1.0.0: Stable

1. Stability promises: the save format, network protocol and mod API are versioned, and every older save loads (with a test save from each earlier version kept in the repo).
2. Packaging: a macOS app bundle, a Linux AppImage, and the Windows build with its icon and version info.
3. Credits screen and a proper ending sequence after the Hollow Wyrm.
4. A final README, a changelog covering v0.1 to v1.0, and the release.
