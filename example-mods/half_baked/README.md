# Half-Baked Nuclear Machinery

An original parody inspired by [HBM's Nuclear Tech Mod](https://github.com/HbmMods/Hbm-s-Nuclear-Tech-GIT). The materials and machinery are fictional game abstractions.

The pack includes 12 blocks, 17 items, 22 shapeless recipes, ten original pixel textures, two Overworld ores and a loot-bearing research shed. It is optional; new ore and shed generation starts when this mod is installed, while existing chunks keep their edits.

## Start an industry

1. Mine Budget Copper Ore and smelt it. Mine Glowium Ore with a stone-tier pickaxe or better to collect dust.
2. Craft two Budget Steel Ingots from two Iron and one Coal. Build a Spin Doctor Centrifuge with six Steel, four Copper and two Zap Dust.
3. Open the centrifuge. Put two Glowium Dust in Input and Coal in the secondary slot. Supply 32 energy: a Coal in Power generates 80 starter energy, so this pack can bootstrap without another mod. Later use charged cells or a wired battery/solar source. Run once makes one Spicy Fuel Pellet in four seconds of powered work. Auto repeats while stocked.
4. Build a Questionable Reactor, a Cool Story Tower and Coolant Canisters. Place the tower within three blocks horizontally and two vertically of the reactor. Load a pellet and coolant into the reactor's input slots.
5. Run a 15-second batch for four Energy Cells and one Spent Nonsense. Reactor batches do not need electric input. Full outputs pause safely. Removing cooling scrams a native batch without consuming its unfinished inputs; Stop clears partial work. Existing paid script batches imported from an older save retain their original meltdown rules until finished or scrammed by their operator.
6. Build an Industrial Battery and cables. A charged Energy Cell stores 240 energy and leaves a reusable Discharged Energy Cell. Banks store up to 1,000 energy, supply connected electric machines, and collect solar power when Ad Absurdum is installed. Recharge empty cells in a bank. Native processing and power work with Half Baked installed alone.

## Nukes

Craft a **Pocket Apocalypse** (four Steel, three pellets, eight Gunpowder), or upgrade it into **Management Decision** (eight Steel, six pellets, a Pocket Apocalypse and four Energy Cells). Place it on a surface.

Carry a Bad Idea Key and use `/hbm arm small` or `/hbm arm large` within three blocks. Arming consumes the key and starts a ten-second countdown, with broadcasts at the start and five seconds. `/hbm disarm` within four blocks cancels your countdown and returns its key. An unarmed device is harmless to mine; mining an armed device cancels its countdown.

The small nuke creates one radius-six blast. The large nuke creates nine bounded radius-eight blasts across a cluster, making a substantially larger crater. Both affect blocks, mobs and nearby players through the existing host-owned explosion system. They respect dimension separation: the same coordinates in another world are untouched. Bedrock and liquids retain the engine's ordinary explosion rules. These are game explosions, not simulations of real weapons.

A blast leaves a dimension-specific fallout zone for two minutes. Eight concurrent fuses and 64 recent zones are the limits. Fallout and carried spent waste increase a saved dose; sufficient dose causes damage. Full worn hazmat gear blocks new exposure, and a Very Heavy Shielding block intersecting the sampled line to fallout blocks that zone's exposure. Dose decays outside exposure.

Carry a Click Counter for `/hbm geiger`. `/hbm decon` requires a Portable Scrubber and one Coolant Canister; `/hbm tablet` consumes an Anti-Spicy Tablet and lowers dose. Crafting spent waste into a barrel removes it from your carried inventory. Barrels are decorative containment blocks, not chest inventories.

## Commands and persistence

`/hbm help`, `/hbm refine`, `/hbm reactor`, `/hbm status`, `/hbm scram`, `/hbm arm small|large`, `/hbm disarm`, `/hbm geiger`, `/hbm decon`, `/hbm tablet`.

Native processing, slots and energy are saved with placed containers; legacy jobs, fuses, fallout and doses are stored in the mod's saved variables. Machine batches and fuses pause while their chunks are unloaded and resume when those chunks return. On reopening a save, a still-present cooling tower is checked before processing continues. Fallout continues to count down while the world runs; nothing advances while the world is closed.

Machine buttons and production commands use the machine's saved inventory. Hoppers feed inputs from above/sides and extract outputs, waste and empty shells from below. Control mode, partial work and energy persist; unloaded chunks pause. All multiplayer transactions and explosions run on the host.


Native machine controls: right-click for six item slots, processing/energy gauges and Run once / Stop / Auto. Sneak-right-click places against machines. `/hbm refine`, `/hbm reactor`, `/hbm scram` and `/hbm status` control nearby native production; they no longer pay from the player's bag. Nukes, fallout and protection still use the script systems.
