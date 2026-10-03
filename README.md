# MINCERAFT

A native, compiled block-building parody written in Rust, with no browser, no web view and no JavaScript. It opens a real OpenGL window through [macroquad](https://github.com/not-fl3/macroquad)/miniquad, and the voxel renderer talks to the GPU directly with its own shaders and vertex buffers.

The game ships with **no image or audio files**. At startup it builds the whole texture atlas (grass, ores, mobs, tools, hearts, the logo) from noise and tiny ASCII sprites, and synthesises every sound effect and the music.

Its only dependencies are macroquad (window, input, audio), [Rhai](https://rhai.rs), the sandboxed scripting language used for code mods, and [gilrs](https://gitlab.com/gilrs-project/gilrs) for game controllers.

## Download and play

Ready-made builds are on the [Releases page](https://github.com/BoatChinasAreFake/crispy-octo-bassoon/releases). Download the zip for your computer, unzip it anywhere, and run the game inside it: on **Windows**, double-click `minceraft.exe` (if Windows warns about an unrecognised app, click **More info**, then **Run anyway**). Worlds, settings and mods are kept in your own data folder, so a new version can be unzipped anywhere and picks up where you left off:

- **Windows:** `%APPDATA%\Minceraft` (paste that into File Explorer's address bar)
- **macOS:** `~/Library/Application Support/Minceraft`
- **Linux:** `~/.local/share/minceraft`

The first time a version with this runs, it copies any `saves`, `settings.txt` and `mods` it finds beside it into that folder (so unzipping over an older version brings its worlds along; the originals stay where they were). The world list and the Mods screen show the folder. To keep everything beside the game instead (on a USB stick, say), put an empty file named `portable.txt` next to it. Release builds check GitHub once at startup for a newer version and offer a download button on the title screen if there is one (using your system's `curl`; turn it off with **Update Check** in Options). If the game ever crashes it writes `crash.txt` to the same folder: please include it when you report the bug. The title screen shows which version you're running.

New releases are built by GitHub Actions (`.github/workflows/release.yml`) whenever a version tag like `v0.2.0` is pushed, or from the Actions tab with **Run workflow**.

## Build and run

To build it yourself, you need a Rust toolchain ([rustup.rs](https://rustup.rs)).

```sh
cargo run --release
```

Linux also needs the X11/GL, ALSA and udev development libraries, for example on Debian/Ubuntu:
`sudo apt install libx11-dev libxi-dev libgl1-mesa-dev libasound2-dev libudev-dev`.
If you can't install ALSA or udev, `cargo run --release --no-default-features` builds a version without sound or controller support (add `--features sound` or `--features gamepad` to get one back).
Windows builds with no extra setup. macOS should work but hasn't been tested.

Run the tests with `cargo test --release`. GitHub Actions runs the tests, Clippy and a minimal (`--no-default-features`) build on every push and pull request (`.github/workflows/ci.yml`).

## Features

- **Infinite procedural terrain.** Seeded Perlin noise produces oceans, beaches, plains, forests, deserts, snowy biomes, swamps, jungles, badlands and taigas (see Biomes), ridged mountains, spaghetti caves, big caverns (the deepest ones flooded), ravines that split the surface open, Glowshrooms and Pointy Rocks lighting and littering the caves, ore veins (gold included, for all the good it'll do you), trees (oak, spruce and jungle), flowers, tall grass, pumpkins, desert Pokey Plants and frozen seas. Chunks generate, and light themselves, on background threads; the main thread only joins light up at chunk borders.
- **Biomes.**
  - **Swamps** sink to just around sea level: pools with lily pads, mud, and wide oaks trailing leaves. Bloops like it there at night.
  - **Jungles** grow very tall trees with wide crowns, thick undergrowth and **Melons** (break one for **Melon Slices**). **Squawkers** live there.
  - **Badlands** rise into steep hills of banded **Terracotta** under **Red Sand**, with dead bushes, cacti, more gold higher up, Rattlers at night, and no rain.
  - **Taigas** are cool spruce forests full of Woofers. Snowy places grow spruce too.
  - **Spruce** and **Jungle** logs make planks, burn and smelt like oak; saplings grow into whatever tree suits the biome they're planted in.
- **Custom voxel renderer.** It culls hidden faces, adds per-vertex ambient occlusion and smoothed sky lighting, and uses frustum culling, translucent sorted water, cutout leaves and glass, fog, and mipmaps. **Greedy meshing** joins neighbouring faces with the same texture and light into one big rectangle (the shader repeats the texture across it). Faces only join along a direction their shading doesn't change in, so it looks the same as before at a fraction of the triangles, and the render distance goes up to 32 chunks.
- **Compact world storage.** Like Minecraft, each 16-block-tall section of a chunk stores a small palette of the blocks it contains plus packed indices (0 bits per block for all-air or all-stone sections, 4–8 bits for mixed ones, direct ids past 256 kinds). Typical terrain averages under 2 bits per block. F3 shows the live figure.
- **Graphics options.** **Leaves: Waving** sways the canopy in the wind, **Water: Shiny** reflects the sky (more at a glancing angle) with the sun glinting off ripples, and **Lighting: Smooth** blends light across faces with shading in the corners (Flat lights each face evenly, and is a bit faster). **Clouds: Fancy** makes them thick blocks with shaded sides (**Fast** is a flat layer). **UI Size** (Small, Normal, Large, Huge) scales the menus and HUD. **Brightness** runs from Moody (dark caves, like Minecraft's lowest setting) to Bright; the default lifts dim places so caves fade out gradually rather than going black. All in Options.
- **Real light.** Every block has a sky light and a block light level from 0 to 15, spread Minecraft style: sky light pours straight down and fades one level per step under an overhang, while torches (14), lava and Glowrock (15) and lamps light their surroundings, fading a level per block. Solid blocks stop light, leaves and water dim it, and it all updates as you dig and build, across chunk borders. Caves are properly dark, torchlight is warm, and a held torch lights your way.
- **Day/night cycle** (20 minutes), with a sun, moon, stars, sunrise/sunset glow and scrolling clouds.
- **Physics.** AABB collision, gravity, sprint-jumping, sneaking (it stops you walking off ledges), swimming and fall damage.
- **Survival mode.** Health, hunger, experience, mining times that depend on your tool, pickaxe tiers that gate ore drops, tools that wear out (and an anvil to fix them), 142 crafting recipes, food, cooking, chests, armour, beds, a bow, farming, fishing, death (which drops your things) and respawn.
- **Hunger.** A food bar of ten drumsticks, with Minecraft's rules. Sprinting, jumping, swimming, fighting, digging and getting hurt use up hidden saturation first, then food. With a full bar you heal quickly; at 18 points or more you heal slowly. At 6 points or less you're too hungry to sprint, and on an empty bar you starve down to half a heart (dramatic, but not fatal). Food fills the bar: raw food barely helps, cooked food keeps you full much longer. You can't eat when you're full, except legendary snacks (Suspiciously Golden Oinkchop, Big Bob), which also heal you outright. Cake is 7 drumsticks in one bite.
- **Durability.** Pickaxes, swords, the hoe, the bow, the Fishing Stick and armour wear out, with Minecraft's numbers: 59 uses for wood, 131 for stone, 250 for iron and 1561 for Dimond. A bar under the item shows how worn it is, and the tooltip counts the uses left. Breaking a block uses a tool once (swords twice, and blocks that break instantly not at all). Hitting a mob uses a sword once and anything else twice. Every bow shot, catch and tilled block counts too. Each piece of armour takes a quarter of every hit it softens. Wear stays with the item in chests, on the ground and in saves.
- **Experience.** Green orbs from defeated mobs (5 for monsters, 1 to 3 for farm animals), coal, iron, gold and Dimond ore, float toward the nearest player. Taking things out of a furnace and catching fish pay experience too. Levels follow Minecraft's curve, and the bar and your level sit above the hotbar. Dying drops 7 points per level (at most 100) as orbs and loses the rest, unless the world keeps inventories.
- **Structures.** The generator builds things, the same in every copy of a world: mossy **Dungeons** deep underground around a **Monster Cage (Still Occupied)** that keeps spawning Groaners, Rattlers or Webbers while you're nearby (break it to stop it), **Ruined Towers** crumbling toward the top, the odd lone **Hut** (definitely not a village; rare now) with a bed, a furnace and a chest, desert **Wishing Wells** with something at the bottom, and **Villages** in every world (on plains, deserts, taigas and snowy land, usually a few hundred blocks apart): a well on a cobbled square with lamp posts, gravel paths, three to six houses (each with a Hmmer), and farms of watered crops, guarded by a **Clanker**. Their chests are filled the first time the chunk loads, with loot to match: dungeons have the good stuff (Dimonds, gold, iron, used and sometimes enchanted tools), huts have bread and seeds. F3 shows the nearest one.
- **Weather.** Clear skies turn to rain (snow in cold places) and now and then a thunderstorm. It stops at the first block in its way, so caves and roofs stay dry. Rain dims the day, greys the sky, waters crops like a nearby pond does and makes fish bite faster. Thunderstorms are dark enough for monsters to come out, and lightning strikes near players: it hurts, and it sets off TNT. Sleeping through a storm clears it. Joined players see the same weather.
- **Enchanting.** An **Enchanting Table (Bookshelf-Powered Guesswork)** is a bookshelf, 2 Dimonds and 4 cobblestone. Right-click it, put in a tool, sword or piece of armour and some gold ingots, and pick one of three offers: they cost 1, 2 or 3 levels and as many gold ingots, and need your level to be at least the number shown. Bookshelves in a ring two blocks out (on the table's level and the one above; 15 is the most that count) make the offers stronger, up to level 30. Only the first enchantment of each offer is shown. There are five:
  - **Efficiency** (pickaxes, up to V): digs faster.
  - **Sharpness** (swords, up to V): 1.25 more damage per level.
  - **Protection** (armour, up to IV): another 4% off each hit per level, per piece (still at most 80% in all).
  - **Unbreaking** (anything that wears out, up to III): lasts twice as long at I, four times at III.
  - **Fortune** (pickaxes, up to III): more coal, Dimonds and other ores that drop something else.

  Enchanted things shimmer, and their tooltip lists what they have. The anvil keeps the left item's enchantments when it repairs.
- **Anvils.** An **Anvil (Drops Ominously)** costs 10 iron (a real one is 31; this one's a bargain). Right-click it, put a worn tool or piece of armour on the left and what it's made of on the right: planks, cobblestone, iron, Dimond, gold, wool, or string for bows and Fishing Sticks. Each unit mends a quarter of its durability and costs a level. Or put two of the same thing together to merge them, which costs 3 levels and adds a 12% bonus. Every use has a 12% chance of knocking the anvil down a stage (Chipped, then Damaged, then gone).
- **World settings.** **World Settings** in the pause menu changes a world's rules at any time (for the world's owner; joined players can look but not touch):
  - **Keep Inventory**: keep your things (and experience) when you die.
  - **Difficulty**:
    - Peaceful: no monsters and no hunger.
    - Easy: monsters hit about half as hard, and starving stops at five hearts.
    - Normal: as described above.
    - Hard: monsters hit 50% harder, and starving can kill.
  - **Daylight Cycle**: turn it off and the sun stays where it is.
  - **Weather Cycle**: turn it off to keep the current weather, and a button next to it changes the weather now.
- **Flowing water and lava.** Water spreads 7 blocks from a source and lava 3, falling first and spreading sideways from whatever it can't fall through. Two water sources side by side make a third, and taking a source away drains the rest. Nothing moves until something changes nearby, so the sea sits still until you dig next to it. Water meeting a lava source makes **Obsidian (Very Committed)**, meeting flowing lava makes cobblestone, and lava poured on water turns it to stone. Water washes away plants and torches; lava burns them, glows in the dark, and sets you on fire (jump in water to put it out). Lava lakes lie deep underground. A **Bucket (Empty, Optimistic)** (3 iron) scoops up a source and pours it somewhere else; water boils away in the Scorchlands.
- **Breeding, taming and shears.** Feed an animal its favourite food (Oinkers carrots or potatoes, Fluffers and Mooers wheat, Clucksters seeds) and it falls in love for half a minute. Two in love make a baby, which grows up in four minutes. Animals follow anyone holding their food. **Shears (For Fluffers, Not Haircuts)** (2 iron) take 1–3 wool off a Fluffer; it grows back once the Fluffer has eaten grass. **Woofers** live in forests and snowy places: feed one bones and there's a one-in-three chance it becomes yours. A tamed Woofer wears a collar, follows you (teleporting when left behind), goes after whatever you hit or whatever hits you, and sits or stands when you right-click it. Feed it meat to breed more.
- **Zappy Dust (legally distinct redstone, simplified).** **Zappy Ore (Tingly)** deep underground drops Zappy Dust. Power comes from a **Lever (Pull It)**, a **Button (Press It)** (on for a second), a **Pressure Plate (Step On It)** (on while anything stands on it) and a **Block of Zappy Dust (Always On)**. Dust laid on the floor carries power up to 15 blocks, along the floor and up or down a step. Anything next to a switch or powered dust is powered: **Zappy Lamps** light up, doors open (and close again), and TNT goes off.
- **Hmmers and trading.** Every hut gets a **Hmmer** (a legally distinct villager) with a job: Farmer, Librarian, Smith or Fisher. Right-click one to see its five trades. Gold ingots are the money (finally, a use for them): sell wheat, carrots, feathers, coal, iron, cod or string, and buy bread, cake, books, bookshelves, enchanted books, iron tools and armour (the tools come enchanted), fishing gear and bait. Each trade can be made six times before the Hmmer runs out; they restock every day. Hmmers remember their **regulars**: after five trades with the same one you get 10% off, after fifteen a fifth off, after thirty nearly a third.
- **Enchanted books.** A **Book (Mostly Wheat)** is 3 wheat and a string. Enchant it at the table like a tool (any enchantment fits a book) to get an **Enchanted Book (Spoilers Inside)**, or buy one from a Librarian. At the anvil, put a tool on the left and a book on the right to add its enchantments, or two books together to merge them. Combining two enchanted tools at the anvil merges their enchantments too.
- **Combat.** Weapons take a moment to be ready after a swing (swords 0.6 seconds, pickaxes 0.8, hands 0.3), shown by a bar under the crosshair. Hitting early does less damage, and only a fully charged blow can be a critical hit or a sprinting knockback. A fully charged sword blow also sweeps the mobs right next to the target. Hold right-click with a **Shield (Door You Can Carry)** (6 planks and an iron) to block hits from in front (mobs, arrows, half an explosion) while you walk slowly; the shield wears instead. Every armour point takes 2.5% off knockback.
- **The Scorchlands.** A second, hotter world under a bedrock sky, with Scorchrock, Embersand, gold ore, a lava sea, a dim red glow and no weather. Build an obsidian frame (at least 4 wide and 5 tall, corners optional), light it with a **Sparker (Hot Hands in a Can)** (iron and coal), and stand in the portal for two seconds. Every block there is eight here, and a portal appears at the other end if there isn't one nearby already (the two stay linked). **Grumblers** live there: leave them be, because hitting one brings the whole crowd (they drop Groaner Goo and sometimes gold). Compasses spin in the Scorchlands.
- **Boats, minecarts and rails.** A **Boat (Mostly Waterproof)** (5 planks) floats: right-click to get in, W and S row, A and D steer, sneak to get out. A **Minecart (Wheeled Bucket)** (5 iron) runs on **Rails (Choo Choo)** (6 iron and a stick make 16), which join up and curve round corners by themselves; W pushes the cart the way you're looking. **Powered Rails (Zoom)** (gold, a stick and Zappy Dust) speed carts up when powered and brake them when not. A **Detector Rail (Tattletale)** powers what's next to it while a cart is on it. A **Minecart with Chest (Freight)** carries 27 stacks and a **Minecart with Hopper (Vacuum)** 5 (right-click to look inside): hopper carts pick up what they roll over and take from containers above the track, a hopper under the track unloads a cart, and one pointing at a cart loads it. Hit a vehicle three times to pick it up (spilling its cargo). Joined players ride them too.
- **Saplings and leaf decay.** Leaves drop a **Sapling (Tree, Eventually)** now and then (and an **Apple** once in a while). Plant it on grass or dirt, give it light and room, and in a few minutes it's a tree; Bone Dust speeds it up. Chop a tree down and its leaves wither away over the next few seconds instead of hanging in the air.
- **Fences, gates, ladders, trapdoors, panes and dyes.** A **Fence (Keeps Honest Animals In)** joins up with fences, gates and solid blocks beside it, and is too tall to jump (animals stay in; a Galloper clears it). A **Fence Gate (Swings Both Ways)** and a **Trapdoor (Floor Door)** open and shut with a right-click. A **Ladder (Up, Mostly)** goes on walls: walk into it to climb, sneak to hold on. A **Glass Pane (Window, Budget)** joins up like a fence. **Dyes** in eight colours come from flowers, coal, Bone Dust, pumpkins, gold (finally!), Pokey Plants and tropical fish, and turn wool and glass into **Coloured Wool** and **Stained Glass**.
- **Fire.** A **Sparker** now lights any block you point at, lava sets things alight as it flows, and lightning starts fires. Fire spreads through wood, wool, leaves and hay, burns them away, sets off TNT and goes out when there's nothing left (on Scorchrock it burns forever). Rain puts it out. Walking through it sets you alight; punch it out.
- **Brewing and potions.** Fill a **Glass Bottle (Empty, Hopeful)** at water, then brew it at a **Brewing Stand (Chemistry, Loosely)** (three cobblestone and a Grumbler Tusk from the Scorchlands): Glowshroom for **Healing**, Zappy Dust for **Speed**, **Ember Shroom** (Scorchlands) for **Fire Resistance**, carrot for **Night Vision**, feather for **Leaping**. Brew any potion again with Hisspowder for a **splash** version you throw. Effects last three minutes and show on screen.
- **More Zappy parts.** A **Zappy Torch (Contrarian)** is lit unless the block it's on is powered. A **Repeater (Says It Again)** passes power one way, a moment later, back at full strength. A **Piston (Pushy)** shoves up to 12 blocks, and a **Sticky Piston (Clingy)** pulls one back too. A **Dispenser (Spits Things)** fires arrows, pours buckets, lights TNT, bursts splash potions, fertilises with Bone Dust or just spits things out, each time it's powered. A **Comparator (Counts Your Stuff)** powers what's in front of it while the container behind it (a chest, furnace, hopper or loaded cart) has anything in it; right-click to make it wait until it's half full.
- **Hoppers.** A **Hopper (Funnel With Ambition)** (five iron and a chest) takes items from the container above it or off the floor on top of it and passes them into whatever its spout points at: chests, furnaces (input from above, fuel from the side), brewing stands and other hoppers. Power switches it off.
- **Gallopers.** Legally distinct horses wander the plains. Keep trying to ride one (Apples and wheat help) and it comes round. Put a **Saddle (Some Assembly Required)** on it and ride: it's fast, jumps high and goes where you look. Tamed Gallopers breed on Apples and are saved with the world.
- **The Hollow.** The endgame. Make **Staring Eyes** (a Stare Pearl and an Ember Shroom) and throw them to find a **Crypt**, a buried room with a ring of twelve **Eye Frames**. Fill every frame and the pit opens into a portal to a floating island in a starry void, circled by the **Hollow Wyrm**. It heals from the **Wyrm Crystals** on the obsidian pillars, so break them first (they explode). Beat it for a pile of experience, the **Wyrm Egg (Trophy, Allegedly)**, and a portal home. A boss bar shows its health.
- **Beacons.** The Wyrm Egg, five glass and three obsidian make a **Beacon (Wyrm-Powered Lighthouse)**. Put it under open sky on an obsidian pyramid (3×3, then 5×5, then 7×7) and it shines a beam into the sky and gives everyone within 16, 28 or 40 blocks an effect: Speed, Leaping, Night Vision, Fire Resistance or a little Healing now and then. Right-click to choose.
- **Name Tags and skins.** A **Name Tag (Hello, My Name Is)** (string and a book) names a mob: the name floats above it, and it's never cleared away. **Options > Skin** picks one of six looks (Stove, Alexa, Kettle, Toaster, Blender, Fridge) that everyone else sees too.
- **Signs, item frames, compass and map.** A **Sign (Words Go Here)** holds four lines of text that float above it, readable from a distance. An **Item Frame (Look What I Have)** (8 sticks and wool) hangs on a wall and shows off whatever you put in it; hit it to take the item out. The **Compass (Points Home, Mostly)** (4 iron and Zappy Dust) points to your spawn and shows the distance. Hold a **Map (You Are Here)** (8 wheat and a compass) to see the land around you from above, with you on it.
- **Death drops.** Dying drops your whole inventory, armour included, where you fell. The death screen tells you where, and it all waits five minutes. Worlds created with **Keep Inventory** on (a toggle on the Create World screen, or `--keep-inventory` for servers) let you keep everything instead.
- **Creative mode.** Flight, instant breaking, infinite blocks, pick-block and a full item palette (scroll it with the mouse wheel).
- **Mobs** (legally distinct):
  - **Oinker**: wanders around, runs when hit, drops Raw Oinkchop.
  - **Hisser**: sneaks up, flashes, swells and explodes. Drops Hisspowder.
  - **Groaner**: chases you at night, bites, and burns in sunlight. Drops Groaner Goo.
  - **Fluffer**: a woolly wanderer that baas. Drops Wool and Raw Baa-con.
  - **Starer**: very tall, very dark, keeps monster hours. Harmless until you look it in the eye or hit it; then it vibrates with rage, teleports after you and hits hard. Hates water. Drops the Stare Pearl.
  - **Cluckster**: a nervous little bird that flutters down instead of falling. Drops Feathers and Raw Cluckets.
  - **Mooer**: large, calm, black and white. Drops Raw Moo-steak.
  - **Rattler**: a bony archer that comes out at night, keeps its distance and shoots Pointy Sticks (arcing them for range). Burns in sunlight. Drops Bones and spare Pointy Sticks.
  - **Webber**: eight legs, eight eyes, walks straight up walls. Hunts at night; in daylight it leaves you alone unless you hit it. Drops String.
  - **Bloop**: a hopping cube in three sizes. Big and medium ones hurt on contact, and killing one splits it into two to four smaller Bloops. Only the smallest drop Groaner Goo.
  - **Squawker**: a loud, bright parrot of the jungle that flutters down like a Cluckster. Likes seeds. Drops Feathers.
  - **Clanker**: a village's iron guard. It stays near the square, goes after any monster within 16 blocks and punches it into the sky, and fights back if you hit it. Hard to knock about. Drops iron.
- **Bow (Twangy).** Craft it from sticks and string, then right-click to fire Pointy Sticks (crafted from a stick, a feather and cobblestone, or a bone and a feather). Arrows arc, stick in walls, and work in multiplayer.
- **More parody blocks and items:**
  - **Bed (One Block, Budget Cuts)**: right-click at night to skip to morning and set your spawn. It refuses if monsters are nearby.
  - **Cake (Not a Lie)**: place it, then right-click to eat the whole thing in one bite.
  - **Stare Pearl**: right-click to teleport to wherever you're looking. It stings a bit.
  - **Gold (Shiny, Useless)**: too soft for tools. Crafting a "gold pickaxe" gets you a wooden one. It does make a **Suspiciously Golden Oinkchop** that fully heals you.
  - **Pumpkin** and **Jack o'Lantern** (a light source with a face on every side), **Pokey Plant** (don't hug it), **Ice** (fast underfoot, melts when broken), **Bouncy Goo Block** (sneak to land softly), **Sponge** (soaks up nearby water when placed) and **Wool**.
  - Sneak to place blocks against beds and cakes instead of using them.
- **Farming (deliberately over-engineered).** Till grass or dirt with a **Hoe**, plant **Wheat Seeds** (from tall grass), **Carrots** or **Potatoes**, and watch four growth stages. Every tilled block has its own soil, and growth multiplies together:
  - **water** within 4 blocks (hydrated farmland turns dark);
  - **light**: sunshine or a torch or lantern nearby (crops don't grow in the dark);
  - **nutrients**: wheat eats nitrogen, carrots phosphorus, potatoes potassium. Top them up with **Compost**, **Bone Dust** and **Wood Ash**;
  - **crop rotation**: a bonus for planting something different, "soil fatigue" for the same crop three times running;
  - **company**: crops grow 20% faster with a player nearby.

  Meanwhile weeds sprout on bare farmland and steal nutrients, Clucksters peck at seedlings unless a **Scarecrow** is nearby, jumping on farmland tramples it, and dry, unused farmland turns back into dirt. A **Soil Probe** explains all of it in one long sentence. Wheat makes **Bread** and **Hay Bales** (which soften falls).
- **Fishing (also over-engineered).** Cast a **Fishing Stick** and wait. Fish nibble first (reel in then and you scare them off), then really bite, and you get a moment to reel in. Big fish start a tug-of-war: hold right-click to reel, but ease off before the line tension snaps it. What you catch depends on the biome, the time of day (dawn and dusk are best), the water's size and depth (puddles give boots), bait (**Wiggly Worms**, dug up from dirt) and your **Angler level**. Catches include cod, salmon, tropical fish, pufferfish (don't eat it), junk, treasure, and one legendary fish. The **Fishing Log** in the pause menu keeps count and records the biggest.
- **Items on the ground.** Broken blocks, mob loot, explosions and broken chests leave their items on the ground, where they bob, slide, merge with matching items nearby, float in water and vanish after five minutes. Walk into them to pick them up (if there's room). **Q** throws one of the held item, **Ctrl+Q** the whole stack, and clicking outside the inventory screen throws whatever's on the cursor. When your inventory is full, new things land at your feet instead of disappearing. Items on the ground are saved with the world.
- **Armour.** Four slots (helmet, chestplate, leggings, boots) in four tiers: **Woolly** (from wool; the Woolly Socks are for sandals), **Iron** ("Bucket With Ambition"), **Golden** (soft, shiny, why) and **Dimond** ("Maximum Flex"). The recipes use Minecraft's amounts: 5 of the material for a helmet, 8 for a chestplate, 7 for leggings and 4 for boots. Right-click to put a piece on, shift-click it in the inventory, or drop it into its slot beside the inventory. Every armour point takes 4% off damage from mobs, arrows and explosions, up to 80% (a full Dimond set is 20 points). Falling, cactus hugs and poisonous snacks still hurt as usual. Worn armour shows on your character, to other players too, and there's an armour bar above your hearts.
- **Slabs, stairs and doors.** Slabs and stairs come in Planks, Cobblestun and Stone Brick. Three blocks make 6 slabs and six make 4 stairs. Slabs go on top when you aim at the upper half of something, and a slab placed on a matching slab makes the full block. Stairs face away from you. You walk up slabs and stairs without jumping, and only their real shape gets in your way or gets hit. **Doors** (6 planks make 3) are two blocks tall: right-click to open or close them (sneak to place against one), and breaking either half takes the whole door. Wooden ones burn in furnaces.
- **Chests and furnaces.** A **Chest (Latches on Every Side)** (8 planks) holds 27 stacks. A **Furnace** (8 cobblestone) has an input, a fuel slot and a take-only output. It cooks one item every 8 seconds while it has fuel, and it glows while lit. It turns raw meat and fish into cooked versions, potatoes into Baked Potatoes, sand into glass, cobblestone into stone, and logs into (legally distinct) coal. Coal, wood, sticks, hay and wooden tools all burn. Cooking a Pufferfish only halves the damage, and a Cooked Boot is still a boot. Right-click to open either one (sneak to place blocks against it), and shift-click to move whole stacks. Furnaces keep cooking while you're away, as long as their chunk is loaded. Breaking either one spills what was inside onto the ground. Everything is saved with the world.
- **More blocks:** Sandstone (under deserts), Stone Bricks, Mossy Cobblestun, Hay Bale, Bookshelf, Lantern, Mushroom (in forests; two make a Suspicious Stew), Scarecrow and Weeds.
- **Advancements.** 116 of them, each with a toast and a fanfare ("Getting Wood", "DIMONDS!", "Don't Blink", "The Cake Is Not a Lie"...). They're saved per world, and the pause menu lists them.
- **TNT.** Light it with a torch (or bare hands), and it chain-reacts.
- **Sound.** Synthesised effects for mining, placing and footsteps (different for stone, wood, grass, sand and glass), plus hurt sounds, oinks, baas, clucks, moos, rattles, skittering, bloops, bow twangs, groans, Hisser hisses, Starer warps, boings, an advancement fanfare, explosions, eating, splashes, item pickups, crafting and menu clicks. Sounds get quieter with distance. Music drifts in now and then, picked to suit the moment: a calm pentatonic tune or rolling arpeggios by day, a slow minor piece at night, and low, sparse pads in caves and the other dimensions (four pieces, each played on a piano-like instrument with a little hall reverb). Rain only falls where it can reach (not in caves or under roofs) and has its own patter (muffled under a roof, silent in snow and deserts), and deep in dark caves something rumbles now and then. Volume and music are in Options. Run `minceraft --export-sounds <dir>` to write every sound out as a WAV.
- **Accessibility.** **Subtitles** (Options) caption what you hear in the bottom corner, with an arrow toward where it came from ("< Groaner groans"). **Colour-blind** mode runs the world through a daltonising filter, so red and green differences (Zappy Dust on or off, ripe crops, coloured wool) show up as differences in brightness and blue.
- **Multiplayer over LAN or the internet.** Open any world from the pause menu (the game can ask your router to forward the port by itself), or run a headless dedicated server. It syncs blocks, player movement, mobs, TNT, explosions, damage, loot, sounds, time of day, weather and chat, and remembers every player's things between visits. Servers can require a password, keep an allow-list, and give operators commands in chat, and a public server checks that players' block edits are within reach and at a human rate. Everything uses the Rust standard library, with no accounts and no central server.
- **Mods.** Drop a folder with a `mod.txt` into `mods/` to add blocks, items, tools, food, recipes, textures (pixel art, noise or PNG), ores, plants and simple effects (bouncy or fast blocks, items that heal, launch, explode, give things or spawn mobs). Servers send their mods to players automatically. See **[MODDING.md](MODDING.md)** and `example-mods/cheese`.
- **Code mods (scripting).** Mods can also include sandboxed [Rhai](https://rhai.rs) scripts, which react to events (chat commands, breaking and placing blocks, item use, joins, mob deaths, ticks) and call a game API (blocks, items, health, teleport, explosions, mobs, time, messages). Scripts run on the machine that owns the world, including dedicated servers. See **[SCRIPTING.md](SCRIPTING.md)** and `example-mods/commands`.
- **Multiple worlds.** **Singleplayer** on the title screen opens a world list with each world's name, mode, seed, when it was last played, and size. You can play, create (with a name, game mode and optional seed; any text works as a seed), rename or delete worlds. Each world is its own folder, `saves/<world>/`. Only your edits are stored, and the terrain regenerates from the seed. A save from before world slots (`saves/world.mncr`) is moved in automatically as "My World".
- **World backups.** Each time you open a world, a copy of it as it was goes into `backups/<world>/` (in the data folder), and the last 5 are kept. **Backups** on the world list shows them; **Restore as a New World** makes a separate world from one (named like "My World (backup 2026-09-30 14:05)"), so restoring never overwrites anything. Deleting a world deletes its backups too.
- **Region files.** Everything placed in the world (block edits, what's in chests, furnaces and the like, sign text, item frames, farm soil) lives in region files (`saves/<world>/world.regions/`, 32×32 chunks each). They're read on a background thread as someone comes near and written out and forgotten when everyone leaves, so a huge, well-travelled world doesn't need to fit in memory. Saving writes the regions in memory. Worlds from before region files move everything out on the first save. Each file remembers the mods it was written with, so mod blocks and items stay right even in regions you haven't visited in a while. (Items lying on the ground stay in `world.mncr`; they vanish after five minutes anyway.)
- First-person hand and held item (items have a pixel of thickness like Minecraft's, and tools are gripped by the handle), third-person view, block-breaking cracks, particles, screen shake, a panoramic title screen with splash texts, and options for render distance, FOV, sensitivity and fullscreen.
- **Rebindable controls.** **Options > Controls** lists every action with two slots: click one and press the key or mouse button you want (Esc leaves it empty). **Reset to Defaults** puts them back, and How to Play shows your keys.
- **Game controllers.** Plug in a controller any time (Xbox, PlayStation and most others work):

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
- **Game modes.** Survival, Creative and **Spectator** (fly through everything, touch nothing; mobs and other players can't see you). A world has a starting mode, and each player can be switched with `/gamemode`. **Hardcore** is a Create World choice: one life on Hard, no keep-inventory and no cheats; die and you can only spectate.
- **Commands.** In single player (and for operators on a server): `/tp [player] <x y z | player>` (with `~` for "here"), `/give [player] <item> [count]` (items by key or by name, like `/give dimond pickaxe`), `/gamemode <survival|creative|spectator> [player]`, `/weather <clear|rain|thunder>`, `/seed`, `/summon <mob> [x y z] [count]`, `/locate <structure|biome>` (village, dungeon, tower, hut, well, outpost, desert ruins, trail ruins, ocean ruins, hushed city, fortress, snout camp, trial chambers, or a biome), `/setblock <x y z> <block>` and `/kill [player | mobs | monsters | <mob>]`. `/help` lists them all.
- **Statistics.** The pause menu shows what you've done in this world: blocks mined and placed, items crafted, mobs defeated, deaths, damage dealt and taken, distance walked, swum, flown and ridden, jumps, time played, fish caught and things eaten. On a server, the host keeps every player's statistics (and game mode) with the world, so they're there next time you join.
- **Axes and shovels.** Wooden, stone, copper, iron and dimond ones. Axes speed up anything wooden (and hit hard, but slowly), shovels anything earthy (dirt, grass, sand, gravel, snow, mud). Craft them from three (axe) or one (shovel) of the material and two sticks; Efficiency, Fortune and Unbreaking fit them.
- **Copper gear.** A copper pickaxe and sword (between stone and iron: they mine what stone mines, a bit faster, and last longer) and copper armour (between gold and iron).
- **Gliders and rockets.** The Hollow's outer islands (build out from the main island) have spires with loot: **Gliders**, **Boom Rockets**, **Hollow Boxes**, diamonds and gear. Wear a Glider in the chest slot, jump off something high and press Jump again to spread it; look down to dive and up to climb. Use a rocket while gliding for a push (craft rockets from gunpowder and string). Gliders wear out with use; repair them with feathers at an anvil.
- **Hollow Boxes.** Chests that keep what's inside when you break them: pick the box up and put it down somewhere else. Craft one from a chest and four Hollow Stone.
- **The sea.** Coral reefs in warm, shallow oceans (coral dies if it's taken out of the water), schools of **Fishies**, and at night **Soggy Groaners** that swim after you, some throwing **Soggy Spears**. A spear hits hard, and you can throw it (then go and pick it up).
- **Copper and bamboo.** Copper ore underground; copper blocks slowly turn from orange to green over about an hour of play nearby. Right-click one with Goo to wax it and stop it where it is. Bamboo grows in jungle groves (and keeps growing, up to 16 tall); make bamboo blocks, planks, mosaic, slabs and stairs.
- **Crafting tables and the recipe book.** Small recipes (four ingredients or fewer, the ones that would fit Minecraft's 2x2 grid) work anywhere; everything bigger needs a **Crafting Table (No Longer Decorative)** within four blocks (right-click it, or just open your inventory nearby; the host checks joined players too). The **recipe book** lists every recipe you've discovered (you discover one as soon as you've held any of its ingredients, or what it makes), with search, tabs, a "craftable now" filter and a **pin** that keeps one recipe's shopping list on screen while you gather it.
- **Bees and honey (over-engineered, like fishing and farming).** Wild **Bee Nests** hang in flowery trees, and every nest and **Beehive** keeps a colony record. A colony's foraging is the product of its population, health, daylight, weather, the flowers within seven blocks (variety is a bonus), the climate, privacy (bees work harder when nobody's watching) and its **queen** (Plain, Gentle, Busy or Hardy). Honey takes after the flowers: **Wildflower**, **Sunny**, **Blue**, **Lavender** or, from Torchflowers dug up at ruins, **Ancient**, and each bottle does something different. **Mites** creep in and spread between crowded hives (dust them with Wood Ash); a full, well-fed colony **swarms** into an empty hive nearby or flies off forever; a colony with no honey starves. Rob a hive without a **Bee Smoker** and its bees sting. Break a calm nest to catch its queen in a jar and start your own colony. The **Hive Tool** tells you everything, every harvest earns Beekeeper levels, the **Beekeeping Log** keeps count, and crops near a working hive grow a quarter faster. Honeycomb waxes copper, and makes **Honey Blocks**.
- **Archaeology (very in-depth).** Buried **dig sites** of four cultures: Sunken Sandstone ruins under deserts, Trailfolk ruins under forests, the Drowned Port on warm sea floors, and the Hushed Ones' city in the Deep Dark, each with its own pottery, relics and stories. Their fill hides **Suspicious Sand** and **Suspicious Gravel**: dig it and the find is lost. Hold right-click with a **Brush** instead: brushing builds **pressure** on the find, the grit sometimes shifts, and pushing too hard **cracks** it (Pristine, Fine, Worn, Cracked; a fourth crack shatters it). A Dimond Brush and Archaeologist levels are gentler. Depth matters: the recent layer gives pottery shards and coins, the old layer relics, clay tablets and map fragments, and the ancient layer the rarest relics, often **encrusted** (clean them at a **Restoration Bench**). Twelve shard designs go on **Decorated Pots**, **clay tablets** are someone's diary, **map fragments** point to another site, and the **Field Journal** records each culture's collection (finish one for a reward).
- **The Deep Dark.** Wide deepslate caverns near the bottom of the world, carpeted in **sculk** that listens. **Sculk Sensors** hear footsteps (sneak to walk silently), blocks broken and placed, deaths and explosions, and give off Zappy power. A sensor wakes **Sculk Shriekers**: each shriek is a warning and the darkness closes in, and the fourth summons **The Hush**, blind and enormous, which goes wherever it last heard something and shushes you through walls. **Sculk Catalysts** spread sculk around anything that dies nearby. Deep in the caverns stand **Hushed Cities**: deepslate halls, soul lanterns, and chests of Echo Shards, upgrade templates and enchanted books. A **Recovery Compass** points to where you last died.
- **Grindstone and smithing (Scorchite).** The **Grindstone** strips enchantments and gives some experience back, or grinds two worn copies of a thing into one. The **Smithing Table** upgrades Dimond gear to **Scorchite**, the new top tier, with a **Scorchite Upgrade Template** (from Hushed Cities, and copyable) and a **Scorchite Ingot** (scrap smelted from **Old Debris** deep in the Scorchlands, plus gold). Scorchite keeps its enchantments, hits harder, digs faster, lasts longer, shrugs off knockback, and floats in lava.
- **More animals.** **Sneakers** (foxes) nap through the day, hunt Clucksters at night and steal whatever's lying about; feed one Cluckets and it trusts you and brings you what it finds. **Ribbits** (frogs) hop about swamps and eat small Bloops, leaving a **Froglight** coloured by the climate. **Rollos** (armadillos) roll into an armoured ball when threatened and shed **scutes** (brush one for more); six scutes make **Woofer Armour** for your tame Woofer. **Bees** buzz between flowers and their hives.
- **Death messages.** Everyone in the world is told how you died ("Bot5 was struck down by a command. Harsh.").
- **Falling sand and gravel.** Sand, gravel, red sand and suspicious blocks fall when what's under them goes (dig under a beach and it pours in), and so do anvils, which hurt whatever they land on. Nothing moves until something changes nearby, so the generator's overhangs stay put until you disturb them.
- **Music.** A **Note Block (Plinky)** (eight planks and Zappy Dust) plays when you hit it or when Zappy power reaches it; right-click to tune it up a step (25 notes, two octaves). The block underneath picks the instrument: wood a double bass, stone a bass drum, sand or gravel a snare, glass a hi-hat, gold a bell, ice chimes, terracotta a flute, wool a guitar, anything else a harp. A **Jukebox** (eight planks and a Dimond) plays a **Music Disc** to everyone nearby, louder the closer you are, and the background music steps aside while it plays. Eight discs, each composed by the game itself (chords, a bass line, drums and a tune that comes back changed): Cat Probably, Blocks Again, Chirpier, Too Far, Mall Muzak, Strad-Adjacent, Wait For It, and Oinkstep. Discs turn up in dungeon, tower and Hushed City chests; Oinkstep comes from the Snouts.
- **The Scorchlands, filled in.**
  - **Fortresses** of Scorch Bricks stand over the lava sea: a hall and four long bridges out to towers, with chests (gold, saddles, Dimonds, sometimes Scorchite scrap). **Sizzler Cages** keep making **Sizzlers**, which hover above you and throw fireballs three at a time. Their **Sizzle Rods** make **Sizzle Powder**, which brews **Strength** and makes two Staring Eyes from a Stare Pearl.
  - **Weepers**, huge and miserable, drift over the caverns crying explosive fireballs. Hit any fireball and it goes back where it came from (a Weeper's own one finishes it off). A **Weeper Tear** brews **Regeneration**.
  - **Strutters** walk on lava (and shiver blue off it). Saddle one, hold an **Ember Shroom on a Stick** and ride it across the lava sea; feed them Ember Shrooms to breed them.
  - **Snouts** live in **Snout Camps** of gilded rock and gold. Leave their gold alone and they leave you alone; throw a gold ingot near one and it admires it for a bit, then throws something back (obsidian, Stare Pearls, string, potions, now and then the Oinkstep disc).
- **Raids.** **Pilferers** (crossbows) camp around **Pilferer Outposts** and wander in **patrols** led by a captain with an Ominous Banner. Defeat the captain and you get **Bad Omen**; walk into a village with it and a **raid** begins. Waves of raiders march on the square (three on Easy, five on Normal, seven on Hard): Pilferers, then **Hacklers** with axes, then **Invoicers** (who summon flying **Fees** that pass through walls, and send **Late Fees** snapping up out of the ground) and big **Rampagers**. A bar at the top shows the wave and who's left. Win and everyone there is a **Hero of the Village** for twenty minutes (a third off every trade); lose every Hmmer, or leave, and the raid is lost. Ring a village's **Bell** to make raiders glow and send the Hmmers indoors. Invoicers carry a **Totem of Not Dying**: keep it in your hotbar and it saves you once from a killing blow.
- **Biome colours.** Grass, leaves and water take their colour from the biome they're in (lush in the jungle, dusty in the badlands, murky green in swamps), and blend smoothly across biome borders instead of changing at a hard line.
- **Cherry Groves and Mangrove Swamps.** Pink-leaved **Cherry** trees on gentle hills, with **Pink Petals** across the grass; and warm, muddy **Mangrove Swamps** where the trees stand on tangled **roots** over the water. Both woods make their own planks.
- **Automation.** An **Observer** sends a short pulse out of its back whenever the block in front of it changes. A **Crafter** crafts once each time it's powered, using the recipe whose ingredients are inside it, and spits the result out of its face. A **Copper Bulb** lights or goes dark each time it's powered (it stays as it is in between).
- **Trial Chambers.** Copper-and-tuff halls deep underground (`/locate trial_chambers`). **Trial Spawners** wake when you come close and send out a wave of monsters, a couple at a time and more with more players; beat the wave and they pay a **Trial Key** and some loot, then rest for twenty minutes. A Trial Key opens a **Vault** (dimonds, armour trim templates, crossbows and more). **Breezes** bounce around keeping their distance and throw **Wind Charges** that knock you flying without breaking anything; craft their **Breeze Rods** into Wind Charges of your own and throw one at your feet to launch yourself.
- **Fireworks.** Use a Boom Rocket on the ground (not gliding) and it shoots up and bursts in colour. A **Crossbow** fires Rockets when you have any (they burst on what they hit and hurt everything nearby) and Pointy Sticks otherwise. Dispensers launch Rockets too.
- **Armour trims.** A trim template, a piece of armour and iron, gold, dimond or copper on the **Smithing Table** pick out a pattern on the armour (Coast, Wild, Ward or Spire), and everyone can see it. Templates come from Trial Chambers and copy like the upgrade template (seven dimonds and some tuff bricks).
- **Goats, Axolotls and Camels.** **Goats** live high in snowy and taiga hills, jump high, and now and then lower their heads and charge you; if one misses and hits a wall it may knock off a **Goat Horn** (blow it to be heard a long way off). **Axolotls** swim in jungle, mangrove and swamp water and hunt Soggy Groaners and fish; when they win, everyone nearby gets a little **Regeneration**. **Camels** wander the desert: saddle one (no taming needed) and ride it, with a friend in the back seat; Jump makes it **dash** forward every few seconds. Goats breed on wheat, Camels on cactus, Axolotls on tropical fish.
- **Spyglass, Lodestone and Bundle.** Hold right-click with a **Spyglass** (glass and two copper) to zoom right in. Use a compass on a **Lodestone** (eight stone bricks round an iron) and it points there instead of home until the Lodestone is broken. A **Bundle** (two string and wool) holds a mix of small stacks, 64 items' worth: in the inventory, left-click it carrying a stack to put the stack in, right-click it to take the last thing out; use it in the world to tip it all out. Bundles work for joined players too (the host keeps track of what's in each one).
- **Ominous Trials and the Mace.** Drink an **Ominous Bottle** (from trial waves) for **Bad Omen**, then walk up to a Trial Spawner: it turns **ominous**, with bigger and tougher waves that pay an **Ominous Trial Key**. Those open **Ominous Vaults**, which can hold a **Heavy Core**. Put a Heavy Core and a Breeze Rod together for the **Mace**: an ordinary hit on the ground, but fall on something from a height and it lands harder the further you fell (and knocks everything nearby away), and you bounce off without taking fall damage.
- **The Pale Garden.** A grey, quiet forest of huge **Pale Oaks**, with **Pale Moss** on the ground and hanging from the branches. Some trunks hide a **Creaking Heart**. At night the heart wakes and a **Creaking** steps out of it: it can't be hurt and it only moves when nobody is looking at it. Break its heart to be rid of it; at dawn it goes back in by itself.
- **Sniffers and ancient seeds.** **Sniffer Eggs** turn up at Trailfolk dig sites. Put one down and it hatches (faster on moss); the **Sniffer** wanders about sniffing and now and then digs up **Torchflower Seeds** or a **Pitcher Pod** from grass, dirt and moss. Plant them for **Torchflowers** and **Pitcher Plants**.
- **Books and lecterns.** A **Book and Quill** (book, feather and coal) opens a writing screen: type pages, flip between them, then **sign** it with a title to make a **Written Book** anyone can read. Put a book on a **Lectern** (four planks and a bookshelf) and everyone who comes by can read it (or take it with the button on the reading screen). Joined players' books are kept by the host.
- **Banners and the loom.** Make a **Banner** from six wool and a stick and put it up on the ground or a wall. On a **Loom** (two planks and two string), add up to two pattern layers (stripes, a pale, a cross, a border, a chevron or a chief) in any dye colour. Use the loom holding a shield to paint one of your banners on it. Banners you put up appear on your maps.
- **Personal Chest.** Eight obsidian round a Staring Eye. Every Personal Chest opens the same private storage, yours alone: put something in at home and take it out from a Personal Chest on the other side of the world. On a server, each player has their own.
- **Animal colourings.** Gallopers come in five coats, Woofers and Mooers in a few shades, and Axolotls in four colours, plus a very rare blue one. Babies usually take after a parent.
- **Better maps.** Use a map to zoom out (1:1, 1:2, 1:4 and 1:8). Maps shade the slopes, mark banners, and a map in an item frame shows the land around it.
- **Offhand and tidying.** Press **F** to swap the held item into your **other hand** (the slot under your armour). A shield blocks from either hand, and a block or torch in the other hand gets placed when your main hand holds a tool or nothing. Chests and your backpack have a **Sort** button that stacks things together and puts them in order.
- **Copper Golems.** Put a pumpkin (or a Jack o'Lantern) on a block of copper and a **Copper Golem** steps out. Give it a **Copper Chest** (a chest and four copper) and it takes things out of it, a stack at a time, and carries them to a chest nearby that already holds that thing, or else to an empty one. Nowhere to put something? It puts it back and has a rest.
- **Floaties.** Scorch Fortress chests sometimes hold a **Dried Floaty**. Put it down next to water and it wakes up as a baby **Floaty**, which grows into a big, gentle, cloud-white flier in twenty minutes (feed it flowers to hurry it along). Put a **Harness** (three wool, two string, two glass) on a grown one and climb aboard: four seats, and whoever got on first steers. It flies where the driver looks; W to go, Jump to rise. Shears take the harness off.
- **More of the Pale Garden.** **Eyeblossoms** grow in the moss: shut by day, open and faintly glowing at night. Hitting a Creaking does it no harm, but its heart oozes **Resin**: nine clumps make a block, and they bake into **Resin Bricks** for building.
- **Fireflies, leaf litter and wildflowers.** **Firefly Bushes** grow in swamps and mangroves, and at night fireflies blink around them. **Leaf Litter** drifts under forest trees, and **Wildflowers** fill the meadows.
- **Spears and Rotsteeds.** Wooden, stone, iron and dimond **spears** (two sticks and the material) stab, and throw like the Soggy Spear. Hit something with a spear from a mount that's moving and the charge goes into the blow (and sends it flying). **Rotsteeds**, undead horses, wander the plains at night: tame, saddle and ride one like a Galloper.
- **Distant terrain.** Past the edge of the loaded world, a low-detail picture of the land carries on to the horizon (about two and a half times the render distance), so mountains, coasts and seas show from far off. Turn it off in Video Settings.
- **Smarter mobs.** Mobs find their way round walls, up steps and through open doors to get where they're going (or to you), instead of walking straight into things, and go as near as they can when there's no way through.
- **Moon phases.** The moon waxes and wanes over an eight-day cycle. A full moon brings more monsters out (and hatches eggs sooner); a new moon is quieter. F3 shows the day and the phase.
- **Home blocks.** A **Campfire** (three sticks, coal and three logs) cooks raw food put on it (right-click, or just drop it there) without fuel, and lights the place up; don't stand in it. A **Smoker** (a furnace and four logs) cooks food twice as fast, and a **Blast Furnace** (a furnace, five iron and three stone) does everything else twice as fast. A **Barrel** (seven planks) is a chest. An **Armour Stand** (six sticks and stone) holds a set of armour and wears it for everyone to see. **Paintings** (eight sticks and two wool) hang on walls; the picture depends on where you hang one.
- **Shipwrecks and buried treasure.** Wrecks lie broken on the sea bed with a chest at each end (`/locate shipwreck`). Their chests often hold a **Treasure Map**: hold it to see the land around the X and how far off it is, then dig up the **Buried Treasure** in the beach (gold, iron, dimonds and more).
- **Turtles, Dolphins, Pandas, Polar Bears and Llamas.** **Turtles** come ashore on beaches; fed Lily Pads, a pair lays **Turtle Eggs** in the sand that hatch at night, and the babies shed **Turtle Scutes** as they grow (five make a **Turtle Shell**, a helmet you can see much further underwater in). **Dolphins** swim in pods in the open sea; feed one a fish and it leads you to the nearest shipwreck. **Pandas** laze about in jungles and breed on bamboo. **Polar Bears** roam the snow and leave you be, unless you go near a cub. **Llamas** graze the plains: feed one hay to tame it and it follows you about; give it a chest and it carries a chest's worth of things (right-click it empty-handed to open the pack; sneak-click to tell it to stay).
- **Villager life.** Hmmers who move in now have eight jobs: besides Farmers, Librarians, Smiths and Fishers there are **Clerics**, **Armourers**, **Cartographers** (who sell Treasure Maps) and **Butchers**. A Groaner that catches a Hmmer turns it into a **Zombie Hmmer**; give one a Golden Chop and after a while it's itself again, and so grateful you get its best prices. Now and then a **Wanderer** turns up with odds and ends to sell, and wanders off again after a couple of days.
- **Advancement tabs.** The advancements screen sorts them into Getting Started, Creatures, Home and Craft, and Adventure, with how many of each you've done.
- **Video settings.** **Options > Video Settings** has render distance, brightness, **Max FPS** (30 to 240, or unlimited), **VSync**, **Anti-aliasing** (off, 2x, 4x, 8x), **Particles** (all, fewer, minimal), **View Bobbing**, **Fog**, clouds (fancy, fast or off), leaves, water, lighting, **Shadows** (the sun and moon cast real shadows from blocks, mobs and players), **Water Depth** (deep water darkens and the bright ripples of caustics play across the sea floor), **Distant Terrain** and fullscreen. VSync and anti-aliasing apply the next time the game starts.
- **Saved settings.** Render distance, FOV, sensitivity, fullscreen, volume, music, key bindings, skin, subtitles, colour-blind mode, graphics options, brightness, and your multiplayer name and last server are kept in `settings.txt` in the data folder (see Download and play). It's plain `key=value` text: edit it by hand if you like, and anything it can't make sense of falls back to the default.

## Controls

These are the defaults; change them in **Options > Controls**. Game controllers work too (see Features).

| Key | Action |
| --- | --- |
| WASD / arrows | Move |
| Mouse | Look |
| Space | Jump / swim up (double-tap in creative to fly) |
| Shift | Sneak / fly down |
| Ctrl or R | Sprint |
| Left mouse | Mine / attack |
| Right mouse | Place block / eat (when hungry) / light TNT / fire a bow / open a chest, furnace, anvil, enchanting table, brewing stand, hopper, dispenser, loaded minecart or door / open gates and trapdoors / switch a comparator's mode or a beacon's effect / put on armour / block with a shield / trade with a Hmmer / feed, shear or tame animals / ride a Galloper / name a mob / get in a boat or minecart / write on a sign / fill a bucket or bottle / drink or throw a potion |
| Middle mouse | Pick block (creative) |
| 1–9, mouse wheel | Select hotbar slot |
| E or Tab | Inventory and crafting (shift-click a recipe to craft many) |
| T or Enter | Chat (`/` starts a command: server commands, or ones from script mods). Up and Down recall what you sent; Page Up, Page Down or the wheel scroll back through the last 100 lines |
| Q / Ctrl+Q | Throw one of the held item / the whole stack |
| F2 | Screenshot (saved as a PNG in the `screenshots` folder next to the game) |
| F5 | Toggle third person |
| F3 | Debug screen: position, biome, light and more on the left; system, graphics card, OpenGL version, frame times and the block you're looking at on the right; a frame-time graph in the corner |
| F | Swap the held item into the other hand |
| F11 | Fullscreen |
| Esc | Pause menu |

## Multiplayer

### On the same network (LAN)

1. **Host:** start or continue a world, press **Esc**, then click **Open to LAN**. The pause menu shows your address, for example `192.168.1.20:25565`.
2. **Friends:** on the title screen, click **Multiplayer**, type a name and the host's address, then click **Join Server**. For a second copy on the same computer, use `127.0.0.1`.
3. Press **T** to chat.

### Over the internet, from your own world

1. *(Optional, recommended)* Type a password in the **Password** box on the Multiplayer screen before hosting. The world uses it.
2. In your world, press **Esc** and click **Open to Internet**. The game asks your router, using UPnP, to forward TCP port 25565 to your computer, and shows the address to share, for example `203.0.113.7:25565`.
3. Friends join with that address and the password.

If the router says no (UPnP is off, or it's not supported), the pause menu says so. You can then forward **TCP port 25565** to your computer by hand in the router's settings.

If the menu says you're **behind a second NAT**, your internet provider shares one public address between many customers ("carrier-grade NAT"). People outside can't reach you, no matter how the port is set. There are two ways around it:
- If your connection has IPv6, the pause menu shows an `[IPv6]:port` address. That usually works directly, as long as your friends have IPv6 too.
- Otherwise, run a dedicated server somewhere that has a public address.

### Dedicated server (VPS, Raspberry Pi, spare PC)

```sh
cargo build --release
./target/release/minceraft --server --password hunter2
```

The server has no window and needs no GPU or sound card. Options:

- `--port N`: TCP port to listen on (default 25565).
- `--world FILE`: the world save (default `saves/server.mncr`, created if missing). Block edits, containers, signs and frames go in region files beside it (`saves/server.regions/`).
- `--seed N` and `--creative`: settings for a new world.
- `--keep-inventory`: players keep their things when they die.
- `--difficulty peaceful|easy|normal|hard`: overrides the world's difficulty.
- `--max-players N`: player limit (default 16).
- `--allow-list`: only let in the players on the allow-list (and operators).
- `--upnp`: ask the router to forward the port, for servers at home.

Console commands:

- `list`: who's online.
- `players`: everyone the server remembers, with where they are, their health and experience.
- `info <name>`: one player's position, health, food, experience, enchanting count, everything they carry and what they wear, whether they're online or not.
- `forget <name>`: drop a player's saved things, so they start afresh next time.
- `say <text>`, `kick <name>`, `time <day|noon|night|midnight|0.0-1.0>`, `password <pw|off>`.
- `ban <name|ip>`, `unban <ip>`, `bans`: bans are by IP address.
- `op <name>`, `deop <name>`, `ops`: operators can use all of these commands in chat, with a slash (`/kick Bob`). Everyone else can use `/list` and `/help`.
- `allowlist on|off|list|add <name>|remove <name>`: turning it on adds whoever is online, so nobody is kicked out by surprise. Operators always get in.
- `save`, `stop`, `help`.

The lists live next to the server in plain text: `allow-list.txt`, `ops.txt` and `banned-ips.txt`. Names aren't accounts, so combine the allow-list with a password. Whoever hosts a world from the pause menu is always an operator there and can use the same commands in chat (their lists last until the world is closed).

The world autosaves every 5 minutes and when you type `stop`. Stopping with Ctrl+C loses anything since the last autosave.

On a cloud server, allow TCP port 25565 in its firewall or security group.

### How it works

- **Who runs what:** the host (or dedicated server) runs the world: mobs, TNT, the time of day and saving. Players send it their edits, movement and attacks, and it passes them on to everyone.
- **Joining:** a joining player gets the world seed plus the block edits, signs and frames the host has in memory, and those of each region as the host reads it from disk, so only the changes travel over the network, never whole chunks.
- **Passwords:** checked with a challenge-response using SHA-256, so the password itself is never sent over the network.
- **Everything else is unencrypted:** chat and game traffic can be read by anyone on the path, so don't share secrets in chat.
- **Anti-cheat and anti-grief:** the host doesn't take a player's game at its word.
  - **Block edits** must be within reach of where the server thinks the player is, at a human rate, and follow the game's rules: you can break things, place blocks into empty space, till and trample farmland, but not turn stone into diamond ore, break bedrock, place bedrock, or grow crops yourself. Rejected edits are undone on the player's screen.
  - **Movement:** positions must make sense. Jumping across the map puts you back where you were.
  - **Attacks, TNT, farming and fishing:** hits, ignitions, fertiliser and Soil Probe use must be within reach and at a human pace. Fish are rolled by the host, so a modified client can't award itself a legendary one.
  - **Chat:** rate-limited (a burst of five, then one a second), with control characters stripped. Nobody can call themselves "Server".
  - **Strikes:** players who keep sending things a real game wouldn't are kicked.
- **Connection limits:**
  - Messages from players are capped at 256 KB, and the host reads a bounded amount from each connection per frame, so one flooding connection can't stall everyone.
  - At most three connections per address (except from the same computer).
  - Five wrong passwords lock an address out for ten minutes.
  - Connections that go silent for 30 seconds, or don't finish logging in within 15 seconds, are dropped.
- **Inventories:** each player's inventory lives on their own machine, so using it feels instant, but the host keeps a ledger of what every player really owns. It builds the ledger only from what it has seen happen: blocks they broke (the host rolls the random drops), loot and catches it sent, recipes it let them craft, blocks they placed, arrows they shot and food they ate.
  - Placing, planting, crafting, shooting, fishing and fertilising all need the items in the ledger. Hoes, rods and probes must be owned, and a sword only hits harder if you really have it.
  - Mining can't go faster than the tools you really own allow.
  - Every few seconds each player's game compares counts with the host. If a modified client has conjured items, the host's numbers win.
  - Items on the ground live on the host too. Breaking a block puts its drops on the host's ground, and players pick things up by asking the host, which checks they're close enough and have room, then hands them over through the ledger. Throwing (Q) takes the items from the ledger before they land, so you can't throw what you don't have.
  - Tools wear out on the host too. It can't see which of your pickaxes you're holding, so it counts uses per kind of tool; every time those add up to one tool's durability, one leaves the ledger. Your own game breaks the same tool at the same moment, and a modified one that doesn't loses it at the next check anyway.
  - Experience is counted by the host: orbs are its, it decides who collected them, and it tells each player their total. Anvil repairs are checked against that total and the ledger (the iron has to be real, and so do the levels), so modified clients can't repair for free.
  - Enchanting is checked the same way: the host works out the table's offers itself (from the bookshelves it sees and how many times the player has enchanted before), charges the levels and gold, and remembers which enchanted things each player has. Efficiency, Sharpness and Fortune only count for a player if the host knows about their enchantment; one a modified client makes up is ignored, and it falls off anything they throw or put in a chest.
  - Dying drops everything through the host, like throwing: it takes the items from the ledger and puts them on its ground.
  - Doors and slabs follow the same rules: a door's top half only goes on its own bottom half, opening one is free, and placing stairs, slabs or doors costs the item.
  - Chests and furnaces live on the host. Players see a copy of whatever they have open, and every move in or out is checked against both the container and the ledger. You can't put in what you don't own, take what isn't there, or reach into a chest from across the map. A broken container spills its contents on the host's ground.
  - Creative worlds skip all of this, since everything is free there anyway.
- **Saving and leaving:** only the host or server saves. It remembers each player by name: their inventory (with wear and enchantments), armour, experience, health, hunger and where they were. A player who leaves and comes back, even after the server restarts, picks up where they left off. The host's ledger is what counts if their report and the ledger disagree. There are no accounts, so use a password to keep strangers from borrowing a name. If the host leaves, everyone returns to the title screen.
- **Addresses:** the host listens on IPv4 and, where available, IPv6. Addresses can be `IP`, `IP:port`, `[IPv6]:port` or a hostname like `play.example.com`.

## Getting started in survival

Punch a **Tree Chunk** to get logs, then craft **Planks**, then **Sticks**, then a **Wooden Pickaxe**. From there, stone gives you a stone pickaxe, iron gives you an iron pickaxe, and iron is what you need to mine **Dimonds**. Coal plus a stick makes torches, and eight cobblestone make a **Furnace**, which turns raw food into much better food. Small recipes work anywhere, but bigger ones (the furnace, chests, tools) need a **Crafting Table** close by, so make one early. The recipe book fills up as you pick things up.

## Code map

| File | What it does |
| --- | --- |
| `src/main.rs` | Window, input, main loop, screenshot scenes |
| `src/screens/` | The menus, settings, inventory and crafting, crafting stations, logs and the HUD |
| `src/game.rs` | Gameplay rules, spawning, explosions, scene assembly |
| `src/world.rs` | Chunks, threaded terrain generator, raycasts |
| `src/mesher.rs` | Chunk meshing with AO, smooth lighting and greedy merging |
| `src/tint.rs` | Biome colours for grass, leaves and water, blended across borders |
| `src/render.rs` | Raw OpenGL pipelines and shaders |
| `src/lod.rs` | Distant terrain past the render distance |
| `src/pathing.rs` | Mobs finding their way round walls, up steps and through doors |
| `src/moon.rs` | Moon phases and what they change |
| `src/entity.rs` | Physics, mobs, models, particles |
| `src/player.rs` | Player controller |
| `src/texture.rs` | Procedural texture atlas |
| `src/block.rs` | Block and item registry, tools, recipes |
| `src/mods.rs` | Mod loader (`mod.txt` parser, textures, mod packs) |
| `src/scripting.rs` | Script mods: sandboxed Rhai engine, events and game API |
| `src/inventory.rs` | Inventory and crafting |
| `src/containers.rs` | Chests and furnaces: contents, cooking, host-checked moves |
| `src/drops.rs` | Items on the ground: physics, merging, pickups, network sync |
| `src/building.rs` | Slabs, stairs and doors: facing, merging, opening |
| `src/hunger.rs` | Food, saturation and exhaustion |
| `src/xp.rs` | Experience levels and orbs |
| `src/anvil.rs` | Anvil repairs and merging |
| `src/enchant.rs` | Enchantments, the enchanting table and its offers |
| `src/structures.rs` | Dungeons, towers, huts and wells, and their loot |
| `src/treasure.rs` | Shipwrecks, Treasure Maps and Buried Treasure |
| `src/trial.rs` | Trial Chambers: Trial Spawners, Vaults, Breezes' wind charges |
| `src/fireworks.rs` | Firework rockets and crossbows |
| `src/trims.rs` | Armour trims |
| `src/gadgets.rs` | The Spyglass, Lodestone compasses and Bundles |
| `src/creaking.rs` | The Creaking and its heart |
| `src/sniffers.rs` | Sniffers, their eggs and the seeds they dig up |
| `src/books.rs` | Books and quills, written books and lecterns |
| `src/banners.rs` | Banners, their patterns and the loom |
| `src/stash.rs` | Personal Chests' shared storage |
| `src/weather.rs` | Rain, snow, thunderstorms and lightning |
| `src/players.rs` | Remembering joined players between visits |
| `src/rules.rs` | World rules: keep inventory, difficulty, daylight and weather cycles |
| `src/settings.rs` | `settings.txt` |
| `src/paths.rs` | The data folder (and carrying files over into it), the crash log |
| `src/updates.rs` | The new-version check and download button |
| `src/backups.rs` | World backups: made on opening a world, listed and restored from the world list |
| `build.rs`, `assets/minceraft.ico` | The Windows exe's icon (drawn from the game's textures by `--export-icon`) |
| `src/keybinds.rs` | Rebindable keys and mouse buttons |
| `src/pad.rs` | Game controllers |
| `src/liquids.rs` | Flowing water and lava, buckets |
| `src/animals.rs` | Breeding, taming and shearing |
| `src/wiring.rs` | Zappy Dust, switches, lamps and powered doors |
| `src/villagers.rs` | Hmmers and trading, their jobs, Zombie Hmmers and Wanderers |
| `src/combat.rs` | Attack charge, sweeps, shields and knockback |
| `src/scorch.rs` | The Scorchlands and portals |
| `src/vehicles.rs` | Boats, minecarts (plain, chest and hopper), rails and detector rails |
| `src/decor.rs` | Signs and item frames |
| `src/navigation.rs` | Compass and map |
| `src/admin.rs` | Server commands, allow-list and operators |
| `src/light.rs` | Sky and block light, spread and retracted per chunk |
| `src/trees.rs` | Saplings, tree growth and leaf decay |
| `src/carpentry.rs` | Fences, gates, ladders, trapdoors, panes and dyes |
| `src/fire.rs` | Fire spreading and burning |
| `src/potions.rs` | Brewing, potions and their effects |
| `src/contraptions.rs` | Zappy torches, repeaters, pistons and dispensers |
| `src/hoppers.rs` | Hoppers moving items between containers |
| `src/horses.rs` | Gallopers, Camels and Rotsteeds: taming, saddles, seats and riding |
| `src/hollow.rs` | The Hollow, Crypts, Staring Eyes and the Hollow Wyrm |
| `src/nametags.rs` | Skins and Name Tags |
| `src/access.rs` | Subtitles and the colour-blind filter |
| `src/golems.rs` | Clankers guarding villages, and Copper Golems sorting chests |
| `src/floaty.rs` | Floaties: hatching, harnesses and four-seat flying |
| `src/nature.rs` | Fireflies (and the ground covers they live among) |
| `src/home.rs` | Campfires, Smokers, Blast Furnaces, Barrels, Armour Stands and Paintings |
| `src/wildlife.rs` | Turtles, Dolphins, Pandas, Polar Bears and Llamas (and their packs) |
| `src/beacon.rs` | Beacons and their effects |
| `src/crafting.rs` | Crafting tables and the recipe book |
| `src/bees.rs` | Bees, colonies, hives and honey |
| `src/archaeology.rs` | Dig sites, brushing, finds, restoration and the Field Journal |
| `src/deepdark.rs` | The Deep Dark, sculk, shriekers and The Hush |
| `src/smithing.rs` | The grindstone and smithing table |
| `src/critters.rs` | Sneakers, Ribbits, Rollos and Axolotls |
| `src/falling.rs` | Sand, gravel and anvils falling |
| `src/music.rs` | Note blocks and jukeboxes |
| `src/songs.rs` | Note block instruments and the music discs, synthesised |
| `src/fortress.rs` | Fortresses, Snout camps, fireballs, Sizzler cages and bartering |
| `src/raids.rs` | Outposts, patrols, raids, the Bell and the Totem |
| `src/playtest.rs` | `--playtest`: a host and bots checking multiplayer agrees |
| `src/save.rs` | Binary save format |
| `src/regions.rs` | Region files: block edits streamed from disk |
| `src/net.rs` | Network protocol and non-blocking TCP |
| `src/multiplayer.rs` | Host and client sync logic |
| `src/server.rs` | Headless dedicated server |
| `src/upnp.rs` | Router port forwarding (UPnP) |
| `src/sound.rs` | Sound synthesis and playback |
| `src/noise.rs` | Perlin noise and RNG |
| `src/ui.rs` | HUD and menu widgets |
| `src/modes.rs` | Survival, creative, spectator and hardcore |
| `src/cheats.rs` | `/tp`, `/give`, `/gamemode`, `/weather`, `/seed` |
| `src/stats.rs` | Statistics |
| `src/glider.rs` | Gliders and rockets |
| `src/boxes.rs` | Hollow Boxes that keep their contents |
| `src/copper.rs` | Random block ticks: copper weathering, bamboo growth, coral drying out |
| `src/tools.rs` | Axes, shovels and the copper tier: which tool digs what, and how fast |

For headless testing, `minceraft --screenshot out.png --mode title|survival|creative|inventory|night|options|controls|host|join|internet|mods|palette|showcase|worlds|createform|newworld|farm|fish|zoo|kitchen|chest|furnace|building|armour|death|anvil|rules|xp|enchant|table|hut|tower|well|dungeon|ravine|rain|thunder|snow|liquids|animals|zappy|trade|scorch|portal|vehicles|decor|carpentry|brewing|contraptions|hollow|machines|village|swamp|jungle|badlands|taiga|cave|caverain|modzoo|backups|stats|video|newblocks|glider|reef|spire|cherry|mangrove|trials|newmobs|palegarden|banners|golems|advancements|shipwreck|homestead [--frames N] [--time 0..1] [--yaw R] [--pitch R] [--pos x,y,z] [--distance chunks] [--colour-blind] [--subtitles] [--flat-lighting] [--brightness 0..1] [--hold ITEM] [--ui-scale 0.8..1.4] [--fast-clouds] [--pretend-update TAG] [--seed N]` renders a scene and saves a PNG.

`minceraft --playtest [--bots N] [--seconds S] [--seed N] [--port N]` is a multiplayer check in one command: it hosts a world and connects bot players to it over real sockets (no window needed). The bots wander, build and knock down blocks, chat and fill a shared chest, then it checks that the host and every bot agree on every block, that the bots see and hear each other, that each bot's inventory matches the host's ledger, and that the chest holds what went in. Then it tries the newer things: a bot glides (the host must see it), throws a Soggy Spear and picks it up again, breaks a full Hollow Box and puts it down elsewhere (the contents must come too), and is made a spectator (hidden from the others, its block edits refused); and the host must have everyone's statistics. Then the latest batch: a chest is refused without a crafting table and allowed with one, a bot smokes a hive and bottles its honey, brushes suspicious sand, upgrades a pickaxe to Scorchite at a smithing table, and `/kill` must reach everyone as a death message. And the newest: sand and gravel fall for everyone, a bot tunes a note block and swaps a disc in and out of a jukebox, swats a fireball back, gets Bad Omen and sees a raid's bar, and is saved by a Totem. It exits non-zero on any mismatch; CI runs it with six bots for a minute.

Not affiliated with any block-game company.
