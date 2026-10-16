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
| `on_timer(name)` | A timer started with `timer_start` ran out (see Minigames) | |
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

**Dimensions and worn armour**:

| Function | Returns |
| --- | --- |
| `dimension()` | Stable key of the currently active realm |
| `player_dimension(player)` | `overworld`, `scorchlands`, `hollow` or a qualified mod dimension key; empty for an unknown player |
| `wearing(player, item)` | Whether the item is in a worn armour slot; joined reports also require host-ledger ownership and can lag five seconds |
| `get_block_in(dim, x, y, z)` | Block name in that realm; air if the chunk is unloaded |
| `chunk_loaded_in(dim, x, z)` | Whether its chunk is present |
| `surface_y_in(dim, x, z)` | Loaded surface including edits, or the generator's ground estimate for an unloaded column |
| `mobs_near_in(dim, x, y, z, radius)` | Realm-local count, radius bounded to 128 |
| `mob_count_in(dim)` | Total mobs in the active or parked realm |
| `locate_structure_in(dim, qualified_name, x, z)` | `[x,y,z]` of a nearby template structure, or `()`; survey bounded to 96 chunks |

Use fully qualified mod dimension keys. Existing coordinate-only APIs act in the current realm, which may be a joined player's realm during their event. Delayed actions should store the realm key and use the explicit `*_in` APIs. Check `chunk_loaded_in` before treating an unloaded `get_block_in` result as a missing machine. `surface_y_in` outside loaded chunks is an estimate, not a scan of saved edits.

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

**Dimension changes** (queued like other world changes):

| Function | Effect |
| --- | --- |
| `set_block_in(dim, x, y, z, block)` | Realm-specific block edit |
| `explode_in(dim, x, y, z, radius)` | Realm-specific host explosion, bounded to radius 8; affected chunks load first |
| `spawn_mob_in(dim, kind, x, y, z)` | Spawn a base or qualified mod creature only in that realm |
| `travel(player, dim, x, z)` | Load nearby destination columns and move the local or joined player above dry solid ground, using the normal dimension snapshots |

`travel` checks the requested column and eight nearby columns. If none supports a safe landing below the build ceiling, it leaves the player where they were and sends a message. It does not consume items itself; scripts choose their travel costs. Coordinates must be finite and within 30 million blocks. Realm-local explosions preserve other realms at identical coordinates. No script files run on joining clients.

**Minigames** (changes applied right after your event function returns, like the rest): scores, a sidebar everyone sees, timers, teams and spawns. Everything here is saved with the world.

| Function | Effect |
| --- | --- |
| `score_add(objective, player, n)`, `score_set(objective, player, n)` | Change a score. An objective is any name you like (`"dug"`, `"kills"`); a "player" can be any name too (a team, say) |
| `score_get(objective, player)`, `scores(objective)` | A score now (0 if none), or every score of an objective as a map |
| `score_reset(objective)` | Forget an objective's scores |
| `sidebar_show(objective, title)`, `sidebar_hide()` | Show an objective down the right of everyone's screen, highest first (the top 10) |
| `timer_start(name, seconds)`, `timer_stop(name)`, `timer_left(name)` | A timer: when it runs out, `on_timer(name)` runs. Starting it again restarts it |
| `countdown_show(timer, label)`, `countdown_hide()` | Show a timer at the top of everyone's screen ("Round ends in 1:23") |
| `team_create(team, colour)` | Make a team (`red`, `blue`, `green`, `yellow`, `purple`, `orange`, `aqua`, `pink`); its members' names show in its colour |
| `team_join(player, team)`, `team_leave(player)` | Put a player on a team, or on none |
| `team_of(player)`, `team_members(team)` | A player's team (`""` for none), or a team's players |
| `team_friendly_fire(team, on)`, `set_pvp(on)` | Can players hurt each other at all, and can teammates? (Players can also use `/team` and operators `/pvp`) |
| `set_spawn(player, x, y, z)`, `set_world_spawn(x, y, z)` | Where a player comes back when they die, or where everyone new starts |

See `example-mods/digrace` for a whole minigame: two teams, two minutes, the most blocks dug wins.

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
- **New mob types:** a mod can now define new mob *types* with a `[mob <name>]` section in its `mod.txt` (name, texture, size, health, speed, a drop, and one of a few base body shapes). Because a code mod is a single folder that may ship both `mod.txt` and `.rhai` files, a script can spawn its own data-defined mobs with `spawn_mob("mymob", x, y, z)` (or the mod `spawn <name>` action), and `on_mob_death` reports the mob's name. See the `[mob]` section in [MODDING.md](MODDING.md). These mobs are host-authoritative and sync to joined players like blocks and items. A mob is a passive wanderer by default, but a `hostile` mob can pursue and melee-attack via `attack_damage`, `attack_reach`, `aggro_range`, and `attack_cooldown`. It can also fire host-resolved projectiles configured with bounded damage, range, speed, arrow/billboard/cube appearance, a safe timed effect (including effect-only shots), and a deterministic one-to-five-shot fan. Projectile state and appearance replicate to joined players; collision, damage, and effects remain host-authoritative. Projectiles can also home in (`projectile_homing`) or burst over an area (`projectile_blast`), and mobs can fly (`flying`, `fly_height`, `fly_speed`, `perches`) and be tamed and bred (`tame_item`, `tame_chance`, `breed_item`). Mobs can also have custom bodies built from up to 16 animated boxes (`part` lines). They can trade (`trade` lines, on the Hmmer trading screen) and be bosses (`boss`, with a health bar, an enraged phase and summons). Arbitrary projectile geometry/rendering and custom UI screens still require Rust code.
- **Custom UI screens and bespoke rendering still need a Rust change.** Modded mobs reuse an existing body template painted with the mod's texture; arbitrary per-mob geometry is not exposed. Game screens (menus, inventories, the furnace UI, and so on) are Rust state machines in `ui.rs`/`main.rs` with no scripting surface, so a mod cannot add or replace a screen. These remain out of scope.

### Autopilot rocket scenes

`rocket_depart(player, x, y, z, tier, remaining)` attaches a pilot to a four-second ascent from the engine origin. `rocket_land(player, tier, remaining)` starts or resumes a three-second descent above the player’s current safe landing column. `rocket_clear(player)` removes that player’s scene from every realm and releases a descending pilot at the saved ground anchor. Tiers are 1–4 and times are clamped to the corresponding phase duration. These commands run only on the host, which sends the moving model, exhaust and cockpit to everyone. A flight that has finished and isn't moved on (or cleared) within ten seconds ends by itself, setting its pilot down at the ground anchor; so does one whose pilot leaves. Repeating a phase updates its pilot association without restarting its animation. Scripts retain travel, resource checks and saved phase records.
