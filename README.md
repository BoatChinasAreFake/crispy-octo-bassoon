# MINCERAFT

A native, compiled block-building parody written in Rust, with no browser, no web view and no JavaScript. It opens a real OpenGL window through [macroquad](https://github.com/not-fl3/macroquad)/miniquad, and the voxel renderer talks to the GPU directly with its own shaders and vertex buffers.

The game ships with **no image or audio files**. At startup it builds the whole texture atlas (grass, ores, mobs, tools, hearts, the logo) from noise and tiny ASCII sprites, and synthesises every sound effect and the music.

Its only dependencies are macroquad (window, input, audio) and [Rhai](https://rhai.rs), the sandboxed scripting language used for code mods.

## Build and run

You need a Rust toolchain ([rustup.rs](https://rustup.rs)).

```sh
cargo run --release
```

Linux also needs the X11/GL and ALSA development libraries, for example on Debian/Ubuntu:
`sudo apt install libx11-dev libxi-dev libgl1-mesa-dev libasound2-dev`.
If you can't install ALSA, `cargo run --release --no-default-features` builds a silent version.
Windows builds with no extra setup. macOS should work but hasn't been tested.

Run the tests with `cargo test --release`. GitHub Actions runs the tests, Clippy and a silent (`--no-default-features`) build on every push and pull request (`.github/workflows/ci.yml`).

## Features

- **Infinite procedural terrain.** Seeded Perlin noise produces oceans, beaches, plains, forests, deserts, snowy biomes, ridged mountains, spaghetti caves, big caverns (the deepest ones flooded), ravines that split the surface open, Glowshrooms and Pointy Rocks lighting and littering the caves, ore veins (gold included, for all the good it'll do you), trees, flowers, tall grass, pumpkins, desert Pokey Plants and frozen seas. Chunks generate on background threads.
- **Custom voxel renderer.** It culls hidden faces, adds per-vertex ambient occlusion and smoothed sky lighting, and uses frustum culling, translucent sorted water, cutout leaves and glass, fog, and mipmaps.
- **Compact world storage.** Like Minecraft, each 16-block-tall section of a chunk stores a small palette of the blocks it contains plus packed indices (0 bits per block for all-air or all-stone sections, 4–8 bits for mixed ones, direct ids past 256 kinds). Typical terrain averages under 2 bits per block. F3 shows the live figure.
- **Dynamic lighting.** Torches and Glowrock light the area around them, torchlight is warm, and a held torch lights your way.
- **Day/night cycle** (10 minutes), with a sun, moon, stars, sunrise/sunset glow and scrolling clouds.
- **Physics.** AABB collision, gravity, sprint-jumping, sneaking (it stops you walking off ledges), swimming and fall damage.
- **Survival mode.** Health, hunger, experience, mining times that depend on your tool, pickaxe tiers that gate ore drops, tools that wear out (and an anvil to fix them), 75 crafting recipes, food, cooking, chests, armour, beds, a bow, farming, fishing, death (which drops your things) and respawn.
- **Hunger.** A food bar of ten drumsticks, with Minecraft's rules. Sprinting, jumping, swimming, fighting, digging and getting hurt use up hidden saturation first, then food. With a full bar you heal quickly; at 18 points or more you heal slowly. At 6 points or less you're too hungry to sprint, and on an empty bar you starve down to half a heart (dramatic, but not fatal). Food fills the bar: raw food barely helps, cooked food keeps you full much longer. You can't eat when you're full, except legendary snacks (Suspiciously Golden Oinkchop, Big Bob), which also heal you outright. Cake is 7 drumsticks in one bite.
- **Durability.** Pickaxes, swords, the hoe, the bow, the Fishing Stick and armour wear out, with Minecraft's numbers: 59 uses for wood, 131 for stone, 250 for iron and 1561 for Dimond. A bar under the item shows how worn it is, and the tooltip counts the uses left. Breaking a block uses a tool once (swords twice, and blocks that break instantly not at all). Hitting a mob uses a sword once and anything else twice. Every bow shot, catch and tilled block counts too. Each piece of armour takes a quarter of every hit it softens. Wear stays with the item in chests, on the ground and in saves.
- **Experience.** Green orbs from defeated mobs (5 for monsters, 1 to 3 for farm animals), coal, iron, gold and Dimond ore, float toward the nearest player. Taking things out of a furnace and catching fish pay experience too. Levels follow Minecraft's curve, and the bar and your level sit above the hotbar. Dying drops 7 points per level (at most 100) as orbs and loses the rest, unless the world keeps inventories.
- **Structures.** The generator builds things, the same in every copy of a world: mossy **Dungeons** deep underground, **Ruined Towers** crumbling toward the top, **Huts** (definitely not a village) with a bed, a furnace and a chest, and desert **Wishing Wells** with something at the bottom. Their chests are filled the first time the chunk loads, with loot to match: dungeons have the good stuff (Dimonds, gold, iron, used and sometimes enchanted tools), huts have bread and seeds. F3 shows the nearest one.
- **Weather.** Clear skies turn to rain (snow in cold places) and now and then a thunderstorm. Rain dims the day, greys the sky, waters crops like a nearby pond does and makes fish bite faster. Thunderstorms are dark enough for monsters to come out, and lightning strikes near players: it hurts, and it sets off TNT. Sleeping through a storm clears it. Joined players see the same weather.
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
- **Death drops.** Dying drops your whole inventory, armour included, where you fell. The death screen tells you where, and it all waits five minutes. Worlds created with **Keep Inventory** on (a toggle on the Create World screen, or `--keep-inventory` for servers) let you keep everything instead.
- **Creative mode.** Flight, instant breaking, infinite blocks, pick-block and a full item palette.
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
- **Advancements.** 48 of them, each with a toast and a fanfare ("Getting Wood", "DIMONDS!", "Don't Blink", "The Cake Is Not a Lie"...). They're saved per world, and the pause menu lists them.
- **TNT.** Light it with a torch (or bare hands), and it chain-reacts.
- **Sound.** Synthesised effects for mining, placing and footsteps (different for stone, wood, grass, sand and glass), plus hurt sounds, oinks, baas, clucks, moos, rattles, skittering, bloops, bow twangs, groans, Hisser hisses, Starer warps, boings, an advancement fanfare, explosions, eating, splashes, item pickups, crafting and menu clicks. Sounds get quieter with distance. A calm procedural tune drifts in now and then. Volume and music are in Options. Run `minceraft --export-sounds <dir>` to write every sound out as a WAV.
- **Multiplayer over LAN or the internet.** Open any world from the pause menu (the game can ask your router to forward the port by itself), or run a headless dedicated server. It syncs blocks, player movement, mobs, TNT, explosions, damage, loot, sounds, time of day, weather and chat, and remembers every player's things between visits. Servers can require a password, and a public server checks that players' block edits are within reach and at a human rate. Everything uses the Rust standard library, with no accounts and no central server.
- **Mods.** Drop a folder with a `mod.txt` into `mods/` to add blocks, items, tools, food, recipes, textures (pixel art, noise or PNG), ores, plants and simple effects (bouncy or fast blocks, items that heal, launch, explode, give things or spawn mobs). Servers send their mods to players automatically. See **[MODDING.md](MODDING.md)** and `example-mods/cheese`.
- **Code mods (scripting).** Mods can also include sandboxed [Rhai](https://rhai.rs) scripts, which react to events (chat commands, breaking and placing blocks, item use, joins, mob deaths, ticks) and call a game API (blocks, items, health, teleport, explosions, mobs, time, messages). Scripts run on the machine that owns the world, including dedicated servers. See **[SCRIPTING.md](SCRIPTING.md)** and `example-mods/commands`.
- **Multiple worlds.** **Singleplayer** on the title screen opens a world list with each world's name, mode, seed, when it was last played, and size. You can play, create (with a name, game mode and optional seed; any text works as a seed), rename or delete worlds. Each world is its own folder, `saves/<world>/`. Only your edits are stored, and the terrain regenerates from the seed. A save from before world slots (`saves/world.mncr`) is moved in automatically as "My World".
- First-person hand and held item, third-person view, block-breaking cracks, particles, screen shake, a panoramic title screen with splash texts, and options for render distance, FOV, sensitivity and fullscreen.
- **Saved settings.** Render distance, FOV, sensitivity, fullscreen, volume, music, and your multiplayer name and last server are kept in `settings.txt` next to the game. It's plain `key=value` text: edit it by hand if you like, and anything it can't make sense of falls back to the default.

## Controls

| Key | Action |
| --- | --- |
| WASD / arrows | Move |
| Mouse | Look |
| Space | Jump / swim up (double-tap in creative to fly) |
| Shift | Sneak / fly down |
| Ctrl or R | Sprint |
| Left mouse | Mine / attack |
| Right mouse | Place block / eat (when hungry) / light TNT / fire a bow / open a chest, furnace, anvil, enchanting table or door / put on armour |
| Middle mouse | Pick block (creative) |
| 1–9, mouse wheel | Select hotbar slot |
| E or Tab | Inventory and crafting (shift-click a recipe to craft many) |
| T or Enter | Chat, and `/commands` from script mods |
| Q / Ctrl+Q | Throw one of the held item / the whole stack |
| F5 | Toggle third person |
| F3 | Debug info |
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
- `--world FILE`: the world save (default `saves/server.mncr`, created if missing).
- `--seed N` and `--creative`: settings for a new world.
- `--keep-inventory`: players keep their things when they die.
- `--difficulty peaceful|easy|normal|hard`: overrides the world's difficulty.
- `--max-players N`: player limit (default 16).
- `--upnp`: ask the router to forward the port, for servers at home.

Console commands: `list`, `say <text>`, `kick <name>`, `ban <name|ip>`, `unban <ip>`, `bans`, `time <day|night>`, `password <pw|off>`, `save`, `stop`. Bans are by IP address and are kept in `banned-ips.txt`.

The world autosaves every 5 minutes and when you type `stop`. Stopping with Ctrl+C loses anything since the last autosave.

On a cloud server, allow TCP port 25565 in its firewall or security group.

### How it works

- **Who runs what:** the host (or dedicated server) runs the world: mobs, TNT, the time of day and saving. Players send it their edits, movement and attacks, and it passes them on to everyone.
- **Joining:** a joining player gets the world seed plus every block edit so far, so only the changes travel over the network, never whole chunks.
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

Punch a **Tree Chunk** to get logs, then craft **Planks**, then **Sticks**, then a **Wooden Pickaxe**. From there, stone gives you a stone pickaxe, iron gives you an iron pickaxe, and iron is what you need to mine **Dimonds**. Coal plus a stick makes torches, and eight cobblestone make a **Furnace**, which turns raw food into much better food. Crafting works anywhere, so the crafting table is purely decorative.

## Code map

| File | What it does |
| --- | --- |
| `src/main.rs` | Window, screens/menus, input, main loop |
| `src/game.rs` | Gameplay rules, spawning, explosions, scene assembly |
| `src/world.rs` | Chunks, threaded terrain generator, raycasts |
| `src/mesher.rs` | Chunk meshing with AO and smooth lighting |
| `src/render.rs` | Raw OpenGL pipelines and shaders |
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
| `src/weather.rs` | Rain, snow, thunderstorms and lightning |
| `src/players.rs` | Remembering joined players between visits |
| `src/rules.rs` | World rules: keep inventory, difficulty, daylight and weather cycles |
| `src/settings.rs` | `settings.txt` |
| `src/save.rs` | Binary save format |
| `src/net.rs` | Network protocol and non-blocking TCP |
| `src/multiplayer.rs` | Host and client sync logic |
| `src/server.rs` | Headless dedicated server |
| `src/upnp.rs` | Router port forwarding (UPnP) |
| `src/sound.rs` | Sound synthesis and playback |
| `src/noise.rs` | Perlin noise and RNG |
| `src/ui.rs` | HUD and menu widgets |

For headless testing, `minceraft --screenshot out.png --mode title|survival|creative|inventory|night|options|host|join|internet|mods|palette|showcase|worlds|createform|newworld|farm|fish|zoo|kitchen|chest|furnace|building|armour|death|anvil|rules|xp|enchant|table|hut|tower|well|dungeon|ravine|rain|thunder|snow [--frames N] [--time 0..1] [--yaw R] [--pitch R] [--pos x,y,z]` renders a scene and saves a PNG.

Not affiliated with any block-game company.
