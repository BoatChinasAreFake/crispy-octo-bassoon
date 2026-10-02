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

Defines a new mob *type*. It borrows one of a few base-game body shapes and wears your texture. By default it's a passive wanderer; mark it `hostile` and give it `attack_damage` (for melee), `ranged_damage`, and/or a `projectile_effect` (for projectiles) and it will pursue and attack players just like a base-game monster. It never spawns naturally; spawn it from a script with `spawn_mob("<name>", x, y, z)`, from a block's `on_break`/`on_use` with the `spawn <name>` action, or with cheats. The host owns every mob and syncs them to joined players, just like blocks and items; attacks are resolved host-side so multiplayer stays authoritative and deterministic.

| Key | Meaning | Default |
| --- | --- | --- |
| `name` | Display name | the section name |
| `texture` | Painted over the whole body | white |
| `template` / `model` | Body shape: `quadruped`, `biped`, `blob` or `bird` (ignored when it has `part` lines) | `quadruped` |
| `part` | One box of a custom body (repeat for each, up to 16); see **Custom shapes** below | the template |
| `size` | Body width and height, in blocks. A template body is drawn at this height (keeping its shape) | `0.9` |
| `width`, `height` | Override `size` for one dimension (width 0.2–8, height 0.2–8) | from `size` |
| `health` | Hit points (1–1000) | `10` |
| `speed` | Walking-speed multiplier (0.1–4) | `1` |
| `hostile` | Spawns at night and counts toward the monster cap. With `attack_damage > 0` it also pursues and melee-attacks players; with `attack_damage = 0` it only counts toward the cap (unchanged from before) | `false` |
| `attack_damage` | Melee damage per hit (0–50). `0` means it never attacks, even when `hostile` | `0` |
| `attack_reach` | Horizontal distance within which it can land a hit (0.5–4) | `1.3` |
| `aggro_range` | How far it notices and chases a player (1–48) | `16` |
| `attack_cooldown` | Seconds between hits (0.25–10; the 0.25 floor prevents a zero-cooldown exploit) | `1` |
| `ranged_damage` | Ranged/projectile damage per hit (0–30). `0` still fires when `projectile_effect` is set | `0` |
| `ranged_range` | How far it will open fire with a projectile (1–48) | `16` |
| `projectile_speed` | How fast the projectile flies, in blocks/second (8–48) | `24` |
| `projectile_model` | Bounded shape: `arrow`, camera-facing `billboard`, or `cube` | `arrow` |
| `projectile_texture` | Optional local, qualified, or base-game texture for the projectile | classic arrow textures |
| `projectile_scale` | Projectile render scale (0.25–4) | `1` |
| `projectile_effect` | Timed effect on a confirmed player hit: `none`, `speed`, `fire_resistance`, `night_vision`, `leaping`, `strength`, or `regeneration` | `none` |
| `projectile_effect_duration` | Effect duration in seconds (0.5–300; used when an effect is set) | `10` |
| `projectile_effect_amplifier` | Effect amplifier (0–3; `0` is level I) | `0` |
| `projectile_count` | Projectiles per volley (1–5) | `1` |
| `projectile_spread` | Total horizontal fan angle in degrees (0–45) | `0` |
| `ranged_cooldown` | Seconds between volleys (0.5–10; the 0.5 floor prevents a projectile-spam exploit) | `2` |
| `projectile_homing` | How fast its projectiles turn toward the nearest player, in degrees per second (0–180). Above `0` they fly straight (no drop), never turn back for a player behind them, and fizzle out after 5 seconds | `0` |
| `projectile_blast` | Blast radius in blocks when a projectile lands (0–4). Everyone inside takes its damage (half at the edge) and its effect; blocks and mobs are never harmed | `0` |
| `drops` | One item it may drop on death: `item [count]` (1–64) | nothing |
| `flying` | Flies instead of walking (no gravity) | `false` |
| `fly_height` | Blocks above the ground it cruises at (1–16) | `4` |
| `fly_speed` | Top flying speed, blocks/second (0.5–10) | `3` |
| `perches` | A wild flier lands to rest now and then, then takes off again | `false` |
| `tame_item` | Right-click it with this to (maybe) tame it. `none`: can't be tamed | `none` |
| `tame_chance` | Chance each `tame_item` works (0.01–1) | `0.33` |
| `trade` | One trade (repeat for each, up to 8): `item [n] [+ item [n]] -> item [n]`; see **Trading** below | none |
| `boss` | A boss: a health bar across the top of the screen, never tamed or despawned, announced when beaten | `false` |
| `enrage_at` | A boss enrages below this fraction of its health (0–1; `0` never) | `0.5` |
| `enrage_speed` | Enraged, it moves this many times faster (1–3) | `1.5` |
| `enrage_cooldown` | Enraged, its cooldowns are multiplied by this (0.25–1) | `0.6` |
| `summon` | What an enraged boss calls for help: `mob [count]` (count 1–4; at most 8 of them within 64 blocks) | nothing |
| `summon_every` | Seconds between calls for help (5–120) | `20` |
| `knockback_resist` | How much it shrugs off knockback (0–1; 1 is immovable) | `0.7` for bosses, else `0` |
| `xp` | Experience it drops (0–1000) | the usual few points |
| `breed_item` | Feed two of them this to breed them; wild ones follow anyone holding it (or the `tame_item`). `none`: doesn't breed | `none` |

A mob melee-attacks only when it is `hostile` **and** `attack_damage > 0`. It fires projectiles when it is `hostile` and either `ranged_damage > 0` or `projectile_effect` is set, so an effect-only volley with zero damage is supported. A ranged mob fires only while the player is within `ranged_range` and has a clear line of sight. If it has both melee and ranged attacks, melee takes priority inside `attack_reach`; both attacks and the entire volley share one cooldown, so a multishot volley consumes only one `ranged_cooldown`.

Multishot is deterministic and uses no random numbers or wall clock. For `n = 1`, the projectile uses the center aim (`0°`). For `n > 1`, projectile `i` uses `-spread/2 + i*spread/(n-1)` degrees around world Y, in index order. Thus odd counts include the center ray, even counts straddle it symmetrically, and `projectile_spread` is the **total** angle from the first ray to the last. Every projectile keeps the same vertical lob, spawn point, damage, appearance, and optional effect.

The host creates the volley, simulates collision and damage, and applies effects, so combat remains authoritative. Projectile positions, velocities, and bounded model/texture/scale are included in normal arrow snapshots for joined players; timed effects are sent from the host after a confirmed hit. Counts and spreads default to one centered projectile, preserving existing mods and base arrows. `ranged_damage` and `projectile_effect` both default off, so an old `hostile = true` mob with no attack fields still only counts toward the monster cap.

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

[mob slinger]
name = Cheese Slinger
texture = mouse
template = biped
health = 20
speed = 1.0
hostile = true
ranged_damage = 4      // lobs an arrow when you're at range...
ranged_range = 18
projectile_speed = 26
projectile_model = billboard
projectile_texture = cheese_slice
projectile_scale = 0.75
projectile_effect = leaping
projectile_effect_duration = 4
projectile_effect_amplifier = 0
projectile_count = 3
projectile_spread = 18       // total fan: -9°, 0°, +9°
ranged_cooldown = 2.0
attack_damage = 3      // ...and still bites up close (melee wins inside attack_reach)
attack_reach = 1.5
aggro_range = 24
```

**Custom shapes.** Instead of a template, build the body from your own boxes, one `part` line each (up to 16):

```text
part = <name> <x> <y> <z> <width> <height> <depth> [options]
```

Positions and sizes are in blocks, measured from the mob's feet: x is to its right, y is up, and **-z is the way it faces**. Each number can be -4 to 4 (sizes 0.01 to 4). The options, all `key=value` with no spaces:

| Option | Meaning | Default |
| --- | --- | --- |
| `pivot=x,y,z` | The point it turns about (a leg's hip, a wing's shoulder) | the middle of its top face |
| `anim=` | `still`, `walk` (swings forward and back as it walks), `sway` (side to side as it walks: tails, spider legs), `wing` (beats up and down as it moves), `flap` (a bird's wing: beats while flying or falling), `tilt` (a fixed lean), `bob` (bobs up and down as it walks) or `spin` (turns round as it moves) | `still` |
| `amount=` | How much it moves (-3 to 3). A minus sign swings the other way, so give opposite legs opposite signs. For `flap`, use `-1` for a left wing and `1` for a right one. For `tilt`, it's the lean in degrees (-180 to 180) | `1` (`0` for tilt) |
| `texture=` | This part's texture (otherwise the mob's `texture`) | the mob's texture |
| `face=` | A texture just for its front (-z) face: eyes, a mouth | |
| `top=` | A texture just for its top | |

The hitbox still comes from `size`, `width` and `height`, so keep the parts roughly inside it. Mistakes are reported on the Mods screen with their line number, and a broken option is skipped rather than spoiling the whole mob.

```text
[mob cheese_crab]
name = Cheddar Crab
texture = butter
size = 0.6
width = 1.0
part = shell -0.4 0.25 -0.3 0.8 0.3 0.6 anim=bob amount=0.5 top=wheel_top
part = eyes -0.25 0.55 -0.3 0.5 0.12 0.1 face=wheel_top
part = claw_l -0.75 0.3 -0.55 0.3 0.2 0.35 pivot=-0.45,0.4,-0.3 anim=sway amount=1.2
part = claw_r 0.45 0.3 -0.55 0.3 0.2 0.35 pivot=0.45,0.4,-0.3 anim=sway amount=-1.2
part = leg_l1 -0.6 0 -0.15 0.25 0.3 0.08 pivot=-0.4,0.3,-0.1 anim=walk amount=1
part = leg_l2 -0.6 0 0.1 0.25 0.3 0.08 pivot=-0.4,0.3,0.14 anim=walk amount=-1
part = leg_r1 0.35 0 -0.15 0.25 0.3 0.08 pivot=0.4,0.3,-0.1 anim=walk amount=-1
part = leg_r2 0.35 0 0.1 0.25 0.3 0.08 pivot=0.4,0.3,0.14 anim=walk amount=1
```

Run `minceraft --screenshot zoo.png --mode modzoo` to see every loaded mod's mobs lined up.

**Homing and blasts.** `projectile_homing` and `projectile_blast` work with all the other projectile keys (count, spread, effects, looks). A homing shot steers toward the nearest player ahead of it, at most `projectile_homing` degrees a second, so you can still dodge a slow-turning one. A blast shot goes off when it hits a player, a block, or runs out of time, and hurts every player within the radius (the full `ranged_damage` at the centre, half at the edge) and gives them its effect. Blasts never break blocks or hurt mobs, so a mod can't use them to grief a world.

**Flying.** A `flying` mob cruises `fly_height` above whatever is under it, rising over hills and trees. A hostile flier with `attack_damage` swoops down to the player's height to bite; one with only ranged attacks keeps its height and shoots from above. Set `perches = true` and a wild one lands for a rest every so often (a tamed one lands when told to sit).

**Taming and breeding.** Give a mob a `tame_item` and right-clicking it with that item has a `tame_chance` of making it yours, Woofer style: it sits straight away; right-click to make it follow you (or sit again). It follows you, teleports to you if left far behind, goes after whatever you hit (if it has `attack_damage`), and is saved with the world. A tamed **hostile** mob never attacks players and isn't cleared by sleeping, Peaceful or golems. With a `breed_item`, feeding two adults makes them fall in love and have a baby; a tamed pair's baby is born tame. Feeding your own hurt pet its `breed_item` heals it. Wild hostile mobs can't be bred until tamed.

```text
[mob cheese_bat]
name = Cheese Bat
texture = mouse
template = bird
size = 0.5
flying = true
fly_height = 5
fly_speed = 4
perches = true
tame_item = cheese_slice
tame_chance = 0.5
breed_item = cheese_slice

[mob stinker]
name = Stinky Seeker
texture = mouse
template = blob
hostile = true
ranged_damage = 3
ranged_range = 20
projectile_model = billboard
projectile_texture = cheese_slice
projectile_homing = 60       // gently curves toward you
projectile_blast = 2.5       // and splashes everyone nearby
ranged_cooldown = 3
```

**Trading.** Give a mob `trade` lines and right-clicking it opens the same trading screen as a Hmmer, with its name as the title:

```text
trade = slice 8 -> gold 1                  // give 8 slices, get a gold ingot
trade = gold 3 + slice 4 -> cheese_pickaxe // two things in, one out
trade = bone for diamond                   // "for" works instead of "->"
```

Items are named as anywhere else in a mod (a bare name means this mod's, then the base game's; the gold ingot is `gold`). Each trade can be made six times, then it restocks the next day. Joined players' trades go through the host, which checks they really have what they're giving. A hostile mob only trades once it's tamed, and holding its `tame_item` or `breed_item` feeds it instead of opening the trades.

**Bosses.** `boss = true` puts the mob's name and health bar across the top of the screen for everyone within 64 blocks, makes it shrug off most knockback, stops it from being tamed or despawning, and tells everyone when it's beaten. Below `enrage_at` of its health it enrages: it moves `enrage_speed` times faster, its attacks come `enrage_cooldown` times as often apart, and every `summon_every` seconds it calls `summon` mobs to its side (never more than eight of them within 64 blocks). Give it a big `xp` and `drops` for a proper reward. Like any modded mob it never spawns by itself: put it somewhere with a script (`spawn_mob`) or a block's `spawn` action.

```text
[mob big_cheese]
name = The Big Cheese
texture = wheel_top
template = biped
width = 1.4
height = 2.8
health = 200
hostile = true
attack_damage = 7
attack_reach = 2.2
aggro_range = 28
boss = true
summon = angry_cheese 2
xp = 200
drops = slice 32
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

Data-defined mobs cover their own stats, a drop, a templated look, hostile melee combat, hostile ranged combat with bounded appearance, timed effects, deterministic multishot, homing and blast projectiles, flying, taming and breeding, custom box-built bodies with simple animations, trading, and bosses. Arbitrary projectile geometry/rendering and custom UI screens still require a Rust change.

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
