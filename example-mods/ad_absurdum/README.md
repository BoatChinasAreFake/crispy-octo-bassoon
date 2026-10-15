# Ad Absurdum

An original space-tech parody inspired by [Ad Astra](https://github.com/terrarium-earth/Ad-Astra).

25 declared blocks, 24 items, 27 recipes, sixteen original pixel textures, six creatures, five supply structures and **five separate planet dimensions**. There are no planet-biome replacements in the Overworld.

## Prepare a first launch

1. Craft Aerospace Steel from two Iron and one Coal. Make an engine kit, two pressure tanks and two flight circuits. These combine with eight Steel into a Tier 1 Paperwork Rocket.
2. Make a launch pad and a Rocket Assembly Bench. Place them within three blocks of your operating position and clear five blocks above the pad. `/astra assemble 1` consumes the rocket kit and builds a five-block engine/body/nose assembly. `/astra pack` repacks an intact nearby rocket you own.
3. Build a fuel refinery. `/astra refine` beside it consumes two Coal and one empty Tank to make two Fuel Canisters.
4. Build a Breath Printer. `/astra refill` beside it consumes one Battery and one empty Tank to make four Oxygen Canisters. Batteries have a normal recipe; `/astra charge` at a Sun Tax Panel can also produce two from one Steel and one Zap Dust, once per minute per player.
5. Craft and wear at least a spacesuit Helmet and Chestplate. Carry a Life Support Pack and oxygen. Leggings and Boots add armour; hot planets also require a Heat Shield.
6. `/astra launch moon` checks the rocket, pad, nearby bench, suit, oxygen, tier and fuel before starting. A visible rocket carries the seated pilot through four seconds of ascent, then transfers to the separate Moon dimension for a three-second powered descent. The rocket, pad and bench are removed from the origin and returned as items after touchdown, so they can be deployed again without duplicating equipment.

The engine looks for nearby dry solid ground and places the arriving pilot above it. Rocket flight uses a visible, host-controlled autopilot vehicle: the pilot stays attached while it rises and descends, nearby observers see its moving hull, bright engine flame and drifting exhaust, and first-person pilots see a cockpit window frame and instrument panel. F5 switches to the exterior third-person view. The transition between planets remains a dimension change; this is not an orbital simulation. Each player owns their deployed rockets; up to 64 may be deployed and sixteen departures may run concurrently.

## Worlds and progression

| Destination | Dimension key | Minimum rocket | Fuel | Material | Player gravity |
| --- | --- | --- | --- | --- | --- |
| The Mostly Moon | `ad_absurdum:moon` | 1 | 1 | Lunarium | 0.17× |
| Marzipan | `ad_absurdum:mars` | 2 | 2 | Redium | 0.38× |
| Vexing Venus | `ad_absurdum:venus` | 3 | 3 | Vexium | 0.90× |
| Mercurial | `ad_absurdum:mercury` | 3 | 3 | Sunsteel | 0.38× |
| Glaze-io | `ad_absurdum:glacio` | 4 | 4 | Cryosteel | 0.65× |
| Saved Overworld departure | `overworld` | 1 | 1 | — | 1× |

Each planet has distinct ground, hill profile, fixed sky/light, ore, crawling enemy and supply outpost. Mine its ore and smelt the relevant metal. Upgrade a Tier 1 rocket with Lunarium for Tier 2; Redium upgrades it to Tier 3; Sunsteel and Vexium unlock Tier 4. Cryosteel is a late exploration resource. Craft a planet's Beacon Frames from its metal for an optional ordinary portal connection.

There is no free emergency return command: deploy the recovered pad and bench, assemble your recovered rocket and `/astra launch home` with fuel. The latest Overworld departure is saved per player. The exact pilot landing is adjusted to nearby safe ground. Make supplies for both directions before leaving.

## Oxygen, heat and rovers

One oxygen canister powers a worn Helmet/Chestplate and carried Life Support Pack for thirty seconds and returns an empty tank. Missing oxygen or required equipment causes damage every five seconds. Hot planets also damage players without a carried Heat Shield. Armour reports from joined players are host-checked and can lag up to five seconds.

`/astra rover` consumes a Flat-Pack Rover and spawns a custom wheeled supply companion. Tame it with a Flight Circuit; it follows you and trades Batteries for oxygen/fuel through the regular trade screen. The rover is not a rideable vehicle and has no cargo-inventory GUI.

Planet wildlife is host-spawned near visiting players with local/global limits. Gravity changes player falling and jumping behaviour; gliding and mob gravity retain their existing physics. Oxygen is a per-player buffer, not a sealed-room atmosphere simulation.

`/astra help`, `/astra refine`, `/astra refill`, `/astra charge`, `/astra assemble 1|2|3|4`, `/astra pack`, `/astra launch moon|mars|venus|mercury|glacio|home`, `/astra rover`, `/astra status`.

Rockets, pending flights, departure points, oxygen buffers, solar cooldowns and exploration scores persist with the world. Disconnected pilots' departures pause until they return. Each planet has its own saved realm. Crafting a Tier 1 rocket earns **Rocket Paperwork**.

The deployed rocket now has a narrow faceted white fuselage with red trim, a tapered red nose, a dark bell nozzle and four swept red fins. Its five-block assembly and saved block keys are unchanged. The three rocket shape presets keep rendering and collision consistent across clients (protocol 51).

Flight snapshots synchronize the phase, pilot, safe ground anchor and progress to joining players. Saved script flight records reconstruct a pending ascent or descent after loading; offline pilots pause. Equipment is returned once, after touchdown. Protocol 54 is required. `rocket_inflight`, `rocket_landing` and `rocket_cockpit` stage the actual flight renderer for visual checks.


Native machine controls: right-click the production machines to see actions, ingredient counts and progress. Sneak-right-click still places against them. Commands remain available; both controls use the same host-side scripts and recipes.
