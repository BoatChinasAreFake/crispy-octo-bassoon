# MINCERAFT

A native, compiled block-building parody written in Rust, with no browser, no web view and no JavaScript. It opens a real OpenGL window through [macroquad](https://github.com/not-fl3/macroquad)/miniquad, and the voxel renderer talks to the GPU directly with its own shaders and vertex buffers.

The game ships with **no image files**. At startup it builds the whole texture atlas (grass, ores, mobs, tools, hearts, the logo) from noise and tiny ASCII sprites.

## Build and run

You need a Rust toolchain ([rustup.rs](https://rustup.rs)).

```sh
cargo run --release
```

Linux also needs the X11/GL development libraries, for example on Debian/Ubuntu:
`sudo apt install libx11-dev libxi-dev libgl1-mesa-dev`.
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
| Q | Drop (yeet) the held item |
| F5 | Toggle third person |
| F3 | Debug info |
| F11 | Fullscreen |
| Esc | Pause menu |

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
| `src/block.rs` | Blocks, items, tools, recipes |
| `src/inventory.rs` | Inventory and crafting |
| `src/save.rs` | Binary save format |
| `src/noise.rs` | Perlin noise and RNG |
| `src/ui.rs` | HUD and menu widgets |

For headless testing, `minceraft --screenshot out.png --mode title|survival|creative|inventory|night [--frames N] [--time 0..1] [--yaw R] [--pitch R] [--pos x,y,z]` renders a scene and saves a PNG.

Not affiliated with any block-game company.
