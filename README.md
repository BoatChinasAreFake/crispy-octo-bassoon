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

- **Infinite procedural terrain.** Seeded Perlin noise produces oceans, beaches, plains, forests, deserts, snowy biomes, ridged mountains, spaghetti caves, big caverns, ore veins, trees, flowers and tall grass. Chunks generate on background threads.
- **Custom voxel renderer.** It culls hidden faces, adds per-vertex ambient occlusion and smoothed sky lighting, and uses frustum culling, translucent sorted water, cutout leaves and glass, fog, and mipmaps.
- **Dynamic lighting.** Torches and Glowrock light the area around them, torchlight is warm, and a held torch lights your way.
- **Day/night cycle** (10 minutes), with a sun, moon, stars, sunrise/sunset glow and scrolling clouds.
- **Physics.** AABB collision, gravity, sprint-jumping, sneaking (it stops you walking off ledges), swimming and fall damage.
- **Survival mode.** Health, natural regen, mining times that depend on your tool, pickaxe tiers that gate ore drops, 17 crafting recipes, food, death and respawn.
- **Creative mode.** Flight, instant breaking, infinite blocks, pick-block and a full item palette.
- **Mobs** (legally distinct):
  - **Oinker**: wanders around, runs when hit, drops Raw Oinkchop.
  - **Hisser**: sneaks up, flashes, swells and explodes. Drops Hisspowder.
  - **Groaner**: chases you at night, bites, and burns in sunlight. Drops Groaner Goo.
- **TNT.** Light it with a torch (or bare hands), and it chain-reacts.
- **Sound.** Synthesised effects for mining, placing and footsteps (different for stone, wood, grass, sand and glass), plus hurt sounds, oinks, groans, Hisser hisses, explosions, eating, splashes, item pickups, crafting and menu clicks. Sounds get quieter with distance. A calm procedural tune drifts in now and then. Volume and music are in Options. Run `minceraft --export-sounds <dir>` to write every sound out as a WAV.
- **Multiplayer over LAN or the internet.** Open any world from the pause menu (the game can ask your router to forward the port by itself), or run a headless dedicated server. It syncs blocks, player movement, mobs, TNT, explosions, damage, loot, sounds, time of day and chat. Servers can require a password, and a public server checks that players' block edits are within reach and at a human rate. Everything uses the Rust standard library, with no accounts and no central server.
- **Mods.** Drop a folder with a `mod.txt` into `mods/` to add blocks, items, tools, food, recipes, textures (pixel art, noise or PNG), ores, plants and simple effects (bouncy or fast blocks, items that heal, launch, explode, give things or spawn mobs). Servers send their mods to players automatically. See **[MODDING.md](MODDING.md)** and `example-mods/cheese`.
- **Code mods (scripting).** Mods can also include sandboxed [Rhai](https://rhai.rs) scripts, which react to events (chat commands, breaking and placing blocks, item use, joins, mob deaths, ticks) and call a game API (blocks, items, health, teleport, explosions, mobs, time, messages). Scripts run on the machine that owns the world, including dedicated servers. See **[SCRIPTING.md](SCRIPTING.md)** and `example-mods/commands`.
- **Saving and loading.** Only your edits are stored (`saves/world.mncr`); the terrain regenerates from the seed.
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
| Right mouse | Place block / eat / light TNT |
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

Console commands: `list`, `say <text>`, `kick <name>`, `time <day|night>`, `password <pw|off>`, `save`, `stop`.

The world autosaves every 5 minutes and when you type `stop`. Stopping with Ctrl+C loses anything since the last autosave.

On a cloud server, allow TCP port 25565 in its firewall or security group.

### How it works

- **Who runs what:** the host (or dedicated server) runs the world: mobs, TNT, the time of day and saving. Players send it their edits, movement and attacks, and it passes them on to everyone.
- **Joining:** a joining player gets the world seed plus every block edit so far, so only the changes travel over the network, never whole chunks.
- **Passwords:** checked with a challenge-response using SHA-256, so the password itself is never sent over the network.
- **Everything else is unencrypted:** chat and game traffic can be read by anyone on the path, so don't share secrets in chat.
- **Anti-grief:**
  - a player's block edits must be within reach of where the server thinks they are, and at a human rate;
  - rejected edits are undone on their screen;
  - connections that go silent for 30 seconds, or don't finish logging in within 15 seconds, are dropped.
- **Saving and leaving:** only the host or server saves. Players' inventories and positions are not saved on the server. If the host leaves, everyone returns to the title screen.
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

For headless testing, `minceraft --screenshot out.png --mode title|survival|creative|inventory|night|options|host|join|internet|mods|palette|showcase [--frames N] [--time 0..1] [--yaw R] [--pitch R] [--pos x,y,z]` renders a scene and saves a PNG.

Not affiliated with any block-game company.
