# Scripting (code mods)

Mods can include code: any `.rhai` file in a mod folder is a **script**. Scripts are written in [Rhai](https://rhai.rs/book/), a small language that looks like a mix of Rust and JavaScript. They react to things happening in the game and change the world through the API below.

```text
mods/
  commands/
    mod.txt       <- at least a [mod] section (can also define blocks, items...)
    main.rhai     <- your code (all .rhai files in the folder are loaded)
```

A complete example is `example-mods/commands`. It adds chat commands (`/home`, `/sethome`, `/tower`, `/kit`, `/spawn`...), a welcome message, a lucky-Dimond chance when mining stone, a kill counter, and a nightfall announcement. To try it, copy it into `mods/`.

## Where scripts run

- **Scripts run where the world lives:** in single player on your computer; in multiplayer on the host, or on the dedicated server.
- **Joining players never run scripts.** They aren't even sent the script files. What a script does reaches them through normal gameplay: block changes, chat, items, teleports.
- **Scripts start when a world opens.** Loading happens with a new world, a loaded save, or the server starting, and `on_load` runs then. Changes to `.rhai` files take effect the next time a world opens.

## Events

Define any of these functions and the game calls them. Returning `false` from the events marked "cancellable" stops the normal behaviour. Returning nothing or `true` lets it happen.

| Function | When | Cancellable |
| --- | --- | --- |
| `on_load()` | The world opened | |
| `on_tick(dt)` | 20 times a second (`dt` = 0.05) | |
| `on_chat(player, text)` | Someone sent a chat line. Lines starting with `/` are commands; return `false` once you've handled one | yes (hides the message) |
| `on_block_break(player, x, y, z, block)` | A player broke a block | yes |
| `on_block_place(player, x, y, z, block)` | A player placed a block | yes |
| `on_use_item(player, item)` | Right-click with an item | yes, for the host's own player; for joined players the event still runs but their game has already done the normal thing |
| `on_player_join(player)` / `on_player_leave(player)` | Multiplayer joins and leaves | |
| `on_mob_death(kind, x, y, z, killer)` | A mob died (`kind` is `oinker`, `hisser`, `groaner`, `fluffer`, `starer`, `cluckster`, `mooer`, `rattler`, `webber`, `bloop`, `woofer`, `hmmer` or `grumbler`; `killer` is a player name or `""`) | |
| `on_advancement(player, key)` | The world's player earned an advancement (`key` is like `"getting_wood"` or `"dimonds"`). Advancements belong to the world's own player, so this runs in single player and for the host | |

- **Names:** blocks and items are passed as names: `"stone"`, `"glass"`, `"cheese:wheel"`.
- **Players** are passed by name. On a dedicated server, console commands arrive as chat from `"Server"`.
- **Not reported:** blocks destroyed by explosions don't trigger `on_block_break`.

## Game API

**Reading the world** (answered immediately):

| Function | Returns |
| --- | --- |
| `get_block(x, y, z)` | Block name at a position (`"air"` if unloaded) |
| `surface_y(x, z)` | Height of the top solid block |
| `block_exists(name)` | Whether a block or item by that name exists (including mod ones) |
| `players()` | Array of player names |
| `player_pos(name)` | `[x, y, z]` (feet), or `()` if unknown |
| `player_health(name)`, `player_food(name)` | 0–20, or `-1` for nobody by that name. For joined players it's their latest report to the host (every 5 seconds), so it can lag slightly |
| `held_item(name)` | Item key they're holding (`"air"` for bare hands), or `""` for nobody by that name. For joined players, only something the host knows they own |
| `count_item(name, item)` | How many they have. For joined players this is the host's own tally, so it can't be faked |
| `player_items(name)` | Everything they have, as an array of `[item, count]` pairs |
| `time()`, `is_night()` | Time of day 0–1 (0 = sunrise, 0.25 = noon, 0.5 = sunset) |
| `mobs_near(x, y, z, radius)` | Number of mobs within the radius |
| `random()`, `random_int(lo, hi)` | Random numbers |
| `get_var(key)`, `set_var(key, value)` | Your mod's memory, **saved with the world** (see below). `get_var` returns `()` for a key that was never set |

**Changing the world** (applied right after your event function returns):

| Function | Effect |
| --- | --- |
| `set_block(x, y, z, name)` | Place a block (`"air"` removes one) |
| `fill(x1, y1, z1, x2, y2, z2, name)` | Fill a box, up to 32,768 blocks; returns the count |
| `message(player, text)` | A line only that player sees |
| `broadcast(text)` | A line everyone sees |
| `give(player, item[, n])`, `take(player, item, n)` | Add or remove items |
| `heal(player, n)`, `damage(player, n)` | Health (20 = full) |
| `teleport(player, x, y, z)`, `launch(player, speed)` | Move a player |
| `explode(x, y, z, radius)` | Boom (radius up to 8) |
| `spawn_mob(kind, x, y, z)` | Spawn an `oinker`, `hisser`, `groaner`, `fluffer`, `starer`, `cluckster`, `mooer`, `rattler`, `webber`, `bloop`, `woofer`, `hmmer` or `grumbler` |
| `set_time(t)` | Set the time of day (0–1) for everyone |
| `play_sound(name, x, y, z)` | `explode hiss oink groan pop click splash craft hurt eat break glass baa warp fanfare boing cluck moo rattle skitter bloop twang thunk woof snip hmm` |
| `log(text)` / `print(text)` | Debug output: chat in single player, the console on a server |

- **Timing:** changes are queued, so `get_block` right after `set_block` in the same event still sees the old block.
- **Errors throw changes away:** if an event hits an error, none of its queued changes are applied, so a crash halfway through can't leave half a building behind.

## Saved variables

- **What's saved:** everything a mod stores with `set_var` is written into the world's save file and restored before `on_load` the next time the world opens. That makes homes, kill counts, "already claimed" flags, quest progress and so on permanent.
- **When it's saved:** whenever the world is:
  - in single player: **Save World**, or quitting;
  - on a multiplayer host: the host saving;
  - on a dedicated server: every 5 minutes, and on `stop`.
- **What can be saved:** `()`, true/false, whole numbers, decimals, text, single characters, blobs, arrays and maps, nested inside each other up to 32 levels, and up to 8 MB per mod. Named function pointers (`Fn("on_tick")`) are also saved: only the name is stored, and it is rebound to the matching function when the world loads.
- **What can't:** closures and curried function pointers (`|x| x + 1`, `Fn("add").curry(10)`). They capture live runtime state that can't be reconstructed on load, so variables holding them are skipped with a chat message rather than failing the save.
- **Mods switched off:** variables belong to the mod's folder name. If a mod is switched off or missing when a world loads, its saved variables are kept and written back untouched, so turning a mod off for a while doesn't lose its data. Renaming a mod's folder does start it fresh.
- **Starting over:** to reset a mod's data, overwrite each variable, for example with `set_var("key", ())`.

```rust
fn on_chat(player, text) {
    if text == "/sethome" {
        set_var(`home_${player}`, player_pos(player));   // survives restarts
        return false;
    }
}
```

## Rhai in 30 seconds

```rust
let x = 5;                 // variables
let name = `Hi ${player}`; // string interpolation (backticks)
let parts = text.split(" ");
if parts.len() > 1 && parts[1] == "big" { ... } else { ... }
for i in 0..10 { set_block(x, y + i, z, "glass"); }
switch cmd { "/a" => { ... }, "/b" => { ... }, _ => { } }
let home = get_var("home");
if home == () { /* not set yet */ }
fn helper(a, b) { a + b }  // your own functions
```

- **Numbers:** they're integers or floats. The game API accepts either.
- **Converting text:** use `parse_int("12")` and `parse_float("1.5")`.
- **Full language guide:** <https://rhai.rs/book/>.

## Safety

Scripts are sandboxed, so running someone else's mod on your server can't harm your computer:

- **No outside access:** no files, network, programs or other OS features. `eval` and `import` are disabled.
- **Bounded work:** each event is capped at 500,000 operations, so an endless `loop {}` is stopped in milliseconds rather than freezing the game. Recursion depth and string, array and map sizes are capped as well.
- **One event can't flood the world:** it can queue at most 20,000 world changes.
- **Broken scripts turn themselves off:** a mod whose scripts hit 25 errors is switched off until the world is reopened.

Errors appear in chat, in the console, and on the Mods screen (errors when loading, with line numbers).

## Limitations

- **Joined players:** scripts can read a joined player's position, health, food, held item and inventory (read-only; to change them use `give`, `take`, `heal` and the other commands).
- **New mob types:** a mod can now define new mob *types* with a `[mob <name>]` section in its `mod.txt` (name, texture, size, health, speed, a drop, and one of a few base body shapes). Because a code mod is a single folder that may ship both `mod.txt` and `.rhai` files, a script can spawn its own data-defined mobs with `spawn_mob("mymob", x, y, z)` (or the mod `spawn <name>` action), and `on_mob_death` reports the mob's name. See the `[mob]` section in [MODDING.md](MODDING.md). These mobs are host-authoritative and sync to joined players like blocks and items. A mob is a passive wanderer by default, but a `hostile` mob can pursue and melee-attack via `attack_damage`, `attack_reach`, `aggro_range`, and `attack_cooldown`. It can also fire host-resolved projectiles configured with bounded damage, range, speed, arrow/billboard/cube appearance, a safe timed effect (including effect-only shots), and a deterministic one-to-five-shot fan. Projectile state and appearance replicate to joined players; collision, damage, and effects remain host-authoritative. Projectiles can also home in (`projectile_homing`) or burst over an area (`projectile_blast`), and mobs can fly (`flying`, `fly_height`, `fly_speed`, `perches`) and be tamed and bred (`tame_item`, `tame_chance`, `breed_item`). Arbitrary projectile geometry/rendering, bosses and trading still require Rust code.
- **Custom UI screens and bespoke rendering still need a Rust change.** Modded mobs reuse an existing body template painted with the mod's texture; arbitrary per-mob geometry is not exposed. Game screens (menus, inventories, the furnace UI, and so on) are Rust state machines in `ui.rs`/`main.rs` with no scripting surface, so a mod cannot add or replace a screen. These remain out of scope.
