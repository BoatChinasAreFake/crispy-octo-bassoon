# Half-Baked Nuclear Machinery

An original parody inspired by [HBM's Nuclear Tech Mod](https://github.com/HbmMods/Hbm-s-Nuclear-Tech-GIT). The materials and machinery are fictional game abstractions.

The pack includes 11 blocks, 16 items, 20 shapeless recipes, ten original pixel textures, two Overworld ores and a loot-bearing research shed. It is optional; new ore and shed generation starts when this mod is installed, while existing chunks keep their edits.

## Start an industry

1. Mine Budget Copper Ore and smelt it. Mine Glowium Ore with a stone-tier pickaxe or better to collect dust.
2. Craft two Budget Steel Ingots from two Iron and one Coal. Build a Spin Doctor Centrifuge with six Steel, four Copper and two Zap Dust.
3. Stand within three blocks and run `/hbm refine`. Two Glowium Dust and one Coal produce one Spicy Fuel Pellet. It checks the machine and your actual inventory before consuming anything.
4. Build a Questionable Reactor, a Cool Story Tower and Coolant Canisters. The recipes are in `mod.txt` and the crafting palette.
5. Put the reactor and tower within three blocks of your operating position. `/hbm reactor` consumes one pellet and one coolant, then completes a 15-second batch: four Energy Cells and one Spent Nonsense. Collecting a cell earns **Questionable Power**.
6. Removing the tower during a batch causes a small meltdown and a one-minute radiation zone. `/hbm scram` cancels your nearby batch before dismantling; used fuel and coolant are lost. Removing the reactor cancels its batch. Up to sixteen batches may run; only one per reactor.

## Nukes

Craft a **Pocket Apocalypse** (four Steel, three pellets, eight Gunpowder), or upgrade it into **Management Decision** (eight Steel, six pellets, a Pocket Apocalypse and four Energy Cells). Place it on a surface.

Carry a Bad Idea Key and use `/hbm arm small` or `/hbm arm large` within three blocks. Arming consumes the key and starts a ten-second countdown, with broadcasts at the start and five seconds. `/hbm disarm` within four blocks cancels your countdown and returns its key. An unarmed device is harmless to mine; mining an armed device cancels its countdown.

The small nuke creates one radius-six blast. The large nuke creates nine bounded radius-eight blasts across a cluster, making a substantially larger crater. Both affect blocks, mobs and nearby players through the existing host-owned explosion system. They respect dimension separation: the same coordinates in another world are untouched. Bedrock and liquids retain the engine's ordinary explosion rules. These are game explosions, not simulations of real weapons.

A blast leaves a dimension-specific fallout zone for two minutes. Eight concurrent fuses and 64 recent zones are the limits. Fallout and carried spent waste increase a saved dose; sufficient dose causes damage. Full worn hazmat gear blocks new exposure, and a Very Heavy Shielding block intersecting the sampled line to fallout blocks that zone's exposure. Dose decays outside exposure.

Carry a Click Counter for `/hbm geiger`. `/hbm decon` requires a Portable Scrubber and one Coolant Canister; `/hbm tablet` consumes an Anti-Spicy Tablet and lowers dose. Crafting spent waste into a barrel removes it from your carried inventory. Barrels are decorative containment blocks, not chest inventories.

## Commands and persistence

`/hbm help`, `/hbm refine`, `/hbm reactor`, `/hbm status`, `/hbm scram`, `/hbm arm small|large`, `/hbm disarm`, `/hbm geiger`, `/hbm decon`, `/hbm tablet`.

Processing, fuses, fallout and doses are stored in this mod's saved variables. Machine batches and fuses pause while their chunks are unloaded and resume when those chunks return. On reopening a save, a still-present cooling tower is checked before processing continues. Fallout continues to count down while the world runs; nothing advances while the world is closed.

Machine buttons and chat commands use the same inventory transactions. There is no separate power grid or machine storage inventory. All multiplayer transactions and explosions run on the host.


Native machine controls: right-click the production machines to see actions, ingredient counts and progress. Sneak-right-click still places against them. Commands remain available; both controls use the same host-side scripts and recipes.
