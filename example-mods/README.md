# Example mods

Copy any of these folders into the game's `mods/` folder, enable it on the Mods screen, and open a new world or reopen your save. They can be installed together. Dedicated servers run their scripts; joining players receive the data and textures automatically. Scripts remain on the host.

| Folder | Example | Content |
| --- | --- | --- |
| [half_baked](half_baked/README.md) | Half-Baked Nuclear Machinery | Ores, refining, reactors, coolant, protective suits, shielding, two nukes and fallout |
| [dubiety](dubiety/README.md) | Dubiety | A tiny embedded word model, a tameable flying companion, terminal, private notes and state hints |
| [ad_absurdum](ad_absurdum/README.md) | Ad Absurdum | Four rocket tiers, five separate planet dimensions, materials, fuel, oxygen, suits, outposts and a supply rover |
| [dusk](dusk/README.md) | The Slightly Dim Forest | One isolated dimension, three biomes, four ruins, eight creatures, four progressive bosses and Moonmetal equipment |
| cheese | Cheese Mod | Basic custom blocks, items, textures, recipes and mob definitions |
| commands | Handy Commands | Sandboxed events, commands and persistent variables |
| digrace | Dig Race | Teams, scores, timers and a minigame |
| climate | Climate Fields | A climate-targeted inland biome and structure |
| pirate | Pirate | A translation |

The four parody packs use original names, pixel patterns, scripts and prose. They are examples for this engine, not ports or copies of the Minecraft mods. Their machinery uses chat commands and their rocket is a voxel assembly with a timed departure and dimension landing, rather than a physics-driven spacecraft. Dubiety uses word transitions rather than a neural language model. These choices keep the packs small and self-contained.

The new packs contain 66 declared blocks, 53 items, 67 recipes, 45 textures, 15 mobs, six dimensions, three biomes and ten structures. Slab/portal variants are extra generated registry entries. They require protocol 52 for planetary player gravity; the save format remains 16 and Overworld generation remains revision 12.

Four native achievements accompany their progression: Questionable Power, Rocket Paperwork, Pocket Philosopher, and Complaint Resolved. The usual Off the Map and Homebrew achievements also apply.

For a controlled render, install the packs and use `--screenshot out.png --mode half_baked_lab`, `astra_rocket`, `dusk_guardians`, or `dubiety_companion`. These stages inspect the real game models and textures; they are deliberately arranged displays.
