# MINCERAFT

A native, compiled block-building parody written in Rust, with no browser, no web view and no JavaScript. It opens a real OpenGL window through [macroquad](https://github.com/not-fl3/macroquad)/miniquad, and the voxel renderer talks to the GPU directly with its own shaders and vertex buffers.

The game ships with **no image or audio files**. At startup it builds the whole texture atlas (grass, ores, mobs, tools, hearts, the logo) from noise and tiny ASCII sprites, and synthesises every sound effect and all the music.

Its only dependencies are macroquad (window, input, audio), [Rhai](https://rhai.rs) (the sandboxed scripting language used for code mods) and [gilrs](https://gitlab.com/gilrs-project/gilrs) (game controllers).

**Contents:** [Download and play](#download-and-play) · [Build from source](#build-from-source) · [Getting started](#getting-started) · [Controls](#controls) · [The game](#the-game) · [Multiplayer](#multiplayer) · [Modding](#modding) · [For developers](#for-developers)

## Download and play

Ready-made builds are on the [Releases page](https://github.com/BoatChinasAreFake/crispy-octo-bassoon/releases). Download the zip for your computer, unzip it anywhere and run the game inside it. On **Windows**, double-click `minceraft.exe` (if Windows warns about an unrecognised app, click **More info**, then **Run anyway**).

Worlds, settings and mods are kept in your own data folder, so a new version can be unzipped anywhere and picks up where you left off:

- **Windows:** `%APPDATA%\Minceraft` (paste that into File Explorer's address bar)
- **macOS:** `~/Library/Application Support/Minceraft`
- **Linux:** `~/.local/share/minceraft`

A few more things worth knowing:

- **Moving in.** The first time it runs, the game copies any `saves`, `settings.txt` and `mods` it finds beside it into the data folder, so unzipping over an older version brings its worlds along (the originals stay where they were). The world list and the Mods screen show the folder.
- **Portable mode.** To keep everything beside the game instead (on a USB stick, say), put an empty file named `portable.txt` next to it.
- **Updates.** Release builds check GitHub once at startup and offer a download button on the title screen when there's a newer version (using your system's `curl`; turn it off with **Update Check** in Options). The title screen shows which version you're running.
- **Crashes.** If the game ever crashes it writes `crash.txt` to the data folder. Please include it when you report the bug.

## Build from source

You need a Rust toolchain ([rustup.rs](https://rustup.rs)).

```sh
cargo run --release
```

- **Linux** also needs the X11/GL, ALSA and udev development libraries, for example on Debian/Ubuntu: `sudo apt install libx11-dev libxi-dev libgl1-mesa-dev libasound2-dev libudev-dev`. If you can't install ALSA or udev, `cargo run --release --no-default-features` builds a version without sound or controller support (add `--features sound` or `--features gamepad` to get one back).
- **Windows** builds with no extra setup.
- **macOS** should work but hasn't been tested.

## Getting started

Punch a **Tree Chunk** to get logs, then craft **Planks**, then **Sticks**, then a **Wooden Pickaxe**. From there, stone gives you a stone pickaxe, iron gives you an iron pickaxe, and iron is what you need to mine **Dimonds**.

Coal and a stick make torches, and eight cobblestone make a **Furnace**, which turns raw food into much better food. Small recipes work anywhere, but bigger ones (the furnace, chests, tools) need a **Crafting Table** close by, so make one early. The recipe book fills up as you pick things up.

Before the first night, put up a shelter and make a **Bed** (three wool and three planks): sleeping in it skips the night and sets where you come back if you die.

## Controls

These are the defaults; change them in **Options > Controls**, where every action has two slots (click one and press a key or mouse button; Esc leaves it empty). **Reset to Defaults** puts them back, and How to Play shows your keys.

| Key | Action |
| --- | --- |
| WASD / arrows | Move |
| Mouse | Look |
| Space | Jump / swim up (double-tap in creative to fly) |
| Shift | Sneak / fly down |
| Ctrl or R | Sprint |
| Left mouse | Mine / attack |
| Right mouse | Use: place a block, eat, open things (chests, furnaces, doors, stations), sleep in a bed, ride, trade, block with a shield, fire a bow, and so on |
| Middle mouse | Pick block (creative) |
| 1–9, mouse wheel | Select hotbar slot (in the inventory or a chest screen, 1–9 over one of your slots swaps it with that hotbar slot) |
| E or Tab | Inventory and crafting (shift-click a recipe to craft many) |
| T or Enter | Chat (`/` starts a command). Up and Down recall what you sent; Tab finishes a command (and the player, item, mob or place in it); Page Up, Page Down, the wheel or the scrollbar go back through the last 500 lines; long lines wrap |
| Q / Ctrl+Q | Throw one of the held item / the whole stack |
| F | Swap the held item into the other hand |
| F2 | Screenshot (a PNG in the `screenshots` folder) |
| F3 | Debug screen: position, biome, light, the nearest structure and more on the left; system, graphics card, frame times and the block you're looking at on the right |
| F5 | Toggle third person |
| F11 | Fullscreen |
| Esc | Pause menu (logs and progress, and opening your world to others, have pages of their own) |

**Game controllers** work too: plug one in any time (Xbox, PlayStation and most others).

| Controller | Action |
| --- | --- |
| Left stick (press: sprint) | Walk |
| Right stick | Look |
| A / Cross | Jump |
| B / Circle | Sneak; close screens |
| X / Square | Throw item |
| Y / Triangle | Inventory |
| Right trigger / left trigger | Mine or attack / place or use |
| Bumpers, D-pad left and right | Hotbar |
| Start / Select | Pause / third person |

## The game

Everything here is legally distinct, and most of it is a little sillier than you'd expect.

### Worlds

- **Infinite procedural terrain.** Seeded Perlin noise makes oceans, beaches, plains, forests, deserts, snowy land, swamps, jungles, badlands, taigas, savannas, birch and dark forests, meadows, ridged mountains, spaghetti caves, big caverns (the deepest ones flooded), ravines, ore veins (gold included, for all the good it'll do you), lava lakes and four kinds of sea. The sea is shallow by the shore and drops away to 10–25 blocks deep further out. Chunks generate and light themselves on background threads. The world is 256 blocks tall, with the sea at 63 (worlds made before it grew keep their sea at 40 and their land as it was, and get the extra room to build up into).
- **The land, reshaped** (new worlds from v0.6). Three slow noises shape the land, as in modern Minecraft: *continentalness* (deep ocean, the coast, inland, far inland), *erosion* (rugged to worn flat) and *peaks and valleys*. Curves turn them into the height of the land, so mountains rise inland where it's rugged, broad lowlands spread where it's worn down, and the sea floor shelves away from the coast. The biome then comes from the shape first (the sea, a river, the beach, a peak, wet lowland), then from the temperature band and moisture, with a "weirdness" noise picking the rarer variants (Cherry Groves on the hills, the Pale Garden, Ice Spikes). Swamps and mangroves are the wet lowland that was low already. Worlds made before keep generating exactly as they always have.
- **Climate and depth underground** (generation revision 8+, v0.6 Part 2 core). Deep Dark is below height 24 under inland rugged land; lush caves are damp, mild underground from height 24; dripstone occupies drier, non-frozen land at heights 10–48. The rules reuse the surface climate. Existing cave mechanics and geodes remain, and older worlds—including Part 1's revision 7—keep their generator.
- **Broader terrain** (generation revision 9). New worlds have 3× broader continentalness, erosion and valley noise, and 2× broader temperature/moisture regions. River channel half-widths target 5–11 blocks independently of broad valleys, with a smooth underwater shelf and gradual banks rather than a hard cut at the waterline. Bays and rocky headlands break up the coast; gentle slopes get sandy beaches. Mountains have foothills, passes, worn plateaus and occasional sharper summits, with snow selected by elevation and temperature. Ordinary inland borders blend tree density/species and ground cover over about 24 blocks. Older worlds, including revisions 8 and 9, keep their terrain. Common structure slots reserve space around mansion candidates.
- **Relief, islands and beach life** (generation revision 10). Broad continents keep their scale but gain rolling hills and less extensive valley flattening. Ordinary offshore islands have wooded or cold interiors and gentle sandy shores. Mushroom islands use a much sparser 4,096-block candidate grid and 360–520-block shaping radii, with exposed land hundreds of blocks across. Beaches gain scrub, gravel patches, fallen log clusters and sparse warm-shore turtle nests. Marine spawning uses local fish-school/dolphin-pod caps and hard world limits, checks water space, and does not share land-animal caps.
- **Beach and ocean wildlife.** Starfish and sea snails creep along shallow floors; clams, mussels and oysters form small colonies; crabs roam sandy shores and shallows. Sardines swim in schools, tuna inhabit deeper open ocean, and eels stay near the seabed. Populations use species limits and a separate 80–160-creature world cap; distant wild populations recycle while named/kept animals persist. Frozen seas are excluded from these new species. Seafood drops can be furnace-cooked into Cooked Seafood or used as fish ingredients in the Cooking Pot; shellfish also drop seashells (three craft into two Bone Dust). Starfish have no food drop. Meet wildlife for Life at the Shore; harvest seafood for Shore Supper. Protocol 47 is required for multiplayer. Coastal models use species-specific face textures: granular starfish skin, shell growth ribs and rings, snail whorls, mottled crab plates, scaled fish sides, contrasting backs/bellies and fin rays. The starfish has five evenly spaced arms with three tapering sections each and a raised centre. Screenshot modes `coastal_floor`, `coastal_fish` and `coastal_starfish` provide controlled model-review aquariums. Coastal wildlife is available in older worlds too.
- **Beach finds and tide pools** (generation revision 11). Low weathered driftwood logs replace the older full log clusters; harvest them for two Planks or use them as furnace fuel. Collect and place shell patches, including waterlogged patches that leave water when broken. Two seashells craft a decorative patch; the earlier Bone Dust recipe remains. Dune Grass grows in clumps above the wet shoreline and can be harvested/replanted on sand, gravel or earth. Sheltered tide pools have supported sand floors, checked dry rims, and shallow shell beds; coastal shellfish use their actual water depth. Beachcomber rewards coastal finds and A Little Tide Time rewards entering a pool. Existing worlds retain revisions 0–10. Revision 11 introduced protocol 48; current multiplayer uses protocol 57. `tide_pool` shows a natural basin; `beach_showcase` stages the three new decorations beside a generated beach.
- **Narrower beaches** (generation revision 12). Sandy beaches require nearby sea water, with a smoothly varying 16–28 block search reach. Low coastal plains beyond that strip use their inland climate biome. Terrain heights, rivers, oceans and rocky shores stay the same; dune grass, shells, driftwood and sheltered pools remain on eligible beaches. Existing worlds retain revisions 0–11; revision 12 introduced protocol 49; current multiplayer requires protocol 57.

- **Settling into the land.** New surface buildings check their full footprint, including river crossings. Villages check squares, paths, farms and house sites; growth skips unsuitable ground. Mansions stay in dark forest, and hot springs need cold, fairly level ground. Template structures added by mods check their footprint too and receive supporting foundations in the Overworld.
- **Climate-target mod biomes.** Mods can optionally target temperature, moisture, continentalness and erosion. Nearby targets compete in ordinary inland climate space; rivers, shores, peaks, wet lowlands and rare variants keep their built-in identity. Existing mods without targets retain their patch-selection rules. See MODDING.md and `example-mods/climate`.
- **Rivers and beaches** (new worlds). **Rivers** wind along the valley floors, down to the sea, with banks a little above the water and a sandy or gravelly bed; they freeze over in the cold. **Beaches** line the coasts with sand, and turtles like them best.
- **Biomes change by degrees** in new worlds: they come in bands of temperature, so snow always gives way to taiga before plains or forest, and the desert's heat to savanna. Grass, leaf and water colours blend over twenty-odd blocks, the ground (snow, sand, grass) is ragged where two biomes meet rather than ruled, and swamps and mangroves meet the water gently instead of dropping off a step. Biomes are a bit smaller than before, too.
- **Biomes.**
  - **Swamps** sit just around sea level: pools with lily pads, mud, and wide oaks trailing leaves. **Mangrove Swamps** are warmer, with trees standing on tangled roots over the water.
  - **Jungles** grow very tall trees, thick undergrowth, **Melons** and **Bamboo**.
  - **Badlands** rise into banded **Terracotta** under **Red Sand**, with dead bushes, cacti, more gold higher up, and no rain.
  - **Taigas** are cool spruce forests; snowy places grow spruce too.
  - **Savannas** are dry and yellow, dotted with flat-topped **Acacias**, and roamed by Gallopers.
  - **Birch Forests** are pale and tidy; **Dark Forests** are so thick with wide **Dark Oaks** and giant mushrooms that it's dim at noon.
  - **Meadows** are lush high grassland full of flowers, and the highest mountains turn to bare **Stony Peaks**.
  - **Ice Spikes** stand in the snow: tall pillars of **Packed Ice**.
  - **Mushroom Islands** rise out of the open sea, covered in **Mycelium** and giant red and brown mushrooms. Nothing hostile spawns there, only **Mushmooers**.
  - **Seas** come in four temperatures. **Warm oceans** are clear turquoise, full of coral and glowing **Sea Pickles**; **lukewarm** ones have some coral and plenty of **Seagrass**; ordinary seas grow tall **Kelp** forests; **frozen** ones are iced over and dotted with **icebergs**. Kelp grows taller over time, and breaking it brings down everything above; dry it in a furnace for **Dried Kelp**. You swim through sea plants, and breaking one leaves the water behind.
  - **Cherry Groves** cover gentle hills in pink-leaved trees and **Pink Petals**.
  - **The Pale Garden** is a grey, quiet forest of huge **Pale Oaks** on a carpet of **Pale Moss**, with moss hanging from the branches and **Eyeblossoms** that open and glow at night (see Creatures for what lives in the trunks).
  - **Acacia**, **Birch** and **Dark Oak** join the woods, each with the full set: planks, slabs, stairs, fences, gates, doors and boats.
  - Every kind of wood makes its own planks, and saplings grow into whatever tree suits where they're planted.
  - Grass, leaves and water take their colour from the biome and blend smoothly across borders: lush green jungles, dry olive savannas, cool blue-green taigas and snowfields, murky swamps and deep dark forests. Leaves have a colour of their own, a little deeper than the grass, and the green fringe on the sides of grass blocks matches the top.
  - **Firefly Bushes** blink in swamps at night, **Leaf Litter** drifts under forest trees, and **Wildflowers** fill the meadows.
- **Cave-ins** (not in Minecraft). Dig out too big a room underground and the ceiling creaks, drops dust, and after a few seconds comes down as rubble. A ceiling stays up within five blocks of a wall or a pillar, or within three of a wooden beam (a log in the ceiling), so prop big rooms up. The **Support Gauge** (copper, Zappy Dust and a stick) reads the room you're in. Natural caves have had ages to settle; only your digging can fall.
- **Spelunker's Rope** (not in Minecraft; six string and a stick make two). Click the top of a ledge and it goes over the edge and unrolls down the hole (up to 32 blocks); against a wall it hangs straight down. Climb it like a ladder. It stays put, maps mark it, and breaking any part takes the whole rope back.
- **Cave biomes.**
  - **Dripstone caves** bristle with **Pointy Rock**: stalagmites growing up from the floor and stalactites hanging from the ceiling, and a stack of them makes one long spike (a broad base, a middle and a point). Stalactites drop when you break what holds them (or stand under a loose one too long), and landing on a stalagmite hurts twice as much.
  - **Lush caves** have **Moss** floors, **Azaleas**, and **Cave Vines** hanging with **Glow Berries** that light the place up. Pick and eat the berries, plant them under a ceiling for a new vine, and climb the vines like ladders. Bone Dust on moss spreads it.
  - **Amethyst geodes**: smooth basalt and calcite shells round a hollow of **Amethyst**. **Budding Amethyst** slowly grows buds into clusters, which break into **Amethyst Shards** for **Tinted Glass**.
- **World options.** The Create World screen sets a name, game mode and seed (any text works), **Keep Inventory**, **Hardcore**, and how the world is generated: **Structures** (None, Few, Normal or Lots), **Biomes** (Small, Normal, Large or Huge) and **Land** (Flat-ish, Normal, Hilly or Amplified). Worlds made before these options keep generating exactly as they always did. New worlds also get proper village houses, fewer villages, bigger and rarer Scorchlands fortresses, taller ice spikes, a rarer Deep Dark, and buildings that settle into the land around them instead of floating or sitting in a hole.
- **Multiple worlds.** **Singleplayer** opens a world list with each world's name, mode, seed, when it was last played, and size. Play, create, rename or delete worlds. Each world is its own folder, `saves/<world>/`; only your edits are stored, and the terrain regenerates from the seed.
- **Backups.** Each time you open a world, a copy of it as it was goes into `backups/<world>/`, and the last 5 are kept. **Backups** on the world list shows them, and **Restore as a New World** makes a separate world from one, so restoring never overwrites anything.
- **World settings.** **World Settings** in the pause menu changes a world's rules at any time (for its owner; joined players can look but not touch): **Keep Inventory**, **Difficulty** (Peaceful has no monsters or hunger; Easy halves monster damage and stops starving at five hearts; Hard hits 50% harder and starving can kill), **Daylight Cycle**, **Weather Cycle** (with a button to change the weather now), **Seasons** and the **World Border** (1,000 to 10,000 blocks from the middle; it glows red as you get close).

### Places

The generator builds things, the same in every copy of a world. `/locate` finds the nearest of each, and F3 shows the closest one.

- **Villages** on plains, deserts, savannas, taigas and snowy land: a well on a square with lamp posts, paths, three to six houses (each with a Hmmer), watered farms and a **Clanker** on guard. In newer worlds the houses are proper ones: cottages, gabled houses and two-storey houses with pitched roofs, glass windows and a ladder up to the bedroom. Worlds made now build each village its biome's way: desert villages in **Mud Bricks** and sandstone under flat roofs, savanna villages in acacia under **Thatch**, taiga villages in dark oak on mossy footings, snowy villages in birch under **Frost Shingles**, and plains villages as before. Every house has a workstation for whoever lives there. (Mud Bricks, Thatch and Frost Shingles are only found in villages.)
- **Dungeons** deep underground around a **Monster Cage (Still Occupied)** that keeps spawning monsters while you're nearby, plus **Ruined Towers**, the odd lone **Hut** (definitely not a village), desert **Wishing Wells** and **Pilferer Outposts** (dark oak watchtowers with arrow slits and a lookout, kept well away from villages).
- **Desert Pyramids** with a hidden room of chests over a **TNT trap** (mind the pressure plate), **Jungle Temples** with **Tripwires** (string between two **Tripwire Hooks**) that set off arrow dispensers, **Abandoned Mineshafts** of rails, beams and **Cobwebs** with loot in parked minecarts, and **Igloos**, some with a ladder down to a basement holding two prisoners.
- **Ocean Monuments** stand on the deep sea floor, built of **Prismarine** and lit with **Sea Lanterns**. **Guardians** charge lasers at you, three **Elder Guardians** curse anyone nearby with **Mining Fatigue**, and the middle hides blocks of gold. Frame a **Conduit** (made from a **Heart of the Sea**, found in buried treasure, and **Nautilus Shells**) with prismarine for **Conduit Power**: no drowning nearby.
- **Shipwrecks** on the sea bed, often with a **Treasure Map** to **Buried Treasure** in a beach. **Coral reefs** grow in warm, shallow oceans.
- **Woodland Mansions** hide deep in the dark forests of new worlds: three storeys of dark oak, with a cross of corridors on every floor, libraries, storerooms, bedrooms, dining rooms and ladders up, full of **Pilferers** and their **Invoicers**. Two rooms upstairs have no door at all: find the gap and dig in for the best loot in the house. They're rare: a **Cartographer** sells a **Woodland Explorer Map** to the nearest one.
- **Dig sites** of four lost cultures (see Archaeology).
- **Trial Chambers**: copper-and-tuff halls whose **Trial Spawners** wake as you come close and send out a wave; beat it for a **Trial Key** to open a **Vault**. **Breezes** bounce around throwing **Wind Charges**. Drink an **Ominous Bottle** first for bigger waves, **Ominous Vaults** and a chance at a **Heavy Core** (which makes the **Mace**: fall on something from a height and it lands harder the further you fell).
- **The Deep Dark.** Wide deepslate caverns near the bottom of the world, carpeted in **sculk** that listens. **Sculk Sensors** hear footsteps (sneak to walk silently) and give off Zappy power; they wake **Sculk Shriekers**, and the fourth shriek summons **The Hush**, blind and enormous, which goes wherever it last heard something. **Hushed Cities** stand deep in the caverns, with chests of Echo Shards, templates and enchanted books.
- **The Scorchlands.** A second, hotter world under a bedrock sky. Build an obsidian frame (at least 4 wide and 5 tall), light it with a **Sparker** and stand in the portal. Every block there is eight here. It has a lava sea, **Grumblers** (hit one and the whole crowd comes), Scorch Brick **Fortresses** (in newer worlds, a two-storey hall with long arched bridges and roofed walks out to tall towers) with **Sizzlers** that throw fireballs, **Weepers** that cry explosive ones (hit a fireball to send it back), **Strutters** you can ride across the lava with an **Ember Shroom on a Stick**, and **Snouts** who trade for gold you throw them. The fortresses' halls are walked by tall, sooty **Charred Rattlers**, whose blades leave you **Wilting** (health drains, all the way down), and patches of **Sorrow Sand** (slow going) lie on the higher floors. In new worlds the land down there has more shape: the basalt deltas' floors are jagged with spikes, the soul sand valleys open into wide flat halls, the fungus forests roll over hills, great rock pillars run from the lava up to the roof, ledges jut from the cavern walls, magma lines the lava sea's shores and glowrock hangs in bigger clusters.
  - **Its biomes.** Between the old Scorchrock wastes lie four more, each with its own haze and its own creature. **Crimson Forests** grow huge **Crimson Fungi** (stems capped with wart and glowing **Shroomlights**) on red **Nylium**, with roots, fungi and **Weeping Vines**, and red spores in the air; **Tuskers** live there, big boars that charge and toss you (feed two **Crimson Fungus** to breed them; they can't abide **Teal Fungus**, held or put down). **Teal Forests** are the same in teal, and calmer: **Sporelings** potter about, and hitting one gets you a faceful of spores that leave you slow and weak; Starers like it there too. **Basalt Deltas** are grey with ash: **Basalt** and **Blackstone**, basalt pillars, pools of lava, and **Magma Blocks** that burn your feet unless you sneak. **Magma Bloops** bounce about and leave **Magma Cream** (four make a Magma Block, and it brews Fire Resistance). **Soul Sand Valleys** are drifts of Sorrow Sand and **Soul Soil** around great fossil ribs of **Bone Blocks**, lit by blue **Soul Fire** that burns for ever (a Sparker lights it on soul ground). **Wisps** drift over them, blue flames that set you alight; water puts them out, and their **Soul Embers** make Soul Lanterns. Bone Dust on a fungus standing on its own Nylium grows a huge one. `/locate crimson` (or teal, basalt, soul) finds them.
  - **Snout Bastions** stand over the lava sea: big crumbling keeps of **Blackstone Bricks**, gilded here and there, with ramparts, chests and gold lying about, **Tusker** pens in the back corners and a **treasure room** in the middle, round a core of Magma Blocks crowned with gold, with the best chest in the Scorchlands. Snouts live there, guarded by **Snout Brutes**, who are always cross and can't be bought off. Open a chest anywhere near a Snout and it's cross too.
- **The Wilter.** A second boss, built rather than found: four Sorrow Sand in a T with a **Charred Skull** (a Charred Rattler drops one now and then) on each of the top three. It gathers itself for a few seconds, bursts out, and flies about throwing wilting skulls; past half health it throws three at a time and dives at you. Beat it for the **Wilter Star**.
- **The Hollow.** The endgame. Throw **Staring Eyes** to find a buried **Crypt**, fill its twelve **Eye Frames**, and drop through to a floating island circled by the **Hollow Wyrm**. Break the **Wyrm Crystals** that heal it, then beat it for experience, the **Wyrm Egg** and a portal home. The outer islands have spires with **Gliders**, **Boom Rockets** and **Hollow Boxes**. In new worlds the Crypt is more than one room (corridors lead off to a library, a store room, cells and a flooded well room, with loot to find), the outer islands are lumpier, with craggy undersides and rocks drifting round them, and their spires are tall round towers with glowing windows and tips.
- **Three worlds in one.** The ordinary world, the Scorchlands and the Hollow are separate dimensions, each with its own coordinates centred near 0 (F3 says which one you're in), and none of them has an edge. Things left in one stay there: mobs, dropped items, boats and carts, chests and signs. Personal Chests and backpacks are the same everywhere. Dying anywhere brings you back to your spawn in the ordinary world, a compass only finds spawn in the ordinary world (and a lodestone only in its own dimension), and waypoints and the death marker remember which dimension they're in. Worlds saved before the dimensions were separated are sorted out the first time they're opened (players, buildings, chests, mobs and region files all move to the right dimension).

### Weather and skies

- **Day and night** (20 minutes) with a sun, moon, stars, sunrise and sunset glow, and clouds (which carry on out over the distant land).
- **Weather.** Rain (snow in cold places) and now and then a thunderstorm. It stops at the first block in its way, so caves and roofs stay dry. Rain waters crops and makes fish bite; thunderstorms are dark enough for monsters, and lightning hurts, sets off TNT and starts fires.
- **Snow piles up.** While it snows, snow settles on open ground near players, up to half a block deep, and melts away slowly once it stops.
- **Moon phases.** The moon waxes and wanes over eight days. A full moon brings more monsters out; a new moon is quieter.
- **Seasons** (off unless you turn them on): eight days each. Leaves go rusty in autumn, crops grow faster in spring and hardly at all in winter, and winter rain falls as snow almost everywhere.
- **Nicer skies.** Shooting stars on clear nights, **auroras** over the snow, and a **rainbow** after the rain. They fade in and out, and so does the rain (and its patter).

### Surviving

- **Health, hunger and breath.** A food bar of ten drumsticks with Minecraft's rules: effort uses hidden saturation first, a full bar heals you, a low one stops you sprinting, and an empty one starves you (to half a heart, unless it's Hard). Raw food barely helps; cooked food keeps you full much longer. Underwater a row of bubbles shows your **breath**: 15 seconds of it (a **Turtle Shell** adds 10), then you start to drown.
- **The chill.** Out in the cold at night (snowy land, taigas, the peaks, or just very high up), or wet in the cold, you get **Chilly** (your health stops coming back by itself) and then **Freezing** (slow and hungry, and at the very worst it bites). A fire, a warm lamp, a roof, the day or a hot meal warm you up; frost creeps round the edge of the screen as it bites.
- **Hot springs.** Steaming pools on warm **Spring Rock** in the snowy mountains of new worlds. A soak gets rid of the chill fast and slowly heals you.
- **Physics.** Collision, gravity, sprint-jumping, sneaking (it stops you walking off ledges), swimming and fall damage. **Lily pads** hold you up from above but let you swim up through them.
- **Experience.** Orbs from mobs, ores, furnaces and fishing float toward the nearest player. Levels follow Minecraft's curve and pay for enchanting and anvils.
- **Durability.** Tools, weapons and armour wear out with Minecraft's numbers (59 uses for wood up to 1561 for Dimond); a bar under the item shows how worn it is.
- **Combat.** Weapons take a moment to be ready after a swing (a bar under the crosshair); a fully charged blow can crit, knock back and sweep. **Shields** block hits from in front. Armour comes in Woolly, Copper, Iron, Golden, Dimond and **Scorchite** tiers, shows on your character, and takes up to 80% off damage. Bows, crossbows, spears, the **Mace** and thrown **Wind Charges** round out the arsenal.
- **Beds and sleep.** A bed faces away from whoever put it down. Right-click it at night to lie down; the screen fades, and if nobody gets you up the night is skipped and you step out beside the bed. Jumping, sneaking or getting hurt gets you up early. It refuses if monsters are nearby, sets where you come back after dying, and clears a storm. (Don't try it in the Scorchlands or the Hollow.)
- **Death.** Dying drops your things where you fell (they wait five minutes), and everyone is told how you died ("Bot5 was struck down by a command. Harsh."). Held maps and the **Recovery Compass** show where.
- **Game modes.** Survival, Creative (flight, instant breaking, infinite blocks) and **Spectator** (fly through everything, touch nothing). **Hardcore** is one life on Hard; die and you can only spectate.
- **Advancements** (235 of them, in four tabs, each with a toast and a fanfare) and **Statistics** (blocks, crafts, kills, distances, fish, food and more) are kept per world.

### Building

- **Real light.** Every block has sky and block light from 0 to 15, spread Minecraft style. Caves are properly dark, torchlight is warm, and a held torch lights your way. Torches go on the floor or **on walls**, leaning out from them.
- **Flowing water and lava.** Water spreads 7 blocks and lava 3; two water sources make a third; water meeting lava makes obsidian, cobblestone or stone. **Buckets** move them about. Sand, gravel, Concrete Powder and anvils **fall**.
- **Blocks for building.** Slabs, stairs and doors (with see-through windows) in several materials; fences, gates, ladders, trapdoors and glass panes that join up; dyes for wool, glass and concrete; **Concrete**, **Glazed Terracotta**, **Candles**, **Chains**, **Scaffolding**, Resin Bricks, copper that weathers green (wax it to stop it), bamboo, and plenty more.
- **Decoration.** Signs (dye their words, or make them glow), item frames, paintings, banners with patterns from the **Loom**, armour stands, lanterns, campfires and **Books** you write and put on a **Lectern**.
- **Home and craft.** A **Cauldron** holds three levels of water (buckets and bottles fill and empty it); drop a dye in and dip **woolly armour** to colour it (everyone sees it), or dip dyed armour or a patterned banner in plain water to wash it. A **Composter** turns plant scraps into Compost. **Groaner**, **Rattler** and **Hisser** heads drop now and then, to put on a shelf. A **Lightning Rod** (three copper ingots) takes any strike within 32 blocks, so nothing nearby catches fire, and gives off a pulse of Zappy power.
- **Fire** clings to the sides of what's burning and creeps up and along it, spreads through wood, wool, leaves and hay, sets off TNT and goes out when there's nothing left. **TNT** chain-reacts.

### Crafting and stations

- **Crafting.** Over 140 recipes. Small ones (four ingredients or fewer) work anywhere; bigger ones need a **Crafting Table** within four blocks. The **recipe book** lists everything you've discovered, with search, tabs, a "craftable now" filter and a **pin** that keeps one recipe's shopping list on screen.
- **Furnaces**, **Smokers** (food, twice as fast), **Blast Furnaces** (everything else, twice as fast) and **Campfires** (cook without fuel).
- **Enchanting.** Put a tool, weapon or armour and some gold in an **Enchanting Table** and pick one of three offers; bookshelves around it make them stronger. Efficiency, Sharpness, Protection, Unbreaking, Fortune, Silk Touch, Looting, Frost Walker, Riptide and Mending (treasure only). **Enchanted Books** carry them to the anvil.
- **Anvils** mend tools with their material and merge two of a thing (or a tool and a book). Each use may chip the anvil. The **Grindstone** strips enchantments.
- **Smithing.** The **Smithing Table** upgrades Dimond gear to **Scorchite** (with a template from the Hushed Cities and scrap from the Scorchlands) and adds **armour trims** everyone can see.
- **Brewing.** Fill bottles at water and brew Healing, Speed, Fire Resistance, Night Vision, Leaping, Strength and Regeneration, then splash versions you throw.
- **Beacons.** The Wyrm Egg makes a **Beacon**: on an obsidian pyramid it gives everyone nearby an effect of your choice. Use a **Wilter Star** on one with all three layers for a **Starred Beacon**: the effect at level II, Regeneration too, and half as far again.

### Storage

- **Chests** hold 27 stacks, and come in tiers: sneak and right-click one holding 8 iron, 8 gold or 4 Dimonds to upgrade it where it stands, contents and all, to an **Iron Chest** (45), a **Gold Chest** (54) or a **Diamond Chest** (72). The bigger ones can be crafted outright too. Lids swing open while you're looking inside. Chests, barrels and your inventory have a **Sort** button.
- **Backpacks.** Right-click with one to open your pack, a chest you carry. A **Backpack** (an empty Bundle, four wool and two string) reaches 27 slots of it, a **Big Backpack** 45 and a **Huge Backpack** all 72. The pack belongs to you, not the bag, so upgrading loses nothing and a dropped backpack doesn't spill your things.
- **Bundles** hold a mix of small stacks, **Hollow Boxes** keep their contents when you break them, and every **Personal Chest** opens the same private storage, yours alone.
- **Items on the ground** bob, merge, float and vanish after five minutes. **Q** throws one, **Ctrl+Q** the stack. Press **F** to put something in your **other hand**.

### Creatures

- **Animals.** Oinkers, Fluffers (shear them), Mooers, **Mushmooers** (red-spotted; shear one for five mushrooms and a plain Mooer), Clucksters, Squawkers, Sneakers (foxes that steal), Ribbits (frogs that leap out of the water to snatch small Bloops and leave **Froglights**), Rollos (armadillos), Goats, Axolotls, Turtles (their eggs crack if you land on them or walk over them: sneak past), Dolphins (feed one a fish and it leads you to a shipwreck), Pandas, Polar Bears, Llamas (pack animals), Bees, Fishies and **Sniffers** (hatched from eggs found at dig sites; they dig up ancient seeds). Most come in a few colourings, and babies take after a parent.
- **Breeding and taming.** Feed animals their favourite food and they make babies. Tame **Woofers** with bones: they follow you, fight for you and wear **Woofer Armour**. Mobs find their way round walls and through doors.
- **Leads.** Four string and a Goo make two. Put one on an animal (or a pet, or a Snow Golem) and it follows on a rope; right-click a fence to tie it up there. Stretch it too far and it snaps.
- **Things to ride.** **Gallopers** (give one iron, gold or dimond **Galloper Armour** from a chest), **Donkeys** and **Mules** (a Galloper's and a Donkey's foal), which carry a chest you can fill (sneak and right-click to open it), **Camels** (with a seat for a friend and a dash) and **Rotsteeds** with a saddle; **Strutters** over lava; and **Floaties**, big gentle fliers grown from a Dried Floaty, with a **Harness** and four seats.
- **Monsters.** **Groaners** (and **Soggy Groaners** in the sea, some throwing spears), **Rattlers** (bony archers), **Hissers** (they explode), **Webbers** (they climb walls), **Bloops** (they split), **Starers** (don't look them in the eye), the **Creaking** (it only moves when nobody's looking; break its heart in a Pale Oak to be rid of it), **Witches** (they throw Slowness, Poison and Weakness potions and drink their own to heal), **desert Groaners** that make you hungry, **snowy Rattlers** whose arrows slow you, and the Scorchlands' lot (Tuskers, Sporelings, Magma Bloops, Wisps and Snout Brutes among them). They come out on the surface at night (and in thunderstorms), and in caves day and night, on any floor out of the daylight and away from torchlight (except the Deep Dark, which keeps to itself). Only wild monsters count towards how many there can be at once, so a village full of Hmmers or a sea full of fish doesn't keep them away, and monsters left far behind wander off to make room for new ones nearer you.
- **Hmmers.** Villagers with eight jobs and five trades each, paid in gold ingots (finally, a use for them). They give regulars a discount. A Groaner can turn one into a **Zombie Hmmer** (cure it with a Golden Chop), and a **Wanderer** turns up now and then.
- **Village life.** Each Hmmer claims a bed and a workstation near home, and its job comes from the workstation: a Composter (Farmer), Lectern (Librarian), Smithing Table (Smith), Barrel (Fisher), Brewing Stand (Cleric), Blast Furnace (Armourer), **Cartography Table** (Cartographer; 4 planks and 2 wheat) or Smoker (Butcher). A Hmmer with no job takes the job of the first free workstation it finds; break a workstation before anyone has traded with its Hmmer and it's out of work. Hmmers work at their workstation in the morning, potter about in the afternoon, and go indoors to bed at night (and lie in it). While a village has a free bed, two grown-ups now and then have a baby, who grows up and looks for a bed and a job of its own.
- **Trade levels.** Every trade with a Hmmer (by anyone) teaches it a little, and it goes from **Novice** to Apprentice, Journeyman, Expert and **Master**. A Novice offers its first two trades, each level opens the next, and a Master adds a special of its job's (an enchanted Dimond sword from a Smith, a Totem from a Cleric, a saddle from a Butcher...). Going up a level restocks everything. The trade screen shows how far along it is.
- **Reputation.** Each village has its own opinion of you, shown on the trade screen. Trading (+1), curing a Zombie Hmmer (+25), seeing off monsters near it at night or in a raid (+2) and saving it from a raid (+30) make it like you; hitting a Hmmer (-8) or its Clanker (-12), or killing one (-30), make it remember. Grudges fade a little each day. A loved player pays up to a quarter less and now and then gets a flower from the Clanker; a disliked one pays up to half again, the Hmmers keep their distance and the Clanker comes after them; a hated one can't trade there at all.
- **Growing villages.** A village with people about keeps itself up. Once a day, a village with every bed taken starts on a new house: first where its plan left a gap, then out at the ends of its paths. It goes up a few blocks at a time over a couple of minutes, in the village's own style, with a bed and a workstation, so there's room for a baby and work for a grown-up. Trees and plants are cleared away; anybody's building is left alone. A village also builds itself a new Clanker for every six Hmmers it has (at most one a day).
- **Shop Stalls** (an original). Make one (4 planks, 2 wool and a chest) and the first to open it owns it. Put out up to nine lots and price each in Gold Ingots (left-click a price to raise it, right-click to lower it, shift for 5 at a time). In the daytime, now and then, a Hmmer from within 24 blocks walks over and looks: it buys a whole lot if the price is fair for what it's worth to the village just then, and the likelier the cheaper it is; the gold goes in the till. Demand rises and falls with what the village needs: things its Notice Board is asking for are wanted more, food more the more Hmmers there are to feed, and each sale satisfies the village a little for a while. The stall shows how each lot is doing (Wanted!, Steady, Slow, Glutted).
- **The village paper** (an original). Each morning there's a fresh issue: take one (free, once a day) from any village's Notice Board, and hold it to read it. Every village has its own paper (The Little Hmmington Gazette, The Muddlecombe Bugle...), reporting what went on in the world yesterday: trees punched with bare hands ("Local Player Ann Punches Tree"), unfortunate ends, raids won and lost, Hissers going off near a village, new houses and Clankers, bounties, babies, Hmmers made Master, sales at stalls, bosses beaten and storms. A quiet day still makes the paper.
- **Notice Boards and bounties.** The first time someone comes by, a village puts up a **Notice Board** on its square (you can make one too: 6 planks, 2 sticks and a book, but nobody pins anything to one out in the wilds). It holds three bounties: two things the village is short of ("Bring 20 Wheat") and a nuisance it wants seen off ("Defeat 5 Rattlers"; Desert Groaners count as Groaners, and Snowy and Charred Rattlers as Rattlers). Bring bounties are handed in all at once; Defeat bounties count what you've beaten since they went up. Each pays Gold Ingots and experience once per player, and the village thinks a little better of you (+3). New bounties go up every other day.
- **Raids.** Defeat a patrol captain for **Bad Omen**, walk into a village, and waves of **Pilferers**, **Hacklers**, **Invoicers** (who summon flying **Fees**) and **Rampagers** march on the square. Win and you're a **Hero of the Village**. Ring the **Bell** to make raiders glow.
- **Golems.** **Clankers** guard villages. Put a pumpkin on a block of copper for a **Copper Golem**, which sorts a **Copper Chest** into the chests nearby, or on two blocks of snow for a **Snow Golem**, which pelts monsters with snowballs, leaves snow behind it in the cold, and melts in the heat and the rain.
- **Underground and underwater.** **Bats** roost in dark caves, **Glow Squid** shine in dark water and drop **Glow Ink Sacs** (rub one on a sign to make its words glow), and the **Allay** fetches: hand it an item and it brings you every matching one it finds lying around (take it back empty-handed). Pilferer Outposts keep one caged.
- **Name Tags** name a mob for good.

### Farming, fishing, bees and digging

These are deliberately over-engineered.

- **Farming.** Every tilled block has its own soil. Growth multiplies water, light, the right **nutrient** for the crop (top it up with Compost, Bone Dust and Wood Ash), crop rotation and company. Weeds steal nutrients, Clucksters peck at seedlings unless a **Scarecrow** is near, and a **Soil Probe** explains it all in one long sentence. Saplings grow into trees, and leaves decay when the tree's gone.
- **Cooking, over-engineered.** Put a **Cooking Pot** (three iron and a bucket) on a campfire and drop in up to eight ingredients. What kinds go in decides the dish: **Hearty Stew**, **Fish Chowder**, **Garden Soup**, **Mushroom Risotto**, **Fruit Compote**, **Porridge**, **Hunter's Pie**, **Kelp Broth**, or a **Hodgepodge** when nothing fits. Serve it into a **Bowl** whenever you like, and it's graded **F to S**: on how well the ingredients suit the dish (and how many different ones went in), how fresh they were (leftovers less so, Groaner Goo never, and a pot left off the fire goes stale), the seasoning (a pinch of **Salt**, boiled out of a bottle of water, and some **Herbs**, from flowers; not too much, and no salt in the sweet things), and the cooking time (each dish has its own; too soon is underdone, too long is burnt). Right-click the pot empty-handed to see how it's coming along. Better meals fill you more, give a short buff of their own (Strength, Night Vision, Speed...) and warm you up. The **Cookbook** fills in as you go, with your best grade for each.
- **Fishing.** Fish nibble, then bite; big ones start a tug-of-war on the line. Catches depend on biome, time of day, the water's size and depth, bait and your **Angler level**. The **Fishing Log** records the biggest.
- **Bees.** Every hive keeps a colony record: population, health, flowers, weather, privacy and its **queen**. Honey takes after the flowers, mites spread between crowded hives, full colonies swarm, and the **Hive Tool** tells you everything.
- **Archaeology.** Four cultures' dig sites hide **Suspicious Sand** and **Gravel**. Brush carefully (push too hard and the find cracks), go deeper for rarer finds, restore encrusted relics at a **Restoration Bench**, and fill in the **Field Journal**.

### Zappy Dust and machines

Legally distinct redstone, simplified. Power comes from levers, buttons, pressure plates, Zappy blocks, **Observers** and **Detector Rails**, and travels up to 15 blocks of dust. It lights **Zappy Lamps** and **Copper Bulbs**, opens doors and sets off TNT. **Zappy Torches** invert, **Repeaters** delay, **Comparators** read containers, **Pistons** push and pull, **Dispensers** fire whatever's in them, **Hoppers** move items between containers, a **Crafter** crafts when powered, and **Note Blocks** play.

More parts: a **Daylight Sensor** gives power by day (use it to turn it round, and it gives power by night instead), a **Target** lights up for a second when an arrow hits it, a **Dropper** drops one item when powered (or passes it into a container in front), an **Activator Rail** throws riders out of carts and stops hopper carts from collecting, a **Trapped Chest** gives power while someone has it open, and a **Calibrated Sculk Sensor** hears only one kind of thing (footsteps, blocks, music or mobs; use it to change which).

### Getting around

- **Boats** (one for each wood; they fly along ice, faster still on packed ice), **Minecarts** (plain, chest and hopper) on **Rails**, **Powered Rails** and **Detector Rails**.
- **Gliders**: wear one, jump off something high and press Jump again; use a **Boom Rocket** for a push.
- **Compass** (points home, or to a **Lodestone**), **Maps** (zoom out, shade slopes, mark banners and where you died), **Spyglass**, **Treasure Maps** and **Woodland Explorer Maps**.
- **Waypoints.** `/wp add <name>` marks where you stand; each one shows as a coloured label with its distance, and on held maps. `/wp list` and `/wp remove <name>` do what they say.

### Music and sound

- Every sound is synthesised: footsteps by material, every mob, explosions, splashes and bubbles, rain that's muffled under a roof, and things rumbling in deep caves. Sounds fade with distance.
- **Clean on good speakers.** Sounds reach the mixer as 32-bit float at 44.1 kHz, upsampled through a band-limited filter, so nothing harsh is added above the sound itself (the audio library's own converter used to add a fizz that studio monitors made plain). A look-ahead limiter on the mix keeps busy moments from clipping, and quiet passages and fades are left untouched.
- **Music** drifts in now and then, picked to suit the moment: calm and pentatonic by day, slow and minor at night, broken chords in three in the morning, low open fifths in caves and other dimensions. Every piece (and every disc) is composed by the rules: in a key, over chord progressions that go somewhere, in 8-bar periods whose first half ends on the dominant and whose second comes home, with the tune's strong beats on the chord, a step back after every leap, the theme repeated and answered, and chords voiced to move as little as they can. Each is about three minutes long, with an introduction, the theme, a contrasting middle, the theme again (ornamented) and an ending that cadences home, played on a piano-like instrument with a little hall reverb.
- **Note Blocks** (the block underneath picks the instrument) and a **Jukebox** with eight discs the game composed itself.
- `minceraft --export-sounds <dir>` writes every sound out as a WAV (32-bit float, 44.1 kHz).

### Graphics and settings

- **Renderer.** Hidden-face culling, ambient occlusion, smooth lighting, frustum culling, sorted translucent water, greedy meshing and render distances up to 32 chunks. Each chunk section stores a small palette of its blocks plus packed indices (F3 shows the bits per block). Chunks are meshed on worker threads, so walking into new land doesn't take time out of the frame (a block you break right by you still vanishes on the same frame).
- **Low Memory** (Video Settings) keeps fewer chunks loaded past the render distance, uses fewer mesh threads, turns Distant Land off, keeps the music as 16-bit (from the next start) and only the last music disc played. Every sound is kept in mono now, which halves what the music takes for everyone.
- **Region files are compressed** (with a small compressor written into the game), several times smaller than before; older worlds load as they are and are compressed as they're saved.
- **Video Settings.** Render distance, brightness, Max FPS (a hard cap, paced precisely even where the system's timer is coarse: no frame comes sooner than its share of a second, even after a slow one), VSync (switched straight away where the graphics driver allows, and with frames also held to the display's refresh rate in case the driver ignores it), anti-aliasing, particles, view bobbing, fog, clouds (fancy, fast or off), waving leaves, **shiny water** (it reflects the sky, more at a glancing angle, and the sun glints off ripples), smooth or flat lighting, **Shadows** from the sun and moon (with a quality setting from Potato to "What the hell is wrong with you?"), water depth with caustics, and fullscreen.
- **Distant Land.** Past the loaded world, a low-detail picture of the land carries on to the horizon, so mountains and coasts show from far off: **Blocky**, **Smooth** (gently sloped tiles) or **OFF**.
- **Accessibility** (Options > Accessibility). **Subtitles** caption what you hear, with an arrow toward it; **Colour-blind** mode makes red and green differences show up as brightness and blue; the **Narrator** reads each menu's title and the button under the pointer aloud, using the computer's own speech (Windows and macOS have it built in; on Linux, install `speech-dispatcher` or `espeak-ng`); **Sneak** and **Sprint** can be **Toggle** (press once to start, again to stop) rather than held; **High Contrast** puts menus and the HUD's text on solid black with thick white edges and yellow for what's under the pointer. **UI Size** (Options) scales the menus and HUD.
- **Languages.** **Options > Language** picks a translation a mod brought. Every line of text in the game is listed in [`lang/strings.txt`](lang/strings.txt), the starting point for a translation; see MODDING.md and `example-mods/pirate` (the game in Pirate).
- **Creative inventory.** Tabs (Building Blocks, Plants and Nature, Tools and Armour, Food, Everything Else), a search box, and a button to switch between the palette and your own inventory. Drag the scrollbar or use the wheel; right-click an item in your hotbar to pick up another of it. Save up to three hotbars and load them back.
- **Skins.** **Options > Skin** picks one of six looks (Stove, Alexa, Kettle, Toaster, Blender, Fridge), or a skin of your own: put a 64×64 PNG in the usual layout (old 64×32 ones work too) in the `skins` folder in the data folder, and it's on the list after the six. The folder starts with `template.png`, Stove unfolded, to paint over. In multiplayer the server passes your skin to everyone else.
- **Commands.** In single player, and for operators on a server: `/tp`, `/give` (items by key or name, like `/give dimond pickaxe`), `/gamemode`, `/weather`, `/seed`, `/summon`, `/locate`, `/setblock` and `/kill`. `/help` lists them all.
- **Saved settings** live in `settings.txt` in the data folder: plain `key=value` text you can edit by hand. Anything it can't make sense of falls back to the default.

## Multiplayer

Open any world to friends from the pause menu, or run a headless dedicated server. Blocks, players, mobs, weather, chat, containers and the rest stay in sync, and the host remembers every player's things between visits. Everything sent is encrypted. It all uses the Rust standard library, with no accounts and no central server.

### The server list

The **Multiplayer** screen lists games on your network (they turn up by themselves within a couple of seconds) and the servers you've saved, each with its player count and ping, or why it can't be reached (offline, or on another version). Click a server to choose it and double-click (or **Join**) to play; type an address and press **Add to List** to keep it, **Remove** to drop it, **Refresh** to ask them all again. The list is kept in `servers.txt` in the data folder.

The game remembers the key each server showed the first time you reached it. If a server's key ever changes, joining stops with a warning, since someone may be pretending to be that server. If you know its owner reset it (a reinstall, a new computer), choose it and press **Forget Key**.

### On the same network (LAN)

1. **Host:** start or continue a world, press **Esc**, then click **Open to LAN**. The pause menu shows your address, for example `192.168.1.20:25565`.
2. **Friends:** on the title screen, click **Multiplayer** and type a name. The host's game shows up in the list by itself: double-click it. (Or type its address and click **Join**; for a second copy on the same computer, use `127.0.0.1`.)
3. Press **T** to chat.

### Over the internet, from your own world

1. *(Optional, recommended)* Type a password in the **Password** box on the Multiplayer screen before hosting. The world uses it.
2. In your world, press **Esc** and click **Open to Internet**. The game asks your router (using UPnP) to forward TCP port 25565 to your computer, and shows the address to share, for example `203.0.113.7:25565`.
3. Friends join with that address and the password.

If the router says no, the pause menu says so, and you can forward **TCP port 25565** by hand in the router's settings. If it says you're **behind a second NAT**, your internet provider shares one public address between many customers and nobody outside can reach you, however the port is set. If your connection has IPv6, the `[IPv6]:port` address the pause menu shows usually works directly (as long as your friends have IPv6 too). Otherwise, run a dedicated server somewhere with a public address.

### Playing together

- **Teams.** `/team create <name> [colour]` makes one (red, blue, green, yellow, purple, aqua, orange or pink), `/team join <name>` and `/team leave` move you, `/team colour <colour>` repaints yours and `/team list` shows them all; operators can `/team remove` one. Teammates' names float in their team's colour, they show as coloured dots on each other's maps, and `/tc <text>` talks to your team alone.
- **PvP and friendly fire.** Players can't hurt each other until an operator says `/pvp on`. Then a team decides whether its own members can (`/team friendlyfire on|off`; off to begin with).
- **The Trading Table** (4 planks, 2 gold, a chest). Two players open it and each puts their offer on their own side. When both press **Accept**, the host swaps the two sides in one go; changing anything takes both Accepts back, so nobody is caught out.
- **Land claims** (opt-in: an operator turns them on with `/claims on`). A **Survey Post** (2 logs, a compass, 2 sticks) claims 16 blocks all round it, up to three per player. Nobody else can build there or open your chests unless you name them with `/claim friend add <name>`. `/claim` says whose land you're on, and claims show on maps.
- **Post Boxes** (5 iron, a chest, a feather). Right-click one to open your outbox: put up to nine stacks in, write who it's for and a note, and press **Send**. About twenty seconds later a slightly put-upon Cluckster brings it to them, or, if they're not playing, it waits in their **inbox** (any Post Box, the **Inbox** button) and they're told there's post when they next join.
- **Minigames.** Mods can keep scores, run timers and a countdown, put a sidebar on everyone's screen, and sort players into teams and spawn points (see SCRIPTING.md and `example-mods/digrace`).

### Dedicated server

For a VPS, a Raspberry Pi or a spare PC. It has no window and needs no GPU or sound card.

```sh
cargo build --release
./target/release/minceraft --server --password hunter2
```

| Option | What it does |
| --- | --- |
| `--port N` | TCP port to listen on (default 25565) |
| `--world FILE` | The world save (default `saves/server.mncr`, created if missing); edits go in region files beside it |
| `--seed N`, `--creative` | Settings for a new world |
| `--keep-inventory` | Players keep their things when they die |
| `--difficulty peaceful\|easy\|normal\|hard` | Overrides the world's difficulty |
| `--max-players N` | Player limit (default 16) |
| `--motd TEXT` | The name players see in their server lists |
| `--allow-list` | Only let in players on the allow-list (and operators) |
| `--upnp` | Ask the router to forward the port, for servers at home |

Console commands:

- `list` (who's online), `players` (everyone the server remembers), `info <name>` (one player's position, health, inventory and armour) and `forget <name>` (they start afresh next time).
- `say <text>`, `kick <name>`, `time <day|noon|night|midnight|0.0-1.0>`, `password <pw|off>`.
- `ban <name|ip>`, `unban <ip>`, `bans` (bans are by IP address).
- `op <name>`, `deop <name>`, `ops`. Operators can use all of these in chat with a slash (`/kick Bob`); everyone else gets `/list` and `/help`.
- `pvp on|off`, `claims on|off`, and `team` (as in chat).
- `allowlist on|off|list|add <name>|remove <name>`. Turning it on adds whoever is online, so nobody is kicked out by surprise.
- `save`, `stop`, `help`.

The lists are plain text next to the server: `allow-list.txt`, `ops.txt` and `banned-ips.txt`. Names aren't accounts, so combine the allow-list with a password. Whoever hosts from the pause menu is always an operator there.

The world autosaves every 5 minutes and on `stop` (Ctrl+C loses anything since the last autosave). On a cloud server, allow TCP port 25565 in its firewall.

### How it works

- **The host runs the world:** mobs, TNT, time, weather and saving. Players send it their edits, movement and attacks, and it passes them on. A joining player gets the seed plus the edits, so only changes travel, never whole chunks. If the host leaves, everyone returns to the title screen.
- **Encryption.** Each connection opens with a key exchange (X25519), and from then on every message both ways is encrypted and sealed (ChaCha20-Poly1305), with keys made fresh for that connection: nobody in between can read it, change it or replay it. The host also has a lasting key of its own (`host_key.txt` beside the settings, or in a dedicated server's folder; its fingerprint is in the server's log), which takes part in every exchange, so only the real host can finish one; players' games remember it (see the server list). All of it is written out in `src/online/crypto.rs` and checked against the standards' own test vectors, so the game still needs nothing but the standard library.
- **Passwords** are checked with a challenge-response tied to that one connection's secret, so the password itself is never sent and a proof can't be passed on to the real server by someone in the middle.
- **The host doesn't take a player's word for anything.**
  - Block edits must be within reach, at a human rate, and follow the rules (no turning stone into Dimond ore, no breaking bedrock). Rejected edits are undone on the player's screen. Movement must make sense, and attacks, ignitions and fishing must be within reach and at a human pace. Chat is rate-limited, and nobody can call themselves "Server".
  - **The ledger.** Each player's inventory lives on their own machine, so it feels instant, but the host keeps a ledger of what every player really owns, built only from what it saw happen: drops it rolled, loot it sent, recipes it allowed, blocks placed, arrows shot, food eaten. Placing, crafting, shooting and the rest need the items in the ledger; every few seconds the counts are compared and the host's win. Tool wear, experience, enchantments, anvil repairs, dropped items and deaths all go through the host too.
  - Chests, furnaces, backpacks and the other containers live on the host; players see a copy, and every move in or out is checked against the container and the ledger. That goes for Trading Tables (each player can only touch their own side, and only the host swaps) and post (inboxes are take-only; parcels on their way are the host's, kept with the world).
  - Hits on other players are checked for range, rate, `/pvp` and teams, and edits and chests on someone else's land are refused.
  - Creative worlds skip the ledger, since everything is free there anyway.
  - Players who keep sending things a real game wouldn't are kicked.
- **Connection limits.** Messages are capped at 256 KB and read a bounded amount per frame; at most three connections per address; five wrong passwords lock an address out for ten minutes; silent connections are dropped after 30 seconds.
- **Dimensions.** Each player is in one dimension, and only hears about that one: its blocks, mobs, items and the players there (chat, the time and who's online go to everyone). The host keeps every dimension with someone in it running, even one it isn't in itself. Going through a portal, the host sends that world's edits afresh. `/tp` to a player in another dimension takes you there.
- **Saving.** The host remembers each player by name (inventory, armour, experience, health, hunger, statistics, which dimension they were in and where), even across server restarts. Use a password to keep strangers from borrowing a name.
- **Addresses** can be `IP`, `IP:port`, `[IPv6]:port` or a hostname like `play.example.com`.

## Modding

- **Data mods.** Drop a folder with a `mod.txt` into `mods/` to add blocks, items, tools, food, recipes, textures (pixel art, noise or PNG), ores, plants, mobs and simple effects; whole new biomes, structures (built from a block template) and dimensions with portals of their own; and translations. Servers send their mods to players automatically. See **[MODDING.md](MODDING.md)** and `example-mods/cheese` (and `example-mods/pirate` for a translation).
- **Code mods.** Mods can also include sandboxed [Rhai](https://rhai.rs) scripts that react to events (chat commands, blocks broken and placed, item use, joins, mob deaths, ticks) and call a game API. Scripts run on the machine that owns the world, dedicated servers included. See **[SCRIPTING.md](SCRIPTING.md)** and `example-mods/commands`.

The larger [parody example packs](example-mods/README.md) add optional nuclear machinery and nukes, a tiny local word-model assistant, four rocket tiers with five planet dimensions, and an isolated dusk adventure dimension. They can be installed together; each pack includes a guide to its survival progression and commands. Protocol 50 introduced per-dimension player gravity; current protocol 57 also synchronizes visible rocket flights, cockpit instruments and powered landings. No new dependencies or AI service are required.

The optional Dubiety pack also includes **Obesity**, a separately summoned companion that eats any offered item and swallows cows, pigs, sheep and chickens within five blocks. Its larger orange model has a broad mouth; it demands another meal every eight seconds. Craft its Beacon from a normal Companion Beacon and eight Wheat, then use `/dubiety obesity`.

## For developers

### Tests and CI

- `cargo test --release --locked` runs the tests; `cargo clippy --release --all-targets` should have no warnings.
- GitHub Actions runs the tests, Clippy and a minimal (`--no-default-features`) build on every push and pull request (`.github/workflows/ci.yml`), and builds releases from a version tag like `v0.2.0` or the **Run workflow** button (`.github/workflows/release.yml`).
- **Screenshots without playing.** `minceraft --screenshot out.png --mode <scene> [--frames N] [--time 0..1] [--yaw R] [--pitch R] [--pos x,y,z] [--seed N] [--gen 2] ...` renders a scene and saves a PNG. Scenes added from v0.5 on show a world as it's made now; older ones keep the oldest generator so they look as they always have (`--gen N` picks one either way, `--gen 0` being the oldest). There are dozens of scenes, from `title` and `inventory` to `village` (and `desertvillage`, `savannavillage`, `taigavillage`, `snowyvillage`), `bedtime`, `noticeboard`, `growingvillage`, `shopstall`, `newspaper`, `scorch` and `palegarden`, plus `portrait --mob <name>` for a close-up of any creature. The scenes and flags are in `src/shots.rs`.
- **Part 2 core review.** Scope, compatibility notes, review corrections and test results are in [docs/v0.6-part2-core/README.md](docs/v0.6-part2-core/README.md). This branch does not release Part 2; the two originals remain undecided.
- **Part 2 verification scenes.** `depth_dripstone`, `depth_lush`, `depth_dark`, `settled_village`, `settled_mansion`, `settled_spring`, and `climate_mod` use the newest generator by default. Install `example-mods/climate` as `mods/climate` for the last scene. To export surface maps, height maps, cave slices at 10/32/48, mod-biome maps and structure CSV overlays for twelve fixed seeds: `MINCERAFT_BIOME_MAPS=out/maps cargo test --release --locked biome_maps -- --ignored`. Raw maps are 512×512 bytes, sampled every 8 blocks, centred on the origin; the name files provide the palettes. A mod map's zero means base terrain; other values are registry biome index plus one.
- **Terrain verification scenes.** `terrain_mountains`, `terrain_meadow`, `terrain_river`, `terrain_beach`, `rockyshore`, and `biomeborder` show natural sites using the newest generator. Set `MINCERAFT_MAP_CHUNKS=512` on the map command for a 512×512-chunk region (1024×1024 samples); add `MINCERAFT_MAP_SURFACE_ONLY=1` to export only surface biomes and heights. `map_metadata.txt` records revision, dimensions, sampling and minimum coordinates.
- **Playtest bots.** `minceraft --playtest [--bots N] [--seconds S] [--seed N] [--port N]` hosts a world and connects bot players over real sockets (no window needed). They wander, build, chat, fill chests, glide, trade, fight, brew, dig and more, and then it checks that the host and every bot agree on every block, every inventory matches the host's ledger, and everyone saw and heard what they should. It exits non-zero on any mismatch; CI runs it with six bots for a minute.

### Code map

Each folder's `mod.rs` starts with a note on what's in it, and every file starts with a note on what it does.

| Where | What |
| --- | --- |
| `src/main.rs` | Window, input and the main loop |
| `src/game.rs` | Gameplay rules, spawning, explosions, scene assembly |
| `src/shots.rs` | The `--screenshot` harness and its scenes |
| `src/screens/` | Menus, settings, the inventory and creative tabs, crafting stations, logs and the HUD |
| `src/engine/` | Rendering (`render`, `mesher`, `texture`, `light`, `tint`, `lod`), chunk storage (`palette`), noise, sound, UI widgets, controls (`keybinds`, `pad`), settings, saving (`save`, `regions`, `backups`, and the compressor, `squash`), meshing on worker threads (`meshpool`), languages (`lang`), the narrator, the data folder, updates, UPnP and accessibility |
| `src/land/` | The world and its generator (`world`; the land's shape and biomes in new worlds, `shape`), the dimensions (`dims`), mods' biomes, structures and dimensions (`modgen`), and the realm each lives in (`realms`, saved by `realm_save`), structures, weather, seasons, skies, liquids, fire, falling blocks, trees and ground cover, random ticks (`copper`), the Scorchlands, their biomes (`wilds`), fortresses, bastions and Woodland Mansions, hot springs and the chill (`springs`), the Hollow, the Deep Dark, Trial Chambers, treasure and archaeology |
| `src/blocks/` | The block and item registry (`block`), building blocks, doors, decor, containers, chests, beds, backpacks, Personal Chests, Hollow Boxes, Zappy wiring and contraptions, the newer Zappy parts (`zapparts`), hoppers, crafting, enchanting, anvils, smithing, trims, brewing, beacons, banners, books, fireworks, music, cauldrons, composters, heads and lightning rods (`homecraft`), the blocks for playing together (`together`) and the Trading Table (`trading`), village blocks (`villageblocks`) and Shop Stalls (`stalls`) |
| `src/creatures/` | Mob physics, models and AI (`entity`), pathfinding, animals, wildlife, critters, the Scorchlands' beasts, bees, horses (and Donkeys and Mules), leads, Floaties, golems (Snow Golems too), villagers (and their trade levels), village life (beds, workstations, the day and babies: `village_life`), villages' new houses and Clankers (`village_growth`), Notice Boards and bounties (`bounties`), the village paper (`paper`), what villages think of you (`reputation`), raids, Sniffers, the Creaking, name tags and players' own skins (`skins`) |
| `src/survival/` | The player, inventory, items on the ground, hunger and breath, cooking, experience, combat, tools, gadgets, navigation, gliders, vehicles, farming, fishing, advancements, statistics, game modes, world rules, waypoints and other handy things (`qol`) |
| `src/online/` | The network protocol and its encryption (`crypto.rs`), hosting and joining, the server list and LAN discovery (`servers.rs`), the dedicated server, admin, the host's ledger, remembered players, teams and PvP (`teams`), land claims (`claims`), the post (`post`), cheats, mods, scripting and mods' minigames (`minigames`), and the playtest bots |
| `build.rs`, `assets/minceraft.ico` | The Windows exe's icon (drawn from the game's textures by `--export-icon`) |

Not affiliated with any block-game company.

Natural revision 10 verification scenes: `rolling_hills`, `offshore_island`, `large_mushroom`, `beach_ecology`, and `sea_wildlife`. They use actual generated sites and the normal marine spawning pass.


Player poses now blend between walking, crouching, swimming/treading water, creative flight, gliding, jumping and landing. Articulated knees and elbows are shared by built-in skins, custom skins, armour and trims; Glider wings spread with the glide transition. Successful food and drink uses show the consumed item moving toward the mouth in first and third person, with crumbs for food. These are cosmetic follow-through animations: consumption timing and collisions retain their existing rules. Creative-flight flags are host-filtered, swimming follows each realm’s water, and short meal gestures synchronize without changing inventories. Protocol 57 is required.


Third-person players now carry their selected item and offhand equipment on the shared skeleton. Tools swing through mining/placement, shields rise in either hand, spears thrust, and hurt players recoil. Hold use with a bow to draw it; release to fire. Ladder, rope, cave-vine and scaffold climbing alternate hands and knees, and hold still while hanging. Mobs blend swimming pitch/bob, flying bank and sitting transitions without changing collisions or AI.

Right-click a Half Baked centrifuge/reactor/cooling tower or an Ad Absurdum refinery/oxygen generator/solar panel/rocket bench/launch pad to open native controls (sneak to place against it). Panels show inventory ingredient counts and reactor/solar progress, run the existing scripts, and close when the machine is removed, you leave its realm, or you move out of reach. LAN actions are indexed and host-checked for reach, life state and machine type; scripts retain payment, cooldown and cooling rules. The machine-panel advancement is cosmetic. Protocol 57 adds equipment/combat state and machine requests; save format 16 and generator revision 12 remain unchanged.

Native verification scenes: `pose_tool`, `pose_shield`, `pose_bow`, `pose_spear`, `pose_climb`, `pose_hang`, `pose_hurt`, `mob_movement`, `machine_reactor` and `machine_rocket`. Install the corresponding example packs for machine scenes.
