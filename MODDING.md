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
| `shape` | `slab` or `stairs` (they behave like the base game's: slabs stack into `full`, stairs face away from you) | `cube` |
| `full` | For slabs: the block two of them make | none (two slabs stay two slabs) |
| `smelts_into` | What a furnace turns it into | doesn't cook |
| `burns_for` | Seconds it burns as furnace fuel (0.5–600) | not fuel |

### `[item <name>]`

| Key | Meaning | Default |
| --- | --- | --- |
| `name` | Display name | the section name |
| `texture` | Icon and in-hand sprite | white |
| `stack` | Max stack size (1–64) | `64` |
| `food` | Hunger points restored when eaten (right-click; 2 points = one drumstick) | not edible |
| `pickaxe` | Works as a pickaxe of this tier (1–4) | `0` |
| `damage` | Attack damage (hand = 1, dimond sword = 7) | `1` |
| `on_use` | Actions on right-click (see below) | |
| `consume` | Using it uses one up | `true` |
| `durability` | Uses before it breaks (1–16000); makes it a tool that wears out, stacks to 1, and can be enchanted with Unbreaking | lasts forever (armour: `200`) |
| `repair` | What mends it at an anvil (each one restores a quarter) | can't be repaired |
| `armor` | `helmet`, `chestplate`, `leggings` or `boots`: it's worn in that slot | not armour |
| `armor_points` | For armour: points of protection (1–10; each takes 4% off damage) | `2` |
| `looks_like` | For armour: which base-game set it looks like when worn: `wool`, `iron`, `gold` or `diamond` | `iron` |
| `smelts_into`, `burns_for` | As for blocks | |

A pickaxe (`pickaxe` above 0) can be enchanted with Efficiency and Fortune, a `damage` above 1 with `durability` counts as a sword (Sharpness), and armour takes Protection.

For example, a slab of a mod block and a cheese sword:

```
[block cheese_slab]
name = Cheese Slab
texture = cheese
shape = slab
full = cheese_block

[item cheese_sword]
name = Cheese Sword (Mature)
texture = cheese_sword
damage = 5
durability = 300
repair = cheese
```

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
| `spawn oinker` / `hisser` / `groaner` / `fluffer` / `starer` / `cluckster` / `mooer` / `rattler` / `webber` / `bloop` / `woofer` / `hmmer` / `grumbler` | Spawn a mob (host only). The names they parody (`pig`, `creeper`, `zombie`, `sheep`, `enderman`, `chicken`, `cow`, `skeleton`, `spider`, `slime`, `wolf`, `villager`, `zombified_piglin`) work too |

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
`grass_top grass_side dirt stone cobblestone sand gravel water log_side log_top leaves planks glass bedrock coal_ore iron_ore diamond_ore snow snow_side bricks tnt_side tnt_top tnt_bottom crafting_table_top crafting_table_side glowrock torch flower tall_grass stick coal iron diamond gunpowder porkchop goo wooden_pickaxe stone_pickaxe iron_pickaxe diamond_pickaxe wooden_sword stone_sword iron_sword diamond_sword white gold_ore pumpkin_top pumpkin_side jack_o_lantern cactus_top cactus_side ice bouncy_goo bed_top bed_side cake_top cake_side sponge wool gold golden_oinkchop stare_pearl mutton feather cluckets moo_steak bone pointy_stick string bow`.

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

### `[mob <name>]`

Defines a new mob *type*. It borrows one of a few base-game body shapes and wears your texture. By default it's a passive wanderer; mark it `hostile` and give it an `attack_damage` and it will pursue and melee-attack players just like a base-game monster. It never spawns naturally; spawn it from a script with `spawn_mob("<name>", x, y, z)`, from a block's `on_break`/`on_use` with the `spawn <name>` action, or with cheats. The host owns every mob and syncs them to joined players, just like blocks and items; attacks are resolved host-side so multiplayer stays authoritative and deterministic.

| Key | Meaning | Default |
| --- | --- | --- |
| `name` | Display name | the section name |
| `texture` | Painted over the whole body | white |
| `template` / `model` | Body shape: `quadruped`, `biped`, `blob` or `bird` | `quadruped` |
| `size` | Body width and height, in blocks | `0.9` |
| `width`, `height` | Override `size` for one dimension (width 0.2–8, height 0.2–8) | from `size` |
| `health` | Hit points (1–1000) | `10` |
| `speed` | Walking-speed multiplier (0.1–4) | `1` |
| `hostile` | Spawns at night and counts toward the monster cap. With `attack_damage > 0` it also pursues and melee-attacks players; with `attack_damage = 0` it only counts toward the cap (unchanged from before) | `false` |
| `attack_damage` | Melee damage per hit (0–50). `0` means it never attacks, even when `hostile` | `0` |
| `attack_reach` | Horizontal distance within which it can land a hit (0.5–4) | `1.3` |
| `aggro_range` | How far it notices and chases a player (1–48) | `16` |
| `attack_cooldown` | Seconds between hits (0.25–10; the 0.25 floor prevents a zero-cooldown exploit) | `1` |
| `drops` | One item it may drop on death: `item [count]` (1–64) | nothing |

A mob attacks only when it is `hostile` **and** `attack_damage > 0`. Existing mods that set `hostile = true` without the attack fields keep `attack_damage = 0`, so they behave exactly as before (they count toward the monster cap but don't attack).

```text
[mob mouse]
name = Tiny Mouse
texture = mouse        // a [texture] in this mod, or a base-game name
template = quadruped
size = 0.4
health = 6
speed = 1.5
drops = cheese_slice 2

[mob brute]
name = Angry Brute
texture = mouse
template = biped
health = 30
speed = 1.2
hostile = true
attack_damage = 5      // now it actually bites
attack_reach = 1.5
aggro_range = 20
attack_cooldown = 1.0
```

Then, from a `.rhai` script in the same mod folder:

```rust
fn on_chat(player, text) {
    if text == "/mouse" {
        let p = player_pos(player);
        spawn_mob("mouse", p.x, p.y, p.z);   // resolves "mouse" or "mymod:mouse"
        return false;
    }
}
```

If two mods both define a `[mob]` with the same section name, a bare name resolves to the first match; use the full `modfolder:name` key (`mymod:mouse`) to pick the one you mean.

Data-defined mobs cover their own stats, a drop, a templated look, and hostile melee combat (pursue and bite). Behaviours that need code (bosses, taming, trading, flying, ranged/projectile attacks) and bespoke per-mob geometry or custom UI screens are not configurable from data; those still need a Rust change.

### `[splashes]`

Each line is an extra title-screen splash text.

---

## Base-game names

- **Blocks:** `grass dirt stone cobblestone sand gravel water lava log leaves planks glass bedrock coal_ore iron_ore diamond_ore snowy_grass bricks tnt crafting_table glowrock torch flower tall_grass gold_ore pumpkin jack_o_lantern cactus ice bouncy_goo bed cake sponge wool sandstone stone_bricks mossy_cobblestone hay_bale bookshelf lantern mushroom scarecrow weeds farmland farmland_wet wheat_0..wheat_3 carrots_0..carrots_3 potatoes_0..potatoes_3 chest furnace furnace_lit planks_slab cobblestone_slab stone_brick_slab planks_stairs cobblestone_stairs stone_brick_stairs anvil anvil_chipped anvil_damaged glowshroom pointy_rock enchanting_table obsidian zap_ore lever button pressure_plate zap_block zap_lamp scorchrock embersand scorch_gold_ore rail powered_rail sign item_frame sapling fence glass_pane fence_gate ladder trapdoor black_wool red_wool orange_wool yellow_wool green_wool blue_wool purple_wool white_stained_glass black_stained_glass red_stained_glass orange_stained_glass yellow_stained_glass green_stained_glass blue_stained_glass purple_stained_glass ember_shroom brewing_stand zappy_torch repeater piston_north sticky_piston_north dispenser hopper hollow_stone eye_frame wyrm_crystal wyrm_egg spruce_log spruce_leaves jungle_log jungle_leaves mud lily_pad red_sand terracotta orange_terracotta red_terracotta yellow_terracotta dead_bush melon detector_rail comparator beacon` (plus the upside-down and turned variants, joined fences and panes, open gates and trapdoors, facings of pistons, repeaters, comparators, dispensers and hoppers, lit detector rails, the beacon's other effects, door halves, fire and portals, which players don't hold)
- **Items:** `stick coal iron diamond gunpowder porkchop goo wooden_pickaxe stone_pickaxe iron_pickaxe diamond_pickaxe wooden_sword stone_sword iron_sword diamond_sword gold golden_oinkchop stare_pearl mutton feather cluckets moo_steak bone pointy_stick string bow hoe wheat_seeds wheat carrot potato bone_dust compost wood_ash soil_probe bread fishing_rod cod salmon pufferfish tropical_fish big_bob soggy_boot message_bottle fish_and_chips suspicious_stew worm cooked_oinkchop cooked_mutton cooked_cluckets steak cooked_cod cooked_salmon baked_potato cooked_pufferfish cooked_boot door wool_helmet wool_chestplate wool_leggings wool_boots iron_helmet iron_chestplate iron_leggings iron_boots golden_helmet golden_chestplate golden_leggings golden_boots diamond_helmet diamond_chestplate diamond_leggings diamond_boots bucket water_bucket lava_bucket shears zap_dust book enchanted_book shield sparker boat minecart compass map apple white_dye black_dye red_dye orange_dye yellow_dye green_dye blue_dye purple_dye glass_bottle water_bottle potion_of_healing potion_of_speed potion_of_fire_resistance potion_of_night_vision potion_of_leaping splash_potion_of_healing splash_potion_of_speed splash_potion_of_fire_resistance splash_potion_of_night_vision splash_potion_of_leaping grumbler_tusk saddle staring_eye name_tag melon_slice chest_minecart hopper_minecart`

Furnaces cook and burn mod things too: give a mod block or item [`smelts_into`](#block-name) to make it smeltable, and [`burns_for`](#block-name) to make it fuel. Mod items with [`durability`](#item-name) wear out as you use them (and the right `damage` makes one a sword), and those with `armor` (plus `armor_points` and `looks_like`) are worn and soften blows just like base-game gear. All of it mends at an anvil with its [`repair`](#item-name) material. See the `[block …]` and `[item …]` tables above for every key.

## Limits

- Up to **32,000 mod blocks**, **32,000 mod items**, **222 mod mobs** and about **3,800 mod textures**, across all mods together.
- 1 MB per file, and 4 MB for all of a server's mods together.

## Worlds and multiplayer

- **Saves and changing mods:** saves (and each region file, which holds block edits, containers, signs and frames) store mod blocks and items by name. Adding, removing or reordering mods doesn't scramble a world. Blocks from a mod you removed turn into air, and its items disappear. Saved mod mobs (ones you've tamed, bred, fed or named) are stored by name too; one from a mod you removed is dropped on load rather than turning into some other creature.
- **Joining a server:** the server's mods replace yours while you're connected, and yours come back when you leave. Servers only send `.txt` and `.png` files.
- **Dedicated servers:** they load mods from the `mods/` folder where the server runs, and list them in the console at startup.
- **Turning mods off:** the On/Off buttons on the Mods screen write `mods/disabled.txt`. You can edit that file yourself: one mod folder name per line.
