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

Run the tests with `cargo test --release`.

## Features

- **Infinite procedural terrain.** Seeded Perlin noise produces oceans, beaches, plains, forests, deserts, snowy biomes, ridged mountains, spaghetti caves, big caverns, ore veins (gold included, for all the good it'll do you), trees, flowers, tall grass, pumpkins, desert Pokey Plants and frozen seas. Chunks generate on background threads.
- **Custom voxel renderer.** It culls hidden faces, adds per-vertex ambient occlusion and smoothed sky lighting, and uses frustum culling, translucent sorted water, cutout leaves and glass, fog, and mipmaps.
- **Compact world storage.** Like Minecraft, each 16-block-tall section of a chunk stores a small palette of the blocks it contains plus packed indices (0 bits per block for all-air or all-stone sections, 4–8 bits for mixed ones, direct ids past 256 kinds). Typical terrain averages under 2 bits per block. F3 shows the live figure.
- **Dynamic lighting.** Torches and Glowrock light the area around them, torchlight is warm, and a held torch lights your way.
- **Day/night cycle** (10 minutes), with a sun, moon, stars, sunrise/sunset glow and scrolling clouds.
- **Physics.** AABB collision, gravity, sprint-jumping, sneaking (it stops you walking off ledges), swimming and fall damage.
- **Survival mode.** Health, natural regen, mining times that depend on your tool, pickaxe tiers that gate ore drops, 48 crafting recipes, food, beds, a bow, farming, fishing, death and respawn.
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
- **More blocks:** Sandstone (under deserts), Stone Bricks, Mossy Cobblestun, Hay Bale, Bookshelf, Lantern, Mushroom (in forests; two make a Suspicious Stew), Scarecrow and Weeds.
- **Advancements.** 40 of them, each with a toast and a fanfare ("Getting Wood", "DIMONDS!", "Don't Blink", "The Cake Is Not a Lie"...). They're saved per world, and the pause menu lists them.
- **TNT.** Light it with a torch (or bare hands), and it chain-reacts.
- **Sound.** Synthesised effects for mining, placing and footsteps (different for stone, wood, grass, sand and glass), plus hurt sounds, oinks, baas, clucks, moos, rattles, skittering, bloops, bow twangs, groans, Hisser hisses, Starer warps, boings, an advancement fanfare, explosions, eating, splashes, item pickups, crafting and menu clicks. Sounds get quieter with distance. A calm procedural tune drifts in now and then. Volume and music are in Options. Run `minceraft --export-sounds <dir>` to write every sound out as a WAV.
- **Multiplayer over LAN or the internet.** Open any world from the pause menu (the game can ask your router to forward the port by itself), or run a headless dedicated server. It syncs blocks, player movement, mobs, TNT, explosions, damage, loot, sounds, time of day and chat. Servers can require a password, and a public server checks that players' block edits are within reach and at a human rate. Everything uses the Rust standard library, with no accounts and no central server.
- **Mods.** Drop a folder with a `mod.txt` into `mods/` to add blocks, items, tools, food, recipes, textures (pixel art, noise or PNG), ores, plants and simple effects (bouncy or fast blocks, items that heal, launch, explode, give things or spawn mobs). Servers send their mods to players automatically. See **[MODDING.md](MODDING.md)** and `example-mods/cheese`.
- **Code mods (scripting).** Mods can also include sandboxed [Rhai](https://rhai.rs) scripts, which react to events (chat commands, breaking and placing blocks, item use, joins, mob deaths, ticks) and call a game API (blocks, items, health, teleport, explosions, mobs, time, messages). Scripts run on the machine that owns the world, including dedicated servers. See **[SCRIPTING.md](SCRIPTING.md)** and `example-mods/commands`.
- **Multiple worlds.** **Singleplayer** on the title screen opens a world list with each world's name, mode, seed, when it was last played, and size. You can play, create (with a name, game mode and optional seed; any text works as a seed), rename or delete worlds. Each world is its own folder, `saves/<world>/`. Only your edits are stored, and the terrain regenerates from the seed. A save from before world slots (`saves/world.mncr`) is moved in automatically as "My World".
- First-person hand and held item, third-person view, block-breaking cracks, particles, screen shake, a panoramic title screen with splash texts, and options for render distance, FOV, sensitivity and fullscreen.

## Controls

| Key | Action |
| --- | --- |
| WASD / arrows | Move |
| Mouse | Look |
| Space | Jump / swim up (double-tap in creative to fly) |
| Shift | Sneak / fly down |
| Ctrl or R | Sprint |
| Left mouse | Mine / attack |
| Right mouse | Place block / eat / light TNT / fire a bow |
| Middle mouse | Pick block (creative) |
| 1–9, mouse wheel | Select hotbar slot |
| E or Tab | Inventory and crafting (shift-click a recipe to craft many) |
| T or Enter | Chat, and `/commands` from script mods |
| Q | Drop (yeet) the held item |
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
  - Creative worlds skip all of this, since everything is free there anyway.
- **Saving and leaving:** only the host or server saves. Players' inventories and positions are not saved on the server (a player who rejoins starts fresh). If the host leaves, everyone returns to the title screen.
- **Addresses:** the host listens on IPv4 and, where available, IPv6. Addresses can be `IP`, `IP:port`, `[IPv6]:port` or a hostname like `play.example.com`.

## Getting started in survival

Punch a **Tree Chunk** to get logs, then craft **Planks**, then **Sticks**, then a **Wooden Pickaxe**. From there, stone gives you a stone pickaxe, iron gives you an iron pickaxe, and iron is what you need to mine **Dimonds**. Coal plus a stick makes torches. Crafting works anywhere, so the crafting table is purely decorative.

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
| `src/save.rs` | Binary save format |
| `src/net.rs` | Network protocol and non-blocking TCP |
| `src/multiplayer.rs` | Host and client sync logic |
| `src/server.rs` | Headless dedicated server |
| `src/upnp.rs` | Router port forwarding (UPnP) |
| `src/sound.rs` | Sound synthesis and playback |
| `src/noise.rs` | Perlin noise and RNG |
| `src/ui.rs` | HUD and menu widgets |

For headless testing, `minceraft --screenshot out.png --mode title|survival|creative|inventory|night|options|host|join|internet|mods|palette|showcase|worlds|createform|newworld [--frames N] [--time 0..1] [--yaw R] [--pitch R] [--pos x,y,z]` renders a scene and saves a PNG.

Not affiliated with any block-game company.
