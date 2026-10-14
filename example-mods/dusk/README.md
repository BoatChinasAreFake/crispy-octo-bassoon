# The Slightly Dim Forest

An original adventure parody inspired by [The Twilight Forest](https://github.com/TeamTwilight/twilightforest). The entire forest uses **`dusk:twilit_realm`**, a separately generated and saved dimension. Its ores, biomes, ruins and scripted wildlife are restricted to that realm; installing it does not change Overworld terrain.

The pack has sixteen blocks, ten items, seven recipes, ten original pixel textures, three biomes, four ruin templates and eight creature types, including four bosses. The custom deer has antlers and animated legs; the serpent and three-headed final boss use custom box models.

## Enter and explore

Craft four Duskwood Gate Frames from two Logs, two Moss Blocks and one Dimond. Build the ordinary portal frame shape from these blocks and light it with a Sparker. The generated return portal brings you home through the same portal system.

Duskwood Groves have tall trees and glowing Firefly Reeds. Polite Thorn Thickets have thicker scrub; Moonlit Glades have berries and fewer trees. Harvest Duskberries for food, make Firefly Jars and mine Moonmetal to smelt into stronger pickaxes and swords. Passive Oh Deer and Flying Reading Lamps share the realm with hostile Pocket Accountants and Thorny Issues. Spawning is bounded near players and stops when nobody is visiting.

## Guardians

| Order | Guardian | Ruin | Fight | Trophy |
| --- | --- | --- | --- | --- |
| 1 | The Soggy Serpent | Soggy Courtyard | A melee serpent with an enraged phase | Soggy Scale |
| 2 | The Overdue Librarian | Overdue Library | Ranged rune volleys, melee and summoned accountants | Overdue Crown |
| 3 | Minotax | Minotax Maze | Heavy melee and a faster enraged charge | Tax-Deductible Horn |
| 4 | The Complaint Department | Complaint Mound | Three heads, blast volleys and summoned thornlings | Complaint Department Heart |

`/dusk where serpent|lich|minotax|hydra` surveys for the relevant ruin within 96 chunks of your current forest position. Walk there; `/dusk challenge <guardian>` within four blocks of its altar starts the fight. Trying to mine an altar also starts its challenge and leaves the altar in place.

Boss encounters check your saved guardian order before spawning. One encounter of each type can be active at a time. The challenger and a different player who lands the finishing blow both receive progression, so co-op does not strand the original challenger. Actual drops follow the ordinary boss/drop system. Collecting the final heart earns **Complaint Resolved**.

`/dusk journal` shows your saved completion count. Defeating a guardian unlocks the next challenge; ordinary movement through later biomes remains open. This is encounter progression, not an implementation of every original Twilight Forest biome barrier. Ruins use the engine's layer templates, rather than procedural mega-dungeons.

Commands: `/dusk help`, `/dusk where <guardian>`, `/dusk challenge <guardian>`, `/dusk journal`.

Guardians, tamed/named creatures, trophies, progression and active encounter records save with the world. Unloaded encounters keep their lock; a loaded, empty encounter area clears a stale lock after a minute. No horror or sleep-harassment mechanics are included.
