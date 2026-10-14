# Dubiety

A friendly, original assistant parody inspired by the assistant concept in [Verity JE](https://www.curseforge.com/minecraft/mc-mods/verity-je). There are no horror states, punishments, secret world edits or outside connections.

The pack has a Terminal, three items, four recipes, four original textures and a tameable flying companion with a stepped yellow body and a single face.

## The embedded model

Dubiety uses an original **64-sentence, nine-topic trigram corpus**. The corpus function is 4,681 bytes of source. At world opening, it builds **525 two-word states and 558 weighted next-word transitions**; repeated transitions are the weights. Inference samples the next word from the current topic and two previous words, stopping at punctuation or 28 words. There are no neural weights, downloads, API keys, GPUs or new runtime dependencies.

Topic routing recognizes mining, food, building, combat, travel, nuclear machinery, space and the dusk forest; unmatched prompts use general conversation. Word generation can recombine shared phrases and can be awkward. This is deliberately a very small language model with limited knowledge, not a reasoning chatbot or an autonomous planner. Its game-state observations are separate, factual API reads.

## Meet the companion

Craft a Quite Small Thinking Chip from Gold, Zap Dust and a Book. Use a chip plus Iron and Glass to make a Terminal, or a chip plus two Glass to make a Companion Beacon. A Notebook gives a reminder of the note commands.

Carry the Beacon or stand within two blocks of a Terminal:

- `/dubiety ask <words>` produces a local word-model response. Low health and hunger add actual status hints.
- `/dubiety status` reports your dimension, health, food, inventory-entry count, model size and chat count.
- `/dubiety remember <note>` saves a private note, limited to 160 characters. Eight notes are retained; a ninth replaces the oldest.
- `/dubiety recall` lists your notes. Notes are indexed by player and remain private through ordinary multiplayer messaging.
- `/dubiety forget` clears your notes and the four-entry recent-prompt buffer.
- `/dubiety summon` consumes one chip to create a companion, with a thirty-second cooldown and nearby population checks. Offer another chip to tame it, then use the normal pet follow/sit interaction.

While carrying the Beacon, urgent health/hunger hints appear at most once every 45 seconds. The companion does not build, fight, manipulate your inventory or secretly learn from public chat. Recent prompts are a bounded journal; they are not added to the model's training corpus. Memory means persistent notes in this world, not cloud or cross-world memory.

Scripts and model inference run only on the host, including for joining players. The original corpus is embedded in `main.rhai`; notes and counters use saved mod variables. Obtaining a Beacon earns **Pocket Philosopher**.
