# Modding Minceraft

A mod is a **folder inside `mods/`** that contains a text file called `mod.txt`, plus any PNG textures it uses. The `mods/` folder goes next to where you run the game (for `cargo run`, the repository root).

`mod.txt` is data: it describes content. A multiplayer server automatically **sends it to everyone who joins**, so players don't install anything themselves.

For behaviour that data can't express (chat commands, events, custom game rules), add **scripts**: `.rhai` files in the same folder. See **[SCRIPTING.md](SCRIPTING.md)**. Scripts are sandboxed and only run on the machine that owns the world.

To try one:

1. Copy `example-mods/cheese` into `mods/`, so you have `mods/cheese/mod.txt`.
2. Start the game, click **Mods** on the title screen, and check that the Cheese Mod is **On**.
3. Start a creative world. The cheese blocks and items are at the end of the palette (press **E**).

After editing a mod, press **Reload Mods** on the Mods screen. Mistakes show there with their line numbers, for example `line 12: unknown texture "chese"`, and the rest of the mod still loads.

---

## The file format

```text
// Comments start with // (or ;)
[section name]
key = value
```

Sections: `[mod]`, `[block <name>]`, `[item <name>]`, `[texture <name>]`, `[recipe]`, `[ore <name>]`, `[plant <name>]`, `[splashes]`.

Names are lowercase words like `cheese_ore`.

**Referring to things:**
- *Your own things:* use their name (`slice`).
- *Base-game things:* use their name (`stone`, `stick`, `diamond`).
- *Another mod's things:* use `modfolder:name` (`cheese:slice`).
- *Name clashes:* if your mod and the base game share a name, your mod's wins.

### `[mod]`

```text
[mod]
name = Cheese Mod
version = 1.0
author = You
description = One line shown on the Mods screen.
```

### `[block <name>]`

| Key | Meaning | Default |
| --- | --- | --- |
| `name` | Display name | the section name |
| `texture` | Texture for all faces | white |
| `top`, `side`, `bottom` | Per-face textures (override `texture`) | |
| `model` | `cube`, or `cross` (plants and torches) | `cube` |
| `solid` | Players and mobs collide with it | `true` for cubes |
| `transparent` | Glass-like: light passes through; hides faces touching the same block | `false` |
| `cutout` | Leaf-like: has see-through holes but still blocks light | `false` |
| `hardness` | Seconds to break by hand, or `unbreakable` | `1` |
| `tool` | `pickaxe` makes pickaxes speed it up (and hands slow) | hand |
| `tier` | Pickaxe tier needed for a drop: 1 wood, 2 stone, 3 iron, 4 dimond | `1` with a pickaxe tool, else `0` |
| `drops` | `self`, `none`, or an item/block name | `self` |
| `light` | Glow radius in blocks (torch = 8, Glowrock = 11) | `0` |
| `sound` | `stone`, `wood`, `grass`, `sand` or `glass` | `stone` (`grass` for cross) |
| `bounce` | Bounciness 0–1.5; landing sends you back up (sneak to land softly) | `0` |
| `speed` | Walking speed multiplier on top of it (0.2–3) | `1` |
| `on_break` | Actions when a player breaks it (see below) | |
| `creative` | Show it in the creative palette | `true` |

### `[item <name>]`

| Key | Meaning | Default |
| --- | --- | --- |
| `name` | Display name | the section name |
| `texture` | Icon and in-hand sprite | white |
| `stack` | Max stack size (1–64) | `64` |
| `food` | Health restored when eaten (right-click) | not edible |
| `pickaxe` | Works as a pickaxe of this tier (1–4) | `0` |
| `damage` | Attack damage (hand = 1, dimond sword = 7) | `1` |
| `on_use` | Actions on right-click (see below) | |
| `consume` | Using it uses one up | `true` |

### Actions (`on_use`, `on_break`)

Separate several actions with `;`, for example `on_use = launch 18; message Wheee!; give slice 2`.

| Action | Effect |
| --- | --- |
| `heal 4` | Restore health (20 = full) |
| `explode 3` | Explosion of radius 0.5 to 6 |
| `launch 15` | Shoot the player upward (negative numbers push down) |
| `message Some text` | Show a chat message |
| `give slice 2` | Give items |
| `time day` / `night` / `noon` / `midnight` / `0.0`–`1.0` | Set the time (host only) |
| `spawn oinker` / `hisser` / `groaner` / `fluffer` / `starer` | Spawn a mob (host only). The names they parody (`pig`, `creeper`, `zombie`, `sheep`, `enderman`) work too |

### `[texture <name>]`

Every texture is 16×16. There are four ways to make one, and some can be combined:

**Pixel art.** Write 16 rows of 16 characters, and define each character with a `color` line. `.` means transparent.

```text
[texture ruby]
color # = 200,20,40
color o = 255,120,140      // or #ff788c, or 255,120,140,128 with alpha
................
......####......
....##oo####....
   ... (16 rows in total) ...
```

**Noise.** A speckled texture from a single colour:

```text
[texture butter]
noise = #fff2a0
variation = 0.06
```

**Paint over another texture.** Start from `base`, then add pixel rows. Here `.` means "keep the base pixel", which is ideal for ores:

```text
[texture ruby_ore]
base = stone
color # = 200,20,40
................
...##...........
   ...
```

**A PNG file** in the mod folder, any size (it's scaled to 16×16). Either reference it directly as `texture = ruby.png`, or declare it with `[texture ruby]` and `file = ruby.png`.

Base-game texture names you can use anywhere a texture is expected:
`grass_top grass_side dirt stone cobblestone sand gravel water log_side log_top leaves planks glass bedrock coal_ore iron_ore diamond_ore snow snow_side bricks tnt_side tnt_top tnt_bottom crafting_table_top crafting_table_side glowrock torch flower tall_grass stick coal iron diamond gunpowder porkchop goo wooden_pickaxe stone_pickaxe iron_pickaxe diamond_pickaxe wooden_sword stone_sword iron_sword diamond_sword white gold_ore pumpkin_top pumpkin_side jack_o_lantern cactus_top cactus_side ice bouncy_goo bed_top bed_side cake_top cake_side sponge wool gold golden_oinkchop stare_pearl mutton`.

### `[recipe]`

Crafting is shapeless, as in the base game. You can have up to 6 inputs:

```text
[recipe]
inputs = wheel 3, stick 2
output = cheese_pickaxe 1
```

### `[ore <name>]` and `[plant <name>]`

These add your blocks to newly generated terrain:

```text
[ore ruby_veins]
block = ruby_ore
replace = stone     // which block it replaces (default stone)
min_y = 5
max_y = 40
chance = 0.004      // roughly this fraction of eligible blocks (max 0.05)

[plant roses]
block = rose        // usually a model = cross block
on = grass          // what it grows on (default grass)
chance = 0.01       // per surface column (max 0.2)
```

Terrain that was already generated doesn't change. Explore new areas, or start a new world.

### `[splashes]`

Each line is an extra title-screen splash text.

---

## Base-game names

- **Blocks:** `grass dirt stone cobblestone sand gravel water log leaves planks glass bedrock coal_ore iron_ore diamond_ore snowy_grass bricks tnt crafting_table glowrock torch flower tall_grass`
- **Items:** `stick coal iron diamond gunpowder porkchop goo wooden_pickaxe stone_pickaxe iron_pickaxe diamond_pickaxe wooden_sword stone_sword iron_sword diamond_sword`

## Limits

- Up to **66 mod blocks**, **134 mod items** and **160 mod textures**, across all mods together.
- 1 MB per file, and 4 MB for all of a server's mods together.

## Worlds and multiplayer

- **Saves and changing mods:** saves store mod blocks and items by name. Adding, removing or reordering mods doesn't scramble a world. Blocks from a mod you removed turn into air, and its items disappear.
- **Joining a server:** the server's mods replace yours while you're connected, and yours come back when you leave. Servers only send `.txt` and `.png` files.
- **Dedicated servers:** they load mods from the `mods/` folder where the server runs, and list them in the console at startup.
- **Turning mods off:** the On/Off buttons on the Mods screen write `mods/disabled.txt`. You can edit that file yourself: one mod folder name per line.
